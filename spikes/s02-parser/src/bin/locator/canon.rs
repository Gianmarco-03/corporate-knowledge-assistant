//! Testo canonico `cka-text-v1`, sezioni, locator `Text` e conversione degli offset
//! (piano 01-09, Task 1).
//!
//! STUB di `test-write`: tipi e firme minime perché i test rossi compilino; ogni funzione è
//! `todo!()`. L'implementazione è dell'esecutore. Interfaccia fissata dai test:
//! - `md::parse` e `docx::parse_document_xml` producono la stessa sequenza di [`Block`]
//!   (testo inline già canonico, NFC, senza `\r`); [`sections`] costruisce le sezioni;
//! - [`TextLocator`] porta sempre `projection`, `doc_sha256` e `section_text_sha256`
//!   (campi obbligatori, `deny_unknown_fields`); [`resolve`] li ricontrolla prima di
//!   restituire testo (RISK-001 di risk_audit_12);
//! - gli offset sono Unicode scalar value, semiaperti; [`to_utf16_range`] li converte in
//!   code unit UTF-16 per `String.prototype.slice`.
use serde::{Deserialize, Serialize};

/// Nome e versione della proiezione testuale. Una modifica delle regole è una nuova versione.
pub(crate) const PROJECTION: &str = "cka-text-v1";

/// Passo del percorso di sezione: titolo canonico e ordinale 1-based fra le sorelle con lo
/// stesso titolo sotto lo stesso genitore.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct SectionStep {
    pub(crate) title: String,
    pub(crate) ordinal: u32,
}

/// Intervallo semiaperto `[start, end)`; in [`TextLocator`] in Unicode scalar value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct CharRange {
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// Blocco prodotto dai lettori MD e DOCX, già in `cka-text-v1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Block {
    /// Titolo di livello 1..=6, testo inline canonico senza spazi esterni.
    Heading { level: u8, text: String },
    /// Paragrafo, voce di elenco, blocco di codice o tabella (righe `\n`, celle `\t`).
    Text(String),
}

/// Esito di un lettore: blocchi e costrutti non coperti.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Parsed {
    pub(crate) blocks: Vec<Block>,
    pub(crate) unsupported: Vec<String>,
}

/// Sezione: percorso dalla radice e testo canonico (blocchi uniti da `\n`, titolo escluso).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Section {
    pub(crate) path: Vec<SectionStep>,
    pub(crate) text: String,
}

/// Proiezione di un documento: sezioni in ordine di documento e costrutti non coperti.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub(crate) struct Projection {
    pub(crate) sections: Vec<Section>,
    pub(crate) unsupported: Vec<String>,
}

/// Locator `Text` legato alla versione del documento e della proiezione.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct TextLocator {
    pub(crate) projection: String,
    pub(crate) doc_sha256: String,
    pub(crate) section_path: Vec<SectionStep>,
    pub(crate) char_range: CharRange,
    pub(crate) section_text_sha256: String,
}

/// Rifiuto di [`resolve`]: mai testo restituito per un locator che non combacia.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ResolveError {
    /// `doc_sha256` del locator diverso da quello del documento.
    VersionMismatch,
    /// `projection` del locator diversa da [`PROJECTION`].
    ProjectionMismatch,
    /// Hash del testo canonico di sezione diverso: le regole di proiezione sono cambiate
    /// senza cambiare versione, o la sezione non è quella.
    SectionHashMismatch,
    /// Nessuna sezione con quel percorso.
    SectionNotFound,
    /// Intervallo fuori dal testo di sezione o con `start > end`.
    RangeOutOfBounds,
}

/// CRLF e CR → `\n`, poi NFC. Usato per portare un excerpt in `cka-text-v1`.
pub(crate) fn canon_text(_s: &str) -> String {
    todo!("01-09: canon_text")
}

