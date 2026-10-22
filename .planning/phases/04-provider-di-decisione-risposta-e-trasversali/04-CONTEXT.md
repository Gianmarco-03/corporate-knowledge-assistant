# Phase 4: Provider di decisione, risposta e trasversali - Context

**Gathered:** 2026-10-21
**Status:** Ready for planning
**Criticità:** critica — proposta da ROADMAP («confine JEV/LLM della regola no-leak») e ratificata dall'utente con questo CONTEXT. La fase consuma `AuthorizedEvidence`, cioè l'ultimo confine del retrieval autorizzato prima dei provider esterni. In `plan`: `implementation-risk-audit` + `test-design` di GPT via AgentBridge.

<domain>
## Phase Boundary

La fase costruisce per intero i provider del lato decisione e risposta, più due porte trasversali, ciascuno dietro la propria porta:

- nel RAG Service: `DecisionEngine` (adapter JEV), `Reranker` (adapter JEV + baseline TEI bge-reranker), `AnswerGenerator` (adapter Anthropic Messages API) e `Cache` (`NoOpCache`);
- in tutti e tre i servizi: `AuditSink` (adapter PostgreSQL append-only), perché lo usano le fasi 5, 6, 7, 8, 9, 11 e 12;
- per ogni porta: adapter reale, fake deterministico e una suite di contract test che gira identica su entrambi; il modulo di resilienza (circuit breaker, budget di retry, limiti di concorrenza) usato dagli adapter JEV e LLM;
- la tabella «Tracciabilità provider → consumer» aggiornata con le operazioni nuove decise qui.

