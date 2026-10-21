# 04 — Ricerca decisionale (discuss fase 4: Provider di decisione, risposta e trasversali)

Canale: Claude (WebSearch/WebFetch/curl, skill `claude-api`). Accesso fonti esterne: **2026-10-21**. Stile caveman full. Legenda: **[P]** primaria · **[S]** secondaria · **[R]** repo · **[STIMA]** inferenza, da misurare.
Contesto letto: ROADMAP §Phase 4, §Tracciabilità, §Decisioni aperte per fase; `01-CONTRATTO.md` §3/§6; `01-CONTEXT.md` D-16..D-22; `01-RICERCA-DISCUSSIONE.md` §JEV/LLM/reranker; ADR-0002, ADR-0010; `02-CONTEXT.md` D-10, D-19..D-22; `03-CONTEXT.md` D-08, D-14..D-17; `interfaces-and-concurrency.md`; `DEPENDENCIES.md` §JEV, §Caching.

Punti ereditati: nessun `deferred-items.md` nomina la fase 4. Riferimento indiretto: `03-CONTEXT.md` D-08 (versione ACL per documento visibile alla cache della fase 4).
WIRING-MAP: zero righe (plan 2 non fatto) → nessun `WM-NNN`. Innesti attesi dalla Fondazione (02-CONTEXT D-22, ROADMAP perimetro): slot provider fase 4 nel composition root del rag-service (+ servizi di `AuditSink`).

Rischio pipeline (spec §8): fase 1 all'onda 4 di 11. ADR non emessi che toccano la fase: **0008** (JEV forma `rank`, concorrenza, pin; piano 01-11), **0009** (reranker baseline; 01-15), **0011** (LLM, routing, structured output, injection; 01-12, onda 7), **0012** (firme; 01-16). Emesso: ADR-0010 `Proposed`. Decisioni che dipendono dai numeri → provvisorie, discuss si riapre solo se le misure cambiano le premesse (ROADMAP §Strategia passo 2).
Dipendenze da discuss non ancora fatti: Q-04 (discuss 11 → `AuditSink`), Q-05 + policy fallback (discuss 8 → `AnswerGenerator`/`DecisionEngine`), Q-07 (discuss 13 → adapter JEV). Qui si fissa la **forma**; il plan 4 arriva dopo quei discuss (stesso schema di fase 3 con Q-02).

---

## Area A — Firme delle porte (conferma S-07)

- V1 [R] `01-CONTRATTO.md` §3: split ISP. `Reranker { rank(ctx, query, AuthorizedEvidence) → Vec<RankScore> }` (impl: JEV + baseline TEI bge-reranker). `DecisionEngine { validate_query → QueryDecision{complexity: Simple|Complex,…}, assess_evidence(&AuthorizedEvidence), validate_citations(&AuthorizedEvidence, &StructuredAnswer) }` + versione JEV risolta nei metadati.
- V2 [R] ROADMAP criterio 1 elenca **4 op in `DecisionEngine`** incluso `rank` → divergenza con lo split. Serve allineare il testo del criterio (via `gsd-tools`) o tornare a porta unica.
- V3 [R] `AuthorizedEvidence`: niente `Default`/`Serialize`; solo `reorder`, `resolve(&PassageRef)`; `Debug` redatto. Unico costruttore `authorize_response` (rag-service). → by-type: adapter JEV/LLM ricevono SOLO passaggi già autorizzati. Fonda criterio 3 lato tipo.
- V4 [R] ADR-0010 §Decisione 1: deadline esplicita in firma di `DecisionEngine`, `AnswerGenerator`, `Reranker::rank`. Fake compresi. → `CallContext { request_id, trace, deadline }` (01-CONTRATTO §3).
- V5 [R] `AuditSink::append(MutationContext, AuditEvent) → MutationOutcome` con `event_id` = idempotency key; `Cache::put(CallContext, …)`, `Cache::invalidate(MutationContext, InvalidationScope::{Subject, AclVersion, DocumentVersion, EmbeddingSpace})`; `CacheKey::new(AuthorizationScope, corpus_version, space, pipeline_version, query_digest)` senza `Default`.
- V6 [R] `interfaces-and-concurrency.md` firme vecchie (`rank` dentro `DecisionEngine`, `append(event) → Result<()>`) → superate da S-07, riallineamento spetta alla Fondazione (01-CONTRATTO §6).
- V7 [R] 03 D-05: variazioni di firma entrano nel brief di execute di **01-16** (non eseguito) → ADR-0012 nasce allineato. Stesso canale disponibile per fase 4.
- Default: confermare S-07 (split) + aggiornare criterio 1 ROADMAP: «le tre op di `DecisionEngine` + `Reranker::rank`». Varianti candidate da Aree B-G (segnali, metadati, `Unassessed`).

