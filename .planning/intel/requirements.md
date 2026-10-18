# Requisiti — intel sintetizzata

Generato da gsd-doc-synthesizer il 2026-10-18 (MODE: new). Fonti PRD: docs/product/requirements.md, README.md, docs/product/project-charter.md.
Formato ID: `REQ-{slug}`; l'ID originale (FR/NFR/BR) è conservato tra parentesi. Ogni voce ha `source:`.
Ambito: MVP salvo diversa indicazione.

Nota sulle varianti: dove due PRD descrivono lo stesso requisito con criteri diversi, le varianti sono conservate separatamente e marcate COMPETING (vedi INGEST-CONFLICTS.md). Non sono state fuse.

---

## Requisiti funzionali

### REQ-login-email-password (FR-001)
- source: docs/product/requirements.md
- descrizione: Login con email e password.
- accettazione: Credenziali corrette emettono token; gli errori non rivelano dettagli.
- corroborato da: README.md (MVP punto 1: "accedere con email e password")
- vincoli correlati: NFR-002 (Argon2id), MFA escluso (D-0001-f)

### REQ-role-management (FR-002)
- source: docs/product/requirements.md
- descrizione: Gestione ruoli.
- accettazione: `admin` assegna i ruoli `admin`, `manager`, `employee`, `guest`.
- corroborato da: README.md ("Ruoli iniziali: admin, manager, employee, guest")
- lacuna: i permessi di ciascun ruolo (chi può caricare, chi consulta l'audit, quali classificazioni documento vede ciascun ruolo) non sono definiti in nessun documento. Vedi context.md, domande aperte.

### REQ-document-upload (FR-003)
- source: docs/product/requirements.md
- descrizione: Upload di documenti.
- accettazione: PDF, DOCX e Markdown validi producono un job osservabile.
- corroborato da: README.md (MVP punti 2-3: caricare PDF/DOCX/Markdown e seguirne l'indicizzazione asincrona fino allo stato `queryable`)

### REQ-indexing-atomic-publish (FR-004)
- source: docs/product/requirements.md
- descrizione: Indicizzazione.
- accettazione: Un job riuscito pubblica atomicamente una versione queryable.
- corroborato da: README.md (MVP punto 3), docs/architecture/architecture.md (pubblicazione atomica), docs/adr/0002-incremental-corpus-and-jev.md (conseguenze)

### REQ-authorized-query (FR-005)
- source: docs/product/requirements.md
- descrizione: Query autorizzata.
- accettazione: Retrieval solo su documenti e chunk accessibili all'utente.
- corroborato da: README.md (MVP punto 4: "porre una domanda sui soli documenti autorizzati"), docs/product/project-charter.md ("senza poter vedere contenuti oltre il proprio ruolo")

### REQ-cited-answer (FR-006)
- source: docs/product/requirements.md
- descrizione: Risposta con citazioni.
- accettazione: Ogni claim verificabile contiene una citation valida.
- corroborato da: README.md (MVP punto 5), docs/product/project-charter.md ("deve mostrare le fonti")

### REQ-in-app-viewer (FR-007) — COMPETING VARIANTS
- descrizione: Viewer in-app che apre il documento citato.
- variante A
  - source: docs/product/requirements.md (FR-007)
  - accettazione: La citation apre versione e pagina corrette, con highlight **se disponibile**.
- variante B
  - source: README.md (MVP punto 6)
  - accettazione: Aprire il documento nell'app **alla pagina e al passaggio citato** (posizionamento a livello di passaggio, senza clausola "se disponibile"; nessuna menzione esplicita della versione).
- contesto tecnico (non risolve la variante): docs/architecture/architecture.md richiede "citazioni riproducibili fino a versione, pagina e posizione" e "chunk + positional metadata" nella pipeline di ingestion.
- stato: da risolvere dall'utente (WARNING in INGEST-CONFLICTS.md).
- regola correlata: BR-004 (il viewer esegue un nuovo controllo di accesso).

### REQ-refusal (FR-008)
- source: docs/product/requirements.md
- descrizione: Rifiuto.
- accettazione: Evidenza insufficiente o contraddittoria produce un rifiuto esplicito.
- corroborato da: docs/product/project-charter.md ("rifiutare risposte non supportate"), BR-005

### REQ-audit-logging (FR-009)
- source: docs/product/requirements.md
- descrizione: Audit.
- accettazione: Login, upload, ACL, ingestion, query, rifiuti e deny sono tracciati.

### REQ-audit-consultation
- source: README.md (MVP punto 7)
- descrizione: Un utente, se autorizzato, può consultare gli eventi di audit.
- accettazione: non specificata nel PRD; endpoint previsto `GET /api/v1/audit-events` "audit secondo ruolo e filtri" (source: docs/architecture/interfaces-and-concurrency.md).
- nota: complementare a FR-009 (FR-009 copre la registrazione, questo la consultazione). Ruoli autorizzati non definiti.

### REQ-online-discovery (FR-010)
- source: docs/product/requirements.md
- descrizione: Scoperta online.
- accettazione: Un connettore propone nuovi documenti in quarantena.
- corroborato da: README.md (sezione "Dominio e corpus"), docs/adr/0002-incremental-corpus-and-jev.md (D-0002-b), docs/data/corpus-strategy.md

### REQ-document-update-no-disruption (FR-011)
- source: docs/product/requirements.md
- descrizione: Aggiornamento dei documenti.
- accettazione: Le nuove versioni non interrompono le query sulla versione pubblicata.

Esclusione: il secondo fattore di autenticazione è escluso dall'MVP. source: docs/product/requirements.md, README.md

---

## Requisiti non funzionali

### REQ-deny-by-default (NFR-001)
- source: docs/product/requirements.md
- categoria: Sicurezza
- requisito: Deny-by-default; nessun chunk negato esce dal Document Service.
- corroborato da: README.md (Principi: "Deny by default"; "Nessun chunk non autorizzato raggiunge reranker, JEV, LLM, cache o log"), docs/architecture/architecture.md (Driver)

### REQ-password-argon2id (NFR-002)
- source: docs/product/requirements.md
- categoria: Password
- requisito: Hash Argon2id versionato; nessuna cifratura reversibile.
- corroborato da: README.md, DEPENDENCIES.md (DECISA)

### REQ-bounded-concurrency (NFR-003)
- source: docs/product/requirements.md
- categoria: Concorrenza
- requisito: Fan-out e worker bounded con backpressure e deadline.
- corroborato da: README.md (Principi: "Concorrenza limitata e backpressure; nessun fan-out senza limite")

### REQ-idempotency (NFR-004)
- source: docs/product/requirements.md
- categoria: Idempotenza
- requisito: Il retry di upload, job ed eventi non duplica effetti.
- corroborato da: README.md (Principi: "Ogni operazione ripetibile espone un'idempotency key o usa un identificatore stabile")

### REQ-traceability (NFR-005)
- source: docs/product/requirements.md
- categoria: Tracciabilità
- requisito: `request_id`, trace e document version presenti in risposta e audit.

### REQ-ports-extensibility (NFR-006)
- source: docs/product/requirements.md
- categoria: Estensibilità
- requisito: Provider dietro porte Rust; tipi SDK confinati agli adapter.
- corroborato da: README.md (Principi: "Gli adapter esterni dipendono da porte interne stabili"), docs/adr/0001-system-boundaries-and-stack.md (D-0001-e)

### REQ-time-to-queryable (NFR-007)
- source: docs/product/requirements.md
- categoria: Ingestion
- requisito: Time To Queryable misurato su hardware noto.
- target iniziali: vedi constraints.md C-31 (source: README.md, target p95 iniziali da riesaminare dopo baseline). Complementari, non concorrenti (vedi INFO in INGEST-CONFLICTS.md).

### REQ-evaluation-corpus-separation (NFR-008)
- source: docs/product/requirements.md
- categoria: Evaluation
- requisito: Evaluation corpus separato dal knowledge corpus dinamico.
- corroborato da: docs/adr/0002-incremental-corpus-and-jev.md (D-0002-a), README.md, docs/data/corpus-strategy.md

### REQ-safe-degradation (NFR-009)
- source: docs/product/requirements.md
- categoria: Degradazione
- requisito: Il fallimento di JEV/LLM non bypassa autorizzazione o policy.
- corroborato da: docs/architecture/interfaces-and-concurrency.md (Backpressure e degradazione)

### REQ-contract-versioning-tests
- source: README.md (Principi)
- requisito: I contratti tra servizi sono versionati e testati.
- corroborato da: docs/adr/0001-system-boundaries-and-stack.md (conseguenze: contract test)

### REQ-untrusted-online-source
- source: README.md (Principi)
- requisito: Un documento online non è attendibile solo perché proviene da un URL valido.
- correlato: BR-001, REQ-online-discovery

---

## Regole di business

### REQ-online-quarantine (BR-001)
- source: docs/product/requirements.md
- regola: Un documento scoperto online entra in quarantena.

### REQ-jev-signals-only (BR-002)
- source: docs/product/requirements.md
- regola: JEV produce segnali; il codice conserva la decisione finale.

### REQ-deterministic-security (BR-003)
- source: docs/product/requirements.md
- regola: Autorizzazione, provenance e mapping citation sono deterministici.
- corroborato da: README.md ("RBAC, provenienza dei chunk, link delle citazioni, validazione dei file e decisioni irreversibili restano deterministici"), docs/adr/0002-incremental-corpus-and-jev.md (D-0002-d)

### REQ-viewer-access-recheck (BR-004)
- source: docs/product/requirements.md
- regola: Il viewer esegue un nuovo controllo di accesso.

### REQ-unsupported-refusal (BR-005)
- source: docs/product/requirements.md
- regola: Una risposta non supportata viene rifiutata.
- corroborato da: docs/product/project-charter.md

### REQ-immutable-cited-version (BR-006)
- source: docs/product/requirements.md
- regola: Le query citano una versione immutabile.

### REQ-no-cross-service-tables (BR-007)
- source: docs/product/requirements.md
- regola: Nessun servizio legge direttamente le tabelle altrui.
- corroborato da: DEPENDENCIES.md (migrazioni per servizio), docs/architecture/architecture.md (nessuna join cross-service)

---

## Riepilogo

- Funzionali: 12 (FR-001..FR-011 + REQ-audit-consultation)
- Non funzionali: 11 (NFR-001..NFR-009 + REQ-contract-versioning-tests + REQ-untrusted-online-source)
- Regole di business: 7 (BR-001..BR-007)
- Totale: 30
- Con varianti concorrenti: 1 (REQ-in-app-viewer)
