//! Prescan anti-bomb di PDF e DOCX, nel figlio prima di PDFium (contratto W17, punto 5).
//!
//! Gira nel figlio isolato, sotto i suoi limiti: anche il prescan legge input ostile.
//!
//! PDF (Task 3 del piano 01-07, regole del ripasso del 2026-10-21, plan_version 6):
//! 1. scansione a byte con nomi normalizzati (escape `#xx` risolti), prima di ogni
//!    caricamento: `/Encrypt` dichiarato, `/ObjStm` e cross-reference stream (`/XRef`)
//!    contati, nomi attivi (`/JavaScript`, `/JS`, `/Launch`). Nessuno dei conteggi è un
//!    motivo di rifiuto: servono alla quota del ramo cifrato e al limite B dell'invariante I2;
//! 2. PDF non cifrato: prima passata con `defer_objstm` (gli object stream restano compressi),
//!    decodifica in streaming di ogni stream con un contatore unico dei byte distinti
//!    (invariante I1, arresto appena supera il totale), poi il caricamento vero, che riespande
//!    gli stessi object stream già misurati (entrano solo nel lavoro I2);
//!    PDF cifrato: caricamento con quota per stream min(tetto, ⌊totale / object stream⌋),
//!    ammesso solo se lopdf l'apre con la password vuota; ordine fisso dei controlli
//!    (`password_required`, `encrypt_state`, riconciliazione con la xref, contenuto);
//! 3. pagine (tetto → `ResourceLimit`), catene di filtri, tetti per stream, totale e
//!    rapporto, contenuti attivi → quarantena con motivi fissi; gli allegati incorporati si
//!    contano e non causano quarantena. I filtri immagine sono ammessi, come ultimo filtro
//!    e con il tetto `max_image_bytes`, su `/Subtype /Image` e, dalla gap-closure 1.4, sulla
//!    forma T: il dizionario d'immagine miniatura senza `/Subtype` (`is_subtypeless_image`).
//!    Dalla gap-closure 1.5, sui DCT e sui JPX il tetto vale anche sulle dimensioni codificate
//!    nei dati (marker SOF del JPEG, marker SIZ del codestream JPEG 2000), lette entro
//!    `IMAGE_HEADER_SCAN_BYTES` dopo i soli filtri che precedono il filtro immagine.
//!
//! I2 (lavoro cumulativo di tutte le passate) è `decompress_work_bytes`: byte contati più i
//! limiti analitici delle espansioni del loader, mai una stima più bassa del reale; oltre il
//! limite B = 2 × totale + 2 × `xref_streams` × tetto per stream è un errore di programma
//! (`Corrupt`, detail `work_bound`).
//!
//! DOCX: voci, somma dichiarata, macro, `<!DOCTYPE`/`<!ENTITY`, relazioni esterne.

// Modifica: Claude (gsd-executor 01.4-02) · 2026-10-23 · filtri immagine ammessi anche sulla forma T senza /Subtype, con lo stesso tetto (gap-closure 1.4, D-07)
// Modifica: Claude (gsd-executor 01.5-01) · 2026-10-23 · tetto delle immagini DCT e JPX sulle dimensioni codificate (gap-closure 1.5, D-01, D-02, D-09, D-10)
use std::collections::BTreeSet;
use std::io::{self, Read};
use std::path::Path;

use lopdf::xref::XrefEntry;
use lopdf::{Document, LoadOptions, Object, ObjectId};

use crate::protocol::Kind;

const MIB: u64 = 1024 * 1024;

/// Blocco di lettura del contatore I1: l'arresto avviene entro un blocco oltre il totale.
const READ_BLOCK: usize = 16 * 1024;

/// Nome privato del `/Type` degli object stream nella prima passata del ramo non cifrato.
const DEFERRED_OBJSTM: &[u8] = b"S02DeferredObjStm";

/// Motivi di quarantena (stringhe fisse del contratto, nell'ordine di emissione). Dal
/// ripasso del 2026-10-21 un allegato incorporato non è più un motivo di quarantena.
pub const REASON_JAVASCRIPT: &str = "javascript";
pub const REASON_LAUNCH: &str = "launch";
pub const REASON_OPEN_ACTION_JS: &str = "open_action_js";
pub const REASON_EXTERNAL_LINK: &str = "external_link";
const REASON_ORDER: [&str; 4] = [
    REASON_JAVASCRIPT,
    REASON_LAUNCH,
    REASON_OPEN_ACTION_JS,
    REASON_EXTERNAL_LINK,
];

/// Tetti del prescan (Soglie del lanciatore, Task 1 di 01-07).
#[derive(Debug, Clone)]
pub struct Caps {
    /// Tetto per stream, anche al caricamento (`max_decompressed_size`): 64 MiB.
    pub per_stream_bytes: u64,
    /// Somma reale dei byte decompressi distinti (I1), object stream compresi: 512 MiB.
    pub total_bytes: u64,
    /// Rapporto massimo di espansione: 100.
    pub max_ratio: u64,
    /// Il rapporto vale oltre questa soglia di byte espansi: 16 MiB.
    pub ratio_min_bytes: u64,
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

impl Default for Caps {
    fn default() -> Self {
        Caps {
            per_stream_bytes: 64 * MIB,
            total_bytes: 512 * MIB,
            max_ratio: 100,
            ratio_min_bytes: 16 * MIB,
            max_filter_chain: 2,
            max_pages: 2000,
            max_image_bytes: 64 * MIB,
            docx_max_entries: 1000,
            docx_max_total_bytes: 200 * MIB,
        }
    }
}

/// Esito positivo del prescan, con le grandezze che la calibrazione registra.
///
/// Schema unico del rapporto (punto 5 di `<interfaces>` del piano, v6): `encryption`,
/// `embedded_files`, `objstm_count`, `objstm_decompressed_bytes`, `xref_streams`,
/// `decompress_work_bytes`. Per un DOCX valgono `none` e 0.
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
    /// Somma reale dei byte decompressi distinti (I1); per i DOCX i byte letti dalle parti.
    pub total_decompressed_bytes: u64,
    /// Voci dell'archivio (DOCX).
    pub entries: usize,
    /// Cifratura ammessa: `none` | `empty_password`.
    pub encryption: Encryption,
    /// Allegati incorporati trovati e ignorati.
    pub embedded_files: u32,
    /// Somma reale dei byte decompressi degli object stream (parte di I1).
    pub objstm_decompressed_bytes: u64,
    /// Cross-reference stream contati a byte.
    pub xref_streams: u32,
    /// Lavoro cumulativo di decompressione (I2), mai sotto il valore reale.
    pub decompress_work_bytes: u64,
}

/// Cifratura di un PDF ammesso (schema unico, campo `encryption`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Encryption {
    /// Nessun `/Encrypt` (anche ogni DOCX).
    #[default]
    None,
    /// Cifrato, aperto con la password vuota (`empty_password`).
    EmptyPassword,
}

impl Encryption {
    /// Valore del campo `encryption` nel `data` del figlio.
    pub fn as_str(self) -> &'static str {
        match self {
            Encryption::None => "none",
            Encryption::EmptyPassword => "empty_password",
        }
    }
}

/// Filtro di `LoadOptions` della prima passata del ramo non cifrato (punto 2 del Task 3):
/// rinomina il `/Type` degli object stream perché lopdf 0.45.0 non li espanda (il filtro
/// gira su ogni oggetto prima dell'espansione, `load_objects_raw`); ogni altro oggetto torna
/// invariato. Senza stato: il nome privato basta a riconoscere i contenitori dopo il
/// caricamento.
fn defer_objstm(id: ObjectId, obj: &mut Object) -> Option<(ObjectId, Object)> {
    if let Object::Stream(s) = obj
        && s.dict.has_type(b"ObjStm")
    {
        s.dict.set("Type", Object::Name(DEFERRED_OBJSTM.to_vec()));
    }
    Some((id, obj.clone()))
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
    xref: u32,
    javascript: bool,
    launch: bool,
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

/// Un object stream non sta dentro un altro e i dizionari non sono cifrati: i conteggi di
/// `/ObjStm` e `/XRef` sono completi anche sui PDF cifrati (in eccesso, mai in difetto, se il
/// nome compare anche altrove).
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
            b"XRef" => out.xref = out.xref.saturating_add(1),
            b"JavaScript" => out.javascript = true,
            b"Launch" => out.launch = true,
            b"JS" if !in_body(start) => out.javascript = true,
            _ => {}
        }
    }
    out
}

// ---- invarianti I1 e I2 ----------------------------------------------------------------

/// Contatore unico dei byte decompressi distinti (I1) e del lavoro cumulativo (I2).
#[derive(Debug)]
struct Meter {
    /// Byte decompressi distinti contati in streaming (I1).
    counted: u64,
    /// Lavoro di tutte le passate: contati, riespansioni e limiti analitici del loader (I2).
    work: u64,
    total_cap: u64,
    bound: u64,
}

impl Meter {
    fn new(caps: &Caps, xref_streams: u32) -> Meter {
        Meter {
            counted: 0,
            work: 0,
            total_cap: caps.total_bytes,
            bound: work_bound(caps, xref_streams),
        }
    }

    /// Lavoro senza byte distinti nuovi (riespansioni, limiti analitici del loader).
    fn add_work(&mut self, n: u64) -> Result<(), Rejection> {
        self.work = self.work.saturating_add(n);
        if self.work > self.bound {
            return Err(fail(
                Kind::Corrupt,
                format!("work_bound work={} bound={}", self.work, self.bound),
            ));
        }
        Ok(())
    }

    /// Byte distinti appena decompressi: entrano in I1 e in I2; I1 si ferma appena supera
    /// il totale (`total_decompressed`).
    fn add_distinct(&mut self, n: u64) -> Result<(), Rejection> {
        self.counted = self.counted.saturating_add(n);
        self.work = self.work.saturating_add(n);
        if self.counted > self.total_cap {
            return Err(fail(
                Kind::DecompressionLimit,
                format!(
                    "total_decompressed counted={} work={}",
                    self.counted, self.work
                ),
            ));
        }
        self.add_work(0)
    }
}

/// Limite B di I2: 2 × totale + 2 × `xref_streams` × tetto per stream.
fn work_bound(caps: &Caps, xref_streams: u32) -> u64 {
    caps.total_bytes.saturating_mul(2).saturating_add(
        u64::from(xref_streams)
            .saturating_mul(2)
            .saturating_mul(caps.per_stream_bytes),
    )
}

/// Quota per stream del ramo cifrato: min(tetto per stream, ⌊totale / object stream⌋).
fn encrypted_quota(caps: &Caps, objstm: u32) -> u64 {
    match objstm {
        0 => caps.per_stream_bytes,
        n => caps.per_stream_bytes.min(caps.total_bytes / u64::from(n)),
    }
}

// ---- caricamento -----------------------------------------------------------------------

fn is_memory_limit(e: &lopdf::Error) -> bool {
    matches!(
        e,
        lopdf::Error::Decompress(lopdf::DecompressError::MemoryLimitExceeded { .. })
    )
}

