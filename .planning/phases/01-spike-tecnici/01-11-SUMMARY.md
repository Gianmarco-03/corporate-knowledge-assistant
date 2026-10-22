---
phase: 01-spike-tecnici
plan: 11
subsystem: spike-jev-rank-concorrenza-pin
tags: [jev, typesafe, systemone, noul, choice, rerank, ndcg, recall, reqwest, rustls, retry-after, 429, 529, deadline, timeout_at, buffered, cost-guard, acl, assert_visible, pin, alias, adr]

# Dependency graph
requires:
  - phase: 01-01
    provides: "workspace degli spike, crate spikes/common (env::required, manifest, csv::Writer, timer::Stopwatch) e Cargo.toml di s04-jev con le dipendenze fissate"
  - phase: 01-02
    provides: "harness Compose chiuso, session.sh (with/oneshot/ps/down/release), hw-manifest.sh, runner Linux, file segreti esterno CKA_SPIKES_ENV"
  - phase: 01-05
    provides: "ADR-0010 (deadline esplicita, variante timeout_at, retry al più uno dentro la deadline), client JEV di riferimento, build offline con crate vendorizzate e normalizzazione di hw.json"
  - phase: 01-06
    provides: "corpus ratificato: acl.json, manifest.json, oracles/queries.json con i tre assi verdi (74 oracoli usati)"
provides:
  - "Crate s04-jev come libreria (jev, candidates, metrics) più CLI: contratto per 01-15 rank_form_a(query, passages, concurrency, deadline), Passage{id, doc_key, classification, text}, ndcg_at_k e recall_at_k"
  - "Client JEV con pin da JEV_MODEL senza default, Bearer da TYPESAFE_API_KEY, https_only, Debug redatto, retry unico su 429/529 solo se l'attesa sta nella deadline, errori tipizzati, contatore di costo da usage con arresto oltre il budget"
  - "Guardia di visibilità per ruolo sull'intero insieme prima del primo invio, più riconteggio indipendente sul punto di invio (unauthorized_sent)"
  - "30 insiemi di 20 candidati dal solo testo degli oracoli ratificati, seme 0x01115EED, mix 10 it / 10 en / 10 cross"
  - "Evidenze con soglie scritte prima: forma A nDCG@5 1,0 e recall@8 1,0 con p95 606 ms a concorrenza 16; B 0,679 e C 0,729; 0 risposte 429 e 529 a 4, 8 e 16; alias jev-latest → jev-1.13.0, jev-preview → jev-1.14.0; pin stabile su 3.803 risposte; unauthorized_sent 0 su 273 righe; spesa 0,078 USD su 5"
  - "ADR-0008 Proposed: forma A, concorrenza 16, retry entro deadline, pin jev-1.13.0, guardia di visibilità nell'adapter, margine di calibrazione ±0,1"
affects: [01-12, 01-15, 01-16, 01-17, fase 4 provider di decisione, DEPENDENCIES.md riga Modello testuale]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Guardia tutto-o-niente sull'insieme prima del fan-out, più un secondo conteggio indipendente sul punto di invio: due controlli che dovrebbero dare lo stesso numero, scritto in ogni riga"
    - "Retry come decisione di tempo: si attende retry-after solo se l'attesa finisce prima della deadline globale, altrimenti errore tipizzato e mai un segnale di default"
    - "Registro di spesa su file (results/raw/jev/spend.json) condiviso fra sottocomandi, così il tetto vale per la sessione e non per il processo"
    - "Grezzi jsonl per invocazione e sottocomando collect che scrive i CSV a header fisso"
    - "Wording delle domande in file versionati inclusi nel binario con include_str!, sha256 in ogni riga"

