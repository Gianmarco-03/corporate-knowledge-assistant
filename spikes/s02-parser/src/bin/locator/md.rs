//! Lettore Markdown → blocchi `cka-text-v1` (piano 01-09, Task 1).
//!
//! Si esegue solo nel figlio `locator locate` (job `op: locate`). Lettore a righe del
//! sottoinsieme di Markdown dei documenti sintetici (ATX, paragrafi, elenchi, codice
//! recintato, tabelle a pipe, note); ciò che non copre finisce in `unsupported`.
use unicode_normalization::UnicodeNormalization;

use crate::canon::{Block, Parsed};

/// Blocchi canonici di un Markdown: front matter, `\newpage`, immagini e note esclusi; HTML
/// grezzo in `unsupported`; inline senza marcatori di enfasi, link ridotti al testo.
pub(crate) fn parse(text: &str) -> Parsed {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    let mut lines: Vec<&str> = text.split('\n').collect();
    let body_start = front_matter_end(&lines);
    lines.drain(..body_start);

    let mut r = Reader::default();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();
        if let Some(fence) = fence_of(line) {
            r.flush();
            let mut code: Vec<&str> = Vec::new();
            i += 1;
            while i < lines.len() && !closes_fence(lines[i], fence) {
                code.push(lines[i]);
                i += 1;
            }
            r.push_raw(code.join("\n"));
            i += 1;
            continue;
        }
        // Riga vuota o `\newpage` (interruzione di pagina manuale): chiudono il blocco.
        if trimmed.is_empty() || trimmed == "\\newpage" {
            r.flush();
        } else if let Some((level, title)) = atx(line) {
            r.flush();
            r.blocks.push(Block::Heading {
                level,
                text: finish_inline(title),
            });
        } else if is_footnote_def(trimmed) {
            r.flush();
            // La definizione e le sue righe rientrate di continuazione restano fuori.
            i += 1;
            while i < lines.len()
                && lines[i].starts_with([' ', '\t'])
                && !lines[i].trim().is_empty()
            {
                i += 1;
            }
            continue;
        } else if is_html_block(trimmed) {
            r.flush();
            r.unsupported.push("html_block".to_owned());
            while i < lines.len() && !lines[i].trim().is_empty() {
                i += 1;
            }
            continue;
        } else if trimmed.starts_with('|') {
            r.flush();
            let mut rows: Vec<String> = Vec::new();
            while i < lines.len() && lines[i].trim().starts_with('|') {
                let cells = split_row(lines[i].trim());
                if !is_separator(&cells) {
                    rows.push(
                        cells
                            .iter()
                            .map(|c| finish_inline(c))
                            .collect::<Vec<_>>()
                            .join("\t"),
                    );
                }
                i += 1;
            }
            r.push_raw(rows.join("\n"));
            continue;
        } else if is_thematic_break(trimmed) {
            r.flush();
            r.unsupported.push("thematic_break".to_owned());
        } else if let Some(item) = list_item(line) {
            r.flush();
            r.para.push(item);
        } else if let Some(quoted) = trimmed.strip_prefix('>') {
            r.unsupported.push("blockquote".to_owned());
            r.para.push(quoted);
        } else {
            r.para.push(line);
        }
        i += 1;
    }
    r.flush();
    let mut unsupported = r.unsupported;
    unsupported.extend(r.inline_unsupported);
    unsupported.sort();
    unsupported.dedup();
    Parsed {
        blocks: r.blocks,
        unsupported,
    }
}

#[derive(Default)]
struct Reader<'a> {
    blocks: Vec<Block>,
    para: Vec<&'a str>,
    unsupported: Vec<String>,
    inline_unsupported: Vec<String>,
}

impl<'a> Reader<'a> {
    /// Chiude il paragrafo (o la voce di elenco) in corso: righe unite da uno spazio.
    fn flush(&mut self) {
        if self.para.is_empty() {
            return;
        }
        let joined = self
            .para
            .drain(..)
            .map(str::trim)
            .collect::<Vec<_>>()
            .join(" ");
        let (text, html) = inline(&joined);
        if html {
            self.inline_unsupported.push("html_inline".to_owned());
        }
        let text = compress(&text);
        if !text.is_empty() {
            self.blocks.push(Block::Text(text.nfc().collect()));
        }
    }

