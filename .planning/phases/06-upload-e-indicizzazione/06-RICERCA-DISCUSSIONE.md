# 06 — Ricerca decisionale per il discuss (Upload e indicizzazione)

**Modifica:** Claude (Anthropic) · 2026-10-26 · brief di ricerca del discuss della fase 6 (canale Claude, nessuna ricerca GPT richiesta)

Stile caveman full. Fonti esterne con URL + accesso 2026-10-26. Fonti repo con file:sezione: prevalgono su esterno finché utente non riapre.

## 0. Preflight

- Goal ROADMAP: utente autorizzato carica doc con ACL esplicita → queryable; versione pubblicata atomica; chunk con pagina/posizione; provider fase 3 solo via porte. Req DOC-01, DOC-02, PLAT-05. Dipende 3, 5.
- Decisioni da chiudere (ROADMAP §Decisioni aperte): Q-01 permessi per ruolo (vincola 7, 10), Q-02 classificazione (vincola 3, 7, 10), Q-06 quarantena upload manuale.
- Criticità: ROADMAP già «critica (ingestion: upload, orchestrazione parsing file non fidati)». Regola skill: goal tocca ingestion → critica. Plan: GPT `implementation-risk-audit` + `test-design`.
- Pipeline sfalsata: fase 1 verify gaps_found (1.6 da pianificare), fasi 2-5 discusse non pianificate. WIRING-MAP zero righe: seam Fondazione (fascia 06, route registry, slot AuditSink document, hook auth) promessi solo nei CONTEXT 02/04/05.
- deferred-items.md: nessuno. Punti ereditati veri:
  - P1 · 03 D-03: job GC orfani + tabella `pending` + protocollo `live|deleting` → fase 6 (consumatore `DocumentStore::delete`). ADR-0003: N = 24 h; pulizia tempfile all'avvio.
  - P2 · 03 D-04: worker scarica originale in tempfile locale, poi `parse(&LocalFile)`.
  - P3 · 03 D-06/D-07: Q-02 deve produrre **insiemi di etichette**; `set_acl` all'upload (fase 6 consumatore).
  - P4 · 03 Deferred: tombstone `DELETE /api/v1/documents/{id}` → assegnare in discuss 6 o 12; se preso → `VectorIndex::withdraw` in riconciliazione.
  - P5 · 02 D-10: idempotency key, outbox, worker lease + SKIP LOCKED → prima fase che li usa = 6 nel document-service.
  - P6 · 02 TODO: `IdempotencyKey` file vuoto contracts → fase 6 (assunzione provvisoria), decide wiring-design.
  - P7 · 05 §Downstream: fase 6 consuma `Principal` e ruoli (insieme piatto, nessuna gerarchia, 05 D-16).
  - P8 · 01-VERIFICATION deferred: 01-04 TEST-4 semaforo validazioni mai saturato → fase 6 (admission control prodotto).
  - P9 · threat model: T-05..T-08, T-10, T-11, T-12 (audit upload), T-39 (AV/CDR rinviato a 14, ADR-0013).
  - P10 · 04 D-14..D-17: AuditSink PG append-only schema `document`, `append` fuori dalla tx del chiamante, `event_id` da idempotency key, catena hash.
- **Incoerenza ROADMAP:** criterio 2 «oltre 10 MB» vs 01 D-04 + ADR-0003 + T-05 «limite rigido 25 MiB; target TTQ definito su 10 MB». Criterio 5 «pagina» vale solo PDF: DOCX/MD hanno locator `Text` (sezione + offset, 03 D-10, ADR-0005).

## Area A — Q-06 quarantena upload manuale + validazione

