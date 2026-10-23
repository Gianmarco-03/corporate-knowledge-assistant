---
phase: 01-spike-tecnici
plan: 09
subsystem: spike-locator-citazione
tags: [s02-parser, locator, citazioni, cka-text-v1, section-path, ordinale, unicode-scalar, utf-16, nfc, sha2, docx, quick-xml, zip, markdown, pandoc, pdfium, pdfium-render, rotate, cropbox, mediabox, viewport, pdf-js, job-locate, adr-0005]

# Dependency graph
requires:
  - phase: 01-03
    provides: "Markdown sintetici syn-0N e i quattro DOCX derivati con pandoc (syn-01, syn-02, syn-05, syn-09), con sha256 nel manifest data/corpus/manifest.json"
  - phase: 01-07
    provides: "contratto W17: job JSON op locate → argv locator locate --input --excerpt-file, out_dir null, radici ammesse /data, /results/raw, /work; prescan::check_docx; protocol::emit e ChildOutcome; parse-parent batch; immagine worker"
  - phase: 01-08
    provides: "PDFium chromium/8100 nel worker e convenzione dei bounds in spazio utente prima di /Rotate (Frame di extract/pdf.rs), ipotesi verificata qui sui tracciati"
provides:
  - "Testo canonico cka-text-v1 (canon.rs, md.rs, docx.rs): Markdown e DOCX danno la stessa sequenza di blocchi; sezioni con percorso {title, ordinal} e testo canonico"
  - "Locator Text versionato: projection + doc_sha256 + section_path + char_range in Unicode scalar semiaperto + section_text_sha256; resolve rifiuta prima di dare testo (ProjectionMismatch, VersionMismatch, SectionNotFound, SectionHashMismatch, RangeOutOfBounds)"
  - "Conversione scalar → UTF-16 per il viewer JavaScript (to_utf16_range), verificata contro una fixture a mano e contro Node"
  - "Funzione pura pdfrect::to_viewport: spazio utente PDF → viewport non scalata, come convertToViewportRectangle di PDF.js a scala 1"
  - "Bin locator: locate (solo figlio, job op: locate), roundtrip prepare/check, rects in tre stadi, offsets con soli campi got_*"
  - "Evidenze in spikes/s02-parser/results/locator/, RISULTATI-locator.md ed ADR-0005 Proposed nella forma multi-span di D-10"
affects: [01-16, 01-17, S-07, crates/contracts, porta DocumentParser, viewer PDF e testo (VIEW-01, VIEW-02), fase 3 provider di ingestion e indice]

# Tech tracking
tech-stack:
  added: ["sha2 (workspace, =0.11.1) come dipendenza di s02-parser: già nel workspace e nel lock, nessun download"]
  patterns:
    - "Fixture con valori attesi scritte a mano da test-write e committate prima del bin e dei risultati (6175711), lette dai test con include_str!; il bin sotto test non produce mai valori attesi"
    - "Proiezione testuale con nome e versione dentro il locator; una regola cambiata senza nuova versione fa fallire resolve (SectionHashMismatch), non aggiorna in silenzio"
    - "Locate esegue ricerca e risoluzione su due letture e due proiezioni distinte dello stesso file"
    - "Misure DOCX/MD solo via job op: locate del lanciatore di 01-07; i bin normali compongono job e leggono RunReport senza aprire documenti"

key-files:
  created:
    - spikes/s02-parser/src/bin/locator/main.rs
    - spikes/s02-parser/src/bin/locator/canon.rs
    - spikes/s02-parser/src/bin/locator/docx.rs
    - spikes/s02-parser/src/bin/locator/md.rs
    - spikes/s02-parser/src/bin/locator/pdfrect.rs
    - spikes/s02-parser/locator-gen/gen_pages.py
    - spikes/s02-parser/locator-gen/check_offsets.mjs
    - spikes/s02-parser/locator-gen/expected_samples.json
    - spikes/s02-parser/locator-gen/offset_vectors.expected.json
    - spikes/s02-parser/locator-gen/rect_cases.expected.json
    - spikes/s02-parser/results/locator/hw.json
    - spikes/s02-parser/results/locator/locator_samples.json
    - spikes/s02-parser/results/locator/roundtrip.csv
    - spikes/s02-parser/results/locator/rect_transform.csv
    - spikes/s02-parser/results/locator/offset_vectors.json
    - spikes/s02-parser/RISULTATI-locator.md
    - docs/adr/0005-locator-citazione.md
  modified:
    - spikes/s02-parser/Cargo.toml
    - spikes/Cargo.lock

