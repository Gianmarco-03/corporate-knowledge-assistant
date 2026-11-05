# Phase 6: Upload e indicizzazione - Context

**Gathered:** 2026-11-05 (prima parte: 2026-10-26, area «Ruoli e classificazione»)
**Status:** Ready for planning
**Criticità:** critica. È proposta dal ROADMAP («ingestion: upload, orchestrazione del parsing di file non fidati») e l'utente l'ha ratificata con questo CONTEXT il 2026-11-05. In `plan`: `implementation-risk-audit` e `test-design` di GPT via AgentBridge.

<domain>
## Phase Boundary

La fase costruisce il percorso che porta un documento dal browser allo stato `queryable`, nel Document Service:

- **upload in streaming** con ACL esplicita (`POST /api/v1/documents`), validazione, idempotenza e admission control;
- **coda e worker** di ingestion, con la pipeline per versione: validazione, parsing, chunking, embedding, pubblicazione atomica. Usa i provider della fase 3 solo tramite le porte;
- **arricchimento delle tabelle con Docling in background**, dopo la prima pubblicazione (D-21);
- **stato del job** (`GET /api/v1/ingestion-jobs/{id}`) con diagnostica;
- **modifica dell'ACL** e **tombstone** di un documento (D-22, D-25);
- **GC degli orfani**, con tabella `pending` e protocollo `live|deleting` (03 D-03, ADR-0003);
- **outbox e relay** verso `AuditSink` (D-18);
- **feature upload di `apps/web`**: pagina di upload, stato del job, elenco «i miei caricamenti» con modifica dell'ACL e cancellazione.

La fase non costruisce:
- **`POST /api/v1/documents/{id}/versions`** e la relativa UI: fase 12, DOC-03 (D-23);
- **elenco dei documenti visibili** (`GET /api/v1/documents` filtrato per visibilità) e `AccessPolicy`: fase 7;
- **retrieval autorizzato**: fase 7;
- **viewer e `GET /documents/{id}/content`**: fase 9;
- **quarantena con approvazione umana** e scoperta online: fase 12;
- **antivirus e CDR**: rinviati alla fase 14 (ADR-0013, T-39);
- **misura del TTQ di prodotto**: fase 14, DOC-04.

Requisiti: DOC-01, DOC-02, PLAT-05. Criteri di successo 1-5 del ROADMAP, fase 6, con il criterio 2 da correggere (D-07). Chiude Q-01, Q-02 e Q-06, che vincolano le fasi 3, 7 e 10.

Ricerca a supporto: `06-RICERCA-DISCUSSIONE.md`, Aree A-J, con fonti (URL e data di accesso 2026-10-26, oppure file e sezione del repo). Ogni decisione qui sotto cita l'area che la fonda.

**Rischio di pipeline (spec §8).**
- La fase 1 non è chiusa: il secondo giro di `/verify 1` ha dato `gaps_found` e ha inserito la gap-closure 1.7 (`ORCHESTRATOR-STATUS.md`, stesso giorno, altra sessione).
- Le fasi 2, 3, 4 e 5 sono discusse ma non pianificate. Sulla fase 03.1 (grafo di conoscenza), inserita il 2026-11-05, il discuss è in corso.
- La WIRING-MAP non ha ancora righe: i seam consumati qui esistono solo come promesse nei CONTEXT 02, 03, 04 e 05. Se `wiring-design` ne cambia la forma, i vincoli sotto si aggiornano.

</domain>

<decisions>
## Implementation Decisions

### Ruoli e classificazione: Q-01 e Q-02 (Aree B, C)
- **D-01: Quattro livelli di classificazione**, tradotti in insiemi di etichette scritti con il documento:

  | Livello | Etichette |
  |---|---|
  | `public` | `role:guest`, `role:employee`, `role:manager`, `role:admin` |
  | `internal` | `role:employee`, `role:manager`, `role:admin` |
  | `confidential` | `role:manager`, `role:admin` |
  | `restricted` | `role:admin` + `user:<uuid>` degli utenti nominati |

  In ogni caso si aggiunge `user:<uploader>`. Il livello è una scelta obbligatoria, senza default: è l'«ACL esplicita» del goal. L'indice resta agnostico: vede solo etichette, e un insieme vuoto vede zero documenti.
  - Fonte: 03 D-06 e D-07; `docs/quality/test-plan.md` («matrice ruolo × classificazione»); Microsoft Purview (livelli ordinati, pochi); Area B. Decisione utente.