    /// Blocco già composto (codice o tabella): solo NFC.
    fn push_raw(&mut self, text: String) {
        if !text.is_empty() {
            self.blocks.push(Block::Text(text.nfc().collect()));
        }
    }
}

/// Indice della prima riga dopo il front matter YAML (`---` … `---` o `...` in testa).
fn front_matter_end(lines: &[&str]) -> usize {
    if lines.first().map(|l| l.trim_end()) != Some("---") {
        return 0;
    }
    lines
        .iter()
        .enumerate()
        .skip(1)
        .find(|(_, l)| matches!(l.trim_end(), "---" | "..."))
        .map_or(0, |(i, _)| i + 1)
}

/// Recinto di apertura di un blocco di codice (``` o ~~~, almeno 3), con al più 3 spazi.
fn fence_of(line: &str) -> Option<(char, usize)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let c = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let n = rest.chars().take_while(|x| *x == c).count();
    (n >= 3).then_some((c, n))
}

fn closes_fence(line: &str, (c, n): (char, usize)) -> bool {
    let t = line.trim();
    t.chars().all(|x| x == c) && t.chars().count() >= n
}

/// Titolo ATX: `#`..`######` seguito da spazio o fine riga, con al più 3 spazi davanti;
/// la sequenza di chiusura `#` facoltativa si toglie.
fn atx(line: &str) -> Option<(u8, &str)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let n = rest.bytes().take_while(|b| *b == b'#').count();
    if !(1..=6).contains(&n) {
        return None;
    }
    let after = &rest[n..];
    if !(after.is_empty() || after.starts_with([' ', '\t'])) {
        return None;
    }
    let mut title = after.trim();
    let closing = title.trim_end_matches('#');
    if closing.is_empty() || closing.ends_with([' ', '\t']) {
        title = closing.trim_end();
    }
    Some((n as u8, title))
}

fn is_footnote_def(t: &str) -> bool {
    t.starts_with("[^") && t.find("]:").is_some_and(|i| i > 2)
}

/// Riga che apre un blocco HTML grezzo: `<` seguito da lettera, `/`, `!` o `?`.
fn is_html_block(t: &str) -> bool {
    let mut c = t.chars();
    c.next() == Some('<')
        && c.next()
            .is_some_and(|x| x.is_ascii_alphabetic() || matches!(x, '/' | '!' | '?'))
}

fn is_thematic_break(t: &str) -> bool {
    let compact: String = t.chars().filter(|c| !c.is_whitespace()).collect();
    compact.len() >= 3
        && ['-', '*', '_']
            .iter()
            .any(|m| compact.chars().all(|c| c == *m))
}

/// Testo di una voce di elenco (`-`, `*`, `+`, `N.`, `N)` seguiti da spazio), marcatore tolto.
fn list_item(line: &str) -> Option<&str> {
    let t = line.trim_start();
    if let Some(rest) = t.strip_prefix(['-', '*', '+'])
        && rest.starts_with([' ', '\t'])
    {
        return Some(rest.trim_start());
    }
    let digits = t.bytes().take_while(u8::is_ascii_digit).count();
    if (1..=9).contains(&digits) {
        let rest = &t[digits..];
        if let Some(after) = rest.strip_prefix(['.', ')'])
            && after.starts_with([' ', '\t'])
        {
            return Some(after.trim_start());
        }
    }
    None
}

/// Celle di una riga di tabella: pipe esterne tolte, `\|` non separa.
fn split_row(t: &str) -> Vec<String> {
    let inner = t.strip_prefix('|').unwrap_or(t);
    let inner = match inner.strip_suffix('|') {
        Some(s) if !s.ends_with('\\') => s,
        _ => inner,
    };
    let mut cells = Vec::new();
    let mut cur = String::new();
    let mut chars = inner.chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => {
                cur.push(c);
                if let Some(n) = chars.next() {
                    cur.push(n);
                }
            }
            '|' => cells.push(std::mem::take(&mut cur)),
            _ => cur.push(c),
        }
    }
    cells.push(cur);
    cells
}

