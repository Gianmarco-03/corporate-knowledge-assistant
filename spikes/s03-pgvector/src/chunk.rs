//! Chunker di misura (piano 01-10): circa 1.600 caratteri con sovrapposizione di 200,
//! mai a cavallo di due pagine o due sezioni; offset in caratteri.
//!
//! È un chunker di misura, non quello di prodotto: serve a confrontare i modelli di
//! embedding sugli stessi pezzi di testo. Il testo dei PDF viene da `pdftotext -layout`
//! (eseguito nel runner), una pagina per form feed; i Markdown si spezzano per sezione.
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, bail};
use serde::Deserialize;

/// Lingua del chunk, nome della configurazione testuale di PostgreSQL (`regconfig`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lang {
    Italian,
    English,
}

impl Lang {
    /// Nome della configurazione testuale di PostgreSQL.
    pub fn regconfig(self) -> &'static str {
        match self {
            Lang::Italian => "italian",
            Lang::English => "english",
        }
    }

    /// Lingua dal campo `lang` del manifest (`it`, `en`) o dal nome della `regconfig`.
    pub fn parse(s: &str) -> anyhow::Result<Lang> {
        match s {
            "it" | "italian" => Ok(Lang::Italian),
            "en" | "english" => Ok(Lang::English),
            other => bail!("lingua non prevista: {other}"),
        }
    }
}

/// Lunghezza massima di un chunk, in caratteri.
pub const CHUNK_CHARS: usize = 1_600;
/// Sovrapposizione fra chunk consecutivi della stessa pagina o sezione, in caratteri.
pub const OVERLAP_CHARS: usize = 200;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Chunk {
    pub doc_key: String,
    /// Pagina fisica del PDF a partire da 1; `None` per i Markdown.
    pub page: Option<u32>,
    /// Intestazioni dalla più esterna alla più interna separate da " > "; `None` per i PDF.
    pub section_path: Option<String>,
    /// Offset in caratteri: relativi alla pagina per i PDF, al documento per i Markdown.
    pub char_start: usize,
    pub char_end: usize,
    pub lang: Lang,
    pub body: String,
}

impl Chunk {
    /// Id stabile (UUID versione 8) derivato da documento, pagina o sezione e offset: lo
    /// stesso corpus produce gli stessi id in ogni worktree, così i candidati esportati
    /// per il reranker (piano 01-15) sono confrontabili. Hash FNV-1a a 128 bit, non
    /// crittografico: serve solo a rendere l'id deterministico.
    pub fn stable_id(&self) -> uuid::Uuid {
        const OFFSET: u128 = 0x6c62_272e_07bb_0142_62b8_2175_6295_c58d;
        const PRIME: u128 = 0x0000_0000_0100_0000_0000_0000_0000_013b;
        let key = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            self.doc_key,
            self.page.map_or(String::new(), |p| p.to_string()),
            self.section_path.as_deref().unwrap_or(""),
            self.char_start,
            self.char_end
        );
        let mut h = OFFSET;
        for b in key.bytes() {
            h ^= u128::from(b);
            h = h.wrapping_mul(PRIME);
        }
        uuid::Builder::from_custom_bytes(h.to_be_bytes()).into_uuid()
    }
}

/// Finestre `[start, end)` in caratteri su un testo lungo `len`: al più `CHUNK_CHARS`
/// caratteri, consecutive sovrapposte di `OVERLAP_CHARS`, copertura completa.
fn windows(len: usize) -> Vec<(usize, usize)> {
    let mut out = Vec::new();
    if len == 0 {
        return out;
    }
    let mut start = 0;
    loop {
        let end = (start + CHUNK_CHARS).min(len);
        out.push((start, end));
        if end == len {
            break;
        }
        start = end - OVERLAP_CHARS;
    }
    out
}

/// Spezza `chars[from..to]` in finestre con offset assoluti in `chars`. Le finestre di
/// soli spazi non diventano chunk.
fn window_chunks(
    chars: &[char],
    from: usize,
    to: usize,
    mut make: impl FnMut(usize, usize, String) -> Chunk,
) -> Vec<Chunk> {
    windows(to - from)
        .into_iter()
        .filter_map(|(a, b)| {
            let body: String = chars[from + a..from + b].iter().collect();
            if body.trim().is_empty() {
                None
            } else {
                Some(make(from + a, from + b, body))
            }
        })
        .collect()
}