key-decisions:
  - "ADR-0005 Proposed: forma decisa multi-span con al più due span (Storia di D-10, 2026-10-21); lo spike misura il caso a span singolo; la consecutività degli span non è misurata e va ai test della fase dei contratti"
  - "L-4 fallisce 8/11 e resta dichiarato: pandoc porta U+0027 in U+2019 nei DOCX; cka-text-v1 non ripiega la tipografia per scelta; la decisione regge perché un locator si risolve sempre sul proprio file (doc_sha256)"
  - "Offset in Unicode scalar value semiaperti sul testo canonico; conversione UTF-16 lato viewer"
  - "Rettangoli PDF salvati nello spazio utente prima di /Rotate e trasformati lato sistema: i bounds di PDFium (tracciato e char box) sono già in quello spazio, 8/8"
  - "Testo prima del primo titolo = sezione con percorso vuoto (nessun caso nel corpus)"

patterns-established:
  - "Un valore atteso numerico di uno spike ha una sola fonte, una fixture a mano con campo derivation, committata prima del codice misurato"
  - "Un esito sotto soglia si riporta col numero del contratto e con la causa; non si normalizza via la differenza che lo produce"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Il locator di citazione ha due varianti: PDF = pagina + rettangoli; DOCX/Markdown = percorso di sezione (titolo + ordinale fra sorelle omonime) + intervallo di caratteri sul testo canonico cka-text-v1, senza pagine per DOCX/MD (D-10; criterio 2)"
    verification:
      - {test: TEST-2, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/canon.rs:507#homonymous_siblings_get_consecutive_ordinals ; :549#inserting_a_differently_titled_sibling_keeps_ordinals ; :598#twin_sisters_yield_two_locators_distinct_by_ordinal (test (1))", status: pass, evidence: "rieseguito dal bridge sull'host: cargo test -p s02-parser --bin locator --locked --offline, 68/68 verdi; gate dell'integratore nel runner verde"}
      - {test: "definizione registrata", ref: "docs/adr/0005-locator-citazione.md#Decisione ; spikes/s02-parser/RISULTATI-locator.md:15-30#Definizioni", status: pass, evidence: "letto dal bridge: Decisione con le due varianti, cka-text-v1, ordinale, offset, legame alla versione, conversione UTF-16, rettangoli; Definizioni committate in 3292022 prima dei risultati (0582280)"}
      - {test: "ratifica di ADR-0005", ref: "docs/adr/0005-locator-citazione.md:3", status: human_judgment, human_judgment: true, rationale: "Stato Proposed; la forma multi-span è una decisione dell'utente (Storia di D-10) ma la ratifica dell'ADR spetta al consolidamento 01-17; la validazione della consecutività dei due span non è stata misurata"}
  - must_have: "Il round-trip excerpt → locator → testo riestratto è esatto su almeno 10 campioni DOCX/MD; lo stesso documento in MD e DOCX produce lo stesso percorso di sezione, lo stesso intervallo e lo stesso hash del testo canonico di sezione (D-10)"
    verification:
      - {test: TEST-1, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/docx.rs:429#md_and_docx_twins_agree_on_path_range_hash_and_projection ; :409#twin_docx_has_hand_written_canonical_text ; :456#title_subtitle_author_date_are_excluded ; :470 ; :498 ; :580 ; spikes/s02-parser/src/bin/locator/md.rs:492#twin_markdown_has_hand_written_canonical_text ; :534#front_matter_is_excluded ; :541 ; :554 ; :566 ; :605 ; spikes/s02-parser/src/bin/locator/main.rs:1334#locate_markdown_payload_is_minimal_and_exact ; :1384#locate_docx_twin_finds_the_same_locator ; spikes/s02-parser/src/bin/locator/canon.rs:446 ; :455 ; :467 ; :757 ; :768 ; :777", status: pass, evidence: "68/68 verdi sull'host (bridge) e nel runner (gate); i blocchi #[cfg(test)] dei cinque file hanno sha256 identico fra i test rossi 836ed68 e c5e9db6 (ricalcolato dal bridge)"}
      - {test: "L-1, L-2, L-3, L-5", ref: "spikes/s02-parser/results/locator/locator_samples.json ; roundtrip.csv", status: pass, evidence: "ricalcolato dal bridge: 23 righe, 12 campioni; equal 23/23, match_count 1 su 23, doc_sha256 = manifest 23/23, section_path = atteso 23/23; 11 coppie MD/DOCX: percorso 11/11, intervallo 11/11"}
      - {test: "L-4", ref: "spikes/s02-parser/results/locator/locator_samples.json ; RISULTATI-locator.md:80", status: human_judgment, human_judgment: true, rationale: "soglia 100% non raggiunta: hash di sezione uguale in 8/11 coppie; divergono s01-scenario, s02-governance, s02-razionale (ricalcolato dal bridge). Causa dichiarata: apostrofi U+0027 → U+2019 di pandoc nel DOCX, con percorso e intervallo uguali. Se l'argomento «un locator si risolve sul proprio file» basta a tenere la decisione lo ratifica l'utente in 01-17"}
  - must_have: "Gli offset sono Unicode scalar value, intervallo semiaperto, sul testo canonico NFC; la conversione in code unit UTF-16 per il viewer JavaScript è specificata e verificata contro una fixture di vettori scritta a mano (con caratteri fuori BMP) prima di ogni misura, mai contro l'output del bin sotto test"
    verification:
      - {test: TEST-3, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/canon.rs:393#utf16_conversion_matches_hand_fixture ; :406#scalar_slice_matches_hand_fixture ; :421#utf16_range_never_splits_a_surrogate_pair ; :433#out_of_range_offsets_are_rejected ; spikes/s02-parser/src/bin/locator/md.rs:512#astral_combining_and_crlf_offsets_are_hand_checked (test (4))", status: pass, evidence: "68/68 verdi (bridge, host)"}
      - {test: "L-6", ref: "node spikes/s02-parser/locator-gen/check_offsets.mjs --fixture spikes/s02-parser/locator-gen/offset_vectors.expected.json --got spikes/s02-parser/results/locator/offset_vectors.json", status: pass, evidence: "rieseguito dal bridge: exit 0, «OK 10/10»; ricalcolo indipendente: fixture coerente con Array.from e String.prototype.slice 10/10, bin uguale alla fixture 10/10, nessuna riga del bin con utf16_range o expected"}
  - must_have: "La trasformazione dei rettangoli PDF (pagine ruotate, CropBox diverso da MediaBox) è verificata al 100% su rettangoli numerici i cui valori attesi sono derivati a mano in una sola fixture, separatamente dall'estrazione della bbox con PDFium"
    verification:
      - {test: TEST-5, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/pdfrect.rs:189-224#rot{0,90,180,270}_cropbox_{equal,shifted} (8 test) ; :231#transform_preserves_size_and_stays_in_viewport ; :262#rotate_is_normalized_modulo_360 ; :277#inverted_input_rect_is_normalized", status: pass, evidence: "68/68 verdi (bridge, host); i test leggono la fixture con include_str! (pdfrect.rs:88) e non hanno costanti attese proprie"}
      - {test: "L-7, L-8", ref: "spikes/s02-parser/results/locator/rect_transform.csv contro locator-gen/rect_cases.expected.json", status: pass, evidence: "ricalcolato dal bridge: expected del CSV identici alla fixture; transform 8/8 e extract_path 8/8 con errore massimo 0,000 pt (soglia 0,5)"}
      - {test: "L-9 (informativa)", ref: "rect_transform.csv righe extract_text", status: pass, evidence: "8 righe presenti, scarto 67,528 pt su tutte: è la distanza voluta fra il marcatore e il rettangolo; nessuna soglia"}
  - must_have: "Il parsing di DOCX e MD per il locator avviene solo nel figlio isolato, lanciato con un job op: locate del contratto di 01-07, con i limiti d'archivio (D-05, D-08)"
    verification:
      - {test: TEST-8, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/main.rs:961#locate_accepts_exactly_the_resolve_argv ; :980#locate_rejects_output_flags_and_extra_args ; :1004 ; :1113#prepared_jobs_are_locate_with_null_out_dir_and_unique_ids ; :1165#negative_path_job_points_outside_allowed_roots ; :1183 ; :1202 ; :1228#locate_job_with_out_dir_is_rejected_by_the_contract ; :1248#reports_bind_one_to_one_by_job_id", status: pass, evidence: "68/68 verdi (bridge, host)"}
      - {test: TEST-7, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/docx.rs:593#parse_file_quarantines_external_relationship ; :606#parse_file_rejects_doctype_before_reading_xml ; :618 ; :529#doctype_or_entity_in_xml_is_an_error ; :515#hyperlink_targets_and_field_codes_are_not_resolved ; spikes/s02-parser/src/bin/locator/md.rs:590#links_and_raw_html_are_not_resolved ; spikes/s02-parser/src/bin/locator/main.rs:1409 ; :1424#locate_enforces_read_caps ; :1448", status: pass, evidence: "68/68 verdi (bridge, host)"}
      - {test: "L-10 (job ok)", ref: "spikes/s02-parser/results/locator/locator_samples.json (status, job_id)", status: pass, evidence: "ricalcolato dal bridge: 23/23 status ok, job_id che finiscono con -86f821f0"}
      - {test: "L-10 (job negativo e contratto dei job)", ref: "results/raw/locator/jobs.jsonl e reports.jsonl (ignorati da git)", status: human_judgment, human_judgment: true, rationale: "neg-path-86f821f0 → job_error/PathNotAllowed (excerpt_file_outside_roots), 24 job con out_dir null, nessun out_dir_exists, batch exit 2 solo per il negativo: dichiarati da rapporto e RISULTATI, ma i file grezzi non sono nel repo e il bridge non li ha potuti rileggere; il comportamento del padre è coperto da :1165 e :1228"}
  - must_have: "Fixture attese indipendenti e committate prima dei risultati (TEST-6)"
    verification:
      - {test: TEST-6, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/canon.rs:363#offset_fixture_is_hand_written_with_required_cases ; spikes/s02-parser/src/bin/locator/pdfrect.rs:166#rect_fixture_is_hand_written_with_eight_cases ; spikes/s02-parser/src/bin/locator/main.rs:1071#expected_samples_cover_the_plan_cases ; :1278#offsets_rows_carry_only_got_fields_matching_the_fixture ; :1298#offsets_ignore_expected_fields_of_the_input", status: pass, evidence: "68/68 verdi (bridge, host)"}
      - {test: "TEST-6 (ordine dei commit)", ref: "git log 45083a6^1..45083a6^2", status: pass, evidence: "rieseguito dal bridge: le tre fixture entrano da sole in 6175711 (21:30), i test rossi in 836ed68, le soglie in 3292022, il bin in 087a843, i risultati in 0582280; il diff di RISULTATI dopo 3292022 aggiunge solo dalla riga 52 (Setup)"}
  - must_have: "RISK-001 di risk_audit_12: una regola di normalizzazione cambiata senza cambiare projection fa fallire il round-trip (migrazione negativa)"
    verification:
      - {test: TEST-4, risks: [RISK-001 di risk_audit_12], ref: "spikes/s02-parser/src/bin/locator/canon.rs:642#locator_from_v1_on_v2_is_version_mismatch (test (2)) ; :665#locator_with_other_projection_is_rejected ; :679#changed_normalization_without_new_projection_fails_roundtrip ; :695#tampered_section_hash_is_rejected ; :707#unknown_path_or_range_is_rejected ; :733#serialized_locator_requires_version_fields", status: pass, evidence: "68/68 verdi (bridge, host)"}
  - must_have: "Scanner di sicurezza con exit 0 e MEDIUM motivati"
    verification:
      - {test: "security-scanner", ref: "python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src/bin/locator spikes/s02-parser/locator-gen", status: pass, evidence: "rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, 5 MEDIUM (archive-decompress docx.rs:35; pdf-untrusted-parse main.rs:766, :769, :830, :831), motivati in RISULTATI-locator.md:93"}
      - {test: "HIGH upload-in-memory eliminato", ref: "spikes/s02-parser/src/bin/locator/main.rs:163-192#read_bounded", status: human_judgment, human_judgment: true, rationale: "secondo il rapporto lo scanner segnalava HIGH sul pattern read_to_end, sostituito con la lettura a blocchi da 64 KiB con take(cap + 1); quella versione non è mai stata committata, quindi il bridge vede solo il risultato (0 HIGH), non il prima"}

# Metrics
duration: "circa 1 h 11 min il 2026-10-22: fixture a mano 6175711 21:30 → documentazione c5e9db6 22:30; merge su main 45083a6 22:41"
completed: 2026-10-22
---

# Fase 1 Piano 09: S-02 / A-10, locator di citazione per PDF, DOCX e Markdown · Summary

**Il locator a due varianti regge: su 12 campioni (23 job `op: locate`) il testo riestratto è identico all'excerpt e il percorso di sezione è quello scritto a mano in 23 casi su 23. Markdown e DOCX danno lo stesso percorso e lo stesso intervallo in 11 coppie su 11, ma lo stesso hash di sezione solo in 8 su 11, perché pandoc porta gli apostrofi da U+0027 a U+2019: L-4 fallisce e resta dichiarato. La conversione degli offset da scalar a UTF-16 coincide con la fixture a mano in 10 vettori su 10. I rettangoli PDF, con /Rotate e CropBox spostata, tornano esatti (errore 0,000 pt) sia dalla funzione pura sia dai bounds di PDFium. ADR-0005 è Proposed, nella forma multi-span decisa dall'utente.**

## Performance

- **Duration:** circa 1 h 11 min. Le fixture a mano `6175711` (21:30:16) precedono i test rossi `836ed68` (21:39:55). L'esecutore va da `c303ab3` (21:59:05) alla documentazione `c5e9db6` (22:30:01). Il merge su main è `45083a6` (22:41:27).
- **Started:** 2026-10-22T21:30:16+02:00
- **Completed:** 2026-10-22T22:41:27+02:00
- **Tasks:** 2 di 2
- **Files modified:** 19 nel diff del merge (`git diff --stat 45083a6^1 45083a6`). Sono i 17 `files_modified` del piano più `spikes/s02-parser/Cargo.toml` e `spikes/Cargo.lock`, una riga ciascuno (deviazione (a), sotto). Nessun file di 01-07, 01-08, 01-03 o del Dockerfile del worker nel diff.

## Accomplishments

- **Testo canonico `cka-text-v1`** (`spikes/s02-parser/src/bin/locator/canon.rs`). `canon_text` (`canon.rs:96-99`) unifica gli a capo e applica NFC. `sections` (`:129-187`) costruisce le sezioni con una pila di titoli aperti e un contatore per titolo sotto ogni genitore, che dà l'ordinale. `locate` (`:192-224`) trova tutte le occorrenze. `resolve` (`:230-250`) controlla proiezione, versione, percorso, hash e intervallo, in quest'ordine, prima di dare testo. `to_utf16_range` e `slice_scalars` (`:257-286`) convertono e ritagliano.
- **Lettori.** `md.rs` è un lettore a righe del sottoinsieme di Markdown dei documenti sintetici (`md.rs:12`): front matter (`:144`), codice recintato (`:157`), titoli ATX (`:175`), elenchi (`:218`), tabelle a pipe (`:238`), inline (`:297`). `docx.rs` passa da `check_docx` (`docx.rs:32`), poi legge `word/document.xml` con `take` (`:46`) e `quick-xml` a eventi (`:287-329`): `DocType` o un'entità non predefinita sono un errore (`:296`, `:314`).
- **Trasformazione dei rettangoli** (`spikes/s02-parser/src/bin/locator/pdfrect.rs:47-69`): funzione pura, CropBox efficace (`:19-28`), `/Rotate` normalizzato modulo 360 (`:11-16`).
- **Bin `locator`** (`spikes/s02-parser/src/bin/locator/main.rs`): CLI (`main.rs:97-145`); `locate` solo figlio, con letture a blocchi con tetto (`:165-192`) e ricerca e risoluzione su due proiezioni distinte (`:243-276`); `roundtrip prepare` (`:334-384`) e `check` (`:487-523`, `:541`); `offsets` con soli campi `got_*` (`:676-704`); `rects` in tre stadi (`:765-873`).
- **Strumenti** (`spikes/s02-parser/locator-gen/`): `gen_pages.py` genera gli 8 PDF copiando gli attesi dalla fixture senza formule; `check_offsets.mjs` verifica fixture e bin con la semantica di Node.
- **Fixture a mano** di `test-write` (`6175711`): 10 vettori di offset, 8 casi di rettangoli, 12 campioni.
- **Evidenze** (`spikes/s02-parser/results/locator/`), `RISULTATI-locator.md` e `docs/adr/0005-locator-citazione.md` (Proposed).

## Task Commits

1. **Fixture a mano (agente test-write, distinto dall'esecutore):** `6175711` (test). Tre file, nessun codice.
2. **Test rossi (test-write):** `836ed68` (test). RED_EXPECTED: 63 rossi su `todo!()`, 5 verdi strutturali (fixture, serde, contratto di 01-07), 68 in tutto.
3. **Dipendenza `sha2`:** `c303ab3` (chore), deviazione (a).
4. **Task 1, soglie e definizioni prima della misura:** `3292022` (docs).
5. **Task 1, bin `locator`, `gen_pages.py`, `check_offsets.mjs`:** `087a843` (feat).
6. **Task 2, misure:** `0582280` (test).
7. **Task 2, esito e ADR-0005:** `c5e9db6` (docs).

**Merge:** `45083a6`. **Plan metadata:** questo SUMMARY e `01-09-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s02-parser/src/bin/locator/canon.rs`: testo canonico, sezioni, locator `Text`, `resolve`, offset; TEST-2, TEST-3, TEST-4, parte di TEST-1 e TEST-6.
- `spikes/s02-parser/src/bin/locator/md.rs`: lettore Markdown; test (3) lato MD, (4), (5), TEST-7 lato MD.
- `spikes/s02-parser/src/bin/locator/docx.rs`: lettore DOCX dopo `check_docx`; test (3) lato DOCX e concordanza, (5), TEST-7.
- `spikes/s02-parser/src/bin/locator/pdfrect.rs`: trasformazione pura; TEST-5.
- `spikes/s02-parser/src/bin/locator/main.rs`: CLI e sottocomandi; TEST-1 end-to-end, TEST-6, TEST-7, TEST-8.
- `spikes/s02-parser/locator-gen/{gen_pages.py,check_offsets.mjs,expected_samples.json,offset_vectors.expected.json,rect_cases.expected.json}`.
- `spikes/s02-parser/results/locator/{hw.json,locator_samples.json,roundtrip.csv,rect_transform.csv,offset_vectors.json}`, `spikes/s02-parser/RISULTATI-locator.md`, `docs/adr/0005-locator-citazione.md`.
- `spikes/s02-parser/Cargo.toml`, `spikes/Cargo.lock`: una riga ciascuno per `sha2`.

## Esiti delle soglie

Ricalcolati dal bridge dai file di `results/locator/` e dalle fixture, contro le soglie di `3292022`.

| Id | Misura | Valore | Soglia | Esito |
|----|--------|--------|--------|-------|
| L-1 | Round-trip esatto, una sola occorrenza, sha256 del sorgente = manifest | 23/23 job (12 campioni) | 100% su ≥ 10 | raggiunta |
| L-2 | Stesso `section_path` MD/DOCX | 11/11 | 100% | raggiunta |
| L-3 | Stesso `char_range` MD/DOCX | 11/11 | 100% | raggiunta |
| L-4 | Stesso `section_text_sha256` MD/DOCX | 8/11 (72,7%) | 100% | **non raggiunta** |
| L-5 | `section_path` = atteso scritto a mano | 23/23 | 100% | raggiunta |
| L-6 | UTF-16 del bin = fixture; fixture = semantica di Node | 10/10 e 10/10 | 100% | raggiunta |
| L-7 | Stadio `transform` | 8/8, errore massimo 0,000 pt | 100% entro 0,5 pt | raggiunta |
| L-8 | Stadio `extract_path` (bounds grezzi = `user_rect`, trasformati = atteso) | 8/8, 0,000 pt | 100% entro 0,5 pt | raggiunta |
| L-9 | Stadio `extract_text` | scarto 67,528 pt su 8/8, voluto | informativa | — |
| L-10 | Job `op: locate` ok; `neg-path` → `PathNotAllowed` | 23/23 ok (ricalcolato); negativo dichiarato | 100% | raggiunta (negativo: human_judgment) |

**L-4.** Le tre coppie divergenti sono `s01-scenario`, `s02-governance` e `s02-razionale`. Nel testo di sezione c'è un apostrofo: il Markdown ha U+0027, il DOCX prodotto da pandoc U+2019. Percorso e intervallo restano uguali, perché la sostituzione cambia uno scalar con uno scalar. L'hash cambia, ed è giusto che cambi: `cka-text-v1` non ripiega la tipografia, per scelta scritta prima della misura. Gli excerpt dei campioni non contengono apostrofi, per questo il round-trip è esatto anche su queste coppie. La decisione regge perché un locator si risolve sempre sul file da cui nasce (`doc_sha256`). Il prodotto indicizza ogni file per sé e non usa la concordanza dell'hash fra due formati dello stesso contenuto.

**L-9.** Il marcatore «MARK <case_id>» sta 40 pt sopra il rettangolo per costruzione di `gen_pages.py`. Lo scarto di 67,528 pt misura quella distanza, non un errore della trasformazione, che sui bounds del tracciato dà 0,000 pt.

**[NV] sui bounds di PDFium, risolto.** I bounds del tracciato e i char box sono nello spazio utente della pagina, prima di `/Rotate` e senza traslazione della CropBox: grezzi = `[100, 200, 180, 230]` in 8 casi su 8. La trasformazione si applica una sola volta, lato sistema.

## Decisions Made

### Decisioni dell'utente già prese

- **D-10 e il suo paragrafo *Storia*** (`01-CONTEXT.md:36-37`, 2026-10-21): locator multi-span con al più due span. ADR-0005 scrive questa forma come decisione (`docs/adr/0005-locator-citazione.md:43-53`). Lo spike ha misurato il caso a span singolo, cioè `len = 1`. La validazione della consecutività dei due span **non è misurata** e va ai test della fase che implementa i contratti. Nessun test è stato aggiunto o cambiato per la forma multi-span, perché il design dei test non la copre.

### Scelte dell'esecutore

- **`resolve` su una seconda lettura e una seconda proiezione** (`main.rs:250-252`): il piano chiede un percorso di codice distinto dalla ricerca. Riusare la stessa `Projection` non avrebbe provato nulla sui byte del file.
- **Lettura a blocchi da 64 KiB con `take(cap + 1)`** (`main.rs:163-192`), al posto di `read_to_end`. Il tetto è lo stesso, ma lo scanner non segnala più HIGH upload-in-memory.
- **Nomi XML per `local_name`** (`docx.rs:79-81`): il prefisso di namespace in OOXML è arbitrario, `w:` è solo una convenzione.
- **`w:tab` solo fuori da `w:pPr`** (`docx.rs:145`): dentro `pPr` i `w:tab` sono tabulazioni impostate, non testo.
- **Disegni, oggetti e `mc:Choice` saltati e registrati in `unsupported`** (`docx.rs:91-98`, `:127-133`). Le caselle di testo creerebbero paragrafi annidati. `Choice` più `Fallback` duplicherebbero il testo.
- **`extract_path` passa solo se i bounds grezzi coincidono con `user_rect` e quelli trasformati con l'atteso** (`main.rs:847-849`): così si vede se PDFium li dà già ruotati.
- **Testo prima del primo titolo = sezione con percorso vuoto** (`canon.rs:174-182`). Nessun caso nel corpus.
- **`roundtrip check` legge il manifest da `/data/corpus/manifest.json`** (`main.rs:38`), sostituibile con `S02_CORPUS_MANIFEST`, perché la CLI fissata dai test non ha il flag. La voce DOCX si trova per `derived_from` e `format` (`main.rs:444`).
- **Job negativo con `excerpt_file` = `/etc/hostname`** (`main.rs:41`): il file esiste ma sta fuori dalle radici ammesse, quindi il padre lo rifiuta senza leggerlo.

## Deviations from Plan

### Livello a (dentro il piano)

1. **Dipendenza `sha2` in `s02-parser`.** `doc_sha256` e `section_text_sha256` hanno bisogno di sha256, e `sha2` non era fra le dipendenze del crate. Aggiunta `sha2.workspace = true` in `spikes/s02-parser/Cargo.toml`. In `spikes/Cargo.lock` cambia una riga, l'arco verso `sha2 0.11.1`, già nel lock per s01, s04 e s06: nessuna versione nuova, nessun download. I due file sono fuori dai `files_modified`. Lo ha autorizzato il coordinatore. Commit `c303ab3`, verificato dal bridge con `git diff 45083a6^1 45083a6 -- spikes/Cargo.lock spikes/s02-parser/Cargo.toml`.
2. **`--env-file` con `cygpath`.** Il primo tentativo di sessione (`01-09-20261022t201928z-44ab`) è fallito. `docker compose … --env-file` aveva ricevuto il percorso in forma Git Bash (`/c/Users/…`), che `docker.exe` non risolve con `MSYS_NO_PATHCONV=1`. La sessione si è chiusa con `down` e rilascio prima di ogni build o misura. Il secondo tentativo passa il percorso per `cygpath -m`, in uno script di sessione nello scratchpad, fuori dal repo. Nessun file del repo cambiato, nessuna misura persa.
3. **`expected_samples.json` scritto da `test-write`.** Il piano lo assegnava all'esecutore nel Task 2, prima delle misure. È entrato invece con le altre fixture in `6175711`, cioè ancora prima, e l'esecutore non l'ha toccato. Il vincolo del piano (campioni committati prima dei risultati) è rispettato con margine.

### Segnalazioni su file chiusi

Nessuna. Il rapporto dichiara «nessun difetto di altri piani trovato».

### Deviazione d'ambiente del gate

4. **Test sull'host e nel runner.** Sull'host Windows i test di `lib` ed `extract` che richiedono `qpdf` o `libpdfium.so` Linux falliscono per ambiente. Per questo il gate d'onda usa `cargo test -p s02-parser` nel runner e il resto del workspace sull'host.

---

**Total deviations:** 3 di livello a, 1 d'ambiente; nessuna di livello b o c. Nessun test tolto o indebolito: i blocchi `#[cfg(test)]` dei cinque file del bin hanno lo stesso sha256 in `836ed68` e in `c5e9db6`.
**Impact on plan:** perimetro esteso di due righe di manifest e lock, autorizzate; nessun file di altre fasi o piani.

## Gate d'onda

- **Host:** `cargo fmt` e `cargo clippy --workspace -D warnings` verdi; `cargo test` sul workspace con `--exclude s02-parser`, 116 verdi.
- **Runner, sul main fuso `69af2c9`:** `cargo test -p s02-parser` con exit 0, 257 verdi (locator 68) e 1 ignorato per costruzione.
- **Perimetro:** `proprieta-file` conforme.
- **Scanner:** `--fail-on HIGH` PASS.
- **Bridge:** ha rieseguito sull'host `cargo test -p s02-parser --bin locator --locked --offline` (68/68), lo scanner sui percorsi del piano (exit 0) e `check_offsets.mjs` (exit 0). Il resto del gate non l'ha ripetuto.

## Scanner di sicurezza

`python skills/security-scanner/security_scan.py --fail-on HIGH spikes/s02-parser/src/bin/locator spikes/s02-parser/locator-gen`, rieseguito dal bridge: exit 0, 0 CRITICAL, 0 HIGH, 5 MEDIUM.

- **`archive-decompress`, `docx.rs:35`.** Lo zip si apre solo dopo `check_docx` (`docx.rs:32`), che limita voci, somma dichiarata e reale, rapporto, DTD ed entità; `document.xml` si legge con `take` (`:46`).
- **`pdf-untrusted-parse`, `main.rs:766`, `:769`, `:830`, `:831`.** Il sottocomando `rects` apre solo i PDF sintetici generati da `gen_pages.py`, che non vengono dall'esterno. Per il contratto di 01-07 non serve il figlio.

## Discrepanze fra rapporto e prove

- **Conteggio del runner.** Il rapporto riporta 248 verdi nella propria sessione: 84 + 9 + 31 + 68 + 1 + 7 + 37 + 11, cioè 248. Il gate dell'integratore sul main fuso `69af2c9` ne conta 257, perché il main comprende anche 01-10. I numeri non si contraddicono. Il bridge ha rieseguito solo i 68 del locator.
- **File del diff.** Il rapporto conta 16 file (`836ed68..HEAD`) e il merge ne porta 19. La differenza sono le tre fixture di `6175711`, che stanno prima della base dell'esecutore.
- **Evidenze grezze non rileggibili.** `results/raw/locator/` è ignorata da git e non esiste sul main (`jobs.jsonl`, `reports.jsonl`, `cases.json`). Per questo l'esito del job negativo e il contratto dei 24 job sono marcati `human_judgment` in coverage. Tutto ciò che sta nei file committati è stato ricalcolato.
- **HIGH dello scanner.** La versione con `read_to_end` non è mai stata committata: il bridge vede solo il risultato.
- Il resto coincide con le prove: commit e ordine, valori L-1…L-9, header dei CSV, `hw.json` con `git_rev` `087a843` e immagine `cka-spikes-worker@sha256:446219af…`, ADR con T-07, T-10, T-17 e link a file esistenti.

## Threat Flags

- **T-01-09-01** (lettura DOCX/MD): `check_docx` prima dell'XML, letture con tetto, solo figlio via `op: locate`; TEST-7, TEST-8.
- **T-01-09-02** (identità del locator): ordinale, `doc_sha256`, `projection`, `section_text_sha256`; TEST-2, TEST-4.
- **T-01-09-03** (offset verso il viewer): conversione verificata contro la fixture a mano e contro Node; TEST-3, L-6.
- **T-01-09-04** (link esterni): relazioni esterne in quarantena dal prescan, destinazioni mai risolte; `docx.rs:593`, `:515`, `md.rs:590`.
- **T-01-09-05** (payload del figlio): solo locator, hash ed excerpt riestratto, mai il testo completo; `main.rs:1334`.
- **RISK-001 di risk_audit_12** (proiezione immutabile): `canon.rs:679`.

## Known Stubs

Nessuno. `docx::parse_document_xml` (`docx.rs:24-27`) è usata solo dai test sull'XML scritto a mano. Lo dichiara `cfg_attr(not(test), allow(dead_code))`.

## User Setup Required

Nessuno.

## Punti aperti

- **L-4 e la tipografia di pandoc:** se basta l'argomento «un locator si risolve sul proprio file» lo ratifica l'utente nel consolidamento (01-17).
- **Consecutività dei due span** della forma multi-span: non misurata. Va nei test della fase dei contratti (S-07 / 01-16, poi Fondazione).
- **Ratifica di ADR-0005:** resta Proposed fino a 01-17.
- **Nuove versioni di proiezione:** un cambio di `cka-text-v1` richiede `cka-text-v2` e il ricalcolo dei locator salvati.

## Next Phase Readiness

- **01-16 (S-07)** riceve il tipo `Locator` = `Pdf { spans: [{ page, rects }] }` | `Text { projection, doc_sha256, spans: [{ section_path, char_range, section_text_sha256 }] }`, con 1 ≤ `len` ≤ 2, e gli errori di risoluzione tipizzati (`RISULTATI-locator.md:99`).
- **01-17** riceve le righe di threat model (T-07, T-10, T-17) e la nota per DEPENDENCIES.md: nessuna dipendenza nuova di prodotto (`RISULTATI-locator.md:100-101`).
- **Porta `DocumentParser`:** per DOCX e MD deve produrre i blocchi di `cka-text-v1`, per i PDF i rettangoli nello spazio utente.

## Self-Check: PASSED

- 19 file nel diff del merge `45083a6`: 17 `files_modified` più `Cargo.toml` e `Cargo.lock` (deviazione (a)).
- Rieseguiti dal bridge: ordine dei commit (fixture → test rossi → soglie → bin → risultati), invarianza delle Soglie dopo `3292022`, identità dei blocchi di test fra `836ed68` e `c5e9db6`, ricalcolo di L-1…L-9 dai file di `results/locator/`, `check_offsets.mjs` (exit 0), `cargo test -p s02-parser --bin locator` sull'host (68/68), scanner con soglia HIGH (exit 0).
- Gate d'onda nel runner e sull'host: dal rapporto dell'integratore, non ripetuto dal bridge oltre il locator.

**Modifica:** Claude (Anthropic) · 2026-10-23 · creazione (bridge di 01-09)
