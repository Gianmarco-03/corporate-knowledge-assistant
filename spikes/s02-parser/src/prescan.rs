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
    // Schema unico dei sei campi (punto 5 di `<interfaces>` del piano, v6): `objstm_count`
    // sopra più i cinque campi qui sotto. Stub del test-write: da riempire in 01-07.
    /// Cifratura ammessa: `none` | `empty_password`.
    pub encryption: Encryption,
    /// Allegati incorporati trovati e ignorati.
    pub embedded_files: u32,
    /// Somma reale dei byte decompressi degli object stream (I1).
    pub objstm_decompressed_bytes: u64,
    /// Cross-reference stream contati a byte.
    pub xref_streams: u32,
    /// Lavoro cumulativo di decompressione (I2), mai sotto il valore reale.
    pub decompress_work_bytes: u64,
}

/// Cifratura di un PDF ammesso (schema unico, campo `encryption`). Stub del test-write.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encryption {
    /// Nessun `/Encrypt` (anche ogni DOCX).
    #[default]
    None,
    /// Cifrato, aperto con la password vuota (`empty_password`).
    EmptyPassword,
}

/// Filtro di `LoadOptions` della prima passata del ramo non cifrato (punto 2 del Task 3):
/// rinomina il `/Type` degli object stream perché lopdf non li espanda; ogni altro oggetto
/// torna invariato. Stub del test-write: da implementare in 01-07.
#[cfg_attr(not(test), allow(dead_code))]
fn defer_objstm(id: (u32, u16), obj: &mut Object) -> Option<((u32, u16), Object)> {
    let _ = (id, obj);
    todo!("stub del test-write: defer_objstm da implementare in 01-07")
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
    //! Test del prescan (piano 01-07 v6, `test_design_11`): TEST-7 e TEST-8 secondo le regole
    //! del ripasso del 2026-10-21, TEST-12 (invariante I1), TEST-13 (invariante I2), TEST-14
    //! (caratterizzazione di lopdf 0.45.0, casi c1-c7) e la parte DOCX di TEST-15.
    //!
    //! PDF costruiti nel test (xref classica con `build`, xref in stream con `modern`); i PDF
    //! cifrati li genera `qpdf` (runner: `session.sh oneshot`), che sull'host Windows manca.
    //!
    //! Convenzione dei test (dichiarata dal test-write, il piano non fissa il formato): il
    //! detail di `DecompressionLimit`/`total_decompressed` porta i token `counted=<n>` (byte
    //! contati da I1 all'arresto) e `work=<n>` (lavoro I2 a quel punto), es.
    //! `total_decompressed counted=4210688 work=4210688`. Gli altri detail del piano
    //! (`password_required`, `object_dropped`, `objstm_dropped`, `encrypt_state`) si
    //! confrontano sul primo token. Un blocco di lettura del contatore vale al più 64 KiB.
    use super::*;
    use std::collections::BTreeMap;
    use std::io::Write;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU32, Ordering};

    const MIB: u64 = 1024 * 1024;
    const KIB: u64 = 1024;
    /// Tolleranza dell'arresto di I1: un blocco di lettura.
    const READ_BLOCK: u64 = 64 * KIB;
    /// Password dei PDF cifrati generati con `qpdf` (valori di prova, non segreti).
    const OWNER_PW: &str = "proprietario-07";
    const USER_PW: &str = "utente-07";

    // ---- costruzione dei PDF: xref classica ---------------------------------------------

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

    /// Scrive `n` copie di `byte` nel compressore, a blocchi (nessun buffer da `n` byte).
    fn feed_repeat<W: Write>(w: &mut W, byte: u8, n: u64) {
        let chunk = vec![byte; 64 * 1024];
        let mut left = n;
        while left > 0 {
            let k = left.min(chunk.len() as u64) as usize;
            w.write_all(&chunk[..k]).unwrap();
            left -= k as u64;
        }
    }

    /// `n` byte a zero compressi in streaming.
    fn zlib_zeros(n: u64) -> Vec<u8> {
        let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        feed_repeat(&mut e, 0, n);
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

    /// Documento le cui `pages` pagine hanno ciascuna un contenuto Flate di `each` byte a zero
    /// (oggetti 100..). Con `shared` tutte le pagine puntano allo stesso stream 100.
    fn doc_flate_pages(pages: u32, each: u64, shared: bool, catalog_extra: &str) -> Vec<u8> {
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
        let data = zlib_zeros(each);
        for i in 0..pages {
            let content = if shared { 100 } else { 100 + i };
            objs.push((
                10 + i,
                dict(&format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents {content} 0 R >>"
                )),
            ));
            if !shared || i == 0 {
                objs.push((content, stream("/Filter /FlateDecode", &data)));
            }
        }
        build(&objs, "")
    }

    /// Object stream Flate che contiene gli oggetti `(numero, corpo)` (xref classica: i membri
    /// non compaiono nella tabella).
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

    // ---- costruzione dei PDF: xref in stream, object stream veri, revisioni -------------

    /// Object stream di un PDF `modern`: membri `(numero, corpo)`, `pad` spazi in coda al
    /// contenuto decompresso, nome del `/Type` scritto così com'è (escape `#xx` ammessi).
    struct ObjStm {
        num: u32,
        type_name: &'static str,
        members: Vec<(u32, Vec<u8>)>,
        pad: u64,
    }

    impl ObjStm {
        fn new(num: u32, members: Vec<(u32, Vec<u8>)>) -> ObjStm {
            ObjStm {
                num,
                type_name: "ObjStm",
                members,
                pad: 0,
            }
        }

        /// Corpo dell'oggetto (dizionario + stream) e byte del contenuto decompresso.
        fn body(&self) -> (Vec<u8>, u64) {
            let mut header = String::new();
            let mut bodies: Vec<u8> = Vec::new();
            for (n, b) in &self.members {
                header.push_str(&format!("{n} {} ", bodies.len()));
                bodies.extend_from_slice(b);
                bodies.push(b' ');
            }
            let first = header.len();
            let mut e = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
            e.write_all(header.as_bytes()).unwrap();
            e.write_all(&bodies).unwrap();
            feed_repeat(&mut e, b' ', self.pad);
            let data = e.finish().unwrap();
            let plain = (first + bodies.len()) as u64 + self.pad;
            let dict = format!(
                "/Type /{} /N {} /First {first} /Filter /FlateDecode",
                self.type_name,
                self.members.len()
            );
            (stream(&dict, &data), plain)
        }

        fn plain_len(&self) -> u64 {
            self.body().1
        }
    }

    /// PDF con xref in stream: offset di `startxref` e `/Size`, per le revisioni.
    struct Built {
        bytes: Vec<u8>,
        startxref: usize,
        size: u32,
    }

    fn row_normal(off: usize) -> [u8; 7] {
        let o = u32::try_from(off).unwrap().to_be_bytes();
        [1, o[0], o[1], o[2], o[3], 0, 0]
    }

    fn row_compressed(container: u32, index: u16) -> [u8; 7] {
        let c = container.to_be_bytes();
        let i = index.to_be_bytes();
        [2, c[0], c[1], c[2], c[3], i[0], i[1]]
    }

    fn row_free(next_gen: u16) -> [u8; 7] {
        let g = next_gen.to_be_bytes();
        [0, 0, 0, 0, 0, g[0], g[1]]
    }

    fn push_obj(out: &mut Vec<u8>, n: u32, body: &[u8]) {
        out.extend_from_slice(format!("{n} 0 obj\n").as_bytes());
        out.extend_from_slice(body);
        out.extend_from_slice(b"\nendobj\n");
    }

    /// Cross-reference stream `/W [1 4 2]`, Flate, con `startxref` e `%%EOF`.
    fn push_xref_stream(out: &mut Vec<u8>, num: u32, rows: &[[u8; 7]], dict_extra: &str) {
        let data: Vec<u8> = rows.concat();
        let body = stream(
            &format!("/Type /XRef /W [1 4 2] {dict_extra} /Filter /FlateDecode"),
            &zlib(&data),
        );
        let off = out.len();
        push_obj(out, num, &body);
        out.extend_from_slice(format!("startxref\n{off}\n%%EOF\n").as_bytes());
    }

    /// PDF 1.7 con oggetti normali, object stream veri (membri di tipo 2 nella xref) e una
    /// cross-reference stream. L'oggetto 1 è il Catalog.
    fn modern(normal: Vec<(u32, Vec<u8>)>, objstms: &[ObjStm]) -> Built {
        let mut out: Vec<u8> = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
        let mut all = normal;
        let mut compressed: BTreeMap<u32, (u32, u16)> = BTreeMap::new();
        for o in objstms {
            all.push((o.num, o.body().0));
            for (i, (m, _)) in o.members.iter().enumerate() {
                compressed.insert(*m, (o.num, u16::try_from(i).unwrap()));
            }
        }
        all.sort_by_key(|(n, _)| *n);
        let mut offsets: BTreeMap<u32, usize> = BTreeMap::new();
        for (n, body) in &all {
            offsets.insert(*n, out.len());
            push_obj(&mut out, *n, body);
        }
        let max = offsets
            .keys()
            .chain(compressed.keys())
            .copied()
            .max()
            .unwrap_or(0);
        let xnum = max + 1;
        let size = xnum + 1;
        let xoff = out.len();
        let rows: Vec<[u8; 7]> = (0..size)
            .map(|i| {
                if i == xnum {
                    row_normal(xoff)
                } else if let Some(off) = offsets.get(&i) {
                    row_normal(*off)
                } else if let Some((c, idx)) = compressed.get(&i) {
                    row_compressed(*c, *idx)
                } else {
                    row_free(if i == 0 { 65535 } else { 0 })
                }
            })
            .collect();
        push_xref_stream(&mut out, xnum, &rows, &format!("/Size {size} /Root 1 0 R"));
        Built {
            bytes: out,
            startxref: xoff,
            size,
        }
    }

    /// Aggiornamento incrementale con una nuova cross-reference stream (`/Prev`, `/Index`):
    /// oggetti `normal` aggiunti o sostituiti, `freed` liberati.
    fn append_xref_revision(base: &Built, normal: Vec<(u32, Vec<u8>)>, freed: &[u32]) -> Built {
        let mut out = base.bytes.clone();
        let mut rows: BTreeMap<u32, [u8; 7]> = BTreeMap::new();
        for (n, body) in normal {
            rows.insert(n, row_normal(out.len()));
            push_obj(&mut out, n, &body);
        }
        for f in freed {
            rows.insert(*f, row_free(1));
        }
        let xnum = base.size;
        let size = base.size + 1;
        let xoff = out.len();
        rows.insert(xnum, row_normal(xoff));
        let index: Vec<String> = rows.keys().map(|k| format!("{k} 1")).collect();
        let rows_v: Vec<[u8; 7]> = rows.values().copied().collect();
        push_xref_stream(
            &mut out,
            xnum,
            &rows_v,
            &format!(
                "/Size {size} /Root 1 0 R /Prev {} /Index [{}]",
                base.startxref,
                index.join(" ")
            ),
        );
        Built {
            bytes: out,
            startxref: xoff,
            size,
        }
    }

    /// Catalog (1) con `catalog_extra`, Pages (2), una pagina (3) con il contenuto 4.
    fn base_objects(catalog_extra: &str) -> Vec<(u32, Vec<u8>)> {
        vec![
            (
                1,
                dict(&format!(
                    "<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>"
                )),
            ),
            (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
            (
                3,
                dict("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R >>"),
            ),
            (4, stream("", b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET")),
        ]
    }

    /// `count` object stream piccoli (oggetti 5..) con un membro ciascuno (5 + count..).
    /// Numeri bassi: `qpdf` ignora le voci della xref oltre un massimo legato alla lunghezza
    /// del file (visto nel runner: un PDF da 1 KB con `/Size 1004` perde le voci da 1000).
    fn small_objstms(count: u32) -> Vec<ObjStm> {
        (0..count)
            .map(|i| {
                let member = (5 + count + i, dict(&format!("<< /K {i} >>")));
                ObjStm::new(5 + i, vec![member])
            })
            .collect()
    }

    /// `/Extra [...]` con un riferimento a ogni membro (così `qpdf` non li scarta).
    fn extra_refs(objstms: &[ObjStm]) -> String {
        let refs: Vec<String> = objstms
            .iter()
            .flat_map(|o| o.members.iter().map(|(m, _)| format!("{m} 0 R")))
            .collect();
        format!("/Extra [{}]", refs.join(" "))
    }

    /// Dizionario con una stringa letterale di `n` byte `A` (contenuto grande dei membri).
    fn pad_dict(n: usize) -> Vec<u8> {
        let mut v = b"<< /Pad (".to_vec();
        v.extend(std::iter::repeat_n(b'A', n));
        v.extend_from_slice(b") >>");
        v
    }

    // ---- qpdf e lopdf per le fixture cifrate --------------------------------------------

    fn scratch(ext: &str) -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!("s02-prescan-tw-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(format!("d{}.{ext}", N.fetch_add(1, Ordering::SeqCst)))
    }

    /// `qpdf <opts> <in> <out>` (nel runner; exit 3 = solo avvisi).
    fn qpdf(input: &[u8], opts: &[&str]) -> Vec<u8> {
        let i = scratch("pdf");
        let o = scratch("pdf");
        std::fs::write(&i, input).unwrap();
        let out = Command::new("qpdf")
            .args(opts)
            .arg(&i)
            .arg(&o)
            .output()
            .unwrap_or_else(|e| panic!("qpdf non eseguibile (serve il runner): {e}"));
        assert!(
            matches!(out.status.code(), Some(0 | 3)),
            "qpdf {opts:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        std::fs::read(&o).unwrap()
    }

    /// `qpdf --check` senza avvisi (fixture costruite a mano, revisioni comprese).
    fn qpdf_check(pdf: &[u8]) {
        let p = scratch("pdf");
        std::fs::write(&p, pdf).unwrap();
        let out = Command::new("qpdf")
            .arg("--check")
            .arg(&p)
            .output()
            .unwrap_or_else(|e| panic!("qpdf non eseguibile (serve il runner): {e}"));
        assert_eq!(
            out.status.code(),
            Some(0),
            "fixture non valida per qpdf: {}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Cifrato AES-256 con password utente vuota (`qpdf --encrypt "" <owner> 256`).
    fn encrypt_empty_user(input: &[u8], objstm_mode: &str) -> Vec<u8> {
        qpdf(
            input,
            &[objstm_mode, "--encrypt", "", OWNER_PW, "256", "--"],
        )
    }

    /// Cifrato AES-256 con password utente non vuota.
    fn encrypt_with_user(input: &[u8]) -> Vec<u8> {
        qpdf(input, &["--encrypt", USER_PW, OWNER_PW, "256", "--"])
    }

    /// Documento caricato da lopdf senza opzioni (password vuota tentata da lopdf), solo per
    /// leggere le fixture: mai usato come oracolo del prescan.
    fn lopdf_doc(bytes: &[u8]) -> Document {
        Document::load_mem_with_options(bytes, LoadOptions::default())
            .unwrap_or_else(|e| panic!("fixture non caricabile da lopdf: {e}"))
    }

    /// Byte decompressi di uno stream Flate, contati senza trattenerli.
    fn inflated_len(data: &[u8]) -> u64 {
        std::io::copy(
            &mut flate2::read::ZlibDecoder::new(data),
            &mut std::io::sink(),
        )
        .unwrap_or_else(|e| panic!("fixture: object stream non decodificabile: {e}"))
    }

    /// Contenitori di object stream rimasti in `Document::objects` e somma reale dei loro byte
    /// decompressi (fixture cifrate: i contenitori restano decifrati e compressi, vedi c3).
    fn objstm_containers(doc: &Document) -> (u32, u64) {
        let mut n = 0u32;
        let mut sum = 0u64;
        for o in doc.objects.values() {
            if let Object::Stream(s) = o
                && s.dict.has_type(b"ObjStm")
            {
                n += 1;
                sum += inflated_len(&s.content);
            }
        }
        (n, sum)
    }

    /// Offset (voce normale della xref) dell'oggetto `num`.
    fn offset_of(doc: &Document, num: u32) -> usize {
        match doc.reference_table.get(num) {
            Some(lopdf::xref::XrefEntry::Normal { offset, .. }) => *offset as usize,
            other => panic!("oggetto {num} senza voce normale: {other:?}"),
        }
    }

    /// Inizio dei dati dello stream dell'oggetto all'offset dato.
    fn stream_data_start(bytes: &[u8], offset: usize) -> usize {
        let at = offset + find(&bytes[offset..], b"stream").expect("stream assente");
        let mut i = at + b"stream".len();
        if bytes[i] == b'\r' {
            i += 1;
        }
        assert_eq!(bytes[i], b'\n', "EOL dopo stream");
        i + 1
    }

    // ---- tetti, esiti e invarianti ------------------------------------------------------

    /// Tetti ridotti per i test; i campi non toccati sono le Soglie (`Caps::default`).
    fn caps() -> Caps {
        Caps {
            per_stream_bytes: MIB,
            total_bytes: 4 * MIB,
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

    fn failed_of(r: Result<PrescanReport, Rejection>) -> (Kind, String) {
        match r {
            Err(Rejection::Failed(k, d)) => (k, d),
            other => panic!("atteso Failed, ottenuto {other:?}"),
        }
    }

    /// `Failed(kind, detail)` con il primo token del detail uguale a `detail`.
    fn assert_failed(r: Result<PrescanReport, Rejection>, kind: Kind, detail: &str) -> String {
        let (k, d) = failed_of(r);
        assert_eq!(
            (k, d.split_whitespace().next()),
            (kind, Some(detail)),
            "detail {d:?}"
        );
        d
    }

    fn reasons_of(r: Result<PrescanReport, Rejection>) -> Vec<String> {
        match r {
            Err(Rejection::Quarantined(r)) => r,
            other => panic!("atteso Quarantined, ottenuto {other:?}"),
        }
    }

    fn assert_ok(r: Result<PrescanReport, Rejection>) -> PrescanReport {
        match r {
            Ok(rep) => rep,
            Err(e) => panic!("atteso Ok, ottenuto {e:?}"),
        }
    }

    /// Valore del token `key=<n>` del detail (convenzione dei test, vedi testa del modulo).
    fn token(detail: &str, key: &str) -> u64 {
        detail
            .split_whitespace()
            .find_map(|t| t.strip_prefix(key)?.strip_prefix('='))
            .and_then(|v| v.parse().ok())
            .unwrap_or_else(|| panic!("detail {detail:?} senza {key}=<n>"))
    }

    /// Limite B di I2: 2 × totale + 2 × `xref_streams` × tetto per stream.
    fn bound_b(c: &Caps, xref_streams: u32) -> u64 {
        2 * c.total_bytes + 2 * u64::from(xref_streams) * c.per_stream_bytes
    }

    /// Quota per stream del ramo cifrato: min(tetto per stream, ⌊totale / object stream⌋).
    fn encrypted_quota(c: &Caps, objstm: u32) -> u64 {
        match objstm {
            0 => c.per_stream_bytes,
            n => c.per_stream_bytes.min(c.total_bytes / u64::from(n)),
        }
    }

    /// I2 sul ramo non cifrato: al più B, mai sotto prescan + riespansione degli object
    /// stream + due passate di caricamento delle xref stream.
    fn assert_work_unencrypted(r: &PrescanReport, c: &Caps) {
        let b = bound_b(c, r.xref_streams);
        let floor =
            2 * r.objstm_decompressed_bytes + 2 * u64::from(r.xref_streams) * c.per_stream_bytes;
        assert!(
            r.decompress_work_bytes <= b,
            "I2: work {} oltre B {b}: {r:?}",
            r.decompress_work_bytes
        );
        assert!(
            r.decompress_work_bytes >= floor,
            "I2: work {} sotto il minimo {floor} (stima più bassa del reale): {r:?}",
            r.decompress_work_bytes
        );
    }

    /// I2 sul ramo cifrato: al più B, mai sotto decodifica dopo il caricamento + limite
    /// analitico n × quota + una passata delle xref stream.
    fn assert_work_encrypted(r: &PrescanReport, c: &Caps) {
        let b = bound_b(c, r.xref_streams);
        let floor = r.objstm_decompressed_bytes
            + u64::from(r.objstm_count) * encrypted_quota(c, r.objstm_count)
            + u64::from(r.xref_streams) * c.per_stream_bytes;
        assert!(
            r.decompress_work_bytes <= b,
            "I2: work {} oltre B {b}: {r:?}",
            r.decompress_work_bytes
        );
        assert!(
            r.decompress_work_bytes >= floor,
            "I2: work {} sotto il minimo {floor}: {r:?}",
            r.decompress_work_bytes
        );
    }

    /// Arresto di I1 su `total_decompressed`: contati oltre il totale ma entro un blocco,
    /// lavoro I2 entro B.
    fn assert_total_stop(detail: &str, c: &Caps, xref_streams: u32) {
        let counted = token(detail, "counted");
        let work = token(detail, "work");
        assert!(
            counted > c.total_bytes && counted <= c.total_bytes + READ_BLOCK,
            "I1: contati {counted}, attesi in ({}, {}]",
            c.total_bytes,
            c.total_bytes + READ_BLOCK
        );
        assert!(
            work <= bound_b(c, xref_streams),
            "I2: work {work} oltre B {}",
            bound_b(c, xref_streams)
        );
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
    fn benign_pdf_passes_with_unencrypted_schema() {
        let r = assert_ok(check_pdf(&doc(1, "", vec![], ""), &caps()));
        assert_eq!(r.pages, Some(1));
        assert_eq!(r.encryption, Encryption::None);
        assert_eq!(r.embedded_files, 0);
        assert_eq!(r.objstm_count, 0);
        assert_eq!(r.objstm_decompressed_bytes, 0);
        assert_eq!(r.xref_streams, 0);
        assert_work_unencrypted(&r, &caps());
    }

    // ---- TEST-7: cifratura (punto 3-bis) -----------------------------------------------

    fn encrypt_obj() -> (u32, Vec<u8>) {
        (
            50,
            dict("<< /Filter /Standard /V 1 /R 2 /O <00> /U <00> /P -4 >>"),
        )
    }

    /// (k3) Cifrato solo per i permessi, password utente vuota: ammesso, analizzato sul
    /// contenuto decifrato, `encryption = empty_password`.
    #[test]
    fn empty_user_password_is_admitted_as_empty_password() {
        let pdf = encrypt_empty_user(&doc(1, "", vec![], ""), "--object-streams=disable");
        let r = assert_ok(check_pdf(&pdf, &caps()));
        assert_eq!(r.encryption, Encryption::EmptyPassword);
        assert_eq!(r.pages, Some(1));
        assert_eq!(r.embedded_files, 0);
    }

    /// (k4) Password utente non vuota: `Encrypted`/`password_required`.
    #[test]
    fn user_password_is_encrypted_password_required() {
        let pdf = encrypt_with_user(&doc(1, "", vec![], ""));
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::Encrypted,
            "password_required",
        );
    }

    /// Gestore standard con `/O` e `/U` che non autenticano la password vuota.
    #[test]
    fn standard_handler_without_empty_password_is_password_required() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encrypt 50 0 R /ID [<01> <01>]",
        );
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::Encrypted,
            "password_required",
        );
    }

    /// Nome `/Encrypt` scritto con escape: mai `ok`; `Encrypted`/`password_required` se lopdf
    /// lo vede, altrimenti `Corrupt`/`encrypt_state` (dichiarato a byte, non caricato).
    #[test]
    fn escaped_encrypt_name_is_never_ok() {
        let pdf = doc(
            1,
            "",
            vec![encrypt_obj()],
            "/Encr#79pt 50 0 R /ID [<01> <01>]",
        );
        let (k, d) = failed_of(check_pdf(&pdf, &caps()));
        let first = d.split_whitespace().next();
        assert!(
            matches!(
                (k, first),
                (Kind::Encrypted, Some("password_required"))
                    | (Kind::Corrupt, Some("encrypt_state"))
            ),
            "{k:?} {d:?}"
        );
    }

    /// `/Encrypt` dichiarato con un corpo illeggibile: mai `ok`.
    #[test]
    fn encrypt_declared_with_corrupt_body_is_never_ok() {
        let pdf =
            b"%PDF-1.7\nnot an object at all\ntrailer\n<< /Root 1 0 R /Encrypt 5 0 R >>\n%%EOF\n";
        let k = kind_of(check_pdf(pdf, &caps()));
        assert!(matches!(k, Kind::Encrypted | Kind::Corrupt), "kind {k:?}");
    }

    #[test]
    fn garbage_pdf_is_corrupt() {
        assert_eq!(
            kind_of(check_pdf(b"%PDF-1.7\nthis is not a pdf\n", &caps())),
            Kind::Corrupt
        );
    }

    // ---- TEST-7: allegati (punto 3-ter) ------------------------------------------------

    fn attachment(data_filter: &str, data: &[u8]) -> Vec<(u32, Vec<u8>)> {
        vec![
            (
                50,
                dict("<< /Type /Filespec /F (a.exe) /UF (a.exe) /EF << /F 51 0 R >> >>"),
            ),
            (
                51,
                stream(&format!("/Type /EmbeddedFile {data_filter}"), data),
            ),
            (52, dict("<< /Names [(a.exe) 50 0 R] >>")),
        ]
    }

    /// (k5) Un allegato non causa quarantena: si conta in `embedded_files`.
    #[test]
    fn embedded_file_is_counted_not_quarantined() {
        let pdf = doc(
            1,
            "/Names << /EmbeddedFiles 52 0 R >>",
            attachment("", b"MZ\x90\x00"),
            "",
        );
        let r = assert_ok(check_pdf(&pdf, &caps()));
        assert_eq!(r.embedded_files, 1, "{r:?}");
    }

    /// (k6) Stream dell'allegato oltre il tetto per stream: `DecompressionLimit`, nessuna
    /// esenzione per `/Type /EmbeddedFile`.
    #[test]
    fn embedded_file_over_per_stream_cap_is_decompression_limit() {
        let pdf = doc(
            1,
            "/Names << /EmbeddedFiles 52 0 R >>",
            attachment("/Filter /FlateDecode", &zlib_zeros(MIB + 1)),
            "",
        );
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::DecompressionLimit);
    }

    /// `/Launch` che punta a un allegato resta in quarantena, con il solo motivo `launch`.
    #[test]
    fn launch_pointing_to_embedded_file_is_quarantined_as_launch() {
        let mut extra = attachment("", b"MZ\x90\x00");
        extra.push((53, dict("<< /S /Launch /F 50 0 R >>")));
        let pdf = doc(
            1,
            "/Names << /EmbeddedFiles 52 0 R >> /OpenAction 53 0 R",
            extra,
            "",
        );
        assert_eq!(
            reasons_of(check_pdf(&pdf, &caps())),
            vec!["launch".to_owned()]
        );
    }

    // ---- TEST-8: object stream (k1, k2, conteggio) --------------------------------------

    /// (k1) Più di 64 object stream piccoli, somma reale sotto il totale: nessun rifiuto per
    /// numero; `objstm_count` e la somma reale nel rapporto; I2 entro B.
    #[test]
    fn many_small_objstm_are_not_rejected_by_count() {
        let osm = small_objstms(70);
        let expected: u64 = osm.iter().map(ObjStm::plain_len).sum();
        let pdf = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let c = caps();
        let r = assert_ok(check_pdf(&pdf, &c));
        assert_eq!(r.objstm_count, 70);
        assert_eq!(r.objstm_decompressed_bytes, expected);
        assert_eq!(r.xref_streams, 1);
        assert_eq!(r.encryption, Encryption::None);
        assert_work_unencrypted(&r, &c);
    }

    /// Conteggio con nomi normalizzati (`#xx`): tre object stream validi, nessun rifiuto.
    #[test]
    fn objstm_count_uses_normalized_names() {
        let mut osm = small_objstms(3);
        osm[1].type_name = "Obj#53tm";
        osm[2].type_name = "#4FbjStm";
        let pdf = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let r = assert_ok(check_pdf(&pdf, &caps()));
        assert_eq!(r.objstm_count, 3);
    }

    #[test]
    fn objstm_within_caps_passes() {
        let extra = vec![(60, objstm(&[(70, b"<< /Foo 1 >>")]))];
        assert_ok(check_pdf(&doc(1, "", extra, ""), &caps()));
    }

    #[test]
    fn objstm_expanding_over_per_stream_cap_is_decompression_limit() {
        // Il tetto per stream vale anche sugli object stream.
        let big = pad_dict(2 * MIB as usize);
        let extra = vec![(60, objstm(&[(70, &big)]))];
        assert_eq!(
            kind_of(check_pdf(&doc(1, "", extra, ""), &caps())),
            Kind::DecompressionLimit
        );
    }

    /// (k2) Object stream ciascuno sotto il tetto per stream, somma reale oltre il totale:
    /// `DecompressionLimit`/`total_decompressed`, contati ≤ totale + un blocco, lavoro ≤ B.
    #[test]
    fn objstm_sum_over_total_stops_at_total_plus_one_block() {
        let mut osm = small_objstms(6);
        for o in &mut osm {
            o.pad = 900 * KIB;
        }
        let pdf = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let c = caps();
        let d = assert_failed(
            check_pdf(&pdf, &c),
            Kind::DecompressionLimit,
            "total_decompressed",
        );
        assert_total_stop(&d, &c, 1);
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

    // ---- TEST-8: ramo cifrato (k1e, k2e) -------------------------------------------------

    /// (k1e) Cifrato con password vuota, più di 64 object stream piccoli, somma sotto il
    /// totale: nessun rifiuto, `encryption = empty_password`, I2 entro B.
    #[test]
    fn encrypted_many_small_objstm_are_admitted() {
        let osm = small_objstms(70);
        let plain = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let pdf = encrypt_empty_user(&plain, "--object-streams=preserve");
        let (containers, real) = objstm_containers(&lopdf_doc(&pdf));
        assert!(
            containers > 64,
            "fixture: {containers} object stream dopo qpdf"
        );
        let c = caps();
        let r = assert_ok(check_pdf(&pdf, &c));
        assert_eq!(r.encryption, Encryption::EmptyPassword);
        assert!(r.objstm_count > 64, "{r:?}");
        assert_eq!(r.objstm_decompressed_bytes, real);
        assert!(r.xref_streams >= 1, "{r:?}");
        assert_work_encrypted(&r, &c);
    }

    /// (k2e) Cifrato con password vuota, object stream sotto la quota del ramo cifrato e somma
    /// reale oltre il totale (contenuti delle pagine): `total_decompressed`, contati ≤ totale
    /// + un blocco, lavoro ≤ B.
    #[test]
    fn encrypted_sum_over_total_stops_at_total_plus_one_block() {
        let plain = doc_flate_pages(6, 900 * KIB, false, "");
        let pdf = encrypt_empty_user(&plain, "--object-streams=generate");
        let c = caps();
        let d = assert_failed(
            check_pdf(&pdf, &c),
            Kind::DecompressionLimit,
            "total_decompressed",
        );
        assert_total_stop(&d, &c, 1);
    }

    // ---- TEST-12: I1, byte distinti contati una sola volta ------------------------------

    fn roomy_caps() -> Caps {
        Caps {
            per_stream_bytes: 4 * MIB,
            total_bytes: 5 * MIB,
            ..caps()
        }
    }

    /// Uno stream condiviso da due pagine si conta una volta: 3 MiB ≤ 5 MiB (contato due
    /// volte sarebbe 6 MiB, oltre il totale).
    #[test]
    fn i1_shared_stream_is_counted_once() {
        let pdf = doc_flate_pages(2, 3 * MIB, true, "");
        assert_ok(check_pdf(&pdf, &roomy_caps()));
    }

    /// La riespansione degli object stream nel caricamento vero entra in I2, non in I1:
    /// object stream da 2 MiB + contenuto da 2 MiB stanno sotto 5 MiB; `objstm_decompressed_bytes`
    /// è la somma reale.
    #[test]
    fn i1_objstm_reexpansion_is_not_counted_twice() {
        let mut osm = small_objstms(1);
        osm[0].pad = 2 * MIB;
        let expected = osm[0].plain_len();
        let mut objs = base_objects(&extra_refs(&osm));
        objs[3] = (4, stream("/Filter /FlateDecode", &zlib_zeros(2 * MIB)));
        let pdf = modern(objs, &osm).bytes;
        let c = roomy_caps();
        let r = assert_ok(check_pdf(&pdf, &c));
        assert_eq!(r.objstm_decompressed_bytes, expected);
        assert_work_unencrypted(&r, &c);
    }

    /// Ramo cifrato: object stream da circa 2 MiB e contenuto da 2 MiB condiviso da due
    /// pagine, contati una volta (≤ 5 MiB); `objstm_decompressed_bytes` è la somma reale.
    #[test]
    fn i1_encrypted_counts_distinct_streams_once() {
        let page = "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 100 0 R >>";
        let plain = build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R /Extra 5 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [10 0 R 11 0 R] /Count 2 >>")),
                (5, pad_dict(2 * MIB as usize)),
                (10, dict(page)),
                (11, dict(page)),
                (100, stream("/Filter /FlateDecode", &zlib_zeros(2 * MIB))),
            ],
            "",
        );
        let pdf = encrypt_empty_user(&plain, "--object-streams=generate");
        let (containers, real) = objstm_containers(&lopdf_doc(&pdf));
        assert!(
            containers >= 1 && real >= 2 * MIB,
            "fixture: {containers} {real}"
        );
        let c = roomy_caps();
        let r = assert_ok(check_pdf(&pdf, &c));
        assert_eq!(r.encryption, Encryption::EmptyPassword);
        assert_eq!(r.objstm_decompressed_bytes, real);
        assert_work_encrypted(&r, &c);
    }

    // ---- TEST-13: I2 con più cross-reference stream -------------------------------------

    /// Due revisioni con xref stream: `xref_streams` = 2 e B cresce di conseguenza; il lavoro
    /// non scende mai sotto le due passate di caricamento delle xref stream.
    #[test]
    fn i2_work_counts_every_xref_stream() {
        let osm = small_objstms(3);
        let base = modern(base_objects(&extra_refs(&osm)), &osm);
        let rev = append_xref_revision(&base, vec![(4, stream("", b"BT ET"))], &[]);
        let c = caps();
        let r = assert_ok(check_pdf(&rev.bytes, &c));
        assert_eq!(r.xref_streams, 2);
        assert_work_unencrypted(&r, &c);
    }

    // ---- TEST-7: contenuti attivi ------------------------------------------------------

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

    /// Quarantena sul contenuto decifrato: JS nascosto in un object stream cifrato.
    #[test]
    fn javascript_hidden_in_encrypted_objstm_is_quarantined() {
        let plain = doc(
            1,
            "/OpenAction 50 0 R",
            vec![(50, dict("<< /S /JavaScript /JS (app.alert\\(1\\)) >>"))],
            "",
        );
        let pdf = encrypt_empty_user(&plain, "--object-streams=generate");
        assert!(
            !pdf.windows(10).any(|w| w == b"JavaScript"),
            "fixture: il JS deve stare solo nell'object stream cifrato"
        );
        let r = reasons_of(check_pdf(&pdf, &caps()));
        assert!(r.contains(&"javascript".to_owned()), "motivi {r:?}");
        assert!(r.contains(&"open_action_js".to_owned()), "motivi {r:?}");
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

    // ---- TEST-7: catene di filtri ------------------------------------------------------

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

    // ---- TEST-7: tetti di decompressione Flate -----------------------------------------

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

    // ---- TEST-14: caratterizzazione di lopdf 0.45.0 (c1-c7) ------------------------------

    /// (c1) La versione risolta di lopdf è 0.45.0 (equivalente a `cargo tree -p s02-parser -i
    /// lopdf` nel runner, letto dal `Cargo.lock` del workspace).
    #[test]
    fn c1_resolved_lopdf_is_0_45_0() {
        let lock = include_str!("../../Cargo.lock");
        let versions: Vec<&str> = lock
            .split("[[package]]")
            .filter(|p| p.contains("\nname = \"lopdf\"\n"))
            .filter_map(|p| {
                p.lines()
                    .find_map(|l| l.strip_prefix("version = \""))
                    .and_then(|v| v.strip_suffix('"'))
            })
            .collect();
        assert_eq!(versions, vec!["0.45.0"]);
    }

    /// (c2) Ramo non cifrato con `defer_objstm`: i membri non entrano in `Document::objects`,
    /// il contenitore resta compresso (il filtro agisce prima dell'espansione), gli oggetti
    /// normali sono uguali al caricamento senza filtro.
    #[test]
    fn c2_defer_objstm_keeps_objstm_compressed_on_unencrypted_load() {
        let member: &[u8] = b"<< /Foo 1 >>";
        let pdf = doc(1, "", vec![(60, objstm(&[(70, member)]))], "");
        let opts = LoadOptions {
            filter: Some(defer_objstm),
            ..LoadOptions::with_max_decompressed_size(MIB as usize)
        };
        let deferred = Document::load_mem_with_options(&pdf, opts).unwrap();
        let full = lopdf_doc(&pdf);
        assert!(
            full.objects.contains_key(&(70, 0)),
            "fixture: membro atteso"
        );
        assert!(
            !deferred.objects.contains_key(&(70, 0)),
            "membro espanso nonostante il filtro"
        );
        let Some(Object::Stream(container)) = deferred.objects.get(&(60, 0)) else {
            panic!("contenitore 60 assente o non stream");
        };
        let Some(Object::Stream(original)) = full.objects.get(&(60, 0)) else {
            panic!("fixture: contenitore 60");
        };
        assert_eq!(
            container.content, original.content,
            "contenitore non compresso"
        );
        assert!(matches!(
            container.dict.get(b"Filter"),
            Ok(Object::Name(n)) if n == b"FlateDecode"
        ));
        for id in [(1, 0), (2, 0), (10, 0), (100, 0)] {
            assert_eq!(
                deferred.objects.get(&id),
                full.objects.get(&id),
                "oggetto {id:?}"
            );
        }
    }

    /// (c2) Sugli oggetti che non sono object stream il filtro restituisce l'oggetto invariato.
    #[test]
    fn c2_defer_objstm_returns_other_objects_unchanged() {
        let doc_ = lopdf_doc(&doc(1, "", vec![], ""));
        for (id, obj) in &doc_.objects {
            let mut o = obj.clone();
            let back = defer_objstm(*id, &mut o).expect("oggetto scartato dal filtro");
            assert_eq!(back, (*id, obj.clone()), "oggetto {id:?} cambiato");
        }
    }

    /// (c3) Ramo cifrato con password vuota: lopdf ignora il filtro ed espande gli object
    /// stream nel caricamento. Si registra (stderr) se i contenitori restano in
    /// `Document::objects`, assunzione del prescan che deepsearch_6 non chiude.
    #[test]
    fn c3_encrypted_load_ignores_filter_and_expands_members() {
        let osm = small_objstms(3);
        let plain = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let pdf = encrypt_empty_user(&plain, "--object-streams=preserve");
        let opts = LoadOptions {
            filter: Some(defer_objstm),
            ..LoadOptions::with_max_decompressed_size(MIB as usize)
        };
        let d = Document::load_mem_with_options(&pdf, opts).unwrap();
        assert!(d.was_encrypted() && !d.is_encrypted());
        let compressed: Vec<(u32, u32)> = d
            .reference_table
            .entries
            .iter()
            .filter_map(|(n, e)| match e {
                lopdf::xref::XrefEntry::Compressed { container, .. } => Some((*n, *container)),
                _ => None,
            })
            .collect();
        assert!(!compressed.is_empty(), "fixture: nessun membro compresso");
        for (member, _) in &compressed {
            assert!(
                d.objects.contains_key(&(*member, 0)),
                "membro {member} non espanso"
            );
        }
        let kept = compressed
            .iter()
            .filter(|(_, c)| d.objects.contains_key(&(*c, 0)))
            .count();
        eprintln!(
            "c3: contenitori rimasti in Document::objects: {kept}/{} membri",
            compressed.len()
        );
    }

    /// (c3) Ramo cifrato: un object stream sopra la quota per stream (min(tetto,
    /// ⌊totale/n⌋) = 512 KiB con 8 object stream) è `DecompressionLimit`/`objstm_dropped`.
    #[test]
    fn c3_encrypted_objstm_over_quota_is_objstm_dropped() {
        let mut osm = small_objstms(8);
        osm[7].members[0].1 = pad_dict(600 * KIB as usize);
        let plain = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let pdf = encrypt_empty_user(&plain, "--object-streams=preserve");
        let (containers, _) = objstm_containers(&lopdf_doc(&pdf));
        assert_eq!(containers, 8, "fixture: object stream dopo qpdf");
        assert_eq!(encrypted_quota(&caps(), 8), 512 * KIB);
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "objstm_dropped",
        );
    }

    /// Ritocca l'ultimo trailer classico di un PDF di `qpdf` e aggiunge una revisione che
    /// sostituisce l'oggetto `updated` (senza stringhe: la decifratura non lo cambia) e libera
    /// `freed`.
    fn append_classic_revision(base: &[u8], updated: u32, freed: u32) -> Vec<u8> {
        let tpos = base
            .windows(7)
            .rposition(|w| w == b"trailer")
            .expect("trailer classico");
        let spos = tpos + find(&base[tpos..], b"startxref").unwrap();
        let trailer = String::from_utf8_lossy(&base[tpos + 7..spos])
            .trim()
            .to_owned();
        let prev: usize = String::from_utf8_lossy(&base[spos + 9..])
            .split_whitespace()
            .next()
            .unwrap()
            .parse()
            .unwrap();
        let mut out = base.to_vec();
        let off = out.len();
        push_obj(&mut out, updated, b"<< /K 2 >>");
        let xoff = out.len();
        let mut subs = [
            (updated, format!("{off:010} 00000 n \n")),
            (freed, "0000000000 00001 f \n".to_owned()),
        ];
        subs.sort_by_key(|(n, _)| *n);
        out.extend_from_slice(b"xref\n");
        for (n, row) in subs {
            out.extend_from_slice(format!("{n} 1\n{row}").as_bytes());
        }
        let trailer = trailer.replacen("<<", &format!("<< /Prev {prev}"), 1);
        out.extend_from_slice(format!("trailer\n{trailer}\nstartxref\n{xoff}\n%%EOF\n").as_bytes());
        out
    }

    /// (c4) xref in tabella, xref in stream e aggiornamento incrementale con due revisioni:
    /// la riconciliazione del ramo cifrato considera solo le voci in uso dell'ultima
    /// revisione (nessun falso `object_dropped`); `xref_streams` uguale al numero reale.
    #[test]
    fn c4_xref_forms_and_incremental_update_have_no_false_drops() {
        let plain = doc(
            1,
            "/Extra 5 0 R /Extra2 6 0 R",
            vec![(5, dict("<< /K 1 >>")), (6, dict("<< /K 9 >>"))],
            "",
        );
        let c = caps();

        // Fixture, verificate con lopdf prima del prescan.
        let table = encrypt_empty_user(&plain, "--object-streams=disable");
        let in_stream = encrypt_empty_user(&plain, "--object-streams=generate");
        let d = lopdf_doc(&table);
        let cat = d.catalog().unwrap();
        let x = cat.get(b"Extra").unwrap().as_reference().unwrap().0;
        let y = cat.get(b"Extra2").unwrap().as_reference().unwrap().0;
        let two_revs = append_classic_revision(&table, x, y);
        let d2 = lopdf_doc(&two_revs);
        assert!(
            d2.was_encrypted() && !d2.is_encrypted(),
            "fixture: revisione 2"
        );
        // lopdf unisce le revisioni: si registra come vede la voce liberata (caratterizzazione).
        eprintln!(
            "c4: voce {y} liberata nella revisione 2, vista da lopdf: {:?}",
            d2.reference_table.get(y)
        );
        qpdf_check(&two_revs);
        assert_eq!(
            d2.objects
                .get(&(x, 0))
                .and_then(|o| o.as_dict().ok())
                .map(|o| o.get(b"K").ok().cloned()),
            Some(Some(Object::Integer(2))),
            "fixture: {x} sostituito dalla revisione 2"
        );
        let osm = small_objstms(2);
        let base = modern(base_objects(&extra_refs(&osm)), &osm);
        let (m0, m1) = (osm[0].members[0].0, osm[1].members[0].0);
        let rev = append_xref_revision(&base, vec![(m0, dict("<< /K 7 >>"))], &[m1]);
        qpdf_check(&rev.bytes);
        let d3 = lopdf_doc(&rev.bytes);
        assert!(
            d3.objects.contains_key(&(m0, 0)),
            "fixture: revisione xref stream"
        );

        let r = assert_ok(check_pdf(&table, &c));
        assert_eq!(
            (r.encryption, r.xref_streams),
            (Encryption::EmptyPassword, 0)
        );
        let r = assert_ok(check_pdf(&in_stream, &c));
        assert_eq!(
            (r.encryption, r.xref_streams),
            (Encryption::EmptyPassword, 1)
        );
        let r = assert_ok(check_pdf(&two_revs, &c));
        assert_eq!(
            (r.encryption, r.xref_streams),
            (Encryption::EmptyPassword, 0)
        );
        // Non cifrato, due revisioni con xref stream: contate tutte e due.
        let r = assert_ok(check_pdf(&rev.bytes, &c));
        assert_eq!(r.xref_streams, 2);
    }

    /// (c5) Ramo cifrato, oggetto in uso illeggibile (lopdf lo scarta in silenzio):
    /// `Corrupt`/`object_dropped`.
    #[test]
    fn c5_encrypted_unreadable_object_is_object_dropped() {
        let mut pdf = encrypt_empty_user(&doc(1, "", vec![], ""), "--object-streams=disable");
        let d = lopdf_doc(&pdf);
        let page = *d.get_pages().values().next().expect("fixture: pagina");
        let contents = d
            .get_object(page)
            .unwrap()
            .as_dict()
            .unwrap()
            .get(b"Contents")
            .unwrap()
            .as_reference()
            .unwrap();
        let off = offset_of(&d, contents.0);
        let kw = off + find(&pdf[off..], b" obj").expect("intestazione obj") + 1;
        pdf[kw..kw + 3].copy_from_slice(b"obx");
        // lopdf, sul ramo cifrato, scarta l'oggetto in silenzio e restituisce Ok.
        let dropped = lopdf_doc(&pdf);
        assert!(
            !dropped.objects.contains_key(&contents),
            "fixture: oggetto {contents:?} ancora caricato"
        );
        assert_failed(check_pdf(&pdf, &caps()), Kind::Corrupt, "object_dropped");
    }

    /// (c5) Ramo cifrato, object stream con Flate corrotto (blocchi AES del testo cifrato
    /// alterati, padding intatto): `DecompressionLimit`/`objstm_dropped`.
    #[test]
    fn c5_encrypted_corrupt_objstm_is_objstm_dropped() {
        let extra: Vec<(u32, Vec<u8>)> = (0..50)
            .map(|i| {
                let v = hex(&noise(32, 100 + i));
                let mut body = b"<< /V <".to_vec();
                body.extend_from_slice(&v[..v.len() - 1]);
                body.extend_from_slice(b"> >>");
                (200 + u32::try_from(i).unwrap(), body)
            })
            .collect();
        let refs: Vec<String> = (200..250).map(|n| format!("{n} 0 R")).collect();
        let plain = doc(1, &format!("/Extra [{}]", refs.join(" ")), extra, "");
        let mut pdf = encrypt_empty_user(&plain, "--object-streams=generate");
        let d = lopdf_doc(&pdf);
        let container = d
            .reference_table
            .entries
            .values()
            .find_map(|e| match e {
                lopdf::xref::XrefEntry::Compressed { container, .. } => Some(*container),
                _ => None,
            })
            .expect("fixture: object stream");
        let start = stream_data_start(&pdf, offset_of(&d, container));
        let Some(Object::Stream(s)) = d.objects.get(&(container, 0)) else {
            panic!("fixture: contenitore");
        };
        assert!(s.content.len() > 200, "fixture: object stream troppo corto");
        for b in &mut pdf[start + 16..start + 80] {
            *b ^= 0xFF;
        }
        // lopdf, sul ramo cifrato, perde in silenzio i membri dell'object stream.
        let dropped = lopdf_doc(&pdf);
        let lost = dropped
            .reference_table
            .entries
            .iter()
            .filter(|(n, e)| {
                matches!(e, lopdf::xref::XrefEntry::Compressed { container: c, .. } if *c == container)
                    && !dropped.objects.contains_key(&(**n, 0))
            })
            .count();
        assert!(lost > 0, "fixture: nessun membro perso dopo la corruzione");
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "objstm_dropped",
        );
    }

    /// (c6) Password utente non vuota: `load_mem_with_options` dà `Ok` o `Err` (registrato su
    /// stderr) e l'esito è `Encrypted`/`password_required`, mai `Corrupt`/`object_dropped`.
    #[test]
    fn c6_user_password_is_password_required_never_object_dropped() {
        let osm = small_objstms(3);
        let plain = modern(base_objects(&extra_refs(&osm)), &osm).bytes;
        let pdf = qpdf(
            &plain,
            &[
                "--object-streams=preserve",
                "--encrypt",
                USER_PW,
                OWNER_PW,
                "256",
                "--",
            ],
        );
        let load = Document::load_mem_with_options(
            &pdf,
            LoadOptions::with_max_decompressed_size(MIB as usize),
        );
        eprintln!(
            "c6: load_mem_with_options senza password -> {}",
            match &load {
                Ok(d) => format!(
                    "Ok (is_encrypted={}, oggetti={})",
                    d.is_encrypted(),
                    d.objects.len()
                ),
                Err(e) => format!("Err({e})"),
            }
        );
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::Encrypted,
            "password_required",
        );
    }

    /// (c6) Gestore non standard (`/Adobe.PubSec`): `Encrypted`/`password_required`.
    #[test]
    fn c6_pubsec_handler_is_password_required() {
        let pdf = doc(
            1,
            "",
            vec![(
                50,
                dict(
                    "<< /Filter /Adobe.PubSec /SubFilter /adbe.pkcs7.s4 /V 4 /R 4 /Recipients [<3082000A>] >>",
                ),
            )],
            "/Encrypt 50 0 R /ID [<01> <01>]",
        );
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::Encrypted,
            "password_required",
        );
    }

    /// (c7) Ramo non cifrato, stream Flate non decodificabile: `Corrupt`, nessuno stream
    /// saltato.
    #[test]
    fn c7_undecodable_flate_stream_is_corrupt() {
        let pdf = doc_with_content("/Filter /FlateDecode", b"this is not zlib data at all");
        assert_eq!(kind_of(check_pdf(&pdf, &caps())), Kind::Corrupt);
    }

    // ---- DOCX (TEST-7, schema di TEST-15) -----------------------------------------------
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

    /// DOCX benigno: i sei campi dello schema unico valgono `none` e 0.
    #[test]
    fn benign_docx_passes_with_none_and_zero_schema() {
        let r = assert_ok(check_docx(&write_docx(&base_docx()), &caps()));
        assert_eq!(r.encryption, Encryption::None);
        assert_eq!(
            (
                r.embedded_files,
                r.objstm_count,
                r.objstm_decompressed_bytes,
                r.xref_streams,
                r.decompress_work_bytes
            ),
            (0, 0, 0, 0, 0)
        );
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
