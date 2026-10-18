# Roadmap: Corporate Knowledge Assistant (CKA)

**Modifica:** Claude (gsd-roadmapper) · 2026-10-18 · creazione della roadmap da ingest (12 fasi, 37/37 requisiti)

## Overview

Il percorso parte dalle fondamenta (workspace Cargo con tre servizi, frontend Vite, Compose con PostgreSQL + `pgvector`, CI) e dagli spike che bloccano le scelte di ingestion e retrieval. Poi costruisce il vertical slice M1 in fasi verticali: identità, upload e indicizzazione, retrieval autorizzato, risposta citata, viewer. M2 completa RBAC end-to-end, guardrail, audit e il corpus incrementale con le fonti online in quarantena. M3 aggiunge la suite di evaluation, l'osservabilità e l'hardening. M4 chiude con demo riproducibile e case study. Ogni fase indica la criticità: le fasi critiche (identità, RBAC + retrieval autorizzato, ingestion) ricevono l'audit dei rischi GPT nel loop.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): lavoro pianificato della milestone
- Decimal phases (2.1, 2.2): inserimenti urgenti (marcati INSERTED)

- [ ] **Phase 1: Fondamenta e spike tecnici** - Workspace, Compose, migrazioni per servizio, CI, threat model e spike che bloccano ingestion e retrieval (M0/M1, critica)
- [ ] **Phase 2: Identità e accesso** - Login email/password, Argon2id, JWT/JWKS, gestione ruoli da admin (M1, critica)
- [ ] **Phase 3: Upload e indicizzazione** - Upload in streaming, job osservabile, parsing con metadati posizionali, pubblicazione atomica idempotente (M1, critica)
- [ ] **Phase 4: Retrieval autorizzato** - Ricerca vettoriale deny-by-default dentro il Document Service (M1, critica)
- [ ] **Phase 5: Risposta citata e rifiuto** - Flusso query RAG con DecisionEngine JEV, citazioni a versione immutabile, rifiuto motivato (M1, normale)
- [ ] **Phase 6: Viewer in-app delle citazioni** - La citazione apre versione e pagina, con evidenziazione quando ci sono coordinate, e ricontrollo di accesso (M1, critica)
- [ ] **Phase 7: RBAC end-to-end e guardrail** - Matrice ruolo × classificazione anti-leakage, revoca durante query, degradazione sicura (M2, critica)
- [ ] **Phase 8: Audit end-to-end e tracciabilità** - Eventi di audit in tutti i servizi, consultazione per ruolo, request_id/trace/versione (M2, critica)
- [ ] **Phase 9: Corpus incrementale e scoperta online** - Nuove versioni senza interruzioni, connettore online, quarantena e approvazione (M2, critica)
- [ ] **Phase 10: Suite di evaluation** - Evaluation corpus versionato e metriche RAG riproducibili con oracoli indipendenti (M3, normale)
- [ ] **Phase 11: Osservabilità, prestazioni e hardening** - Trace distribuita, Time To Queryable misurato, backpressure e race condition (M3, normale)
- [ ] **Phase 12: Deployment, demo e case study** - Demo end-to-end riproducibile da clone pulito e case study (M4, normale)

## Phase Details