key-files:
  created:
    - spikes/s04-jev/RISULTATI-jev.md
    - spikes/s04-jev/questions/form-a.json
    - spikes/s04-jev/questions/form-b.json
    - spikes/s04-jev/questions/form-c.json
    - spikes/s04-jev/results/jev/hw.json
    - spikes/s04-jev/results/jev/forms.csv
    - spikes/s04-jev/results/jev/concurrency.csv
    - spikes/s04-jev/results/jev/aliases.csv
    - docs/adr/0008-jev-forma-rank-concorrenza-pin.md
  modified:
    - spikes/s04-jev/src/main.rs
    - spikes/s04-jev/src/lib.rs
    - spikes/s04-jev/src/jev.rs
    - spikes/s04-jev/src/candidates.rs
    - spikes/s04-jev/src/metrics.rs

key-decisions:
  - "Forma A per rank: una richiesta per candidato con Noul relevant, sufficient e instructs_model; nDCG@5 1,0 contro 0,729 (C) e 0,679 (B), p95 606 ms ≤ 2.000 (ADR-0008 §Decisione 1)"
  - "Concorrenza 16: il livello più alto con 0 risposte 429 su 3 run, secondo la regola pre-registrata; è il massimo misurato, non un limite trovato (ADR-0008 §Decisione 2)"
  - "Retry al più uno su 429/529 e solo entro la deadline (MAX_RETRIES da 2 a 1 in acbac93, allineato ad ADR-0010 Decisione 5)"
  - "Pin jev-1.13.0 da JEV_MODEL senza default; jev-latest oggi coincide ma è mobile, jev-preview è già 1.14.0"
  - "Varianza oltre soglia per 1 candidato su 600 (std 0,0954): per la regola scritta prima diventa un vincolo di calibrazione, margine ≥ ±0,1 attorno al taglio dei Noul"