/// Testo di un PDF con le pagine separate da form feed (`pdftotext -layout`).
pub fn chunk_pages(doc_key: &str, lang: Lang, text: &str) -> Vec<Chunk> {
    let mut out = Vec::new();
    for (i, page_text) in text.split('\u{c}').enumerate() {
        let page = u32::try_from(i + 1).unwrap_or(u32::MAX);
        let chars: Vec<char> = page_text.chars().collect();
        out.extend(window_chunks(&chars, 0, chars.len(), |a, b, body| Chunk {
            doc_key: doc_key.to_string(),
            page: Some(page),
            section_path: None,
            char_start: a,
            char_end: b,
            lang,
            body,
        }));
    }
    out
}

/// Livello e titolo di una riga di intestazione ATX (`#` … `######` seguiti da spazio).
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(' ') {
        return None;
    }
    let title = rest.trim().trim_end_matches('#').trim();
    Some((level, title.to_string()))
}

/// Percorso di sezione: intestazioni di livello ≥ 2 dalla più esterna alla più interna.
/// L'H1 è il titolo del documento e non entra nel percorso (convenzione dei dati degli
/// oracoli, es. `syn-06`); il testo sotto il solo H1 ha come percorso il titolo stesso.
fn section_path(stack: &[(usize, String)]) -> String {
    let inner: Vec<&str> = stack
        .iter()
        .filter(|(l, _)| *l >= 2)
        .map(|(_, t)| t.as_str())
        .collect();
    if inner.is_empty() {
        stack
            .iter()
            .map(|(_, t)| t.as_str())
            .collect::<Vec<_>>()
            .join(" > ")
    } else {
        inner.join(" > ")
    }
}

/// Markdown spezzato per sezione (intestazioni `#`). Il front matter YAML iniziale
/// (`---` … `---`) non si indicizza; offset in caratteri sul documento intero.
pub fn chunk_markdown(doc_key: &str, lang: Lang, md: &str) -> Vec<Chunk> {
    let chars: Vec<char> = md.chars().collect();
    // Righe con offset in caratteri: (inizio, testo senza fine riga).
    let mut lines: Vec<(usize, &str)> = Vec::new();
    let mut pos = 0;
    for raw in md.split_inclusive('\n') {
        lines.push((pos, raw.trim_end_matches(['\n', '\r'])));
        pos += raw.chars().count();
    }

    // Front matter: le righe fino al secondo `---` compreso restano fuori.
    let mut first = 0;
    if lines.first().is_some_and(|l| l.1.trim_end() == "---")
        && let Some(close) = lines.iter().skip(1).position(|l| l.1.trim_end() == "---")
    {
        first = close + 2;
    }
    let first = first.min(lines.len());

    // Confini di sezione: ogni riga di intestazione apre una sezione nuova.
    let mut sections: Vec<(usize, usize, String)> = Vec::new();
    let mut stack: Vec<(usize, String)> = Vec::new();
    let mut cur_start = lines.get(first).map_or(chars.len(), |l| l.0);
    let mut cur_path = String::new();
    for &(start, text) in &lines[first..] {
        if let Some((level, title)) = heading(text) {
            if start > cur_start {
                sections.push((cur_start, start, cur_path.clone()));
            }
            while stack.last().is_some_and(|(l, _)| *l >= level) {
                stack.pop();
            }
            stack.push((level, title));
            cur_path = section_path(&stack);
            cur_start = start;
        }
    }
    if chars.len() > cur_start {
        sections.push((cur_start, chars.len(), cur_path));
    }

    let mut out = Vec::new();
    for (from, to, path) in sections {
        out.extend(window_chunks(&chars, from, to, |a, b, body| Chunk {
            doc_key: doc_key.to_string(),
            page: None,
            section_path: Some(path.clone()),
            char_start: a,
            char_end: b,
            lang,
            body,
        }));
    }
    out
}

/// Voce di `manifest.json` del corpus usata dal chunker.
#[derive(Clone, Debug, Deserialize)]
pub struct CorpusDoc {
    pub doc_id: String,
    pub lang: String,
    pub format: String,
    pub classification: String,
    pub sha256: String,
}

/// Legge `manifest.json` del corpus.
pub fn read_manifest(corpus_dir: &Path) -> anyhow::Result<Vec<CorpusDoc>> {
    let path = corpus_dir.join("manifest.json");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("manifest del corpus {} illeggibile", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("manifest {} non valido", path.display()))
}

