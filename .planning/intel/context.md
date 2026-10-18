# Contesto — intel sintetizzata

Generato da gsd-doc-synthesizer il 2026-10-18 (MODE: new).
Fonti DOC: docs/architecture/api-contract.md, docs/architecture/data-model.md, docs/quality/llm-evaluation.md, docs/quality/test-plan.md, docs/security/threat-model.md, deliverables/checklist.md. Più contesto non normativo da README.md e docs/product/project-charter.md, e decisioni utente del 2026-10-18.
Contenuto riportato testualmente o quasi, con attribuzione. Ogni voce ha `source:`.

---

## Topic: Visione e ipotesi di prodotto

- source: docs/product/project-charter.md
  > I dipendenti devono trovare risposte attendibili nei documenti aziendali senza poter vedere contenuti oltre il proprio ruolo. Il sistema deve mostrare le fonti e rifiutare risposte non supportate.
- source: README.md
  > Assistente RAG aziendale per documenti finanziari e societari, con autenticazione, RBAC, citazioni verificabili, visualizzazione in-app, audit e aggiornamento incrementale della knowledge base.
- Dominio iniziale finanziario/societario: documenti reali ricchi di testo, tabelle e grafici. source: README.md
- Stato del progetto: "Architecture definition" — contratti, concorrenza e dipendenze prima del primo vertical slice; il vertical slice verrà implementato dopo l'approvazione dei contratti elementari e del modello di concorrenza. source: README.md

## Topic: Milestone di consegna

- source: deliverables/checklist.md (tutte non completate)
  - M0 — Requisiti, threat model e ADR iniziali.
  - M1 — Vertical slice: upload -> retrieval -> risposta citata.
  - M2 — RBAC, guardrail e audit end-to-end.
  - M3 — Evaluation suite, osservabilità e hardening.
  - M4 — Deployment, demo e case study.
- Utile per il roadmapper come scheletro di fasi. Nota: M0 è parzialmente coperto (requisiti e ADR esistono; threat model è uno stub).

## Topic: Risorse API iniziali

- source: docs/architecture/api-contract.md
  > Risorse iniziali: auth, users, documents, ingestion jobs, queries e audit events.
- Il dettaglio degli endpoint esiste già in docs/architecture/interfaces-and-concurrency.md (constraints.md C-02..C-04); lo stub non aggiunge né contraddice nulla.

## Topic: Modello dati iniziale

- source: docs/architecture/data-model.md
  > Entità iniziali: User, Role, Document, DocumentPermission, Chunk, IngestionJob, Conversation, Query e AuditEvent.
- Nessun campo, relazione o schema definito.
- Osservazione: `Conversation` è citata qui e nelle responsabilità del RAG Service (docs/architecture/architecture.md: "conversazioni"), ma nessun requisito funzionale (FR) definisce conversazioni multi-turno. Da chiarire se è nell'MVP.
- Osservazione: mancano entità per `DocumentVersion`, `Source`/provenance, quarantena/approvazione, outbox ed evaluation, tutte richieste da ADR-0002, constraints.md C-08, C-20, C-23, C-25.

## Topic: Metriche di valutazione LLM/RAG

- source: docs/quality/llm-evaluation.md
  > Metriche minime: recall@k, faithfulness, citation correctness, answer relevance, refusal accuracy, leakage rate, latenza e costo/query.
- Nessuna soglia definita. Le soglie JEV vanno calibrate su dati etichettati (ADR-0002 conseguenze).

## Topic: Strategia di test

- source: docs/quality/test-plan.md
  > La suite deve includere una matrice ruolo × classificazione documento e test negativi che provino l'assenza di leakage.
- Correlati: race condition da testare (constraints.md C-09), dataset di sicurezza sintetico (C-26), E2E Playwright su login/upload/query/citation/RBAC (decisions.md, PROPOSTA), fake `DecisionEngine` nei test unitari (C-27).
- Osservazione: la "classificazione documento" (livelli di riservatezza) non è definita in nessun documento; è necessaria per costruire la matrice.

## Topic: Priorità di sicurezza (threat model)

- source: docs/security/threat-model.md
  > Priorità: broken access control, prompt injection indiretta, file malevoli, esfiltrazione tramite citazioni/log, poisoning e furto di token.
- Mitigazioni già presenti in altri documenti: deny-by-default (NFR-001), nessun chunk non autorizzato verso reranker/JEV/LLM/cache/log (README.md), quarantena e provenance (C-21, C-25), segnali di prompt injection da JEV solo come segnale (C-22, BR-002), ricontrollo autorizzativo nel viewer (BR-004).

## Topic: Struttura prevista del repository