## Area B — Adapter JEV

Fonti: https://api.typesafe.ai/openapi.json (curl, HTTP 200, 21660 B, OpenAPI `info.version` 0.2.0, sha256 `ee599512…c057c3`, **identico** al 2026-10-18) · https://docs.typesafe.ai/models.md · `01-RICERCA-DISCUSSIONE.md` §1 [P, 2026-10-18].

- B1 [P] Contratto invariato: `POST /v1/systemone`, `SystemOneRequest{model, state, questions}`, tipi `noul|choice|score|box` (box = immagini, fuori scope); response `{model, answers, usage}`. OpenAPI dichiara solo 200/422; 400/401/413/415/429/529 solo in prosa → adapter gestisce codici non tipizzati.
- B2 [P] Modelli correnti `jev-1.14.0` (dal 2026-10-14) e `jev-1.13.0`, stesso prezzo $0.042/Mtok input, output gratis; 64k per request / 32k per question.
- B3 [R] D-17 fase 1: pin `jev-1.13.0`, versione risolta = `response.model`, registrata (parte tecnica Q-07). Docs: «If you've tuned confidence thresholds against a specific version, send its versioned ID».
- B4 Pratica consolidata (contract drift): snapshot dell'OpenAPI versionato nel repo con hash + test che (a) serializza le request e le valida contro lo schema snapshot, (b) un controllo manuale/periodico confronta lo snapshot con l'upstream. [STIMA, pattern consumer-driven; nessun generatore, coerente con 02 D-01 classi a mano].
- B5 Segnali, non decisioni (ROADMAP criterio 1 «solo segnali»; ADR-0002 «soglie calibrate su dati etichettati»): adapter restituisce `noul ∈ [0,1]`/probabilità per domanda + `model` risolto + `usage`. Soglie → config del caso d'uso fase 8, valori calibrati fase 13/14 (whitelist ROADMAP: valori dei file di parametri). Per `complexity` (D-18) Choice dà già `choice` + `confidence`: segnale sufficiente.
- B6 [P] Mapping errori: 400/401/413/415/422 → errore tipizzato non ritentabile (config/contratto); 429 (+`retry-after`)/529/5xx → ritentabile entro ADR-0010 §5; body non parsabile, `answers` senza una domanda, `noul` fuori [0,1], NaN → `MalformedResponse`. **Mai** default positivo (criterio 2).
- B7 [P] Privacy: JEV USA, retention «as long as necessary», ZDR solo enterprise. D-16 fase 1 ammette corpus pubblico/sintetico. → minimizzazione: `state` = solo query + testo passaggi passati dall'argomento; niente titoli/autore/utente/ruoli. `questions` id non inviati al modello [P docs].
- B8 [R] Forma `rank` (A: 1 request per candidato vs B: batch) e concorrenza (4/8/16, 429) = **ADR-0008 non emesso** (01-11). Firma porta indipendente dalla forma → provvisorio.

## Area C — Resilienza: deadline, concorrenza, retry, circuit breaker

