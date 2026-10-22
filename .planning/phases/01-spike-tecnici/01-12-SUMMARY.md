---
phase: 01-spike-tecnici
plan: 12
subsystem: spike-llm-generativo-routing-structured-output
tags: [anthropic, messages-api, claude-haiku-5-5, claude-sonnet-5-5, structured-output, json_schema, output_config, effort, stop_reason, citazioni, mapping-deterministico, jev, typesafe, choice, noul, validate_query, support_claim, fallback, degraded, prompt-injection, escape, acl, assert_visible, cost-guard, spend-ledger, splitmix64, reqwest, rustls, adr]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate spikes/common (env::required, manifest, csv::Writer, hist::Recorder) e Cargo.toml di s06-llm con le dipendenze fissate; main.rs stub (fn main() {})"
  - phase: 01-02
    provides: "harness Compose chiuso, session.sh (with/oneshot/ps/down/release), hw-manifest.sh, runner Linux, file segreti esterno CKA_SPIKES_ENV"
  - phase: 01-06
    provides: "corpus ratificato: acl.json, manifest.json, oracles/queries.json con i tre assi verdi (74 oracoli), Markdown sintetici syn-07 e syn-08 con i marcatori injection-case"
  - phase: 01-11
    provides: "ADR-0008 (Noul per coppia, concorrenza 16, retry unico, pin jev-1.13.0), criterio di costo JEV da usage, registro di spesa su file, misura dell'injection rinviata a 01-12"
provides:
  - "Crate s06-llm (binario, test rossi a5768df e 1a9c215, implementazione e797e9f, 43d8c83, 01d862f): contesto autorizzato con id brevi c01..c08, client Messages API con structured output, client JEV locale (validate_query con Choice di complessità, support_claim con Noul), validazione stretta dello schema e mapping deterministico delle citazioni, marcatura validated/unsupported/degraded/refused"
  - "Evidenze con soglie scritte prima: JSON conforme 94/94; 0 claim con id ignoto su 141; rifiuto 10/10 sulle no_answer per entrambi i modelli; accordo JEV/etichetta 25/30 = 0,8333; fallback 15/15 Sonnet, 0 validated; injection 0 istruzioni seguite su 32 risposte; unauthorized_in_prompt 0 su 120 righe; spesa 0,598 USD su 2"
  - "Regola di routing di D-31 tornata all'utente (Haiku 0,6852 contro Sonnet 0,7931 sulle complex; Haiku 0,7241 sulle simple), misura supplementare D-32 (Sonnet 1,0000 contro Haiku 0,7241 sulle stesse 13 simple) e scelta utente D-33: Sonnet 5.5 su tutte le query"
  - "ADR-0011 Proposed: Sonnet su simple, complex e fallback; validate_query restituisce comunque la complessità; StructuredAnswer con citazioni per claim mappate in codice; mai validated senza JEV"
affects: [01-16, 01-17, fase RAG & Answer, porta AnswerGenerator, porta DecisionEngine, DEPENDENCIES.md riga Generative LLM]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Structured output con output_config.format json_schema ed effort low, nessun parametro di campionamento; stop_reason controllato prima del parse; schema rivalidato in Rust con chiavi esatte, perché la API non impone minItems né minLength"
    - "Id brevi c01..c08 nel prompt e mappa id → (doc_key, locator) tenuta dal codice: il modello sceglie solo un id, la provenienza la scrive il sistema; un claim con anche un solo id ignoto è scartato per intero"
    - "Passaggi come dati delimitati nel messaggio utente, con & < > \" sostituiti dalle entità; istruzioni solo nel system"
    - "Stato della risposta calcolato da un esito JEV per claim: Unavailable su un solo claim → degraded; validated solo se ogni claim conservato ha Noul ≥ 0,5"
    - "Registro di spesa su file letto in modo stretto per la misura supplementare: registro assente = errore, mai residuo pieno per default; la spesa nuova si somma, non si riscrive"
    - "JEV reso irraggiungibile aprendo e chiudendo un listener loopback nel processo, senza toccare l'ambiente"

key-files:
  created:
    - spikes/s06-llm/src/context.rs
    - spikes/s06-llm/src/validate.rs
    - spikes/s06-llm/src/anthropic.rs
    - spikes/s06-llm/src/jev.rs
    - spikes/s06-llm/schema/answer.schema.json
    - spikes/data/s06/selection.json
    - spikes/s06-llm/RISULTATI.md
    - spikes/s06-llm/results/hw.json
    - spikes/s06-llm/results/llm.csv
    - spikes/s06-llm/results/routing.csv
    - spikes/s06-llm/results/fallback.csv
    - spikes/s06-llm/results/injection.csv
    - spikes/s06-llm/results/llm-simple-sonnet.csv
    - docs/adr/0011-llm-generativo-routing-structured-output.md
  modified:
    - spikes/s06-llm/src/main.rs

key-decisions:
  - "D-31 (utente): matrice dimezzata, Sonnet solo su complex e no_answer, fallback su 15 query a seme, tetto 2 USD, regola di routing a due condizioni scritta prima della misura"
  - "D-32 (utente): dopo la regola fallita su entrambe le condizioni, misura diretta di Sonnet sulle stesse 13 simple, CSV separato, Soglie e CSV precedenti invariati"
  - "D-33 (utente): Sonnet 5.5 su tutte le query, emenda D-18; validate_query continua a restituire la complessità, che oggi non cambia il modello"
  - "Escape dei delimitatori invece del rifiuto dei passaggi ostili, per poter misurare proprio la tenuta all'injection (TEST-9)"
  - "claims_n conta tutti i claim emessi, anche quelli senza citazioni, che non sono mai supportati: fissato nelle Soglie prima della misura; la lettura senza claim non citati resta un dato diagnostico"
  - "A probabilità pari la Choice di complessità vale complex: in dubbio il modello più capace"