- **D-02: Utenti nominati solo su `restricted`; l'uploader è sempre incluso.** La matrice della fase 10 resta ruolo × livello, con una sola dimensione in più su `restricted`.
  - Fonte: Area B, variante B2 limitata. Decisione utente.
- **D-03: Caricano solo manager e admin.** `employee` e `guest` ricevono 403. Un manager può assegnare tutti e quattro i livelli, compreso `restricted`: può leggerlo tramite `user:<uploader>`, quindi non assegna un livello che non potrebbe leggere.
  - Fonte: OWASP File Upload CS («Only allow authorized users to upload files»); Area C. Decisione utente, **contro la raccomandazione** (tutti tranne guest).
- **D-04: Gestiscono ACL, livello e cancellazione l'uploader e gli admin.** L'uploader deve avere **ancora oggi** il ruolo manager o admin, letto dal token verificato. Un uploader declassato perde la gestione, che resta agli admin. Conserva la lettura tramite l'etichetta `user:<uuid>`.
  - Fonte: principio del minimo privilegio; 05 D-16 (il cambio di ruolo revoca le sessioni); Area C. Decisione utente.
- **D-05: Vedono il job e la sua diagnostica l'uploader e gli admin**, non chiunque veda il documento.
  - Fonte: Area C. Decisione utente.

  Matrice risultante (Q-01):

  | Azione | admin | manager | employee | guest |
  |---|---|---|---|---|
  | Upload | sì, tutti i livelli | sì, tutti i livelli | no | no |
  | Vedere stato del job | tutti | propri | — | — |
  | Cambiare ACL o livello | tutti | propri | no | no |
  | Tombstone | tutti | propri | no | no |

### Quarantena, validazione e stati: Q-06 (Area A)
- **D-06: Nessuna approvazione umana sull'upload manuale.** La quarantena dell'upload è solo logica: finché non è pubblicato, nessun contenuto è visibile. Un PDF attivo (`/JavaScript`, `/Launch`) finisce in `quarantined`, stato **terminale**: mai queryable, motivo visibile a uploader e admin, nessun rilascio. Chi vuole indicizzarlo carica un PDF ripulito. L'approvazione umana resta solo alla fase 12.
  - Fonte: `docs/data/corpus-strategy.md:88-89`; ADR-0003 (D-05); T-10; OWASP File Upload CS; Area A. Decisione utente.
- **D-07: Il criterio 2 del ROADMAP si allinea agli ADR.** Rifiuto oltre **25 MiB**; i PDF che richiedono una password finiscono in `failed` (`Encrypted`); quelli con password utente vuota sono ammessi. Il target del TTQ resta promesso solo fino a 10 MB: fra 10 e 25 MiB il file si indicizza, senza promesse di tempo.
  - Fonte: ADR-0003 (limite su tre livelli); ADR-0004 e ripasso 01-07 del 2026-10-21; `README.md:82`; Area A. Decisione utente.
- **D-08: Rifiuto su due livelli.**
  - Difetti visibili all'upload (tipo falso, zip bomb, DTD, oltre il limite): 413, 415 o 422 immediato, nessun documento né job, solo l'evento `upload_rejected`.
  - PDF attivo visto all'upload: job `quarantined` col motivo, **senza conservare il file**.
  - Difetti trovati dal worker: job `failed` o `quarantined`.
  - Fonte: ADR-0003 (validazione prima di storage e DB, admission control contro T-05); T-06, T-07, T-10; Area A. Decisione utente.
- **D-09: Stati del job.** In corso: `received`, `validating`, `parsing`, `chunking`, `embedding`, `publishing`. Terminali: `queryable`, `failed`, `quarantined`. La cancellazione durante l'ingestion (D-22) può aggiungere un terminale `cancelled`, da fissare in `plan`.
  - Fonte: criterio 1 del ROADMAP; ADR-0004 (esiti tipizzati); Area A. Approvato dall'utente.