- C1 [R] ADR-0010 (Proposed): deadline `tokio::time::Instant` globale; variante A `timeout_at` + `buffer_unordered(8)`; `reqwest::Client` condiviso HTTP/2; retry max 1 per chiamata solo 429/5xx e solo se tempo residuo > latenza attesa, `Retry-After` rispettato se entro deadline; cancellata mai ritentata.
- C2 [R] `interfaces-and-concurrency.md` §Backpressure: «Circuit breaker per JEV, embedding e LLM»; §Query: max 8 decisioni JEV concorrenti (baseline).
- C3 Due limiti distinti [R 01-RICERCA §2]: per-query (`buffer_unordered(n)`) + globale per adapter (`Arc<Semaphore>`) contro 429 a cascata fra query concorrenti.
- C4 Circuit breaker, opzioni:

| Opzione | Stato | Pro | Contro | Fonte |
|---|---|---|---|---|
| `failsafe` 1.3.0 | ultima release 2024-07-05, 17 M download | policy consecutive_failures / success_rate, backoff jitter, futures | inattivo da 27 mesi; clock reale (test con tokio paused da verificare) | https://docs.rs/crate/failsafe/latest · https://crates.io/api/v1/crates/failsafe |
| `recloser` | ring buffer, `AsyncRecloser` | API stile failsafe | piccolo, manutenzione non verificata | https://docs.rs/recloser |
| tower 0.5.3 `retry::budget::TpsBudget` | mantenuto | budget di retry anti-storm (deposit su successo, withdraw su errore) | è budget di retry, non breaker | https://docs.rs/tower/latest/tower/retry/budget/index.html |
| breaker scritto a mano (closed/open/half-open, contatore consecutivi, clock tokio) | — | ~100 righe, deterministico con `tokio::time::pause`, nessuna dipendenza | codice nostro da testare | [STIMA] |

  Default: breaker a mano + budget retry ADR-0010 (max 1). Motivo: preferenza utente per toolchain semplice, crate candidato fermo dal 2024, test deterministici richiesti (03 D-17).
- C5 Circuit aperto → errore tipizzato `Unavailable { reason: CircuitOpen }`, mai segnale positivo; fallback = policy fase 8 (D-19 fase 1: Sonnet senza JEV, risposta degradata/rifiutata).

## Area D — Adapter LLM (`AnswerGenerator`)

Fonte: skill `claude-api` (cache 2026-10-06) §Structured Outputs, §Error codes, §Models, curl/examples; [R] D-18..D-20 fase 1.

- D1 [P] Structured output: `output_config: {format: {type: json_schema, …}}` su `POST /v1/messages`; supportato da `claude-haiku-5-5` e `claude-sonnet-5-5`. Schema: obbligatorio `additionalProperties: false` su ogni oggetto; **non supportati** `minLength/maxLength`, `minimum/maximum`, vincoli di array complessi, schemi ricorsivi → validazione client-side obbligatoria (gli SDK Py/TS li tolgono e validano lato client; noi senza SDK lo facciamo in Rust).
- D2 [P] Incompatibile con Citations native (400) e prefill → conferma D-20 fase 1 (claim con id, niente citations native).
- D3 [P] `stop_reason`: `refusal` (HTTP 200, output può non rispettare lo schema; `stop_details.category`) e `max_tokens` (output troncato) → esiti tipizzati distinti, mai parse dell'output. Haiku 5.5: refusal **senza** fallback server-side; Sonnet 5.5: `fallbacks: "default"` + beta `server-side-fallback-2026-07-01` solo su Claude API.
- D4 [P] Thinking: Haiku 5.5 adaptive di default, effort default `medium`; Sonnet 5.5 adaptive, `disabled` → 400 (spegnere = `between_tools`); niente sampling non default (400). Prima request con schema nuovo: costo di compilazione, cache 24 h.
- D5 [P] Errori: 400/401/402/403/404/413 non ritentabili; 429 (`retry-after`), 500, 529 `overloaded_error` ritentabili → stesso schema di C1.
- D6 [P] `inference_geo` top-level: valori `"us"` | `"global"`, nessuna opzione EU documentata. Retention Anthropic 30 gg [01-RICERCA §3.2]. `metadata.user_id` facoltativo → non inviarlo (minimizzazione, criterio 3).
- D7 Routing (D-18 fase 1): Haiku = Simple, Sonnet = Complex, complessità da JEV. Regola porte/adapter (CLAUDE.md «tipi di provider non attraversano una porta»): la porta riceve una `Complexity` di dominio, l'adapter mappa → model id da config. Il fallback «Sonnet se JEV giù» (D-19) = scelta del caso d'uso fase 8, che passa `Complex`.
- D8 Riferimenti brevi [STIMA, pratica RAG]: l'LLM vede etichette posizionali (`S1..Sn`), non UUID; adapter rimappa a `PassageRef` via `AuthorizedEvidence::resolve`. Claim con id sconosciuto → scartato (D-20 fase 1, già deciso), conteggio nei metadati.
- D9 Metadati per eval/costo (EVAL-02: costo/query, latenza, versioni): `GenerationMeta { model_resolved, input_tokens, output_tokens, stop_reason, dropped_claims }`. Analogo per JEV (`DecisionMeta { model_resolved, input_tokens }`).
- D10 [R] Q-05 (multi-turno) aperta, discuss 8. Provider-first: nessuna op senza consumatore → `generate` single-turn ora; se Q-05 = sì, la riconciliazione (passo 5) aggiunge la storia alla firma.