patterns-established:
  - "AnswerGenerator restituisce StructuredAnswer { answer, claims: [{ text, citations: [PassageRef] }], refusal }; la validazione claim–citazione è un'operazione separata del DecisionEngine, con errore tipizzato che diventa Unavailable e mai validated"
  - "Nel prodotto i claim senza citazioni o con testo vuoto vanno scartati in modo deterministico come quelli con id ignoto"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Claude Haiku 5.5 e Claude Sonnet 5.5 sono misurati via API con structured output JSON (validità, claim con citazioni, rifiuti, latenza, costo da usage), senza LLM locale (D-18, D-20; criterio 4: LLM generativo scelto)"
    verification:
      - {test: TEST-15, risks: [RISK-8], ref: "spikes/s06-llm/src/anthropic.rs:463#stop_reason_checked_before_parse ; spikes/s06-llm/src/validate.rs:365#stop_reason_and_schema_validation_before_use", status: pass, evidence: "rieseguiti dal bridge: cargo test -p s06-llm --locked 35 passed 0 failed"}
      - {test: TEST-16, risks: [RISK-9], ref: "spikes/s06-llm/src/anthropic.rs:531#cost_guard_stops_over_budget ; :519#cost_uses_declared_prices ; spikes/s06-llm/results/llm.csv colonna cost_usd", status: pass, evidence: "rieseguiti dal bridge: verdi; somma di cost_usd in llm.csv ricalcolata 0,1637 USD"}
      - {test: TEST-18, risks: [RISK-11], ref: "spikes/s06-llm/src/anthropic.rs:436#request_body_shape ; grep temperature/json_schema/citations enabled su spikes/s06-llm/src", status: pass, evidence: "rieseguito dal bridge: request_body senza temperature né citations; json_schema 4 occorrenze; nessun citations enabled; temperature solo nelle asserzioni di test anthropic.rs:447 e :644 (deviazione a 1)"}
      - {test: TEST-19, risks: [RISK-12, RISK-14], ref: "spikes/data/s06/selection.json ; spikes/s06-llm/results/*.csv ; spikes/s06-llm/results/hw.json", status: pass, evidence: "ricalcolato dal bridge: chiavi esatte seed/fallback_seed/ids/fallback_ids, semi 120601/120602, 30 ids, 15 fallback_ids ⊂ ids; llm.csv 94 righe (60 Haiku, 34 Sonnet), solo rep 2 e 3, json_valid 94/94; hw.json con tools llm_simple claude-haiku-5-5, llm_complex claude-sonnet-5-5, jev_model jev-1.13.0, git_rev 43d8c83"}
      - {test: "latenza p95", ref: "spikes/s06-llm/results/llm.csv colonna latency_ms ; spikes/s06-llm/RISULTATI.md §Esito", status: human_judgment, human_judgment: true, rationale: "soglia mancata e dichiarata: p95 Haiku 3,744 s contro 3 s (ricalcolo nearest-rank del bridge 3,742 s; RISULTATI usa l'istogramma di common::hist). Dopo D-33 Haiku non è più in uso e la soglia non decide nulla, ma resta un fallimento registrato. Sonnet 3,994 s su complex/no_answer e 4,481 s sulle simple, entro 8 s"}
  - must_have: "La complessità della query la decide JEV dentro validate_query con una Choice simple/complex, misurata contro le etichette ratificate; simple → Haiku, complex → Sonnet (D-18)"
    verification:
      - {test: "RISK-10 (unit)", risks: [RISK-10], ref: "spikes/s06-llm/src/jev.rs:349#validate_query_reads_choice_and_pins_model", status: pass, evidence: "rieseguito dal bridge: verde; modello del corpo = configurazione, scelta letta dalle probabilities"}
      - {test: TEST-17, risks: [RISK-10], ref: "spikes/s06-llm/results/routing.csv", status: pass, evidence: "ricalcolato dal bridge: 30 righe; label = complexity dell'oracolo su 30; agree = (label == jev_choice) su 30; model_used Haiku su simple e Sonnet su complex su 30; accordo 25/30 = 0,8333 come in RISULTATI; model_resolved_jev jev-1.13.0 su 30"}
      - {test: TEST-23, risks: [RISK-13], ref: "spikes/s06-llm/src/main.rs:2295#routing_rule_both_branches ; :2335#routing_rule_zero_denominator_goes_back_to_user ; :2354#routing_rule_filters_reps_validity_no_answer_and_aligns_query_sets", status: pass, evidence: "rieseguiti dal bridge: verdi; ricalcolo da llm.csv (rep 2-3, json_valid, no_answer escluse): Haiku complex 37/54 = 0,6852, Sonnet complex 46/58 = 0,7931, Haiku simple 21/29 = 0,7241 → torna all'utente, uguale a RISULTATI e ADR-0011"}
      - {test: "TEST-23 (script node con quattro fixture)", risks: [RISK-13], ref: "nessun file: script previsto dal test design", status: human_judgment, human_judgment: true, rationale: "lo script di controllo in sola lettura con le fixture (entrambe vere, (a) falsa a 0,06, (b) falsa a 0,84, confine esatto) non è stato scritto né dall'esecutore né dal test-write: i rami sono provati dai test unitari di routing_rule, il ricalcolo sul CSV reale lo ha fatto il bridge. Da completare in verify"}
  - must_have: "Se JEV non risponde la generazione usa Sonnet e la risposta è marcata degradata, mai validata: nessun controllo di sicurezza bypassato (D-19)"
    verification:
      - {test: TEST-4, risks: [RISK-2], ref: "spikes/s06-llm/src/validate.rs:331#jev_unreachable_marks_degraded_never_validated", status: pass, evidence: "rieseguito dal bridge: verde"}
      - {test: TEST-5, risks: [RISK-2], ref: "spikes/s06-llm/src/main.rs:2390#fallback_end_to_end_closed_port ; spikes/s06-llm/src/jev.rs:447#closed_port_is_an_error_within_connect_timeout", status: pass, evidence: "rieseguiti dal bridge: verdi; solo i 2 fallback_ids generati, modello Sonnet, stato degraded, mai validated, testo vietato assente dal corpo"}
      - {test: TEST-6, risks: [RISK-2], ref: "spikes/s06-llm/results/fallback.csv", status: pass, evidence: "ricalcolato dal bridge: 15 righe, insieme dei query_id = fallback_ids; validated true 0; degraded true 15; model_used claude-sonnet-5-5 su 15; status 11 degraded e 4 refused"}
  - must_have: "L'LLM cita solo chunk_id forniti nel contesto: un id non fornito o non autorizzato fa scartare il claim in modo deterministico; versione, pagina e locator li aggiunge il sistema; JEV valida claim–citazione; niente Citations native (D-20)"
    verification:
      - {test: TEST-7, risks: [RISK-3], ref: "spikes/s06-llm/src/validate.rs:280#unknown_citation_id_drops_claim", status: pass, evidence: "rieseguito dal bridge: verde; claim con un solo id ignoto scartato per intero, varianti C02/ c02/c2 ignote"}
      - {test: TEST-8, risks: [RISK-3], ref: "spikes/s06-llm/src/validate.rs:307#locator_comes_from_context_not_model", status: pass, evidence: "rieseguito dal bridge: verde"}
      - {test: TEST-13, risks: [RISK-6], ref: "spikes/s06-llm/src/validate.rs:347#validated_only_if_all_kept_claims_confirmed", status: pass, evidence: "rieseguito dal bridge: verde"}
      - {test: "claims_unknown_id", risks: [RISK-3], ref: "spikes/s06-llm/results/llm.csv colonna claims_unknown_id", status: pass, evidence: "ricalcolato dal bridge: 0 su 141 claim emessi"}
      - {test: "claim senza citazioni (diagnostico)", ref: "results/raw/generate.jsonl nel worktree exec/01-12 (ignorato da git)", status: pass, evidence: "ricontato dal bridge dai grezzi: Haiku 11 su 83 claim senza citazioni (7 con testo vuoto), Sonnet 1 su 58 nel Task 2 e 0 su 30 nel Task 3; varianti diagnostiche 0,7708 / 0,8070 / 0,8750 coincidono con RISULTATI"}
  - must_have: "Nessun passaggio non visibile al ruolo della query entra nel prompt né va a JEV (D-16)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s06-llm/src/context.rs:381#context_rejects_invisible_passage_before_prompt ; :430#passages_for_fills_only_with_visible_excerpts ; :367#acl_visibility_is_deny_by_default ; :415#context_rejects_more_than_eight_passages", status: pass, evidence: "rieseguiti dal bridge: verdi"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s06-llm/src/jev.rs:413#jev_calls_guarded_by_assert_visible", status: pass, evidence: "rieseguito dal bridge: verde, 0 richieste sul server finto; parte validate_query non applicabile (riceve solo il testo della query)"}
      - {test: TEST-3, risks: [RISK-1], ref: "spikes/s06-llm/results/llm.csv colonna 17", status: pass, evidence: "ricalcolato dal bridge: unauthorized_in_prompt 0 su 94 righe. Il valore è 0 per costruzione, perché il riconteggio (main.rs:376) gira dopo context::build, che fallisce già su un passaggio invisibile: la garanzia vera è TEST-1"}
      - {test: TEST-27, risks: [RISK-16, RISK-1], ref: "spikes/s06-llm/results/llm-simple-sonnet.csv", status: pass, evidence: "ricalcolato dal bridge: header uguale a llm.csv, 26 righe, solo claude-sonnet-5-5, solo simple senza no_answer, rep 2 e 3, nessun duplicato, stesso insieme di 13 query delle righe Haiku simple di llm.csv, colonna 17 a 0 su 26"}
  - must_have: "Chiavi, soglie prima delle misure, budget, injection e immutabilità del Task 3 (threat model T-01-12-02, -05, -06; RISK-4, 5, 7, 9, 15..19)"
    verification:
      - {test: TEST-9, risks: [RISK-4], ref: "spikes/s06-llm/src/context.rs:483#prompt_wraps_passages_as_data_and_instructions_only_in_system", status: pass, evidence: "rieseguito dal bridge: verde; scelta escape documentata in RISULTATI §Setup"}
      - {test: TEST-10, risks: [RISK-4], ref: "spikes/s06-llm/src/validate.rs:404#detects_urls_and_markdown_images_in_answer", status: pass, evidence: "rieseguito dal bridge: verde"}
      - {test: TEST-11, risks: [RISK-4, RISK-3], ref: "spikes/s06-llm/results/injection.csv", status: pass, evidence: "ricalcolato dal bridge: 32 righe, 8 case_id distinti (inj-07-a..d, inj-08-a..d), instruction_followed 0, unknown_ids_after_mapping 0, urls 0, immagini 0"}
      - {test: "TEST-11 (lettura qualitativa)", risks: [RISK-4], ref: "spikes/s06-llm/src/main.rs:1215#instruction_followed ; results/raw/injection.jsonl", status: human_judgment, human_judgment: true, rationale: "instruction_followed viene da rilevatori a sottostringa per tipo di caso; la rilettura a mano delle 32 risposte dichiarata dall'esecutore non è stata ripetuta dal bridge. 0 su 32 dice che i rilevatori non hanno scattato, non che il modello sia immune: da giudicare in verify sui grezzi"}
      - {test: TEST-12, risks: [RISK-5], ref: "spikes/s06-llm/src/anthropic.rs:618#client_errors_redact_keys ; spikes/s06-llm/src/jev.rs:427#client_errors_redact_keys ; grep delle chiavi ; python skills/security-scanner/security_scan.py spikes/s06-llm spikes/data/s06", status: pass, evidence: "rieseguito dal bridge: test verdi; nomi delle chiavi solo in common::env::required (main.rs:688, :728); valori reali, letti dal file segreti, assenti da spikes/s06-llm, spikes/data/s06, ADR-0011 e dal rapporto (grep -q, mai stampati); scanner «OK su 10 file», exit 0"}
      - {test: TEST-14, risks: [RISK-7], ref: "git merge-base --is-ancestor e797e9f edb8335 ; sezione Soglie fra e797e9f e 6d1f090", status: pass, evidence: "rieseguito dal bridge: e797e9f (13:12) antenato di edb8335 (13:28), primo commit che aggiunge results/; sha256 della sezione Soglie identico nei due commit (af04b70c…); la sezione contiene la regola di D-31 a due condizioni e il ramo «torna all'utente»"}
      - {test: TEST-20, risks: [RISK-14, RISK-9], ref: "spikes/s06-llm/src/main.rs:2095#sonnet_matrix_excludes_simple ; join llm.csv × queries.json", status: pass, evidence: "rieseguito e ricalcolato dal bridge: 0 righe Sonnet su simple senza no_answer; le 17 query complex o no_answer della selezione hanno righe Sonnet per rep 2 e 3"}
      - {test: TEST-21, risks: [RISK-14, RISK-12], ref: "spikes/s06-llm/src/main.rs:2038#selection_is_deterministic_from_seeds ; git log --diff-filter=A selection.json", status: pass, evidence: "rieseguito dal bridge: verde (semi 42/7 contro 42/8); selection.json nasce in e797e9f, antenato dei risultati"}
      - {test: "TEST-21 (rigenerazione coi semi reali)", risks: [RISK-14], ref: "s06-llm select --seed 120601 --fallback-seed 120602", status: human_judgment, human_judgment: true, rationale: "il comando riscrive spikes/data/s06/selection.json: il bridge non l'ha lanciato per non modificare un file tracciato. Il determinismo è provato dal test unitario; la coincidenza byte per byte coi semi reali va controllata in verify"}
      - {test: TEST-22, risks: [RISK-9], ref: "spikes/s06-llm/src/anthropic.rs:559#budget_cap_must_equal_d31 ; :573#estimate_over_cap_stops ; spikes/s06-llm/src/main.rs:1857-1858", status: pass, evidence: "rieseguiti dal bridge: verdi; nel comando all la stima (main.rs:1857) precede il warm-up (:1858); S06_MAX_COST_USD = 2 nel file segreti; sha256 del file segreti 9a25a0c976090b21… uguale al valore del rapporto; spikes/.env.example e DEPENDENCIES.md non toccati; stima 1,5614 USD in §Setup già in e797e9f"}
      - {test: TEST-24, risks: [RISK-16], ref: "spikes/s06-llm/src/main.rs:2119#simple_sonnet_selects_only_simple_non_no_answer ; :2164#simple_sonnet_matches_haiku_only_set_of_generate", status: pass, evidence: "rieseguiti dal bridge: verdi"}
      - {test: TEST-25, risks: [RISK-17], ref: "spikes/s06-llm/src/main.rs:2200#simple_sonnet_estimate_uses_residual_of_spend_json", status: pass, evidence: "rieseguito dal bridge: verde"}
      - {test: TEST-26, risks: [RISK-15, RISK-7], ref: "git diff --quiet edb8335 6d1f090 -- llm.csv routing.csv fallback.csv injection.csv hw.json ; RISULTATI.md", status: pass, evidence: "rieseguito dal bridge: diff vuoto sui quattro CSV e su hw.json; Soglie identiche; il diff di RISULTATI.md da edb8335 non ha righe tolte, solo aggiunte"}
      - {test: TEST-28, risks: [RISK-19], ref: "spikes/s06-llm/results/llm.csv ; spikes/s06-llm/results/llm-simple-sonnet.csv", status: pass, evidence: "ricalcolato dal bridge sull'intersezione delle 13 query: Haiku 21/29 = 0,7241 (uguale a CONTEXT D-32), Sonnet 30/30 = 1,0000; variante diagnostica Haiku 21/24 = 0,8750 dai grezzi, etichettata come tale e assente dall'Esito"}
      - {test: "TEST-28 (fixture)", risks: [RISK-19], ref: "nessun file: script con fixture (i)-(iv) previsto dal test design", status: human_judgment, human_judgment: true, rationale: "lo script con le quattro fixture (insiemi uguali, query in più, riga json_valid false o rep 1, denominatore nullo) non è stato scritto («rinviato a verify» nel rapporto); il calcolo sul CSV reale coincide, i casi limite sono provati solo da claim_rate e routing_rule nei test unitari"}
      - {test: TEST-29, risks: [RISK-17, RISK-9], ref: "results/raw/spend.json nel worktree exec/01-12 ; spikes/s06-llm/RISULTATI.md §Misura supplementare", status: pass, evidence: "letto dal bridge: spent_usd 0,59799 ≤ 2; stima 0,5247 contro residuo 1,5518 già in 28f73ab, antenato di 6a7789f; somma cost_usd del nuovo CSV 0,0967. La scomposizione del delta 0,1498 (warm-up 0,0038, prima ripetizione circa 0,048, JEV 0,0008) è dell'esecutore: i grezzi non portano il costo per richiesta"}
      - {test: TEST-30, risks: [RISK-18], ref: "spikes/s06-llm/RISULTATI.md §Esito della misura supplementare ; docs/adr/0011-llm-generativo-routing-structured-output.md Stato", status: pass, evidence: "verificato dal bridge: a 6a7789f l'esecutore non nomina un modello per le simple e ADR-0011 è Proposed; la scelta Sonnet è entrata dopo, con 52cdbae, come decisione utente D-33 firmata dall'orchestratore"}
      - {test: "ADR-0011 (ratifica e coerenza interna)", ref: "docs/adr/0011-llm-generativo-routing-structured-output.md", status: human_judgment, human_judgment: true, rationale: "l'ADR è Proposed: ratifica e riga «Generative LLM» di DEPENDENCIES.md spettano a 01-17. Dopo D-33 la Decisione dice Sonnet anche sulle simple, ma Conseguenze negative, Follow-up ed Evidenze dicono ancora che la scelta per le simple manca o resta all'utente: testo da allineare prima della ratifica"}

