# Requisiti — Corporate Knowledge Assistant

## Requisiti funzionali MVP

| ID | Requisito | Criterio di accettazione sintetico |
|---|---|---|
| FR-001 | Login email/password | Credenziali corrette emettono token; errori non rivelano dettagli |
| FR-002 | Gestione ruoli | `admin` assegna `admin`, `manager`, `employee`, `guest` |
| FR-003 | Upload | PDF, DOCX e Markdown validi producono un job osservabile |
| FR-004 | Indicizzazione | Job riuscito pubblica atomicamente una versione queryable |
| FR-005 | Query autorizzata | Retrieval solo su documenti e chunk accessibili |
| FR-006 | Risposta citata | Ogni claim verificabile contiene una citation valida |
| FR-007 | Viewer in-app | Citation apre versione e pagina corrette con highlight se disponibile |
| FR-008 | Rifiuto | Evidenza insufficiente/contraddittoria produce rifiuto esplicito |
| FR-009 | Audit | Login, upload, ACL, ingestion, query, rifiuti e deny sono tracciati |
| FR-010 | Scoperta online | Un connettore propone nuovi documenti in quarantena |
| FR-011 | Aggiornamento | Nuove versioni non interrompono query sulla versione pubblicata |

Il secondo fattore di autenticazione è escluso dall'MVP.

## Requisiti non funzionali

| ID | Categoria | Requisito |
|---|---|---|
| NFR-001 | Sicurezza | Deny-by-default; nessun chunk negato oltre il Document Service |
| NFR-002 | Password | Hash Argon2id versionato; nessuna cifratura reversibile |
| NFR-003 | Concorrenza | Fan-out e worker bounded con backpressure e deadline |
| NFR-004 | Idempotenza | Retry di upload, job ed eventi non duplica effetti |
| NFR-005 | Tracciabilità | `request_id`, trace e document version in risposta e audit |
| NFR-006 | Estensibilità | Provider dietro porte Rust; tipi SDK confinati agli adapter |
| NFR-007 | Ingestion | Time To Queryable misurato su hardware noto |
| NFR-008 | Evaluation | Evaluation corpus separato dal knowledge corpus dinamico |
| NFR-009 | Degradazione | Fallimento JEV/LLM non bypassa autorizzazione o policy |

## Regole di business

- BR-001 — Un documento scoperto online entra in quarantena.
- BR-002 — JEV produce segnali; il codice conserva la decisione finale.
- BR-003 — Autorizzazione, provenance e mapping citation sono deterministici.
- BR-004 — Il viewer esegue un nuovo controllo di accesso.
- BR-005 — Una risposta non supportata viene rifiutata.
- BR-006 — Le query citano una versione immutabile.
- BR-007 — Nessun servizio legge direttamente le tabelle altrui.