## Area E — Baseline di `DecisionEngine` (A-06) e significato del guasto

- E1 [R] A-06/D-21 fase 1: baseline di **reranking** = no-rerank + bge-reranker-v2-m3 su TEI GPU; ADR-0009 (01-15) non emesso. Con lo split (Area A) la baseline implementa `Reranker`.
- E2 [R] ROADMAP criterio 1: «le quattro op… sull'adapter JEV, sul fake e sulla baseline». Per `validate_query`, `assess_evidence`, `validate_citations` non esiste baseline semantica scelta in A-06.
- E3 Opzioni per le tre op senza JEV:
  - (a) baseline solo `Reranker`; le tre op restano JEV/fake; guasto JEV → errore tipizzato, la fase 8 applica la policy.
  - (b) `ConservativeDecisionEngine` deterministico: `validate_query` → `Complex`, `assess_evidence`/`validate_citations` → `Unassessed` (mai positivo).
  - (c) baseline euristiche (overlap lessicale claim–passaggio): misurabile in fase 13 ma è un «controllo» debole che può sembrare validazione.
  Vincolo: CLAUDE.md «Un guasto JEV non bypassa mai un controllo di sicurezza»; `interfaces-and-concurrency.md` «Timeout JEV → Fallback/rifiuto; mai bypass».
- Default: (a) + criterio 1 riformulato (baseline sul solo `Reranker`). Motivo: niente op speculative (provider-first), la policy di degradazione è della fase 8.

## Area F — `AuditSink` (Q-04 aperta, discuss 11)

- F1 [R] DEPENDENCIES: «Audit/outbox — Tabelle append-only PostgreSQL — Proposta — idempotenti con `event_id` univoco». architecture.md: «consistenza eventuale per proiezioni e audit aggregato». Race test: «Retry evento audit → nessun duplicato grazie a `event_id`».
- F2 [R] Consumatori `append`: 5 (identity), 6, 7, 9, 12 (document), 8, 11 (rag) → serve un adapter in **tutti e tre** i servizi. 02 D-10: duplicazione fra servizi accettata, nessun riuso di dominio; schema per servizio.
- F3 Append-only in PostgreSQL, pratica [S + conoscenza, da verificare in plan su docs PG]: ruolo applicativo non owner; `REVOKE UPDATE, DELETE, TRUNCATE`; trigger `BEFORE UPDATE OR DELETE` che solleva eccezione (difesa in profondità); `REVOKE` da solo non impedisce INSERT falsi. Fonti: https://www.postgresql.org/message-id/d6310f2c-1b98-af72-dba6-52e77d329f46%40aklaver.com · https://vonng.com/en/pg/audit-change/ [S].
- F4 Idempotenza: `UNIQUE(event_id)` + `INSERT … ON CONFLICT (event_id) DO NOTHING` → `AlreadyApplied`; stesso `event_id` con contenuto diverso → conflitto tipizzato, non silenzio [STIMA, pattern idempotency key].
- F5 Atomicità con la mutazione di dominio: porta senza tipi DB → `append` non condivide la transazione del chiamante. Accettabile per architecture.md (audit eventuale) + `event_id` deterministico dall'idempotency key (retry sicuri). Alternativa: outbox nella stessa tx (fasi proprietarie, 02 D-10). [R]
- F6 Contenuto: regola no-leak (CLAUDE.md: nessun chunk in log) + GDPR art. 5(1)(c) minimizzazione. Pratica: `AuditEvent` tipizzato (enum di tipi evento, actor id opaco, resource id, outcome, request_id, timestamp UTC), **niente payload libero** `serde_json::Value` → leak impossibile per tipo. Testo della query nell'audit: decide Q-03/Q-04 (discuss 11).
- F7 Tamper-evidence (hash chain fra righe): pratica per audit regolati [STIMA]; costo: serializzazione degli insert. Fuori dall'MVP salvo richiesta.
- Default provvisorio: adapter PG per servizio, tabella append-only nella fascia migrazioni 04 dello schema del servizio, aggregazione/ownership a discuss 11. Effetto sul perimetro: tabelle audit negli schemi `identity`/`document` scritte da fase 4 → fascia `04NNNN` per servizio (02 D-09) e righe WIRING-MAP con `fase-proprietaria` 4.

