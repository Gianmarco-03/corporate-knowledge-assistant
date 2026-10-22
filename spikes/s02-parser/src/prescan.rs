//! Prescan anti-bomb di PDF e DOCX, nel figlio prima di PDFium (contratto W17, punto 5).
//!
//! Gira nel figlio isolato, sotto i suoi limiti: anche il prescan legge input ostile.
//!
//! PDF, nell'ordine:
//! 1. scansione a byte con nomi normalizzati (escape `#xx` risolti), prima di ogni
//!    caricamento: `/Encrypt` → `Encrypted`; numero di `/ObjStm` oltre il tetto →
//!    `DecompressionLimit` (un object stream non sta dentro un altro: il conteggio è completo);
//! 2. caricamento con `LoadOptions::with_max_decompressed_size(per_stream)`: il tetto per
//!    stream agisce già sugli object stream espansi dal loader. In modalità tollerante lopdf
//!    0.45.0 scarta in silenzio un object stream oltre il tetto (lo registra solo nel log),
//!    quindi si carica prima in modalità `strict`, dove l'errore
//!    `Error::Decompress(DecompressError::MemoryLimitExceeded)` arriva al chiamante
//!    (→ `DecompressionLimit`); un altro errore stretto ripiega sul caricamento tollerante,
//!    che deve comunque contenere ogni object stream citato dalla xref (altrimenti `Corrupt`);
//! 3. dopo il caricamento: cifratura, pagine (tetto → `ResourceLimit`), catene di filtri,
//!    decodifica in streaming dei soli Flate con `take(cap + 1)` senza trattenere byte, tetti
//!    per stream, totale (l'espansione del loader stimata come object stream × tetto per
//!    stream) e rapporto; poi i contenuti attivi → quarantena con motivi fissi.
//!
//! DOCX: voci, somma dichiarata, macro, `<!DOCTYPE`/`<!ENTITY`, relazioni esterne.
use std::io::{self, Read};
use std::path::Path;

use lopdf::{Document, LoadOptions, Object};

use crate::protocol::Kind;

const MIB: u64 = 1024 * 1024;

/// Motivi di quarantena (stringhe fisse del contratto, nell'ordine di emissione).
pub const REASON_JAVASCRIPT: &str = "javascript";
pub const REASON_LAUNCH: &str = "launch";
pub const REASON_EMBEDDED_FILE: &str = "embedded_file";
pub const REASON_OPEN_ACTION_JS: &str = "open_action_js";
pub const REASON_EXTERNAL_LINK: &str = "external_link";
const REASON_ORDER: [&str; 5] = [
    REASON_JAVASCRIPT,
    REASON_LAUNCH,
    REASON_EMBEDDED_FILE,
    REASON_OPEN_ACTION_JS,
    REASON_EXTERNAL_LINK,
];

/// Tetti del prescan (Soglie del lanciatore, Task 1 di 01-07).
#[derive(Debug, Clone)]
pub struct Caps {
    /// Tetto per stream, anche al caricamento (`max_decompressed_size`): 64 MiB.
    pub per_stream_bytes: u64,
    /// Totale decompresso, caricamento compreso: 512 MiB.
    pub total_bytes: u64,
    /// Rapporto massimo di espansione: 100.
    pub max_ratio: u64,
    /// Il rapporto vale oltre questa soglia di byte espansi: 16 MiB.
    pub ratio_min_bytes: u64,
    /// Object stream ammessi (2 × massimo del corpus benigno).
    pub max_object_streams: u32,
    /// Lunghezza massima della catena di filtri: 2.
    pub max_filter_chain: usize,
    /// Limite di pagine: 2.000.
    pub max_pages: u32,
    /// Tetto `Width × Height × componenti` dei filtri immagine: 64 MiB.
    pub max_image_bytes: u64,
    /// Voci DOCX ammesse: 1.000.
    pub docx_max_entries: usize,
    /// Somma dichiarata delle voci DOCX: 200 MiB.
    pub docx_max_total_bytes: u64,
}

/// Object stream ammessi: 2 × massimo del corpus benigno misurato dalla calibrazione
/// (Task 4, `results/isolation/calibration.csv`, colonna `objstm_count`).
pub const MAX_OBJECT_STREAMS: u32 = 64;

impl Default for Caps {
    fn default() -> Self {
        Caps {
            per_stream_bytes: 64 * MIB,
            total_bytes: 512 * MIB,
            max_ratio: 100,
            ratio_min_bytes: 16 * MIB,
            max_object_streams: MAX_OBJECT_STREAMS,
            max_filter_chain: 2,
            max_pages: 2000,
            max_image_bytes: 64 * MIB,
            docx_max_entries: 1000,
            docx_max_total_bytes: 200 * MIB,
        }
    }
}

/// Esito positivo del prescan, con le grandezze che la calibrazione registra.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PrescanReport {
    /// Pagine (PDF); `None` per i DOCX.
    pub pages: Option<u32>,
    /// `/ObjStm` contati a byte prima del caricamento.
    pub objstm_count: u32,
    /// Rapporto massimo espanso/compresso fra gli stream Flate.
    pub max_flate_ratio: f64,
    /// Massimo di byte espansi da un singolo stream.
    pub max_stream_bytes: u64,
    /// Byte espansi in totale, stima del caricamento compresa.
    pub total_decompressed_bytes: u64,
    /// Voci dell'archivio (DOCX).
    pub entries: usize,
}

/// Rifiuto tipizzato del prescan.
#[derive(Debug, Clone, PartialEq)]
pub enum Rejection {
    Failed(Kind, String),
    Quarantined(Vec<String>),
}

fn fail(kind: Kind, detail: impl Into<String>) -> Rejection {
    Rejection::Failed(kind, detail.into())
}

// ---- scansione a byte ------------------------------------------------------------------

fn is_delimiter(b: u8) -> bool {
    matches!(
        b,
        b'(' | b')' | b'<' | b'>' | b'[' | b']' | b'{' | b'}' | b'/' | b'%'
    )
}