/// Sha256 esadecimale minuscolo dei byte.
pub(crate) fn sha256_hex(_bytes: &[u8]) -> String {
    todo!("01-09: sha256_hex")
}

/// Sezioni dai blocchi: un titolo apre una sezione figlia dell'ultimo titolo di livello
/// minore; il testo è dei blocchi fino al titolo successivo di qualunque livello.
pub(crate) fn sections(_parsed: &Parsed) -> Projection {
    todo!("01-09: sections")
}

/// Tutte le occorrenze dell'excerpt (portato in `cka-text-v1`) nei testi di sezione.
pub(crate) fn locate(_p: &Projection, _doc_sha256: &str, _excerpt: &str) -> Vec<TextLocator> {
    todo!("01-09: locate")
}

/// Testo dell'intervallo, dopo i controlli di proiezione, versione e hash di sezione.
pub(crate) fn resolve(
    _loc: &TextLocator,
    _p: &Projection,
    _doc_sha256: &str,
) -> Result<String, ResolveError> {
    todo!("01-09: resolve")
}

/// Scalar `[start, end)` → code unit UTF-16; `None` se fuori dal testo o `start > end`.
pub(crate) fn to_utf16_range(_text: &str, _r: CharRange) -> Option<CharRange> {
    todo!("01-09: to_utf16_range")
}

/// Testo degli scalar `[start, end)`; `None` se fuori dal testo o `start > end`.
pub(crate) fn slice_scalars(_text: &str, _r: CharRange) -> Option<String> {
    todo!("01-09: slice_scalars")
}

#[cfg(test)]
mod tests {
    //! Test del design test_design_5: TEST-2 (percorso con ordinali), TEST-3 (offset contro
    //! la fixture a mano), TEST-4 (locator con proiezione, versione o hash non coincidenti,
    //! RISK-001), parte di TEST-1 e TEST-6; test (1), (2) e (4) del Task 1.
    use super::*;

    fn step(title: &str, ordinal: u32) -> SectionStep {
        SectionStep {
            title: title.to_owned(),
            ordinal,
        }
    }

    fn h(level: u8, text: &str) -> Block {
        Block::Heading {
            level,
            text: text.to_owned(),
        }
    }

    fn t(text: &str) -> Block {
        Block::Text(text.to_owned())
    }

    fn parsed(blocks: Vec<Block>) -> Parsed {
        Parsed {
            blocks,
            unsupported: vec![],
        }
    }

    fn range(start: usize, end: usize) -> CharRange {
        CharRange { start, end }
    }

    // ---- fixture a mano (TEST-3, TEST-6) ----------------------------------------------

    const OFFSET_FIXTURE: &str = include_str!("../../../locator-gen/offset_vectors.expected.json");

    struct Vector {
        case_id: String,
        text: String,
        scalar: CharRange,
        utf16: CharRange,
        expected: String,
    }

    fn pair(v: &serde_json::Value) -> CharRange {
        let a = v.as_array().expect("intervallo [s,e]");
        assert_eq!(a.len(), 2, "intervallo di due estremi");
        range(
            a[0].as_u64().unwrap() as usize,
            a[1].as_u64().unwrap() as usize,
        )
    }

    fn vectors() -> Vec<Vector> {
        let o: serde_json::Value = serde_json::from_str(OFFSET_FIXTURE).expect("fixture JSON");
        o["vectors"]
            .as_array()
            .expect("vectors")
            .iter()
            .map(|v| Vector {
                case_id: v["case_id"].as_str().unwrap().to_owned(),
                text: v["text"].as_str().unwrap().to_owned(),
                scalar: pair(&v["scalar_range"]),
                utf16: pair(&v["utf16_range"]),
                expected: v["expected"].as_str().unwrap().to_owned(),
            })
            .collect()
    }

