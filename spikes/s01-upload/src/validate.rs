//! Validazione prima di storage e DB: magic bytes, limiti DOCX, prefiltro byte-level PDF.
//!
//! Il formato si riconosce SOLO dai byte (magic), mai dal nome file del client (T-08):
//! `%PDF-` → PDF; `PK\x03\x04` → DOCX (con `[Content_Types].xml` e `word/document.xml`);
//! altrimenti Markdown (UTF-8 valido, senza NUL). Un archivio ZIP che non è un DOCX, o
//! un file che non è nessuno dei tre formati, è 415 (polyglot e magic mismatch).
//!
//! **Il controllo PDF è un prefiltro.** Scandisce i byte grezzi e decodifica le
//! sequenze `#hh` nei nomi, ma i nomi dentro object stream compressi (`/ObjStm` con
//! `FlateDecode`) gli sfuggono. Il controllo autorevole dei PDF resta al processo
//! isolato di S-02 (D-08); qui si scartano solo i casi economici (D-05).
//!
//! La funzione è sincrona e gira dentro `spawn_blocking`: deadline e cancellazione si
//! controllano a ogni voce zip e a ogni MiB di scansione (RISK-003).

use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

/// Esito di un file ammesso.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verdict {
    Accepted,
    Quarantined,
}

impl Verdict {
    /// Valore del campo `status` della risposta.
    pub fn as_str(self) -> &'static str {
        match self {
            Verdict::Accepted => "accepted",
            Verdict::Quarantined => "quarantined",
        }
    }
}

/// File rifiutato: stato HTTP e codice d'errore (es. `validation_timeout`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rejection {
    pub http_status: u16,
    pub code: String,
}

impl Rejection {
    fn new(http_status: u16, code: &str) -> Self {
        Rejection {
            http_status,
            code: code.to_string(),
        }
    }
    fn unsupported(code: &str) -> Self {
        Self::new(415, code)
    }
    fn unprocessable(code: &str) -> Self {
        Self::new(422, code)
    }
}

const MIB: u64 = 1024 * 1024;

/// Limiti DOCX (T-07, anti zip bomb).
pub const DOCX_MAX_ENTRIES: usize = 1_000;
pub const DOCX_MAX_DECLARED_TOTAL: u64 = 200 * MIB;
pub const DOCX_RATIO_MIN_SIZE: u64 = 16 * MIB;
pub const DOCX_MAX_RATIO: u64 = 100;

/// Nomi PDF che portano al rifiuto o alla quarantena (confronto dopo decodifica `#hh`).
const PDF_REJECT: &[&[u8]] = &[b"Encrypt"];
const PDF_QUARANTINE: &[&[u8]] = &[
    b"JavaScript",
    b"JS",
    b"Launch",
    b"EmbeddedFile",
    b"EmbeddedFiles",
];

/// Limite: oltre questa lunghezza un nome PDF non è fra quelli cercati.
const PDF_NAME_MAX: usize = 64;

/// Guardia comune di deadline e cancellazione.
struct Guard<'a> {
    deadline: Instant,
    cancel: &'a AtomicBool,
}

impl Guard<'_> {
    fn check(&self) -> Result<(), Rejection> {
        if self.cancel.load(Ordering::Relaxed) {
            return Err(Rejection::unprocessable("validation_cancelled"));
        }
        if Instant::now() >= self.deadline {
            return Err(Rejection::unprocessable("validation_timeout"));
        }
        Ok(())
    }
}

fn io_err(_e: std::io::Error) -> Rejection {
    Rejection::new(500, "validation_io")
}