patterns-established:
  - "Costruttore del client JEV con Acl: la guardia di visibilità vive dentro l'adapter, come difesa in profondità sul filtro di retrieval"
  - "Nessun errore JEV diventa un segnale: Unauthorized, RateLimited, Timeout, BudgetExceeded, Http, Other"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Nessun passaggio non visibile al ruolo della query viene inviato a JEV: il controllo è deterministico prima di ogni richiesta e il conteggio è un hard gate (D-16)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/s04-jev/src/candidates.rs:456#assert_visible_rejects_invisible_passage ; :477#assert_visible_rejects_unknown_document ; :486#assert_visible_uses_manifest_classification ; :494#is_visible_follows_roles_and_profiles", status: pass, evidence: "rieseguiti dal bridge sull'host: cargo test -p s04-jev --locked --offline 16 passed 0 failed"}
      - {test: TEST-2, risks: [RISK-1], ref: "spikes/s04-jev/src/jev.rs:904#send_path_blocks_before_network_on_invisible_passage", status: pass, evidence: "rieseguito dal bridge: forme A, B e C con l'invisibile al decimo posto → Unauthorized, server finto con 0 richieste, unauthorized_sent 0"}
      - {test: TEST-3, risks: [RISK-1, RISK-7], ref: "spikes/s04-jev/src/candidates.rs:532#candidate_sets_only_visible_to_role", status: pass, evidence: "rieseguito dal bridge sul corpus reale di spikes/data/corpus: verde"}
      - {test: TEST-4, risks: [RISK-1], ref: "spikes/s04-jev/results/jev/forms.csv colonna 15", status: pass, evidence: "ricalcolato dal bridge: awk -F, 'NR>1 && $15!=0' non stampa nulla; 91 righe 200 per forma (warm-up rep 0 + 30 query × 3), 273 in tutto"}
      - {test: TEST-17, risks: [RISK-12], ref: "grep danger_accept_invalid|http:// su spikes/s04-jev/src ; spikes/s04-jev/src/jev.rs:308-325", status: pass, evidence: "rieseguito dal bridge: grep vuoto fuori da 127.0.0.1; https_only(!loopback), verifica dei certificati di default"}
  - must_have: "La forma della chiamata JEV rank è misurata: una request per candidato con domande Noul (A), state unico con N Noul (B), una Choice (C), con nDCG@5, recall@8, latenza, token e varianza su 3 ripetizioni (D-22; criterio 4)"
    verification:
      - {test: TEST-13, risks: [RISK-8], ref: "spikes/s04-jev/src/metrics.rs:53#ndcg_recall_known_values ; :74#metrics_no_relevant_is_zero_not_nan", status: pass, evidence: "rieseguiti dal bridge: verdi (nDCG@5 ≈ 0,6241 sul caso noto, 0 e mai NaN senza rilevanti)"}
      - {test: TEST-14, risks: [RISK-9], ref: "spikes/s04-jev/results/jev/forms.csv colonna questions_sha256 ; spikes/s04-jev/questions/form-*.json", status: pass, evidence: "ricalcolato dal bridge: un solo sha per forma, uguale a sha256sum del file a HEAD (a e9b6a24b…, b a455735c…, c edcfbcf1…); tre JSON validi"}
      - {test: TEST-15, risks: [RISK-10, RISK-6], ref: "spikes/s04-jev/results/jev/*.csv ; spikes/s04-jev/results/jev/hw.json ; spikes/s04-jev/RISULTATI-jev.md §Setup", status: pass, evidence: "ricalcolato dal bridge: header esatti; forme a, b, c con rep 0..3; nDCG@5 A 1,0000 B 0,6790 C 0,7286, recall@8 1,0000 / 0,9500 / 0,9639, p95 A 606 ms nearest-rank su 90 rank: coincidono con RISULTATI; concurrency.csv livelli 4/8/16 × run 1..3, cost_usd cumulato 0,031993; hw.json JSON valido, sha 5fe5ec06…, tools.jev_model jev-1.13.0, digest del runner presente; stima 0,1477 USD in §Setup"}
      - {test: "TEST-15 (varianza per candidato)", risks: [RISK-10], ref: "spikes/s04-jev/results/jev/forms.csv colonna noul_relevant_std", status: human_judgment, human_judgment: true, rationale: "il CSV porta solo il massimo per query: il bridge conferma che nella forma A solo Q-066 supera 0,05 (0,0954) e che B (max 0,3323) e C (max 0,1852) hanno più query sopra soglia. Media, p95 e il conteggio «1 su 600» (32 per B, 11 per C) vengono dai grezzi jsonl per candidato in results/raw/jev/, non versionati e assenti dal main: non ricalcolabili"}
  - must_have: "La concorrenza 4/8/16 è misurata con il conteggio dei 429 e dei 529 e il rispetto di retry-after; la risoluzione degli alias jev-latest, jev-1.13.0, jev-preview è registrata da response.model (D-22)"
    verification:
      - {test: TEST-7, risks: [RISK-3], ref: "spikes/s04-jev/src/jev.rs:976#retry_after_honoured_within_deadline", status: pass, evidence: "rieseguito dal bridge: per 429 e 529, due richieste a distanza ≥ 1 s, retries = 1, esito 200"}
      - {test: TEST-8, risks: [RISK-3], ref: "spikes/s04-jev/src/jev.rs:1008#rate_limited_beyond_deadline_is_typed_error_not_signal ; :1031#silent_server_is_timeout_within_deadline", status: pass, evidence: "rieseguiti dal bridge: RateLimited entro deadline + 100 ms con una sola richiesta; Timeout entro deadline + 100 ms con server muto"}
      - {test: "concurrency.csv", risks: [RISK-10], ref: "spikes/s04-jev/results/jev/concurrency.csv", status: pass, evidence: "riletto dal bridge: 9 righe, 200/200 ok in ognuna, status_429 0, status_529 0, other_errors 0, retries 0. Nessun 429 reale: il ramo retry-after è provato solo dai test locali TEST-7 e TEST-8"}
      - {test: "aliases.csv", risks: [RISK-5], ref: "spikes/s04-jev/results/jev/aliases.csv", status: pass, evidence: "riletto dal bridge: jev-latest → jev-1.13.0, jev-1.13.0 → jev-1.13.0, jev-preview → jev-1.14.0, tutti 200; request_id vuoto (JEV non manda l'header, deviazione 5)"}
  - must_have: "JEV è chiamato con il modello pinnato jev-1.13.0 da configurazione e la versione risolta è registrata in ogni riga (D-17)"
    verification:
      - {test: TEST-10, risks: [RISK-5], ref: "spikes/s04-jev/src/jev.rs:1053#request_uses_configured_model_and_records_resolved ; :1086#client_requires_configured_model", status: pass, evidence: "rieseguiti dal bridge: model del corpo = configurazione, model_resolved restituito; senza JEV_MODEL la costruzione fallisce"}
      - {test: TEST-11, risks: [RISK-5], ref: "spikes/s04-jev/results/jev/forms.csv colonna model_resolved ; aliases.csv", status: pass, evidence: "ricalcolato dal bridge: 273 righe su 273 con model_resolved jev-1.13.0, uguale alla risoluzione del pin in aliases.csv. Le 1.800 richieste di concorrenza sono riportate a jev-1.13.0 da RISULTATI: concurrency.csv non ha la colonna, il dato per richiesta sta nei grezzi"}
  - must_have: "Chiave, budget e testo inviato (threat model T-01-11-02, T-01-11-03, T-01-11-04)"
    verification:
      - {test: TEST-5, risks: [RISK-2], ref: "grep TYPESAFE_API_KEY spikes/s04-jev/src ; grep -rqF del valore ; python skills/security-scanner/security_scan.py spikes/s04-jev", status: pass, evidence: "rieseguito dal bridge: TYPESAFE_API_KEY solo in common::env::required (jev.rs:86) e nel set_var di una chiave fittizia del test (jev.rs:1097); il valore reale, letto dal file segreti, non compare in spikes/s04-jev, ADR-0008 né nei due rapporti tmp (grep -q, mai stampato); scanner «OK su 11 file», exit 0"}
      - {test: TEST-6, risks: [RISK-2], ref: "spikes/s04-jev/src/jev.rs:936#client_errors_and_debug_redact_key", status: pass, evidence: "rieseguito dal bridge: Debug ed errore senza la sentinella; il server finto riceve Bearer sk-test-REDACT-ME"}
      - {test: TEST-12, risks: [RISK-6], ref: "spikes/s04-jev/src/jev.rs:1112#cost_of_one_million_input_tokens ; :1123#cost_guard_stops_session_over_budget", status: pass, evidence: "rieseguiti dal bridge: 1 Mtok di input = 0,042 USD; oltre il budget la chiamata è rifiutata senza invio"}
      - {test: TEST-9, risks: [RISK-4], ref: "git merge-base --is-ancestor 28d8ea7 8f1bb68 ; sezione Soglie fra 28d8ea7 e HEAD", status: pass, evidence: "rieseguito dal bridge: 28d8ea7 (17:22) antenato di 8f1bb68, primo commit che aggiunge results/jev/; tabella e regole di lettura delle Soglie identiche; le sessioni partono alle 15:25:12Z, cioè dopo 28d8ea7"}
      - {test: TEST-16, risks: [RISK-11], ref: "spikes/s04-jev/results/jev/forms.csv colonna instructs_model_flagged ; spikes/s04-jev/RISULTATI-jev.md §Anomalie", status: human_judgment, human_judgment: true, rationale: "coperto solo in parte. La colonna è valorizzata (0 su tutte le 91 righe di forma A, «na» su B e C) e il bridge conferma 0 flag su 1.800 invii; ma gli oracoli injection_case ratificano i fatti legittimi di syn-07 e syn-08, non le istruzioni ostili, e il piano vieta estrazioni nuove: il testo ostile non è mai arrivato a JEV. Si è misurato solo il tasso di falsi positivi (0/1.800, massimo 0,20); il rilevamento sul testo ostile non è verificabile con gli oracoli attuali e resta aperto per 01-12 o per un oracolo nuovo"}
      - {test: "ADR-0008 (ratifica)", ref: "docs/adr/0008-jev-forma-rank-concorrenza-pin.md Stato", status: human_judgment, human_judgment: true, rationale: "l'ADR è Proposed: la ratifica dell'utente e la riga «Modello testuale» di DEPENDENCIES.md (jev-latest → jev-1.13.0) spettano al consolidamento 01-17. La misura è contro JEV reale con testo del corpus ratificato (74 oracoli verdi), su 30 query: la generalizzazione a un corpus vero e a query concorrenti di più utenti è da giudicare"}