/// File sorgente di un documento indicizzabile: PDF in `raw/`, Markdown in `synthetic/`.
/// I DOCX non si indicizzano: ripetono il loro Markdown (ORACOLI.md, «Duplicati del
/// corpus») e la loro estrazione è materia di S-02.
pub fn source_path(corpus_dir: &Path, doc: &CorpusDoc) -> Option<PathBuf> {
    match doc.format.as_str() {
        "pdf" => Some(corpus_dir.join("raw").join(format!("{}.pdf", doc.doc_id))),
        "md" => Some(
            corpus_dir
                .join("synthetic")
                .join(format!("{}.md", doc.doc_id)),
        ),
        _ => None,
    }
}

/// Testo del PDF con `pdftotext -layout` (pagine separate da form feed), in UTF-8.
pub fn pdftotext_layout(pdf: &Path) -> anyhow::Result<String> {
    let out = Command::new("pdftotext")
        .arg("-layout")
        .arg("-enc")
        .arg("UTF-8")
        .arg(pdf)
        .arg("-")
        .output()
        .with_context(|| format!("pdftotext non eseguibile su {}", pdf.display()))?;
    if !out.status.success() {
        bail!("pdftotext su {} uscito con {}", pdf.display(), out.status);
    }
    String::from_utf8(out.stdout).context("output di pdftotext non UTF-8")
}

/// Chunk di un documento del manifest; `None` se il formato non si indicizza.
pub fn chunk_document(corpus_dir: &Path, doc: &CorpusDoc) -> anyhow::Result<Option<Vec<Chunk>>> {
    let Some(path) = source_path(corpus_dir, doc) else {
        return Ok(None);
    };
    let lang = Lang::parse(&doc.lang)?;
    let chunks = if doc.format == "pdf" {
        chunk_pages(&doc.doc_id, lang, &pdftotext_layout(&path)?)
    } else {
        let md = std::fs::read_to_string(&path)
            .with_context(|| format!("{} illeggibile", path.display()))?;
        chunk_markdown(&doc.doc_id, lang, &md)
    };
    Ok(Some(chunks))
}

