# Test design 01-11 — S-04/A-09 forma del rank JEV, concorrenza e pin

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-11-PLAN.md
  sha256: f6d0118353964301f4f1dfb47f6902046f7499d0d0f34422a447755fd33777da
risks:
  - id: RISK-1
    severity: CRITICAL
    title: Passaggio non visibile al role_profile inviato a JEV (assert_visible assente dal percorso di invio, aggirabile o con verità sbagliata)
    source: threat_model T-01-11-01
  - id: RISK-2
    severity: HIGH
    title: TYPESAFE_API_KEY in log, errori, CSV o RISULTATI
    source: threat_model T-01-11-02
  - id: RISK-3
    severity: HIGH
    title: Su 429/529 o timeout il client inventa un segnale positivo o ritenta oltre la deadline
    source: must_have "conteggio dei 429 e dei 529 e il rispetto di retry-after"; Task 1 action
  - id: RISK-4
    severity: HIGH
    title: Soglie scritte o ritoccate dopo la misura
    source: Task 1 acceptance "Soglie committate prima di ogni file in results/jev/"
  - id: RISK-5
    severity: HIGH
    title: Modello non pinnato o versione risolta non registrata in ogni riga
    source: must_have "JEV è chiamato con il modello pinnato jev-1.13.0 ... versione risolta registrata in ogni riga"
  - id: RISK-6
    severity: MEDIUM
    title: Spesa oltre S04_MAX_COST_USD (contatore assente o calcolato male)
    source: threat_model T-01-11-03
  - id: RISK-7
    severity: MEDIUM
    title: Candidati costruiti da oracoli non ratificati o da testo non presente negli oracoli
    source: Task 1 action candidates.rs
  - id: RISK-8
    severity: MEDIUM
    title: nDCG@5 / recall@8 calcolati male, scelta della forma falsata
    source: must_have "nDCG@5, recall@8"
  - id: RISK-9
    severity: MEDIUM
    title: Wording delle domande cambiato durante la sessione senza traccia (questions_sha256 non coerente con i file)
    source: Task 1 action wording versionato
  - id: RISK-10
    severity: MEDIUM
    title: Evidenze incomplete (forme, livelli, ripetizioni, alias, header, hw.json)
    source: must_have forms/concurrency/aliases
  - id: RISK-11
    severity: MEDIUM
    title: Segnali JEV su excerpt injection non misurati (instructs_model mai valutato)
    source: threat_model T-01-11-04
  - id: RISK-12
    severity: MEDIUM
    title: Verifica TLS disattivata
    source: threat_model T-01-11-05
tests:
  - {id: TEST-1, title: assert_visible_rejects_invisible_passage, type: cargo-unit, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: send_path_blocks_before_network_on_invisible_passage, type: cargo-integration-locale, priority: P0, catches: [RISK-1]}
  - {id: TEST-3, title: candidate_sets_only_visible_to_role, type: cargo-unit, priority: P0, catches: [RISK-1, RISK-7]}
  - {id: TEST-4, title: Hard gate unauthorized_sent = 0 e somma coerente col numero di invii, type: check-evidenze, priority: P0, catches: [RISK-1]}
  - {id: TEST-5, title: La chiave non compare in output e artefatti, type: check-statico+runtime, priority: P0, catches: [RISK-2]}
  - {id: TEST-6, title: client_errors_and_debug_redact_key, type: cargo-unit, priority: P0, catches: [RISK-2]}
  - {id: TEST-7, title: retry_after_honoured_within_deadline, type: cargo-integration-locale, priority: P0, catches: [RISK-3]}
  - {id: TEST-8, title: rate_limited_beyond_deadline_is_typed_error_not_signal, type: cargo-integration-locale, priority: P0, catches: [RISK-3]}
  - {id: TEST-9, title: Soglie committate prima dei risultati e invariate, type: check-git, priority: P0, catches: [RISK-4]}
  - {id: TEST-10, title: request_uses_configured_model_and_records_resolved, type: cargo-integration-locale, priority: P0, catches: [RISK-5]}
  - {id: TEST-11, title: model_resolved valorizzato su ogni riga 200 e pin confermato in aliases.csv, type: check-evidenze, priority: P1, catches: [RISK-5]}
  - {id: TEST-12, title: cost_guard_stops_session_over_budget, type: cargo-unit, priority: P1, catches: [RISK-6]}
  - {id: TEST-13, title: ndcg_recall_known_values, type: cargo-unit, priority: P1, catches: [RISK-8]}
  - {id: TEST-14, title: questions_sha256 coincide con i file versionati, type: check-evidenze, priority: P2, catches: [RISK-9]}
  - {id: TEST-15, title: Validità delle evidenze, type: check-evidenze, priority: P1, catches: [RISK-10, RISK-6]}
  - {id: TEST-16, title: Copertura injection e instructs_model_flagged valorizzato, type: check-evidenze, priority: P2, catches: [RISK-11]}
  - {id: TEST-17, title: Nessun bypass TLS, type: check-statico, priority: P2, catches: [RISK-12]}