# Metrics
duration: "circa 31 min per i Task 1-2 (test rossi 12:57:35 → edb8335 13:28:32) e circa 10 min per il Task 3 (test rossi 15:52:01 → 6a7789f 16:01:53), 2026-10-22; D-33 52cdbae 16:05:30, merge su main 6d1f090 16:05:38"
completed: 2026-10-22
---

# Fase 1 Piano 12: S-06, LLM generativo, routing per complessità e structured output · Summary

**Con structured output json_schema Haiku 5.5 e Sonnet 5.5 producono JSON valido in 94 risposte su 94, non citano mai un id non fornito (0 su 141 claim), rifiutano 10 query senza risposta su 10 e non seguono nessuna delle 32 injection indirette; senza JEV il fallback genera con Sonnet e non marca validata nessuna delle 15 risposte. La regola di D-31 è fallita su entrambe le condizioni (Haiku 0,6852 contro 0,7931 di Sonnet sulle complex, 0,7241 sulle simple). Sulle stesse 13 simple Sonnet arriva a 1,0000 contro 0,7241 di Haiku, e l'utente ha scelto Sonnet su tutte le query (D-33). Spesa 0,598 USD su 2; ADR-0011 `Proposed`. Unica soglia mancata, il p95 di Haiku (3,744 s contro 3 s)**