/// Chunk di tutto il corpus del manifest, in ordine di manifest (DOCX esclusi).
pub fn chunk_corpus(corpus_dir: &Path) -> anyhow::Result<Vec<Chunk>> {
    let mut out = Vec::new();
    for doc in read_manifest(corpus_dir)? {
        if let Some(chunks) = chunk_document(corpus_dir, &doc)? {
            out.extend(chunks);
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chars(s: &str) -> Vec<char> {
        s.chars().collect()
    }

    fn slice(s: &str, start: usize, end: usize) -> String {
        chars(s)[start..end].iter().collect()
    }

    /// Pagina 2 di esattamente 4.000 caratteri con «à» e «€» (byte != caratteri).
    fn long_page() -> String {
        let unit = "Ricavi à 12,5 € ";
        let mut s: String = unit.repeat(4_000 / unit.chars().count() + 1);
        s = s.chars().take(4_000).collect();
        assert_eq!(s.chars().count(), 4_000);
        assert!(s.len() > 4_000, "il testo deve avere caratteri multibyte");
        s
    }

    // TEST-10 (RISK-6): chunk mai a cavallo di pagina, offset in caratteri, sovrapposizione 200.
    #[test]
    fn chunk_never_crosses_page_or_section() {
        let p1 = "Prima pagina: utile netto 3,4 € milioni.";
        let p2 = long_page();
        let p3 = "Terza pagina à fine.";
        let text = format!("{p1}\u{c}{p2}\u{c}{p3}");
        let pages = [p1.to_string(), p2.clone(), p3.to_string()];

        let chunks = chunk_pages("doc-x", Lang::Italian, &text);
        assert!(!chunks.is_empty());

        for c in &chunks {
            let page = c.page.expect("ogni chunk PDF ha una pagina") as usize;
            assert!((1..=3).contains(&page), "pagina fuori intervallo: {page}");
            assert_eq!(c.doc_key, "doc-x");
            assert_eq!(c.lang, Lang::Italian);
            assert!(c.section_path.is_none());
            let page_text = &pages[page - 1];
            assert!(c.char_end <= page_text.chars().count());
            assert_eq!(
                c.body,
                slice(page_text, c.char_start, c.char_end),
                "body = pagina[char_start..char_end] in caratteri"
            );
            assert!(
                c.body.chars().count() <= CHUNK_CHARS,
                "chunk oltre 1.600 caratteri"
            );
            assert!(!c.body.contains('\u{c}'), "chunk a cavallo di due pagine");
        }

        // Pagine 1 e 3: un chunk ciascuna, intero.
        for (n, p) in [(1u32, p1), (3u32, p3)] {
            let of_page: Vec<_> = chunks.iter().filter(|c| c.page == Some(n)).collect();
            assert_eq!(of_page.len(), 1, "pagina {n}");
            assert_eq!(of_page[0].body, p);
            assert_eq!(
                (of_page[0].char_start, of_page[0].char_end),
                (0, p.chars().count())
            );
        }

        // Pagina 2: coperta tutta, consecutivi sovrapposti di esattamente 200 caratteri.
        let mut p2c: Vec<_> = chunks.iter().filter(|c| c.page == Some(2)).collect();
        p2c.sort_by_key(|c| c.char_start);
        assert!(p2c.len() >= 3, "4.000 caratteri richiedono almeno 3 chunk");
        assert_eq!(p2c.first().unwrap().char_start, 0);
        assert_eq!(p2c.last().unwrap().char_end, 4_000);
        for w in p2c.windows(2) {
            assert_eq!(
                w[0].char_end - w[1].char_start,
                OVERLAP_CHARS,
                "sovrapposizione 200"
            );
        }
    }

    // TEST-10 (RISK-6), parte Markdown: sezioni adiacenti separate, section_path annidato.
    #[test]
    fn chunk_markdown_sections_are_separate_and_nested() {
        let md = "## A\n\nIntro della sezione A.\n\n### B\n\nTesto di B con 5 €.\n\n### C\n\nTesto di C à.\n\n## D\n\nTesto di D.\n";
        let chunks = chunk_markdown("syn-x", Lang::Italian, md);
        assert!(!chunks.is_empty());

        for c in &chunks {
            assert!(c.page.is_none());
            assert!(c.section_path.is_some());
            assert_eq!(
                c.body,
                slice(md, c.char_start, c.char_end),
                "offset Markdown in caratteri sul documento"
            );
            assert!(c.body.chars().count() <= CHUNK_CHARS);
            let markers = [
                "Testo di B",
                "Testo di C",
                "Testo di D",
                "Intro della sezione A",
            ];
            let found = markers.iter().filter(|m| c.body.contains(*m)).count();
            assert!(found <= 1, "due sezioni nello stesso chunk: {:?}", c.body);
        }

        let path_of = |marker: &str| {
            chunks
                .iter()
                .find(|c| c.body.contains(marker))
                .and_then(|c| c.section_path.clone())
                .unwrap_or_else(|| panic!("nessun chunk con {marker}"))
        };
        assert_eq!(path_of("Intro della sezione A"), "A");
        assert_eq!(path_of("Testo di B"), "A > B");
        assert_eq!(path_of("Testo di C"), "A > C");
        assert_eq!(path_of("Testo di D"), "D");
    }

    // Id stabile: stesso chunk → stesso id; offset o documento diversi → id diversi.
    #[test]
    fn stable_id_is_deterministic_and_distinct() {
        let text = format!("{}\u{c}{}", long_page(), long_page());
        let a = chunk_pages("doc-x", Lang::Italian, &text);
        let b = chunk_pages("doc-x", Lang::Italian, &text);
        let ids_a: Vec<_> = a.iter().map(Chunk::stable_id).collect();
        let ids_b: Vec<_> = b.iter().map(Chunk::stable_id).collect();
        assert_eq!(ids_a, ids_b);
        let unique: std::collections::HashSet<_> = ids_a.iter().collect();
        assert_eq!(unique.len(), ids_a.len(), "id duplicati");
        let other = chunk_pages("doc-y", Lang::Italian, &text);
        assert_ne!(other[0].stable_id(), ids_a[0]);
    }

    // Il front matter YAML non si indicizza; l'H1 non entra nel percorso delle sezioni.
    #[test]
    fn markdown_front_matter_skipped_and_h1_is_title_only() {
        let md = "---\ndoc_id: x\n---\n\n# Titolo\n\nPremessa.\n\n## 1. Sezione\n\nCorpo.\n";
        let chunks = chunk_markdown("x", Lang::Italian, md);
        assert!(chunks.iter().all(|c| !c.body.contains("doc_id")));
        let pre = chunks.iter().find(|c| c.body.contains("Premessa")).unwrap();
        assert_eq!(pre.section_path.as_deref(), Some("Titolo"));
        let sec = chunks.iter().find(|c| c.body.contains("Corpo")).unwrap();
        assert_eq!(sec.section_path.as_deref(), Some("1. Sezione"));
    }
}