# Metrics
duration: "circa 30 min di calendario (test rossi 17:08:12 → ultimo commit 17:38:11, 2026-10-21); merge su main 18:28:14"
completed: 2026-10-21
---

# Fase 1 Piano 11: S-04, forma del rank JEV, concorrenza, pin e alias · Summary

**Una richiesta JEV per candidato (forma A) ordina perfettamente i 30 insiemi ratificati, nDCG@5 1,0 e recall@8 1,0 con p95 606 ms a concorrenza 16, contro 0,679 della forma B e 0,729 della C; a 4, 8 e 16 richieste in volo nessun 429 né 529, il pin `jev-1.13.0` risolve a sé stesso su 3.803 risposte, nessun passaggio non visibile è partito (0 su 273 righe) e la spesa è stata 0,078 USD su 5; ADR-0008 propone forma A, concorrenza 16, retry unico entro la deadline e un margine di calibrazione di ±0,1, mentre il rilevamento di `instructs_model` sul testo ostile resta da misurare**

## Performance

- **Duration:** circa 30 minuti di calendario. I test rossi `0c338cb` (17:08:12, agente test-write separato) precedono le soglie `28d8ea7` (17:22:23). L'ultimo commit del piano è `a5785c6` (17:38:11); il merge su main è `02a26c4` (18:28:14).
- **Started:** 2026-10-21T17:08:12+02:00
- **Completed:** 2026-10-21T17:38:11+02:00
- **Tasks:** 2 di 2
- **Files modified:** 14, esattamente i `files_modified` del piano (`git diff --stat f123d45 a5785c6`). `lib.rs`, `candidates.rs`, `jev.rs` e `metrics.rs` sono nati come stub del test-write in `0c338cb`; `main.rs` era lo stub di 01-01.