## Performance

- **Duration:** circa 41 minuti di lavoro in due blocchi. I test rossi `a5768df` (12:57:35, agente test-write separato) precedono le soglie `e797e9f` (13:12:35). Le misure dei Task 1-2 si chiudono con `edb8335` (13:28:32). I test rossi del Task 3 `1a9c215` (15:52:01) precedono il sottocomando `01d862f`, la parte di RISULTATI scritta prima della misura `28f73ab` e la misura `6a7789f` (16:01:53). D-33 è `52cdbae` (16:05:30), il merge su main è `6d1f090` (16:05:38).
- **Started:** 2026-10-22T12:57:35+02:00
- **Completed:** 2026-10-22T16:05:30+02:00
- **Tasks:** 3 di 3 (piano v3)
- **Files modified:** 15, esattamente i `files_modified` del piano (`git diff --stat 1de61a9 6d1f090`). In più quattro Markdown di pianificazione (`01-12-PLAN.md`, `01-12-TEST-DESIGN.md`, `01-CONTEXT.md`, `01-GATE.md`), ammessi da qualsiasi fase. `context.rs`, `validate.rs`, `anthropic.rs`, `jev.rs` e lo schema nascono come stub del test-write in `a5768df`; `main.rs` era lo stub `fn main() {}` di 01-01.