## Area G — `Cache` / `NoOpCache`

- G1 [R] DEPENDENCIES: porta `Cache`, primo adapter `NoOpCache`, Redis solo dopo misura (A-07, fase 14). «Non cacheare nell'MVP risposte complete o contenuti autorizzati senza identità, versione policy, versioni documenti».
- G2 [R] 01-CONTRATTO §3: `CacheKey` con `AuthorizationScope` obbligatorio; voce letta si ricontrolla contro l'autorizzazione corrente. 03 D-08: versione ACL per documento → la cache vede la revoca.
- G3 Contratto `NoOpCache`: `get` → sempre `None`; `put` → `Ok`; `invalidate` → `Applied`. Suite banale su NoOp.
- G4 Consumatori: 8 (percorso query), 10 (canale cache nella matrice di leak), 12/14 (`invalidate`). Con solo NoOp il test della fase 10 è vuoto. Opzione: fake in-memory (`testkit`) che rispetta scope e invalidazione → la suite verifica: chiave del soggetto A non serve il soggetto B; `invalidate(Subject|AclVersion|DocumentVersion)` svuota. [STIMA, 03 D-17]
- G5 `CacheValue`: con NoOp non serve un payload reale → tipo opaco minimo, estensione in fase 14. Discrezione.

## Area H — Prova no-leak (criterio 3)