## Accomplishments

- Oracolo di visibilità `Acl` da `acl.json` e `manifest.json` (`spikes/s04-jev/src/candidates.rs:53`), deny by default: documento ignoto, ruolo ignoto o classificazione assente danno «non visibile» (`candidates.rs:116-124`). Decide la classificazione del manifest, non quella dichiarata dal passaggio.
- `assert_visible` (`candidates.rs:153`) chiamata due volte: in costruzione dell'insieme (`candidates.rs:400`) e nel client, sull'intero insieme, prima del primo invio (`jev.rs:352`, `:528`, `:582`, `:616`). In più un riconteggio indipendente sul punto di invio (`jev.rs:361`, `:423`).
- Client JEV (`jev.rs:306`): pin obbligatorio, `https_only` salvo `127.0.0.1`, `connect_timeout` 2 s, `Debug` redatto (`jev.rs:684`), retry unico su 429/529 entro la deadline (`jev.rs:443-466`), errori tipizzati (`jev.rs:158`), costo da `usage` (`jev.rs:224`, `:480`) con arresto oltre il budget (`jev.rs:371`).
- Forme A, B e C (`jev.rs:541`, `:576`, `:610`); `rank_form_a_each` (`jev.rs:521`) per le misure.
- 30 insiemi da 20 passaggi, seme `0x01115EED`, solo testo degli oracoli verdi (`candidates.rs:290`).
- CLI `sets`, `estimate`, `aliases`, `concurrency`, `forms`, `collect` (`main.rs:169`), registro di spesa fra sottocomandi (`main.rs:244-279`).
- Due sessioni reali contro JEV, 3.805 richieste, soglie committate prima; RISULTATI completo; ADR-0008 `Proposed`.

## Esito delle soglie, in breve