fn is_white(b: u8) -> bool {
    matches!(b, 0 | 9 | 10 | 12 | 13 | 32)
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

/// Nomi PDF interessanti trovati nei byte grezzi.
#[derive(Debug, Default)]
struct ByteScan {
    encrypt: bool,
    objstm: u32,
    javascript: bool,
    launch: bool,
    embedded_file: bool,
}

/// Intervalli dei dati degli stream (`stream` EOL … `endstream`), dove i nomi brevi non
/// contano (byte compressi: un `/JS` casuale non è un'azione).
fn stream_bodies(bytes: &[u8]) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(off) = find(&bytes[i..], b"stream") {
        let at = i + off;
        let after = at + 6;
        let is_end = at >= 3 && &bytes[at - 3..at] == b"end";
        let eol = match bytes.get(after) {
            Some(b'\n') => Some(after + 1),
            Some(b'\r') => Some(if bytes.get(after + 1) == Some(&b'\n') {
                after + 2
            } else {
                after + 1
            }),
            _ => None,
        };
        match (is_end, eol) {
            (false, Some(start)) => {
                let end = find(&bytes[start..], b"endstream").map_or(bytes.len(), |o| start + o);
                out.push((start, end));
                i = end.max(after);
            }
            _ => i = after,
        }
        if i >= bytes.len() {
            break;
        }
    }
    out
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    if needle.is_empty() || hay.len() < needle.len() {
        return None;
    }
    hay.windows(needle.len()).position(|w| w == needle)
}

fn scan_bytes(bytes: &[u8]) -> ByteScan {
    let bodies = stream_bodies(bytes);
    let in_body = |pos: usize| bodies.iter().any(|&(s, e)| pos >= s && pos < e);
    let mut out = ByteScan::default();
    let mut name = Vec::with_capacity(32);
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'/' {
            i += 1;
            continue;
        }
        let start = i;
        i += 1;
        name.clear();
        while i < bytes.len() && !is_white(bytes[i]) && !is_delimiter(bytes[i]) && name.len() < 64 {
            if bytes[i] == b'#'
                && let (Some(h), Some(l)) = (
                    bytes.get(i + 1).copied().and_then(hex_val),
                    bytes.get(i + 2).copied().and_then(hex_val),
                )
            {
                name.push(h << 4 | l);
                i += 3;
                continue;
            }
            name.push(bytes[i]);
            i += 1;
        }
        match name.as_slice() {
            b"Encrypt" => out.encrypt = true,
            b"ObjStm" => out.objstm = out.objstm.saturating_add(1),
            b"JavaScript" => out.javascript = true,
            b"Launch" => out.launch = true,
            b"EmbeddedFile" | b"EmbeddedFiles" => out.embedded_file = true,
            b"JS" if !in_body(start) => out.javascript = true,
            _ => {}
        }
    }
    out
}

// ---- caricamento -----------------------------------------------------------------------

fn is_memory_limit(e: &lopdf::Error) -> bool {
    matches!(
        e,
        lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })
    )
}

fn load(bytes: &[u8], caps: &Caps) -> Result<Document, Rejection> {
    let limit = usize::try_from(caps.per_stream_bytes).unwrap_or(usize::MAX);
    let strict = LoadOptions {
        strict: true,
        ..LoadOptions::with_max_decompressed_size(limit)
    };
    match Document::load_mem_with_options(bytes, strict) {
        Ok(doc) => Ok(doc),
        Err(e) if is_memory_limit(&e) => Err(fail(
            Kind::DecompressionLimit,
            format!("load_per_stream: {e}"),
        )),
        Err(_) => {
            let lenient = LoadOptions::with_max_decompressed_size(limit);
            let doc = Document::load_mem_with_options(bytes, lenient).map_err(|e| {
                if is_memory_limit(&e) {
                    fail(Kind::DecompressionLimit, format!("load_per_stream: {e}"))
                } else {
                    fail(Kind::Corrupt, format!("load: {e}"))
                }
            })?;
            // Il caricamento tollerante salta un object stream che non decodifica (anche
            // per il tetto): ogni contenitore citato dalla xref deve esserci.
            let missing = doc.reference_table.entries.values().any(|entry| {
                matches!(entry, lopdf::xref::XrefEntry::Compressed { container, .. }
                    if !matches!(doc.objects.get(&(*container, 0)), Some(Object::Stream(_))))
            });
            if missing {
                return Err(fail(Kind::Corrupt, "objstm_unloadable"));
            }
            Ok(doc)
        }
    }
}

// ---- filtri e decodifica ---------------------------------------------------------------

fn filters(dict: &lopdf::Dictionary) -> Result<Vec<Vec<u8>>, Rejection> {
    match dict.get(b"Filter") {
        Err(_) => Ok(Vec::new()),
        Ok(Object::Name(n)) => Ok(vec![n.clone()]),
        Ok(Object::Array(a)) => a
            .iter()
            .map(|o| match o {
                Object::Name(n) => Ok(n.clone()),
                _ => Err(fail(Kind::Corrupt, "filter_not_name")),
            })
            .collect(),
        Ok(Object::Null) => Ok(Vec::new()),
        Ok(_) => Err(fail(Kind::Corrupt, "filter_not_name")),
    }
}

const IMAGE_FILTERS: [&[u8]; 4] = [
    b"DCTDecode",
    b"JPXDecode",
    b"JBIG2Decode",
    b"CCITTFaxDecode",
];

fn is_image(dict: &lopdf::Dictionary) -> bool {
    matches!(dict.get(b"Subtype"), Ok(Object::Name(n)) if n == b"Image")
}

fn int(dict: &lopdf::Dictionary, key: &[u8]) -> Option<u64> {
    dict.get(key)
        .ok()
        .and_then(|o| o.as_i64().ok())
        .and_then(|v| u64::try_from(v).ok())
}

/// Byte dichiarati da un'immagine: `Width × Height × componenti` (× 2 a 16 bit).
fn image_bytes(dict: &lopdf::Dictionary) -> Option<u64> {
    let w = int(dict, b"Width")?;
    let h = int(dict, b"Height")?;
    let comps = match dict.get(b"ColorSpace") {
        Ok(Object::Name(n)) if n == b"DeviceGray" || n == b"CalGray" => 1,
        Ok(Object::Name(n)) if n == b"DeviceRGB" || n == b"CalRGB" || n == b"Lab" => 3,
        _ => 4,
    };
    let depth = if int(dict, b"BitsPerComponent") == Some(16) {
        2
    } else {
        1
    };
    w.checked_mul(h)?.checked_mul(comps)?.checked_mul(depth)
}