uncovered: []
```

## Come leggere questo design

Questo spike manda testo a un provider esterno, quindi il rischio CRITICAL è quello che la regola di sicurezza del progetto mette al primo posto: nessun passaggio non autorizzato raggiunge JEV. Il piano mette `assert_visible` «immediatamente prima di ogni richiesta» e conta `unauthorized_sent`. I test verificano che la guardia stia davvero nel percorso di invio (TEST-2, con un server finto che conta le richieste ricevute) e non solo che la funzione esista. Il secondo gruppo di rischi riguarda la correttezza del client in condizioni di errore: con 429 o timeout il client non deve produrre un segnale positivo, perché nel prodotto un segnale JEV inventato equivale a un controllo bypassato.

Il crate `s04-jev` ha `lib.rs`, quindi i test unitari stanno nei moduli `#[cfg(test)]` di `src/jev.rs`, `src/candidates.rs`, `src/metrics.rs`. I test «locali» avviano un piccolo server HTTP su `127.0.0.1:0` con `tokio::net::TcpListener` (il crate non ha `wiremock` fra le dipendenze e questo piano non tocca `Cargo.toml`): il server risponde con status e header scelti dal test e conta le richieste. Il client deve accettare l'URL base come parametro, già previsto dal piano (`JEV_API_URL`). Comando singolo: `cd spikes && cargo test -p s04-jev <nome>`.

## Test

**TEST-1 — `assert_visible_rejects_invisible_passage`** (in `candidates.rs`). Mappa di visibilità costruita da un `acl.json` e un manifest minimi in memoria: un passaggio di un documento `restricted` con `role_profile = employee` → `Err` che nomina il documento; tutti i passaggi visibili → `Ok`; un passaggio con `document_id` sconosciuto al manifest → `Err` (deny by default, mai «visibile perché non trovato»).