Fonte autorevole:
- docs/data/corpus-strategy.md:88-89: «Upload manuale: pubblicazione dopo validazione e ACL esplicita. Scoperta online: quarantena obbligatoria.» architecture.md §Flusso ingestion: `upload/discovery -> quarantine -> deterministic validation -> ...` (stesso flusso per entrambi).
- intel/context.md Q-06: compatibili se quarantena per upload = solo stato transitorio di validazione.
- ADR-0003 §Conseguenze + 01 D-03/D-05: validazione completa (magic, struttura DOCX, zip, DTD) **prima** di storage e DB; PDF attivi (`/JavaScript`, `/Launch`) → quarantena; cifrati con password utente vuota ammessi, altrimenti `Encrypted`.
- ADR-0004: esiti tipizzati parser `Encrypted, DecompressionLimit, Timeout, ResourceLimit, OutputLimit, Crashed, Corrupt, Unsupported`, stato `Quarantined`.
- OWASP File Upload CS (https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html, 2026-10-26): «Only allow authorized users to upload files»; allowlist estensioni + tipo reale (Content-Type «can be spoofed»); nome file generato dall'app; AV/sandbox «if available»; revisione manuale in sandbox «before releasing the files to the public» — con risorse.
- ADR-0013: AV/CDR rinviati a fase 14 (T-39).
Pratica consolidata:
- Upload interno da utente autenticato e autorizzato: niente approvazione umana; stato di job `validating` = quarantena logica (nessun chunk visibile finché non pubblicato). Approvazione umana solo per contenuto esterno non fidato (fase 12, BR-001).
- PDF attivo: terminale non queryable `quarantined` con motivo, visibile a uploader/admin; non serve al viewer (T-10).
Opzioni:
- A1 quarantena transitoria = stati `received → validating`, nessuna approvazione; PDF attivi → `quarantined` terminale. **Raccomandata** (allinea corpus-strategy, flusso unico architecture, fase 12 resta unica con approvazione).
- A2 quarantena con approvazione admin anche per upload: raddoppia la UI di revisione della fase 12, contro corpus-strategy.
Limite dimensione: ROADMAP crit. 2 → riscrivere «oltre 25 MiB» (gsd-tools), TTQ resta su 10 MB.

## Area B — Q-02 classificazione → etichette