- **D-10: Diagnostica a codici.** L'uploader vede il codice tipizzato di ADR-0004 (`Encrypted`, `Corrupt`, `DecompressionLimit`, …), un messaggio fisso scritto da noi e la fase del fallimento. L'admin vede in più il `request_id`. Stderr del parser e dettagli (es. quale tetto) restano nei log redatti.
  - Fonte: ADR-0004; 02 D-05 e D-20; regola no-leak di CLAUDE.md. Decisione utente.

### Coda, retry e concorrenza (Area D)
- **D-11: Coda su tabella Postgres, scritta a mano.** `ingestion_jobs` è insieme la coda e il modello di stato della UI. Claim con `FOR UPDATE SKIP LOCKED`, `lease_until` e heartbeat. Nessun crate di coda, nessun broker.
  - Fonte: `interfaces-and-concurrency.md` §Upload e ingestion; `DEPENDENCIES.md` §Da evitare; PostgreSQL SELECT (SKIP LOCKED per tabelle a coda); 04 D-07 come precedente; Area D. Decisione utente.
- **D-12: Retry per classe di errore.**
  - Permanenti, subito al terminale: `Corrupt`, `Encrypted`, `Unsupported`, `DecompressionLimit`, quarantena.
  - Transitori (TEI, Garage, DB): 5 tentativi con backoff esponenziale, poi `failed`.
  - Fonte: ADR-0004; River job retries come riferimento contrario (25 tentativi, pensati per job generici); Area D. Decisione utente.
- **D-13: Parser in `Crashed`, `Timeout` o `ResourceLimit`: un solo nuovo tentativo**, poi `failed`. Copre l'OOM dovuto al carico, perché il budget del worker è condiviso, e limita a due i figli isolati che un file ostile può consumare.
  - Fonte: ADR-0004 (2 job `parse`, budget del cgroup); T-09. Decisione utente.
- **D-14: Un worker morto riparte da zero, e il tentativo si conta al claim.** Così una poison pill, un file che uccide l'intero worker, esaurisce i tentativi invece di bloccare la coda. Nessun artefatto intermedio: niente testo estratto a riposo da proteggere e pulire. Il costo è rifare OCR ed embedding dopo un crash.
  - Fonte: 03 D-09 (pubblicazione in una transazione); criterio 3 del ROADMAP; ragionamento di discussione (nessuna fonte esterna specifica). Decisione utente.
- **D-15: Tetti ai job attivi, per uploader e globale.** Tetto dell'uploader: **429** + `Retry-After`. Tetto globale: **503** + `Retry-After`. Claim FIFO. Valori in `plan`.
  - Fonte: 02 D-06; ADR-0003 (admission control); stima di discussione: circa 8 documenti al minuto con 2 job `parse`. Decisione utente.

### Idempotenza e deduplica (Area E)
- **D-16: Tre casi distinti per «non duplicare» (criterio 4).**
  - **A. Retry della stessa richiesta:** `Idempotency-Key` obbligatoria su `POST /api/v1/documents`. Stessa chiave e stesso payload → stessa risposta. Chiave mancante → 400; stessa chiave con payload diverso → 422; richiesta ancora in corso → 409.
  - **B. Stesso uploader, stesso hash di un suo documento attivo, chiave nuova:** nessun nuovo documento né chunk, risposta con `document_id` e job esistenti. Se livello o ACL richiesti sono diversi: 409 con il `document_id`. «Attivo» esclude i documenti cancellati, `failed` e `quarantined`.
  - **C. Uploader diverso, stesso hash:** due documenti e un solo blob. Nessuna risposta rivela che il contenuto esiste già.
  - Job ed eventi usano chiavi derivate, che danno `AlreadyApplied` alla ripetizione.
  - Fonte: draft-ietf-httpapi-idempotency-key-header-08; T-11 e ADR-0003; 02 D-05; Area E. Decisione utente sul caso B, che il brief non copriva.