fn as_usize(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

/// Caricamento del ramo non cifrato con tetto per stream. In modalità tollerante lopdf
/// 0.45.0 scarta in silenzio un object stream oltre il tetto (lo registra solo nel log),
/// quindi si carica prima in modalità `strict`, dove l'errore
/// `Error::Decompress(DecompressError::MemoryLimitExceeded)` arriva al chiamante
/// (→ `DecompressionLimit`); un altro errore stretto ripiega sul caricamento tollerante, che
/// deve comunque contenere ogni object stream citato dalla xref (altrimenti `Corrupt`).
fn load_plain(
    bytes: &[u8],
    caps: &Caps,
    filter: Option<lopdf::FilterFunc>,
) -> Result<Document, Rejection> {
    let lenient = LoadOptions {
        filter,
        ..LoadOptions::with_max_decompressed_size(as_usize(caps.per_stream_bytes))
    };
    let strict = LoadOptions {
        strict: true,
        ..lenient.clone()
    };
    match Document::load_mem_with_options(bytes, strict) {
        Ok(doc) => Ok(doc),
        Err(e) if is_memory_limit(&e) => Err(fail(
            Kind::DecompressionLimit,
            format!("load_per_stream: {e}"),
        )),
        Err(_) => {
            let doc = Document::load_mem_with_options(bytes, lenient).map_err(|e| {
                if is_memory_limit(&e) {
                    fail(Kind::DecompressionLimit, format!("load_per_stream: {e}"))
                } else {
                    fail(Kind::Corrupt, format!("load: {e}"))
                }
            })?;
            let missing = doc.reference_table.entries.values().any(|entry| {
                matches!(entry, XrefEntry::Compressed { container, .. }
                    if !matches!(doc.objects.get(&(*container, 0)), Some(Object::Stream(_))))
            });
            if missing {
                return Err(fail(Kind::Corrupt, "objstm_unloadable"));
            }
            Ok(doc)
        }
    }
}

/// Errori di lopdf legati alla cifratura (password, gestore o algoritmo non supportati).
fn is_password_error(e: &lopdf::Error) -> bool {
    matches!(
        e,
        lopdf::Error::InvalidPassword
            | lopdf::Error::Decryption(_)
            | lopdf::Error::Unimplemented(_)
    )
}

/// Riconciliazione del ramo cifrato (punto 3-bis): lopdf 0.45.0 scarta in silenzio oggetti
/// illeggibili, object stream non espandibili ed errori di decifratura, quindi ogni voce in
/// uso della xref deve risultare caricata, salvo il dizionario `/Encrypt` che lopdf rimuove.
fn reconcile(doc: &Document) -> Result<(), Rejection> {
    let encrypt_id = doc
        .encryption_state
        .as_ref()
        .and_then(|s| s.encrypt_object_id());
    let mut member: Option<(u32, u32)> = None;
    let mut object: Option<ObjectId> = None;
    for (&num, entry) in &doc.reference_table.entries {
        match *entry {
            XrefEntry::Compressed { container, .. } => {
                if member.is_none() && !doc.objects.contains_key(&(num, 0)) {
                    member = Some((num, container));
                }
            }
            XrefEntry::Normal { generation, .. } => {
                let id = (num, generation);
                if object.is_none() && Some(id) != encrypt_id && !doc.objects.contains_key(&id) {
                    object = Some(id);
                }
            }
            XrefEntry::Free | XrefEntry::UnusableFree => {}
        }
    }
    if let Some((num, container)) = member {
        return Err(fail(
            Kind::DecompressionLimit,
            format!("objstm_dropped object={num} container={container}"),
        ));
    }
    if let Some((num, generation)) = object {
        return Err(fail(
            Kind::Corrupt,
            format!("object_dropped object={num} generation={generation}"),
        ));
    }
    Ok(())
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

/// Chiavi ammesse nel dizionario di uno stream della forma T: quelle di un'immagine
/// miniatura, niente di più.
const IMAGE_LIKE_KEYS: [&[u8]; 9] = [
    b"Type",
    b"Length",
    b"Filter",
    b"DecodeParms",
    b"Width",
    b"Height",
    b"ColorSpace",
    b"BitsPerComponent",
    b"Decode",
];

/// Filtri immagine ammessi sulla forma T: i soli osservati dalla diagnosi. `JBIG2Decode` e
/// `CCITTFaxDecode` restano fuori per costruzione, perché quei decoder leggono le dimensioni
/// dai segmenti incorporati o da `/DecodeParms` e non da `/Width` e `/Height`: il tetto
/// `max_image_bytes` non li limiterebbe davvero.
const SUBTYPELESS_IMAGE_FILTERS: [&[u8]; 1] = [b"DCTDecode"];

/// Vero se la chiave è un intero diretto e positivo (un riferimento indiretto, un reale, lo
/// zero o un negativo non bastano).
fn positive_int(dict: &lopdf::Dictionary, key: &[u8]) -> bool {
    matches!(dict.get(key), Ok(Object::Integer(v)) if *v > 0)
}

/// Forma T: dizionario d'immagine senza `/Subtype`, con un filtro immagine ammesso.
///
/// Motivo. La gap-closure 1.4 (piano 01.4-02) ha diagnosticato sulle 4 pagine DocLayNet di
/// `OTC_NSANY_2004` (01-08) uno stream miniatura di pagina: oggetto `6 0`, riferito dalla
/// chiave `/Thumb` della pagina (oggetto 3), dizionario `/BitsPerComponent 8 /ColorSpace
/// <ref a /DeviceRGB> /Filter [/ASCII85Decode /DCTDecode] /Width 74 /Height 105 /Length`,
/// senza `/Type` né `/Subtype`, come il formato PDF descrive le miniature. La regola di 01-07
/// lo respingeva con `image_filter_on_non_image`; l'utente l'ha classificato come falso
/// positivo (D-37 di `01-CONTEXT.md`) e la regola si allarga solo a questa forma (D-07 di
/// `01.4-CONTEXT.md`): `/Subtype` assente, `/Type` assente o `/XObject`, `/Width` e
/// `/Height` interi positivi diretti, chiavi solo fra `IMAGE_LIKE_KEYS`, filtro fra
/// `SUBTYPELESS_IMAGE_FILTERS`. Che il filtro sia l'ultimo della catena lo controlla
/// `check_chain`, che poi applica lo stesso tetto `max_image_bytes` del ramo `/Image`.
///
/// La forma si riconosce dal solo dizionario e non dal riferimento `/Thumb`: la prima
/// passata del ramo non cifrato tiene compressi gli object stream (`defer_objstm`), quindi
/// un dizionario di pagina dentro un object stream non sarebbe visibile al momento del
/// controllo.
fn is_subtypeless_image(dict: &lopdf::Dictionary, filter: &[u8]) -> bool {
    SUBTYPELESS_IMAGE_FILTERS.contains(&filter)
        && dict.get(b"Subtype").is_err()
        && match dict.get(b"Type") {
            Err(_) => true,
            Ok(Object::Name(n)) => n == b"XObject",
            Ok(_) => false,
        }
        && positive_int(dict, b"Width")
        && positive_int(dict, b"Height")
        && dict
            .iter()
            .all(|(k, _)| IMAGE_LIKE_KEYS.contains(&k.as_slice()))
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

/// Lettore che conta i byte prodotti (Flate interno di una catena `[Flate, Flate]`: lavoro
/// I2, non byte distinti del documento).
struct Tally<R> {
    inner: R,
    bytes: u64,
}

impl<R: Read> Read for Tally<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes = self.bytes.saturating_add(n as u64);
        Ok(n)
    }
}

/// Byte prodotti da `r` fino a `cap + 1`, letti a blocchi fissi e scartati, ciascun blocco
/// sommato subito al contatore I1. Un errore di decodifica è `Corrupt`: nessuno stream si
/// salta.
fn drain_counted<R: Read>(r: R, cap: u64, meter: &mut Meter) -> Result<u64, Rejection> {
    let mut limited = r.take(cap.saturating_add(1));
    let mut buf = [0u8; READ_BLOCK];
    let mut n = 0u64;
    loop {
        match limited.read(&mut buf) {
            Ok(0) => return Ok(n),
            Ok(k) => {
                n += k as u64;
                meter.add_distinct(k as u64)?;
            }
            Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
            Err(e) => return Err(fail(Kind::Corrupt, format!("flate_decode: {e}"))),
        }
    }
}

/// Espansione Flate di uno stream secondo la sua catena di filtri (già validata).
/// I filtri prima del primo Flate riducono e si applicano sul buffer; Flate annidati si
/// leggono in streaming; i filtri dopo l'ultimo Flate non espandono e non si decodificano.
fn flate_expansion(
    chain: &[Vec<u8>],
    raw: &[u8],
    cap: u64,
    meter: &mut Meter,
) -> Result<Option<u64>, Rejection> {
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
    if flates < 2 {
        return drain_counted(flate2::read::ZlibDecoder::new(data), cap, meter).map(Some);
    }
    let mut inner = Tally {
        inner: flate2::read::ZlibDecoder::new(data).take(cap.saturating_add(1)),
        bytes: 0,
    };
    let n = drain_counted(flate2::read::ZlibDecoder::new(&mut inner), cap, meter)?;
    meter.add_work(inner.bytes)?;
    if inner.bytes > cap {
        return Err(fail(
            Kind::DecompressionLimit,
            format!("per_stream oltre {cap} (flate interno)"),
        ));
    }
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
                if !(is_image(dict) || is_subtypeless_image(dict, f)) || i + 1 != chain.len() {
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

// ---- dimensioni codificate delle immagini DCT e JPX (gap-closure 1.5) --------------------

/// Segmento marker JPEG di lunghezza massima, marker compreso (2 + L massimo).
const MAX_MARKER_SEGMENT: usize = 2 + 65_535;

/// Finestra di lettura dell'intestazione di un'immagine: 262.925 byte di dati.
///
/// Motivo (D-10 di `01.5-CONTEXT.md`): copre il SOI, 4 segmenti APP pieni (per esempio JFIF,
/// EXIF, XMP e un pezzo di profilo ICC) e l'intestazione di un SOF con il massimo di 255
/// componenti (marker, L, P, Y, X, Nf e 3 × 255 byte di componenti). Per i JPX la stessa
/// finestra copre SOC più un SIZ massimo (2 + 2 + 38 + 3 × 16.384 = 49.194 byte) e, nel formato
/// JP2, i box che precedono `jp2c` (firma, `ftyp`, `jp2h` con un profilo ICC fino a circa
/// 200 KiB). Oltre la finestra il marker non si cerca: l'esito è `beyond_scan`.
const IMAGE_HEADER_SCAN_BYTES: usize = 2 + 4 * MAX_MARKER_SEGMENT + 2 + 8 + 3 * 255;

/// Prefisso dei dati grezzi passato ad `ascii85` o ad `ascii_hex` durante la lettura
/// dell'intestazione. Un prefisso troppo pieno di spazi produce `beyond_scan`, mai
/// un'ammissione.
const PREFILTER_RAW_BYTES: usize = 4 * IMAGE_HEADER_SCAN_BYTES;

/// Codec dell'immagine, dal filtro immagine in coda alla catena.
#[derive(Debug, Clone, Copy)]
enum Codec {
    Dct,
    Jpx,
}

impl Codec {
    /// Primo token del detail di rifiuto.
    fn detail(self) -> &'static str {
        match self {
            Codec::Dct => "dct_sof",
            Codec::Jpx => "jpx_siz",
        }
    }
}

/// Dimensioni codificate nei dati dell'immagine.
#[derive(Debug, Clone, Copy)]
struct Dims {
    width: u64,
    height: u64,
    components: u64,
}

/// Errore della lettura del prefisso: un sottomotivo del detail, oppure il rifiuto di
/// `Meter::add_work` (`Corrupt` / `work_bound`), propagato con il suo `Kind` e il suo detail
/// (T-01.5-09).
#[derive(Debug)]
enum PrefixError {
    Prefilter(&'static str),
    Work(Rejection),
}

/// Al più `IMAGE_HEADER_SCAN_BYTES` byte di dati dell'immagine, più `more`, vero se i dati
/// proseguono oltre. Applica in ordine i soli filtri che precedono il filtro immagine:
/// ASCII85 e ASCIIHex su un prefisso grezzo di `PREFILTER_RAW_BYTES` byte, Flate letto sempre
/// fino a `IMAGE_HEADER_SCAN_BYTES + 1` byte o alla fine dei dati, così la quantità letta è
/// deterministica. I byte prodotti da Flate entrano nel lavoro I2. L'immagine non si
/// decodifica mai.
fn image_prefix(
    chain: &[Vec<u8>],
    raw: &[u8],
    meter: &mut Meter,
) -> Result<(Vec<u8>, bool), PrefixError> {
    let pre = &chain[..chain.len().saturating_sub(1)];
    let mut owned: Option<Vec<u8>> = None;
    let mut more = false;
    for f in pre {
        let cur: &[u8] = owned.as_deref().unwrap_or(raw);
        let next = match f.as_slice() {
            b"ASCII85Decode" | b"ASCIIHexDecode" => {
                let cut = cur.len().min(PREFILTER_RAW_BYTES);
                more |= cut < cur.len();
                let decoded = if f.as_slice() == b"ASCII85Decode" {
                    ascii85(&cur[..cut])
                } else {
                    ascii_hex(&cur[..cut])
                };
                decoded.map_err(|_| PrefixError::Prefilter("prefilter"))?
            }
            b"FlateDecode" => {
                // Copia limitata da `take`: al più IMAGE_HEADER_SCAN_BYTES + 1 byte in memoria.
                // Equivale a `read_to_end` sullo stesso `Take`, che il piano prescriveva:
                // `io::copy` è stato scelto perché la regola lessicale `upload-in-memory`
                // dello scanner segnala `read_to_end` senza vedere il `take` (falso
                // positivo, verify della fase 1.5).
                let mut limited =
                    flate2::read::ZlibDecoder::new(cur).take(IMAGE_HEADER_SCAN_BYTES as u64 + 1);
                let mut out = Vec::new();
                let n = io::copy(&mut limited, &mut out)
                    .map_err(|_| PrefixError::Prefilter("prefilter"))?;
                meter.add_work(n).map_err(PrefixError::Work)?;
                out
            }
            _ => return Err(PrefixError::Prefilter("invalid")),
        };
        owned = Some(next);
    }
    let mut data = match owned {
        Some(d) => d,
        None => raw[..raw.len().min(IMAGE_HEADER_SCAN_BYTES + 1)].to_vec(),
    };
    if data.len() > IMAGE_HEADER_SCAN_BYTES {
        data.truncate(IMAGE_HEADER_SCAN_BYTES);
        more = true;
    }
    Ok((data, more))
}

/// Dimensioni del primo SOF di un JPEG, lette per segmenti dal SOI.
///
/// La lettura salta il contenuto di ogni segmento con la sua lunghezza: un SOF finto dentro
/// APP1 (la miniatura EXIF), DHT, DAC o JPG non si vede. Sono SOF `C0`-`C3`, `C5`-`C7`,
/// `C9`-`CB` e `CD`-`CF`; `C4` (DHT), `C8` (JPG) e `CC` (DAC) no. Il primo SOF decide.
/// Sottomotivi: `missing` (niente SOI, oppure SOI, EOI o SOS prima del SOF), `invalid`
/// (byte fuori marker, marker `00`, L < 2, SOF incoerente o con X, Y o Nf a 0), `truncated`
/// (dati finiti dentro un marker o un segmento), `beyond_scan` (finestra finita prima del SOF).
fn read_sof(data: &[u8], more: bool) -> Result<Dims, &'static str> {
    // Fine dei dati: dentro un marker o un segmento (`inside`) oppure su un confine.
    let end = |inside: bool| {
        if more {
            "beyond_scan"
        } else if inside {
            "truncated"
        } else {
            "missing"
        }
    };
    if data.len() < 2 {
        return Err(if more {
            "beyond_scan"
        } else if data == [0xFF] {
            "truncated"
        } else {
            "missing"
        });
    }
    if data[..2] != [0xFF, 0xD8] {
        return Err("missing");
    }
    let mut i = 2;
    loop {
        match data.get(i) {
            None => return Err(end(false)),
            Some(&0xFF) => {}
            Some(_) => return Err("invalid"),
        }
        // Byte FF di riempimento prima del codice del marker.
        let mut j = i + 1;
        while data.get(j) == Some(&0xFF) {
            j += 1;
        }
        let Some(&marker) = data.get(j) else {
            return Err(end(true));
        };
        i = j + 1;
        match marker {
            0x00 => return Err("invalid"),
            0x01 | 0xD0..=0xD7 => continue,
            0xD8..=0xDA => return Err("missing"),
            _ => {}
        }
        let Some(len) = data.get(i..i + 2) else {
            return Err(end(true));
        };
        let len = usize::from(u16::from_be_bytes([len[0], len[1]]));
        if len < 2 {
            return Err("invalid");
        }
        let is_sof = matches!(marker, 0xC0..=0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF);
        let Some(segment) = data.get(i..i + len) else {
            return Err(end(true));
        };
        if is_sof {
            if len < 8 {
                return Err("invalid");
            }
            let height = u16::from_be_bytes([segment[3], segment[4]]);
            let width = u16::from_be_bytes([segment[5], segment[6]]);
            let nf = segment[7];
            if len != 8 + 3 * usize::from(nf) || width == 0 || height == 0 || nf == 0 {
                return Err("invalid");
            }
            return Ok(Dims {
                width: u64::from(width),
                height: u64::from(height),
                components: u64::from(nf),
            });
        }
        i += len;
    }
}

/// Dimensioni del marker SIZ di un JPEG 2000: codestream nudo (SOC) o file JP2, dove il
/// codestream sta nel primo box `jp2c` di primo livello, come in OpenJPEG.
///
/// I box si scorrono con LBox (u32) e TBox; LBox = 1 vuole XLBox (u64) ≥ 16, LBox = 0 è ammesso
/// solo sul `jp2c`, LBox fra 2 e 7 è `invalid`, la fine del box si somma con `checked_add`.
/// Nel codestream il SIZ deve seguire subito SOC, con Lsiz ≥ 41 e Lsiz = 38 + 3 × Csiz; le
/// dimensioni sono Xsiz − XOsiz e Ysiz − YOsiz, le componenti Csiz. Dopo il SIZ la lettura si
/// ferma. Dati finiti prima di poter decidere: `beyond_scan` con `more`, altrimenti `truncated`.
fn read_jpx_siz(data: &[u8], more: bool) -> Result<Dims, &'static str> {
    const SOC: [u8; 2] = [0xFF, 0x4F];
    const JP2_SIGNATURE: [u8; 12] = [
        0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A,
    ];
    let short = |more: bool| if more { "beyond_scan" } else { "truncated" };
    let be32 = |b: &[u8]| u32::from_be_bytes([b[0], b[1], b[2], b[3]]);

    let (codestream, cs_more): (&[u8], bool) = if data.starts_with(&SOC) {
        (data, more)
    } else if data.starts_with(&JP2_SIGNATURE) {
        let mut pos = JP2_SIGNATURE.len();
        loop {
            let Some(head) = data.get(pos..pos + 8) else {
                return Err(short(more));
            };
            let lbox = be32(&head[..4]);
            let is_jp2c = &head[4..8] == b"jp2c";
            let (header, length) = match lbox {
                0 if is_jp2c => (8, None),
                0 => return Err("missing"),
                1 => {
                    let Some(xl) = data.get(pos + 8..pos + 16) else {
                        return Err(short(more));
                    };
                    let xlbox = u64::from_be_bytes([
                        xl[0], xl[1], xl[2], xl[3], xl[4], xl[5], xl[6], xl[7],
                    ]);
                    if xlbox < 16 {
                        return Err("invalid");
                    }
                    (16, Some(xlbox))
                }
                2..=7 => return Err("invalid"),
                n => (8, Some(u64::from(n))),
            };
            let end = match length {
                Some(l) => Some((pos as u64).checked_add(l).ok_or("invalid")?),
                None => None,
            };
            if is_jp2c {
                let start = pos + header;
                // Box chiuso entro i dati: il codestream finisce con il box.
                break match end {
                    Some(e) if e <= data.len() as u64 => {
                        (data.get(start..e as usize).unwrap_or(&[]), false)
                    }
                    _ => (data.get(start..).unwrap_or(&[]), more),
                };
            }
            match end {
                Some(e) if e <= data.len() as u64 => pos = e as usize,
                _ => return Err(short(more)),
            }
        }
    } else if (data.len() < SOC.len() && SOC.starts_with(data))
        || (data.len() < JP2_SIGNATURE.len() && JP2_SIGNATURE.starts_with(data))
    {
        return Err(short(more));
    } else {
        return Err("missing");
    };

    let Some(soc) = codestream.get(..2) else {
        return Err(short(cs_more));
    };
    if soc != SOC {
        return Err("invalid");
    }
    let Some(marker) = codestream.get(2..4) else {
        return Err(short(cs_more));
    };
    if marker != [0xFF, 0x51] {
        return Err("invalid");
    }
    let Some(lsiz) = codestream.get(4..6) else {
        return Err(short(cs_more));
    };
    let lsiz = usize::from(u16::from_be_bytes([lsiz[0], lsiz[1]]));
    if lsiz < 41 {
        return Err("invalid");
    }
    // Segmento SIZ dal marker: FF 51, Lsiz, Rsiz, Xsiz, Ysiz, XOsiz, YOsiz, tile, Csiz, ...
    let Some(siz) = codestream.get(2..4 + lsiz) else {
        return Err(short(cs_more));
    };
    let csiz = u16::from_be_bytes([siz[38], siz[39]]);
    if lsiz != 38 + 3 * usize::from(csiz) || csiz == 0 {
        return Err("invalid");
    }
    let (xsiz, ysiz) = (be32(&siz[6..10]), be32(&siz[10..14]));
    let (xo, yo) = (be32(&siz[14..18]), be32(&siz[18..22]));
    if xo >= xsiz || yo >= ysiz {
        return Err("invalid");
    }
    Ok(Dims {
        width: u64::from(xsiz - xo),
        height: u64::from(ysiz - yo),
        components: u64::from(csiz),
    })
}

/// Tetto delle immagini DCT e JPX sulle dimensioni codificate nei dati (gap-closure 1.5).
///
/// Motivo. Il tetto `max_image_bytes` di `check_chain` guarda `/Width × /Height ×
/// componenti` dichiarati, mentre i decoder di PDFium leggono le dimensioni dentro i dati: il
/// decoder DCT dal marker SOF del JPEG, il decoder JPX dal marker SIZ del codestream. Un PDF
/// che dichiara 1 × 1 e porta un JPEG da 30000 × 30000 passava il prescan (RISK-16 di
/// 01.4-02, TEST-14 di 01-13, D-39 di `01-CONTEXT.md`; D-09 per i JPX). Qui si leggono SOF o
/// SIZ entro `IMAGE_HEADER_SCAN_BYTES`, dopo i soli filtri che precedono il filtro immagine
/// (D-01, D-02, D-10), e lo stream si respinge con detail `dct_sof <sottomotivo>` o
/// `jpx_siz <sottomotivo>`: `over_cap` (`DecompressionLimit`) se larghezza × altezza ×
/// componenti supera il tetto; `larger_than_declared` se un asse supera il dichiarato;
/// `missing`, `truncated`, `beyond_scan`, `invalid` o `prefilter` per un marker che non si
/// legge (tutti `Unsupported`). Più piccolo del dichiarato è ammesso.
///
/// La lettura del JPEG è per segmenti e non una ricerca di byte `FF Cx`, perché un SOF finto
/// (la miniatura EXIF in APP1) non deve nascondere quello vero. `check_stream` la chiama dopo
/// i controlli `per_stream` e di rapporto, così una bomba Flate resta `DecompressionLimit`.
/// Le immagini inline (`BI … ID … EI`) stanno negli stream di contenuto e restano fuori: sono
/// debito del prescan di prodotto della fase 3 e di ADR-0004.
fn check_image_header(
    chain: &[Vec<u8>],
    stream: &lopdf::Stream,
    caps: &Caps,
    meter: &mut Meter,
) -> Result<(), Rejection> {
    let codec = match chain.last().map(Vec::as_slice) {
        Some(b"DCTDecode") => Codec::Dct,
        Some(b"JPXDecode") => Codec::Jpx,
        _ => return Ok(()),
    };
    let (data, more) = match image_prefix(chain, &stream.content, meter) {
        Ok(p) => p,
        Err(PrefixError::Work(r)) => return Err(r),
        Err(PrefixError::Prefilter(sub)) => {
            return Err(fail(Kind::Unsupported, format!("{} {sub}", codec.detail())));
        }
    };
    let read = match codec {
        Codec::Dct => read_sof(&data, more),
        Codec::Jpx => read_jpx_siz(&data, more),
    };
    let Dims {
        width: w,
        height: h,
        components: n,
    } = read.map_err(|sub| fail(Kind::Unsupported, format!("{} {sub}", codec.detail())))?;
    let cap = caps.max_image_bytes;
    match w.checked_mul(h).and_then(|v| v.checked_mul(n)) {
        Some(b) if b <= cap => {}
        bytes => {
            let b = bytes.map_or_else(|| "overflow".to_owned(), |b| b.to_string());
            return Err(fail(
                Kind::DecompressionLimit,
                format!(
                    "{} over_cap dims={w}x{h}x{n} bytes={b} cap={cap}",
                    codec.detail()
                ),
            ));
        }
    }
    let (Some(dw), Some(dh)) = (int(&stream.dict, b"Width"), int(&stream.dict, b"Height")) else {
        return Err(fail(
            Kind::Unsupported,
            format!("{} invalid", codec.detail()),
        ));
    };
    if w > dw || h > dh {
        return Err(fail(
            Kind::Unsupported,
            format!(
                "{} larger_than_declared dims={w}x{h} declared={dw}x{dh}",
                codec.detail()
            ),
        ));
    }
    Ok(())
}

/// Massimi per stream registrati dalla calibrazione.
#[derive(Debug, Default)]
struct StreamStats {
    max_flate_ratio: f64,
    max_stream_bytes: u64,
}

/// Filtri, decodifica in streaming e tetti di uno stream; ritorna i byte espansi (0 se lo
/// stream non ha Flate).
fn check_stream(
    stream: &lopdf::Stream,
    caps: &Caps,
    meter: &mut Meter,
    stats: &mut StreamStats,
) -> Result<u64, Rejection> {
    let chain = filters(&stream.dict)?;
    check_chain(&chain, &stream.dict, caps)?;
    let Some(n) = flate_expansion(&chain, &stream.content, caps.per_stream_bytes, meter)? else {
        return check_image_header(&chain, stream, caps, meter).map(|()| 0);
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
    stats.max_stream_bytes = stats.max_stream_bytes.max(n);
    if ratio > stats.max_flate_ratio {
        stats.max_flate_ratio = ratio;
    }
    check_image_header(&chain, stream, caps, meter)?;
    Ok(n)
}

// ---- contenuti attivi e allegati -------------------------------------------------------

/// Motivi di quarantena trovati negli oggetti caricati (object stream compresi).
#[derive(Debug, Default)]
struct Active {
    javascript: bool,
    launch: bool,
    open_action_js: bool,
    external_link: bool,
}

impl Active {
    fn reasons(&self) -> Vec<String> {
        let flags = [
            self.javascript,
            self.launch,
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

/// Riferimenti di un dizionario `/EF` di un file specification (punto 3-ter).
fn collect_ef(o: &Object, ids: &mut BTreeSet<ObjectId>, depth: u32) {
    if depth > 64 {
        return;
    }
    let d = match o {
        Object::Dictionary(d) => d,
        Object::Stream(s) => &s.dict,
        Object::Array(v) => {
            v.iter().for_each(|x| collect_ef(x, ids, depth + 1));
            return;
        }
        _ => return,
    };
    for (k, v) in d.iter() {
        if k.as_slice() == b"EF"
            && let Object::Dictionary(ef) = v
        {
            for (_, r) in ef.iter() {
                if let Object::Reference(id) = r {
                    ids.insert(*id);
                }
            }
        }
        collect_ef(v, ids, depth + 1);
    }
}

/// Allegati incorporati (punto 3-ter): stream `/Type /EmbeddedFile` e stream citati da un
/// `/EF`, contati una volta. Si contano soltanto: non si estraggono né si passano ad altri
/// parser; i loro stream sono già passati da filtri e tetti come ogni altro stream.
fn count_embedded_files(doc: &Document) -> u32 {
    let mut ids = BTreeSet::new();
    for (id, o) in &doc.objects {
        if let Object::Stream(s) = o
            && s.dict.has_type(b"EmbeddedFile")
        {
            ids.insert(*id);
        }
        collect_ef(o, &mut ids, 0);
    }
    u32::try_from(ids.len()).unwrap_or(u32::MAX)
}

/// Pagine dall'albero con limite (→ `ResourceLimit`); nessuna pagina → `Corrupt`.
fn page_count(doc: &Document, caps: &Caps) -> Result<u32, Rejection> {
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
    Ok(u32::try_from(pages).unwrap_or(u32::MAX))
}

/// Contenuti attivi del documento caricato (object stream espansi) e della scansione a byte.
fn quarantine(doc: &Document, scan: &ByteScan) -> Result<(), Rejection> {
    let mut active = Active {
        javascript: scan.javascript,
        launch: scan.launch,
        ..Active::default()
    };
    for obj in doc.objects.values() {
        walk(obj, &mut active, 0);
    }
    if let Ok(cat) = doc.catalog()
        && let Ok(oa) = cat.get(b"OpenAction")
        && is_js_action(doc, oa)
    {
        active.open_action_js = true;
    }
    let reasons = active.reasons();
    if !reasons.is_empty() {
        return Err(Rejection::Quarantined(reasons));
    }
    Ok(())
}

// ---- PDF -------------------------------------------------------------------------------

/// Prescan di un PDF tenuto in memoria. Va chiamato nel figlio, prima di PDFium.
pub fn check_pdf(bytes: &[u8], caps: &Caps) -> Result<PrescanReport, Rejection> {
    let scan = scan_bytes(bytes);
    if scan.encrypt {
        check_encrypted(bytes, caps, &scan)
    } else {
        check_plain(bytes, caps, &scan)
    }
}

/// Lavoro di una passata di caricamento sui cross-reference stream (limite analitico).
fn xref_pass(caps: &Caps, scan: &ByteScan) -> u64 {
    u64::from(scan.xref).saturating_mul(caps.per_stream_bytes)
}

/// Ramo non cifrato (punto 2): prima passata con gli object stream compressi, I1 su ogni
/// stream, poi il caricamento vero solo se I1 regge.
fn check_plain(bytes: &[u8], caps: &Caps, scan: &ByteScan) -> Result<PrescanReport, Rejection> {
    let mut meter = Meter::new(caps, scan.xref);
    let mut stats = StreamStats::default();
    let mut objstm_bytes = 0u64;
    {
        let first = load_plain(bytes, caps, Some(defer_objstm))?;
        if first.was_encrypted() || first.is_encrypted() {
            return Err(fail(Kind::Corrupt, "encrypt_state not_declared"));
        }
        meter.add_work(xref_pass(caps, scan))?;
        for obj in first.objects.values() {
            let Object::Stream(s) = obj else {
                continue;
            };
            let n = check_stream(s, caps, &mut meter, &mut stats)?;
            if s.dict.has_type(DEFERRED_OBJSTM) {
                objstm_bytes = objstm_bytes.saturating_add(n);
            }
        }
    }

    // Caricamento vero: riespande gli stessi object stream già misurati.
    let doc = load_plain(bytes, caps, None)?;
    if doc.was_encrypted() || doc.is_encrypted() {
        return Err(fail(Kind::Corrupt, "encrypt_state not_declared"));
    }
    meter.add_work(objstm_bytes)?;
    meter.add_work(xref_pass(caps, scan))?;

    let pages = page_count(&doc, caps)?;
    let report = PrescanReport {
        pages: Some(pages),
        objstm_count: scan.objstm,
        max_flate_ratio: stats.max_flate_ratio,
        max_stream_bytes: stats.max_stream_bytes,
        total_decompressed_bytes: meter.counted,
        entries: 0,
        encryption: Encryption::None,
        embedded_files: count_embedded_files(&doc),
        objstm_decompressed_bytes: objstm_bytes,
        xref_streams: scan.xref,
        decompress_work_bytes: meter.work,
    };
    quarantine(&doc, scan)?;
    Ok(report)
}

/// Ramo cifrato (punto 3-bis): ammesso solo se lopdf l'apre con la password vuota.
fn check_encrypted(bytes: &[u8], caps: &Caps, scan: &ByteScan) -> Result<PrescanReport, Rejection> {
    let quota = encrypted_quota(caps, scan.objstm);
    let opts = LoadOptions::with_max_decompressed_size(as_usize(quota));
    let doc = match Document::load_mem_with_options(bytes, opts) {
        Ok(d) => d,
        Err(e) if is_memory_limit(&e) => {
            return Err(fail(
                Kind::DecompressionLimit,
                format!("load_per_stream: {e}"),
            ));
        }
        Err(e) if is_password_error(&e) => {
            return Err(fail(
                Kind::Encrypted,
                format!("password_required load: {e}"),
            ));
        }
        Err(e) => return Err(fail(Kind::Corrupt, format!("load: {e}"))),
    };
    // Ordine fisso: password, stato della cifratura, riconciliazione, contenuto.
    if doc.is_encrypted() {
        return Err(fail(Kind::Encrypted, "password_required"));
    }
    if !doc.was_encrypted() {
        return Err(fail(Kind::Corrupt, "encrypt_state not_loaded"));
    }
    reconcile(&doc)?;

    let mut meter = Meter::new(caps, scan.xref);
    let containers = doc
        .objects
        .values()
        .filter(|o| matches!(o, Object::Stream(s) if s.dict.has_type(b"ObjStm")))
        .count();
    let expanded = u64::from(scan.objstm).max(containers as u64);
    meter.add_work(expanded.saturating_mul(quota))?;
    meter.add_work(xref_pass(caps, scan))?;

    let mut stats = StreamStats::default();
    let mut objstm_bytes = 0u64;
    for obj in doc.objects.values() {
        let Object::Stream(s) = obj else {
            continue;
        };
        let n = check_stream(s, caps, &mut meter, &mut stats)?;
        if s.dict.has_type(b"ObjStm") {
            objstm_bytes = objstm_bytes.saturating_add(n);
        }
    }

    let pages = page_count(&doc, caps)?;
    let report = PrescanReport {
        pages: Some(pages),
        objstm_count: scan.objstm,
        max_flate_ratio: stats.max_flate_ratio,
        max_stream_bytes: stats.max_stream_bytes,
        total_decompressed_bytes: meter.counted,
        entries: 0,
        encryption: Encryption::EmptyPassword,
        embedded_files: count_embedded_files(&doc),
        objstm_decompressed_bytes: objstm_bytes,
        xref_streams: scan.xref,
        decompress_work_bytes: meter.work,
    };
    quarantine(&doc, scan)?;
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
                        // Storia (gap-closure 1.5, D-08): prima `b"\xFF\xD8\xFF\xE0junk"`, JPEG senza SOF; con il controllo sul SOF un dato senza SOF è respinto, quindi la fixture benigna porta un SOF uguale alle dimensioni dichiarate.
                        &jpeg_sof(w, h, 3),
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

    // ---- gap-closure 1.4: forma T senza /Subtype (piano 01.4-02) ----------------------
    //
    // Modifica: Claude (agente test-write 01.4-02) · 2026-10-23 · test rossi della forma T,
    // in coda al modulo senza toccare righe esistenti.
    //
    // La diagnosi (spikes/results/prescan-image-filter-benigno.txt, sezione «Diagnosi») ha
    // trovato su OTC_NSANY_2004 uno stream miniatura riferito da `/Thumb` della pagina, con
    // dizionario `{BitsPerComponent 8, ColorSpace <N> 0 R, Filter [/ASCII85Decode
    // /DCTDecode], Height 105, Length, Width 74}`: niente `/Subtype` né `/Type`, `/ColorSpace`
    // indiretto verso il nome `/DeviceRGB`. Le fixture qui sotto riproducono quel dizionario
    // tale e quale (riferimento indiretto compreso), con `/Width 8 /Height 8`. Senza
    // FlateDecode lo stream non viene decodificato, quindi i dati non serve che siano ASCII85.
    // Storia (gap-closure 1.5, D-08): la frase precedente non vale più. Il controllo sul SOF
    // applica ASCII85 al prefisso dei dati anche senza FlateDecode, quindi `T_DATA` è ora
    // l'ASCII85 valido di un JPEG con SOF 8 × 8.

    /// Chiavi e catena del dizionario osservato, senza dimensioni (`/ColorSpace` indiretto
    /// verso l'oggetto 6, che vale `/DeviceRGB`, come nel file reale).
    const T_OBSERVED: &str =
        "/BitsPerComponent 8 /ColorSpace 6 0 R /Filter [/ASCII85Decode /DCTDecode]";
    /// Dati della fixture: inizio di un JPEG, mai decodificato dal prescan.
    // Storia (gap-closure 1.5, D-08): il commento qui sopra non è più esatto. Dalla 1.5 il
    // prescan rilegge l'intestazione (ASCII85, poi il SOF) senza decodificare l'immagine, e la
    // fixture è l'ASCII85 di `jpeg_sof(8, 8, 3)` (`fixture_t_data_is_ascii85_of_sof_8x8`).
    // Storia (gap-closure 1.5, D-08): prima `b"\xFF\xD8\xFF\xE0junk"`, JPEG senza SOF e nemmeno ASCII85 valido; con il controllo sul SOF un dato senza SOF è respinto, quindi la fixture benigna porta un SOF uguale alle dimensioni dichiarate.
    const T_DATA: &[u8] = br#"s4IA0!"_al8O`[\!<<*#!!*'"s1eUH#QP,1#Qk)>!!3`4!tbP5f`~>"#;

    /// Una pagina con `/<referrer> 5 0 R` nel proprio dizionario e lo stream 5 con il
    /// dizionario `dict_extra` (`/Length` lo aggiunge `stream`). L'oggetto 6 è il nome
    /// `/DeviceRGB` del `/ColorSpace` indiretto, l'oggetto 7 l'intero 8 (dimensioni
    /// indirette), l'oggetto 8 il nome `/Image` (`/Subtype` indiretto). Il referente è messo
    /// nella pagina anche se la regola non lo guarda: la forma T si riconosce dal solo
    /// dizionario dello stream.
    fn subtypeless_doc(referrer: &str, dict_extra: &str, data: &[u8]) -> Vec<u8> {
        build(
            &[
                (1, dict("<< /Type /Catalog /Pages 2 0 R >>")),
                (2, dict("<< /Type /Pages /Kids [3 0 R] /Count 1 >>")),
                (
                    3,
                    dict(&format!(
                        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] \
                         /Contents 4 0 R /{referrer} 5 0 R >>"
                    )),
                ),
                (4, stream("", b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET")),
                (5, stream(dict_extra, data)),
                (6, dict("/DeviceRGB")),
                (7, dict("8")),
                (8, dict("/Image")),
            ],
            "",
        )
    }

    /// La forma T osservata con dimensioni scelte dal test.
    fn thumb_dict(w: &str, h: &str) -> String {
        format!("{T_OBSERVED} /Width {w} /Height {h}")
    }

    /// `Failed(Unsupported, "image_filter_on_non_image")`, con `what` nel messaggio.
    fn assert_non_image_detail(pdf: &[u8], what: &str) {
        let (k, d) = failed_of(check_pdf(pdf, &caps()));
        assert_eq!(
            (k, d.split_whitespace().next()),
            (Kind::Unsupported, Some("image_filter_on_non_image")),
            "{what}: detail {d:?}"
        );
    }

    #[test]
    fn image_filter_on_subtypeless_image_dict_passes() {
        let pdf = subtypeless_doc("Thumb", &thumb_dict("8", "8"), T_DATA);
        assert_ok(check_pdf(&pdf, &caps()));
    }

    #[test]
    fn image_filter_on_subtypeless_image_dict_over_cap_is_rejected() {
        // 10.000 × 10.000 × 3 byte ≈ 286 MiB, oltre i 64 MiB di max_image_bytes.
        let d = "/BitsPerComponent 8 /ColorSpace /DeviceRGB \
                 /Filter [/ASCII85Decode /DCTDecode] /Width 10000 /Height 10000";
        let pdf = subtypeless_doc("Thumb", d, T_DATA);
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "image_declared",
        );
    }

    #[test]
    fn subtypeless_dict_with_foreign_key_keeps_image_filter_rejection() {
        let d = format!("{} /Resources << >>", thumb_dict("8", "8"));
        assert_non_image_detail(&subtypeless_doc("Thumb", &d, T_DATA), "/Resources");
    }

    #[test]
    fn subtypeless_dict_with_image_filter_not_last_is_rejected() {
        let d = "/BitsPerComponent 8 /ColorSpace 6 0 R /Filter [/DCTDecode /FlateDecode] \
                 /Width 8 /Height 8";
        assert_non_image_detail(
            &subtypeless_doc("Thumb", d, &zlib(T_DATA)),
            "[/DCTDecode /FlateDecode]",
        );
    }

    #[test]
    fn subtypeless_dict_without_size_keeps_image_filter_rejection() {
        let d = format!("{T_OBSERVED} /Height 8");
        assert_non_image_detail(&subtypeless_doc("Thumb", &d, T_DATA), "senza /Width");
    }

    #[test]
    fn subtypeless_dict_with_unobserved_image_filter_is_rejected() {
        let d = "/BitsPerComponent 8 /ColorSpace 6 0 R /Filter [/ASCII85Decode /JBIG2Decode] \
                 /Width 8 /Height 8";
        assert_non_image_detail(&subtypeless_doc("Thumb", d, T_DATA), "/JBIG2Decode");
    }

    #[test]
    fn content_stream_with_image_filter_keeps_detail() {
        // Stesso input di image_filter_on_non_image_stream_is_rejected (01-07), ma sul detail.
        let pdf = doc_with_content("/Filter /DCTDecode", b"\xFF\xD8\xFF\xE0junk");
        assert_non_image_detail(&pdf, "contenuto di pagina");
    }

    // Confini aggiuntivi del test design (TEST-7): verdi sulla base e dopo la correzione.

    #[test]
    fn subtypeless_dict_with_type_other_than_xobject_is_rejected() {
        for t in ["/Page", "/ObjStm"] {
            let d = format!("/Type {t} {}", thumb_dict("8", "8"));
            assert_non_image_detail(&subtypeless_doc("Thumb", &d, T_DATA), &format!("/Type {t}"));
        }
    }

    #[test]
    fn subtypeless_dict_with_subtype_not_image_or_indirect_is_rejected() {
        // La forma T vuole /Subtype assente: /Form e un riferimento indiretto a /Image no.
        for s in ["/Form", "8 0 R"] {
            let d = format!("/Subtype {s} {}", thumb_dict("8", "8"));
            assert_non_image_detail(
                &subtypeless_doc("Thumb", &d, T_DATA),
                &format!("/Subtype {s}"),
            );
        }
    }

    #[test]
    fn subtypeless_dict_with_non_positive_or_indirect_size_is_rejected() {
        // Dimensioni ammesse solo come interi positivi diretti.
        for w in ["0", "-8", "8.0", "7 0 R"] {
            assert_non_image_detail(
                &subtypeless_doc("Thumb", &thumb_dict(w, "8"), T_DATA),
                &format!("/Width {w}"),
            );
        }
    }

    #[test]
    fn subtypeless_dict_with_ccitt_filter_is_rejected() {
        let d = "/BitsPerComponent 8 /ColorSpace 6 0 R \
                 /Filter [/ASCII85Decode /CCITTFaxDecode] /Width 8 /Height 8";
        assert_non_image_detail(&subtypeless_doc("Thumb", d, T_DATA), "/CCITTFaxDecode");
    }

    #[test]
    fn subtypeless_dict_with_overflowing_size_is_rejected() {
        // (2^32 - 1)^2 × 4 componenti × 2 byte va in overflow in checked_mul: mai Ok.
        let d = "/BitsPerComponent 16 /ColorSpace /DeviceCMYK \
                 /Filter [/ASCII85Decode /DCTDecode] /Width 4294967295 /Height 4294967295";
        let pdf = subtypeless_doc("Thumb", d, T_DATA);
        let (k, det) = failed_of(check_pdf(&pdf, &caps()));
        assert!(
            matches!(k, Kind::Unsupported | Kind::DecompressionLimit),
            "kind {k:?} detail {det:?}"
        );
    }

    #[test]
    #[ignore = "richiede spikes/data/s02/raw (fetch.sh di 01-08)"]
    fn nsany_pages_pass_prescan() {
        let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/s02"));
        let manifest = std::fs::read(root.join("manifest.json")).expect("manifest.json di s02");
        let entries: serde_json::Value =
            serde_json::from_slice(&manifest).expect("manifest.json non è JSON");
        let pages: Vec<(String, String)> = entries
            .as_array()
            .expect("manifest.json non è un array")
            .iter()
            .filter(|e| e["doc_id"] == "otc-nsany-2004" && e["role"] == "pdf")
            .map(|e| {
                (
                    e["item_id"].as_str().unwrap_or("?").to_owned(),
                    e["file"].as_str().expect("voce senza file").to_owned(),
                )
            })
            .collect();
        assert_eq!(
            pages.len(),
            4,
            "attese 4 pagine otc-nsany-2004 nel manifest"
        );
        let mut outcomes = Vec::new();
        let mut failed = 0;
        for (item, file) in &pages {
            let path = root.join("raw").join(file);
            let bytes = std::fs::read(&path).unwrap_or_else(|e| {
                panic!("{item}: file assente ({e}); lanciare spikes/data/s02/fetch.sh")
            });
            match check_pdf(&bytes, &Caps::default()) {
                Ok(_) => outcomes.push(format!("{item}: Ok")),
                Err(e) => {
                    failed += 1;
                    outcomes.push(format!("{item}: {e:?}"));
                }
            }
        }
        assert!(
            failed == 0,
            "{failed} su 4 pagine respinte dal prescan:\n{}",
            outcomes.join("\n")
        );
    }

    // ---- gap-closure 1.5: SOF dei DCT e SIZ dei JPX (piano 01.5-01) ----
    //
    // Modifica: Claude (agente test-write 01.5-01) · 2026-10-23 · test di SOF e SIZ con i
    // bypass del test design, in coda al modulo; fuori da qui cambiano solo le due fixture
    // D-08 (`image_doc` e `T_DATA`), con il loro commento Storia.
    //
    // I test usano solo API che esistono già sulla base (`check_pdf`, il report, gli helper):
    // il modulo compila prima della correzione. Un rosso sulla base vale solo se osserva `Ok`
    // (o un delta di lavoro diverso), mai per una costruzione fallita.

    /// Finestra di lettura dell'intestazione: `IMAGE_HEADER_SCAN_BYTES` di `<interfaces>` del
    /// piano 01.5-01, cioè 2 + 4 × 65.537 + 2 + 8 + 3 × 255 = 262.925 byte (SOI, 4 segmenti APP
    /// pieni, intestazione di un SOF con 255 componenti). Letterale locale, perché la costante
    /// non esiste sulla base.
    const WINDOW: usize = 262_925;
    /// Segmento marker JPEG di lunghezza massima, marker compreso (2 + 65.535).
    const FULL_SEGMENT: usize = 2 + 65_535;
    /// Dimensione enorme dei casi `over_cap`: 30000 × 30000 × 3 byte, oltre i 64 MiB.
    const HUGE: u32 = 30_000;

    /// `Failed(kind, detail)` con primo token `prefix` e secondo token `sub`.
    fn assert_coded(
        r: Result<PrescanReport, Rejection>,
        kind: Kind,
        prefix: &str,
        sub: &str,
    ) -> String {
        let (k, d) = failed_of(r);
        let mut t = d.split_whitespace();
        assert_eq!(
            (k, t.next(), t.next()),
            (kind, Some(prefix), Some(sub)),
            "detail {d:?}"
        );
        d
    }

    /// SOI, poi i segmenti `(marker, payload)` con la loro lunghezza, poi EOI.
    fn jpeg_segments(segs: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut out = vec![0xFF, 0xD8];
        for (m, p) in segs {
            out.extend_from_slice(&[0xFF, *m]);
            out.extend_from_slice(
                &u16::try_from(p.len() + 2)
                    .expect("segmento troppo lungo")
                    .to_be_bytes(),
            );
            out.extend_from_slice(p);
        }
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    /// Payload di un SOF: P = 8, Y, X, Nf e Nf componenti `(id, 0x11, 0)`.
    fn sof_payload(w: u16, h: u16, nf: u8) -> Vec<u8> {
        let mut p = vec![8];
        p.extend_from_slice(&h.to_be_bytes());
        p.extend_from_slice(&w.to_be_bytes());
        p.push(nf);
        for c in 0..nf {
            p.extend_from_slice(&[c + 1, 0x11, 0]);
        }
        p
    }

    fn sof_segment(marker: u8, w: u16, h: u16, nf: u8) -> (u8, Vec<u8>) {
        (marker, sof_payload(w, h, nf))
    }

    /// APP0 JFIF da 16 byte (marker escluso): `JFIF\0`, versione 1.1, densità 1 × 1.
    fn app0_jfif() -> (u8, Vec<u8>) {
        (
            0xE0,
            b"JFIF\x00\x01\x01\x00\x00\x01\x00\x01\x00\x00".to_vec(),
        )
    }

    fn u16_of(v: u32) -> u16 {
        u16::try_from(v).expect("dimensione JPEG oltre 65.535")
    }

    /// JPEG minimo: SOI, APP0 JFIF, SOF0 (`w × h × nf`), EOI.
    fn jpeg_sof(w: u32, h: u32, nf: u8) -> Vec<u8> {
        jpeg_segments(&[app0_jfif(), sof_segment(0xC0, u16_of(w), u16_of(h), nf)])
    }

    /// Segmenti di riempimento (APP1 pieni, poi un COM) per `total` byte esatti, marker compresi.
    fn filler(total: usize) -> Vec<(u8, Vec<u8>)> {
        let mut segs = Vec::new();
        let mut left = total;
        while left >= FULL_SEGMENT + 4 || left == FULL_SEGMENT {
            segs.push((0xE1, vec![0x41; FULL_SEGMENT - 4]));
            left -= FULL_SEGMENT;
        }
        if left > 0 {
            assert!(
                left >= 4,
                "riempimento di {left} byte: un segmento ne vuole almeno 4"
            );
            segs.push((0xFE, vec![0x43; left - 4]));
        }
        segs
    }

    /// Codifica ASCII85 come `base64.a85encode` di Python (`z` per i gruppi nulli), con `~>`.
    fn a85(data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for chunk in data.chunks(4) {
            let mut word = [0u8; 4];
            word[..chunk.len()].copy_from_slice(chunk);
            let mut v = u32::from_be_bytes(word);
            if chunk.len() == 4 && v == 0 {
                out.push(b'z');
                continue;
            }
            let mut digits = [0u8; 5];
            for d in digits.iter_mut().rev() {
                *d = (v % 85) as u8 + b'!';
                v /= 85;
            }
            out.extend_from_slice(&digits[..chunk.len() + 1]);
        }
        out.extend_from_slice(b"~>");
        out
    }

    /// Una pagina che disegna `/Im1` = oggetto 5, con il dizionario `dict5` scelto dal test.
    fn page_with_stream5(dict5: &str, data: &[u8]) -> Vec<u8> {
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
                (5, stream(dict5, data)),
            ],
            "",
        )
    }

    /// Come `image_doc`, ma con dizionario e dati scelti dal test (`/Subtype /Image`).
    fn dct_image_doc(dict_extra: &str, data: &[u8]) -> Vec<u8> {
        page_with_stream5(
            &format!("/Type /XObject /Subtype /Image {dict_extra}"),
            data,
        )
    }

    /// Dizionario d'immagine 8 bit con dimensioni, spazio colore e filtro.
    fn img(w: u32, h: u32, cs: &str, filter: &str) -> String {
        format!("/Width {w} /Height {h} /ColorSpace /{cs} /BitsPerComponent 8 /Filter {filter}")
    }

    /// Esito del prescan su un `/Image` DCT con i dati dati.
    fn dct_check(w: u32, h: u32, cs: &str, data: &[u8]) -> Result<PrescanReport, Rejection> {
        check_pdf(&dct_image_doc(&img(w, h, cs, "/DCTDecode"), data), &caps())
    }

    /// Payload del segmento SIZ (dopo il marker e Lsiz): Rsiz, dimensioni, origini, un solo
    /// tile, Csiz e componenti a 8 bit senza segno.
    fn siz_payload(xsiz: u32, ysiz: u32, xo: u32, yo: u32, comps: u16) -> Vec<u8> {
        let mut p = 0u16.to_be_bytes().to_vec();
        for v in [xsiz, ysiz, xo, yo, xsiz, ysiz, 0, 0] {
            p.extend_from_slice(&v.to_be_bytes());
        }
        p.extend_from_slice(&comps.to_be_bytes());
        for _ in 0..comps {
            p.extend_from_slice(&[7, 1, 1]);
        }
        p
    }

    /// Marker e segmento: `FF mm`, lunghezza (payload + 2), payload.
    fn marker_segment(m: u8, payload: &[u8]) -> Vec<u8> {
        let mut out = vec![0xFF, m];
        out.extend_from_slice(
            &u16::try_from(payload.len() + 2)
                .expect("segmento lungo")
                .to_be_bytes(),
        );
        out.extend_from_slice(payload);
        out
    }

    /// COD e QCD minimi (5-3 reversibile, nessun livello di decomposizione), poi EOC.
    fn jpx_tail() -> Vec<u8> {
        let mut out = marker_segment(0x52, &[0, 0, 0, 1, 0, 0, 4, 4, 0, 1]);
        out.extend_from_slice(&marker_segment(0x5C, &[0x40, 0x40]));
        out.extend_from_slice(&[0xFF, 0xD9]);
        out
    }

    /// Codestream con SIZ dato (origini comprese): SOC, SIZ, COD, QCD, EOC.
    fn jpx_codestream_at(xsiz: u32, ysiz: u32, xo: u32, yo: u32, comps: u16) -> Vec<u8> {
        let mut out = vec![0xFF, 0x4F];
        out.extend_from_slice(&marker_segment(
            0x51,
            &siz_payload(xsiz, ysiz, xo, yo, comps),
        ));
        out.extend_from_slice(&jpx_tail());
        out
    }

    /// Codestream nudo minimo: SOC, SIZ (`w × h × comps`, origine 0), COD, QCD, EOC.
    fn jpx_codestream(w: u32, h: u32, comps: u16) -> Vec<u8> {
        jpx_codestream_at(w, h, 0, 0, comps)
    }

    /// Box JP2 con LBox = lunghezza totale.
    fn jp2_box(tbox: &[u8; 4], payload: &[u8]) -> Vec<u8> {
        let mut out = u32::try_from(payload.len() + 8)
            .expect("box lungo")
            .to_be_bytes()
            .to_vec();
        out.extend_from_slice(tbox);
        out.extend_from_slice(payload);
        out
    }

    const JP2_SIGNATURE: &[u8] = b"\x00\x00\x00\x0C\x6A\x50\x20\x20\x0D\x0A\x87\x0A";

    /// File JP2: firma, i box `boxes_before` (con LBox esatto), poi `jp2c` con il codestream.
    fn jp2_file(boxes_before: &[(&[u8; 4], Vec<u8>)], codestream: &[u8]) -> Vec<u8> {
        let mut out = JP2_SIGNATURE.to_vec();
        for (t, p) in boxes_before {
            out.extend_from_slice(&jp2_box(t, p));
        }
        out.extend_from_slice(&jp2_box(b"jp2c", codestream));
        out
    }

    /// `ftyp` (`jp2 `) e `jp2h` con `ihdr` e `colr` enumerato sRGB.
    fn jp2_header_boxes(w: u32, h: u32, comps: u16) -> Vec<(&'static [u8; 4], Vec<u8>)> {
        let mut ihdr = h.to_be_bytes().to_vec();
        ihdr.extend_from_slice(&w.to_be_bytes());
        ihdr.extend_from_slice(&comps.to_be_bytes());
        ihdr.extend_from_slice(&[7, 7, 0, 0]);
        let mut jp2h = jp2_box(b"ihdr", &ihdr);
        jp2h.extend_from_slice(&jp2_box(b"colr", &[1, 0, 0, 0, 0, 0, 16]));
        vec![
            (b"ftyp", b"jp2 \x00\x00\x00\x00jp2 ".to_vec()),
            (b"jp2h", jp2h),
        ]
    }

    /// Esito del prescan su un `/Image` JPX (senza `/ColorSpace`) con i dati e il filtro dati.
    fn jpx_check_with(
        w: u32,
        h: u32,
        filter: &str,
        data: &[u8],
    ) -> Result<PrescanReport, Rejection> {
        let d = format!("/Width {w} /Height {h} /BitsPerComponent 8 /Filter {filter}");
        check_pdf(&dct_image_doc(&d, data), &caps())
    }

    fn jpx_check(w: u32, h: u32, data: &[u8]) -> Result<PrescanReport, Rejection> {
        jpx_check_with(w, h, "/JPXDecode", data)
    }

    /// JPEG benigno lungo `len` byte: SOI, SOF0 `w × h × 3` subito dopo, poi segmenti COM.
    fn long_benign_jpeg(w: u16, h: u16, len: usize) -> Vec<u8> {
        let head = 2 + 19;
        let segs: Vec<(u8, Vec<u8>)> = std::iter::once(sof_segment(0xC0, w, h, 3))
            .chain(filler(len - head - 2).into_iter().map(|(_, p)| (0xFE, p)))
            .collect();
        let j = jpeg_segments(&segs);
        assert_eq!(j.len(), len, "fixture: JPEG di lunghezza diversa");
        j
    }

    // -- DCT: rifiuti di base (rossi sulla base) --

    #[test]
    fn dct_sof_larger_than_declared_is_rejected() {
        let r = dct_check(8, 8, "DeviceRGB", &jpeg_sof(1000, 1000, 3));
        assert_coded(r, Kind::Unsupported, "dct_sof", "larger_than_declared");
    }

    #[test]
    fn dct_sof_over_cap_is_rejected() {
        let r = dct_check(1, 1, "DeviceGray", &jpeg_sof(HUGE, HUGE, 3));
        assert_coded(r, Kind::DecompressionLimit, "dct_sof", "over_cap");
    }

    #[test]
    fn dct_sof_missing_is_rejected() {
        let data = jpeg_segments(&[
            app0_jfif(),
            (0xDB, [vec![0], vec![1; 64]].concat()),
            (0xDA, vec![1, 1, 0x00, 0, 63, 0]),
        ]);
        let r = dct_check(8, 8, "DeviceRGB", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "missing");
    }

    #[test]
    fn dct_data_without_soi_is_rejected() {
        let r = dct_check(8, 8, "DeviceRGB", b"junk");
        assert_coded(r, Kind::Unsupported, "dct_sof", "missing");
    }

    #[test]
    fn dct_sof_truncated_is_rejected() {
        // SOI più i primi 5 byte di un SOF0: la fine cade dentro il segmento.
        let r = dct_check(8, 8, "DeviceRGB", b"\xFF\xD8\xFF\xC0\x00\x11\x08");
        assert_coded(r, Kind::Unsupported, "dct_sof", "truncated");
    }

    #[test]
    fn dct_sof_beyond_scan_is_rejected() {
        // SOI + 5 APP1 da 65.535 = 327.687 byte, oltre la finestra; poi il SOF uguale.
        let mut segs: Vec<(u8, Vec<u8>)> = (0..5).map(|_| (0xE1, vec![0x41; 65_533])).collect();
        segs.push(sof_segment(0xC0, 8, 8, 3));
        let r = dct_check(8, 8, "DeviceRGB", &jpeg_segments(&segs));
        assert_coded(r, Kind::Unsupported, "dct_sof", "beyond_scan");
    }

    #[test]
    fn dct_sof_with_zero_height_is_rejected() {
        let data = jpeg_segments(&[app0_jfif(), sof_segment(0xC0, 8, 0, 3)]);
        let r = dct_check(8, 8, "DeviceRGB", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "invalid");
    }

    #[test]
    fn dct_sof_inside_app_segment_does_not_hide_real_sof() {
        // Un SOF 8 × 8 finto dentro APP1 (lo troverebbe una ricerca di byte FF C0), poi il SOF
        // vero 1000 × 1000: la lettura per segmenti salta APP1.
        let mut fake = b"Exif\x00\x00".to_vec();
        fake.extend_from_slice(&marker_segment(0xC0, &sof_payload(8, 8, 3)));
        let data = jpeg_segments(&[(0xE1, fake), sof_segment(0xC0, 1000, 1000, 3)]);
        let r = dct_check(8, 8, "DeviceRGB", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "larger_than_declared");
    }

    #[test]
    fn dct_sof_other_sof_markers_are_read() {
        for m in [0xC2u8, 0xC3, 0xC9, 0xCF] {
            let data = jpeg_segments(&[app0_jfif(), sof_segment(m, 30_000, 30_000, 3)]);
            let (k, d) = failed_of(dct_check(1, 1, "DeviceGray", &data));
            let mut t = d.split_whitespace();
            assert_eq!(
                (k, t.next(), t.next()),
                (Kind::DecompressionLimit, Some("dct_sof"), Some("over_cap")),
                "SOF {m:02X}: detail {d:?}"
            );
        }
    }

    // -- DCT: percorsi (rossi sulla base) --

    #[test]
    fn dct_sof_after_ascii85_is_checked() {
        let pdf = subtypeless_doc(
            "Thumb",
            &thumb_dict("8", "8"),
            &a85(&jpeg_sof(1000, 1000, 3)),
        );
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::Unsupported,
            "dct_sof",
            "larger_than_declared",
        );
    }

    #[test]
    fn dct_sof_after_ascii_hex_is_checked() {
        let d = img(1, 1, "DeviceGray", "[/ASCIIHexDecode /DCTDecode]");
        let pdf = dct_image_doc(&d, &hex(&jpeg_sof(HUGE, HUGE, 3)));
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "dct_sof",
            "over_cap",
        );
    }

    #[test]
    fn dct_sof_after_flate_is_checked() {
        let d = img(1, 1, "DeviceGray", "[/FlateDecode /DCTDecode]");
        let pdf = dct_image_doc(&d, &zlib(&jpeg_sof(HUGE, HUGE, 3)));
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "dct_sof",
            "over_cap",
        );
    }

    #[test]
    fn dct_sof_with_bad_ascii85_is_rejected() {
        let pdf = subtypeless_doc("Thumb", &thumb_dict("8", "8"), b"\xFF\xD8junk");
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::Unsupported,
            "dct_sof",
            "prefilter",
        );
    }

    #[test]
    fn dct_sof_on_encrypted_branch_is_checked() {
        let plain = dct_image_doc(
            &img(1, 1, "DeviceGray", "/DCTDecode"),
            &jpeg_sof(HUGE, HUGE, 3),
        );
        let pdf = encrypt_empty_user(&plain, "--object-streams=disable");
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "dct_sof",
            "over_cap",
        );
    }

    // -- DCT: bypass di TEST-8 del test design (rossi sulla base) --

    #[test]
    fn dct_sof_extra_one_axis_larger_is_rejected() {
        let r = dct_check(100, 100, "DeviceRGB", &jpeg_sof(100, 200, 3));
        assert_coded(r, Kind::Unsupported, "dct_sof", "larger_than_declared");
        // Assi scambiati: dichiarato 100 × 50, SOF 50 × 100.
        let r = dct_check(100, 50, "DeviceRGB", &jpeg_sof(50, 100, 3));
        assert_coded(r, Kind::Unsupported, "dct_sof", "larger_than_declared");
    }

    #[test]
    fn dct_sof_extra_remaining_sof_markers_are_read() {
        for m in [0xC1u8, 0xC5, 0xC6, 0xC7, 0xCA, 0xCB, 0xCD, 0xCE] {
            let data = jpeg_segments(&[app0_jfif(), sof_segment(m, 30_000, 30_000, 3)]);
            let (k, d) = failed_of(dct_check(1, 1, "DeviceGray", &data));
            let mut t = d.split_whitespace();
            assert_eq!(
                (k, t.next(), t.next()),
                (Kind::DecompressionLimit, Some("dct_sof"), Some("over_cap")),
                "SOF {m:02X}: detail {d:?}"
            );
        }
    }

    #[test]
    fn dct_sof_extra_inconsistent_length_is_invalid() {
        // Nf = 3 ma L = 8 + 3 × 3 + 1.
        let mut p = sof_payload(8, 8, 3);
        p.push(0);
        let data = jpeg_segments(&[app0_jfif(), (0xC0, p)]);
        let r = dct_check(8, 8, "DeviceRGB", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "invalid");
    }

    #[test]
    fn dct_sof_extra_zero_width_or_components_is_invalid() {
        let data = jpeg_segments(&[app0_jfif(), sof_segment(0xC0, 0, 8, 3)]);
        assert_coded(
            dct_check(8, 8, "DeviceRGB", &data),
            Kind::Unsupported,
            "dct_sof",
            "invalid",
        );
        let data = jpeg_segments(&[app0_jfif(), sof_segment(0xC0, 8, 8, 0)]);
        assert_coded(
            dct_check(8, 8, "DeviceRGB", &data),
            Kind::Unsupported,
            "dct_sof",
            "invalid",
        );
    }

    #[test]
    fn dct_sof_extra_non_ff_byte_and_marker_00_are_invalid() {
        let sof = marker_segment(0xC0, &sof_payload(8, 8, 3));
        for junk in [&b"\x12"[..], &b"\xFF\x00"[..]] {
            let mut data = vec![0xFF, 0xD8];
            data.extend_from_slice(junk);
            data.extend_from_slice(&sof);
            data.extend_from_slice(&[0xFF, 0xD9]);
            let (k, d) = failed_of(dct_check(8, 8, "DeviceRGB", &data));
            let mut t = d.split_whitespace();
            assert_eq!(
                (k, t.next(), t.next()),
                (Kind::Unsupported, Some("dct_sof"), Some("invalid")),
                "dopo SOI {junk:02X?}: detail {d:?}"
            );
        }
    }

    #[test]
    fn dct_sof_extra_eoi_then_huge_jpeg_is_missing() {
        let mut data = b"\xFF\xD8\xFF\xD9".to_vec();
        data.extend_from_slice(&jpeg_sof(HUGE, HUGE, 3));
        let r = dct_check(1, 1, "DeviceGray", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "missing");
    }

    #[test]
    fn dct_sof_extra_double_soi_is_missing() {
        let mut data = b"\xFF\xD8".to_vec();
        data.extend_from_slice(&jpeg_sof(HUGE, HUGE, 3));
        let r = dct_check(1, 1, "DeviceGray", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "missing");
    }

    #[test]
    fn dct_sof_extra_junk_then_huge_jpeg_is_missing() {
        let mut data = b"junk".to_vec();
        data.extend_from_slice(&jpeg_sof(HUGE, HUGE, 3));
        let r = dct_check(1, 1, "DeviceGray", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "missing");
    }

    #[test]
    fn dct_sof_extra_sof_straddling_window_is_beyond_scan() {
        // Il SOF0 (19 byte) inizia a WINDOW - 5 e finisce oltre la finestra.
        let mut segs = filler(WINDOW - 5 - 2);
        segs.push(sof_segment(0xC0, 8, 8, 3));
        let data = jpeg_segments(&segs);
        assert_eq!(
            data.len(),
            WINDOW - 5 + 19 + 2,
            "fixture: posizione del SOF"
        );
        let r = dct_check(8, 8, "DeviceRGB", &data);
        assert_coded(r, Kind::Unsupported, "dct_sof", "beyond_scan");
    }

    #[test]
    fn dct_sof_extra_ascii85_with_excess_whitespace_is_beyond_scan() {
        // Oltre 4 × WINDOW spazi prima dei dati ASCII85: il prefisso grezzo dà 0 byte e i dati
        // proseguono.
        let mut data = vec![b' '; 4 * WINDOW + 1];
        data.extend_from_slice(&a85(&jpeg_sof(8, 8, 3)));
        let pdf = subtypeless_doc("Thumb", &thumb_dict("8", "8"), &data);
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::Unsupported,
            "dct_sof",
            "beyond_scan",
        );
    }

    #[test]
    fn dct_sof_extra_thumb_form_on_encrypted_branch_is_checked() {
        let plain = subtypeless_doc(
            "Thumb",
            &thumb_dict("8", "8"),
            &a85(&jpeg_sof(1000, 1000, 3)),
        );
        let pdf = encrypt_empty_user(&plain, "--object-streams=disable");
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::Unsupported,
            "dct_sof",
            "larger_than_declared",
        );
    }

    #[test]
    fn dct_sof_extra_flate_reread_counted_in_work() {
        // La rilettura Flate del prefisso entra nel lavoro I2 (D-10): delta fra l'/Image
        // [/FlateDecode /DCTDecode] e il gemello con il solo /FlateDecode e nessuna chiave
        // d'immagine = min(lunghezza del JPEG, WINDOW + 1).
        let jpeg_len = 400_000;
        let packed = zlib(&long_benign_jpeg(100, 100, jpeg_len));
        let image = dct_image_doc(
            &img(100, 100, "DeviceRGB", "[/FlateDecode /DCTDecode]"),
            &packed,
        );
        let twin = page_with_stream5("/Filter /FlateDecode", &packed);
        let wi = assert_ok(check_pdf(&image, &caps())).decompress_work_bytes;
        let wt = assert_ok(check_pdf(&twin, &caps())).decompress_work_bytes;
        assert_eq!(
            wi.checked_sub(wt),
            Some(jpeg_len.min(WINDOW + 1) as u64),
            "lavoro I2: immagine {wi}, gemello {wt}"
        );
    }

    // -- JPX (D-09): rossi sulla base --

    #[test]
    fn jpx_siz_larger_than_declared_is_rejected() {
        let r = jpx_check(8, 8, &jpx_codestream(1000, 1000, 3));
        assert_coded(r, Kind::Unsupported, "jpx_siz", "larger_than_declared");
    }

    #[test]
    fn jpx_siz_over_cap_is_rejected() {
        let r = jpx_check(1, 1, &jpx_codestream(HUGE, HUGE, 3));
        assert_coded(r, Kind::DecompressionLimit, "jpx_siz", "over_cap");
    }

    #[test]
    fn jpx_siz_in_jp2_box_is_checked() {
        let jp2 = jp2_file(
            &jp2_header_boxes(HUGE, HUGE, 3),
            &jpx_codestream(HUGE, HUGE, 3),
        );
        assert_coded(
            jpx_check(1, 1, &jp2),
            Kind::DecompressionLimit,
            "jpx_siz",
            "over_cap",
        );
    }

    #[test]
    fn jpx_siz_missing_is_rejected() {
        assert_coded(
            jpx_check(8, 8, b"junk"),
            Kind::Unsupported,
            "jpx_siz",
            "missing",
        );
    }

    #[test]
    fn jpx_siz_not_first_marker_is_invalid() {
        // SOC, poi COD, poi il SIZ.
        let mut data = vec![0xFF, 0x4F];
        data.extend_from_slice(&marker_segment(0x52, &[0, 0, 0, 1, 0, 0, 4, 4, 0, 1]));
        data.extend_from_slice(&marker_segment(0x51, &siz_payload(8, 8, 0, 0, 3)));
        data.extend_from_slice(&[0xFF, 0xD9]);
        assert_coded(
            jpx_check(8, 8, &data),
            Kind::Unsupported,
            "jpx_siz",
            "invalid",
        );
    }

    #[test]
    fn jpx_siz_truncated_is_rejected() {
        let cs = jpx_codestream(8, 8, 3);
        let siz_len = 2 + 2 + 38 + 3 * 3;
        let data = &cs[..2 + siz_len / 2];
        assert_coded(
            jpx_check(8, 8, data),
            Kind::Unsupported,
            "jpx_siz",
            "truncated",
        );
    }

    #[test]
    fn jpx_siz_origin_beyond_size_is_invalid() {
        let r = jpx_check(8, 8, &jpx_codestream_at(8, 8, 8, 0, 3));
        assert_coded(r, Kind::Unsupported, "jpx_siz", "invalid");
    }

    #[test]
    fn jpx_siz_zero_components_is_invalid() {
        // Csiz = 0 con Lsiz = 38.
        let r = jpx_check(8, 8, &jpx_codestream(8, 8, 0));
        assert_coded(r, Kind::Unsupported, "jpx_siz", "invalid");
    }

    #[test]
    fn jpx_siz_bad_box_length_is_invalid() {
        // Box `xml ` con LBox = 4 prima di un jp2c con SIZ uguale al dichiarato.
        let mut jp2 = JP2_SIGNATURE.to_vec();
        jp2.extend_from_slice(b"\x00\x00\x00\x04xml ");
        jp2.extend_from_slice(&jp2_box(b"jp2c", &jpx_codestream(8, 8, 3)));
        assert_coded(
            jpx_check(8, 8, &jp2),
            Kind::Unsupported,
            "jpx_siz",
            "invalid",
        );
    }

    #[test]
    fn jpx_siz_beyond_scan_in_jp2_is_rejected() {
        let jp2 = jp2_file(&[(b"xml ", vec![b' '; 300_000])], &jpx_codestream(8, 8, 3));
        assert_coded(
            jpx_check(8, 8, &jp2),
            Kind::Unsupported,
            "jpx_siz",
            "beyond_scan",
        );
    }

    #[test]
    fn jpx_siz_after_flate_is_checked() {
        let data = zlib(&jpx_codestream(HUGE, HUGE, 3));
        let r = jpx_check_with(1, 1, "[/FlateDecode /JPXDecode]", &data);
        assert_coded(r, Kind::DecompressionLimit, "jpx_siz", "over_cap");
    }

    // -- JPX aggiuntivi di TEST-22 del test design (rossi sulla base) --

    #[test]
    fn jpx_siz_extra_inconsistent_lsiz_is_invalid() {
        // Csiz = 3 e Lsiz = 38 + 3 × 3 + 3 = 50.
        let mut p = siz_payload(8, 8, 0, 0, 3);
        p.extend_from_slice(&[7, 1, 1]);
        let mut data = vec![0xFF, 0x4F];
        data.extend_from_slice(&marker_segment(0x51, &p));
        data.extend_from_slice(&jpx_tail());
        assert_coded(
            jpx_check(8, 8, &data),
            Kind::Unsupported,
            "jpx_siz",
            "invalid",
        );
    }

    #[test]
    fn jpx_siz_extra_xlbox_jp2c_is_checked() {
        // jp2c con LBox = 1 e XLBox esatto (intestazione di 16 byte).
        let cs = jpx_codestream(HUGE, HUGE, 3);
        let mut jp2 = JP2_SIGNATURE.to_vec();
        for (t, p) in jp2_header_boxes(HUGE, HUGE, 3) {
            jp2.extend_from_slice(&jp2_box(t, &p));
        }
        jp2.extend_from_slice(&1u32.to_be_bytes());
        jp2.extend_from_slice(b"jp2c");
        jp2.extend_from_slice(&(16 + cs.len() as u64).to_be_bytes());
        jp2.extend_from_slice(&cs);
        assert_coded(
            jpx_check(1, 1, &jp2),
            Kind::DecompressionLimit,
            "jpx_siz",
            "over_cap",
        );
    }

    #[test]
    fn jpx_siz_extra_xlbox_overflow_is_invalid() {
        // Box `uuid` con LBox = 1 e XLBox = u64::MAX prima di un jp2c uguale al dichiarato.
        let mut jp2 = JP2_SIGNATURE.to_vec();
        jp2.extend_from_slice(&1u32.to_be_bytes());
        jp2.extend_from_slice(b"uuid");
        jp2.extend_from_slice(&u64::MAX.to_be_bytes());
        jp2.extend_from_slice(&[0x55; 16]);
        jp2.extend_from_slice(&jp2_box(b"jp2c", &jpx_codestream(8, 8, 3)));
        assert_coded(
            jpx_check(8, 8, &jp2),
            Kind::Unsupported,
            "jpx_siz",
            "invalid",
        );
    }

    #[test]
    fn jpx_siz_extra_lbox_zero_not_jp2c_is_missing() {
        // Box `xml ` con LBox = 0 (arriverebbe alla fine dei dati), seguito da byte qualsiasi.
        let mut jp2 = JP2_SIGNATURE.to_vec();
        jp2.extend_from_slice(b"\x00\x00\x00\x00xml ");
        jp2.extend_from_slice(b"<a/>");
        jp2.extend_from_slice(&jp2_box(b"jp2c", &jpx_codestream(8, 8, 3)));
        assert_coded(
            jpx_check(8, 8, &jp2),
            Kind::Unsupported,
            "jpx_siz",
            "missing",
        );
    }

    #[test]
    fn jpx_siz_extra_one_axis_larger_is_rejected() {
        let r = jpx_check(100, 100, &jpx_codestream(100, 200, 3));
        assert_coded(r, Kind::Unsupported, "jpx_siz", "larger_than_declared");
    }

    #[test]
    fn jpx_siz_extra_on_encrypted_branch_is_checked() {
        let d = "/Width 1 /Height 1 /BitsPerComponent 8 /Filter /JPXDecode";
        let plain = dct_image_doc(d, &jpx_codestream(HUGE, HUGE, 3));
        let pdf = encrypt_empty_user(&plain, "--object-streams=disable");
        assert_coded(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "jpx_siz",
            "over_cap",
        );
    }

    #[test]
    fn jpx_siz_extra_after_ascii_hex_is_checked() {
        let data = hex(&jpx_codestream(HUGE, HUGE, 3));
        let r = jpx_check_with(1, 1, "[/ASCIIHexDecode /JPXDecode]", &data);
        assert_coded(r, Kind::DecompressionLimit, "jpx_siz", "over_cap");
    }

    #[test]
    fn jpx_siz_extra_siz_marker_cut_by_window_is_beyond_scan() {
        // Il contenuto di jp2c inizia a WINDOW - 3: nella finestra restano SOC e un solo byte
        // del marker SIZ, mentre i dati proseguono.
        let xml_payload = WINDOW - 3 - JP2_SIGNATURE.len() - 8 - 8;
        let jp2 = jp2_file(
            &[(b"xml ", vec![b' '; xml_payload])],
            &jpx_codestream(8, 8, 3),
        );
        assert_eq!(
            &jp2[WINDOW - 3..WINDOW],
            b"\xFF\x4F\xFF",
            "fixture: posizione del SIZ"
        );
        assert_coded(
            jpx_check(8, 8, &jp2),
            Kind::Unsupported,
            "jpx_siz",
            "beyond_scan",
        );
    }

    // -- verdi sulla base e dopo: confini benigni --

    #[test]
    fn dct_sof_equal_to_declared_passes() {
        assert_ok(dct_check(64, 48, "DeviceRGB", &jpeg_sof(64, 48, 3)));
    }

    #[test]
    fn dct_sof_smaller_than_declared_passes() {
        assert_ok(dct_check(100, 100, "DeviceRGB", &jpeg_sof(50, 50, 3)));
    }

    #[test]
    fn dct_sof_after_large_app_segments_within_scan_passes() {
        // SOI, 3 APP pieni (JFIF, EXIF, XMP o ICC), poi il SOF uguale: entro la finestra.
        let mut segs: Vec<(u8, Vec<u8>)> = [0xE0u8, 0xE1, 0xE2]
            .iter()
            .map(|m| (*m, vec![0x41; 65_533]))
            .collect();
        segs.push(sof_segment(0xC0, 64, 48, 3));
        assert_ok(dct_check(64, 48, "DeviceRGB", &jpeg_segments(&segs)));
    }

    #[test]
    fn dct_sof_after_exif_thumbnail_sof_passes() {
        // APP1 EXIF con una miniatura JPEG il cui SOF è enorme, poi il SOF vero uguale.
        let mut exif = b"Exif\x00\x00".to_vec();
        exif.extend_from_slice(&jpeg_sof(HUGE, HUGE, 3));
        let data = jpeg_segments(&[(0xE1, exif), sof_segment(0xC0, 64, 48, 3)]);
        assert_ok(dct_check(64, 48, "DeviceRGB", &data));
    }

    #[test]
    fn dct_sof_flate_benign_keeps_work_within_bound() {
        let c = caps();
        let plain = dct_image_doc(
            &img(100, 100, "DeviceRGB", "[/FlateDecode /DCTDecode]"),
            &zlib(&long_benign_jpeg(100, 100, 400_000)),
        );
        assert_work_unencrypted(&assert_ok(check_pdf(&plain, &c)), &c);
        let enc = encrypt_empty_user(&plain, "--object-streams=disable");
        assert_work_encrypted(&assert_ok(check_pdf(&enc, &c)), &c);
    }

    #[test]
    fn fixture_t_data_is_ascii85_of_sof_8x8() {
        assert_eq!(T_DATA, a85(&jpeg_sof(8, 8, 3)).as_slice());
        let decoded = ascii85(T_DATA).unwrap_or_else(|e| panic!("T_DATA non è ASCII85: {e:?}"));
        assert_eq!(decoded, jpeg_sof(8, 8, 3));
    }

    #[test]
    fn fixture_image_doc_carries_sof_equal_to_declared() {
        let doc = lopdf_doc(&image_doc(100, 100));
        let data = match doc.get_object((5, 0)) {
            Ok(Object::Stream(s)) => s.content.clone(),
            other => panic!("oggetto 5 non è uno stream: {other:?}"),
        };
        assert_eq!(data, jpeg_sof(100, 100, 3));
        assert_eq!(
            &data[2 + 18..2 + 18 + 9],
            b"\xFF\xC0\x00\x11\x08\x00\x64\x00\x64"
        );
    }

    #[test]
    fn dct_sof_boundary_dht_dac_jpg_payload_not_sof() {
        // Payload a forma di SOF enorme in DHT (C4), DAC (CC) e JPG (C8): non sono SOF.
        let shaped = sof_payload(30_000, 30_000, 3);
        let data = jpeg_segments(&[
            (0xC4, shaped.clone()),
            (0xCC, shaped.clone()),
            (0xC8, shaped),
            sof_segment(0xC0, 64, 48, 3),
        ]);
        assert_ok(dct_check(64, 48, "DeviceRGB", &data));
    }

    #[test]
    fn dct_sof_boundary_first_of_two_sofs_wins() {
        // Caratterizzazione: decide il primo SOF (libjpeg rifiuta un secondo SOF prima di SOS).
        let data = jpeg_segments(&[
            sof_segment(0xC0, 64, 48, 3),
            sof_segment(0xC0, 30_000, 30_000, 3),
        ]);
        assert_ok(dct_check(64, 48, "DeviceRGB", &data));
    }

    #[test]
    fn dct_sof_boundary_sof_ending_at_window_passes() {
        let mut segs = filler(WINDOW - 19 - 2);
        segs.push(sof_segment(0xC0, 64, 48, 3));
        let data = jpeg_segments(&segs);
        assert_eq!(
            &data[WINDOW - 19..WINDOW - 17],
            b"\xFF\xC0",
            "fixture: posizione del SOF"
        );
        assert!(
            data.len() > WINDOW,
            "fixture: i dati proseguono oltre la finestra"
        );
        assert_ok(dct_check(64, 48, "DeviceRGB", &data));
    }

    #[test]
    fn dct_sof_boundary_many_empty_segments_pass() {
        let mut segs: Vec<(u8, Vec<u8>)> = (0..60_000).map(|_| (0xFE, Vec::new())).collect();
        segs.push(sof_segment(0xC0, 64, 48, 3));
        assert_ok(dct_check(64, 48, "DeviceRGB", &jpeg_segments(&segs)));
    }

    #[test]
    fn dct_sof_boundary_one_axis_smaller_passes() {
        assert_ok(dct_check(100, 100, "DeviceRGB", &jpeg_sof(100, 50, 3)));
    }

    #[test]
    fn dct_sof_boundary_dct_abbreviation_stays_unsupported() {
        let pdf = dct_image_doc(&img(64, 48, "DeviceRGB", "/DCT"), &jpeg_sof(64, 48, 3));
        let (k, d) = failed_of(check_pdf(&pdf, &caps()));
        assert_eq!((k, d.as_str()), (Kind::Unsupported, "filter DCT"));
    }

    #[test]
    fn dct_sof_flate_bomb_keeps_decompression_limit() {
        // TEST-19: una bomba Flate oltre per_stream_bytes davanti a DCTDecode resta per_stream.
        let pdf = dct_image_doc(
            &img(1, 1, "DeviceGray", "[/FlateDecode /DCTDecode]"),
            &zlib_zeros(MIB + 1),
        );
        assert_failed(
            check_pdf(&pdf, &caps()),
            Kind::DecompressionLimit,
            "per_stream",
        );
    }

    #[test]
    fn jpx_siz_equal_to_declared_passes() {
        assert_ok(jpx_check(64, 48, &jpx_codestream(64, 48, 3)));
    }

    #[test]
    fn jpx_siz_in_jp2_equal_passes() {
        let jp2 = jp2_file(&jp2_header_boxes(64, 48, 3), &jpx_codestream(64, 48, 3));
        assert_ok(jpx_check(64, 48, &jp2));
    }

    #[test]
    fn jpx_siz_with_origin_offset_within_declared_passes() {
        // Xsiz − XOsiz = 64 e Ysiz − YOsiz = 48: si confronta la differenza, non Xsiz.
        assert_ok(jpx_check(
            64,
            48,
            &jpx_codestream_at(1064, 1048, 1000, 1000, 3),
        ));
    }

    #[test]
    fn jpx_siz_extra_first_of_two_jp2c_wins() {
        // Caratterizzazione: decide il primo jp2c, come in OpenJPEG.
        let mut jp2 = jp2_file(&jp2_header_boxes(64, 48, 3), &jpx_codestream(64, 48, 3));
        jp2.extend_from_slice(&jp2_box(b"jp2c", &jpx_codestream(HUGE, HUGE, 3)));
        assert_ok(jpx_check(64, 48, &jp2));
    }

    #[test]
    fn jpx_siz_extra_first_codestream_wins_over_trailing_data() {
        // Caratterizzazione: dopo il SIZ la lettura si ferma, il secondo SOC non conta.
        let mut data = jpx_codestream(64, 48, 3);
        data.extend_from_slice(&jpx_codestream(HUGE, HUGE, 3));
        assert_ok(jpx_check(64, 48, &data));
    }

    // -- D-03: nessun nuovo falso positivo sui corpora reali (#[ignore], nel runner) --

    /// Stream la cui catena finisce con `DCTDecode` e con `JPXDecode` (per i conteggi).
    fn image_stream_counts(bytes: &[u8]) -> (u32, u32) {
        let Ok(doc) = Document::load_mem(bytes) else {
            return (0, 0);
        };
        let mut dct = 0;
        let mut jpx = 0;
        for o in doc.objects.values() {
            if let Object::Stream(s) = o
                && let Ok(chain) = filters(&s.dict)
            {
                match chain.last().map(Vec::as_slice) {
                    Some(b"DCTDecode") => dct += 1,
                    Some(b"JPXDecode") => jpx += 1,
                    _ => {}
                }
            }
        }
        (dct, jpx)
    }

    /// `check_pdf` con `Caps::default()` su ogni file; stampa la riga `<tag>: …` e fallisce
    /// se un file è respinto o se i file non sono `attesi`.
    fn assert_corpus_passes(tag: &str, files: &[(String, PathBuf)], attesi: usize, fetch: &str) {
        let mut ok = 0;
        let (mut dct, mut jpx) = (0, 0);
        let mut rejected = Vec::new();
        for (id, path) in files {
            let bytes = std::fs::read(path)
                .unwrap_or_else(|e| panic!("{id}: file assente ({e}); lanciare {fetch}"));
            let (d, j) = image_stream_counts(&bytes);
            dct += d;
            jpx += j;
            match check_pdf(&bytes, &Caps::default()) {
                Ok(_) => ok += 1,
                Err(e) => rejected.push(format!("{id}: {e:?}")),
            }
        }
        eprintln!(
            "{tag}: {ok} su {} ok, stream_dct={dct} stream_jpx={jpx}",
            files.len()
        );
        assert_eq!(
            files.len(),
            attesi,
            "{tag}: file trovati diversi dagli attesi"
        );
        assert!(
            rejected.is_empty(),
            "{tag}: PDF respinti:\n{}",
            rejected.join("\n")
        );
    }

    #[test]
    #[ignore = "richiede spikes/data/s02/raw (fetch.sh di 01-08)"]
    fn s02_corpus_pdfs_pass_prescan() {
        let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/s02"));
        let manifest = std::fs::read(root.join("manifest.json")).expect("manifest.json di s02");
        let entries: serde_json::Value =
            serde_json::from_slice(&manifest).expect("manifest.json non è JSON");
        let files: Vec<(String, PathBuf)> = entries
            .as_array()
            .expect("manifest.json non è un array")
            .iter()
            .filter(|e| e["role"] == "pdf")
            .map(|e| {
                (
                    e["item_id"].as_str().unwrap_or("?").to_owned(),
                    root.join("raw")
                        .join(e["file"].as_str().expect("voce senza file")),
                )
            })
            .collect();
        assert_corpus_passes("s02", &files, 63, "spikes/data/s02/fetch.sh");
    }

    #[test]
    #[ignore = "richiede spikes/data/corpus/raw (fetch.sh di 01-03)"]
    fn corpus_pdfs_pass_prescan() {
        let raw = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../data/corpus/raw"));
        let mut files: Vec<(String, PathBuf)> = std::fs::read_dir(&raw)
            .unwrap_or_else(|e| panic!("{e}; lanciare spikes/data/corpus/fetch.sh"))
            .filter_map(Result::ok)
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "pdf"))
            .map(|p| {
                let id = p
                    .file_name()
                    .map(|n| n.to_string_lossy().into_owned())
                    .unwrap_or_default();
                (id, p)
            })
            .collect();
        files.sort();
        assert_corpus_passes("corpus", &files, 10, "spikes/data/corpus/fetch.sh");
    }
}