- H1 [P] OpenTelemetry GenAI semconv (repo `semantic-conventions-genai`, `docs/gen-ai/gen-ai-spans.md`, **Status: Development**): `gen_ai.request.model`, `gen_ai.response.model`, `gen_ai.usage.input_tokens` Recommended; `error.type` Stable; `gen_ai.input.messages`/`gen_ai.output.messages`/`gen_ai.retrieval.query.text`/`gen_ai.retrieval.documents` **Opt-In**, «likely to contain sensitive information including user/PII data». https://github.com/open-telemetry/semantic-conventions-genai → span con model/token/error, contenuti **mai** (regola no-leak più stretta dell'opt-in).
- H2 [P] OWASP LLM02:2025: «Only grant access to data that is necessary for the specific user or process»; restrizioni nel prompt «may not always be honored and could be bypassed via prompt injection». https://genai.owasp.org/llmrisk/llm022025-sensitive-information-disclosure/
- H3 [R] 02 D-20: `Secret<T>` redatto, body mai loggati di default, test di redazione. 01-CONTRATTO §3: `Debug` redatto su `AuthorizedEvidence`, `Claim`, `StructuredAnswer`.
- H4 Test canary [STIMA, pattern consolidato]: passaggi con stringhe sentinella uniche; server finto (`wiremock` 0.6.5, release 2025-08-24, https://crates.io/crates/wiremock) cattura le request → ogni byte di contenuto nella request viene dagli argomenti (nessun campo extra: titoli, utente, altri chunk); subscriber `tracing` di test + exporter OTel in memoria → nessuna sentinella in log/span; anche nei percorsi d'errore (422 JEV che riecheggia il campo, 400 Anthropic) l'adapter logga solo status + codice, mai il body.
- H5 Per tipo: gli adapter ricevono solo `AuthorizedEvidence` (V3) → un chunk non autorizzato non è rappresentabile.

## Area I — Contract test e dove girano gli adapter reali

- I1 [R] 03 D-14..D-17: una suite per porta su `Arc<dyn Porta>`, su reale e fake; fake dietro feature `testkit`, mai in release; copione di guasti deterministico.
- I2 Provider a pagamento con chiave: in GitHub Actions i segreti non arrivano ai PR da fork [conoscenza, da verificare in plan]; JEV ~$0.0004/query, Haiku ~$0.0008/query [R 01-RICERCA §1.5, §3.3]. DNS intermittente già visto (ORCHESTRATOR «Limiti»).
- I3 Opzioni:
  - (a) CI: adapter reali contro server finto con risposte registrate dagli spike 01-11/01-12 (replay) → protocollo, mapping errori, malformati, timeout; **gate locale** in `verify`: JEV + Anthropic reali con tetto di spesa, comando documentato (come 03 D-15 per GPU).
  - (b) CI con chiavi reali a ogni push: segreti in CI, flakiness, rate limit.
  - (c) solo locale: CI cieca sulle regressioni.
  Default: (a).

## Area J — Prompt injection e output del modello (OWASP LLM01/LLM05)

- J1 [S] OWASP LLM01:2025: segregare contenuto esterno dal prompt utente; privilegi minimi. LLM05:2025 Improper Output Handling: output mai usato senza validazione/sanificazione. https://www.hackerone.com/ai/owasp-top-10-llms-2025 · https://invicti.com/blog/web-security/owasp-top-10-risks-llm-security-2025 [S]
- J2 Pratica nell'adapter: system prompt fisso versionato; passaggi in blocco delimitato come dati; output solo via schema; testo dei claim = testo non fidato (escape nel frontend fasi 8/9). Tenuta all'injection indiretta = misura di ADR-0011 (01-12). JEV Noul `instructs_model` come segnale [R 01-RICERCA §1.7].
- J3 [R] D-20: id non forniti → claim scartato; mapping citazione deterministico dal DB (fase 8).

## Area K — Criticità

- [R] ROADMAP fase 4: «Criticità: critica (confine JEV/LLM della regola no-leak)». Criterio skill: tocca RBAC + retrieval autorizzato (consuma `AuthorizedEvidence`, ultimo confine prima dei provider esterni). → **critica**: in `plan` `implementation-risk-audit` + `test-design` GPT.

## Domande per l'utente (armate)

1. Split `Reranker`/`DecisionEngine` (S-07, ISP) vs criterio 1 ROADMAP con 4 op in una porta → confermo split e riformulo il criterio? (Area A)
2. Baseline: solo `Reranker` (a) / engine conservativo con `Unassessed` (b) / euristiche (c)? (Area E)
3. Segnali grezzi + soglie nel caso d'uso fase 8 (calibrate 13/14), adapter senza soglie? (Area B5)
4. Circuit breaker scritto a mano vs `failsafe` 1.3.0 (fermo dal 2024)? (Area C4)
5. Routing: porta riceve `Complexity`, adapter mappa a model id da config? (Area D7)
6. `AuditSink`: adapter PG append-only per servizio, `AuditEvent` tipizzato senza payload libero, ownership a discuss 11? Hash chain sì/no? (Area F)
7. Cache: NoOp + fake in-memory con scope per i test della fase 10? (Area G4)
8. Reali: replay in CI + gate locale con spesa limitata? (Area I)
9. `inference_geo` `"us"`/`"global"`, nessuna EU → accettato (corpus pubblico/sintetico, D-16)? (Area D6)

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 04)