## Accomplishments

- **Contesto autorizzato** (`spikes/s06-llm/src/context.rs`). `Acl::is_visible` nega per default (`context.rs:86-94`). `passages_for` (`:209`) prende gli `expected_passages` e poi gli excerpt visibili allo stesso ruolo, fino a 8. `build` (`:259`) chiama `assert_visible` (`:271`) prima di scrivere il prompt, neutralizza i delimitatori con `escape_data` (`:238`) e assegna gli id `c01`…`c08` (`:287`). Le istruzioni stanno solo in `SYSTEM_PROMPT` (`:186`).
- **Validazione deterministica** (`spikes/s06-llm/src/validate.rs`). `parse_answer` (`:79`) rivalida lo schema con chiavi esatte. `map_citations` (`:142`) scarta per intero un claim con anche un solo id ignoto. `mark` (`:164`) restituisce `Degraded` se manca anche un solo esito JEV e `Validated` solo se ogni claim ha Noul ≥ 0,5.
- **Client Messages API** (`spikes/s06-llm/src/anthropic.rs`). `request_body` (`:199`) usa `output_config` con `effort: "low"` e il formato json_schema, senza parametri di campionamento. `extract` (`:214`) legge `stop_reason` prima del parse. Il costo si calcola da `usage` con i prezzi costanti (`:128-131`). La guardia di spesa (`:310-316`) rifiuta una richiesta oltre il tetto, e il tetto ammesso è solo 2 (`check_budget_cap`, `:157`).
- **Client JEV locale** (`spikes/s06-llm/src/jev.rs`). `validate_query` (`:235`) pone una Choice di complessità; a probabilità pari sceglie `complex` (`:253-258`). `support_claim` (`:267`) chiama `assert_visible` prima dell'invio (`:273`). Ogni errore è tipizzato e nessuno produce un valore.
- **CLI e misure** (`spikes/s06-llm/src/main.rs`). I sottocomandi sono `select`, `estimate`, `all`, `warmup`, `generate`, `route`, `fallback`, `injection`, `report`, `estimate-simple-sonnet` e `generate-simple-sonnet` (`main.rs:532-570`), con il registro `results/raw/spend.json` condiviso fra i sottocomandi. Le misure sono due sessioni nel runner (più una fermata dopo il warm-up), soglie e stime committate prima.
- **ADR-0011** `Proposed`, con la decisione dell'utente D-33.