/// Decodifica ASCIIHex (riduce: il risultato non supera l'input).
fn ascii_hex(data: &[u8]) -> Result<Vec<u8>, Rejection> {
    let mut out = Vec::with_capacity(data.len() / 2 + 1);
    let mut hi: Option<u8> = None;
    for &b in data {
        if b == b'>' {
            break;
        }
        if is_white(b) {
            continue;
        }
        let v = hex_val(b).ok_or_else(|| fail(Kind::Corrupt, "ascii_hex"))?;
        match hi.take() {
            Some(h) => out.push(h << 4 | v),
            None => hi = Some(v),
        }
    }
    if let Some(h) = hi {
        out.push(h << 4);
    }
    Ok(out)
}

/// Decodifica ASCII85 (riduce: 5 caratteri → 4 byte, `z` → 4 byte).
fn ascii85(data: &[u8]) -> Result<Vec<u8>, Rejection> {
    let mut out = Vec::with_capacity(data.len());
    let mut group = [0u32; 5];
    let mut n = 0usize;
    let mut i = 0;
    while i < data.len() {
        let b = data[i];
        i += 1;
        if b == b'~' {
            break;
        }
        if is_white(b) {
            continue;
        }
        if b == b'z' && n == 0 {
            out.extend_from_slice(&[0, 0, 0, 0]);
            continue;
        }
        if !(b'!'..=b'u').contains(&b) {
            return Err(fail(Kind::Corrupt, "ascii85"));
        }
        group[n] = u32::from(b - b'!');
        n += 1;
        if n == 5 {
            let v = group
                .iter()
                .try_fold(0u32, |acc, d| acc.checked_mul(85)?.checked_add(*d))
                .ok_or_else(|| fail(Kind::Corrupt, "ascii85"))?;
            out.extend_from_slice(&v.to_be_bytes());
            n = 0;
        }
    }
    if n > 1 {
        for d in group.iter_mut().skip(n) {
            *d = 84;
        }
        let v = group
            .iter()
            .try_fold(0u32, |acc, d| acc.checked_mul(85)?.checked_add(*d))
            .ok_or_else(|| fail(Kind::Corrupt, "ascii85"))?;
        out.extend_from_slice(&v.to_be_bytes()[..n - 1]);
    }
    Ok(out)
}

/// Byte prodotti da `r` fino a `cap + 1` (letti in un buffer fisso e scartati). Un errore
/// di decodifica chiude il conteggio ai byte già prodotti: non è una bomba.
fn drain_counted<R: Read>(r: R, cap: u64) -> u64 {
    let mut limited = r.take(cap.saturating_add(1));
    let mut buf = [0u8; 16 * 1024];
    let mut n = 0u64;
    loop {
        match limited.read(&mut buf) {
            Ok(0) | Err(_) => return n,
            Ok(k) => n += k as u64,
        }
    }
}

/// Espansione Flate di uno stream secondo la sua catena di filtri (già validata).
/// I filtri prima del primo Flate riducono e si applicano sul buffer; Flate annidati si
/// leggono in streaming; i filtri dopo l'ultimo Flate non espandono e non si decodificano.
fn flate_expansion(chain: &[Vec<u8>], raw: &[u8], cap: u64) -> Result<Option<u64>, Rejection> {
    let Some(first_flate) = chain.iter().position(|f| f == b"FlateDecode") else {
        return Ok(None);
    };
    let mut owned: Option<Vec<u8>> = None;
    for f in &chain[..first_flate] {
        let cur: &[u8] = owned.as_deref().unwrap_or(raw);
        let next = match f.as_slice() {
            b"ASCIIHexDecode" => ascii_hex(cur)?,
            b"ASCII85Decode" => ascii85(cur)?,
            _ => return Ok(None),
        };
        owned = Some(next);
    }
    let data: &[u8] = owned.as_deref().unwrap_or(raw);
    let flates = chain[first_flate..]
        .iter()
        .take_while(|f| f.as_slice() == b"FlateDecode")
        .count();
    let n = if flates >= 2 {
        let inner = flate2::read::ZlibDecoder::new(data).take(cap.saturating_add(1));
        drain_counted(flate2::read::ZlibDecoder::new(inner), cap)
    } else {
        drain_counted(flate2::read::ZlibDecoder::new(data), cap)
    };
    Ok(Some(n))
}

/// Verifica la catena di filtri di uno stream secondo le Soglie.
fn check_chain(chain: &[Vec<u8>], dict: &lopdf::Dictionary, caps: &Caps) -> Result<(), Rejection> {
    if chain.len() > caps.max_filter_chain {
        return Err(fail(
            Kind::DecompressionLimit,
            format!(
                "filter_chain {} oltre {}",
                chain.len(),
                caps.max_filter_chain
            ),
        ));
    }
    for (i, f) in chain.iter().enumerate() {
        match f.as_slice() {
            b"FlateDecode" | b"ASCIIHexDecode" | b"ASCII85Decode" => {}
            f if IMAGE_FILTERS.contains(&f) => {
                if !is_image(dict) || i + 1 != chain.len() {
                    return Err(fail(Kind::Unsupported, "image_filter_on_non_image"));
                }
                let declared = image_bytes(dict)
                    .ok_or_else(|| fail(Kind::Unsupported, "image_without_size"))?;
                if declared > caps.max_image_bytes {
                    return Err(fail(
                        Kind::DecompressionLimit,
                        format!("image_declared {declared} oltre {}", caps.max_image_bytes),
                    ));
                }
            }
            other => {
                return Err(fail(
                    Kind::Unsupported,
                    format!("filter {}", String::from_utf8_lossy(other)),
                ));
            }
        }
    }
    Ok(())
}

/// Motivi di quarantena trovati negli oggetti caricati (object stream compresi).
#[derive(Debug, Default)]
struct Active {
    javascript: bool,
    launch: bool,
    embedded_file: bool,
    open_action_js: bool,
    external_link: bool,
}

impl Active {
    fn reasons(&self) -> Vec<String> {
        let flags = [
            self.javascript,
            self.launch,
            self.embedded_file,
            self.open_action_js,
            self.external_link,
        ];
        REASON_ORDER
            .iter()
            .zip(flags)
            .filter(|(_, f)| *f)
            .map(|(r, _)| (*r).to_owned())
            .collect()
    }
}

fn walk(o: &Object, a: &mut Active, depth: u32) {
    if depth > 64 {
        return;
    }
    match o {
        Object::Name(n) => match n.as_slice() {
            b"JavaScript" => a.javascript = true,
            b"Launch" => a.launch = true,
            b"EmbeddedFile" | b"EmbeddedFiles" => a.embedded_file = true,
            _ => {}
        },
        Object::Dictionary(d) => walk_dict(d, a, depth),
        Object::Stream(s) => walk_dict(&s.dict, a, depth),
        Object::Array(v) => v.iter().for_each(|x| walk(x, a, depth + 1)),
        _ => {}
    }
}