| Soglia | Esito |
|--------|-------|
| Forma A resta il default se nDCG@5(A) ≥ migliore fra B e C e p95 ≤ 2.000 ms | rispettata: A 1,0000 contro C 0,7286 e B 0,6790; p95 606 ms a concorrenza 16 |
| Concorrenza = livello più alto con 0 risposte 429 su 3 run | 16 (0 risposte 429 e 0 risposte 529 a tutti e tre i livelli) |
| Std del Noul `relevant` per candidato ≤ 0,05 | superata da 1 candidato su 600 nella forma A (0,0954): per la regola scritta prima diventa vincolo di calibrazione (±0,1) |
| `jev-1.13.0` risolve a sé stesso | rispettata su 3.803 risposte |
| `unauthorized_sent` = 0 (hard gate) | rispettata: 0 su 273 righe |
| Spesa ≤ `S04_MAX_COST_USD` | 0,078 USD su 5 (stima 0,148) |

## Task Commits

1. **Test rossi (agente test-write, distinto dall'esecutore):** `0c338cb` (test). RED_EXPECTED: 16 falliti sui `todo!` degli stub; fmt, clippy e scanner verdi.
2. **Task 1: soglie, wording, client e candidati:** `28d8ea7` (feat), poi `acbac93` (fix: `MAX_RETRIES` da 2 a 1, prima della seconda sessione).
3. **Task 2: misure, esito e ADR:** `8f1bb68` (feat: CSV, `hw.json`, RISULTATI), `f9b02b7` (docs: ADR-0008), `a5785c6` (fix: grezzi spostati in `results/raw/jev/`).

**Plan metadata:** questo SUMMARY e `01-11-RACCONTO.md`, scritti dal bridge. Merge su main `02a26c4`.

## Files Created/Modified

- `spikes/s04-jev/src/candidates.rs`: `Acl`, `assert_visible`, `SplitMix64`, `is_green`, `build_candidate_sets`; test TEST-1 e TEST-3.
- `spikes/s04-jev/src/jev.rs`: `ClientConfig`, `Client`, `CallMeta`, `JevError`, forme A/B/C, costo e budget; test TEST-2, 6, 7, 8, 10, 12 con un server finto su loopback.
- `spikes/s04-jev/src/metrics.rs`: nDCG@k e recall@k binari; test TEST-13.
- `spikes/s04-jev/src/lib.rs`: i tre moduli pubblici.
- `spikes/s04-jev/src/main.rs`: CLI, registro di spesa, grezzi jsonl, `collect`.
- `spikes/s04-jev/questions/form-{a,b,c}.json`: wording versionato.
- `spikes/s04-jev/results/jev/*`: tre CSV a header fisso e `hw.json` normalizzato.
- `spikes/s04-jev/RISULTATI-jev.md`, `docs/adr/0008-jev-forma-rank-concorrenza-pin.md`: esito e decisione proposta.

## Decisions Made

- **Forma A.** Vince per qualità con margine (0,27 di nDCG@5 su C), sta sotto il tetto di latenza ed è la più stabile. Costa circa 3,8 volte i token di B e 6,2 volte quelli di C, ma in assoluto circa 0,00035 USD per query.
- **Concorrenza 16**, come da regola. Non è il limite di JEV: è il livello più alto misurato.
- **Retry unico, solo entro la deadline**, allineato ad ADR-0010. In entrambe le sessioni non c'è stato nessun 429, nessun 529 e nessun retry.
- **Ordine delle misure invertito** (alias, concorrenza, forme): la soglia di latenza di A si legge alla concorrenza scelta, che quindi serviva prima.
- **Al più 2 excerpt `injection_case` per insieme**, per non trasformare un insieme in un insieme ostile (fino a 8 su 20 sarebbero stati visibili).
- **`splitmix64` locale** invece del crate `rand`, che non è fra le dipendenze di `s04-jev`; `Cargo.toml` è fuori dai `files_modified`.

## Rischi del test design e trattamento

- **RISK-1 (passaggio non visibile a JEV), CRITICAL:** coperto su quattro livelli, tutti verdi: funzione (TEST-1), percorso di invio (TEST-2), insiemi costruiti (TEST-3), evidenza (TEST-4).
- **RISK-2 (chiave), HIGH:** TEST-5 e TEST-6 verdi, rieseguiti dal bridge senza stampare la chiave.
- **RISK-3 (segnale inventato o retry oltre la deadline), HIGH:** TEST-7 e TEST-8 verdi sul server finto. Con JEV reale il ramo non è stato esercitato (0 risposte 429).
- **RISK-4 (soglie dopo i numeri), HIGH:** TEST-9 verde.
- **RISK-5 (pin), HIGH:** TEST-10 e TEST-11 verdi.
- **RISK-6..10, 12:** verdi; la parte per candidato della varianza è `human_judgment` perché i grezzi non sono versionati.
- **RISK-11 (injection), MEDIUM:** TEST-16 **coperto solo in parte**: misurati solo i falsi positivi.

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Deviazione a] `Acl` nel costruttore del client.** `Client::new(ClientConfig, Acl)` e `Client::new_from_env(Acl)`, mentre `<interfaces>` del piano scrive `new_from_env` senza argomenti. La guardia deve stare nel client e il ruolo arriva con la `Query`. `01-CONTRATTO.md` non fissa la firma del costruttore e `rank_form_a(query, passages, concurrency, deadline)` resta quella promessa. **Nota per 01-15:** `let client = Client::new_from_env(Acl::load(corpus)?)?;`. La firma era già negli stub del test-write (`0c338cb`).