/// Riga separatrice `|---|:--:|` (ogni cella di soli `-` e `:`, almeno un `-`).
fn is_separator(cells: &[String]) -> bool {
    cells.iter().all(|c| {
        let t = c.trim();
        !t.is_empty() && t.contains('-') && t.chars().all(|x| x == '-' || x == ':')
    })
}

/// Inline canonico, spazi compressi ed estremi tolti, NFC (titoli e celle).
fn finish_inline(s: &str) -> String {
    compress(&inline(s).0).nfc().collect()
}

/// Sequenze di spazi ASCII compresse in uno, estremi tolti.
fn compress(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_space = false;
    for c in s.chars() {
        if c == ' ' {
            if !prev_space {
                out.push(' ');
            }
            prev_space = true;
        } else {
            out.push(c);
            prev_space = false;
        }
    }
    out.trim_matches(' ').to_owned()
}

/// Inline di `cka-text-v1`: enfasi e backtick tolti, link ridotti al testo, immagini e
/// riferimenti a note tolti, escape risolti, tag HTML inline tolti. Il booleano dice se
/// c'era HTML inline (registrato come `unsupported`).
fn inline(s: &str) -> (String, bool) {
    let chars: Vec<char> = s.chars().collect();
    let mut out = String::with_capacity(s.len());
    let mut html = false;
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\\' if chars.get(i + 1).is_some_and(char::is_ascii_punctuation) => {
                out.push(chars[i + 1]);
                i += 2;
            }
            '`' => {
                let n = run_len(&chars, i, '`');
                match find_run(&chars, i + n, '`', n) {
                    Some(close) => {
                        out.extend(&chars[i + n..close]);
                        i = close + n;
                    }
                    None => i += n,
                }
            }
            '!' if chars.get(i + 1) == Some(&'[') => match link_end(&chars, i + 1) {
                Some((_, end)) => i = end,
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '[' if chars.get(i + 1) == Some(&'^') => {
                match chars[i..].iter().position(|x| *x == ']') {
                    Some(p) => i += p + 1,
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            '[' => match link_end(&chars, i) {
                Some((label_end, end)) => {
                    let label: String = chars[i + 1..label_end].iter().collect();
                    let (t, h) = inline(&label);
                    html |= h;
                    out.push_str(&t);
                    i = end;
                }
                None => {
                    out.push(c);
                    i += 1;
                }
            },
            '*' => i += 1,
            '_' => {
                let n = run_len(&chars, i, '_');
                let before = i.checked_sub(1).map(|p| chars[p]);
                let after = chars.get(i + n).copied();
                let intraword = before.is_some_and(char::is_alphanumeric)
                    && after.is_some_and(char::is_alphanumeric);
                if intraword {
                    out.extend(&chars[i..i + n]);
                }
                i += n;
            }
            '~' if chars.get(i + 1) == Some(&'~') => i += 2,
            '<' if chars
                .get(i + 1)
                .is_some_and(|x| x.is_ascii_alphabetic() || *x == '/' || *x == '!') =>
            {
                match chars[i..].iter().position(|x| *x == '>') {
                    Some(p) => {
                        html = true;
                        i += p + 1;
                    }
                    None => {
                        out.push(c);
                        i += 1;
                    }
                }
            }
            _ => {
                out.push(c);
                i += 1;
            }
        }
    }
    (out, html)
}

fn run_len(chars: &[char], i: usize, c: char) -> usize {
    chars[i..].iter().take_while(|x| **x == c).count()
}

/// Prima sequenza di esattamente `n` caratteri `c` da `from`.
fn find_run(chars: &[char], from: usize, c: char, n: usize) -> Option<usize> {
    let mut j = from;
    while j < chars.len() {
        if chars[j] == c {
            let m = run_len(chars, j, c);
            if m == n {
                return Some(j);
            }
            j += m;
        } else {
            j += 1;
        }
    }
    None
}