    /// TEST-6: la fixture è quella a mano (classe, autore, casi obbligatori, derivazioni).
    #[test]
    fn offset_fixture_is_hand_written_with_required_cases() {
        let o: serde_json::Value = serde_json::from_str(OFFSET_FIXTURE).unwrap();
        assert_eq!(o["class"], "derivazione-documentata");
        assert_eq!(o["author"], "test-write");
        let vs = o["vectors"].as_array().unwrap();
        for need in [
            "bmp",
            "astral-before",
            "astral-inside",
            "combining",
            "cjk",
            "empty",
        ] {
            let v = vs
                .iter()
                .find(|v| v["case_id"] == need)
                .unwrap_or_else(|| panic!("caso {need} assente"));
            assert!(
                !v["derivation"].as_str().unwrap_or("").is_empty(),
                "derivation vuota in {need}"
            );
        }
        let mut ids: Vec<&str> = vs.iter().map(|v| v["case_id"].as_str().unwrap()).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), vs.len(), "case_id duplicati");
    }

    /// TEST-3: conversione scalar → UTF-16 uguale alla fixture, caso per caso.
    #[test]
    fn utf16_conversion_matches_hand_fixture() {
        for v in vectors() {
            assert_eq!(
                to_utf16_range(&v.text, v.scalar),
                Some(v.utf16),
                "utf16_range di {}",
                v.case_id
            );
        }
    }

    /// TEST-3: testo estratto per scalar uguale a `expected` della fixture.
    #[test]
    fn scalar_slice_matches_hand_fixture() {
        for v in vectors() {
            assert_eq!(
                slice_scalars(&v.text, v.scalar).as_deref(),
                Some(v.expected.as_str()),
                "expected di {}",
                v.case_id
            );
        }
    }

    /// TEST-3: l'intervallo UTF-16 della fixture, applicato alle code unit, dà lo stesso
    /// testo (semantica di `String.prototype.slice`), e la conversione non spezza mai una
    /// coppia surrogata.
    #[test]
    fn utf16_range_never_splits_a_surrogate_pair() {
        for v in vectors() {
            let got = to_utf16_range(&v.text, v.scalar).expect("intervallo valido");
            let units: Vec<u16> = v.text.encode_utf16().collect();
            let sliced = String::from_utf16(&units[got.start..got.end])
                .unwrap_or_else(|_| panic!("surrogato spezzato in {}", v.case_id));
            assert_eq!(sliced, v.expected, "slice UTF-16 di {}", v.case_id);
        }
    }

    /// TEST-3: fuori dal testo o con `start > end` non c'è conversione.
    #[test]
    fn out_of_range_offsets_are_rejected() {
        let text = "A\u{1F600}e";
        assert_eq!(to_utf16_range(text, range(0, 4)), None);
        assert_eq!(to_utf16_range(text, range(2, 1)), None);
        assert_eq!(slice_scalars(text, range(3, 4)), None);
        assert_eq!(slice_scalars(text, range(2, 1)), None);
        assert_eq!(to_utf16_range(text, range(3, 3)), Some(range(4, 4)));
    }

    // ---- testo canonico e hash ---------------------------------------------------------

    /// Definizione dell'hash: sha256 esadecimale minuscolo (valore di `sha256sum`).
    #[test]
    fn sha256_hex_is_lowercase_hex_of_bytes() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    /// CRLF e CR → `\n`, NFC; nessun ripiegamento tipografico.
    #[test]
    fn canon_text_normalizes_newlines_and_nfc_only() {
        assert_eq!(canon_text("a\r\nb\rc"), "a\nb\nc");
        assert_eq!(canon_text("perche\u{301}"), "perch\u{e9}");
        assert_eq!(canon_text("q\u{301}"), "q\u{301}");
        let typo = "\u{2019}\u{201C}x\u{201D} \u{2013} a\u{a0}b";
        assert_eq!(canon_text(typo), typo, "nessun ripiegamento tipografico");
    }

    // ---- sezioni e ordinali (TEST-2) ---------------------------------------------------

    /// Il titolo non fa parte del testo; le sottosezioni sono sezioni a sé.
    #[test]
    fn section_text_excludes_heading_and_subsections() {
        let p = sections(&parsed(vec![
            h(1, "Report"),
            t("Intro."),
            h(2, "Ricavi"),
            t("Uno."),
            t("Due."),
            h(3, "Dettaglio"),
            t("Tre."),
            h(2, "Costi"),
            t("Quattro."),
        ]));
        let got: Vec<(Vec<SectionStep>, String)> = p
            .sections
            .iter()
            .map(|s| (s.path.clone(), s.text.clone()))
            .collect();
        assert_eq!(
            got,
            vec![
                (vec![step("Report", 1)], "Intro.".to_owned()),
                (
                    vec![step("Report", 1), step("Ricavi", 1)],
                    "Uno.\nDue.".to_owned()
                ),
                (
                    vec![step("Report", 1), step("Ricavi", 1), step("Dettaglio", 1)],
                    "Tre.".to_owned()
                ),
                (
                    vec![step("Report", 1), step("Costi", 1)],
                    "Quattro.".to_owned()
                ),
            ]
        );
    }

    /// TEST-2: n sorelle omonime prendono ordinali 1..=n; lo stesso titolo sotto genitori
    /// diversi riparte da 1.
    #[test]
    fn homonymous_siblings_get_consecutive_ordinals() {
        for n in 1..=6u32 {
            let mut blocks = vec![h(1, "Doc")];
            for i in 0..n {
                blocks.push(h(2, "Risultati"));
                blocks.push(t(&format!("Blocco {i}.")));
            }
            let p = sections(&parsed(blocks));
            let ords: Vec<u32> = p
                .sections
                .iter()
                .filter(|s| s.path.len() == 2)
                .map(|s| s.path[1].ordinal)
                .collect();
            assert_eq!(ords, (1..=n).collect::<Vec<_>>(), "n = {n}");
        }
        let p = sections(&parsed(vec![
            h(1, "Doc"),
            h(2, "A"),
            h(3, "Note"),
            t("x."),
            h(2, "B"),
            h(3, "Note"),
            t("y."),
        ]));
        let notes: Vec<Vec<SectionStep>> = p
            .sections
            .iter()
            .filter(|s| s.path.last().is_some_and(|l| l.title == "Note"))
            .map(|s| s.path.clone())
            .collect();
        assert_eq!(
            notes,
            vec![
                vec![step("Doc", 1), step("A", 1), step("Note", 1)],
                vec![step("Doc", 1), step("B", 1), step("Note", 1)],
            ]
        );
    }

    /// TEST-2: inserire una sorella con titolo diverso non cambia gli ordinali delle altre.
    #[test]
    fn inserting_a_differently_titled_sibling_keeps_ordinals() {
        let before = sections(&parsed(vec![
            h(1, "Doc"),
            h(2, "Risultati"),
            t("a."),
            h(2, "Risultati"),
            t("b."),
        ]));
        let after = sections(&parsed(vec![
            h(1, "Doc"),
            h(2, "Risultati"),
            t("a."),
            h(2, "Premessa"),
            t("nuova."),
            h(2, "Risultati"),
            t("b."),
        ]));
        let path_of = |p: &Projection, text: &str| {
            p.sections
                .iter()
                .find(|s| s.text == text)
                .map(|s| s.path.clone())
                .unwrap()
        };
        assert_eq!(path_of(&before, "b."), path_of(&after, "b."));
        assert_eq!(
            path_of(&after, "b."),
            vec![step("Doc", 1), step("Risultati", 2)]
        );
    }

    // ---- locator e resolve (test (1), (2), TEST-4) -------------------------------------

    const SHA_V1: &str = "1111111111111111111111111111111111111111111111111111111111111111";
    const SHA_V2: &str = "2222222222222222222222222222222222222222222222222222222222222222";

    fn twin_sisters() -> Projection {
        sections(&parsed(vec![
            h(1, "Report"),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
        ]))
    }

    /// Test (1): due sorelle omonime con testo identico → due locator distinti per
    /// ordinale, ciascuno risolto sul proprio blocco.
    #[test]
    fn twin_sisters_yield_two_locators_distinct_by_ordinal() {
        let p = twin_sisters();
        let locs = locate(&p, SHA_V1, "Ricavi in crescita");
        assert_eq!(locs.len(), 2);
        let paths: Vec<Vec<SectionStep>> = locs.iter().map(|l| l.section_path.clone()).collect();
        assert_eq!(
            paths,
            vec![
                vec![step("Report", 1), step("Risultati", 1)],
                vec![step("Report", 1), step("Risultati", 2)],
            ]
        );
        for l in &locs {
            assert_eq!(l.char_range, range(0, 18));
            assert_eq!(l.projection, PROJECTION);
            assert_eq!(l.doc_sha256, SHA_V1);
            assert_eq!(resolve(l, &p, SHA_V1).as_deref(), Ok("Ricavi in crescita"));
        }
        // Ogni ordinale punta al proprio blocco: con testi diversi a parità di offset.
        let q = sections(&parsed(vec![
            h(1, "Report"),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
            h(2, "Risultati"),
            t("Ricavi in flessione."),
        ]));
        let second = q
            .sections
            .iter()
            .find(|s| s.path == vec![step("Report", 1), step("Risultati", 2)])
            .unwrap();
        let loc = TextLocator {
            projection: PROJECTION.to_owned(),
            doc_sha256: SHA_V1.to_owned(),
            section_path: second.path.clone(),
            char_range: range(10, 19),
            section_text_sha256: sha256_hex(second.text.as_bytes()),
        };
        assert_eq!(resolve(&loc, &q, SHA_V1).as_deref(), Ok("flessione"));
    }

    /// Test (2) e TEST-4: locator di v1 risolto su v2 (stesso testo, una sorella aggiunta) →
    /// `VersionMismatch`, mai testo.
    #[test]
    fn locator_from_v1_on_v2_is_version_mismatch() {
        let v1 = twin_sisters();
        let loc = locate(&v1, SHA_V1, "Ricavi in crescita")
            .into_iter()
            .next()
            .unwrap();
        let v2 = sections(&parsed(vec![
            h(1, "Report"),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
            h(2, "Risultati"),
            t("Ricavi in crescita."),
        ]));
        assert_eq!(
            resolve(&loc, &v2, SHA_V2),
            Err(ResolveError::VersionMismatch)
        );
    }

    /// TEST-4: proiezione diversa da `cka-text-v1` → rifiuto, anche con hash giusti.
    #[test]
    fn locator_with_other_projection_is_rejected() {
        let p = twin_sisters();
        let mut loc = locate(&p, SHA_V1, "Ricavi").into_iter().next().unwrap();
        loc.projection = "cka-text-v2".to_owned();
        assert_eq!(
            resolve(&loc, &p, SHA_V1),
            Err(ResolveError::ProjectionMismatch)
        );
    }

    /// TEST-4 / RISK-001 (migrazione negativa): una regola di normalizzazione cambiata senza
    /// cambiare `projection` (stesso documento, stesso nome di proiezione, testo di sezione
    /// diverso) fa fallire il round-trip invece di restituire testo.
    #[test]
    fn changed_normalization_without_new_projection_fails_roundtrip() {
        let old_rules = sections(&parsed(vec![h(1, "Doc"), t("Ricavi netti in crescita.")]));
        let loc = locate(&old_rules, SHA_V1, "netti in crescita")
            .into_iter()
            .next()
            .unwrap();
        // Stesse sorgenti, regola «spazi compressi» cambiata: il testo di sezione differisce.
        let new_rules = sections(&parsed(vec![h(1, "Doc"), t("Ricavi  netti in crescita.")]));
        assert_eq!(
            resolve(&loc, &new_rules, SHA_V1),
            Err(ResolveError::SectionHashMismatch)
        );
    }

    /// TEST-4: hash di sezione alterato nel locator → rifiuto.
    #[test]
    fn tampered_section_hash_is_rejected() {
        let p = twin_sisters();
        let mut loc = locate(&p, SHA_V1, "Ricavi").into_iter().next().unwrap();
        loc.section_text_sha256 = SHA_V2.to_owned();
        assert_eq!(
            resolve(&loc, &p, SHA_V1),
            Err(ResolveError::SectionHashMismatch)
        );
    }

    /// TEST-4: percorso inesistente o intervallo fuori sezione → rifiuto tipizzato.
    #[test]
    fn unknown_path_or_range_is_rejected() {
        let p = twin_sisters();
        let base = locate(&p, SHA_V1, "Ricavi").into_iter().next().unwrap();
        let mut missing = base.clone();
        missing.section_path = vec![step("Report", 1), step("Risultati", 3)];
        assert_eq!(
            resolve(&missing, &p, SHA_V1),
            Err(ResolveError::SectionNotFound)
        );
        let mut wide = base.clone();
        wide.char_range = range(0, 999);
        assert_eq!(
            resolve(&wide, &p, SHA_V1),
            Err(ResolveError::RangeOutOfBounds)
        );
        let mut inverted = base;
        inverted.char_range = range(5, 2);
        assert_eq!(
            resolve(&inverted, &p, SHA_V1),
            Err(ResolveError::RangeOutOfBounds)
        );
    }

    /// TEST-4 / RISK-001: `projection`, `doc_sha256` e `section_text_sha256` sono parte
    /// obbligatoria del locator serializzato; campi sconosciuti rifiutati.
    #[test]
    fn serialized_locator_requires_version_fields() {
        let full = serde_json::json!({
            "projection": "cka-text-v1",
            "doc_sha256": SHA_V1,
            "section_path": [{"title": "Report", "ordinal": 1}],
            "char_range": {"start": 0, "end": 6},
            "section_text_sha256": SHA_V2,
        });
        assert!(serde_json::from_value::<TextLocator>(full.clone()).is_ok());
        for drop in ["projection", "doc_sha256", "section_text_sha256"] {
            let mut v = full.clone();
            v.as_object_mut().unwrap().remove(drop);
            assert!(
                serde_json::from_value::<TextLocator>(v).is_err(),
                "{drop} deve essere obbligatorio"
            );
        }
        let mut extra = full;
        extra["page"] = serde_json::json!(3);
        assert!(serde_json::from_value::<TextLocator>(extra).is_err());
    }

    /// Il locator calcolato porta l'hash del testo canonico della propria sezione.
    #[test]
    fn locator_carries_section_text_hash() {
        let p = sections(&parsed(vec![h(1, "Doc"), t("Testo.")]));
        let loc = locate(&p, SHA_V1, "Testo").into_iter().next().unwrap();
        assert_eq!(
            loc.section_text_sha256,
            "f9b350e0303ee5eef6a371dfcb3994193be99614fd02308c8098a1a4be133870"
        );
    }

    /// L'excerpt è portato in `cka-text-v1` prima della ricerca (CRLF, NFC).
    #[test]
    fn excerpt_is_canonicalized_before_search() {
        let p = sections(&parsed(vec![h(1, "Doc"), t("Uno.\nperch\u{e9} no")]));
        let locs = locate(&p, SHA_V1, "Uno.\r\nperche\u{301}");
        assert_eq!(locs.len(), 1);
        assert_eq!(locs[0].char_range, range(0, 11));
    }

    /// Nessuna occorrenza → nessun locator (mai un locator approssimato).
    #[test]
    fn absent_excerpt_yields_no_locator() {
        let p = twin_sisters();
        assert!(locate(&p, SHA_V1, "Ricavi in calo").is_empty());
    }
}