**2. [Deviazione a] `CallMeta` più ricco e `rank_form_a_each`.** In più rispetto al contratto: `output_tokens`, `retry_after`, `retries`, `throttled_429`, `throttled_529`, `request_id` (`jev.rs:117-131`), e il metodo `rank_form_a_each` (`jev.rs:521`). Aggiunte compatibili con il consumo di 01-15.

**3. [Deviazione a] Retry da 2 a 1** (`acbac93`). Il piano non fissa il numero, ADR-0010 dice «al massimo una volta». La prima sessione girava col tetto a 2, ma ha avuto 0 retry: i dati non cambiano.

**4. [Rule 3 - Blocking] Grezzi spostati in `results/raw/jev/`** (`a5785c6`). Durante le sessioni stavano in `results/jev/raw/`, che il pattern `**/results/raw/` di `spikes/.gitignore` (01-02, chiuso) non copre. `collect` rieseguito dai grezzi spostati dà CSV identici. Il campo `normalized_by` di `hw.json` cita ancora il percorso vecchio.

**5. [Deviazione a] `request_id` vuoto in `aliases.csv`.** JEV non manda `x-request-id` né `request-id`.

**6. [Rule 3 - Blocking] `hw.json` normalizzato** con la stessa trasformazione di 01-04 e 01-05 (difetto noto fra `hw-manifest.sh` di 01-02 e `common::manifest` di 01-01). Nessun file chiuso toccato.

**7. [Rule 3 - Blocking] Build offline da vendor** in `spikes/target/vendor-s04` (ignorata), perché il DNS del runner non risolve `index.crates.io`. `Cargo.lock` invariato.

**8. [Deviazione a] `instructs_model` misurato solo sui falsi positivi** (vedi TEST-16 e «Issues Encountered»).

### Integrazione fuori ordine

`01-ONDE.md` (onda 4) chiede «Merge 01-07 poi 01-11». 01-11 è stato fuso per primo, per **decisione utente del 2026-10-21**, perché 01-07 è fermo su tre blocchi esterni al piano (messaggio del merge `02a26c4`). I due piani hanno file disgiunti; il gate d'onda sul main fuso è verde.

---