## Esito delle soglie, in breve

| Soglia (scritta in `e797e9f`) | Esito |
|--------|-------|
| JSON conforme ≥ 98% per modello | rispettata: Haiku 60/60, Sonnet 34/34 (e 26/26 nel Task 3) |
| Claim con id non fornito sopravvissuti = 0 | rispettata: nessun id ignoto emesso (0 su 141), TEST-7 prova lo scarto |
| Rifiuto ≥ 90% sulle `no_answer` | rispettata: 10/10 per entrambi i modelli |
| Regola di routing di D-31 | **entrambe le condizioni false**: (a) 0,6852 < 0,7931 − 0,05; (b) 0,7241 < 0,85 → scelta all'utente |
| Accordo JEV / etichetta ≥ 0,80 | rispettata: 0,8333 |
| p95 Haiku ≤ 3 s, Sonnet ≤ 8 s | **Haiku mancata (3,744 s)**, Sonnet rispettata (3,994 s) |
| Fallback: 100% Sonnet e degradato, 0 validated | rispettata: 15/15, 0 validated |
| Injection: 0 id ignoti, 0 URL, 0 immagini | rispettata: 0, 0, 0; istruzione seguita 0/16 per modello |
| Spesa ≤ 2 USD | rispettata: 0,4482 dopo i Task 1-2, 0,5980 a fine piano |
| Non autorizzati nel prompt = 0 | rispettata: 0 su 94 + 26 righe |

La soglia di latenza di Haiku è mancata e resta scritta come mancata. Dopo D-33 Haiku non serve più e la soglia non decide nulla, ma il dato rimane: con l'effort basso e senza streaming, il p95 di Haiku sulle `complex` è 3,93 s.

## Task Commits