- **D-17: Dopo `failed` si ricarica.** Nessun pulsante «riprova»: parte un documento nuovo. Riga e diagnostica restano; il blob non più referenziato va al GC degli orfani. Non si conservano file falliti o quarantenati.
  - Fonte: ADR-0003 (GC, N = 24 h); ADR-0013 (nessuna revisione manuale dei file). Decisione utente.

### Audit (Area F)
- **D-18: Outbox e relay.** L'evento si scrive nella stessa transazione del dato, in una tabella `outbox` dello schema `document`; un relay separato lo consegna ad `AuditSink`, con `event_id` dalla chiave di idempotenza. Nessun evento perso, con un breve ritardo. L'outbox è riusabile dalle fasi 11 e 12.
  - Fonte: `interfaces-and-concurrency.md`; microservices.io Transactional outbox; 04 D-14; T-12; 02 D-10; Area F. Decisione utente.
- **D-19: Un evento a ogni transizione di stato**, mai per chunk. Eventi minimi: `upload_accepted`, `upload_rejected`, `acl_set`, `ingestion_stage`, `version_published`, `ingestion_failed`, `document_quarantined`, `document_deleted`, più gli eventi dell'arricchimento (D-21). Il relay serializza gli `append`, quindi chi carica non contende la riga di testa della catena hash (04 D-16).
  - Fonte: criterio 5 del ROADMAP; FR-009; Area F. Decisione utente.

### Docling e TTQ (Area G)
- **D-20: Il target TTQ di 15 s vale per il primo `queryable`.** La pubblicazione iniziale contiene testo PDFium e OCR, senza Docling. Stima, **somma di misure separate e non misura unica**: circa 5 s per il PDF digitale da 25 pagine `der-it-01` (prescan + PDFium 1,1 s, OCR 2,5 s, embedding circa 1 s, più la pubblicazione).
  - Fonte: `README.md:82`; `spikes/s02-parser/RISULTATI-ocr.md:178`; ADR-0007. Conseguenza di D-21.
- **D-21: Docling in background, sulla stessa versione di documento.** Idea dell'utente, che chiude l'opzione «Docling asincrono dopo il primo indice» lasciata aperta da ADR-0004 e D-36 di fase 1.
  - Un **job di arricchimento separato**, solo per i PDF con pagine-tabella selezionate, non tocca lo stato `queryable`. Se fallisce, il documento resta interrogabile con le tabelle come testo.
  - **Seconda pubblicazione atomica** sulla stessa `DocumentVersionId`, con una **generazione di indice interna**: il numero di versione e i byte non cambiano, così il criterio 4 resta rispettato.
  - I chunk della generazione precedente si **conservano e non si cercano più**: una citazione già data continua ad aprire la stessa pagina e gli stessi rettangoli.
  - Il job si **scarta** se nel frattempo il documento è stato cancellato o ha una versione più recente.
  - Un **interruttore di configurazione** spegne l'arricchimento.
  - La UI mostra lo stato separato («tabelle in elaborazione»).
  - Fonte: ADR-0004 §Conseguenze; ADR-0005 (locator legato alla versione); 01 D-36; `spikes/s02-parser/RISULTATI-ocr.md:178-182` (Docling 15,3 s su 13 pagine); `spikes/s02-parser/RISULTATI-estrazione.md` e ADR-0004 (F1 celle 0,580 contro 0,011 della baseline); Area G. Decisione utente.
  - **Rischio residuo dichiarato:** fra le due pubblicazioni una domanda su un numero in tabella legge testo non strutturato, e può dare una risposta peggiore anche se citata. La finestra è di decine di secondi, più lunga con la coda piena. La fase 13 valuta i documenti solo a generazione finale; la fase 14 misura la durata reale.

### Perimetro (Aree H, I, J)
- **D-22: Tombstone in fase 6.** `DELETE /api/v1/documents/{id}` fa la cancellazione logica, ritira il documento dall'indice (`VectorIndex::withdraw`), cancella il job in corso senza pubblicare, e manda il blob al GC. Autorizzazione come D-04.
  - Fonte: `interfaces-and-concurrency.md:35` e race «Cancellazione durante ingestion»; fase 9, «documento rimosso → deny»; 03 Deferred; Area H. Decisione utente.