**Total deviations:** 8 di livello a (3 d'ambiente o di repo, 5 di contratto o misura) più l'ordine di integrazione deciso dall'utente; nessuna di livello b né c.
**Impact on plan:** nessuna estensione di perimetro (14 file su 14).

## Issues Encountered

- **`instructs_model` non misurabile sul testo ostile.** Gli oracoli `injection_case` (Q-019, Q-020, Q-055, Q-056, Q-058, Q-070) hanno come `expected_passages` i fatti legittimi di `syn-07-injection-public` e `syn-08-injection-internal-en`, non le istruzioni iniettate. Con la regola «solo testo ratificato, nessuna estrazione nuova» il testo ostile non arriva mai a JEV: 0 flag su 1.800 invii è un tasso di falsi positivi, non di rilevamento.
- **Grezzi per candidato non versionati.** Media e p95 della std per candidato, il conteggio dei candidati sopra 0,05, la ripartizione 258/1.542 degli invii injection e il `model_resolved` delle 1.800 richieste di concorrenza vengono da `results/raw/jev/`, assente dal main. Il bridge ha ricalcolato ciò che sta nei CSV.
- **Nessun 429 reale.** Il comportamento con `retry-after` è provato solo sul server finto.
- **Natura della misura.** JEV è reale, il testo è quello del corpus ratificato (74 oracoli verdi, documenti pubblici o sintetici). La concorrenza è misurata per una query alla volta: un carico con query di più utenti non è stato misurato.

## Discrepanze fra rapporto e prove

- La firma `new_from_env(acl)` è presentata nel rapporto come scelta dell'esecutore; in realtà stava già negli stub del test-write (`0c338cb`), e l'esecutore l'ha mantenuta.
- ADR-0008 §Decisione 2 cita «`buffered`/`buffer_unordered`»; il codice usa `buffered` (`jev.rs:532`), che mantiene l'ordine dei candidati. La variante di default di ADR-0010 è `buffer_unordered`; la differenza non cambia la cancellazione.
- RISULTATI e ADR scrivono «3.803 risposte del pin»: è coerente con il totale di 3.805 richieste (1.800 + 1.820 + 91 + 91 + 3 alias), perché due alias non sono il pin.
- Il resto (conteggi, nDCG, recall, p95, alias, sha, ancestralità delle soglie, assenza della chiave) coincide con le prove ricalcolate dal bridge.

## Threat Flags

- **T-14:** guardia tutto-o-niente più riconteggio, `unauthorized_sent` = 0 su ogni riga.
- **T-16:** passaggi come `state`; rilevamento dell'injection ostile non misurato.
- **T-24:** 0 risposte 429 fino a 16 in volo; tetto di spesa per sessione.
- **Privacy:** JEV è negli Stati Uniti con retention non limitata; sono usciti solo testi pubblici o sintetici.

## Known Stubs

Nessuno nel codice dello spike.

## User Setup Required

Nessuno.

## Punti aperti

- Rilevamento di `instructs_model` sul testo ostile: in 01-12 o con un oracolo che ratifichi gli excerpt ostili.
- Concorrenza con query concorrenti di più utenti: il rate limit dell'organizzazione non è pubblicato.
- `normalized_by` di `hw.json` con il percorso vecchio; difetto di forma fra `hw-manifest.sh` (01-02) e `common::manifest` (01-01) ancora aperto.
- Prima di passare a `jev-1.14.0`: nuova misura di qualità e varianza.

## Next Phase Readiness

- 01-15 può consumare `rank_form_a` a concorrenza 16, costruendo il client con `Acl::load`.
- 01-12 deve contare la spesa con lo stesso criterio e può riprendere la misura dell'injection.
- 01-16 trascrive in S-07 `Reranker::rank` con deadline esplicita, metadati di versione ed errori tipizzati.
- 01-17 ratifica ADR-0008 e porta «Modello testuale» a `jev-1.13.0` in `DEPENDENCIES.md`.

## Self-Check: PASSED

- 14 file su 14 dichiarati nel diff `f123d45..a5785c6`, nessuno fuori elenco; `node scripts/gate-proprieta-file.cjs` conforme.
- Rieseguiti dal bridge sull'host: `cargo test -p s04-jev --locked --offline` 16/16, scanner exit 0, controllo della chiave negativo (mai stampata), ricalcolo di nDCG, recall, p95, `unauthorized_sent`, `model_resolved` e sha dai CSV. Il gate d'onda sul main fuso (fmt, clippy, test del workspace degli spike, compose config, proprietà dei file, segreti) è verde.

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-11