### Phase 1: Fondamenta e spike tecnici
**Goal**: Lo sviluppatore avvia l'intero sistema scheletro da un clone pulito con un solo comando, la CI verifica ogni push, e le scelte tecniche che bloccano ingestion e retrieval sono chiuse con misure documentate
**Milestone**: M0 (chiusura lacune) + M1 (prerequisiti)
**Criticità**: critica (gli spike S-01 upload in streaming e S-02/A-05 parser/OCR fissano il design dell'ingestion su file non fidati)
**Depends on**: Nothing (first phase)
**Requirements**: PLAT-01, PLAT-02, PLAT-03, PLAT-06, PLAT-07
**Success Criteria** (what must be TRUE):
  1. Da un clone pulito, `docker compose up` avvia PostgreSQL con `pgvector`, i tre servizi Rust (Identity, Document, RAG) con health check verdi e il frontend Vite raggiungibile nel browser.
  2. La CI GitHub Actions esegue a ogni push fmt/clippy/test del workspace Cargo (3 servizi + `crates/contracts` + `crates/telemetry`), lint/tsc/vitest di `apps/web` e almeno un contract test OpenAPI per servizio, ed è verde.
  3. Ogni servizio applica solo le proprie migrazioni con credenziali proprie; un test dimostra che l'utente DB di un servizio non può leggere le tabelle di un altro.
  4. Le porte Rust (`DocumentStore`, `DocumentParser`, `Chunker`, `Embedder`, `VectorIndex`, `DecisionEngine`, `AnswerGenerator`, `AccessPolicy`, `AuditSink`, `Cache`) esistono come trait compilati con un fake ciascuna, e `crate-boundary-check` conferma che nessun tipo SDK/DB attraversa una porta.
  5. Gli spike S-01 (multipart in streaming su Axum, memoria misurata), S-02 (parser/OCR in-process vs isolato, coordinate disponibili), S-03 (`pgvector` su corpus incrementale), S-04 (forma della chiamata JEV) e S-07 (firme delle porte) hanno un esito con misure registrato in ADR o in DEPENDENCIES.md, e il threat model è compilato con mitigazioni mappate ai requisiti.
**Decisioni da chiudere nel discuss**: A-01 (tooling OpenAPI), A-02 (storage originali), A-03 (embedding model), A-05 (OCR/parser), A-09 (batching JEV), A-10 (isolamento parser); conferma delle Proposte di runtime (Tokio, axum/tower, sqlx, serde, thiserror/anyhow, Vite, Vitest, Playwright)
**Plans**: TBD

### Phase 2: Identità e accesso
**Goal**: Un utente creato dall'admin accede all'applicazione con email e password, e gli altri servizi riconoscono la sua identità e i suoi ruoli senza chiamare l'Identity Service
**Milestone**: M1
**Criticità**: critica (identità e autenticazione)
**Depends on**: Phase 1
**Requirements**: AUTH-01, AUTH-02, AUTH-03
**Success Criteria** (what must be TRUE):
  1. Un utente creato dall'admin accede dalla pagina di login web con email e password e vede la propria identità e i propri ruoli (`GET /api/v1/auth/me`).
  2. Credenziali errate e utente inesistente producono lo stesso errore generico, senza rivelare quale dato è sbagliato.
  3. L'admin assegna e revoca i ruoli `admin`, `manager`, `employee`, `guest`; un non-admin che ci prova riceve un deny.
  4. Le password esistono solo come hash Argon2id con parametri versionati; il refresh ruota il token, il logout revoca la sessione.
  5. Document e RAG Service verificano localmente firma, issuer, audience e scadenza del token tramite JWKS e rifiutano un token alterato o scaduto.
**Decisioni da chiudere nel discuss**: conferma della Proposta "JWT firmati asimmetricamente" (se rifiutata, rivedere C-02); storage dei token nel browser (vincolo `security-scanner`)
**Plans**: TBD
**UI hint**: yes

### Phase 3: Upload e indicizzazione
**Goal**: Un utente autorizzato carica un documento con ACL esplicita e lo vede diventare queryable, con una versione pubblicata atomicamente e chunk che conservano pagina e posizione
**Milestone**: M1
**Criticità**: critica (ingestion: upload, parsing di file non fidati)
**Depends on**: Phase 2
**Requirements**: DOC-01, DOC-02, PLAT-05
**Success Criteria** (what must be TRUE):
  1. Un utente autorizzato carica PDF, DOCX o Markdown dalla UI con ACL esplicita, riceve subito un job e vede le fasi dell'ingestion fino a `queryable` oppure `failed` con diagnostica.
  2. Un file non valido (tipo non ammesso, oltre 10 MB, PDF corrotto o cifrato) è rifiutato o termina in `failed`, e non diventa mai queryable.
  3. Un job riuscito pubblica la versione in una sola transazione: un test che interrompe il worker a metà non lascia mai un indice parziale visibile.
  4. Ripetere lo stesso upload o rieseguire un job o un evento non duplica contenuto fisico, chunk né versioni.
  5. I chunk pubblicati conservano documento, versione, pagina e, quando il parser le fornisce, le coordinate del passaggio.
**Decisioni da chiudere nel discuss**: Q-01 (permessi per ruolo: chi carica, quali classificazioni vede ciascun ruolo), Q-02 (schema di classificazione dei documenti), Q-06 (l'upload manuale passa da una quarantena transitoria?); conferma della Proposta audit/outbox append-only (primo outbox)
**Plans**: TBD
**UI hint**: yes

### Phase 4: Retrieval autorizzato
**Goal**: Il RAG Service ottiene dal Document Service solo candidati che l'utente può vedere, e nessun chunk negato esce mai dal Document Service
**Milestone**: M1
**Criticità**: critica (RBAC + retrieval autorizzato)
**Depends on**: Phase 3
**Requirements**: SEC-01, SEC-02
**Success Criteria** (what must be TRUE):
  1. Una ricerca per conto di un utente restituisce solo chunk di versioni pubblicate di documenti accessibili al suo ruolo; un utente senza permessi riceve zero risultati, senza indizi sull'esistenza dei documenti negati.
  2. Il filtro di autorizzazione è applicato nella query vettoriale stessa, dentro il Document Service: test negativi dimostrano che nessun chunk negato compare in risposta, log o telemetria.
  3. `POST /internal/v1/retrieval/search` senza identità utente valida propagata risponde deny (deny-by-default).
  4. `POST /internal/v1/documents/{id}/authorize` restituisce la decisione corrente, che riflette subito una revoca di ACL.
**Decisioni da chiudere nel discuss**: indice e metrica `pgvector` (dall'esito di S-03); forma di propagazione dell'identità tra servizi
**Plans**: TBD

### Phase 5: Risposta citata e rifiuto
**Goal**: L'utente pone una domanda e riceve una risposta in cui ogni claim è citato a una versione immutabile, oppure un rifiuto motivato
**Milestone**: M1
**Criticità**: normale
**Depends on**: Phase 4
**Requirements**: QRY-01, QRY-02, QRY-03, QRY-04, QRY-05
**Success Criteria** (what must be TRUE):
  1. L'utente pone una domanda dalla UI e riceve una risposta in cui ogni claim verificabile ha una citazione a documento, versione e pagina.
  2. Una domanda senza evidenza nei documenti autorizzati, o con evidenze contraddittorie, produce un rifiuto esplicito con motivazione, non una risposta.
  3. Il risultato salvato (`GET /api/v1/queries/{id}`) continua a puntare alla versione citata anche dopo la pubblicazione di una versione nuova.
  4. Con il fake `DecisionEngine` e con l'adapter JEV reale, i segnali (validazione query, rerank, sufficienza, validazione claim–citazione) influenzano il flusso, ma la decisione finale e il mapping citazione→chunk sono presi dal codice in modo deterministico.
  5. Ogni query ha una deadline esplicita: se scade, l'utente riceve un rifiuto o un errore chiaro, mai una risposta non validata.
**Decisioni da chiudere nel discuss**: A-04 (LLM generativo), A-06 (baseline di reranking di confronto/fallback), A-09 (conferma batching JEV), S-05 (cancellazione verso provider HTTP), Q-05 (conversazioni multi-turno nell'MVP?)
**Plans**: TBD
**UI hint**: yes

### Phase 6: Viewer in-app delle citazioni
**Goal**: Cliccando una citazione l'utente vede il documento alla versione e alla pagina citate, con il passaggio evidenziato quando possibile, solo se è ancora autorizzato (chiusura del vertical slice M1)
**Milestone**: M1
**Criticità**: critica (ricontrollo autorizzativo sul contenuto servito)
**Depends on**: Phase 5
**Requirements**: VIEW-01, VIEW-02
**Success Criteria** (what must be TRUE):
  1. Cliccando una citazione, il viewer in-app apre il documento alla versione e alla pagina citate.
  2. Se il parser ha fornito coordinate, il passaggio citato è evidenziato; altrimenti (PDF scannerizzati, DOCX) il viewer apre la pagina corretta e indica che l'evidenziazione non è disponibile.
  3. Se l'accesso è stato revocato o il documento rimosso dopo la risposta, aprire la citazione produce un deny e il contenuto non viene servito.
  4. Il viewer funziona anche con JEV e LLM non raggiungibili.
  5. Un test E2E Playwright percorre login → upload → queryable → domanda → risposta citata → viewer alla pagina citata.
**Decisioni da chiudere nel discuss**: conferma della Proposta PDF.js; resa di DOCX e Markdown nel viewer (conversione o vista testuale) entro il vincolo "pagina obbligatoria" di VIEW-01
**Plans**: TBD
**UI hint**: yes

### Phase 7: RBAC end-to-end e guardrail
**Goal**: È dimostrato, con test automatici, che nessun ruolo vede contenuti oltre la propria classificazione in nessun canale, e che guasti o input ostili non aggirano i controlli
**Milestone**: M2
**Criticità**: critica (RBAC + retrieval autorizzato)
**Depends on**: Phase 6
**Requirements**: SEC-03, SEC-04, SEC-05
**Success Criteria** (what must be TRUE):
  1. La matrice ruolo × classificazione gira in CI: per ogni combinazione, test positivi e negativi provano che nessun contenuto vietato compare in risposte, citazioni, viewer, log, cache o prompt verso JEV/LLM.
  2. Revocando un ruolo durante una query in corso, il ricontrollo prima della risposta rimuove le citazioni non più autorizzate o rifiuta.
  3. Con JEV o LLM in timeout o errore (fault injection), il sistema rifiuta o degrada secondo policy, con circuit breaker osservabile, senza mai saltare autorizzazione o policy.
  4. Sostituendo JEV con un fake ostile, autorizzazione, provenance e mapping delle citazioni producono le stesse decisioni: sono deterministici.
  5. I documenti con prompt injection del dataset di sicurezza sintetico non alterano le decisioni di accesso né fanno uscire contenuti negati.
**Decisioni da chiudere nel discuss**: composizione del dataset di sicurezza sintetico (C-26); policy di fallback per guasto JEV/LLM (rifiuto, baseline locale o risposta degradata)
**Plans**: TBD

### Phase 8: Audit end-to-end e tracciabilità
**Goal**: Ogni azione rilevante per la sicurezza lascia una traccia ricostruibile, e un utente autorizzato può consultarla dall'applicazione
**Milestone**: M2
**Criticità**: critica (eventi di autenticazione e deny; consultazione filtrata per ruolo)
**Depends on**: Phase 7
**Requirements**: AUD-01, AUD-02, AUD-03
**Success Criteria** (what must be TRUE):
  1. Login riusciti e falliti, upload, modifiche di ACL e ruoli, fasi di ingestion, query, rifiuti e deny generano eventi di audit append-only.
  2. Un utente autorizzato consulta gli eventi dalla UI filtrando per tipo, utente e intervallo; un utente non autorizzato riceve deny.
  3. Ogni risposta e ogni evento di audit riportano `request_id`, trace id e versione del documento coinvolto, e permettono di ricostruire la catena query → evidenze → risposta.
  4. Il retry di un evento non produce duplicati (`event_id` univoco) e l'audit aggregato converge anche dopo il riavvio di un consumer.
  5. Gli eventi di audit non contengono password, token né contenuto di chunk negati.
**Decisioni da chiudere nel discuss**: Q-03 (chi consulta l'audit e con quali filtri), Q-04 (ownership dell'audit aggregato: proiezione via outbox nel RAG Service?)
**Plans**: TBD
**UI hint**: yes

### Phase 9: Corpus incrementale e scoperta online
**Goal**: La knowledge base si aggiorna con nuove versioni e documenti scoperti online senza interrompere le query e senza pubblicare nulla che non abbia superato quarantena e approvazione
**Milestone**: M2
**Criticità**: critica (ingestion da fonti online non fidate)
**Depends on**: Phase 8
**Requirements**: DOC-03, ONL-01, ONL-02, ONL-03
**Success Criteria** (what must be TRUE):
  1. Caricando una nuova versione di un documento pubblicato, le query continuano a rispondere sulla versione precedente finché la nuova non è pubblicata atomicamente; dopo, citano la nuova.
  2. Un connettore su una fonte ufficiale (API, RSS o manifest) scopre in modo incrementale nuovi documenti, che entrano in quarantena con metadati di provenance completi (URL canonico, checksum, licenza, stato di provenance).
  3. Nessun documento scoperto online diventa queryable senza checksum/deduplicazione, policy di provenance e licenza, validazione del file e approvazione esplicita dalla UI di revisione; il triage JEV vi appare solo come segnale consultivo.
  4. Un candidato con URL valido ma metadati falsi, contenuto sospetto o file malformato resta in quarantena o viene rifiutato, con diagnostica visibile.
  5. Il connettore raggiunge solo gli host consentiti (protezione SSRF) e rispetta limiti di dimensione e tempo.
**Decisioni da chiudere nel discuss**: scelta della prima fonte ufficiale e della sua licenza; emendamento di ADR-0002 per il triage JEV dei documenti (consigliato da INGEST-CONFLICTS.md)
**Plans**: TBD
**UI hint**: yes

### Phase 10: Suite di evaluation
**Goal**: La qualità di retrieval, risposte, citazioni e rifiuti è misurata con una suite riproducibile su un evaluation corpus versionato, i cui valori attesi provengono da fonti indipendenti dalla pipeline
**Milestone**: M3
**Criticità**: normale
**Depends on**: Phase 9
**Requirements**: EVAL-01, EVAL-02
**Success Criteria** (what must be TRUE):
  1. L'evaluation corpus è versionato e immutabile per release, separato dal knowledge corpus: i documenti scoperti online non vi entrano mai e ogni modifica produce una nuova versione del benchmark.
  2. Un comando (o `POST /internal/v1/evaluations`) esegue la suite e produce un report versionato con recall@k, faithfulness, citation correctness, answer relevance, refusal accuracy, leakage rate, latenza e costo/query.
  3. Ogni run registra versione del corpus, configurazione, modelli LLM/embedding e la versione JEV effettiva risolta dall'alias, così che una regressione sia attribuibile.
  4. Ogni valore atteso ha superato il triplo guardrail (`eval-oracle-check`: Claude, double-check GPT, utente); le soglie sono definite e la CI fallisce su regressione.
  5. Il report confronta il reranking JEV con la baseline scelta in A-06 e documenta la calibrazione delle soglie JEV.
**Decisioni da chiudere nel discuss**: Q-07 (registrazione della versione JEV effettiva); soglie delle metriche; uso di TAT-DQA e verifica delle condizioni d'uso di FinanceBench
**Plans**: TBD

### Phase 11: Osservabilità, prestazioni e hardening
**Goal**: Il comportamento del sistema sotto carico è visibile e misurato, e i limiti di concorrenza reggono senza degradare la sicurezza
**Milestone**: M3
**Criticità**: normale
**Depends on**: Phase 10
**Requirements**: PLAT-04, DOC-04, OBS-01
**Success Criteria** (what must be TRUE):
  1. Una richiesta che attraversa i tre servizi appare come un'unica trace in locale, con metriche di saturazione di pool, code e fan-out.
  2. Il Time To Queryable è misurato su hardware documentato per i profili del README (upload ack, PDF 25 e 100 pagine, scannerizzato 25 pagine, job fallito visibile) con report p95, e i target sono riesaminati.
  3. Oltre la soglia di coda, i servizi rispondono `429` con `Retry-After`; ogni fan-out ha limite, deadline, cancellazione e budget di retry verificati da test.
  4. Le race condition elencate in interfaces-and-concurrency.md (upload identici, cancellazione durante ingestion, nuova versione durante query, revoca durante query, documento rimosso, retry audit, timeout JEV) hanno test automatici verdi.
  5. La decisione sulla cache (A-07) è presa su misure; se una cache viene introdotta, la sua invalidazione è testata (S-06).
**Decisioni da chiudere nel discuss**: A-07 (NoOpCache vs Redis), S-06 (invalidazione cache, solo se introdotta); hardware di riferimento per il benchmark
**Plans**: TBD

### Phase 12: Deployment, demo e case study
**Goal**: Chiunque cloni il repository può riprodurre la demo MVP end-to-end, e il progetto è presentabile come case study portfolio
**Milestone**: M4
**Criticità**: normale
**Depends on**: Phase 11
**Requirements**: DLV-01, DLV-02
**Success Criteria** (what must be TRUE):
  1. Da un clone pulito, seguendo il runbook, la demo gira con Docker Compose: login → upload → queryable → domanda → risposta citata → viewer alla pagina citata → audit.
  2. Script e dataset di demo sono versionati; la suite di evaluation e i test RBAC anti-leakage sono verdi sull'artefatto usato per la demo.
  3. Le immagini sono non-root, con health check e filesystem read-only dove possibile; nessun segreto è presente in immagini o file Compose versionati.
  4. Il case study descrive problema, architettura, decisioni (ADR), risultati di evaluation e limiti noti.
**Plans**: TBD

## Decisioni aperte per fase

Le decisioni restano aperte: ciascuna va chiusa nel `discuss` della fase indicata.

| Voce | Argomento | Fase |
|------|-----------|------|
| A-01 | Tooling OpenAPI (`utoipa` vs contract-first) | 1 |
| A-02 | Storage originali (MinIO vs `bytea`) | 1 |
| A-03 | Embedding model | 1 |
| A-04 | LLM generativo | 5 |
| A-05 | OCR/parser | 1 |
| A-06 | Baseline di reranking di confronto | 5 |
| A-07 | Caching (`NoOpCache` vs Redis) | 11 |
| A-08 | Container | chiusa (U-01) |
| A-09 | Batching JEV | 1 (spike S-04), conferma in 5 |
| A-10 | Parser in-process vs isolato (sempre nel Document Service) | 1 |
| S-01, S-02, S-03, S-04, S-07 | Spike che bloccano le fasi successive | 1 |
| S-05 | Cancellazione verso provider HTTP | 5 |
| S-06 | Invalidazione cache (se introdotta) | 11 |
| Q-01 | Permessi per ruolo | 3 |
| Q-02 | Schema di classificazione dei documenti | 3 |
| Q-03 | Chi consulta l'audit | 8 |
| Q-04 | Ownership dell'audit aggregato | 8 |
| Q-05 | Conversazioni multi-turno nell'MVP | 5 |
| Q-06 | Quarantena per l'upload manuale | 3 |
| Q-07 | Versione JEV effettiva nell'evaluation | 10 |
| Proposta JWT asimmetrici | Token e JWKS | 2 |
| Proposta PDF.js | Viewer | 6 |

## Progress

**Execution Order:**
Le fasi si eseguono in ordine numerico: 1 → 2 → 3 → … → 12 (le fasi decimali si inseriscono tra gli interi).

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Fondamenta e spike tecnici | 0/TBD | Not started | - |
| 2. Identità e accesso | 0/TBD | Not started | - |
| 3. Upload e indicizzazione | 0/TBD | Not started | - |
| 4. Retrieval autorizzato | 0/TBD | Not started | - |
| 5. Risposta citata e rifiuto | 0/TBD | Not started | - |
| 6. Viewer in-app delle citazioni | 0/TBD | Not started | - |
| 7. RBAC end-to-end e guardrail | 0/TBD | Not started | - |
| 8. Audit end-to-end e tracciabilità | 0/TBD | Not started | - |
| 9. Corpus incrementale e scoperta online | 0/TBD | Not started | - |
| 10. Suite di evaluation | 0/TBD | Not started | - |
| 11. Osservabilità, prestazioni e hardening | 0/TBD | Not started | - |
| 12. Deployment, demo e case study | 0/TBD | Not started | - |