Fonte autorevole:
- 03 D-06: `AuthorizationPredicate { subject_id, acl_version, labels }`; visibile se etichette doc ∩ etichette soggetto ≠ ∅; vuoto = zero. Mappa classificazione → etichette la applica chi scrive (6, 12); cambio matrice ricalcola etichette + incrementa versioni.
- test-plan.md: «matrice ruolo × classificazione documento» + test negativi di leakage (fase 10).
- Microsoft Purview sensitivity labels (https://learn.microsoft.com/en-us/purview/sensitivity-labels, 2026-10-26): esempi «Personal, Public, General, Confidential, and Highly Confidential»; un solo label per item; ordine = priorità; «effectiveness is noticeably reduced when users have more than five main labels»; label può portare permessi per utenti/gruppi.
- Azure AI Search security trimming (già in 03 D-06): gruppi come stringhe nel filtro.
Pratica consolidata:
- Livelli ordinati pochi (3-5) + mapping livello → ruoli ammessi = etichette `role:*` calcolate al write. Grant puntuali opzionali `user:<uuid>`.
- Risorse private di default, creatore controlla i grant: Open WebUI (https://docs.openwebui.com/features/authentication-access/rbac/groups, 2026-10-26).
Proposta concreta (4 livelli, insieme piatto di ruoli 05 D-16):
| livello | etichette |
|---|---|
| `public` | role:guest, role:employee, role:manager, role:admin |
| `internal` | role:employee, role:manager, role:admin |
| `confidential` | role:manager, role:admin |
| `restricted` | role:admin + `user:<uuid>` esplicite |
Più `user:<uploader>` sempre. ACL esplicita = scelta obbligatoria del livello (nessun default) + eventuali utenti aggiuntivi.
Varianti: B1 solo livello (matrice pulita per fase 10); B2 livello + grant utente (più realistico, matrice con una dimensione in più); B3 solo ACL libera per ruoli (niente classificazione: contro test-plan).

## Area C — Q-01 permessi per ruolo

Fonte autorevole:
- FR-002 (requirements.md): solo ruoli, nessun permesso descritto. 05 D-16: ruoli insieme piatto, solo admin cambia ruoli.
- OWASP File Upload CS: solo utenti autorizzati caricano.
- Riferimenti prodotto: Onyx — gestione connettori/document set è permesso di gruppo o admin; Curator = admin di gruppo (https://docs.onyx.app/admins/permissions/understanding_permissions, 2026-10-26). Open WebUI — creatore + grant.
Pratica: principio del minimo privilegio; guest sola lettura.
Proposta matrice:
| azione | admin | manager | employee | guest |
|---|---|---|---|---|
| upload | sì | sì | sì | no |
| livello assegnabile | tutti | ≤ confidential | ≤ internal | — |
| vedere stato job | tutti | propri | propri | — |
| cambiare ACL/livello di un doc | tutti | propri, ≤ confidential | no | no |
| tombstone (se fase 6) | sì | propri | no | no |
Regola chiave: non si assegna un livello che non si potrebbe leggere (no write-up oltre la propria visibilità, analogo «no read up» Bell-LaPadula). Consultazione audit = Q-03, fase 11.

## Area D — Job, coda, worker

Fonte autorevole:
- interfaces-and-concurrency.md §Upload e ingestion: job restituito dopo persistenza sicura originale + outbox; worker lease + `FOR UPDATE SKIP LOCKED`; pool distinti parsing/OCR/embedding/DB; lock logico `(source, external_id, version)`; checksum + unique per retry.
- DEPENDENCIES §Da evitare: «Message broker dedicato senza requisito misurato».
- PostgreSQL SELECT (https://www.postgresql.org/docs/current/sql-select.html, 2026-10-26): SKIP LOCKED «not suitable for general purpose work, but can be used to avoid lock contention with multiple consumers accessing a queue-like table».
- ADR-0004: worker 2 GiB, 2 job `parse`, 2 `ocr_tesseract`, 1 `ocr_render` concorrenti.
Pratica consolidata (River, coda PG):
- default 25 tentativi, backoff `attempts^4` s ±10% (https://riverqueue.com/docs/job-retries, 2026-10-26);
- rescuer dei job bloccati in running: default 1 h (https://riverqueue.com/docs/maintenance-services, 2026-10-26).
- Lease breve + heartbeat = variante più reattiva (awa: «heartbeat-based crash recovery», https://docs.rs/crate/awa/0.6.5, 2026-10-26).
Crate Rust: graphile_worker, awa, pgqrs, forge-jobs (https://docs.rs/graphile_worker, https://docs.rs/crate/pgqrs/0.4.0, 2026-10-26): ecosistema frammentato, maturità/manutenzione non verificate. Precedente progetto: breaker scritto a mano (04 D-07), preferenza toolchain semplice.
Raccomandazione: tabella `ingestion_jobs` = coda (insert nella stessa tx dei metadati); claim `FOR UPDATE SKIP LOCKED` + `lease_until` + heartbeat; errori permanenti (Corrupt, Encrypted, Unsupported, DecompressionLimit, Quarantined) → terminale subito; transitori (TEI, Garage, DB) → retry con backoff, 5 tentativi; scritto a mano.

## Area E — Idempotenza e deduplica

Fonte autorevole:
- IETF draft-ietf-httpapi-idempotency-key-header-08 (22-10-2026, https://datatracker.ietf.org/doc/draft-ietf-httpapi-idempotency-key-header/, 2026-10-26): chiave mancante dove richiesta → 400; riuso con payload diverso → 422; richiesta concorrente in corso → 409; fingerprint MAY; scadenza MAY, da pubblicare.
- 02 D-05: errori problem+json RFC 9457 (coerente col draft).
- T-11 + ADR-0003: deduplica fisica invisibile, risposta stessa forma, `put` sempre.
- PLAT-05 / crit. 4: retry di upload, job, eventi non duplica contenuto fisico, chunk, versioni.
Ragionamento: deduplica **logica** fra documenti di utenti diversi rivelerebbe l'esistenza di un doc altrui (canale laterale T-11) → solo fisica. Dentro lo stesso documento: nuova versione con sha256 = versione corrente → nessuna versione nuova.
Raccomandazione: `Idempotency-Key` obbligatoria su `POST /api/v1/documents` (client genera UUID per tentativo di upload); fingerprint = sha256 contenuto + metadati; TTL 24 h (= N GC); job e publish con chiave derivata (job_id, version) → `AlreadyApplied`.

## Area F — Audit della fase 6

Fonte autorevole:
- 04 D-14: `append` non condivide la tx del chiamante (consistenza eventuale), `event_id` da idempotency key.
- microservices.io Transactional outbox (https://microservices.io/patterns/data/transactional-outbox.html, 2026-10-26): messaggio salvato nella stessa tx del dato, relay separato; at-least-once → consumer idempotenti. Problema risolto: dual write.
- FR-009: upload, ACL, ingestion tracciati. T-12 ripudio.
Opzioni:
- F1 append diretto dopo commit, retry sulla stessa chiave: finestra di perdita se il processo muore fra commit e append.
- F2 outbox nella stessa tx (tabella `outbox` schema document) + relay → AuditSink: zero perdite; è l'outbox promessa dall'interfaces doc, riusabile da 11 (Q-04) e 12. **Raccomandata.**
Eventi minimi: `upload_accepted`, `upload_rejected`, `acl_set`, `ingestion_stage` (transizioni di stato, non per chunk), `version_published`, `ingestion_failed`, `document_quarantined` (+ `document_deleted` se tombstone in 6).

## Area G — Docling nella pipeline e TTQ

Fonte autorevole:
- README TTQ p95: PDF digitale 25 pp ≤ 15 s; scannerizzato 25 pp ≤ 45 s; job fallito visibile ≤ 5 s.
- ADR-0004 Conseguenze: Docling sulle 13 pagine selezionate ≈ 15 s da solo; riavvio per documento ≈ 19 s GPU; percorso composto fuori quota. D-36 fase 1: punto aperto per «ADR di ingestion»; opzioni D-27 (meno pagine, Docling asincrono dopo primo indice, euristica più precisa).
- 03 D-12: adapter Docling in fase 3 alle condizioni degli spike. Fase 14 misura TTQ (DOC-04).
Opzioni:
- G1 Docling sincrono: una sola versione, TTQ 25 pp ≈ 35-40 s oltre target, dichiarato, fase 14 rimisura.
- G2 due passate: pubblica subito testo PDFium, poi arricchimento tabelle → nuova versione indice: TTQ rispettato, complessità (doppio embedding, citazioni che cambiano chunk).
- G3 Docling spento di default (flag): TTQ rispettato, tabelle solo come testo PDFium.
Raccomandazione: G1 dietro flag di configurazione (G3 = interruttore), decisione definitiva in fase 14 sui numeri reali.

## Area H — Tombstone, cancellazione, GC

Fonte autorevole:
- interfaces-and-concurrency.md: `DELETE /api/v1/documents/{id}` tombstone; race «Cancellazione durante ingestion → job cancellato e versione non pubblicata»; fase 9 crit. 3 «documento rimosso dopo la risposta → deny».
- 03 Deferred: tombstone senza fase; se preso → `VectorIndex::withdraw` (fase 3, riconciliazione).
- ADR-0003: GC orfani N = 24 h, protocollo `live|deleting` + `pending`; fase 6 lo costruisce (03 D-03).
Ragionamento: fase 9 presuppone un documento rimovibile; fase 12 riguarda versioni e fonti online.
Opzioni: H1 tombstone in fase 6 (DELETE logico + withdraw + cancellazione job in corso + blob al GC); H2 in fase 12. Raccomandazione: H1.

## Area I — Progresso del job e perimetro frontend

Fonte:
- README: stato di job fallito visibile ≤ 5 s.
- TanStack Query polling (https://tanstack.com/query/v5/docs/framework/react/guides/polling, 2026-10-26): `refetchInterval` funzione della query, `false` ferma; esempio ufficiale «poll until done» a 2 s. TanStack Query nello stack (02 D-23).
- SSE: connessione lunga attraverso Caddy, riconnessione, EventSource senza header Authorization nativo → complessità.
Raccomandazione: polling 1-2 s su `GET /api/v1/ingestion-jobs/{id}` fino a stato terminale.
Perimetro UI: pagina upload (file + livello + utenti aggiuntivi), stato job, elenco «i miei caricamenti». `GET /api/v1/documents` filtrato per visibilità richiede `AccessPolicy` → fase 7.

## Area J — Nuova versione di un documento esistente

Fonte: crit. 3 fase 6 «pipeline per versione»; fase 12 crit. 1 «Caricando una nuova versione …» (DOC-03 fase 12); lock logico `(source, external_id, version)`.
Opzioni: J1 fase 6 espone anche `POST /api/v1/documents/{id}/versions`; J2 fase 6 costruisce il modello per versione + test interno, endpoint alla fase 12. Raccomandazione: J2 (perimetro stretto, DOC-03 della 12).