fn walk_dict(d: &lopdf::Dictionary, a: &mut Active, depth: u32) {
    for (k, v) in d.iter() {
        match k.as_slice() {
            b"JS" | b"JavaScript" => a.javascript = true,
            b"EF" | b"EmbeddedFiles" => a.embedded_file = true,
            b"Launch" => a.launch = true,
            _ => {}
        }
        walk(v, a, depth + 1);
    }
}

fn is_js_action(doc: &Document, o: &Object) -> bool {
    let Ok((_, o)) = doc.dereference(o) else {
        return false;
    };
    let Ok(d) = o.as_dict() else {
        return false;
    };
    d.has(b"JS") || matches!(d.get(b"S"), Ok(Object::Name(n)) if n == b"JavaScript")
}

/// Prescan di un PDF tenuto in memoria. Va chiamato nel figlio, prima di PDFium.
pub fn check_pdf(bytes: &[u8], caps: &Caps) -> Result<PrescanReport, Rejection> {
    // 1. Prima del caricamento.
    let scan = scan_bytes(bytes);
    if scan.encrypt {
        return Err(fail(Kind::Encrypted, "encrypt_in_trailer"));
    }
    if scan.objstm > caps.max_object_streams {
        return Err(fail(
            Kind::DecompressionLimit,
            format!(
                "objstm_count={} oltre {}",
                scan.objstm, caps.max_object_streams
            ),
        ));
    }
    let mut total = u64::from(scan.objstm).saturating_mul(caps.per_stream_bytes);
    if total > caps.total_bytes {
        return Err(fail(
            Kind::DecompressionLimit,
            format!(
                "total_load_estimate objstm_count={} × per_stream oltre {}",
                scan.objstm, caps.total_bytes
            ),
        ));
    }

    // 2. Caricamento con tetto per stream.
    let doc = load(bytes, caps)?;

    // 3. Dopo il caricamento.
    if doc.is_encrypted() || doc.was_encrypted() {
        return Err(fail(Kind::Encrypted, "encrypted_document"));
    }
    let pages = doc.page_iter().take(caps.max_pages as usize + 1).count();
    if pages == 0 {
        return Err(fail(Kind::Corrupt, "no_pages"));
    }
    if pages > caps.max_pages as usize {
        return Err(fail(
            Kind::ResourceLimit,
            format!("pages oltre {}", caps.max_pages),
        ));
    }

    let mut report = PrescanReport {
        pages: Some(u32::try_from(pages).unwrap_or(u32::MAX)),
        objstm_count: scan.objstm,
        total_decompressed_bytes: total,
        ..PrescanReport::default()
    };
    let mut active = Active {
        javascript: scan.javascript,
        launch: scan.launch,
        embedded_file: scan.embedded_file,
        ..Active::default()
    };
    for obj in doc.objects.values() {
        walk(obj, &mut active, 0);
        let Object::Stream(stream) = obj else {
            continue;
        };
        let chain = filters(&stream.dict)?;
        check_chain(&chain, &stream.dict, caps)?;
        let Some(n) = flate_expansion(&chain, &stream.content, caps.per_stream_bytes)? else {
            continue;
        };
        if n > caps.per_stream_bytes {
            return Err(fail(
                Kind::DecompressionLimit,
                format!("per_stream oltre {}", caps.per_stream_bytes),
            ));
        }
        let raw = stream.content.len().max(1) as u64;
        let ratio = n as f64 / raw as f64;
        if n > caps.ratio_min_bytes && n > raw.saturating_mul(caps.max_ratio) {
            return Err(fail(
                Kind::DecompressionLimit,
                format!("ratio {ratio:.0} oltre {} su {n} byte", caps.max_ratio),
            ));
        }
        // L'espansione degli object stream è già nella stima del caricamento.
        if !stream.dict.has_type(b"ObjStm") {
            total = total.saturating_add(n);
        }
        if total > caps.total_bytes {
            return Err(fail(
                Kind::DecompressionLimit,
                format!("total oltre {}", caps.total_bytes),
            ));
        }
        report.max_stream_bytes = report.max_stream_bytes.max(n);
        if ratio > report.max_flate_ratio {
            report.max_flate_ratio = ratio;
        }
    }
    report.total_decompressed_bytes = total;

    if let Ok(cat) = doc.catalog()
        && let Ok(oa) = cat.get(b"OpenAction")
        && is_js_action(&doc, oa)
    {
        active.open_action_js = true;
    }
    let reasons = active.reasons();
    if !reasons.is_empty() {
        return Err(Rejection::Quarantined(reasons));
    }
    Ok(report)
}

// ---- DOCX ------------------------------------------------------------------------------

/// Esito della lettura in streaming di una parte XML.
#[derive(Debug, Default)]
struct PartScan {
    bytes: u64,
    doctype: bool,
    entity: bool,
    macro_ct: bool,
    external: bool,
}

/// `TargetMode = "External"` (spazi e apici ammessi) nel testo minuscolo.
fn has_external_target(lower: &[u8]) -> bool {
    let mut from = 0;
    while let Some(off) = find(&lower[from..], b"targetmode") {
        let mut i = from + off + b"targetmode".len();
        while lower.get(i).is_some_and(|b| is_white(*b)) {
            i += 1;
        }
        if lower.get(i) == Some(&b'=') {
            i += 1;
            while lower.get(i).is_some_and(|b| is_white(*b)) {
                i += 1;
            }
            if matches!(lower.get(i), Some(b'"' | b'\'')) && lower[i + 1..].starts_with(b"external")
            {
                return true;
            }
        }
        from = from + off + 1;
    }
    false
}

/// Legge una parte con `take(declared + 1)` a blocchi, cercando i marcatori a cavallo dei
/// blocchi; nessuna parte è trattenuta intera.
fn scan_part<R: Read>(r: R, declared: u64) -> io::Result<PartScan> {
    const OVERLAP: usize = 64;
    let mut limited = r.take(declared.saturating_add(1));
    let mut out = PartScan::default();
    let mut window: Vec<u8> = Vec::with_capacity(64 * 1024 + OVERLAP);
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = limited.read(&mut buf)?;
        if n == 0 {
            break;
        }
        out.bytes += n as u64;
        let keep = window.len().min(OVERLAP);
        window.drain(..window.len() - keep);
        window.extend(buf[..n].iter().map(u8::to_ascii_lowercase));
        out.doctype |= find(&window, b"<!doctype").is_some();
        out.entity |= find(&window, b"<!entity").is_some();
        out.macro_ct |= find(&window, b"macroenabled").is_some();
        out.external |= has_external_target(&window);
    }
    Ok(out)
}