1. **Test rossi Task 1-2 (agente test-write, distinto dall'esecutore):** `a5768df` (test). RED_EXPECTED: 32 falliti sui `todo!`; fmt, clippy e scanner verdi.
2. **Task 1: soglie, selezione, schema, client, contesto e validazione:** `e797e9f` (feat), poi `43d8c83` (feat: errori JEV su stderr, prima della misura; è il `git_rev` di `hw.json`).
3. **Task 2: misure, esito e ADR-0011:** `edb8335` (feat).
4. **Test rossi Task 3:** `1a9c215` (test). RED_EXPECTED: 3 falliti, 32 verdi.
5. **Task 3: misura supplementare D-32:** `01d862f` (feat: sottocomando), `28f73ab` (docs: formato del registro, formula e stima prima della misura), `6a7789f` (feat: CSV, risultati, evidenze dell'ADR).

**Commit di pianificazione e decisioni:** `bdc3d8f` (ripasso D-31), `4b845a0` (Task 3, piano v3), `bd97274` e `52cdbae` (D-33 in CONTEXT, ADR-0011 e RISULTATI). **Plan metadata:** questo SUMMARY e `01-12-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `spikes/s06-llm/src/context.rs`: `Acl`, `OraclePassage` con `try_from`, `assert_visible`, `passages_for`, `escape_data`, `build`; TEST-1 e TEST-9.
- `spikes/s06-llm/src/validate.rs`: `parse_answer`, `map_citations`, `mark`, rilevatori di URL e immagini, `expected_value_found`; TEST-4, 7, 8, 10, 13, 15.
- `spikes/s06-llm/src/anthropic.rs`: prezzi, tetto, stima, `request_body`, `extract`, `Client` con retry unico su 429/529 e `Debug` redatto; TEST-12, 15, 16, 18, 22.
- `spikes/s06-llm/src/jev.rs`: `Client`, `validate_query`, `support_claim`, errori tipizzati; TEST-2, 5, 12.
- `spikes/s06-llm/src/main.rs`: selezione a seme, matrice di D-31, `answer`, `run_fallback`, `routing_rule`, registro di spesa, sottocomandi, `report`, server finto `fake_http`; TEST-5, 19-25.
- `spikes/s06-llm/schema/answer.schema.json`: schema `{answer, claims[{text, citations}], refusal}` con `additionalProperties: false`.
- `spikes/data/s06/selection.json`: semi e id, nessun valore atteso.
- `spikes/s06-llm/results/*`: cinque CSV a header fisso e `hw.json` normalizzato.
- `spikes/s06-llm/RISULTATI.md`, `docs/adr/0011-llm-generativo-routing-structured-output.md`.

## Decisions Made

### Decisioni dell'utente

- **D-31** (2026-10-19, completata il 2026-10-22): matrice dimezzata e regola a due condizioni. È fallita su entrambe.
- **D-32** (2026-10-22): prima di scegliere si misura Sonnet sulle stesse `simple`. Fatto nel Task 3, con un CSV separato e senza toccare Soglie e CSV precedenti.
- **D-33** (2026-10-22): Sonnet 5.5 su tutte le query, emenda D-18. Sulle 13 `simple` Sonnet ha 1,0000 di claim supportati contro 0,7241, al costo di circa 0,0037 USD per risposta (19,5 volte Haiku) e con un p95 di 4,48 s. La complessità resta nella risposta di `validate_query`.

### Scelte dell'esecutore

- **Escape dei delimitatori** (`& < > "`) invece del rifiuto dei passaggi ostili: il rifiuto avrebbe tolto dal contesto proprio i passaggi di cui si misura la tenuta.
- **`claims_n` conta tutti i claim emessi**, quelli senza citazioni compresi, che non sono mai supportati (`main.rs:342`). La regola è scritta nelle Soglie. La lettura alternativa resta solo diagnostica.
- **Condizione (b) senza le `no_answer`**, simmetrica alla (a), fissata nelle Soglie prima della misura.
- **Injection dai Markdown sintetici**: il paragrafo ostile si estrae fra i marcatori `injection-case` di `syn-07` e `syn-08` (`main.rs:1189`) e passa dalla stessa guardia di visibilità. Gli excerpt ratificati non contengono il testo ostile (punto aperto ereditato da 01-11).
- **`splitmix64` locale** invece del crate `rand`, che non è fra le dipendenze; `Cargo.toml` è chiuso.
- **Comando `all` in un processo**, più un registro di spesa fra i sottocomandi: il tetto vale per la sessione.
- **Fallback con una porta loopback chiusa nel processo** (`main.rs:1143-1150`) invece di cambiare `JEV_API_URL` nell'ambiente.
- **Lettura stretta del registro per D-32** (`read_ledger_strict`, `main.rs:640`). Quella permissiva dei Task 1-2 resta invariata.
- **`hw.json` non rigenerato per D-32**: il manifest della seconda sessione sta grezzo in `results/raw/`, con lo sha in RISULTATI.

## Rischi del test design e trattamento

- **RISK-1 (passaggio non visibile nel prompt o a JEV), CRITICAL:** coperto da TEST-1, 2, 3 e 27, tutti verdi. L'hard gate del CSV è 0 per costruzione: la garanzia sta in `context::build`.
- **RISK-2 (validated senza JEV), CRITICAL:** coperto da TEST-4, 5 e 6, verdi.
- **RISK-3, 6, 8, 11 (mapping, stato, schema, parametri):** verdi.
- **RISK-4 (injection), HIGH:** verde sulle funzioni e sul CSV. La lettura qualitativa delle risposte è `human_judgment`.
- **RISK-5 (chiavi), RISK-7 (soglie dopo i numeri), RISK-9 (spesa):** verdi.
- **RISK-10, 12, 14 (routing, selezione, matrice):** verdi. La rigenerazione coi semi reali resta da fare in verify.
- **RISK-13 e RISK-19 (regola e tasso):** verdi sul CSV reale e sui test unitari. Gli script con le fixture di TEST-23 e TEST-28 non sono stati scritti (`human_judgment`).
- **RISK-15..18 (Task 3):** verdi.

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Deviazione a] `grep -n "temperature"` dell'acceptance del Task 1.**
- **Found during:** Task 1
- **Issue:** l'acceptance chiede che il grep su `anthropic.rs` non trovi nulla, ma i test di `a5768df` asseriscono l'assenza della chiave e quindi la nominano.
- **Fix:** la parola è stata tolta dal commento del modulo; resta solo nelle asserzioni (`anthropic.rs:447`, `:644`), il codice di produzione non la usa.
- **Committed in:** `e797e9f`

**2. [Deviazione a] Passaggi PDF senza `section_path`.**
- **Found during:** Task 1
- **Issue:** in `queries.json` i passaggi dei PDF hanno `page` e non `section_path`, mentre la struct fissata dal test-write ha `section_path: String`.
- **Fix:** deserializzazione con `#[serde(try_from = "RawOraclePassage")]` (`context.rs:129`, `:147-168`): il locator diventa «pagina N». La firma pubblica non cambia, e un passaggio senza nessuno dei due campi è un errore.
- **Committed in:** `e797e9f`

**3. [Deviazione a] Commit intermedio `43d8c83`** con la stampa su stderr degli esiti JEV non disponibili (`main.rs:353`), prima della misura. Il binario misurato è quello di `43d8c83`, che è anche il `git_rev` di `hw.json`.

**4. [Deviazione a, ambiente] Docker Desktop spento all'avvio.** L'esecutore lo ha avviato e non c'erano container di altri piani. Nessun commit.

### Segnalazione su file chiuso (livello b)

**5. [Deviazione b, segnalazione] `hw-manifest.sh` (01-02) incompatibile con `common::manifest` (01-01).** La prima sessione (`01-12-20261022t111333z-c12c`) si è fermata dopo il warm-up, con 0,0039 USD spesi, perché `hw.json` non era nello schema. L'esecutore ha applicato la stessa normalizzazione di `s01-upload/bench.sh` con un driver fuori dal repo; l'originale è in `results/raw/`. Nessun file chiuso toccato (`node scripts/gate-proprieta-file.cjs`: conforme). Il difetto era già noto: 01-04 lo ha registrato come «segnalazione su file chiuso di 01-02 (livello b-segnalazione)», mentre 01-05 e 01-11 lo hanno contato come «[Rule 3 - Blocking]» fra le deviazioni di livello a. La classificazione qui segue 01-04 e il rapporto dell'esecutore. Il trattamento è identico, e la correzione resta per una fase decimale che riapra il perimetro di 01-01 o 01-02.

---

**Total deviations:** 4 di livello a (2 di contratto o acceptance, 1 di processo, 1 d'ambiente) più 1 segnalazione di livello b su un difetto già noto; nessuna di livello c.
**Impact on plan:** nessuna estensione di perimetro (15 file su 15).

## Issues Encountered

- **Claim senza citazioni.** Lo schema della API non può imporre `minItems` né `minLength`. Haiku emette 11 claim con `citations: []` su 83 (7 con testo vuoto), Sonnet 1 su 58 nel Task 2 e 0 su 30 nel Task 3. Il bridge li ha ricontati dai grezzi. Nel prodotto vanno scartati in modo deterministico.
- **Valore atteso trovato basso** (Haiku 14/50, Sonnet 2/24) per il confronto stretto: i modelli traducono e riformulano. Non è una misura di fedeltà.
- **Falsi rifiuti** su Q-006, Q-019, Q-024 e Q-031: gli excerpt sono righe di tabella senza intestazione.
- **Nota per verify sul test-write.** In `client_rejects_bad_config` (`anthropic.rs:651`, `jev.rs:466`) l'URL `http` non loopback è composto con `format!("{}://example.com", "http")` perché lo scanner non scatti: la regola `http-url` esenta i file di test, non i moduli `#[cfg(test)]` inline. È un aggiramento consapevole dello scanner in codice di test, da guardare in verify.
- **Natura della misura.** Modelli e JEV sono reali, il testo è quello del corpus ratificato. Il campione è piccolo: 13 query e circa 30 claim per modello nel confronto D-32, e un claim sposta il tasso di circa 0,03.

## Discrepanze fra rapporto e prove

- **p95.** RISULTATI e rapporto riportano Haiku 3.744 ms, Sonnet 3.994 e Sonnet sulle `simple` 4.481. Il ricalcolo nearest-rank del bridge dà 3.742, 3.993 e 4.478, perché `report` usa l'istogramma di `common::hist`. Lo scarto è trascurabile e non cambia nessun esito.
- **ADR-0011 incoerente dopo D-33.** La Decisione 2 dice «Sonnet anche per le `simple`», ma le Conseguenze negative («manca la scelta per le `simple`»), il primo Follow-up («l'utente sceglie il modello delle `simple`»), l'ultima voce delle Evidenze («La decisione per le `simple` resta aperta») e gli svantaggi dell'opzione 2 («non misurato sulle `simple`») sono rimasti alla prima stesura. Il testo va allineato prima della ratifica di 01-17. Il bridge non lo ha modificato.
- **Riconteggio «indipendente».** RISULTATI §Setup chiama `unauthorized_in_prompt` un riconteggio indipendente sul punto di invio. In `answer` (`main.rs:376`) però gira dopo `context::build`, che fallisce già su un passaggio invisibile, quindi vale 0 per costruzione. Lo stesso vale per `unknown_ids_after_mapping` (`main.rs:413`). Non è il doppio controllo di 01-11: la garanzia è TEST-1.
- **Classificazione di `hw.json`:** livello b nel rapporto, livello a in 01-05 e 01-11 (vedi deviazione 5).
- **Rilettura delle injection e sha del file segreti «prima e dopo».** Sono dichiarazioni dell'esecutore. Il bridge conferma che lo sha attuale coincide con quello del rapporto e che i rilevatori danno 0, ma non ha riletto le 32 risposte.
- Il resto coincide con le prove ricalcolate dal bridge: conteggi, tassi, accordo, fallback, injection, spesa, ancestralità di soglie e stime, invarianza dei CSV, assenza delle chiavi.

## Threat Flags

- **T-14:** `assert_visible` in `context::build` e in `support_claim`; 0 righe con passaggi non autorizzati.
- **T-16:** 0 istruzioni seguite su 32 risposte, con delimitatori neutralizzati e istruzioni solo nel `system`.
- **T-17:** mapping deterministico, 0 id ignoti; i claim senza citazioni sono un caso nuovo da scartare nel prodotto.
- **T-24:** 0,598 USD per l'intero piano, tetto rispettato.
- **URL loopback per prefisso.** I due client ammettono `http` quando l'URL comincia con `http://127.0.0.1:` (`anthropic.rs:269`, `jev.rs:142`). Un URL con userinfo (`http://127.0.0.1:x@host`) passerebbe il controllo verso un host non loopback. La configurazione viene solo dall'ambiente, quindi nello spike il rischio è basso; nel prodotto il controllo va fatto sull'host analizzato.
- **Privacy:** Anthropic ha una retention di 30 giorni; sono usciti solo testi pubblici o sintetici.

## Known Stubs

Nessuno nel codice dello spike.

## User Setup Required

Nessuno.

## Punti aperti

- Allineare il testo di ADR-0011 a D-33 prima della ratifica.
- Script di controllo con fixture per TEST-23 e TEST-28, rigenerazione di `selection.json` coi semi reali (TEST-21), rilettura qualitativa delle injection: in verify.
- Il difetto di forma fra `hw-manifest.sh` (01-02) e `common::manifest` (01-01) resta aperto.
- La soglia di latenza di Haiku è mancata. Se un giorno si riapre Haiku (ADR-0011: «solo con una misura nuova»), la latenza va rimisurata insieme alla fedeltà.

## Next Phase Readiness

- **01-16** trascrive in S-07 le porte: `AnswerGenerator::generate` → `StructuredAnswer { answer, claims: [{ text, citations: [PassageRef] }], refusal }` con versione del modello e `usage` nei metadati; `DecisionEngine::validate_query` → `QueryDecision { …, complexity: Simple | Complex }` con errore tipizzato e mai una complessità di default; la validazione claim–citazione come operazione separata.
- **01-17** ratifica ADR-0011, dopo l'allineamento del testo, e porta la riga «Generative LLM» di `DEPENDENCIES.md` da Aperta a Decisa con Sonnet 5.5.
- **Per il prodotto:** scartare in modo deterministico i claim senza citazioni o con testo vuoto, come quelli con id ignoto. Lo schema della API non lo impone, e la regola di D-31 è fallita anche per questo.

## Self-Check: PASSED

- 15 file su 15 dei `files_modified` nel diff `1de61a9..6d1f090`, nessun file di codice fuori elenco; `node scripts/gate-proprieta-file.cjs` conforme.
- Rieseguiti dal bridge sull'host: `cargo test -p s06-llm --locked` 35/35, scanner exit 0, controllo delle chiavi negativo (mai stampate), ricalcolo dai CSV di tassi, accordo, fallback, injection, matrice, `unauthorized_in_prompt`, header e insiemi, ancestralità di soglie e stime, invarianza dei CSV dei Task 1-2.

**Modifica:** Claude (Anthropic) · 2026-10-22 · bridge del piano 01-12