**TEST-2 — `send_path_blocks_before_network_on_invisible_passage`** (in `jev.rs`). Server locale che conta le richieste. Si invoca la funzione pubblica di rank (forma A, B e C) con un insieme di 20 passaggi di cui uno invisibile al ruolo: la chiamata restituisce errore, il server riceve 0 richieste (per la forma A, nemmeno quelle dei passaggi visibili precedenti: la guardia vale sull'insieme prima del primo invio) e il contatore `unauthorized_sent` resta 0 perché nulla è partito. È il test che rende vera l'acceptance «`assert_visible` è chiamata nel percorso di invio».

**TEST-3 — `candidate_sets_only_visible_to_role`** (in `candidates.rs`). Su `queries.json` e `acl.json` reali (letti da `spikes/data/corpus/`, input condivisi): per ognuna delle 30 query selezionate con il seme registrato, ogni passaggio del suo insieme di 20 appartiene a un documento visibile al suo `role_profile`, ogni testo è un `excerpt` di un oracolo con tre assi verdi, e l'insieme contiene gli `expected_passages` della query. Seme fisso: due costruzioni consecutive producono gli stessi insiemi.

**TEST-4 — Hard gate sul CSV.** `awk -F, 'NR>1 && $15!=0' spikes/s04-jev/results/jev/forms.csv` non stampa nulla. In più il numero di righe con `status` 200 per forma è coerente col numero di invii atteso (30 query × 3 ripetizioni più il warm-up dichiarato), così un CSV che non registra le righe non può nascondere un invio.

**TEST-5 — La chiave non trapela.** `grep -rn "TYPESAFE_API_KEY" spikes/s04-jev/src` solo nella lettura con `common::env::required`; a runtime, con la chiave caricata, `grep -rqF "$TYPESAFE_API_KEY" spikes/s04-jev docs/adr/0008-*.md` non trova nulla; scanner su `s04-jev` exit 0.

**TEST-6 — `client_errors_and_debug_redact_key`** (in `jev.rs`). Client costruito con chiave fittizia `"sk-test-REDACT-ME"` passata come parametro; risposta 401 dal server locale con corpo che non contiene la chiave; né l'errore (`{:#}`) né il `Debug` del client contengono `REDACT-ME`. Il server locale verifica inoltre di aver ricevuto `Authorization: Bearer sk-test-REDACT-ME` (la chiave va dove deve andare).

**TEST-7 — `retry_after_honoured_within_deadline`** (in `jev.rs`). Il server risponde 429 con `retry-after: 1` alla prima richiesta e 200 alla seconda. Con deadline 3 s: esito positivo, 2 richieste ricevute, intervallo fra le due ≥ 1 s, `retries = 1` registrato. Variante con 529: stesso comportamento.

**TEST-8 — `rate_limited_beyond_deadline_is_typed_error_not_signal`** (in `jev.rs`). Il server risponde sempre 429 con `retry-after: 5`; deadline 1 s. Esito: errore tipizzato (variante `RateLimited` o equivalente), nessun valore di rilevanza restituito, chiamata conclusa entro deadline + 100 ms, nessuna seconda richiesta oltre la deadline. Variante con server che non risponde mai: errore `Timeout` entro la deadline. Nessun ramo restituisce `relevant = true` o un punteggio di default.

**TEST-9 — Soglie prima delle misure.** Il commit che introduce «## Soglie (scritte PRIMA della misura)» in `RISULTATI-jev.md` è antenato del primo commit che aggiunge file in `results/jev/`; la sezione è identica fra quel commit e HEAD; contiene la regola su forma A, il tetto p95 2.000 ms, la regola sulla concorrenza (0 risposte 429 su 3 run), la varianza ≤ 0,05, il pin `jev-1.13.0`, `unauthorized_sent = 0` e il budget.

**TEST-10 — `request_uses_configured_model_and_records_resolved`** (in `jev.rs`). Il server locale cattura il corpo: `model` è il valore passato da configurazione (`jev-1.13.0` nel test) e la risposta finta con `"model": "jev-1.13.0-build-x"` viene restituita come `model_resolved` nella struttura di esito. Senza `JEV_MODEL` configurato la costruzione del client fallisce (nessun default a `jev-latest`).

**TEST-11 — Pin nelle evidenze.** Ogni riga di `forms.csv` e `concurrency.csv` con status 200 ha `model_resolved` non vuoto; in `forms.csv` tutte le righe 200 hanno `model_resolved` uguale a quello risolto da `jev-1.13.0` in `aliases.csv`; `aliases.csv` ha le tre righe `jev-latest`, `jev-1.13.0`, `jev-preview` con `model_resolved` o status d'errore.

**TEST-12 — `cost_guard_stops_session_over_budget`** (in `jev.rs`). Contatore con prezzo costante e budget 0,001 USD; si registrano `usage` simulati finché la somma supera il budget: la chiamata successiva è rifiutata con errore di budget senza inviare (server locale con 0 richieste aggiuntive). Calcolo: 1.000.000 token input × 0,042 USD/Mtok = 0,042 USD (tolleranza 1e-9), così un errore di unità (per token anziché per milione) è visibile.

**TEST-13 — `ndcg_recall_known_values`** (in `metrics.rs`). Ranking di 8 elementi con rilevanti in posizione 2 e 5, 2 rilevanti totali: recall@8 = 1,0; recall@5 = 1,0; recall@1 = 0; nDCG@5 = (1/log2(3) + 1/log2(6)) / (1 + 1/log2(3)) ≈ 0,6241 (tolleranza 1e-4). Nessun rilevante: 0, mai NaN.

**TEST-14 — Wording tracciato.** Per ogni riga di `forms.csv`, `questions_sha256` è uguale a `sha256sum` del file `questions/form-<form>.json` versionato a HEAD; i tre file sono JSON validi.

**TEST-15 — Evidenze.** Il verify del Task 2 più: `forms.csv` contiene forme a, b, c con `rep` 1..3 per ogni query; `concurrency.csv` contiene livelli 4, 8, 16 con run 1..3 e `cost_usd` cumulato ≤ `S04_MAX_COST_USD` dichiarato in RISULTATI §Setup; `hw.json` ha `tools.jev_model = jev-1.13.0` e il digest del runner; la stima di costo è in §Setup; i numeri di RISULTATI (nDCG@5, p95 per forma, 429 per livello) coincidono con un ricalcolo da CSV.

**TEST-16 — Injection.** Almeno una query del campione ha passaggi `injection-case` visibili al suo ruolo; per quelle righe `instructs_model_flagged` è valorizzato (true/false, non vuoto); RISULTATI riporta il tasso.

**TEST-17 — TLS.** `grep -rn "danger_accept_invalid\|http://" spikes/s04-jev/src | grep -v 127.0.0.1` vuoto (i test locali usano solo loopback).

## Copertura

Il CRITICAL (RISK-1) è coperto in tre livelli: funzione (TEST-1), percorso di invio (TEST-2), dati costruiti (TEST-3) ed evidenza (TEST-4). Tutti i HIGH sono coperti. Nota per l'esecutore: il client JEV deve accettare chiave e URL base come parametri del costruttore (lettura da env solo in `main.rs` o in una funzione `from_env`), altrimenti TEST-2, 6, 7, 8, 10 e 12 non si possono scrivere senza toccare variabili d'ambiente globali.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-11