- source: README.md
  - `apps/web/` — frontend React/TypeScript e visualizzatore documenti
  - `services/identity-service/` — Identity & Access in Rust
  - `services/document-service/` — Document & Indexing in Rust e worker associati
  - `services/rag-service/` — RAG & Answer in Rust
  - `crates/` — librerie Rust interne, senza condivisione dei modelli di dominio tra servizi
  - `tests/` — unit, integration, E2E, security, evaluation
  - `infra/` — container, configurazioni locali, deployment
  - `docs/` — requisiti, architettura, ADR, sicurezza, operations
  - `evaluation/` — dataset manifest, configurazioni e risultati versionati

## Topic: Ruolo di JEV (sintesi descrittiva)

- source: README.md
  > JEV è un decision engine, non un generatore di testo e non un'autorità di sicurezza.
  - Usi: validazione semantica e classificazione della query; relevance filtering e reranking; rilevazione di evidenze insufficienti o contraddittorie; supporto alla decisione di rifiuto; validazione semantica claim-citazione; triage opzionale di pagine e immagini.
  - RBAC, provenienza dei chunk, link delle citazioni, validazione dei file e decisioni irreversibili restano deterministici.
- Riferimento normativo: ADR-0002 (decisions.md, LOCKED).

## Topic: Decisioni utente successive ai documenti (2026-10-18)

- source: decisione utente comunicata dall'orchestratore /gsd:ingest-docs
  - Ambiente locale: Docker Desktop + Docker Compose (chiude la decisione aperta sui container; vedi decisions.md U-01).
  - Metodologia di sviluppo: loop a 4 verbi GSD+ECC (vedi CLAUDE.md). Riguarda il processo, non il prodotto; non produce requisiti.

## Topic: Lacune (stub che rimandano a template esterni)

Questi documenti sono stub non completati che rimandano a template condivisi in `C:\Users\Gianmarco\Documents\Portfolio projects\00-shared-templates\` (i template esistono). Non sono conflitti; sono lacune da colmare, plausibilmente nella fase M0 o nelle fasi che li richiedono.

- docs/product/project-charter.md -> 00-shared-templates/01-project-charter.md (solo ipotesi iniziale)
- docs/architecture/api-contract.md -> 00-shared-templates/04-api-contract.md (solo elenco risorse)
- docs/architecture/data-model.md -> 00-shared-templates/05-data-model.md (solo elenco entità)
- docs/security/threat-model.md -> 00-shared-templates/06-threat-model.md (solo priorità)
- docs/quality/test-plan.md -> 00-shared-templates/07-test-plan.md (un solo requisito di test)
- docs/quality/llm-evaluation.md -> 00-shared-templates/08-llm-evaluation.md (solo elenco metriche, nessuna soglia)
- deliverables/checklist.md -> 00-shared-templates/14-delivery-checklist.md (solo milestone)

Template condivisi senza documento corrispondente nel progetto (possibili lacune future): 09-deployment-runbook, 10-observability, 11-risk-register, 13-portfolio-case-study, 15-demo-script.

## Topic: Domande aperte di prodotto (non conflitti)

Emerse dalla sintesi; nessun documento le risolve.

- Q-01: Permessi per ruolo — cosa possono fare `manager`, `employee`, `guest` (upload, consultazione audit, livelli di classificazione visibili)? source: lacuna tra docs/product/requirements.md FR-002 e README.md
- Q-02: Schema di classificazione dei documenti (livelli di riservatezza) richiesto dalla matrice di test. source: docs/quality/test-plan.md
- Q-03: Chi può consultare l'audit e con quali filtri. source: README.md (MVP punto 7), docs/architecture/interfaces-and-concurrency.md (`GET /api/v1/audit-events`)
- Q-04: Ownership dell'audit aggregato — gli eventi nascono in Identity, Document e RAG, ma la consultazione è esposta dal RAG Service; docs/architecture/architecture.md indica "consistenza eventuale per proiezioni e audit aggregato", implicando una proiezione via outbox. Da confermare.
- Q-05: Conversazioni multi-turno nell'MVP? (entità `Conversation`, nessun FR).
- Q-06: L'upload manuale passa da una quarantena? docs/architecture/architecture.md mostra "upload/discovery -> quarantine"; docs/data/corpus-strategy.md richiede quarantena obbligatoria solo per la scoperta online e per l'upload manuale "pubblicazione dopo validazione e ACL esplicita". Compatibili se la quarantena per l'upload è solo uno stato transitorio di validazione.
- Q-07: Riproducibilità dell'evaluation con l'alias `jev-latest` — registrare la versione effettiva del modello JEV nei risultati di evaluation (vedi INFO in INGEST-CONFLICTS.md).