- **D-23: Solo il modello per versione, senza endpoint.** Tabelle e pipeline lavorano per versione, provate con un test interno che pubblica una v2. Endpoint e UI restano alla fase 12.
  - Fonte: criterio 3 del ROADMAP; DOC-03 alla fase 12; Area J. Decisione utente.
- **D-24: Avanzamento con polling ogni 1-2 s** su `GET /api/v1/ingestion-jobs/{id}` con `refetchInterval` di TanStack Query, fino allo stato terminale. Scartato SSE: `EventSource` non invia l'header `Authorization`, e l'access token vive solo in memoria (05 D-06).
  - Fonte: TanStack Query, guida polling; `README.md` (job fallito visibile entro 5 s); Area I. Decisione utente.
- **D-25: UI e rotte.** Pagina di upload (file, livello obbligatorio, utenti nominati per `restricted`), stato del job, elenco «i miei caricamenti» (admin: tutti) con modifica dell'ACL e cancellazione. Rotte nuove, assenti da `interfaces-and-concurrency.md`: `PATCH /api/v1/documents/{id}/acl` e una rotta per «i miei caricamenti», distinta da `GET /api/v1/documents`, che resta della fase 7.
  - Fonte: criterio 5 del ROADMAP; D-04; Area I. Decisione utente.

### Vincoli dalla WIRING-MAP e dalle fasi precedenti
WIRING-MAP: zero righe al 2026-11-05, quindi nessun `WM-NNN` da citare ancora. Seam che la fase **consuma**, promessi nei CONTEXT e da registrare in `wiring-design` (plan della fase 2):
- **Riga del route registry** di `apps/web` per la feature upload (02 D-23).
- **Fascia di migrazioni** `06NNNN_*.sql` nello schema `document` (02 D-09).
- **Slot `AuditSink`** nel composition root del document-service, con la tabella append-only della fascia `04` (04 D-14).
- **Hook di autenticazione** del document-service, riempito dalla fase 5.
- **`IdempotencyKey`** in `crates/contracts`: file vuoto per assunzione provvisoria di 02 D-22, lo decide `wiring-design`.

Altri vincoli già decisi:
- **Primitive proprie:** idempotency key, outbox e worker con lease nascono qui (02 D-10).
- **`Principal` e ruoli:** insieme piatto, senza gerarchia (05 D-16).
- **Provider:** `put`, `get`, `delete`, `parse`, `chunk`, `embed_documents`, `publish_version`, `set_acl` solo tramite le porte (03 §consumatori).
- **Parser su file locale:** il worker scarica l'originale in un tempfile e chiama `parse(&LocalFile)` (03 D-04).
- **Lettore limitato unico** per ogni dato non fidato (03 D-19).
- **GC degli orfani:** job, tabella `pending`, protocollo `live|deleting`, N = 24 h, pulizia dei tempfile all'avvio (03 D-03, ADR-0003).
- **Concorrenza del worker da 2 GiB:** 2 `parse`, 2 `ocr_tesseract`, 1 `ocr_render` (ADR-0004). Docling ricreato per documento e dopo ogni timeout, disconnessione o errore.
- **Limite su tre livelli** e admission control dell'upload (ADR-0003). Va provata la saturazione del semaforo di validazione, mai saturato in 01-04 TEST-4 (`01-VERIFICATION.md`).
- **Errori problem+json** (02 D-05).