fn zip_err(e: zip::result::ZipError) -> Rejection {
    match e {
        zip::result::ZipError::UnsupportedArchive(m) => {
            fail(Kind::Unsupported, format!("zip: {m}"))
        }
        zip::result::ZipError::CompressionMethodNotSupported(m) => {
            fail(Kind::Unsupported, format!("zip_method {m}"))
        }
        other => fail(Kind::Corrupt, format!("zip: {other}")),
    }
}

/// Prescan di un DOCX su file. Va chiamato nel figlio, prima di leggere il documento.
pub fn check_docx(path: &Path, caps: &Caps) -> Result<PrescanReport, Rejection> {
    let file = std::fs::File::open(path).map_err(|e| fail(Kind::Corrupt, format!("open: {e}")))?;
    let mut z = zip::ZipArchive::new(file).map_err(|e| fail(Kind::Corrupt, format!("zip: {e}")))?;
    if z.len() > caps.docx_max_entries {
        return Err(fail(
            Kind::DecompressionLimit,
            format!("docx_entries {} oltre {}", z.len(), caps.docx_max_entries),
        ));
    }
    let mut parts: Vec<(String, u64)> = Vec::with_capacity(z.len());
    let mut declared = 0u64;
    for i in 0..z.len() {
        let e = z.by_index_raw(i).map_err(zip_err)?;
        if e.encrypted() {
            return Err(fail(Kind::Unsupported, "zip_encrypted_entry"));
        }
        let name = e.name().map_err(zip_err)?.to_ascii_lowercase();
        declared = declared.saturating_add(e.size());
        parts.push((name, e.size()));
    }
    if declared > caps.docx_max_total_bytes {
        return Err(fail(
            Kind::DecompressionLimit,
            format!(
                "docx_declared_total {declared} oltre {}",
                caps.docx_max_total_bytes
            ),
        ));
    }
    if parts.iter().any(|(n, _)| n.ends_with("vbaproject.bin")) {
        return Err(fail(Kind::Unsupported, "vba_project"));
    }

    let mut external = false;
    let mut actual = 0u64;
    for (i, (name, size)) in parts.iter().enumerate() {
        let is_rels = name.ends_with(".rels");
        if !(is_rels || name.ends_with(".xml")) {
            continue;
        }
        let entry = z.by_index(i).map_err(zip_err)?;
        let scan =
            scan_part(entry, *size).map_err(|e| fail(Kind::Corrupt, format!("zip_read: {e}")))?;
        if scan.bytes > *size {
            return Err(fail(Kind::DecompressionLimit, "docx_size_mismatch"));
        }
        actual = actual.saturating_add(scan.bytes);
        if actual > caps.docx_max_total_bytes {
            return Err(fail(Kind::DecompressionLimit, "docx_total"));
        }
        if scan.doctype {
            return Err(fail(Kind::Corrupt, "xml_doctype"));
        }
        if scan.entity {
            return Err(fail(Kind::Corrupt, "xml_entity"));
        }
        if name == "[content_types].xml" && scan.macro_ct {
            return Err(fail(Kind::Unsupported, "macro_content_type"));
        }
        external |= is_rels && scan.external;
    }
    if external {
        return Err(Rejection::Quarantined(vec![
            REASON_EXTERNAL_LINK.to_owned(),
        ]));
    }
    Ok(PrescanReport {
        pages: None,
        total_decompressed_bytes: actual,
        entries: parts.len(),
        ..PrescanReport::default()
    })
}