/// Link `[testo](destinazione)` che parte da `open` (`[`): fine dell'etichetta e indice
/// dopo la `)` di chiusura, con parentesi bilanciate nella destinazione.
fn link_end(chars: &[char], open: usize) -> Option<(usize, usize)> {
    let mut depth = 0usize;
    let mut j = open;
    let label_end = loop {
        match chars.get(j)? {
            '\\' => j += 1,
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    break j;
                }
            }
            _ => {}
        }
        j += 1;
    };
    if chars.get(label_end + 1) != Some(&'(') {
        return None;
    }
    let mut parens = 0usize;
    let mut k = label_end + 1;
    loop {
        match chars.get(k)? {
            '\\' => k += 1,
            '(' => parens += 1,
            ')' => {
                parens -= 1;
                if parens == 0 {
                    return Some((label_end, k + 1));
                }
            }
            _ => {}
        }
        k += 1;
    }
}

#[cfg(test)]
pub(crate) mod tests {
    //! Test (3) lato MD, (4), (5) del Task 1; TEST-7 lato Markdown (link e HTML grezzo non
    //! risolti); TEST-1 sulla proiezione stabile.
    use super::*;
    use crate::canon::{self, Block, CharRange, SectionStep};

    fn step(title: &str, ordinal: u32) -> SectionStep {
        SectionStep {
            title: title.to_owned(),
            ordinal,
        }
    }

    fn texts(md: &str) -> Vec<(Vec<SectionStep>, String)> {
        canon::sections(&parse(md))
            .sections
            .into_iter()
            .map(|s| (s.path, s.text))
            .collect()
    }

    /// Documento MD gemello di `docx::tests::TWIN_DOCUMENT_XML` (test (3)).
    pub(crate) const TWIN_MD: &str = "---\n\
doc_id: twin\n\
title: Non entra\n\
---\n\
\n\
# Relazione\n\
\n\
## Risultati **chiave** del [periodo](https://attacker.invalid/x)\n\
\n\
Il fatturato \u{e8} cresciuto del 5%.\n\
\n\
\\newpage\n\
\n\
| Voce | 2025 | 2026 |\n\
|---|---:|---:|\n\
| Ricavi | 182,4 | 196,0 |\n\
| EBITDA | 27,1 | 30,4 |\n";

    /// Testo canonico atteso della sezione «Risultati chiave del periodo», scritto a mano.
    pub(crate) const TWIN_SECTION_TEXT: &str = "Il fatturato \u{e8} cresciuto del 5%.\nVoce\t2025\t2026\nRicavi\t182,4\t196,0\nEBITDA\t27,1\t30,4";

    /// Test (3) lato MD: titolo con enfasi e link, `\newpage` escluso, tabella multi-cella.
    #[test]
    fn twin_markdown_has_hand_written_canonical_text() {
        assert_eq!(
            texts(TWIN_MD),
            vec![
                (vec![step("Relazione", 1)], String::new()),
                (
                    vec![
                        step("Relazione", 1),
                        step("Risultati chiave del periodo", 1)
                    ],
                    TWIN_SECTION_TEXT.to_owned()
                ),
            ]
        );
    }

    /// Test (4): `A😀e` + `q\u{301}` + CRLF + forma decomposta → `char_range` scritto a mano
    /// (A0 😀1 e2 ␠3 q4 ◌́5 x6 ␠7 p8 e9 r10 c11 h12 é13 ␠14 f15 i16 n17 e18), riestratto
    /// identico.
    #[test]
    fn astral_combining_and_crlf_offsets_are_hand_checked() {
        let md = "# Titolo\r\n\r\nA\u{1F600}e\r\nq\u{301}x perche\u{301} fine\r\n";
        let p = canon::sections(&parse(md));
        assert_eq!(p.sections.len(), 1);
        let text = &p.sections[0].text;
        assert_eq!(text, "A\u{1F600}e q\u{301}x perch\u{e9} fine");
        assert!(!text.contains('\r'));
        let sha = "3333333333333333333333333333333333333333333333333333333333333333";
        for (excerpt, start, end, again) in [
            ("q\u{301}x", 4, 7, "q\u{301}x"),
            ("perche\u{301}", 8, 14, "perch\u{e9}"),
            ("A\u{1F600}e q", 0, 5, "A\u{1F600}e q"),
        ] {
            let locs = canon::locate(&p, sha, excerpt);
            assert_eq!(locs.len(), 1, "{excerpt:?}");
            assert_eq!(locs[0].char_range, CharRange { start, end }, "{excerpt:?}");
            assert_eq!(canon::resolve(&locs[0], &p, sha).as_deref(), Ok(again));
        }
    }