### Claude's Discretion
- **Idempotenza:** fingerprint (sha256 del contenuto più metadati) e TTL della chiave, 24 h dal brief, da pubblicare come dice il draft.
- **Retry e lease:** curva del backoff, durata di lease e heartbeat, tetto dei tentativi per un lease scaduto. Deve essere finito (D-14).
- **Tetti (D-15):** valori numerici, a partire dalla stima di smaltimento.
- **Job non visibile:** 404 o 403, con la regola di non rivelarne l'esistenza.
- **Frontend:** intervallo esatto di polling, forma delle pagine, nome e forma della rotta «i miei caricamenti».
- **Generazione dell'indice (D-21):** forma tecnica nello schema della fase 3, ed embedding solo dei chunk cambiati oppure di tutti (il costo è circa 1 s).

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/06-upload-e-indicizzazione/06-RICERCA-DISCUSSIONE.md` — brief Aree A-J con fonti; §0 elenca i punti ereditati; Area G aggiornata il 2026-11-05.

### Decisioni di progetto
- `docs/adr/0003-storage-originali-garage.md` — storage, chiave sha256, limite su tre livelli, admission control, GC con N = 24 h.
- `docs/adr/0004-parser-ocr-isolamento.md` — parser, OCR, isolamento, esiti tipizzati, concorrenza del worker, costo di Docling.
- `docs/adr/0005-locator-citazione.md` — locator per PDF e testo, legame alla versione.
- `docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md`, `docs/adr/0007-embedding-tei-gpu.md` — indice e embedding.
- `docs/adr/0010-deadline-cancellazione-http.md` — deadline e cancellazione.
- `docs/adr/0012-firme-porte-candidate.md` — firme delle porte, da estendere con D-21 e D-22.
- `docs/adr/0013-rinvio-antivirus-cdr.md` — AV e CDR rinviati.
- `DEPENDENCIES.md` — stack deciso, «Da evitare».
- `.planning/phases/01-spike-tecnici/01-CONTEXT.md` — D-36 (punto aperto Docling).
- `.planning/phases/02-fondazione/02-CONTEXT.md` — D-05, D-06, D-07, D-09, D-10, D-20, D-22, D-23.
- `.planning/phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md` — D-03, D-04, D-06, D-07, D-08, D-09, D-10, D-12, D-19.
- `.planning/phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md` — D-14..D-19 (AuditSink).
- `.planning/phases/05-identita-e-accesso/05-CONTEXT.md` — D-04, D-06, D-16.

### Architettura e requisiti
- `docs/architecture/interfaces-and-concurrency.md` — §Document & Indexing, §Upload e ingestion, race condition; da estendere.
- `docs/architecture/architecture.md` — flusso di ingestion, ownership dei dati.
- `docs/data/corpus-strategy.md` §88-89 — upload manuale contro scoperta online.
- `docs/quality/test-plan.md` — matrice ruolo × classificazione.
- `.planning/REQUIREMENTS.md` — DOC-01, DOC-02, PLAT-05.
- `README.md` — target TTQ, «Limiti noti».

### Sicurezza
- `docs/security/threat-model.md` — T-05..T-12, T-39.
- `skills/security-scanner/SKILL.md` — regole su upload, path traversal, no-leak.

### Misure
- `spikes/s01-upload/RISULTATI.md` — streaming, deduplica, admission.
- `spikes/s02-parser/RISULTATI-ocr.md` §TTQ-parse; `spikes/s02-parser/RISULTATI-estrazione.md` §Euristica pagine-tabella.

### Standard esterni
- OWASP File Upload Cheat Sheet; draft-ietf-httpapi-idempotency-key-header-08; PostgreSQL SELECT (SKIP LOCKED); pattern Transactional outbox; TanStack Query, polling (URL nel brief).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: `services/document-service/` e `apps/web/` sono vuoti. Gli spike non si importano (01-CONTRATTO §6); forme e test si trascrivono.
- `spikes/s01-upload` — upload in streaming, validazione, admission, GC con protocollo `live|deleting`, casi ostili in `results/correctness.csv`.
- `spikes/s02-parser` — lanciatore del figlio isolato e prescan, adattati dalla fase 3.
- `spikes/s07-ports/src/lib.rs` — `IndexVersionDraft`, `publish_version`, `MutationOutcome::AlreadyApplied`, test di retry dopo `OutcomeUnknown`.

### Established Patterns
- Fasce di migrazione per fase e servizio, errori problem+json, route registry esplicito, adapter dietro porte con fake deterministici (fasi 2-4).
- `CallContext` con deadline (ADR-0010) su ogni chiamata esterna; nessuna chiamata esterna dentro una transazione.

### Integration Points
- Porte della fase 3 ← pipeline del worker; `AuditSink` del document-service (fase 4) ← relay dell'outbox.
- `Principal` della fase 5 → permessi di D-03, D-04, D-05.
- Fase 03.1: scrive anch'essa dentro la transazione di `publish_version`.
- Fase 7: legge le etichette di D-01 per il predicato di retrieval.
- Fase 9: documento rimosso → deny (D-22); citazioni su generazioni precedenti (D-21).
- Fase 10: matrice ruolo × livello (D-01..D-03).
- Fase 12: riusa la pipeline per versione, il job e l'outbox.
- Fasi 13 e 14: valutazione a generazione finale, due tempi di TTQ (D-21).

</code_context>

<specifics>
## Specific Ideas

- L'arricchimento in background è un'idea dell'utente, emersa dalla domanda «non si può fare docling in background dopo aver caricato il PDF?».
- L'utente vuole capire i meccanismi: il racconto didattico delle slice dovrebbe spiegare la poison pill e il conteggio al claim, i tre casi di deduplica, l'outbox e la generazione di indice.
- I casi «worker morto a metà», «doppio claim», «cancellazione durante l'ingestion» e «arricchimento superato da una cancellazione» devono essere test distinti e leggibili.

</specifics>

<deferred>
## Deferred Ideas

- Pulsante «riprova» su un job fallito: non adottato (D-17).
- Rilascio admin di un PDF in quarantena: non adottato (D-06).
- Claim a turno fra uploader per l'equità: non adottato, D-15 basta a questa scala.
- SSE per l'avanzamento: non adottato (D-24).
- Euristica pagine-tabella più precisa (precisione 0,435 sull'holdout, 01 D-36): riduce il lavoro di Docling, fuori dalla fase 6; da rivalutare in fase 14 sui numeri reali.

</deferred>

## TODO aperti dal discuss

- **ROADMAP via `gsd-tools`:** criterio 2 secondo D-07. Criterio 5: «pagina» vale solo per i PDF, DOCX e Markdown hanno il locator `Text` (03 D-10, ADR-0005). Tracciabilità: `withdraw` → 6.
- **Riconciliazione del piano della fase 3 (costo noto):** `publish_version` esteso con la generazione interna (D-21) e `VectorIndex::withdraw` (D-22). Oggi la porta pubblica un insieme di chunk per versione. Emendare ADR-0012 e 03 D-09 con *Storia*.
- **Coordinamento con la fase 03.1:** anche il grafo di conoscenza vuole scrivere nella transazione di `publish_version` (`03.1-RICERCA-DISCUSSIONE.md:99,103`). Il `plan 3` deve definire la porta tenendo conto di entrambe, compreso cosa succede agli archi quando l'arricchimento pubblica una nuova generazione.
- **ADR di ingestion in `plan`:** D-21 chiude il punto aperto di 01 D-36 ed è costosa da cambiare. Serve un ADR nuovo da `0000-template.md`, con emenda di ADR-0004 §Conseguenze.
- **Contratti:** estendere `interfaces-and-concurrency.md` con `PATCH .../acl`, «i miei caricamenti», stati del job, 429 e 503, race dell'arricchimento.
- **README e fase 14:** chiarire che il target vale per il primo `queryable`; DOC-04 misura due tempi, TTQ del testo e TTQ delle tabelle.
- **Fase 13:** valutazione a generazione finale, da portare nel suo discuss.
- **Threat model:** aggiungere il rischio residuo della finestra con tabelle piatte (D-21).
- **Assunzione provvisoria:** la stima di 5 s e i tempi di D-21 sono somme di misure separate, da misurare in fase 14.
- **Fonte da verificare:** il limite di `EventSource` (D-24) viene dal brief, senza URL. In `plan`, citare la specifica HTML.
- **Pipeline:** i seam consumati diventano righe `WM-NNN` solo con il `plan` della fase 2. Riallineare i vincoli sopra a quel punto.

**Modifica:** Claude (Anthropic) · 2026-11-05 · CONTEXT della fase 6 dal discuss (decisioni utente 2026-10-26 e 2026-11-05)