/// Controlla il file in `path` entro `deadline`, interrompibile con `cancel`.
pub fn check(path: &Path, deadline: Instant, cancel: &AtomicBool) -> Result<Verdict, Rejection> {
    let guard = Guard { deadline, cancel };
    let mut f = File::open(path).map_err(io_err)?;
    let mut head = [0u8; 8];
    let n = read_up_to(&mut f, &mut head).map_err(io_err)?;
    let head = &head[..n];
    if head.starts_with(b"%PDF-") {
        check_pdf(f, &guard)
    } else if head.starts_with(b"PK\x03\x04") {
        check_docx(f, &guard)
    } else {
        check_markdown(f, &guard)
    }
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> std::io::Result<usize> {
    let mut n = 0;
    while n < buf.len() {
        let k = f.read(&mut buf[n..])?;
        if k == 0 {
            break;
        }
        n += k;
    }
    Ok(n)
}

// ------------------------------------------------------------------------ Markdown

fn check_markdown(mut f: File, guard: &Guard<'_>) -> Result<Verdict, Rejection> {
    f.seek(SeekFrom::Start(0)).map_err(io_err)?;
    let mut buf = vec![0u8; MIB as usize];
    // Coda di un carattere UTF-8 spezzato fra due blocchi (al più 3 byte).
    let mut carry: Vec<u8> = Vec::with_capacity(4);
    loop {
        guard.check()?;
        let k = f.read(&mut buf).map_err(io_err)?;
        if k == 0 {
            break;
        }
        let block = &buf[..k];
        if block.contains(&0) {
            return Err(Rejection::unsupported("unsupported_format"));
        }
        let mut joined = std::mem::take(&mut carry);
        joined.extend_from_slice(block);
        match std::str::from_utf8(&joined) {
            Ok(_) => {}
            Err(e) if e.error_len().is_none() => {
                // Sequenza incompleta in coda: si completa col blocco successivo.
                carry = joined[e.valid_up_to()..].to_vec();
            }
            Err(_) => return Err(Rejection::unsupported("unsupported_format")),
        }
    }
    if !carry.is_empty() {
        return Err(Rejection::unsupported("unsupported_format"));
    }
    Ok(Verdict::Accepted)
}

// ------------------------------------------------------------------------ DOCX

fn check_docx(f: File, guard: &Guard<'_>) -> Result<Verdict, Rejection> {
    // Archivio letto dall'offset 0 (`ArchiveOffset::Known(0)`), senza la ricerca
    // automatica dell'offset: dati anteposti allo ZIP (polyglot) non si riallineano.
    // Limiti anti zip bomb (voci, somma dichiarata, rapporto, lettura con `take()`) sotto;
    // nessuna estrazione su disco, quindi i nomi delle voci non diventano percorsi.
    let config = zip::read::Config {
        archive_offset: zip::read::ArchiveOffset::Known(0),
    };
    let mut zip = match zip::ZipArchive::with_config(config, BufReader::new(f)) {
        Ok(z) => z,
        // Magic ZIP ma archivio illeggibile: non è un formato ammesso.
        Err(_) => return Err(Rejection::unsupported("unsupported_format")),
    };
    if zip.len() > DOCX_MAX_ENTRIES {
        return Err(Rejection::unprocessable("docx_too_many_entries"));
    }
    if zip.index_for_name("[Content_Types].xml").is_none()
        || zip.index_for_name("word/document.xml").is_none()
    {
        // ZIP qualsiasi (anche con nome `.pdf`): polyglot o formato non ammesso.
        return Err(Rejection::unsupported("unsupported_format"));
    }

    // Passo 1: solo metadati dichiarati (nessuna decompressione).
    let mut declared_total: u64 = 0;
    for i in 0..zip.len() {
        guard.check()?;
        let e = zip
            .by_index_raw(i)
            .map_err(|_| Rejection::unprocessable("docx_corrupt"))?;
        let name = e
            .name()
            .map_err(|_| Rejection::unprocessable("docx_corrupt"))?
            .to_ascii_lowercase();
        if name.ends_with("vbaproject.bin") || name.ends_with(".docm") {
            return Err(Rejection::unprocessable("docx_macro"));
        }
        if e.encrypted() {
            return Err(Rejection::unprocessable("docx_encrypted"));
        }
        let size = e.size();
        declared_total = declared_total.saturating_add(size);
        if declared_total > DOCX_MAX_DECLARED_TOTAL {
            return Err(Rejection::unprocessable("docx_too_large"));
        }
        if size > DOCX_RATIO_MIN_SIZE {
            let comp = e.compressed_size().max(1);
            if size / comp > DOCX_MAX_RATIO {
                return Err(Rejection::unprocessable("docx_ratio"));
            }
        }
    }

    // Passo 2: lettura in streaming con `take()` sul dichiarato (+1 per scoprire le voci
    // che mentono), DTD/entità rifiutate nelle parti XML, macro dal content type.
    let mut buf = vec![0u8; 64 * 1024];
    for i in 0..zip.len() {
        guard.check()?;
        let mut e = zip
            .by_index(i)
            .map_err(|_| Rejection::unprocessable("docx_corrupt"))?;
        let name = e
            .name()
            .map_err(|_| Rejection::unprocessable("docx_corrupt"))?
            .to_ascii_lowercase();
        let declared = e.size();
        let is_xml = name.ends_with(".xml") || name.ends_with(".rels");
        let is_ct = name == "[content_types].xml";
        let mut scanner = XmlScanner::default();
        let mut read: u64 = 0;
        let mut since_check: u64 = 0;
        let mut r = (&mut e).take(declared + 1);
        loop {
            let k = r
                .read(&mut buf)
                .map_err(|_| Rejection::unprocessable("docx_corrupt"))?;
            if k == 0 {
                break;
            }
            read += k as u64;
            if read > declared {
                return Err(Rejection::unprocessable("docx_size_mismatch"));
            }
            if is_xml {
                scanner.feed(&buf[..k])?;
                if is_ct && scanner.macro_enabled {
                    return Err(Rejection::unprocessable("docx_macro"));
                }
            }
            since_check += k as u64;
            if since_check >= MIB {
                since_check = 0;
                guard.check()?;
            }
        }
    }
    Ok(Verdict::Accepted)
}

/// Cerca `<!DOCTYPE` e `<!ENTITY` (senza distinzione di maiuscole) e il content type
/// `macroEnabled` attraverso i confini dei blocchi.
#[derive(Default)]
struct XmlScanner {
    tail: Vec<u8>,
    macro_enabled: bool,
}

impl XmlScanner {
    const OVERLAP: usize = 16;

    fn feed(&mut self, block: &[u8]) -> Result<(), Rejection> {
        let mut joined = std::mem::take(&mut self.tail);
        joined.extend_from_slice(block);
        let lower = joined.to_ascii_lowercase();
        if find(&lower, b"<!doctype").is_some() || find(&lower, b"<!entity").is_some() {
            return Err(Rejection::unprocessable("docx_dtd"));
        }
        if find(&lower, b"macroenabled").is_some() {
            self.macro_enabled = true;
        }
        let keep = joined.len().min(Self::OVERLAP);
        self.tail = joined[joined.len() - keep..].to_vec();
        Ok(())
    }
}

fn find(hay: &[u8], needle: &[u8]) -> Option<usize> {
    hay.windows(needle.len()).position(|w| w == needle)
}

// ------------------------------------------------------------------------ PDF

fn check_pdf(mut f: File, guard: &Guard<'_>) -> Result<Verdict, Rejection> {
    let len = f.metadata().map_err(io_err)?.len();
    // Struttura minima: `%%EOF` negli ultimi 1.024 byte (file troncato → 422).
    let tail_len = len.min(1024);
    f.seek(SeekFrom::Start(len - tail_len)).map_err(io_err)?;
    let mut tail = vec![0u8; tail_len as usize];
    f.read_exact(&mut tail).map_err(io_err)?;
    let has_eof = find(&tail, b"%%EOF").is_some();

    f.seek(SeekFrom::Start(0)).map_err(io_err)?;
    let mut scanner = PdfNameScanner::default();
    let mut buf = vec![0u8; MIB as usize];
    loop {
        guard.check()?;
        let k = f.read(&mut buf).map_err(io_err)?;
        if k == 0 {
            break;
        }
        scanner.feed(&buf[..k]);
        if scanner.encrypted {
            return Err(Rejection::unprocessable("pdf_encrypted"));
        }
    }
    scanner.finish();
    if scanner.encrypted {
        return Err(Rejection::unprocessable("pdf_encrypted"));
    }
    if !has_eof {
        return Err(Rejection::unprocessable("pdf_truncated"));
    }
    if scanner.active {
        Ok(Verdict::Quarantined)
    } else {
        Ok(Verdict::Accepted)
    }
}

/// Automa byte per byte: dopo `/` accumula il nome fino a un delimitatore PDF,
/// decodificando `#hh` (ISO 32000-1 §7.3.5), e lo confronta con gli elenchi.
#[derive(Default)]
struct PdfNameScanner {
    in_name: bool,
    name: Vec<u8>,
    overflow: bool,
    /// 0 = nessun `#` in corso; 1 = letto `#`; 2 = letta la prima cifra.
    hex_state: u8,
    hex_hi: u8,
    encrypted: bool,
    active: bool,
}

fn is_delim(b: u8) -> bool {
    matches!(
        b,
        b' ' | b'\t'
            | b'\r'
            | b'\n'
            | 0x0c
            | 0x00
            | b'/'
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'['
            | b']'
            | b'{'
            | b'}'
            | b'%'
    )
}

fn hex_val(b: u8) -> Option<u8> {
    match b {
        b'0'..=b'9' => Some(b - b'0'),
        b'a'..=b'f' => Some(b - b'a' + 10),
        b'A'..=b'F' => Some(b - b'A' + 10),
        _ => None,
    }
}

impl PdfNameScanner {
    /// Fuori da un nome salta direttamente al prossimo `/` (i byte intermedi non
    /// cambiano lo stato): il riempitivo e gli stream non pagano l'automa byte per byte.
    fn feed(&mut self, block: &[u8]) {
        let mut i = 0;
        while i < block.len() {
            if self.in_name {
                self.push(block[i]);
                i += 1;
            } else {
                match block[i..].iter().position(|&b| b == b'/') {
                    Some(p) => {
                        self.start_name();
                        i += p + 1;
                    }
                    None => break,
                }
            }
        }
    }

    fn push(&mut self, b: u8) {
        if self.in_name {
            if is_delim(b) {
                self.end_name();
                // `/` chiude un nome e ne apre subito un altro.
                if b == b'/' {
                    self.start_name();
                }
                return;
            }
            match self.hex_state {
                1 => match hex_val(b) {
                    Some(v) => {
                        self.hex_hi = v;
                        self.hex_state = 2;
                    }
                    None => {
                        self.hex_state = 0;
                        self.add(b'#');
                        self.add(b);
                    }
                },
                2 => {
                    self.hex_state = 0;
                    match hex_val(b) {
                        Some(v) => self.add(self.hex_hi << 4 | v),
                        None => {
                            self.add(b'#');
                            self.add(b);
                        }
                    }
                }
                _ => {
                    if b == b'#' {
                        self.hex_state = 1;
                    } else {
                        self.add(b);
                    }
                }
            }
        } else if b == b'/' {
            self.start_name();
        }
    }

    fn start_name(&mut self) {
        self.in_name = true;
        self.name.clear();
        self.overflow = false;
        self.hex_state = 0;
    }

    fn add(&mut self, b: u8) {
        if self.name.len() < PDF_NAME_MAX {
            self.name.push(b);
        } else {
            self.overflow = true;
        }
    }

    fn end_name(&mut self) {
        self.in_name = false;
        if self.overflow {
            return;
        }
        let n = self.name.as_slice();
        if PDF_REJECT.contains(&n) {
            self.encrypted = true;
        }
        if PDF_QUARANTINE.contains(&n) {
            self.active = true;
        }
    }

    fn finish(&mut self) {
        if self.in_name {
            self.end_name();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(bytes: &[u8]) -> (bool, bool) {
        let mut s = PdfNameScanner::default();
        s.feed(bytes);
        s.finish();
        (s.encrypted, s.active)
    }

    #[test]
    fn nomi_decodificati_e_delimitati() {
        assert_eq!(scan(b"<< /Encrypt 3 0 R >>"), (true, false));
        assert_eq!(scan(b"<</Encr#79pt 3 0 R>>"), (true, false));
        assert_eq!(scan(b"/S/J#61vaScript"), (false, true));
        assert_eq!(scan(b"/JSON /Encrypted /Launcher"), (false, false));
        assert_eq!(scan(b"/Type/EmbeddedFile"), (false, true));
    }
}