    /// Test (5): il front matter YAML non entra nel testo.
    #[test]
    fn front_matter_is_excluded() {
        let md = "---\ndoc_id: x\ntitle: Segreto\n---\n\n# A\n\nTesto.\n";
        assert_eq!(texts(md), vec![(vec![step("A", 1)], "Testo.".to_owned())]);
    }

    /// Paragrafi: righe unite da uno spazio, spazi ASCII compressi, estremi rimossi.
    #[test]
    fn paragraph_lines_join_with_single_space() {
        let md = "# A\n\n  Prima   riga\nseconda    riga  \n\nAltro.\n";
        assert_eq!(
            texts(md),
            vec![(
                vec![step("A", 1)],
                "Prima riga seconda riga\nAltro.".to_owned()
            )]
        );
    }

    /// Elenchi, codice, escape ed enfasi secondo `cka-text-v1`.
    #[test]
    fn lists_code_escapes_and_emphasis() {
        let md = "# A\n\n- uno\n* due\n+ tre\n1. quattro\n\n```\n# non titolo\n  rientro\n```\n\n\\*letterale\\* __forte__ _corsivo_ ~~barrato~~ `codice`\n";
        let p = canon::sections(&parse(md));
        assert_eq!(p.sections.len(), 1, "il # nel codice non è un titolo");
        assert_eq!(
            p.sections[0].text,
            "uno\ndue\ntre\nquattro\n# non titolo\n  rientro\n*letterale* forte corsivo barrato codice"
        );
    }

    /// Titoli ATX: livello dal numero di `#`, testo inline canonico.
    #[test]
    fn atx_headings_are_canonical_blocks() {
        let p = parse("# Uno\n\n### Tre **b**\n\n###### Sei\n");
        assert_eq!(
            p.blocks,
            vec![
                Block::Heading {
                    level: 1,
                    text: "Uno".to_owned()
                },
                Block::Heading {
                    level: 3,
                    text: "Tre b".to_owned()
                },
                Block::Heading {
                    level: 6,
                    text: "Sei".to_owned()
                },
            ]
        );
    }

    /// TEST-7 lato MD: URL dei link e HTML grezzo non entrano nel testo; l'HTML è
    /// registrato in `unsupported`.
    #[test]
    fn links_and_raw_html_are_not_resolved() {
        let md = "# A\n\nVedi [la nota](javascript:alert(1)) e ![logo](https://attacker.invalid/p.png).\n\n<script>fetch('https://attacker.invalid')</script>\n\nFine.\n";
        let parsed = parse(md);
        let p = canon::sections(&parsed);
        let all: String = p.sections.iter().map(|s| s.text.as_str()).collect();
        assert!(!all.contains("javascript"), "{all:?}");
        assert!(!all.contains("attacker"), "{all:?}");
        assert!(!all.contains("script"), "{all:?}");
        assert!(all.contains("Vedi la nota e"), "{all:?}");
        assert!(all.contains("Fine."), "{all:?}");
        assert!(!parsed.unsupported.is_empty(), "HTML grezzo non registrato");
    }

    /// Note a piè di pagina (riferimenti e definizioni) escluse.
    #[test]
    fn footnotes_are_excluded() {
        let md = "# A\n\nTesto[^1] finale.\n\n[^1]: Nota nascosta.\n";
        assert_eq!(
            texts(md),
            vec![(vec![step("A", 1)], "Testo finale.".to_owned())]
        );
    }
}