#[cfg(test)]
mod tests {
    //! TEST-7 e TEST-8: rifiuti del prescan prima di PDFium, con PDF e DOCX costruiti a mano.
    use super::*;
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU32, Ordering};

    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;

    // ---- costruzione dei PDF ----------------------------------------------------------

    /// PDF con tabella xref classica; l'oggetto 1 è il Catalog.
    fn build(objs: &[(u32, Vec<u8>)], trailer_extra: &str) -> Vec<u8> {
        let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let size = objs.iter().map(|(n, _)| *n).max().unwrap_or(0) + 1;
        let mut offsets = vec![None; size as usize];
        for (n, body) in objs {
            offsets[*n as usize] = Some(out.len());
            out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
            out.extend_from_slice(body);
            out.extend_from_slice(b"\nendobj\n");
        }
        let xref = out.len();
        out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
        for off in offsets.iter().skip(1) {
            match off {
                Some(o) => out.extend_from_slice(format!("{o:010} 00000 n \n").as_bytes()),
                None => out.extend_from_slice(b"0000000000 65535 f \n"),
            }
        }
        out.extend_from_slice(
            format!("trailer\n<< /Size {size} /Root 1 0 R {trailer_extra} >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
        );
        out
    }

    fn dict(s: &str) -> Vec<u8> {
        s.as_bytes().to_vec()
    }

    fn stream(dict_extra: &str, data: &[u8]) -> Vec<u8> {
        let mut v = format!("<< /Length {} {dict_extra} >>\nstream\n", data.len()).into_bytes();
        v.extend_from_slice(data);
        v.extend_from_slice(b"\nendstream");
        v
    }

    fn zlib(data: &[u8]) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        e.write_all(data).unwrap();
        e.finish().unwrap()
    }

    /// `n` byte a zero compressi in streaming (nessun buffer da `n` byte).
    fn zlib_zeros(n: u64) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        let chunk = vec![0u8; 64 * 1024];
        let mut left = n;
        while left > 0 {
            let k = left.min(chunk.len() as u64) as usize;
            e.write_all(&chunk[..k]).unwrap();
            left -= k as u64;
        }
        e.finish().unwrap()
    }

    /// Byte pseudo-casuali, incomprimibili (xorshift).
    fn noise(n: usize, seed: u64) -> Vec<u8> {
        let mut x = seed | 1;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                (x >> 24) as u8
            })
            .collect()
    }

    /// Codifica ASCIIHex con il marcatore di fine `>`.
    fn hex(data: &[u8]) -> Vec<u8> {
        let mut v: Vec<u8> = data
            .iter()
            .flat_map(|b| format!("{b:02X}").into_bytes())
            .collect();
        v.push(b'>');
        v
    }

    /// Documento di `pages` pagine (oggetti 10..), contenuti 100.., più gli oggetti `extra`.
    fn doc(
        pages: u32,
        catalog_extra: &str,
        extra: Vec<(u32, Vec<u8>)>,
        trailer_extra: &str,
    ) -> Vec<u8> {
        let kids: Vec<String> = (0..pages).map(|i| format!("{} 0 R", 10 + i)).collect();
        let mut objs = vec![
            (
                1,
                dict(&format!(
                    "<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>"
                )),
            ),
            (
                2,
                dict(&format!(
                    "<< /Type /Pages /Kids [{}] /Count {pages} >>",
                    kids.join(" ")
                )),
            ),
        ];
        for i in 0..pages {
            objs.push((
                10 + i,
                dict(&format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {} 0 R >>",
                    100 + i
                )),
            ));
            objs.push((100 + i, stream("", b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET")));
        }
        objs.extend(extra);
        build(&objs, trailer_extra)
    }

    /// Una pagina con un solo stream di contenuto, filtri e dati scelti dal test.
    fn doc_with_content(filter: &str, data: &[u8]) -> Vec<u8> {
        build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
                (
                    3,
                    dict("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>"),
                ),
                (4, stream(filter, data)),
            ],
            "",
        )
    }

    /// Object stream Flate che contiene gli oggetti `(numero, corpo)`.
    fn objstm(objects: &[(u32, &[u8])]) -> Vec<u8> {
        let mut header = String::new();
        let mut body: Vec<u8> = Vec::new();
        for (n, b) in objects {
            header.push_str(&format!("{n} {} ", body.len()));
            body.extend_from_slice(b);
            body.push(b' ');
        }
        let first = header.len();
        let mut plain = header.into_bytes();
        plain.extend_from_slice(&body);
        stream(
            &format!(
                "/Type /ObjStm /N {} /First {first} /Filter /FlateDecode",
                objects.len()
            ),
            &zlib(&plain),
        )
    }

    /// Tetti ridotti per i test; i campi non toccati sono le Soglie (`Caps::default`).
    fn caps() -> Caps {
        Caps {
            per_stream_bytes: MIB,
            total_bytes: 4 * MIB,
            max_object_streams: 4,
            max_pages: 10,
            ..Caps::default()
        }
    }

    fn kind_of(r: Result<PrescanReport, Rejection>) -> Kind {
        match r {
            Err(Rejection::Failed(k, _)) => k,
            other => panic!("atteso Failed, ottenuto {other:?}"),
        }
    }

    fn reasons_of(r: Result<PrescanReport, Rejection>) -> Vec<String> {
        match r {
            Err(Rejection::Quarantined(r)) => r,
            other => panic!("atteso Quarantined, ottenuto {other:?}"),
        }
    }

    fn assert_ok(r: Result<PrescanReport, Rejection>) {
        if let Err(e) = r {
            panic!("atteso Ok, ottenuto {e:?}");
        }
    }

    // ---- soglie (Task 1) ---------------------------------------------------------------

    #[test]
    fn default_caps_are_the_launcher_thresholds() {
        let c = Caps::default();
        assert_eq!(c.per_stream_bytes, 64 * MIB);
        assert_eq!(c.total_bytes, 512 * MIB);
        assert_eq!(c.max_ratio, 100);
        assert_eq!(c.ratio_min_bytes, 16 * MIB);
        assert_eq!(c.max_filter_chain, 2);
        assert_eq!(c.max_pages, 2000);
        assert_eq!(c.max_image_bytes, 64 * MIB);
        assert_eq!(c.docx_max_entries, 1000);
        assert_eq!(c.docx_max_total_bytes, 200 * MIB);
    }

    // ---- PDF benigno -------------------------------------------------------------------

    #[test]
    fn benign_pdf_passes() {
        assert_ok(check_pdf(&doc(1, "", vec![], ""), &caps()));
    }

    // ---- PDF: cifratura, prima del caricamento -----------------------------------------

    fn encrypt_obj() -> (u32, Vec<u8>) {
        (
            50,
            dict("<< /Filter /Standard /V 1 /R 2 /O <00> /U <00> /P -4 >>"),
        )
    }

    #[test]
    fn encrypt_in_trailer_is_encrypted() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encrypt 50 0 R /ID [<01> <01>]",
        );
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn encrypt_with_escaped_name_is_encrypted() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encr#79pt 50 0 R /ID [<01> <01>]",
        );
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn encrypt_is_detected_even_if_body_is_corrupt() {
        let pdf =
            b"%PDF-1.7\nnot an object at all\ntrailer\n<< /Root 1 0 R /Encrypt 5 0 R >>\n%%EOF\n";
        assert_eq!(kind_of(check_pdf(pdf, &caps())), Kind::Encrypted);
    }

    #[test]
    fn garbage_pdf_is_corrupt() {
        assert_eq!(
            kind_of(check_pdf(b"%PDF-1.7\nthis is not a pdf\n", &caps())),
            Kind::Corrupt
        );
    }

    // ---- PDF: object stream e pagine (TEST-8) ------------------------------------------

    fn garbage_objstm(name: &str) -> Vec<u8> {
        stream(
            &format!("/Type /{name} /N 1 /First 4 /Filter /FlateDecode"),
            b"\x00\x01garbage",
        )
    }

    #[test]
    fn objstm_count_over_cap_is_rejected_before_load() {
        let extra = (0..3).map(|i| (60 + i, garbage_objstm("ObjStm"))).collect();
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn objstm_count_uses_normalized_names() {
        let extra = vec![
            (60, garbage_objstm("ObjStm")),
            (61, garbage_objstm("Obj#53tm")),
            (62, garbage_objstm("#4FbjStm")),
        ];
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn objstm_within_cap_passes() {
        let extra = vec![(60, objstm(&[(70, b"<< /Foo 1 >>")]))];
        let c = Caps {
            max_object_streams: 2,
            ..caps()
        };
        assert_ok(check_pdf(&doc(1, "", extra, ""), &c));
    }

    #[test]
    fn objstm_expanding_over_per_stream_cap_is_decompression_limit() {
        // Il tetto per stream deve agire già al caricamento (max_decompressed_size).
        let mut big = b"<< /Pad (".to_vec();
        big.extend(std::iter::repeat_n(b'A', 2 * MIB as usize));
        big.extend_from_slice(b") >>");
        let extra = vec![(60, objstm(&[(70, &big)]))];
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &caps())),
            Kind::DecompressionLimit
        );
    }

    #[test]
    fn pages_over_limit_is_resource_limit() {
        let c = Caps {
            max_pages: 3,
            ..caps()
        };
        assert_eq!(
            kind_of(check_pdf(&doc(4, "", vec![], ""), &c)),
            Kind::ResourceLimit
        );
    }

    #[test]
    fn pages_at_limit_pass() {
        let c = Caps {
            max_pages: 3,
            ..caps()
        };
        assert_ok(check_pdf(&doc(3, "", vec![], ""), &c));
    }

    // ---- PDF: contenuti attivi (TEST-7) ------------------------------------------------

    #[test]
    fn javascript_hidden_in_compressed_objstm_is_quarantined() {
        let js: &[u8] = b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>";
        let pdf = doc(1, "/OpenAction 70 0 R", vec![(60, objstm(&[(70, js)]))], "");
        assert!(
            !pdf.windows(10).any(|w| w == b"JavaScript"),
            "il JS deve stare solo dentro l'object stream compresso"
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn open_action_javascript_is_quarantined_with_both_reasons() {
        let pdf = doc(
            1,
            "/OpenAction 50 0 R",
            vec![(50, dict("<< /S /JavaScript /JS (app.alert\\(1\\)) >>"))],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
        assert!(r.contains(&"open_action_js".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn bare_js_key_is_javascript() {
        let pdf = doc(1, "", vec![(50, dict("<< /Type /Action /JS (x) >>"))], "");
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn launch_action_is_quarantined() {
        let pdf = doc(1, "", vec![(50, dict("<< /S /Launch /F (cmd.exe) >>"))], "");
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"launch".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn launch_with_escaped_name_is_quarantined() {
        let pdf = doc(
            1,
            "",
            vec![(50, dict("<< /S /Laun#63h /F (cmd.exe) >>"))],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"launch".to_owned()), "motivi {r:?}");
    }

    #[test]
    fn embedded_file_is_quarantined() {
        let pdf = doc(
            1,
            "",
            vec![
                (
                    50,
                    dict("<< /Type /Filespec /F (a.exe) /EF << /F 51 0 R >> >>"),
                ),
                (51, stream("/Type /EmbeddedFile", b"MZ\x90\x00")),
            ],
            "",
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"embedded_file".to_owned()), "motivi {r:?}");
    }

    // ---- PDF: catene di filtri (TEST-7) ------------------------------------------------

    #[test]
    fn filter_chain_longer_than_two_is_rejected() {
        let data = hex(&hex(&hex(b"BT ET")));
        let pdf = doc_with_content(
            "/Filter [/ASCIIHexDecode /ASCIIHexDecode /ASCIIHexDecode]",
            &data,
        );
        let k = kind_of(check_pdf(&pdf, &caps()));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::Unsupported),
            "kind {k:?}"
        );
    }

    #[test]
    fn filter_chain_of_two_allowed_filters_passes() {
        let data = hex(&zlib(b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET"));
        let pdf = doc_with_content("/Filter [/ASCIIHexDecode /FlateDecode]", &data);
        assert_ok(check_pdf(&pdf, &caps()));
    }

    #[test]
    fn lzw_runlength_and_unknown_filters_are_unsupported() {
        for f in ["/LZWDecode", "/RunLengthDecode", "/FooDecode"] {
            let pdf = doc_with_content(&format!("/Filter {f}"), b"\x80\x0b\x60\x50");
            assert_eq!(
                kind_of(check_pdf(&pdf, &caps())),
                Kind::Unsupported,
                "filtro {f}"
            );
        }
    }

    #[test]
    fn image_filter_on_non_image_stream_is_rejected() {
        let pdf = doc_with_content("/Filter /DCTDecode", b"\xFF\xD8\xFF\xE0junk");
        let k = kind_of(check_pdf(&pdf, &caps()));
        assert!(
            matches!(k, Kind::Unsupported | Kind::DecompressionLimit),
            "kind {k:?}"
        );
    }

    fn image_doc(w: u32, h: u32) -> Vec<u8> {
        build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
                (
                    3,
                    dict(
                        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                         /Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
                    ),
                ),
                (4, stream("", b"q 100 0 0 100 0 0 cm /Im1 Do Q")),
                (
                    5,
                    stream(
                        &format!(
                            "/Type /XObject /Subtype /Image /Width {w} /Height {h} \
                             /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode"
                        ),
                        b"\xFF\xD8\xFF\xE0junk",
                    ),
                ),
            ],
            "",
        )
    }

    #[test]
    fn image_filter_on_image_within_cap_passes() {
        assert_ok(check_pdf(&image_doc(100, 100), &caps()));
    }

    #[test]
    fn image_filter_on_image_over_cap_is_rejected() {
        // 10.000 × 10.000 × 3 byte ≈ 286 MiB, oltre i 64 MiB.
        let k = kind_of(check_pdf(&image_doc(10_000, 10_000), &caps()));
        assert!(
            matches!(
                k,
                Kind::DecompressionLimit | Kind::ResourceLimit | Kind::Unsupported
            ),
            "kind {k:?}"
        );
    }

    // ---- PDF: tetti di decompressione Flate (TEST-7) -----------------------------------

    #[test]
    fn flate_stream_at_per_stream_cap_passes() {
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(MIB));
        assert_ok(check_pdf(&pdf, &caps()));
    }

    #[test]
    fn flate_stream_over_per_stream_cap_is_decompression_limit() {
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(MIB + 1));
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::DecompressionLimit);
    }

    #[test]
    fn flate_ratio_over_limit_is_decompression_limit() {
        let c = Caps {
            ratio_min_bytes: 64 * KIB,
            ..caps()
        };
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib_zeros(512 * KIB));
        assert_eq!(kind_of(check_pdf(&pdf, &c)), Kind::DecompressionLimit);
    }

    #[test]
    fn incompressible_flate_under_ratio_passes() {
        let c = Caps {
            ratio_min_bytes: 64 * KIB,
            ..caps()
        };
        let pdf = doc_with_content("/Filter /FlateDecode", &zlib(&noise(512 * KIB as usize, 7)));
        assert_ok(check_pdf(&pdf, &c));
    }

    #[test]
    fn flate_total_over_cap_is_decompression_limit() {
        let c = Caps {
            total_bytes: MIB,
            ..caps()
        };
        let objs = vec![
            (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
            (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
            (
                3,
                dict(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents [4 0 R 5 0 R] >>",
                ),
            ),
            (
                4,
                stream("/Filter /FlateDecode", &zlib(&noise(600 * KIB as usize, 1))),
            ),
            (
                5,
                stream("/Filter /FlateDecode", &zlib(&noise(600 * KIB as usize, 2))),
            ),
        ];
        assert_eq!(
            kind_of(check_pdf(&build(&objs, ""), &c)),
            Kind::DecompressionLimit
        );
    }

    // ---- DOCX (TEST-7) -----------------------------------------------------------------
    // Namespace OOXML sostituiti da URN segnaposto: il prescan non li valida e lo scanner
    // rifiuta gli URL in chiaro.

    const CT_DOCX: &str =
        "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml";
    const CT_MACRO: &str = "application/vnd.ms-word.document.macroEnabled.main+xml";

    fn content_types(main: &str) -> String {
        format!(
            r#"<?xml version="1.0" encoding="UTF-8"?><Types xmlns="urn:test:ooxml/package/2006/content-types"><Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/><Default Extension="xml" ContentType="application/xml"/><Override PartName="/word/document.xml" ContentType="{main}"/></Types>"#
        )
    }

    const RELS: &str = r#"<?xml version="1.0" encoding="UTF-8"?><Relationships xmlns="urn:test:ooxml/package/2006/relationships"><Relationship Id="rId1" Type="urn:test:ooxml/officeDocument/2006/relationships/officeDocument" Target="word/document.xml"/></Relationships>"#;

    const XML_DECL: &str = r#"<?xml version="1.0" encoding="UTF-8"?>"#;
    const DOCUMENT_BODY: &str = r#"<w:document xmlns:w="urn:test:ooxml/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Ciao</w:t></w:r></w:p></w:body></w:document>"#;

    fn base_docx() -> Vec<(String, Vec<u8>)> {
        vec![
            (
                "[Content_Types].xml".into(),
                content_types(CT_DOCX).into_bytes(),
            ),
            ("_rels/.rels".into(), RELS.as_bytes().to_vec()),
            (
                "word/document.xml".into(),
                format!("{XML_DECL}{DOCUMENT_BODY}").into_bytes(),
            ),
        ]
    }

    fn scratch(ext: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("s02-prescan-tw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("d{}.{ext}", N.fetch_add(1, Ordering::SeqCst)))
    }

    fn write_docx(entries: &[(String, Vec<u8>)]) -> PathBuf {
        let path = scratch("docx");
        let mut z = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
        for (name, data) in entries {
            z.start_file(name.as_str(), zip::write::SimpleFileOptions::default())
                .unwrap();
            z.write_all(data).unwrap();
        }
        z.finish().unwrap();
        path
    }

    fn with(mut base: Vec<(String, Vec<u8>)>, name: &str, data: &[u8]) -> Vec<(String, Vec<u8>)> {
        base.retain(|(n, _)| n != name);
        base.push((name.to_owned(), data.to_vec()));
        base
    }

    #[test]
    fn benign_docx_passes() {
        assert_ok(check_docx(&write_docx(&base_docx()), &caps()));
    }

    #[test]
    fn non_zip_docx_is_corrupt() {
        let p = scratch("docx");
        std::fs::write(&p, noise(4096, 3)).unwrap();
        assert_eq!(kind_of(check_docx(&p, &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_too_many_entries_is_rejected() {
        let mut e = base_docx();
        for i in 0..3 {
            e.push((format!("word/media/p{i}.bin"), b"x".to_vec()));
        }
        let c = Caps {
            docx_max_entries: 5,
            ..caps()
        };
        let k = kind_of(check_docx(&write_docx(&e), &c));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::ResourceLimit),
            "kind {k:?}"
        );
    }

    #[test]
    fn docx_with_declared_total_over_cap_is_rejected() {
        let e = with(
            base_docx(),
            "word/media/pad.bin",
            &vec![0u8; 8 * KIB as usize],
        );
        let c = Caps {
            docx_max_total_bytes: 4 * KIB,
            ..caps()
        };
        let k = kind_of(check_docx(&write_docx(&e), &c));
        assert!(
            matches!(k, Kind::DecompressionLimit | Kind::ResourceLimit),
            "kind {k:?}"
        );
    }

    #[test]
    fn docx_with_vba_project_is_unsupported() {
        let e = with(base_docx(), "word/vbaProject.bin", b"\xD0\xCF\x11\xE0");
        assert_eq!(
            kind_of(check_docx(&write_docx(&e), &caps())),
            Kind::Unsupported
        );
    }

    #[test]
    fn docx_with_macro_content_type_is_unsupported() {
        let e = with(
            base_docx(),
            "[Content_Types].xml",
            content_types(CT_MACRO).as_bytes(),
        );
        assert_eq!(
            kind_of(check_docx(&write_docx(&e), &caps())),
            Kind::Unsupported
        );
    }

    #[test]
    fn docx_with_doctype_is_corrupt() {
        let doc = format!("{XML_DECL}<!DOCTYPE w:document [<!ELEMENT a ANY>]>{DOCUMENT_BODY}");
        let e = with(base_docx(), "word/document.xml", doc.as_bytes());
        assert_eq!(kind_of(check_docx(&write_docx(&e), &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_entity_in_any_part_is_corrupt() {
        let styles = format!(r#"{XML_DECL}<!ENTITY lol "lol"><w:styles xmlns:w="x"/>"#);
        let e = with(base_docx(), "word/styles.xml", styles.as_bytes());
        assert_eq!(kind_of(check_docx(&write_docx(&e), &caps())), Kind::Corrupt);
    }

    #[test]
    fn docx_with_external_relationship_is_quarantined() {
        let rels = format!(
            r#"{XML_DECL}<Relationships xmlns="urn:test:ooxml/package/2006/relationships"><Relationship Id="rId9" Type="urn:test:ooxml/officeDocument/2006/relationships/attachedTemplate" Target="https://attacker.invalid/t.dotm" TargetMode="External"/></Relationships>"#
        );
        let e = with(base_docx(), "word/_rels/document.xml.rels", rels.as_bytes());
        let r = reasons_of(check_docx(&write_docx(&e), &caps()));
        assert!(r.contains(&"external_link".to_owned()), "motivi {r:?}");
    }
}