Non costruisce casi d'uso: flusso query, evidence gate, policy di fallback e degradazione sono della fase 8; consultazione e aggregazione dell'audit della fase 11; calibrazione delle soglie e confronto con la baseline delle fasi 13 e 14. Non decide Q-04 (ownership dell'audit aggregato, discuss 11), Q-05 (multi-turno, discuss 8) né la parte di processo di Q-07 (discuss 13): qui si fissa la **forma** che quei discuss riempiranno o riapriranno. Il `plan` della fase 4 arriva dopo di loro (ROADMAP §Strategia di consegna, passi 2-3).

Requisiti: PLAT-01. Criteri di successo 1-5 del ROADMAP, fase 4 (criterio 1 da riformulare, D-01).

Ricerca a supporto: `04-RICERCA-DISCUSSIONE.md`, Aree A-K, con fonti (URL e data di accesso 2026-10-21, o file e sezione del repo). Ogni decisione qui sotto cita l'area che la fonda.

**Rischio di pipeline (spec §8):** la fase 1 è all'onda 4 di 11. ADR-0010 (deadline) e ADR-0008 (JEV: forma A di `rank`, 16 in volo per query senza 429, retry solo 429/529, pin `jev-1.13.0`, guardia ACL nell'adapter, margine soglie ±0,1; proposto durante questo discuss, commit b732149) sono `Proposed`: coerenti con le decisioni sotto, nessuna premessa cambiata. Non sono ancora emessi 0009 (reranker baseline; 01-15), 0011 (LLM, routing, structured output, tenuta all'injection; 01-12) e 0012 (firme; 01-16). Le decisioni che dipendono dai loro numeri restano provvisorie; il discuss si riapre solo se le misure ne cambiano le premesse.

</domain>

<decisions>
## Implementation Decisions

### Porte e baseline (Aree A, B, D, E)
- **D-01: Split S-07 confermato.** `Reranker { rank }` è separato da `DecisionEngine { validate_query, assess_evidence, validate_citations }`, come in `01-CONTRATTO.md` §3 (ISP: la baseline sa solo riordinare). Il criterio 1 del ROADMAP («le quattro operazioni di `DecisionEngine`») si riformula, via `gsd-tools`, in «le tre operazioni di `DecisionEngine` e `Reranker::rank`». Firme con `CallContext` (deadline esplicita, ADR-0010 §1) e input dentro `AuthorizedEvidence`. Le variazioni D-03, D-04, D-06, D-15, D-16 e D-19 entrano nel brief di execute di 01-16, come in 03 D-05. Fonte: Area A V1-V7. Decisione utente.
- **D-02: Baseline solo sul `Reranker`.** La baseline di A-06 (no-rerank + bge-reranker-v2-m3 su TEI, D-21 di fase 1; ADR-0009 atteso) implementa `Reranker`. Per le tre operazioni di `DecisionEngine` non c'è baseline: o JEV o il fake. Un guasto JEV dà un errore tipizzato e la policy (rifiuto o degradazione, D-19 di fase 1) è della fase 8. Il criterio 1 si riformula di conseguenza: suite di `DecisionEngine` su JEV e fake, suite di `Reranker` su JEV, baseline e fake. Scartati: engine «conservativo» con `Unassessed` (adapter in più, nessun consumatore oltre la policy della fase 8) ed euristiche lessicali (sembrano una validazione senza esserlo). Fonte: Area E; CLAUDE.md («un guasto JEV non bypassa mai un controllo di sicurezza»); regola provider-first. Decisione utente.
- **D-03: Gli adapter restituiscono segnali, non decisioni.** JEV dà punteggi `noul ∈ [0,1]`, scelta più confidenza per le Choice (compresa la complessità di D-18), la versione risolta (`response.model`) e i token in un `DecisionMeta`. Le soglie stanno nei parametri di configurazione del caso d'uso della fase 8; i valori si calibrano nelle fasi 13 e 14 (whitelist ROADMAP: valori dei file di parametri), con un margine di almeno ±0,1 attorno al taglio (varianza misurata, ADR-0008 §6). Fonte: ROADMAP criterio 1 («solo segnali»); ADR-0002 (soglie calibrate su dati etichettati); Area B5. Decisione utente.
- **D-04: Routing del modello per `Complexity` di dominio.** `AnswerGenerator::generate(ctx, query, &AuthorizedEvidence, Complexity)`: l'adapter mappa `Simple` → `claude-haiku-5-5` e `Complex` → `claude-sonnet-5-5` da configurazione. Nessun nome di provider attraversa la porta. Il fallback «Sonnet se JEV non risponde» (D-19 di fase 1) è la fase 8 che passa `Complex`. Fonte: CLAUDE.md (tipi di provider confinati); D-18 di fase 1; Area D7. Decisione utente.
- **D-05: `generate` single-turn.** Nessuna storia di conversazione in firma finché il discuss 8 non chiude Q-05; se Q-05 porta il multi-turno nell'MVP, la riconciliazione (passo 5 della strategia) la aggiunge. Fonte: regola provider-first (nessuna operazione senza consumatore); Area D10. Decisione utente.
- **D-06: Contratto della risposta strutturata.** Structured output `output_config.format` (JSON schema con `additionalProperties: false`) e validazione lato client in Rust dei vincoli che l'API non applica (`minLength`/`maxLength`, limiti numerici, cardinalità degli array). `StructuredAnswer { answer, claims: [{ text, citations: [PassageRef] }], refusal }` (D-20 di fase 1) più `GenerationMeta { model_resolved, input_tokens, output_tokens, stop_reason, dropped_claims }` per costo e latenza (EVAL-02). `stop_reason = refusal` o `max_tokens` danno esiti tipizzati distinti e l'output non si parsa; JSON non conforme dà `MalformedResponse`. Un claim che cita un id non fornito si scarta e si conta. Fonte: skill `claude-api` §Structured Outputs (vincoli non supportati, refusal, max_tokens); D-20 di fase 1; ROADMAP criterio 4; Area D1-D3, D9.

### Resilienza (Area C)
- **D-07: Circuit breaker scritto a mano.** Un modulo `resilience` nel rag-service: stati closed/open/half-open su fallimenti consecutivi, clock di tokio, quindi test deterministici con `tokio::time::pause`. Nessuna dipendenza. Circuito aperto → errore tipizzato `Unavailable { reason: CircuitOpen }`, mai un segnale positivo. Scartati `failsafe` 1.3.0 (ultima release 2024-07-05), `recloser` (manutenzione non verificata) e tower `TpsBudget` (è un budget di retry, non un breaker). Fonte: Area C4 (https://crates.io/api/v1/crates/failsafe, https://docs.rs/recloser, https://docs.rs/tower/latest/tower/retry/budget/index.html, consultati 2026-10-21); preferenza utente per toolchain semplici; 03 D-17. Decisione utente.
- **D-08: Solo i guasti del provider aprono il circuito.** Contano timeout, 429, 5xx/529, errori di connessione e risposte malformate. Non contano 400/401/403/413/415/422 (errori di configurazione o di contratto, che resterebbero nascosti) né le cancellazioni per deadline della query. Fonte: mapping errori JEV e Anthropic (Area B6, D5). Decisione utente.
- **D-09: Un breaker per provider.** Uno per JEV (condiviso da `Reranker` e `DecisionEngine`, stesso endpoint) e uno per Anthropic. Decisione utente.
- **D-10: Doppio limite di concorrenza.** Limite per query (variante A `buffer_unordered`, ADR-0010 §2) più un `Arc<Semaphore>` globale per provider, così N query concorrenti non moltiplicano le richieste verso JEV. ADR-0008 §2 fissa 16 in volo per query (massimo misurato senza 429, non un limite trovato); il valore del semaforo globale si fissa in `plan` a partire da lì. La saturazione oltre la deadline dà un errore tipizzato. Retry: al massimo uno, `Retry-After` rispettato entro la deadline (0,5 s se manca); verso JEV solo 429/529 (ADR-0008 §3), verso Anthropic 429/500/529 (ADR-0010 §5). Fonte: `interfaces-and-concurrency.md` §Query (max 8 decisioni JEV), Area C3. Decisione utente.

### Adapter JEV (Area B)
- **D-11: Contratto JEV fissato con uno snapshot.** L'OpenAPI di JEV (`info.version` 0.2.0, sha256 `ee599512cbea3a198b34af3f60addcd72b381d3521196f93c70ae83185c057c3`, identico tra il 2026-10-18 e il 2026-10-21) si versiona nel repo con il suo hash. Un test valida le request serializzate contro lo snapshot; un controllo documentato confronta lo snapshot con l'upstream. Classi Rust scritte a mano, nessun generatore (02 D-01). I codici che l'OpenAPI non dichiara (400, 401, 413, 415, 429, 529) si gestiscono come da documentazione in prosa. Pin `jev-1.13.0` (D-17 di fase 1). Fonte: https://api.typesafe.ai/openapi.json, https://docs.typesafe.ai/models.md (consultati 2026-10-21); Area B1-B4.
- **D-12: Minimizzazione verso JEV.** Lo `state` contiene solo la query e il testo dei passaggi ricevuti come argomento: niente titoli, autori, identità, ruoli. Forma di `rank`: A, una richiesta per candidato con i Noul `relevant`, `sufficient`, `instructs_model`; il wording è materiale di calibrazione versionato con sha256 nei metadati (ADR-0008 §1). In più l'adapter controlla l'insieme dei candidati contro l'ACL prima del primo invio, tutto o niente (ADR-0008 §5), come difesa in profondità oltre al tipo `AuthorizedEvidence`. Fonte: JEV privacy (USA, retention indefinita, ZDR solo enterprise; Area B7); D-16 di fase 1; ROADMAP criterio 3.

### Adapter LLM: dati e geografia (Area D)
- **D-13: `inference_geo = "global"`, nessun dato utente verso i provider.** L'API offre solo `"us"` o `"global"`, senza opzione EU; JEV è ospitato negli USA. Accettato con il corpus pubblico/sintetico (D-16 di fase 1). Non si inviano `metadata.user_id` né identità; solo la query e i passaggi autorizzati. Il limite si scrive nel threat model. Fonte: skill `claude-api` (inference_geo, retention 30 giorni); OWASP LLM02:2025, https://genai.owasp.org/llmrisk/llm022025-sensitive-information-disclosure/ (consultato 2026-10-21); Area D6. Decisione utente.

### AuditSink (Area F; forma provvisoria fino a Q-04)
- **D-14: Un adapter PostgreSQL per servizio.** Tabella append-only nello schema di ciascun servizio (`identity`, `document`, `rag`), fascia di migrazioni `04NNNN` (02 D-09). Il ruolo applicativo non è owner; `REVOKE UPDATE, DELETE, TRUNCATE`; trigger `BEFORE UPDATE OR DELETE` che solleva un'eccezione. L'`append` non condivide la transazione del chiamante: l'audit è a consistenza eventuale (architecture.md) e l'`event_id` deriva dall'idempotency key, quindi i retry sono sicuri. Aggregazione e consultazione: discuss 11 (Q-03, Q-04), che può riaprire questa forma. Duplicazione fra servizi accettata (02 D-10). Scartato: audit centrale nel rag-service via HTTP (anticipa Q-04 e aggiunge una dipendenza runtime). Fonte: DEPENDENCIES.md (Audit/outbox append-only, Proposta → confermata); Area F1-F5; pratica PostgreSQL su REVOKE e trigger (fonti secondarie in Area F3, da verificare in `plan` sulla documentazione ufficiale). Decisione utente.
- **D-15: `AuditEvent` interamente tipizzato.** Enum dei tipi di evento, actor id opaco, resource id, outcome, `request_id`, timestamp UTC; nessun payload libero (`serde_json::Value`). Il leak di chunk o prompt nell'audit è impossibile per costruzione. Se il testo della query entri nell'audit lo decide il discuss 11. Fonte: CLAUDE.md (nessun chunk nei log); GDPR art. 5(1)(c) (minimizzazione); Area F6. Decisione utente.
- **D-16: Idempotenza con controllo del contenuto.** `UNIQUE(event_id)`. Stesso `event_id` con lo stesso digest del contenuto → `AlreadyApplied`; con un digest diverso → errore tipizzato `IdempotencyConflict`, loggato senza contenuto. Fonte: race condition «Retry evento audit» (`interfaces-and-concurrency.md`); Area F4. Decisione utente.
- **D-17: Catena di hash già in fase 4.** Ogni riga contiene `sha256(contenuto canonico ‖ hash precedente)`, una catena per servizio. Gli `append` si serializzano con una riga di testa (`audit_chain_head`) letta con `SELECT … FOR UPDATE` nella stessa transazione breve dell'insert. L'idempotenza si controlla prima di estendere la catena, così un duplicato non aggiunge anelli. Decisione utente, **contro la raccomandazione** (catena fuori dall'MVP: serializza gli insert e complica l'idempotenza; Area F7).
- **D-18: Nessun ancoraggio esterno; il limite è documentato.** Il threat model registra che un owner del database può ricalcolare l'intera catena; la catena rileva le modifiche di chi non può ricalcolarla. L'ancoraggio (export periodico o hash di testa in un log esterno) è un'idea rinviata. Decisione utente.
- **D-19: `verify_chain` come operazione, con consumatore la fase 11.** La verifica ricalcola la catena e restituisce la prima riga rotta. Si aggiunge alla tabella di tracciabilità con consumatore 11. Se il discuss 11 non la conferma, la riconciliazione la ritira. Nel frattempo la provano i contract test. La collocazione (`AuditSink` o una porta di lettura separata, per ISP) si sceglie in `plan`. Fonte: regola provider-first. Decisione utente.

### Cache (Area G)
- **D-20: `NoOpCache` più un fake in memoria.** `NoOpCache` è l'unico adapter di produzione: `get` → `None`, `put` → `Ok`, `invalidate` → `Applied`. Un fake dietro `testkit` rispetta `CacheKey` con `AuthorizationScope` e `InvalidationScope::{Subject, AclVersion, DocumentVersion, EmbeddingSpace}`: la suite verifica che la chiave del soggetto A non serva il soggetto B e che ogni invalidazione svuoti ciò che deve. Serve alla matrice di leak della fase 10. Redis resta alla fase 14 (A-07). Fonte: DEPENDENCIES.md §Caching; `01-CONTRATTO.md` §3 (`CacheKey` con scope); 03 D-08 (versione ACL per documento); Area G. Decisione utente.

### Contract test, prova no-leak e adapter reali (Aree H, I)
- **D-21: Dove gira la suite sugli adapter reali.**
  - **In CI a ogni push:** adapter JEV e Anthropic veri contro un server HTTP finto (`wiremock`, 0.6.5) che riproduce risposte registrate negli spike 01-11 e 01-12: successi, 400/422, 429 con `retry-after`, 529, JSON malformati, timeout, refusal, `max_tokens`. L'adapter `AuditSink` su PostgreSQL vero, come in 03 D-15.
  - **Gate locale dichiarato in `verify`:** JEV e Anthropic veri, con un tetto di **0,50 USD per esecuzione** stimato da `usage` (le richieste cancellate si contano come addebitate, ADR-0010) e un comando documentato.

  Scartati: provider veri in CI (segreti, flakiness già vista col DNS, rate limit) e solo locale (CI cieca). Fonte: Area I; 03 D-15. Decisione utente.
- **D-22: Prova no-leak a sentinelle.** I passaggi dei test portano stringhe sentinella uniche. Il server finto cattura ogni request: ogni contenuto deve venire dagli argomenti (nessun campo in più). Un subscriber `tracing` di test e un exporter OpenTelemetry in memoria non devono mai vedere una sentinella, nemmeno nei percorsi d'errore: dei provider si loggano solo status e codice, mai il body. Sugli span solo modello, token ed `error.type`; i contenuti mai, nemmeno opt-in. Fonte: OpenTelemetry GenAI semconv (`gen_ai.input.messages`/`gen_ai.output.messages` Opt-In, «likely to contain sensitive information»), https://github.com/open-telemetry/semantic-conventions-genai (consultato 2026-10-21); 02 D-20 (redazione); ROADMAP criterio 3; Area H.
- **D-23: Suite, fake e copione dei guasti come in fase 3.** Una suite per porta su `Arc<dyn Porta>`; fake dietro la feature `testkit` dei servizi, mai in release; copione deterministico (errore alla N-esima chiamata, `OutcomeUnknown`, latenza col clock in pausa, conteggio delle chiamate). L'API dei fake fa parte del contratto della fase 4 (le fasi 8 e 10 la usano senza poterla modificare). Fonte: 03 D-14, D-16, D-17.

### Vincoli dalla WIRING-MAP e dalle fasi precedenti
- La WIRING-MAP non ha righe (il `plan` della fase 2 non è ancora fatto): nessun `WM-NNN` da citare. La fase 4 **riempie** gli innesti che la Fondazione pre-registrerà:
  - slot dei provider della fase 4 nel composition root del rag-service;
  - slot di `AuditSink` nel composition root di identity-service e document-service;
  - fascia di migrazioni `04NNNN_*.sql` negli schemi `identity`, `document` e `rag` (D-14).

  `wiring-design` del plan 2 assegnerà le righe con `fase-proprietaria` 4. Va segnalato al plan 2 che la fase 4 tocca tre servizi, non solo il rag-service.
- `reqwest` 0.13 con Rustls, HTTP/2, client condiviso, `https_only`, nessun `danger_*` (ADR-0010 §3-4).
- Segreti `TYPESAFE_API_KEY` e chiave Anthropic da `secrets:`/file e tipo `Secret<T>` (02 D-20; lezione della fase 1.1 sulle variabili dell'host).

### Claude's Discretion
- Etichette posizionali brevi (`S1..Sn`) al posto degli UUID nel prompt, rimappate a `PassageRef` da `AuthorizedEvidence::resolve` (Area D8).
- System prompt fisso e versionato; passaggi in un blocco delimitato come dati; testo dei claim trattato come testo non fidato (Area J). La tenuta all'injection indiretta si misura in ADR-0011.
- Effort e thinking di Haiku e Sonnet, `max_tokens`, uso di `fallbacks: "default"` su Sonnet (solo Claude API; Haiku non ha fallback server-side), secondo ADR-0011.
- Forma concreta di `CacheValue`; nomi delle tabelle; forma canonica del contenuto per l'hash; struttura interna dei moduli `ports`/`adapters`/`resilience`; soglie del breaker (fallimenti consecutivi, intervallo di apertura).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/04-provider-di-decisione-risposta-e-trasversali/04-RICERCA-DISCUSSIONE.md` — brief Aree A-K con fonti e data di accesso.

### Decisioni di progetto
- `.planning/ROADMAP.md` §Phase 4 (criterio 1 da riformulare, D-01/D-02), §Strategia di consegna, §Tracciabilità provider → consumer, §Decisioni aperte per fase.
- `.planning/phases/01-spike-tecnici/01-CONTEXT.md` — D-16 (corpus pubblico/sintetico), D-17 (pin JEV), D-18 (routing Haiku/Sonnet), D-19 (fallback), D-20 (risposta JSON con citazioni per claim), D-21 (reranker baseline), D-22 (spike JEV e cancellazione).
- `.planning/phases/01-spike-tecnici/01-CONTRATTO.md` §3 (firme S-07: `Reranker`, `DecisionEngine`, `AuthorizedEvidence`, `CacheKey`, `CallContext`/`MutationContext`), §6 (ADR attesi, collocazione per servizio, primitive alle fasi proprietarie).
- `.planning/phases/01-spike-tecnici/01-RICERCA-DISCUSSIONE.md` §Ricerca 01 — JEV, cancellazione, LLM, reranker (contratto, errori, limiti, strategie di `rank`).
- `.planning/phases/02-fondazione/02-CONTEXT.md` — D-01 (contract-first, classi a mano), D-09 (fascia migrazioni), D-10 (primitive alle fasi proprietarie), D-19/D-20 (telemetria e redazione), D-22 (tipi in `crates/contracts`).
- `.planning/phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md` — D-05 (variazioni nel brief di 01-16), D-08 (versione ACL per documento), D-14..D-17 (contract suite, CI vs gate locale, testkit, copione dei guasti).
- `docs/adr/0002-incremental-corpus-and-jev.md` — ruolo di JEV, soglie calibrate.
- `docs/adr/0010-deadline-cancellazione-http.md` — deadline, fan-out, retry, HTTP/2 (Proposed).
- `docs/adr/0008-jev-forma-rank-concorrenza-pin.md` — forma A di `rank`, concorrenza 16, retry 429/529, pin, guardia ACL, varianza (Proposed).
- `docs/adr/0009`, `0011`, `0012` — **non ancora emessi** (piani 01-15, 01-12, 01-16): baseline, LLM e injection, firme.
- `DEPENDENCIES.md` — §JEV, §Caching, riga Audit/outbox.

### Piani della fase 1 che portano le variazioni
- `.planning/phases/01-spike-tecnici/01-16-PLAN.md` — firme S-07 (brief di execute: D-01, D-03, D-04, D-06, D-15, D-16, D-19).
- `.planning/phases/01-spike-tecnici/01-11-PLAN.md` — JEV `rank`, concorrenza, risposte da registrare per il replay (D-21).
- `.planning/phases/01-spike-tecnici/01-12-PLAN.md` — LLM, structured output, risposte da registrare per il replay (D-21).

### Architettura
- `docs/architecture/interfaces-and-concurrency.md` — porte, concorrenza, backpressure, race «Retry evento audit» e «Timeout JEV»; firme superate da S-07 (riallineamento in Fondazione).
- `docs/architecture/architecture.md` — audit a consistenza eventuale, ownership dei dati.
- `skills/crate-boundary-check/boundaries.json` — crate proibiti nei moduli puri (criterio 5).

### Contratti esterni
- https://api.typesafe.ai/openapi.json (sha256 `ee599512…c057c3`, 0.2.0) e https://docs.typesafe.ai/api — contratto JEV.
- Skill `claude-api` (structured output, error codes, models) e https://platform.claude.com/docs — Messages API.

### Sicurezza
- `docs/security/threat-model.md` — aggiungere: limite della catena di hash (D-18), dati verso provider USA/global (D-13), canale cache (D-20).
- OWASP Top 10 for LLM Applications 2025 (LLM01, LLM02, LLM05).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: `services/rag-service`, `services/identity-service` e `services/document-service` contengono solo `.gitkeep`.
- `spikes/` è materiale da consultare e trascrivere, mai da importare: client JEV di `spikes/s05-cancel/src/jev.rs` (deadline, cancellazione), spike 01-11 (JEV `rank`) e 01-12 (LLM), pendenti, che produrranno anche le risposte da registrare per il replay.

### Established Patterns
- Porte come `Arc<dyn Trait + Send + Sync>`, errori enum per porta, nessun tipo SDK/DB in firma (01-16; `crate-boundary-check`).
- Gate di piano: `gate-perimetro.cjs`, `gate-proprieta-file.cjs` (colonna `fase-proprietaria` della WIRING-MAP).
- Lezione della fase 1.1: le variabili d'ambiente dell'host possono prevalere su `--env-file`; i segreti si leggono da `secrets:`/file.
- Gate su `spikes/`: partire da `cargo clean -p` dei crate toccati (verify 01.2).

### Integration Points
- Composition root del rag-service (slot provider fase 4) e di identity/document-service (slot `AuditSink`); fascia migrazioni 04 in tre schemi.
- Consumatori: fase 8 (`validate_query`, `rank`, `assess_evidence`, `validate_citations`, `generate`, `Cache get/put`, `append`), 10 (fake ostile di `DecisionEngine`, fake di `Cache`), 13 (adapter JEV e baseline `Reranker`), 5, 6, 7, 9, 11, 12 (`append`), 11 (`verify_chain`), 12 e 14 (`invalidate`).

</code_context>

<specifics>
## Specific Ideas

- L'utente vuole la catena di hash dell'audit già nella fase 4, pur sapendo che senza ancoraggio non protegge da un owner del database (D-17, D-18).
- Preferenza confermata per una toolchain semplice: circuit breaker scritto a mano invece di un crate.

</specifics>

<deferred>
## Deferred Ideas

- Ancoraggio esterno dell'hash di testa dell'audit (export periodico o log esterno): fase 11 o 15.
- Conversazioni multi-turno in `generate`: dipende da Q-05 (discuss 8).
- Baseline semantica per `assess_evidence`/`validate_citations`: nessuna nell'MVP; eventuale confronto in fase 13.

</deferred>

## TODO aperti dal discuss

- ROADMAP fase 4, criterio 1: riformulare in «tre operazioni di `DecisionEngine` (JEV, fake) e `Reranker::rank` (JEV, baseline, fake)» (via `gsd-tools`, D-01/D-02).
- ROADMAP, tabella «Tracciabilità provider → consumer»: `Reranker (4) | rank | 8, 13`; `DecisionEngine (4)` con tre operazioni; baseline sul `Reranker`; `AuditSink (4) | verify_chain | 11`; fake `Cache` → 10 (nella riconciliazione o nel plan 4).
- Brief di execute di **01-16**: `Complexity` in `generate` (D-04), `GenerationMeta`/`DecisionMeta` (D-03, D-06), `AuditEvent` tipizzato e `IdempotencyConflict` (D-15, D-16), `verify_chain` (D-19).
- Brief di execute di **01-11** e **01-12**: registrare le risposte reali (successi, errori, malformati) come fixture per il replay in CI (D-21), senza segreti.
- Discuss 8: policy al guasto JEV con errore tipizzato (D-02), soglie nei parametri (D-03), Q-05 (D-05).
- Discuss 11: conferma della forma di `AuditSink` (D-14), testo della query nell'audit (D-15), consumo di `verify_chain` (D-19).
- Discuss 13: versione JEV risolta già nei `DecisionMeta` (D-03) per la parte di processo di Q-07.
- Plan 2 (Fondazione): slot `AuditSink` e fascia 04 anche in identity e document-service.
- `docs/security/threat-model.md`: D-13, D-18, D-20 (in 01-17 o nel plan 4).

---

*Phase: 04-provider-di-decisione-risposta-e-trasversali*
*Context gathered: 2026-10-21*

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 04)
