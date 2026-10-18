# 01 — Ricerca decisionale (discuss fase 1: Spike tecnici)

Brief caveman per area. Ricerca: Claude (Anthropic), canale di default (WebSearch/WebFetch/context7), accesso fonti 2026-10-18. Voci [NV] = non verificate, da misurare nello spike.
Hardware postazione: i7-13700K 16C/24T, 32 GB RAM, RTX 3050 6 GB VRAM, Windows 11.

- Area A — Upload, storage originali, threat model (S-01, A-02, PLAT-07)
- Area B — Parser/OCR, isolamento, pagina DOCX/MD (S-02, A-05, A-10)
- Area C — pgvector, embedding (S-03, A-03)
- Area D — JEV, cancellazione, LLM, reranker (S-04/A-09, S-05, A-04, A-06)


---

## Ricerca 01 — Upload streaming (S-01), storage originali (A-02), threat model (PLAT-07)

Progetto CKA. Data accesso fonti: **2026-10-18** (tutte). Stile: caveman. Nessun file repo toccato.
Input letti: README.md, DEPENDENCIES.md, docs/architecture/interfaces-and-concurrency.md, docs/security/threat-model.md (solo stub), docs/data/corpus-strategy.md, .planning/REQUIREMENTS.md (ID requisiti).

Vincoli dal repo che pesano:
- "Upload in streaming; mai l'intero file in memoria." / "Job restituito dopo persistenza sicura dell'originale e dell'outbox." / "Transazioni brevi; nessuna chiamata esterna dentro una transazione." / "Outbox nella stessa transazione del cambiamento di dominio."
- Ack p95 ≤500 ms; file target ≤10 MB; corpus 10–30 doc.
- ROADMAP Fase 1 criterio S-01: "memoria di picco costante rispetto alla dimensione del file", misurata con 10 MB e oltre.

---

## 0. Versioni crate (crates.io API, max_stable_version, 2026-10-18)

| Crate | Versione | Ultimo update |
|---|---|---|
| axum | 0.8.9 | 2026-04-14 |
| multer | 3.1.0 | 2024-05-04 |
| tower-http | 0.7.1 | 2026-08-31 |
| sqlx | 0.9.0 | 2026-05-21 |
| aws-sdk-s3 | 1.152.0 | 2026-10-01 |
| object_store | 0.14.2 | 2026-09-15 |
| rust-s3 | 0.38.0 | 2026-10-04 |
| infer | 0.22.0 | 2026-07-15 |
| sha2 | 0.11.1 | 2026-10-12 |
| dhat | 0.3.3 | 2024-02-04 |

Fonte: https://crates.io/api/v1/crates/{name} (2026-10-18). Pin esatto → lockfile.

---

## 1. Axum streaming multipart (S-01)

### Finding
- F1.1 `axum::extract::Multipart` (axum 0.8.9): `next_field()` → `Field`; "limits the request body size to 2MB" di default; modificabile con `DefaultBodyLimit`. Deve essere ultimo extractor (consuma body). Dipendenza opzionale `multer ^3.0.0`. — https://docs.rs/axum/latest/axum/extract/struct.Multipart.html
- F1.2 `Field::chunk(&mut self)` → `Result<Option<Bytes>>` streaming chunk-by-chunk; `Field` impl `Stream<Item=Result<Bytes, MultipartError>>`. `bytes()`/`text()` bufferizzano TUTTO → vietati per file. `file_name()`/`content_type()` = valori client, nessuna sanitizzazione documentata. — https://docs.rs/axum/latest/axum/extract/multipart/struct.Field.html
- F1.3 `DefaultBodyLimit`: default 2MB per extractor basati su Bytes; `disable()`, `max(n)`; è **locale** (solo extractor che lo applicano). `RequestBodyLimitLayer` (tower-http) è **globale**. Doc: con client non fidati aggiungere limite proprio (`tower_http::limit`). Se extractor legge body via `Body::poll_frame` il default NON si applica. — https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
- F1.4 `multer` 3.1.0: `Constraints::new().allowed_fields(..).size_limit(SizeLimit::new().whole_stream(..).per_field(..).for_field(name, n))`; default "does not apply any constraint". Utile se serve whitelist campi/limite per campo (axum `Multipart` non espone Constraints). — https://docs.rs/multer/latest/multer/struct.Constraints.html
- F1.5 Hashing streaming: `sha2::Sha256::update(&chunk)` per chunk, `finalize()` a fine stream; costo O(1) memoria. (crate sha2 0.11.1, fonte crates.io sopra.)
- F1.6 `infer` 0.22.0: detection via magic number su buffer (`get`, `is_document`, `get_from_path`); categorie `doc`, `archive`, `text`... Doc pagina non conferma byte necessari né DOCX/PDF esplicitamente → verificare in spike. Markdown: nessuna magic (è testo). — https://docs.rs/infer/latest/infer/
- F1.7 Peak memory: `dhat` 0.3.3, `Profiler::builder().testing().build()` + `HeapStats::get().max_bytes` assertabile in test; un test per file in `tests/` o `--test-threads=1`; overhead alto, "much worse on Windows"; crate "experimental". — https://docs.rs/dhat/latest/dhat/

### Pipeline upload proposta (default)
```
reverse proxy (body max ~10.5 MiB) 
 → RequestBodyLimitLayer(10 MiB + overhead multipart, es. 10.5 MiB) + RequestBodyTimeout/Timeout (anti slowloris)
 → route POST /api/v1/documents con DefaultBodyLimit::max(stesso valore) (solo questa route)
 → authz (upload permesso) PRIMA di leggere il body
 → Content-Length precheck → 413 early
 → Multipart: 1 solo campo file ammesso (+ eventuale campo metadata JSON piccolo, limite per campo)
 → loop field.chunk(): contatore byte (abort >10 MiB → 413), Sha256.update, primi 8 KiB in buffer per sniff, write su tempfile (tokio::fs, dir dedicata, nome UUID)
 → fine stream: sniff magic + validazione struttura (vedi sotto) → reject 415/422
 → persist originale (A-02) + tx {document, version, job, outbox} → 202 {document_id, job_id}
```
- Validazione tipo (ASVS 5.2.2 L1): allowlist estensione **e** magic **e** content-type dichiarato coerenti.
  - PDF: header `%PDF-` (cercare nei primi 1024 B).
  - DOCX: ZIP `PK\x03\x04` + central directory con `[Content_Types].xml` e `word/document.xml`; leggere solo directory con limiti (n. entry max, somma uncompressed max, ratio) → anti zip-bomb (ASVS 5.2.3, L2 ma economico). NO macro: rifiutare `.docm` / `vbaProject.bin`.
  - Markdown: UTF-8 valido, niente NUL, estensione `.md`/`.markdown`.
- Nome file client: solo metadato display (sanitizzato, max len); storage key = UUID / sha256 (ASVS 5.3.2 L1).
- Tempfile vs diretto-a-store: tempfile costa I/O disco ma permette validazione completa PRIMA di toccare storage/DB e tx brevi. Su Docker Desktop Windows: tempdir su volume Linux (named volume / tmpfs del container), MAI bind mount su NTFS (lento).

### Tabella opzioni parsing multipart
| Opzione | Memoria | Limiti | Pro | Contro |
|---|---|---|---|---|
| `Multipart` + `field.bytes()` | O(file) | DefaultBodyLimit | semplice | viola "mai in memoria" — NO |
| `Multipart` + `field.chunk()` | O(chunk) | DefaultBodyLimit + layer | idiomatico axum, default | no allowed_fields nativo → check manuale |
| `multer` diretto su `Body::into_data_stream()` | O(chunk) | `Constraints` (whole_stream, per_field, allowed_fields) | limiti dichiarativi | più boilerplate; DefaultBodyLimit non si applica → serve layer |
| body raw `application/octet-stream` (no multipart) + header metadata | O(chunk) | layer | niente parsing multipart | meno standard per browser form; metadata in header/query |

**Default raccomandato**: `Multipart` + `field.chunk()` + `RequestBodyLimitLayer` globale + `DefaultBodyLimit::max` su route + contatore manuale. Rationale: zero dipendenze extra, limiti a 3 livelli (proxy, layer, loop), misurabile con dhat.

---

## 2. Storage originali (A-02)

### Finding object storage 2026
- F2.1 **MinIO CE**: README "THIS REPOSITORY IS NO LONGER MAINTAINED"; "We will no longer provide pre-compiled binary releases for the community version"; repo archiviato 2026-04-25; AGPLv3; rimanda ad AIStor (commerciale). — https://github.com/minio/minio
- F2.2 Timeline MinIO: mag-2025 console rimossa da CE; ott-2025 stop immagini Docker/binari; dic-2025 maintenance mode; feb-2026 archiviato. Immagini ufficiali congelate = CVE successive non patchate. — https://glukhov.org/data-infrastructure/object-storage/minio-dead/ ; https://pinggy.io/amp/blog/minio_archived_self_hosted_s3_alternatives/ ; https://github.com/infiniflow/ragflow/issues/13840
- F2.3 Fork community `pgsty/minio` (Docker Hub), AGPLv3, "No New Features — Just Supply Chain Continuity", console ripristinata, CVE best-effort, "please don't treat this as a commercial SLA". Release `RELEASE.2026-02-14T12-00-00Z`. — https://blog.vonng.com/en/db/minio-resurrect/index.md
- F2.4 **RustFS**: Apache-2.0, Rust, immagine `rustfs/rustfs`, porte 9000 (S3)/9001 (console), credenziali default `rustfsadmin/rustfsadmin` (cambiare!), container non-root UID 10001. Release: 1.0.0 GA 16-set-2026; ultima 1.0.3 del 2026-10-17. MinIO on-disk compat = Preview. GA ≠ privo di difetti, maturità ~1 mese. — https://github.com/rustfs/rustfs ; https://github.com/rustfs/rustfs/releases ; https://linuxiac.com/rustfs-1-0-s3-compatible-object-storage-reaches-ga/
- F2.5 **Garage** (Deuxfleurs, AGPLv3, Rust): multipart upload ✅, SSE-C ✅, **bucket versioning ❌**, object lock ❌. HW min 1 GB RAM. Single-node: `dxflrs/garage:v2.3.0`, `garage server --single-node --default-bucket` (template terzi, dev/staging); S3 :3900, admin :3903. — https://garagehq.deuxfleurs.fr/documentation/reference-manual/s3-compatibility/ ; https://garagehq.deuxfleurs.fr/ ; https://zeabur.com/templates/TVAZUD
- F2.6 **SeaweedFS** (Apache-2.0): `weed server -s3` (master+volume+filer+S3 in un processo), immagini `chrislusf/seaweedfs:4.x`; README upstream (giu-2026) promuove `weed mini` come quickstart unico. Senza `AWS_ACCESS_KEY_ID/SECRET` il gateway accetta qualsiasi chiave. — https://railway.com/deploy/seaweedfs ; https://gitea.mediatoday.ru/viaprog/seaweedfs/commit/7f1ac8cf1a2cb67e2ef674cdc0161d3a3f197df5
- F2.7 **LocalStack**: da 2026.03.0 immagine unica, **auth token obbligatorio**; free plan con registrazione (non-commercial secondo terzi); stato pre-2026.03 incompatibile. → sconsigliato per "clone pulito + docker compose up". — https://blog.localstack.cloud/localstack-single-image-next-steps/ ; https://blog.localstack.cloud/localstack-for-aws-release-2026-03-0/

### Finding client Rust
- F2.8 `object_store` 0.14.2 (Apache Arrow): backend `InMemory`, `LocalFileSystem`, S3 (`aws` feature), GCS, Azure, HTTP. `put_multipart`/`WriteMultipart` (parti parallele, no buffer intero)/`BufWriter` (AsyncWrite, sceglie single vs multipart). Range: `get_ranges`, `GetOptions::with_range` + `get_opts`; streaming `GetResult::into_stream()`. Conditional put `PutMode`, `with_if_none_match` (ETag). — https://docs.rs/object_store/latest/object_store/
- F2.9 `aws-sdk-s3` 1.152.0: API completa S3; multipart = `create_multipart_upload`/`upload_part`/`complete_multipart_upload`; `ByteStream::from_path` / `read_from().offset().length()`. Più pesante (compile time, dipendenze smithy). — https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/ ; https://docs.aws.amazon.com/AmazonS3/latest/developerguide/s3_example_s3_CreateMultipartUpload_section.md
- F2.10 `rust-s3` 0.38.0 attivo (2026-10-04); community, superficie più piccola. (crates.io)

### Finding PostgreSQL
- F2.11 TOAST (PG 18.6 docs): valore toastable max **1 GB** (2^30-1); trigger TOAST > `TOAST_TUPLE_THRESHOLD` ~2 kB; strategia EXTENDED default (compressione+out-of-line); `SET STORAGE EXTERNAL` evita compressione inutile su PDF/DOCX già compressi; `COMPRESSION lz4` per colonna. — https://www.postgresql.org/docs/current/storage-toast.html
- F2.12 bytea = valore intero in memoria lato client in lettura/scrittura (protocollo); errori "invalid memory alloc" su valori grandi; lettura a pezzi via `substring()` possibile. sqlx: bind `Vec<u8>`/`&[u8]` intero, nessuna API stream per singolo valore (non trovata doc contraria). — https://postgrespro.com/list/thread-id/1517407 ; https://postgrespro.com/list/thread-id/1928434
- F2.13 Large objects: accesso stream/random (`lo_open/lo_read/lo_lseek`, server-side `lo_put(oid, offset, bytea)`/`lo_get(oid, offset, len)`), max 4 TB, catalogo `pg_largeobject`; orfani non cancellati con la riga → `vacuumlo`/`lo_unlink`; ACL proprie. Usabili da sqlx via SQL (nessun binding nativo). — https://www.postgresql.org/docs/current/largeobjects.html (+ conoscenza generale, da verificare nelle sottosezioni 33.2–33.4)

### Tabella confronto storage (MVP 10–30 doc, ≤10 MB)
| Criterio | PG bytea chunked (tabella `blob_chunk(object_id, seq, data)` 1 MiB) | PG large object | S3: Garage | S3: RustFS | S3: SeaweedFS | S3: MinIO (pgsty fork) |
|---|---|---|---|---|---|---|
| Container extra | 0 | 0 | +1 | +1 | +1 | +1 |
| Atomicità con outbox | **stessa tx** | **stessa tx** | no: 2-phase (PUT poi tx) + GC orfani | idem | idem | idem |
| Memoria upload | O(chunk) se insert per chunk | O(chunk) via `lo_put` | O(part) multipart | idem | idem | idem |
| Range GET viewer | `WHERE seq BETWEEN` | `lo_get(off,len)` | nativo HTTP Range | nativo | nativo | nativo |
| Backup | unico `pg_dump` | pg_dump incluso (blobs) | separato | separato | separato | separato |
| Licenza | PostgreSQL | PostgreSQL | AGPLv3 | Apache-2.0 | Apache-2.0 | AGPLv3 |
| Maturità 2026 | altissima | alta ma ergonomia scomoda (orfani, ACL) | stabile, no versioning | GA da ~1 mese | matura | fork 1-maintainer, no feature |
| Realismo portfolio | medio | basso | alto | alto | alto | alto (brand noto, ma upstream morto) |
| Rischio Windows/Compose | nullo | nullo | basso (init layout) | basso (chown 10001) | basso | basso |
| Crescita (100+ doc, 100 MB file) | ok fino a ~GB totali; DB gonfio, vacuum/backup lenti | ok | ottima | ottima | ottima | ottima |

### Raccomandazione A-02
**Default MVP: PostgreSQL bytea a chunk (1 MiB) dietro porta `DocumentStore`**, schema di proprietà Document Service.
Rationale:
1. Requisito duro "persistenza sicura originale + outbox" → con PG è **una sola tx breve** (dopo tempfile validato): niente stato intermedio, niente GC orfani, niente 2-phase.
2. 10–30 doc × ≤10 MB ≈ ≤300 MB: ben sotto ogni limite (1 GB/valore irrilevante perché a chunk).
3. Chunking rispetta "mai intero file in memoria" (F2.12 vieta bytea monolitico) e dà range read per viewer (PDF.js range requests).
4. Zero container in più su Docker Desktop Windows; backup unico.
5. Porta `DocumentStore { put(stream), get(range) }` già nel design → adapter S3 (`object_store` + Garage o RustFS) aggiungibile senza toccare dominio; testabile con `object_store::memory::InMemory`.
Trigger per passare a S3 (da scrivere nell'ADR): totale originali > ~5 GB, o file > 50 MB, o multi-replica, o backup DB > budget.
Se l'utente privilegia realismo "S3": `object_store` 0.14 + **Garage** (stabile, AGPL ok per uso interno, versioning non serve: versioni gestite dal dominio) o **RustFS** (Apache-2.0, UX MinIO-like, ma GA recente). Escludere MinIO ufficiale (no immagini, archiviato) e LocalStack (token). Flusso S3: key = `sha256/<hex>` (content-addressed, PUT idempotente) → tx {doc, version(object_key), job, outbox} → sweeper orfani (oggetti senza riga dopo N min).

---

## 3. Threat model (PLAT-07)

### Finding fonti
- F3.1 ASVS 5.0 V5 File Handling: 5.2.1 L1 limite dimensione anti-DoS; 5.2.2 L1 estensione + contenuto coerenti (magic bytes); 5.2.3 L2 limite uncompressed/n. file prima di decomprimere; 5.2.4 L3 quote per utente; 5.3.1 L1 file non eseguibili se serviti; 5.3.2 L1 path da dati interni, sanitizzare filename (traversal, LFI/RFI, SSRF); 5.3.3 L3 ignora path utente in decompressione (zip slip); 5.4.1 L2 Content-Disposition con filename server; 5.4.2 L2 encoding filename (RFC 6266); 5.4.3 L2 AV su file non fidati. — https://raw.githubusercontent.com/OWASP/ASVS/master/5.0/en/0x14-V5-File-Handling.md
- F3.2 OWASP File Upload CS: allowlist estensioni (doppie estensioni, NUL, `:` ADS), Content-Type non fidato, magic mai da solo, nomi random server-side, CDR per PDF/DOCX, AV/sandbox, storage fuori webroot/host separato, authn+authz su upload e accesso, CSRF, limiti dimensione/rate, ZIP "not recommended", librerie aggiornate. — https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html
- F3.3 OWASP LLM Top 10 **2025** (nessuna versione 2026 sulla pagina): LLM01 Prompt Injection … LLM10 Unbounded Consumption. — https://genai.owasp.org/llm-top-10/
- F3.4 LLM01: injection indiretta da file/pagine ingeriti; mitigazioni: vincoli system prompt, output format validato da codice deterministico, filtri I/O + RAG Triad, least privilege, human approval, **segregare ed etichettare contenuto esterno**, adversarial testing; "RAG does not fully mitigate prompt injection"; scenario #4 documento modificato nel repository RAG. — https://genai.owasp.org/llmrisk/llm01-prompt-injection/
- F3.5 LLM02: least privilege sui dati, sanitizzazione/redaction, restringere fonti, niente leak via error message (API8:2023). — https://genai.owasp.org/llmrisk/llm022025-sensitive-information-disclosure/
- F3.6 LLM08: leakage per access control debole, cross-context leak, embedding inversion, data poisoning (istruzioni nascoste nei documenti); prevenzione: **permission-aware vector store**, partizionamento, validazione + autenticazione fonti, classificazione, log immutabili del retrieval. — https://genai.owasp.org/llmrisk/llm082025-vector-and-embedding-weaknesses/

### Asset / attori (sintesi)
Asset: credenziali + hash Argon2id, JWT/refresh, chiave firma JWT, originali documenti, chunk/embedding, ACL/ruoli, audit log, prompt/risposte, API key JEV/LLM (`TYPESAFE_API_KEY`), `DATABASE_URL`, segreti object store.
Attori: guest/employee curiosi (escalation orizzontale), utente con upload malevolo, insider manager, fonte online compromessa (ONL), attaccante rete locale, provider esterno (JEV/LLM) come destinatario dati.

### Lista minacce candidate (STRIDE × servizio → mitigazione → area)
Servizi: IAS = Identity & Access, DIS = Document & Indexing, RAS = RAG & Answer, WEB = frontend.

| ID | STRIDE | Servizio | Minaccia | Mitigazione candidata | Area req | Fonte |
|---|---|---|---|---|---|---|
| T-01 | S | IAS | Credential stuffing / brute force login | Argon2id, rate limit per IP+account, errori generici, audit login fail | AUTH-01, AUTH-03, AUD-01 | ASVS V6/V2 |
| T-02 | S/E | tutti | JWT forgiato (alg none/HS↔RS confusion, kid injection) | JWT asimmetrico, alg pinned, verifica iss/aud/exp, JWKS da IAS | AUTH | RFC 8725 |
| T-03 | I | WEB | Furto token (XSS, localStorage) | refresh in cookie HttpOnly+Secure+SameSite, access token in memoria, CSP | AUTH, VIEW | ASVS V3 |
| T-04 | E | IAS | Escalation ruolo (PATCH roles non admin, IDOR) | authz deterministica admin-only, audit cambio ruolo, version column | AUTH-02, AUD-01 | ASVS V8 |
| T-05 | D | DIS | Upload enorme / slowloris / molti upload concorrenti | limite 3 livelli (proxy, RequestBodyLimitLayer, contatore), body timeout, semaforo upload, 429+Retry-After | DOC-01 | ASVS 5.2.1, F1.3 |
| T-06 | T/E | DIS | File con tipo falso (polyglot, estensione doppia) | allowlist ext + magic + struttura; rifiuto 415 | DOC-01 | ASVS 5.2.2, F3.2 |
| T-07 | D | DIS | Zip bomb DOCX / XML bomb (billion laughs) nel parser | limiti n. entry, uncompressed totale, ratio; parser XML senza DTD/entity | DOC-01 | ASVS 5.2.3 |
| T-08 | T | DIS | Path traversal via filename / zip slip | storage key = UUID/sha256, filename solo display sanitizzato, ignora path in archivi | DOC-01 | ASVS 5.3.2/5.3.3 |
| T-09 | E/D | DIS | Exploit parser PDF/DOCX/OCR (RCE, loop, OOM) | parsing in processo/worker isolato, timeout CPU, limiti memoria cgroup, container non-root read-only, librerie aggiornate (S-02) | DOC-01, DOC-04 | F3.2 |
| T-10 | T | DIS | PDF attivo (JS, launch action, embedded file) servito al viewer | viewer PDF.js con scripting disabilitato, `Content-Disposition`, `X-Content-Type-Options: nosniff`, CSP sandbox; CDR opzionale | VIEW-01 | ASVS 5.3.1/5.4.1, F3.2 |
| T-11 | I | DIS | Side-channel dedup: hash identico rivela esistenza doc altrui (ack più veloce / errore "duplicato") | dedup fisico invisibile all'API: risposta identica, nuova version/ACL propria, no messaggi "già esistente" cross-ACL | SEC-02, DOC-01 | analisi (F1.5) |
| T-12 | R | DIS | Upload/cancellazione negati dall'autore | audit append-only con actor, sha256, request_id, version | AUD-01, AUD-03 | — |
| T-13 | I | DIS→RAS | Retrieval restituisce chunk non autorizzati (filtro ACL post-top-k o assente) | filtro ACL **dentro** la query vettoriale (pre-filter), deny-by-default, test matrice ruolo×classificazione | SEC-01, SEC-02, SEC-05 | LLM08 F3.6 |
| T-14 | I | RAS | Chunk negati arrivano a reranker/JEV/LLM/log/cache | gate unico in DIS (`/internal/v1/retrieval/search` solo autorizzati), no-leak nei log (solo ID+hash), redaction | SEC-02 | LLM02 F3.5 |
| T-15 | E | RAS | TOCTOU: ruolo revocato / doc cancellato durante query | ricontrollo `/authorize` prima di risposta e link; viewer ricontrolla | SEC-01, VIEW-02 | interfaces doc |
| T-16 | T/E | RAS | **Prompt injection indiretta** da documento caricato o fonte online ("ignora istruzioni", exfil via link/markdown immagini) | contenuto chunk delimitato/etichettato come dato non fidato; output strutturato validato da codice; niente tool/azioni dal LLM; rendering risposta senza HTML/immagini remote; segnale JEV solo consultivo; dataset sintetico injection in eval | QRY-01..05, SEC-04, ONL-03 | LLM01 F3.4 |
| T-17 | T | RAS | **Citation spoofing**: LLM inventa citazione o cita chunk non nel contesto/non autorizzato | mapping citazione deterministico: ID chunk solo da set recuperato+autorizzato, verifica span/versione, rifiuto se non valida | QRY-01, QRY-03, QRY-04, SEC-03 | LLM09 |
| T-18 | T | DIS/ONL | Data poisoning: doc online o upload interno con contenuti falsi/contradditori | quarantena, provenance+checksum+licenza, approvazione umana, JEV triage consultivo, contraddizione → rifiuto | ONL-01..03, QRY-02 | LLM04, LLM08 |
| T-19 | I/T | RAS | **Cache poisoning / leak cross-utente** (se cache introdotta) | chiave = identità+policy version+doc versions; NoOpCache MVP; no risposte complete in cache | SEC-02 | DEPENDENCIES, LLM08 |
| T-20 | I | DIS | Embedding inversion: embedding esposti ricostruiscono testo | embedding mai esposti via API; stesse ACL dei chunk | SEC-02 | LLM08 |
| T-21 | I | DIS→ONL | SSRF da connettore fonti online (URL interni, redirect, metadata endpoint) | allowlist host, no IP privati/link-local, limite redirect, timeout, size limit, reqwest+Rustls | ONL-01, ONL-03 | ASVS 5.3.2 (SSRF) |
| T-22 | I | tutti | Leak in log/telemetria (chunk, prompt, PII, token) | regola no-leak, campi tracing allowlist, scrubbing | SEC-02, AUD | LLM02 |
| T-23 | I | DIS | Accesso diretto allo storage originali (bucket pubblico, credenziali default `rustfsadmin`, SeaweedFS chiavi qualsiasi) | storage non esposto fuori rete Compose, credenziali da env non versionati, utente DB per-servizio | VIEW, DOC | F2.4, F2.6 |
| T-24 | D | RAS | Unbounded consumption: query costose, fan-out JEV/LLM | deadline globale, semaforo 8, rate limit per utente, circuit breaker | QRY | LLM10 |
| T-25 | I | RAS | Audit leak: utente vede eventi di altri / contenuti nelle query loggate | AUD-02 filtrato per ruolo, audit senza testo chunk | AUD-02 | — |
| T-26 | S | DIS | Fonte online spoofata (dominio simile, MITM) | TLS verificato, pin fonte ufficiale, checksum/ETag, provenance_status | ONL-03 | corpus-strategy |
| T-27 | R | RAS | Risposta contestata senza traccia | audit query con doc version, request_id, trace, decisione JEV | AUD-03, QRY-04 | — |
| T-28 | E | DIS | Supply chain: immagine object store non mantenuta (MinIO CE congelata con CVE) | immagini mantenute e pinnate per digest, scan immagini | PLAT | F2.1–F2.3 |

Priorità README (broken access control, prompt injection indiretta, file malevoli, esfiltrazione via citazioni/log, poisoning, furto token) → coperte da T-13/14/15, T-16, T-06..10, T-14/17/22, T-18, T-03.

---

## 4. Domande per l'utente (armate)

**Q1 — A-02: PG bytea a chunk (default) o S3-compatible?**
Finding: MinIO CE archiviato, niente immagini (F2.1–F2.2); alternative S3 = +1 container, 2-phase PUT→tx + GC orfani (F2.8); PG = una tx con outbox, 10–30 doc ≈ ≤300 MB (F2.11). Proposta: PG chunked ora + porta pronta per S3, con trigger di migrazione in ADR. Opzioni: (a) PG chunked [racc.] (b) S3 subito per realismo portfolio.

**Q2 — Se S3: Garage o RustFS (o SeaweedFS)?**
Finding: Garage stabile AGPLv3, no versioning/object lock (F2.5) — non necessari, versioni nel dominio; RustFS Apache-2.0, GA 2026-09-16, 1.0.3 del 2026-10-17, UX MinIO-like (F2.4); SeaweedFS matura ma quickstart in movimento (`weed mini`) (F2.6). Racc.: Garage per stabilità, RustFS se si vuole console/Apache-2.0.

**Q3 — Tempfile prima della persistenza o streaming diretto allo store?**
Finding: tempfile permette validazione completa (magic, struttura DOCX, zip-bomb, sha256) PRIMA di DB/storage e tx brevi (regola repo); costo = I/O disco ~10 MB, su Docker Desktop serve volume Linux non bind NTFS. Streaming diretto risparmia I/O ma scrive dati non validati (servono rollback/GC). Racc.: tempfile.

**Q4 — Livello difese file oltre ASVS L1: AV (ClamAV) / CDR / blocco PDF con JavaScript o cifrati?**
Finding: ASVS 5.4.3 AV = L2, 5.2.3 limiti archivio = L2 (F3.1); CS raccomanda CDR per PDF/DOCX (F3.2). ClamAV = +1 container (~1 GB RAM firme). Racc.: MVP = L1 + limiti archivio DOCX (economici) + rifiuto PDF cifrati/con `/JavaScript` `/Launch` `/EmbeddedFile` come flag di quarantena; AV rimandato con ADR.

---

## 5. Piano misura spike S-01 (+ input A-02)

Hardware: annotare CPU, RAM, Windows 11 build, Docker Desktop versione, backend WSL2, limiti `.wslconfig`. Build `--release`.

Setup:
- Binario spike Axum 0.8.9: route upload come §1; adapter `DocumentStore` intercambiabili: `PgChunked` (1 MiB), `ObjectStoreS3` (Garage o RustFS via object_store 0.14), `Null` (baseline solo streaming+hash).
- File fixture: 1, 5, 10, 10 MiB+1 B (atteso 413), 50, 100 MiB (con limite alzato solo nello spike, per dimostrare O(1)); PDF reale, DOCX reale, MD, polyglot, zip bomb DOCX, file troncato.

Misure:
1. **Peak heap** (criterio ROADMAP): `dhat` testing mode, un test per dimensione in `tests/`, `HeapStats::get().max_bytes`. Pass: `max_bytes(100 MiB) − max_bytes(1 MiB)` < ~1–2 MiB (cioè non scala con file). Nota: dhat lento su Windows → eseguire nel container Linux.
2. **RSS/container**: cgroup v2 `/sys/fs/cgroup/memory.peak` nel container (reset tra run riavviando), `docker stats --no-stream` come conferma. Pass: delta RSS indipendente da dimensione file.
3. **Ack latency**: `oha` o k6, 10 MiB PDF, concorrenza 1/4/8, 200 richieste per livello, p50/p95/p99 per adapter. Pass: p95 ≤500 ms a concorrenza 1 e 4 (documentare 8).
4. **Breakdown**: span `tracing` per fase (receive+hash, validate, persist_original, tx_outbox) → individua collo di bottiglia.
5. **Correttezza**: sha256 calcolato == `sha256sum` file; 413 a 10 MiB+1 senza leggere oltre (byte letti ≤ limite+chunk); 415 su polyglot; zip bomb rifiutata in < X ms con heap costante; upload interrotto a metà → nessuna riga, nessun oggetto (o oggetto orfano raccolto da sweeper per S3); doppio upload identico → 1 contenuto fisico, risposta indistinguibile (T-11).
6. **Storage**: dimensione tabella/`pg_total_relation_size` dopo 30 doc; tempo `pg_dump` con/ senza blob; range read 64 KiB p95 per viewer.
Output: tabella misure in `.planning/` (da fase), esito S-01 PASS/FAIL, input numerico per ADR A-02.

---

## Fonti (accesso 2026-10-18)
- https://docs.rs/axum/latest/axum/extract/struct.Multipart.html
- https://docs.rs/axum/latest/axum/extract/multipart/struct.Field.html
- https://docs.rs/axum/latest/axum/extract/struct.DefaultBodyLimit.html
- https://docs.rs/multer/latest/multer/struct.Constraints.html
- https://docs.rs/infer/latest/infer/
- https://docs.rs/dhat/latest/dhat/
- https://docs.rs/object_store/latest/object_store/
- https://docs.rs/aws-sdk-s3/latest/aws_sdk_s3/
- https://docs.aws.amazon.com/AmazonS3/latest/developerguide/s3_example_s3_CreateMultipartUpload_section.md
- https://crates.io/api/v1/crates/{axum,multer,aws-sdk-s3,object_store,rust-s3,infer,sqlx,tower-http,sha2,dhat}
- https://github.com/minio/minio
- https://glukhov.org/data-infrastructure/object-storage/minio-dead/
- https://pinggy.io/amp/blog/minio_archived_self_hosted_s3_alternatives/
- https://github.com/infiniflow/ragflow/issues/13840
- https://blog.vonng.com/en/db/minio-resurrect/index.md
- https://github.com/rustfs/rustfs ; https://github.com/rustfs/rustfs/releases
- https://linuxiac.com/rustfs-1-0-s3-compatible-object-storage-reaches-ga/
- https://garagehq.deuxfleurs.fr/ ; https://garagehq.deuxfleurs.fr/documentation/reference-manual/s3-compatibility/
- https://zeabur.com/templates/TVAZUD
- https://railway.com/deploy/seaweedfs
- https://gitea.mediatoday.ru/viaprog/seaweedfs/commit/7f1ac8cf1a2cb67e2ef674cdc0161d3a3f197df5
- https://blog.localstack.cloud/localstack-single-image-next-steps/
- https://blog.localstack.cloud/localstack-for-aws-release-2026-03-0/
- https://www.postgresql.org/docs/current/storage-toast.html
- https://www.postgresql.org/docs/current/largeobjects.html
- https://postgrespro.com/list/thread-id/1517407 ; https://postgrespro.com/list/thread-id/1928434
- https://raw.githubusercontent.com/OWASP/ASVS/master/5.0/en/0x14-V5-File-Handling.md
- https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html
- https://genai.owasp.org/llm-top-10/
- https://genai.owasp.org/llmrisk/llm01-prompt-injection/
- https://genai.owasp.org/llmrisk/llm022025-sensitive-information-disclosure/
- https://genai.owasp.org/llmrisk/llm082025-vector-and-embedding-weaknesses/

Limiti noti: Garage versione (v2.3.0) solo da template terzi (repo ufficiale dietro Anubis); infer DOCX/PDF da confermare in spike; comportamento sqlx su bytea streaming = nessuna API trovata (assenza, non smentita esplicita); dettagli large object da conoscenza generale; T-02/T-03/T-04 citano RFC 8725/ASVS senza fetch in questa sessione.

---

## Ricerca 01 — Parser / OCR / DOCX / isolamento (S-02, A-05, A-10)

Data accesso fonti: 2026-10-18 (tutte). Stile caveman. `[V]` = verificato su fonte; `[S]` = fonte secondaria/aggregatore; `[N]` = non verificato, da misurare nello spike.

Contesto repo: README (PDF/DOCX/MD, TTQ p95: PDF digitale 25 pp ≤15 s, 100 pp ≤60 s, scansione 25 pp ≤45 s), DEPENDENCIES ("OCR/parser — Aperta — testo, tabelle, coordinate, PDF nativi e scannerizzati"), interfaces-and-concurrency (`trait DocumentParser { async fn parse(&ObjectRef) -> Result<ParsedDocument> }`, "Parsing/OCR CPU-bound tramite `spawn_blocking` o processi isolati", spike "Parser/OCR in-process oppure worker isolato"), corpus-strategy (TAT-DQA, FinanceBench, DocLayNet; dataset sicurezza con "PDF corrotti/cifrati e scansioni di bassa qualità").

---

## 1. Parser PDF digitale con bbox + pagina (A-05)

### Findings

- **pdfium-render 0.9.4** (2026-09-06), MIT OR Apache-2.0, 3,3 M download. API: `PdfPageTextChar::loose_bounds()/tight_bounds()/origin()`, `PdfPageTextSegment::bounds()/text()/chars()` (segmento = rettangolo testo stesso baseline+font). `text()` in ordine di definizione documento, "may not match visual reading order". [V] https://crates.io/api/v1/crates/pdfium-render ; https://docs.rs/pdfium-render/latest/pdfium_render/prelude/struct.PdfPageTextChar.html ; https://docs.rs/pdfium-render/latest/pdfium_render/prelude/struct.PdfPageTextSegment.html
- pdfium-render: binding runtime a libreria dinamica (default) o `static`; "will not build Pdfium for you"; feature `pdfium_latest` = `pdfium_7881`. **"Pdfium makes no guarantees about thread safety"** → feature `thread_safe` (default) = mutex globale, serializza tutte le chiamate → nessun parallelismo in-process. 0.9.4 "fixes two double-free errors". [V] https://github.com/ajrcarey/pdfium-render
- Binari PDFium: bblanchon/pdfium-binaries, repo MIT, `pdfium-win-x64.tgz` + variante V8; linux-musl x64 "failing (#191)". PDFium upstream BSD-3/Apache (noto, [N] non riletto). [V] https://github.com/bblanchon/pdfium-binaries
- **mupdf crate 0.8.0** (2026-06-22): **AGPL-3.0**; parole con bbox + structured text. docs.rs mostra solo target linux; build Windows [N]. MuPDF = AGPL o licenza commerciale Artifex; senza: "cannot legally use MuPDF in any software that you distribute (or make available as SaaS)". [V] https://docs.rs/crate/mupdf/latest ; https://mupdf.com/r/Licensing ; https://mupdf.readthedocs.io/en/1.27.0/license.html
- **PyMuPDF**: stesso dual license AGPL/commerciale Artifex. [S] https://pymupdftest.readthedocs.io/en/stable/intro.html
- **lopdf 0.45.0** (2026-09-08, MIT): manipolazione PDF low-level, niente estrazione testo con layout → servirebbe interprete content stream proprio. [V] https://crates.io/api/v1/crates/lopdf
- **pdf-extract 0.12.1** (2026-09-16, MIT): trait `OutputDev`; bbox non pronte, serve tracciare text matrix in OutputDev custom; fork `pdf-strings` espone bbox per span. [V/S] https://crates.io/api/v1/crates/pdf-extract ; https://github.com/bbqsrc/pdf-strings
- **pdf_oxide 0.3.78** (2026-09-08, MIT OR Apache-2.0, crate nato 2025-11-05, 1,3 M download): bbox char/span/page; auto-dichiarati "100% pass rate su 3,830 PDF", "0.8ms mean", "99.5% text parity" vs PyMuPDF/pypdfium2. Numeri self-reported, progetto giovane, nessun benchmark tabelle. [V licenza / S claim] https://crates.io/api/v1/crates/pdf_oxide ; https://pdf.oxide.fyi/docs/comparison/rust
- **Poppler `pdftotext -bbox-layout`**: bbox block/line/word in XHTML. Poppler **GPL** (v2+), uso via CLI = processo separato (aggregazione, non linking) [N parere legale]. Build Windows solo di terze parti; alcuni "pdftotext.exe" Windows sono xpdf (GPLv3) senza `-bbox-layout`. [V/S] https://skia.googlesource.com/third_party/poppler/+/HEAD/utils/pdftotext.cc ; https://fossies.org/linux/poppler/utils/pdftotext.1
- **Docling** (MIT; Python ≥3.10; Win/Linux/macOS x86_64+arm64): PDF/DOCX/MD/HTML/immagini → `DoclingDocument` con bbox + provenance (pagina) per item; TableFormer per struttura tabelle; OCR plug-in (EasyOCR, Tesseract, RapidOCR, OcrMac); `docling-serve` API server; CLI. [V] https://github.com/docling-project/docling ; https://arxiv.org/html/2501.17887v1 ; https://docling-project.github.io/docling/usage/supported_formats/
- Docling perf (tech report, 8 thread): x86 CPU **3.1 s/pag media (mediana 0.79)**, p5–p95 0.6–16.3 s; M3 Max 1.27 s; L4 GPU 0.49 s (mediana 114 ms). Unstructured 4.2 s x86; Marker >16 s; MinerU 3.3 s. Dataset 89 PDF / 4,008 pag (DocLayNet+CCpdf). Nessuna metrica accuratezza tabelle nel report. [V] https://arxiv.org/html/2501.17887v1
  - Implicazione: 25 pp × 3.1 s = ~78 s CPU → **sfora TTQ 15 s** se Docling su tutte le pagine; GPU L4 ~12 s. RTX 3050 6GB [N].
- **Unstructured**: `unstructured-inference` 1.6.11 Apache-2.0; modello hi_res default `layout_v1.1.0` **proprietario**, open = yolox / detectron2_onnx ("generally underperforms"). [V] https://docs.unstructured.io/open-source/concepts/models ; https://simple-repository.app.cern.ch/project/unstructured-inference
- Coordinate viewer: PDF space origin bottom-left, punti; PDF.js `viewport.convertToViewportRectangle([xMin,yMin,xMax,yMax])`; pagine ruotate → normalizzare min/max; backend top-left (PyMuPDF) → flip Y con altezza pagina non ruotata. [S] https://www.nutrient.io/blog/pdfjs-coordinate-systems-pdf-to-screen/

### Tabella candidati parser PDF digitale

| Candidato | Licenza | Bbox | Tabelle | Windows | Integrazione Rust | Manutenzione | Rischio |
|---|---|---|---|---|---|---|---|
| pdfium-render + PDFium | MIT/Apache + BSD | char/segment, ottime | no (solo testo+bbox; ricostruzione propria) | sì, binari bblanchon | nativa FFI | attiva (0.9.4 set-2026) | C++ CVE (UAF 2026), mutex globale |
| pdf_oxide | MIT/Apache | char/span | no metrica | Rust puro [N] | nativa | molto attiva, giovane (<1 anno) | claim self-reported |
| pdf-extract / lopdf | MIT | da costruire | no | sì | nativa | attiva | lavoro proprio alto, CMap/font |
| mupdf crate | **AGPL-3.0** | parole/stext | parziale | [N] | FFI | attiva | **licenza** |
| Poppler CLI `-bbox-layout` | GPL (CLI) | word/line/block | no | build terze parti | processo | stabile | CVE DoS 2025, packaging Win |
| Docling (Python, processo/servizio) | MIT | item+pagina | **sì, TableFormer** | sì | HTTP/CLI | molto attiva | Python+modelli, 3.1 s/pag CPU |
| Unstructured OSS | Apache | sì | modello migliore proprietario | sì | HTTP/CLI | attiva | qualità OSS < hosted |
| PyMuPDF | **AGPL** | word bbox | `find_tables` | sì | processo | attiva | **licenza** |

### Default raccomandato (A-05)

**Ibrido a due livelli**: (1) **pdfium-render** in Rust = estrazione testo + bbox char/segment + numero pagina per TUTTE le pagine (fonte autoritativa coordinate citazione, stesso motore render di Chrome, licenza permissiva, Windows ok); (2) **Docling** come worker esterno opzionale SOLO per pagine con tabelle/layout complesso (o per pagine senza testo → OCR), output riallineato alle bbox PDFium. Motivo: TTQ 15 s/25 pp incompatibile con Docling full-document su CPU (≈78 s); PDFium copre testo nativo; tabelle sono il valore del dominio finanziario. Scartati MuPDF/PyMuPDF (AGPL), Poppler (GPL + packaging Win). pdf_oxide = challenger da misurare nello spike (Rust puro, niente C++), non default per maturità.

---

## 2. OCR scansioni

### Findings

- **Tesseract 5.5.3** build UB Mannheim Windows (installer ~26 MB, winget `tesseract-ocr.tesseract`; vecchio id `UB-Mannheim.TesseractOCR` fermo a 5.4.0). [S] https://www.issoh.co.jp/md/tech/8204 ; https://github.com/UB-Mannheim/tesseract/wiki
- Tesseract CLI: `tesseract img - -l ita+eng tsv` / `hocr` / `pdf`; ordine lingue influenza tempo/output; `--psm 3` default, `--psm 6` blocco uniforme, `-c preserve_interword_spaces=1`. TSV = bbox per word/line/block/par + conf. [V] https://tesseract-ocr.github.io/tessdoc/Command-Line-Usage.html
- `tessdata_best` Apache-2.0, contiene `ita.traineddata`; "Best (most accurate) trained LSTM models", solo motore LSTM. [V] https://github.com/tesseract-ocr/tessdata_best
- Binding Rust: `leptess` 0.14.0 ultimo update **2023-02-21** (fermo); `tesseract` 0.15.2 (2025-04-19). Linking libtesseract/leptonica su Windows = vcpkg, attrito. → CLI in processo separato preferibile. [V] https://crates.io/api/v1/crates/leptess ; https://crates.io/api/v1/crates/tesseract
- **ocrs** (Rust, MIT/Apache, RTen): "early preview. Expect more errors than commercial OCR engines"; solo alfabeto latino, accenti italiani non documentati; bbox word/line via `--json`. [V] https://github.com/robertknight/ocrs
- **ocr-rs 2.4.1** (2026-10-11, Apache-2.0): PaddleOCR PP-OCRv4/v5 via MNN in Rust, bbox per risultato, GPU OpenCL/Vulkan/Metal; Windows non documentato. [V] https://docs.rs/ocr-rs
- **PaddleOCR 3.x**: PP-OCRv5 multilingue 109 lingue, latino migliorato; C++ deploy Linux+Windows da 3.2.0; MKL-DNN CPU default da 3.1.0; italiano non nominato esplicitamente. [S] https://pypi.org/project/paddleocr/ ; https://www.paddleocr.ai/main/en/update/update.html
- **docTR** v1.1.0, Apache-2.0, PyTorch, output Page/Block/Line/Word con geometria; Docker CUDA 12.2. [V] https://github.com/mindee/doctr
- Velocità indicative CPU (blog vendor, bassa affidabilità): Tesseract 5 ~25 pag/min vs EasyOCR ~8 pag/min; test singola immagine Tesseract 0.162 s, RapidOCR 0.212 s, EasyOCR 0.656 s. [S] https://imagetotable.ai/blog/tesseract-vs-easyocr-2026 ; https://codesota.com/ocr/best-for-python
  - Implicazione TTQ scansione 25 pp ≤45 s: Tesseract ~25 pag/min → ~60 s single-process; con 4–8 processi paralleli su i7-13700K (16 core/24 thread) entro target [N].

### Tabella candidati OCR

| Candidato | Licenza | Italiano | Bbox | CPU | GPU (RTX 3050 6GB) | Windows | Integrazione |
|---|---|---|---|---|---|---|---|
| Tesseract 5.5 + tessdata_best ita | Apache | sì (`ita`) | word/line TSV/hOCR | buono, parallelizzabile per pagina | no | installer UB Mannheim | CLI processo |
| ocrs | MIT/Apache | latino, preview | word/line | [N] | no | Rust puro | crate |
| ocr-rs (PP-OCRv5/MNN) | Apache | latino multiling. [N] | box | [N] | OpenCL/Vulkan [N] | [N] | crate |
| PaddleOCR 3 | Apache [S] | latino 109 lingue | sì | MKL-DNN | CUDA | C++ Win da 3.2 | Python/C++ processo |
| docTR | Apache | vocab [N] | word | lento [N] | CUDA | Docker | Python processo |
| Docling OCR (EasyOCR/Tesseract/RapidOCR) | MIT + motore | dipende motore | item | vedi sopra | CUDA | sì | Python processo |

### Default raccomandato (OCR)

**Tesseract 5.5 CLI + `tessdata_best/ita` + `eng`, output TSV**, rendering pagina via PDFium a 300 DPI, un processo per pagina sotto pool limitato. Motivo: italiano supportato ufficialmente, bbox word, licenza Apache, Windows installer, isolamento gratis (processo). Bbox OCR in pixel → convertire in punti PDF (`pt = px × 72 / DPI`) per uniformità con PDFium. Challenger: PP-OCRv5 (ocr-rs o PaddleOCR) se CER Tesseract su scansioni bassa qualità insufficiente.

---

## 3. DOCX / Markdown e significato di "pagina" (A-10)

### Findings

- DOCX non memorizza pagine: "page numbers are determined by the renderer"; `w:lastRenderedPageBreak` = hint scritto all'ultimo salvataggio, non autoritativo, Word lo piazza a volte male; unici confini esatti = `w:br w:type="page"` manuali. [S] https://learn.microsoft.com/en-us/answers/a/774985 ; https://webapp.docx4java.org/OnlineDemo/ecma376/WordML/lastRenderedPageBreak.html ; https://github.com/dmitri-gb/docxjs
- Crate: `docx-rs` 0.4.22 (2026-07-21, MIT) = "A .docx file writer" (lettura limitata); `docx-rust` 0.1.11 (2026-01-22, MIT) parse+generate. Nessuno impagina. [V] https://crates.io/api/v1/crates/docx-rs ; https://crates.io/api/v1/crates/docx-rust
- Docling legge DOCX nativamente (struttura heading/tabelle), senza pagine; DOC/RTF richiedono LibreOffice. [V] https://docling-project.github.io/docling/usage/supported_formats/
- LibreOffice headless `--convert-to pdf`: istanze parallele richiedono profilo separato `-env:UserInstallation=file:///...` (altrimenti consegna al processo esistente); profilo nuovo +~4 s startup (1 s → 5 s riportato). [S] https://ask.libreoffice.org/t/multiple-instances-of-soffice-bin-at-the-same-time/95860
- LibreOffice CVE 2026 su documenti crafted: CVE-2026-63276 stack overflow in export PDF (font CFF→Type1, fix 26.2.5[.2]); CVE-2026-63277 JDBC/Java in headless (disabilitare Java); CVE-2026-4430 / CVE-2026-8357 CVSS 7.6/7.8. [S] https://www.wiz.io/vulnerability-database/cve/cve-2026-63276 ; https://byteiota.com/libreoffice-calc-rce-cve-2026-63277-patch-now-or-disable-java/
  - Implicazione: conversione DOCX→PDF = superficie d'attacco grande → solo in sandbox/processo isolato, senza rete, Java off.

### Opzioni "pagina" DOCX/MD

| Opzione | Pro | Contro |
|---|---|---|
| A. Converti DOCX→PDF (LibreOffice) e tratta come PDF | viewer unico PDF.js, highlight con bbox, citazione "pagina N" | pagine ≠ Word dell'autore (font/metriche), +dipendenza grande, CVE, +~1–5 s/doc, PDF derivato va versionato con hash |
| B. Sezioni logiche (heading) come "pagina" | fedele alla struttura, nessuna dipendenza, MD naturale | "pagina" non confrontabile con Word; sezioni lunghe → highlight serve offset carattere; viewer HTML separato |
| C. Chunk a dimensione fissa | semplice | nessun significato per utente, citazione poco verificabile |
| D. Ibrido: locator = `section_path + char_offset`; PDF di rendering opzionale | citazione stabile indipendente da rendering | due viewer (PDF.js + HTML) |

### Default raccomandato (A-10)

**Markdown**: opzione B/D — locator `heading_path + start/end char offset` sul testo sorgente normalizzato, viewer HTML con highlight per offset; "pagina" non esposta (o = sezione). **DOCX**: opzione A **solo se** lo spike conferma isolamento LibreOffice accettabile; altrimenti D (parse struttura via Docling o docx-rust, locator sezione+offset). Contratto citazione deve quindi essere un enum: `PdfLocator{page, rects[]}` | `TextLocator{section_path, char_range}` con fallback a pagina/sezione.

---

## 4. Isolamento (S-02)

### Findings

- PDFium CVE 2026: CVE-2026-9958 UAF CVSS 8.8 (fix Chrome 148.0.7778.216); CVE-2026-10002 UAF 8.8; CVE-2026-11304 UAF 5.1 (fix Chrome 149.0.7827.53). Nel browser PDFium gira in sandbox; in-process nel worker Rust = nessuna sandbox. [S] https://bugzilla.redhat.com/show_bug.cgi?id=2485271 ; https://fieldguide.lutrasecurity.com/CVE-2026-9958/
- pdfium-render 0.9.4 fixa double-free (classe memory-safety anche nel binding). [V] https://github.com/ajrcarey/pdfium-render
- Poppler 2025: CVE-2025-32364 FPE crash pdftotext, CVE-2025-32365 OOB read JBIG2, CVE-2025-43718 stack overflow metadati annidati (CWE-674), CVE-2025-52885 UAF; maggioranza DoS. [S] https://osv.dev/vulnerability/SUSE-SU-2025:01780-1 ; https://access.redhat.com/hydra/rest/securitydata/cve/CVE-2025-43718.json ; https://ubuntu.com/security/notices/USN-7858-1
- Decompression bomb PDF (FlateDecode senza cap): CVE-2026-59200 Pillow PdfParser (~950 KB → 1 GB, fix cap 200 MB, <12.3.0); CVE-2026-82864 @pdfme/pdf-lib buffer raddoppia senza limite. Mitigazione: cap output durante decompressione, rifiuto rapporto espansione anomalo. [S] https://securelayer7.net/lab/cve-2026-59200-pillow-pdfparser-decompression-bomb-dos ; https://advisories.gitlab.com/npm/@pdfme/pdf-lib/GHSA-vrqm-gvq7-rrwh/
- OWASP File Upload: allowlist estensioni; "don't trust the Content-Type header"; magic bytes "should not be used on its own"; size limit anche "after file decompression"; nome file generato (UUID); storage fuori webroot/altro server; AV o "sandbox if available"; CDR applicabile a PDF e DOCX; solo utenti autorizzati. [V] https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html
- Windows Job Objects: `JOBOBJECT_EXTENDED_LIMIT_INFORMATION` (limite memoria per processo/job), `JOBOBJECT_BASIC_LIMIT_INFORMATION` (tempo CPU, n. processi), `JOBOBJECT_CPU_RATE_CONTROL_INFORMATION`, `JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE` (chiusura handle → kill albero); figli ereditano il job salvo breakaway; jobs annidati da Win8. [V] https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects
- Docker: `--memory`, `--memory-swap` = `--memory` → no swap, `--cpus`; OOM → kernel uccide processi container. [V] https://docs.docker.com/engine/containers/resource_constraints/
- Thread-safety: PDFium non thread-safe → in-process via `spawn_blocking` serializzato da mutex = 1 documento alla volta per processo; parallelismo reale solo multi-processo. [V] https://github.com/ajrcarey/pdfium-render

### Tabella opzioni isolamento

| Opzione | Crash isolation | Limiti mem/tempo | Parallelismo PDFium | Costo | Windows dev |
|---|---|---|---|---|---|
| In-process `spawn_blocking` | **no** (segfault/abort uccide worker; UAF = RCE nel servizio) | solo timeout cooperativo, no cap memoria | 1 (mutex) | minimo | banale |
| Processo figlio per job (stesso binario, `parse-worker` subcommand, stdin/stdout JSON) | sì | Job Object (Win) / rlimit+cgroup (Linux) + kill su timeout | N processi | spawn ~ms + init PDFium | ok con job objects |
| Pool processi persistenti (riciclo ogni K doc) | sì | idem | N | basso | ok |
| Container sidecar (Docling/Tesseract/LibreOffice) `--network none --memory --cpus`, read-only, non-root | sì + FS/rete | cgroup | N | immagine grande | Docker Desktop |

### Default raccomandato (S-02)

**Processo figlio isolato per documento** (subcommand dello stesso binario Rust per PDFium; CLI per Tesseract; Docling/LibreOffice in container sidecar senza rete), con: timeout hard (wall-clock) + kill, Job Object `KILL_ON_JOB_CLOSE` + memory limit su Windows, cgroup/`--memory` su Linux, cap pagine/documento, cap byte decompressi, rifiuto PDF cifrati (stato `failed: encrypted`, mai password utente nell'MVP), file in dir temp per-job, nessuna rete. Pre-validazione in Rust prima dello spawn: dimensione, magic bytes, allowlist estensione. Motivo: CVE UAF 2026 su PDFium/LibreOffice + mutex PDFium annullano il vantaggio in-process; dataset sicurezza richiede già "PDF corrotti/cifrati".

---

## 5. Dataset benchmark parser

| Dataset | Licenza | Contenuto | Uso per parser | Note |
|---|---|---|---|---|
| TAT-DQA | **CC BY 4.0** | 2,758 doc / 3,067 pag da report finanziari, 16,558 QA; PDF originali + JSON con bbox block/word; `block_mapping` evidenze | **sì**: bbox word ground truth (estratti da PDF reader o OCR commerciale), tabelle+testo | ~85% doc 1 pagina → non testa TTQ 25/100 pp; EN only |
| FinanceBench | **CC BY-NC 4.0** (HF card) | 150 QA open (di 10,231), PDF 10-K/10-Q/8-K/earnings, `evidence_page_num` 0-indexed | test citazione a livello **pagina** e TTQ su doc lunghi | **NC**: ok portfolio non commerciale, attribuzione; EN only; GitHub senza LICENSE esplicita |
| DocLayNet | **CDLA-Permissive-1.0** | 80,863 pag, 6 categorie incl. financial reports, 11 classi, COCO + PDF singola pagina + JSON text cells con bbox | **sì** per layout/tabelle (classe Table), mAP layout | 28 GiB core; pagine singole |

Fonti [V]: https://nextplusplus.github.io/TAT-DQA/ ; https://huggingface.co/datasets/PatronusAI/financebench ; https://github.com/patronus-ai/financebench ; https://github.com/DS4SD/DocLayNet ; https://raw.githubusercontent.com/DS4SD/DocLayNet/main/LICENSE

Gap: nessun dataset IT. Servono 3–5 bilanci/relazioni italiane reali (es. relazione finanziaria annuale di società quotata, PDF pubblico) con oracolo prodotto a mano (vedi skill eval-oracle-check: oracolo indipendente dalla pipeline).

---

## Domande per l'utente (2–4)

1. **Docling (Python) ammesso come worker esterno per tabelle/layout, o pipeline solo-Rust?** — Armata da: Docling MIT + TableFormer + bbox/provenance, ma 3.1 s/pag CPU (25 pp ≈ 78 s vs target 15 s); PDFium dà bbox ottime ma zero struttura tabellare. Solo-Rust = tabelle ricostruite euristicamente da bbox.
2. **Isolamento: processo figlio + Job Object/cgroup già nell'MVP, o `spawn_blocking` in-process accettato come debito?** — Armata da: CVE-2026-9958/10002 UAF CVSS 8.8 PDFium, double-free fixati in pdfium-render 0.9.4, mutex globale PDFium (in-process = zero parallelismo), OWASP "sandbox if available".
3. **DOCX: convertire in PDF con LibreOffice (pagine "vere" + PDF.js unico) o locator sezione+offset (niente pagine)?** — Armata da: DOCX non salva pagine (renderer-dependent, `lastRenderedPageBreak` inaffidabile); LibreOffice CVE-2026-63276/63277/4430/8357 + profilo per-istanza (+~4 s).
4. **FinanceBench (CC BY-NC) accettabile per evaluation, o solo TAT-DQA (CC BY) + DocLayNet (CDLA-P) + doc IT propri?** — Armata da: licenze sopra; FinanceBench unico con doc lunghi + `evidence_page_num`.

---

## Spike: cosa MISURARE per chiudere A-05/A-10/S-02

### File di test

- 5 PDF digitali IT/EN: relazione finanziaria annuale IT (100–300 pp, tabelle multi-pagina), bilancio con tabelle numeriche dense, documento a 2 colonne, PDF con grafici vettoriali, doc 25 pp ≤10 MB (target TTQ).
- 3 scansioni: 25 pp 300 DPI buona qualità IT; scansione bassa qualità/inclinata; PDF misto (testo + pagine immagine).
- 20 pagine TAT-DQA (bbox word GT) + 20 pagine DocLayNet financial (layout/Table GT) + 3 PDF FinanceBench (se Q4 = sì).
- 2 DOCX IT (heading, tabelle, page break manuali) + 2 MD.
- Ostili (dataset sicurezza): PDF cifrato (user pwd, owner-only pwd), PDF troncato/corrotto, Flate bomb (~1 MB → ≥1 GB), PDF con 10k pagine vuote, oggetti annidati profondi (stack), JBIG2 malformato, DOCX zip bomb, DOCX con macro/link esterni, file con estensione falsa (magic mismatch).

### Metriche

| # | Metrica | Soglia proposta | Candidati |
|---|---|---|---|
| M1 | TTQ-parse p95 PDF 25 pp / 100 pp (solo parse) | ≤ 5 s / ≤ 20 s (lascia budget a chunk+embed) | pdfium-render, pdf_oxide, Docling CPU, Docling GPU 3050 |
| M2 | Accuratezza testo: CER/WER vs GT TAT-DQA word | CER ≤ 1% digitale | pdfium, pdf_oxide |
| M3 | Bbox: IoU medio word vs GT TAT-DQA; % word con IoU ≥ 0.5 | ≥ 95% | pdfium, pdf_oxide, Docling |
| M4 | Ordine lettura: % paragrafi in ordine corretto su 2 colonne | ≥ 90% | tutti |
| M5 | Tabelle: cell-level F1 / TEDS su 20 tabelle (DocLayNet + bilancio IT) | da fissare dopo baseline | Docling vs euristica bbox PDFium |
| M6 | OCR: CER IT su scansione buona / cattiva; s/pag CPU (1 e 8 processi) e GPU | CER ≤ 3% buona; 25 pp ≤ 30 s | Tesseract best vs fast, PP-OCRv5 |
| M7 | Highlight round-trip: bbox → PDF.js `convertToViewportRectangle` corretto su pagina ruotata 90° e CropBox≠MediaBox | 100% | pipeline completa |
| M8 | Isolamento: per ogni file ostile → esito `failed` tipizzato, nessun crash del servizio, picco RSS, tempo al fallimento | 100% contenuti; fallimento visibile ≤ 5 s (target README) | in-process vs processo+Job Object |
| M9 | Overhead processo figlio: spawn+init PDFium ms; throughput N processi vs mutex in-process | overhead ≤ 10% TTQ | — |
| M10 | DOCX→PDF LibreOffice: s/doc con profilo per-job, stabilità pagine (stesso doc 2 run = stesso n. pagine/hash), diff pagine vs Word | deterministico | LibreOffice vs locator sezione |
| M11 | Footprint: dimensione binari/immagini, setup Windows (passi, tempo) | documentato | tutti |

Hardware da dichiarare: i7-13700K, RTX 3050 6 GB, RAM, versione PDFium (`pdfium_7881` o chromium build), Tesseract 5.5.3, Docling versione esatta.

---

## Ricerca 01 — pgvector + embedding model (S-03, A-03)

Data accesso fonti: 2026-10-18. Stile caveman. Repo letto: README.md, DEPENDENCIES.md, docs/architecture/architecture.md, docs/data/corpus-strategy.md. Nessun file repo modificato.

Contesto vincolante: Rust + sqlx, PostgreSQL + pgvector (Proposta), corpus 10–30 doc → crescita incrementale, IT+EN finanziario, RBAC deny-by-default con predicato ACL DENTRO query vettoriale, HW dev i7-13700K / 32 GB / RTX 3050 6 GB / Win11 + Docker Desktop. Embedding model = Aperta.

Legenda affidabilità: [U] fonte ufficiale; [S] secondaria/aggregatore → verificare; [I] inferenza mia, da misurare in spike.

---

## 1. pgvector — finding

### F1. Versione corrente = 0.8.7 (2026-10-01)
- 0.8.7: fix buffer overflow build IVFFlat. 0.8.4/0.8.3 (giugno 2026): fix corruzione HNSW durante vacuum, fix insert durante vacuum HNSW. 0.8.1 (2025-09-04): supporto PG18. 0.8.0 (2024-10-30): iterative index scans + cost estimation migliore con filtri.
- Conseguenza: pin ≥ 0.8.4 obbligatorio (fix vacuum HNSW = rilevante con insert/delete incrementali). [U]
- Fonte: https://github.com/pgvector/pgvector/blob/master/CHANGELOG.md (accesso 2026-10-18)

### F2. Limiti tipi + operatori
- `vector` indicizzabile fino 2,000 dim; `halfvec` fino 4,000; `bit` fino 64,000; `sparsevec` fino 1,000 non-zero.
- Operatori: `<->` L2, `<#>` inner product negativo, `<=>` cosine, `<+>` L1, `<~>` Hamming, `<%>` Jaccard.
- Embedding normalizzati (bge-m3, Qwen3, OpenAI) → `<=>` cosine ≡ `<#>` inner product in ordinamento; `vector_ip_ops` leggermente più economico. [U]+[I]
- Fonte: https://github.com/pgvector/pgvector (accesso 2026-10-18)

### F3. HNSW vs IVFFlat
| Aspetto | HNSW | IVFFlat |
|---|---|---|
| Default param | `m`=16, `ef_construction`=64, `hnsw.ef_search`=40 | `lists` = rows/1000 (≤1M), sqrt(rows) (>1M); `ivfflat.probes`=1, start sqrt(lists) |
| Build su tabella vuota | OK (grafo incrementale) | NO — va costruito dopo dati, centroidi stantii con crescita |
| Insert concorrenti a query | OK; costo insert maggiore | OK ma qualità degrada senza REINDEX |
| Iterative scan | `strict_order` / `relaxed_order` | solo `relaxed_order` |
| Recall/latency | migliore tradeoff | build veloce, recall peggiore |
| Fit CKA (corpus crescente) | **sì** | no (centroidi su 10–30 doc inutili) |
- Fonte: https://github.com/pgvector/pgvector (accesso 2026-10-18) [U]

### F4. Filtri WHERE + ANN = post-filter → recall loss silenzioso
- Indice ANN restituisce `ef_search` candidati, POI WHERE. Esempio: filtro 10% righe + `ef_search`=40 → ~4 righe utili. Query non rallenta, restituisce meno di k. [S]
- 0.8.0: `SET hnsw.iterative_scan = relaxed_order|strict_order` → scansione continua fino a k righe filtrate o `hnsw.max_scan_tuples` (default 20,000); memoria `hnsw.scan_mem_multiplier` (default 1 × work_mem). IVFFlat: `ivfflat.iterative_scan = relaxed_order`, limite `ivfflat.max_probes`. [U]
- `relaxed_order` → risultati leggermente fuori ordine; ri-ordinare in SQL esterno (subquery + `ORDER BY distance`) o OK se reranker a valle. [U]
- Costo iterative scan cresce ~1/selettività; capped da max_scan_tuples → filtri molto selettivi (es. 0.5%) possono comunque perdere 10–20 punti recall. [S]
- Planner: 0.8.0 stima costi migliore → con filtro selettivo può scegliere B-tree/seq scan = ricerca esatta (recall 100%). Regime recall cambia silenziosamente col piano → verificare `EXPLAIN` in test. [U]+[S]
- Mitigazioni ufficiali: B-tree su colonna filtro; **partial index** per pochi valori distinti (`WHERE (category_id = 123)`); **partitioning** per molti valori. [U]
- Fonti: https://github.com/pgvector/pgvector ; https://www.postgresql.org/about/news/pgvector-0.8.0-released-2952/ ; https://www.thenile.dev/blog/pgvector-080 ; https://hashnode.com/posts/filtered-vector-search-in-pgvector-can-silently-lose-recall/6a5ea5bf7934007949197ec0 ; https://contra.com/p/EEQbEr2Z-pgvector-filterbench-making-vector-search-measurable (accesso 2026-10-18)

### F5. Implicazione RBAC (cuore della decisione)
- Requisito "unauthorized chunk mai restituito" = soddisfatto da QUALSIASI piano (WHERE è sempre applicato, ANN o esatto). Rischio vero ≠ leak, rischio = **under-return/recall loss** per utente con pochi doc autorizzati (es. `guest`). [I]
- Leak possibile solo se predicato ACL applicato FUORI SQL (filtro in Rust dopo LIMIT) → vietare. Predicato deve stare nella stessa query (`WHERE` o RLS policy). [I]
- RLS: nessun benchmark trovato su RLS + iterative scan. Predicati RLS = security barrier quals; interazione con index ordering e leakproof → da verificare con `EXPLAIN` nello spike. [S: nessuna fonte] 
- Opzioni modellazione:
  - A) colonne denormalizzate su chunk: `classification smallint`, `acl_groups int[]` (GIN) + `WHERE classification <= $lvl AND acl_groups && $groups AND version_published` — semplice, un solo HNSW.
  - B) partial HNSW per livello classificazione (4 ruoli → ≤4 indici) — recall pieno dentro livello, costo ×N build/insert.
  - C) partition per classificazione (LIST) — HNSW per partizione, pruning planner.
  - D) **ricerca esatta (no indice ANN) sotto soglia** — recall 100% con qualsiasi filtro.
- Corpus 10–30 doc ≈ 1k–15k chunk [I: 50–500 chunk/doc]. Seq scan esatto 10k × 1024 dim ≈ 40 MB float32, stima pochi–20 ms su 13700K [I, misurare]. → ANN non necessario in MVP; HNSW utile da ~50k–100k chunk [I].

### F6. Quantizzazione
- `halfvec`: ~50% storage, build HNSW più veloce (Neon: −23% build, recall equivalente su DBpedia 1M×1536, m=32, ef_construction=256; tabella Supabase/Katz: index 7,734 MB → 3,867 MB, build 264 s → 90 s, recall@ef_search=100 0.819 → 0.809). [S]
- Binary quantization: `binary_quantize(embedding)::bit(N)` + `bit_hamming_ops`, poi re-rank con vettore originale; ×32 riduzione. Utile solo a milioni di righe. [U]
- Expression index su halfvec possibile mantenendo colonna `vector`: `USING hnsw ((embedding::halfvec(1024)) halfvec_cosine_ops)`. [U, README]
- Fonti: https://neon.com/blog/dont-use-vector-use-halvec-instead-and-save-50-of-your-storage-cost ; https://supabase.com/blog/pgvector-0-7-0 ; https://github.com/pgvector/pgvector (accesso 2026-10-18)

### F7. Parametri raccomandati per scala
| Chunk | Strategia | Param |
|---|---|---|
| ≤ ~50k (MVP) | esatto (no ANN) o HNSW default | se HNSW: m=16, ef_construction=64, ef_search 100, iterative_scan relaxed_order |
| 50k–500k | HNSW, halfvec opzionale | m=16, ef_construction=128, ef_search 100–200, max_scan_tuples 20,000–50,000 |
| 500k–1M | HNSW halfvec, partial/partition su classificazione | m=16–32, ef_construction=128–256, maintenance_work_mem ≥ dimensione indice, `max_parallel_maintenance_workers` |
- Base: README defaults [U]; valori oltre default = pratica comune [I], da validare con spike.
- Build HNSW: `maintenance_work_mem` sufficiente altrimenti build lento ("graph no longer fits"); build parallelo dal 0.6. [U]

### F8. Hybrid search in Postgres
- README pgvector: combinare con full-text Postgres + RRF o cross-encoder. [U]
- Pattern RRF SQL (Railway): CTE lexical (`ts_rank_cd`, `websearch_to_tsquery`, LIMIT 50) + CTE semantic (`<=>`, LIMIT 50) + `FULL OUTER JOIN` + `1/(60+rank)`, k=60. [S]
- `ts_rank_cd` ≠ BM25 (no IDF saturazione). BM25 vero:
  - `pg_textsearch` (Tiger Data) v1.0 2026-04-03, **PostgreSQL License**, PG17/18; supportato su AlloyDB/Cloud SQL (2026-09-21). [U]
  - ParadeDB `pg_search` = AGPL (dichiarato da Tiger, non verificato) → evitare per licenza. [S]
- Bilingue: config `italian` / `english` snowball built-in Postgres [I: verificare `SELECT cfgname FROM pg_ts_config`]. Pattern: colonna `lang regconfig` per chunk, `to_tsvector(lang, text)`; query su entrambi o `simple` fallback.
- RBAC: predicato ACL in ENTRAMBE le CTE (lexical e semantic). [I — requisito]
- bge-m3 produce anche sparse lexical weights → alternativa a tsvector via `sparsevec` (≤1,000 non-zero). [U fastembed + pgvector]
- Fonti: https://github.com/pgvector/pgvector ; https://docs.railway.com/guides/hybrid-search-postgres-pgvector ; https://www.postgresql.org/about/news/pg_textsearch-v10-3264 ; https://www.tigerdata.com/newsroom/google-cloud-brings-native-bm25-full-text-search-to-alloydb-and-cloud-sql-via-tiger-datas-pg_textsearch ; https://www.postgresql.org/docs/current/textsearch-configuration.html (accesso 2026-10-18)

### F9. Docker image
- PostgreSQL corrente = 18 (docs 18.6); PG19 RC1 2026-10-15 → non usare. [U]
- Tag ufficiali: `pgvector/pgvector:pg18-trixie`, `pgvector/pgvector:<ver>-pg18-trixie`, varianti bookworm, pg13–pg18. [U README] Pin consigliato: `pgvector/pgvector:0.8.7-pg18-trixie` [I: verificare esistenza tag su Docker Hub prima del pin; fallback `0.8.x-pg17`].
- pg_textsearch NON incluso nell'immagine pgvector → serve Dockerfile derivato se scelto BM25. [I]
- Fonti: https://github.com/pgvector/pgvector ; https://docs.pgedge.com/pgvector/v0-8-2/additional-installation-methods/ ; https://www.postgresql.org/docs/current/textsearch-configuration.html (accesso 2026-10-18)

---

## 2. Embedding model — finding

### F10. Tabella candidati locali
| Modello | Param | Dim (MRL) | Max token | Licenza | VRAM fp16 | MMTEB multil. mean | Serving Rust |
|---|---|---|---|---|---|---|---|
| Qwen3-Embedding-0.6B | 0.6B (595M) | 1024 (32–1024) | 32,768 | Apache 2.0 | ~1.2 GB | 64.33 [S] | fastembed `qwen3` (candle), TEI, Ollama `qwen3-embedding:0.6b` |
| BAAI/bge-m3 | 568M | 1024 | 8,192 | MIT | ~1.1 GB | 59.56 [S] | fastembed `Bgem3Embedding` (dense+sparse+ColBERT), TEI (XLM-R) [I], Ollama `bge-m3` |
| multilingual-e5-large-instruct | 560M | 1024 | 512 | MIT | ~1.1 GB | 63.22 [S] | fastembed (e5 large non-instruct listato), TEI |
| multilingual-e5-base | 278M [I] | 768 | 512 | MIT | ~0.6 GB | n.d. | fastembed ONNX |
| EmbeddingGemma-300m | 308M | 768 (128–768) | 2,048 | Gemma Terms of Use | ~0.6 GB | 61.15 | fastembed, TEI 1.8, Ollama |
| EmbeddingGemma 2 | 740M (270M text) | 768 | 8,192 | Apache 2.0 | ~0.5 GB text | n.d. | da verificare |
| Granite Embedding 311M R2 | 311M | 768 | 32,768 | Apache 2.0 | ~0.6 GB | n.d. | da verificare |
| nomic-embed-text-v2-moe | 475M (305M attivi) | 768 (→256) | 512 | Apache 2.0 | ~1 GB [I] | n.d. | fastembed `nomic-v2-moe` (candle), Ollama |
| jina-embeddings-v3 | 572M | 1024 | 8,192 | **CC BY-NC 4.0** | — | — | **escluso** (non commerciale) |
| jina-embeddings-v4 | — | — | — | Qwen Research License | — | — | **escluso** |
- Tutti ≤ 1.2 GB fp16 → stanno in RTX 3050 6 GB con margine; Qwen3-4B (~8 GB) NON sta. [S+I]
- Fonti: https://wz-it.com/en/blog/best-embedding-models-german/ (pubbl. 2026-09-16, agg. 2026-10-12) ; https://arxiv.org/pdf/2506.05176 ; https://hub.docker.com/r/ai/qwen3-embedding ; https://www.siliconflow.com/ko/models/qwen3-embedding-0-6b ; https://build.nvidia.com/baai/bge-m3/modelcard ; https://cloud.google.com/vertex-ai/generative-ai/docs/maas/e5/multilingual-e5-large ; https://huggingface.co/blog/embeddinggemma ; https://arxiv.org/html/2509.20354v2 ; https://huggingface.co/nomic-ai/nomic-embed-text-v2-moe ; https://huggingface.co/jinaai/jina-embeddings-v3/blob/e9ac638886aae46dc290174ed6a8f5423a90da49/README.md ; https://jina.ai/en-US/models/jina-embeddings-v4/ (accesso 2026-10-18)

### F11. Evidenza italiana (scarsa, contraddittoria)
- IT-RAG-Bench (Cirillo et al., arXiv 2605.23618, 2026-05-22): 640 query sintetiche IT, nDCG@10: Gemini Embedding 2 0.282, mE5-large ~0.279, bge-m3 0.238, LaBSE 0.189. Qwen3 assente. [U paper, valori bassi = benchmark difficile]
- ESG IT (arXiv 2609.15434): text-embedding-3-large NDCG 0.975 top su italiano; modelli piccoli (all-minilm) calano IT vs EN. [S]
- MDPI BDCC 2025 (12 modelli, SQuAD-it/DICE): multilingual > modelli monolingua IT; Cohere embed-multilingual-v3.0 nDCG@10 0.86 IT vs 0.90 EN. [S, pagina 403]
- Qwen3 paper: 0.6B >> bge-m3 dense su tabella retrieval multilingue (76.74 vs 49.65 avg colonna finale; colonne non identificate). [S]
- Tedesco (WZ-IT): Qwen3-4B > bge-m3 su MIRACL-de (nDCG@10 62.98 vs 57.59); 4B > 8B. "Nessun vincitore unico → testare su propri documenti." [S]
- **Nessun benchmark finanziario IT trovato** → eval in-domain obbligatoria.
- Fonti: https://arxiv.org/abs/2605.23618 ; https://arxiv.org/pdf/2609.15434 ; https://www.mdpi.com/2504-2289/9/5/141 ; https://arxiv.org/pdf/2506.05176 ; https://wz-it.com/en/blog/best-embedding-models-german/ (accesso 2026-10-18)

### F12. API (baseline/fallback, solo se dati possono uscire)
| API | Dim | Max token | Prezzo /1M token | Note |
|---|---|---|---|---|
| OpenAI text-embedding-3-small | 1536 (riducibile) | 8,191 | $0.02 | economico |
| OpenAI text-embedding-3-large | 3072 (riducibile, MRL) | 8,191 | $0.13 (batch $0.065) | top su ESG IT |
| Cohere embed-v4.0 | 256/512/1024/1536 default/3072 | 128K | ~$0.12 [S] | multimodale |
| Voyage voyage-4 / -lite / -large | 1024 default (MRL 256–2048) | 32K | $0.06 / $0.02 / $0.12 [S] | lancio 2026-01-15, spazio embedding condiviso tra taglie |
| Mistral mistral-embed | 1024 | 8,192 | $0.10 [S] | UE (GDPR comodo) |
- Rischio: ogni chunk (anche riservato) esce verso provider → tensione con principio no-leak README; accettabile solo per corpus pubblico/evaluation. [I]
- Fonti: https://developers.openai.com/api/docs/models/text-embedding-3-small ; https://openai.com/blog/new-embedding-models-and-api-updates ; https://community.openai.com/t/inconsistent-pricing-for-text-embedding-3-large-between-model-card-and-pricing-page/1338277 ; https://cloudprice.net/models/cohere-embed-v4-0 ; https://techsy.io/en/blog/voyage-vs-openai-vs-cohere-embeddings ; https://www.getmaxim.ai/bifrost/llm-cost-calculator/provider/mistral/model/mistral-embed.md (accesso 2026-10-18)

### F13. Serving da Rust
| Opzione | Pro | Contro |
|---|---|---|
| fastembed-rs 7.1.1 (2026-10-07) in-process | no servizio extra; ort ONNX per bge-m3/e5/EmbeddingGemma; Qwen3/nomic via candle (feature flag); `Bgem3Embedding` dense+sparse+ColBERT; GPU Windows via `directml` | build Rust più pesante (ort/candle), CUDA non documentato in pagina, embedding nel processo del Document Service (CPU contesa col worker) |
| Ollama HTTP `/api/embed` (batch) | install Windows nativo con GPU, modelli `bge-m3`, `qwen3-embedding:0.6b`, `embeddinggemma` | quantizzazione GGUF → vettori ≠ HF riferimento; meno controllo pooling/istruzioni |
| HF Text Embeddings Inference (container) | standard produzione, batching dinamico, Qwen3-0.6B in quick tour, XLM-R (bge-m3/e5) | GPU via Docker Desktop/WSL2 (RTX 3050 = Ampere sm_86) [I]; immagine CPU `cpu-1.8.x` |
| candle diretto | puro Rust | più codice, meno ottimizzato |
- Tutte dietro porta `EmbeddingProvider` (DEPENDENCIES: "Ogni provider implementa una porta interna"). Adapter HTTP con `reqwest` = già in stack.
- Fonti: https://docs.rs/crate/fastembed/latest ; https://github.com/Anush008/fastembed-rs ; https://huggingface.co/docs/text-embeddings-inference/quick_tour ; https://huggingface.co/docs/text-embeddings-inference/main/en/supported_models ; https://releases.sh/hugging-face/text-embeddings-inference ; https://techsy.io/en/blog/run-embedding-models-locally-ollama (accesso 2026-10-18)

### F14. Latenza/throughput su HW dev
- Nessun dato misurato pubblico per RTX 3050 / 13700K trovato. Qualitativo: ≤600M param OK su CPU per query; indicizzazione iniziale molto più lenta su CPU che GPU. [S WZ-IT]
- Stima [I, da misurare]: query singola (≤64 token) 0.6B su GPU ~10–30 ms; indicizzazione 10k chunk × 512 token su GPU ~ pochi minuti, su CPU ×5–20.
- Target TTQ README: PDF 25 pag ≤ 15 s p95 → ~50–150 chunk/doc → embedding deve stare in ~3–5 s del budget → GPU o modello piccolo su CPU. [I]

---

## 3. Raccomandazione default (da validare con spike)

**Vector store**
- `pgvector/pgvector:0.8.7-pg18-trixie` (pin esatto dopo verifica tag; minimo 0.8.4).
- Colonna `embedding vector(1024)` (storage float32; halfvec solo come expression index quando > ~100k chunk).
- RBAC: colonne denormalizzate su `chunk` (`classification`, `acl_groups int[]`, `published_version_id`) + predicato nella stessa query SQL; ACL in OGNI CTE hybrid.
- MVP: **ricerca esatta** (nessun indice ANN) → recall 100% con qualsiasi filtro ACL, zero tuning; HNSW cosine (m=16, ef_construction=64→128, `hnsw.ef_search`=100, `hnsw.iterative_scan=relaxed_order` + re-sort esterno) attivato a soglia misurata (es. p95 esatto > 50 ms).
- Hybrid: tsvector con `regconfig` per lingua + RRF k=60 come baseline; `pg_textsearch` (PostgreSQL License) come upgrade se spike mostra guadagno.
- Rationale: corpus piccolo rende ANN ottimizzazione prematura; deny-by-default + recall pieno senza dipendere da selettività; percorso di crescita chiaro (HNSW → partial/partition) senza cambiare schema.

**Embedding**
- Default spike: **Qwen3-Embedding-0.6B @1024** (Apache 2.0, 32K ctx, MMTEB 64.33, instruction-aware, VRAM ~1.2 GB).
- Challenger: **bge-m3** (MIT, dense+sparse in un passaggio → hybrid nativo, ONNX/ort maturo) e **multilingual-e5-large-instruct** (MIT, migliore evidenza IT su IT-RAG-Bench, ma 512 token).
- Baseline API (solo su evaluation corpus pubblico): text-embedding-3-large.
- Serving default: adapter HTTP (`reqwest`) verso TEI o Ollama in Compose → build Rust leggera, swap modello senza ricompilare; fastembed-rs in-process come alternativa misurata.
- Esclusi: jina v3/v4/v5 (licenza), Qwen3-4B/8B (VRAM).
- Rationale: nessuna evidenza IT-finanziaria decisiva → scegliere su eval in-domain; Qwen3-0.6B ha miglior score aggregato e licenza pulita; bge-m3 vince se hybrid sparse conta.

---

## 4. Domande per l'utente (armate)

1. **Ricerca esatta in MVP, HNSW solo a soglia?**
   Finding F4/F5: HNSW + filtro ACL = post-filter, recall loss silenzioso con filtri selettivi (es. guest); iterative scan mitiga ma capped a 20,000 tuple. A 1k–15k chunk scan esatto stimato pochi ms e recall 100%. Opzioni: (a) esatto ora + HNSW a soglia; (b) HNSW da subito con iterative_scan per esercitare il percorso di produzione.

2. **Embedding esce dalla macchina?**
   Finding F12: API (OpenAI 3-large top su ESG IT, $0.13/1M) inviano ogni chunk al provider → conflitto con "nessun chunk non autorizzato raggiunge … log/terzi". Opzioni: (a) solo locale; (b) API ammessa solo per evaluation corpus pubblico come baseline; (c) API anche runtime per doc non classificati.

3. **Serving: servizio HTTP (TEI/Ollama) o in-process (fastembed-rs)?**
   Finding F13: fastembed 7.1.1 supporta tutti i candidati (ort + candle, DirectML su Windows) ma appesantisce build Rust e contende CPU col worker; TEI/Ollama = container in più ma swap modello senza ricompilare. GPU RTX 3050 da Docker richiede WSL2 passthrough.

4. **Hybrid lessicale: tsvector built-in o pg_textsearch BM25?**
   Finding F8: `ts_rank_cd` non è BM25; pg_textsearch v1.0 PostgreSQL License ma richiede immagine Docker derivata; bge-m3 offre sparse via `sparsevec`. Numeri finanziari/sigle (es. "EBITDA 2025", codici ISIN) favoriscono match lessicale.

---

## 5. Piano misure spike

### Dataset minimo di valutazione
- 10–15 doc reali (mix IT/EN, testo+tabelle; sottoinsieme TAT-DQA per EN + 5 relazioni/bilanci IT pubblici).
- 60–100 query etichettate: ~50% IT, ~50% EN, ~20% cross-lingual (query IT → evidenza EN e viceversa), ~15% numeriche/sigle, ~10% senza risposta.
- Etichetta = (doc_id, page, span) → chunk rilevanti mappati per overlap span (indipendente dal chunker, regola eval-oracle). Etichettatura umana su estratto sorgente, non generata dalla pipeline.
- Profili ACL sintetici: 4 ruoli; ≥1 profilo con selettività ≤5% chunk (stress filtro); doc "trappola" con risposta solo in contenuto vietato.
- Ground truth ANN: stessa query con ricerca esatta (`SET enable_indexscan=off` o senza indice) → recall ANN vs esatto separato da qualità embedding.

### Metriche
| Metrica | Come | Target indicativo [I] |
|---|---|---|
| Recall@5/@10, MRR@10, nDCG@10 per modello | query etichettate, ricerca esatta | scegliere max; delta < 0.02 = pari → decide licenza/latency |
| Recall per lingua e cross-lingual | split IT/EN/cross | nessun crollo > 0.10 su IT |
| Hybrid vs dense | RRF k=60 vs dense-only | adottare hybrid se +≥0.03 recall@10 |
| Filtered recall ANN vs esatto | HNSW (ef_search 40/100/200, iterative off/relaxed/strict) per selettività 100%/25%/5%/1% | ≥ 0.95 vs esatto; returns-k rate = 100% |
| Leak test | per ogni profilo: 0 chunk con ACL negata in top-k, anche in CTE lexical | **0, hard gate** |
| Latency query p50/p95 | embedding query + SQL, 200 run, warm cache | p95 SQL < 50 ms; p95 end-to-end retrieval < 200 ms |
| Embedding throughput | chunk/s GPU e CPU, batch 16/32/64 | soddisfa TTQ 25 pag ≤ 15 s |
| Index build time + size | HNSW vector vs halfvec, a 10k e 100k chunk sintetici (duplicati/perturbati) | registrare; no target |
| Insert durante query | ingestion concorrente + query loop → p95 e errori | nessun errore, p95 degradazione < 2× |
| Piano query | `EXPLAIN (ANALYZE, BUFFERS)` registrato per ogni config | piano atteso confermato (index vs seq) |
| RLS (se valutato) | stesso set con policy RLS al posto di WHERE | stesso recall + piano |

### Output spike
- Tabella risultati versionata in `evaluation/` (a cura fase successiva), config esatte (versione immagine, modello+revisione, dtype, ef_search).
- Decisione: modello embedding + dim, esatto vs HNSW + soglia, hybrid sì/no → aggiornare DEPENDENCIES.md (Proposta → Decisa) via ADR.

---

## Ricerca 01 — JEV, cancellazione, LLM generativo, reranker baseline

Aree: S-04/A-09 (JEV), S-05 (cancellazione), A-04 (LLM), A-06 (reranker). Accesso fonti: **2026-10-18**. Stile caveman. Nessun file repo toccato.
Contesto letto: README.md, DEPENDENCIES.md §JEV, ADR-0002, interfaces-and-concurrency.md (baseline: top-20, max 8 decisioni JEV concorrenti, deadline globale; regola no-leak: nessun chunk non autorizzato verso reranker/JEV/LLM/cache/log).

Legenda affidabilità: **[P]** fonte primaria · **[S]** secondaria · **[STIMA]** inferenza mia, da misurare nello spike · **[NV]** non verificato (rete/fonte assente).

---

## 1. JEV (S-04 / A-09)

Docs raggiungibili: sì. `https://docs.typesafe.ai/api` via WebFetch OK. `https://api.typesafe.ai/openapi.json` bloccato da WebFetch ma scaricato via curl (HTTP 200, 21660 B, OpenAPI 3.1.0, `info.version` `0.2.0`).

### 1.1 Contratto `POST /v1/systemone` [P]
Fonti: https://docs.typesafe.ai/api · https://api.typesafe.ai/openapi.json (2026-10-18)

- Auth: `Authorization: Bearer <API_KEY>` (securityScheme `HTTPBearer`). `Content-Type: application/json`. Env var usata dagli SDK: `TYPESAFE_API_KEY`.
- Request `SystemOneRequest`, required `model`, `state`, `questions`:
  - `model`: string, nome o alias (`jev-latest`).
  - `state`: string | object | array. **Uno solo per request**; tutte le domande si riferiscono a quello.
  - `questions`: map `id → Question`, `minProperties: 1`. Id solo per il codice, "not sent to the model".
  - `assets` (opz., immagini, header `TypeSafe-Danger-Zone-Images: true`) → fuori scope happy path.
- Tipi domanda:
  - `noul`: `instructions` (opz.), `criteria {true,false}` opz. → answer `{type, noul ∈[0,1]}`.
  - `choice`: `criteria` map, `minProperties 1`, `maxProperties 255` → `{choice, confidence, probabilities}` (somma ≈1).
  - `score`: `criteria` array ordinato, `minItems 1` (doc: "at least two"), `maxItems 10` → `{score (media pesata), confidence, legend, probabilities}`.
  - `instructions`/`criteria` accettano anche JSON strutturato; path tipo `` `passages[3]` `` per puntare a parti dello state.
- Response `SystemOneResponse`, required `model`, `answers`, `usage{input_tokens, output_tokens}`.
- Header opzionale `TypeSafe-Danger-Zone-Long-Context: true` → limite 262k token, "slower and less accurate".

### 1.2 Limiti [P]
Fonte: https://docs.typesafe.ai/api · https://docs.typesafe.ai/models
- Body ≤ **32 MiB** (compresso e decompresso). UTF-8 senza BOM. Nesting ≤ **64**. Numeri finiti (id grandi → stringhe).
- `Content-Encoding`: `gzip` | `x-gzip` | `zstd` (finestra zstd ≤ 8 MiB, un solo layer). SDK comprimono zstd da ≥ 1 KiB.
- Contesto default: **64k token per request**, **32k per question** (state + una domanda).
- Rate limit: per organizzazione, token/s e request/s; **valori numerici non pubblicati** ("in the developer console"). Superamento → `429` (+ `retry-after` se presente).
- Latenza: **nessun SLA/valore ufficiale**. Unico dato: cookbook parallel_questions, GDPR ~54k char, 13 domande → **1 call 0.27s vs 13 call sequenziali 2.71s** (12.2x più economico, 10.0x più veloce, risposte identiche). https://docs.typesafe.ai/cookbooks/parallel_questions.md
- Failure mode noti jev-1.13 (rilevanti per noi): lettura letterale, numeri/date inaffidabili, **"Large state full of irrelevant detail" → accuratezza cala**, adversarial content, ordine opzioni Choice. https://docs.typesafe.ai/model-jaggedness/jev-1.13.md

### 1.3 Errori [P]
| Codice | Significato | Retry? |
|---|---|---|
| 400 | regola request violata; `max_tokens_exceeded` = oltre contesto | no |
| 401 | key mancante/invalida | no |
| 413 | body > 32 MiB | no |
| 415 | encoding non supportato/multiplo | no |
| 422 | validazione (body nomina il campo) | no |
| 429 | rate limit | sì, backoff + `retry-after` |
| 529 | overload temporaneo | sì |
Nota: OpenAPI dichiara solo `200` e `422`; il resto è nella prosa docs → adapter Rust deve gestire codici non tipizzati.
Default retry SDK Python (`RetryPolicy`): `max_retries 2`, `backoff_initial 0.5`, `backoff_max 5.0`, `backoff_jitter 0.25`, status `{408, 429, 500–599}`, `respect_retry_after True`, `timeout 30.0`. https://docs.typesafe.ai/sdk/python/api/retries.md

### 1.4 Modello, alias, versione risolta [P]
Fonte: https://docs.typesafe.ai/models
| Alias/ID | Punta a | Note |
|---|---|---|
| `jev-latest` | `jev-1.13.0` | ultima stabile; default SDK |
| `jev-preview` | `jev-1.14.0` | release più recente (dal 2026-10-14) |
| `jev-1.13.0`, `jev-1.14.0` | — | ID versionati accettati anche se non listati da `GET /v1/models` |
- Versione risolta: campo `model` della response ("May differ from the alias supplied"). Docs: "If you've tuned confidence thresholds against a specific version, send its versioned ID instead."
- Incoerenza docs: esempio OpenAPI restituisce `"model": "jev-latest"`, esempio docs API `"jev-1.13.0"` → spike: verificare cosa torna davvero.
- `GET /v1/models` → `{models:[{name, description, release_date}]}`.

### 1.5 Prezzo / free tier [P + S]
- **$0.042 / Mtok input** ($42/Btok), **output gratis**, jev-1.13 e 1.14 [P models].
- Free tier/crediti: **non trovato** in docs né privacy policy; nessuna pagina pricing (404 riportati da terzi) [S]. → chiedere/verificare in console.
- Ordine di grandezza per noi [STIMA]: rank top-20 × ~500 tok/pair ≈ 10k tok → **≈ $0.0004/query**. Costo trascurabile; vincolo vero = rate limit + latenza.

### 1.6 Privacy / retention [P]
- Privacy Policy: "We will not train or fine tune any artificial intelligence or machine learning models on your prompts or other Input." Hosting: "The Services are hosted in the United States". Retention: "as long as reasonably necessary". https://typesafe.ai/legal/privacy-policy
- DPA: retention "as long as necessary taking into account the purpose"; **nessun termine in giorni, nessuna procedura di cancellazione**; SCC Modulo 2, legge irlandese; subprocessor su https://trust.typesafe.ai/subprocessors; notifica incidenti 72h. https://typesafe.ai/legal/data-processing
- **ZDR solo enterprise** (sales@typesafe.ai). https://docs.typesafe.ai/legal.md
- Implicazione: ogni chunk inviato a JEV esce verso USA con retention indefinita → coerente solo con corpus portfolio/sintetico o con ZDR.

### 1.7 Più domande in una request vs una call per candidato [P + STIMA]
Docs raccomandano: "Send every question that uses the same state in one request" (https://docs.typesafe.ai/primitives.md). Ma lo `state` è **uno**: N candidati ≠ N domande sullo stesso state.

| Strategia | Forma | Pro | Contro | Fonte |
|---|---|---|---|---|
| A. 1 request/candidato | `state={query, passage}`, k Noul (relevant, usable, contradicts_premise, instructs_model) | pattern ufficiale RAG + rerank; candidati isolati (no contaminazione); state piccolo; 1 risposta = 1 chunk → facile da cancellare/limitare | 20 round-trip; dipende da rate limit; query ripagata 20× (trascurabile) | cookbooks/classifying_rag_passages, cookbooks/rerank_typesafe (1200 call, `max_workers=12`, 1.536M tok → $0.0645) |
| B. 1 request, 20 Noul | `state={query, passages[20]}`, Noul "is `passages[i]` relevant…" | 1 round-trip, latenza ~costante | ogni domanda vede 20 passaggi → distrattori ("Large state…" jaggedness); per-question 32k include tutto lo state | primitives (path), jaggedness §5 [STIMA rischio] |
| C. 1 request, 1 Choice su id | `criteria` = id passaggi (≤255) | stile semantic_find / skill_suggestion | probabilità a somma 1 = ranking relativo, non rilevanza assoluta → inadatto ad evidence gate | cookbooks/semantic_find, skill_suggestion |

Mappa sui metodi `DecisionEngine` [STIMA]:
- `validate_query`: 1 request, state = query, più Noul/Choice (fan-out speculativo).
- `rank`: **A** (default) con k Noul per pair → produce anche segnali per `assess_evidence` (stesso call, zero costo extra).
- `assess_evidence`: 1 request su evidenze selezionate (piccolo state) oppure derivato da A + 1 Noul globale "is the answer contained".
- `validate_citations`: 1 request per claim, Choice `supports/contradicts/says_nothing` (cookbook citation_check). https://docs.typesafe.ai/cookbooks/citation_check.md

---

## 2. Cancellazione e deadline (S-05)

Fonti: https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html (reqwest **0.13.5**) · https://docs.rs/reqwest/latest/reqwest/struct.RequestBuilder.html · https://docs.rs/tokio/latest/tokio/task/struct.JoinSet.html (tokio **1.53.3**) · https://github.com/hyperium/h2/issues/633 · https://github.com/hyperium/hyper/issues/2872 · https://github.com/hyperium/hyper/issues/1585 · https://github.com/hyperium/hyper/pull/3647 (2026-10-18)

Finding:
- reqwest timeout [P]: `ClientBuilder::timeout` = totale "from when the request starts connecting until the response body has finished", default nessuno. `RequestBuilder::timeout` "overrides" quello client, per request. `read_timeout` per singola read, si resetta. `connect_timeout` solo connect. `pool_idle_timeout` default 90s; `pool_max_idle_per_host` default `usize::MAX`.
- Drop del future = cancellazione lato client [P docs Rust + S issue hyper]: future Rust lazy, drop → hyper abbandona. HTTP/1.1: connessione con response a metà non torna nel pool → chiusa (nuovo handshake TLS dopo). HTTP/2: drop degli handle con dati pendenti → `RST_STREAM(CANCEL)`; connessione resta riusabile.
- Lato server [STIMA, nessuna doc JEV]: body già inviato = **dati già usciti**; JEV può completare ed **addebitare** la richiesta. Nessun endpoint cancel/idempotency nell'OpenAPI. Cancellazione = solo risparmio risorse client + rispetto deadline, **non** garanzia privacy né costo.
- `tokio::time::timeout(d, fut)` → drop di `fut` a scadenza (stesso effetto). `tokio::time::timeout_at(deadline, …)` per deadline globale condivisa.
- `JoinSet`: "When the JoinSet is dropped, all tasks … are immediately aborted"; `shutdown()` = abort_all + drain; `join_next` cancel-safe [P].
- Rischi noti hyper: connect future droppato in race con checkout pool (issue 1585), task connessione non chiuso (PR 3647) → tenere hyper/reqwest aggiornati, test di soak.

Pattern raccomandato [STIMA su doc P]:
| Bisogno | Strumento | Nota |
|---|---|---|
| deadline globale query | `Instant` deadline in `QueryContext`; ogni call `timeout_at(deadline - margine)` | anche `RequestBuilder::timeout(min(remaining, per_call_cap))` |
| fan-out bounded rank | `futures::stream::iter(cands).map(call).buffer_unordered(8)` dentro `timeout_at` | niente spawn, drop stream = cancella tutto; più semplice |
| fan-out con task spawn | `JoinSet` + `Semaphore` (permit acquisito prima di spawn) | drop JoinSet = abort; serve se lavoro CPU o `'static` |
| limite globale cross-query verso JEV | `Arc<Semaphore>` nel client JEV (es. 8–16) + rate limiter token/s | evita 429 a cascata; distinto dal limite per-query |
| retry | budget per query (es. max 1 retry, solo se `remaining > p95`) | rispettare `retry-after`; mai retry oltre deadline |
| HTTP | `http2` feature + `pool_idle_timeout(90s)` default; `connect_timeout(2s)`; `read_timeout` | verificare se api.typesafe.ai negozia h2 (spike) |
Fallback su timeout: rank parziale → **solo** candidati con decisione JEV; o baseline locale; mai bypass della sicurezza (coerente con doc architettura).

---

## 3. LLM generativo (A-04)

### 3.1 Locale su RTX 3050 6 GB
Fonti: https://ollama.com/library/gemma3/tags · /gemma4/tags · /ministral-3/tags · /llama3.1/tags (curl 2026-10-18; qwen3/qwen3.5 timeout → [NV]) · https://insiderllm.com/guides/best-local-llms-rag/ [S] · https://docs.ollama.com/capabilities/structured-outputs [P]

| Modello (Ollama tag) | Dim. file default | Sta in 6 GB con KV+embedder? | IT | Note |
|---|---|---|---|---|
| `gemma3:4b` | 3.4 GB [P] | sì | buono [S, NV bench IT] | vision, 128k ctx |
| `qwen3:4b` | ~2.5 GB [NV] | sì | multilingue 100+ lingue [S] | thinking opzionale |
| `qwen3:8b` Q4 | ~5.5 GB [S] | **al limite** (KV ctx 8k + embedder → offload CPU) | buono [S] | "answer only from context" meglio di Llama 3.1 8B [S] |
| `llama3.1:8b` Q4 | 4.9 GB [P] | stretto | discreto (IT ufficialmente supportato) | baseline community |
| `ministral-3:8b` | 6.0 GB [P] | no (offload) | buono (Mistral, EU) [NV] | `ministral-3:3b` 3.0 GB entra |
| `gemma4:e2b` / `e4b` | 4.6 / 6.6 GB [P, mapping varianti NV] | e2b sì, e4b no | [NV] | famiglia nuova, verificare |
| `qwen3.5` small | tag 0.8b 1.2 GB; altre taglie 2.7–6.6 GB [P, mapping NV] | da verificare | [NV] | rete fallita su dettaglio |
- Structured output: Ollama `format` = JSON schema (anche `response_format` OpenAI-compat); consigliato temperatura 0 + schema nel prompt [P]. llama.cpp: GBNF / `json_schema` (grammar-constrained) [conoscenza nota, NV oggi]. **Schema valido ≠ grounding**: modelli 4–8B inventano citazioni plausibili → serve validazione JEV + mapping chunk_id in codice.
- Throughput su 3050 (≈ 8B Q4) [STIMA]: decine di tok/s se tutto in VRAM, crollo con offload CPU → misurare.
- i7-13700K + 32 GB: CPU-only 8B Q4 fattibile ma lento [STIMA].

### 3.2 API
Fonti: claude-api skill (prezzi cache 2026-10-06) · https://platform.claude.com/docs/en/build-with-claude/search-results [P] · https://privacy.claude.com/en/articles/7996866 [P] · https://developers.openai.com/api/docs/guides/your-data [P] · Mistral: aggregatori [S]

Claude (ID esatti, nessun suffisso data):
| Modello | ID | $/Mtok in / out | Note rilevanti |
|---|---|---|---|
| Opus 5.5 | `claude-opus-5-5` | 4.00 / 20.00 | thinking non disattivabile, effort default `medium`; forced `tool_choice` → 400 |
| Sonnet 5.5 | `claude-sonnet-5-5` | 2.00 / 10.00 | thinking off solo via `between_tools`; forced tool_choice → 400 |
| Haiku 5.5 | `claude-haiku-5-5` | 0.10 / 0.50 (≤100K prompt; 0.50/2.50 oltre) | effort default `medium` |
- Structured outputs: `output_config: {format: {...}}` (JSON schema) o tool `strict: true` [P skill].
- Citations native: blocchi `search_result` (`source`, `title`, `content[]`, `citations.enabled`) → citazioni `search_result_location` con `search_result_index`, `start_block_index`, `end_block_index`, `cited_text` (non conta come output). Unità minima = blocco di testo → 1 blocco = 1 chunk/frase. Supportato da tutti i modelli attivi tranne Haiku 3 [P].
- **Vincolo chiave: Citations incompatibile con `output_config.format` (400)** [P skill]. Scelta: (a) citations native + testo libero → mapping `source`=chunk_id in codice; (b) structured output con `claims[{text, chunk_ids[]}]` → citazioni dichiarate dal modello, da validare (JEV `validate_citations`).
- Retention: input/output cancellati "within 30 days of receipt or generation"; flag Usage Policy → fino a 2 anni; ZDR su accordo [P].
- `inference_geo` parametro top-level (`usage.inference_geo` riporta dove) [P skill] → verificare opzione EU.

OpenAI: 30 gg abuse-monitoring default, ZDR solo su approvazione [P]. Prezzi/modelli correnti **NV** (non ricercati per rete).
Mistral: hosting EU default, ZDR su richiesta [S, NV]; prezzi in conflitto (Large 3: 0.50/1.50 vs 2.00/6.00; Small 0.15/0.60) [S].

### 3.3 Costo/query [STIMA] (input ~5k tok: 8 chunk×400 + system 1k; output ~600 + thinking)
| Opzione | $/query | Latenza | Privacy | Output strutturato |
|---|---|---|---|---|
| Locale qwen3:4b / gemma3:4b | 0 (energia) | da misurare | dati on-prem | schema via grammar, qualità grounding bassa |
| Locale 8B Q4 | 0 | rischio offload | on-prem | idem, meglio |
| Haiku 5.5 | ≈ 0.0008 | bassa | 30 gg US/geo | structured o citations |
| Sonnet 5.5 | ≈ 0.016 (+thinking) | media | 30 gg | ottimo |
| Opus 5.5 | ≈ 0.032 (+thinking) | alta | 30 gg | ottimo |

### 3.4 Client Rust
- **Nessun SDK Anthropic Rust ufficiale**; solo crate community non affiliati (`anthropic-ai-sdk` 0.2.27, `crimson-crab`, `claudius`, `turboclaude`, `anthropic_rust`, `claude-sdk` 0.2.x) [S, https://docs.rs/crate/anthropic-ai-sdk].
- Raccomandato: **reqwest raw** dietro `AnswerGenerator` (stesso stack di JEV, tipi provider confinati nell'adapter, coerente con DEPENDENCIES "nessun SDK community"). Ollama: HTTP locale `/api/chat` con `format`, stesso adapter pattern.

---

## 4. Reranker baseline (A-06) — JEV rank LOCKED, baseline = confronto/fallback

Fonti: https://qdrant.tech/documentation/fastembed/fastembed-rerankers/ [P] · https://jina.ai/models/jina-reranker-v2-base-multilingual/ [P] · https://bge-model.com/_sources/bge/bge_reranker_v2.rst.txt [P] · https://futureagi.com/blog/best-rerankers-for-rag-2026/ [S]

| Candidato | Licenza | Multilingue/IT | Runtime Rust | Latenza top-20 [STIMA] |
|---|---|---|---|---|
| no-rerank (ordine vettoriale/ibrido) | — | — | nativo | 0 ms |
| `BAAI/bge-reranker-v2-m3` (~568M, XLM-R) | **Apache-2.0** [P/S] | sì | fastembed-rs (ONNX) [NV enum], `ort`, TEI container | GPU 3050: ~100–300 ms; CPU 13700K: ~1–3 s @512 tok |
| `bge-reranker-base` (~278M) | MIT/Apache [NV] | parziale | fastembed-rs | CPU ~0.3–1 s |
| `jina-reranker-v2-base-multilingual` | **CC-BY-NC-4.0** → non commerciale [P fastembed] | sì | fastembed | escluso per licenza (ok solo portfolio puro) |
| `mxbai-rerank-*-v2` | Apache-2.0 [S, NV card] | sì | ONNX non verificato | [NV] |

Raccomandazione: baseline doppia nell'evaluation → **no-rerank** (floor, zero costo) + **bge-reranker-v2-m3 via TEI su GPU** (o `ort`/fastembed-rs in-process su CPU). Implementato come secondo adapter `DecisionEngine::rank` (o porta `Reranker` dedicata) e fallback su timeout/circuit-open JEV. Vantaggio privacy: locale → nessun chunk esce.
Attenzione VRAM: reranker (~1.1 GB fp16) + embedder + LLM locale 8B non coesistono su 6 GB [STIMA].

---

## 5. Default raccomandati

| Decisione | Default | Motivo |
|---|---|---|
| Modello JEV | **ID versionato `jev-1.13.0`** in config; `jev-latest` solo in dev; loggare `response.model` | soglie calibrate (ADR-0002) si rompono al cambio alias; docs lo raccomandano |
| Strategia rank | **A: 1 request per candidato**, 3–4 Noul per pair, `buffer_unordered(8)` + semaforo globale | pattern ufficiale RAG; isolamento; segnali evidenza gratis |
| validate_citations | 1 request per claim, Choice supports/contradicts/says_nothing | cookbook ufficiale |
| Timeout JEV | `connect_timeout 2s`, per-call `min(remaining, 3s)`, max 1 retry su 429/529/5xx solo se tempo residuo | latenza ufficiale assente; misurare p95 |
| Deadline | deadline unica in `QueryContext`, `timeout_at` ovunque | S-05 |
| HTTP client | reqwest 0.13 + rustls + http2 + zstd (opz.) | SDK ufficiale solo Py/TS |
| LLM default | **API Claude Haiku 5.5** con structured output `claims[{text, chunk_ids}]` + validazione JEV; **adapter locale Ollama** (`qwen3:4b` o `gemma3:4b`) per dev/offline/privacy | costo ≈ $0.0008/q, schema affidabile; locale 6 GB non regge 8B + reranker |
| Upgrade qualità | Sonnet 5.5 come flag di config, misurato su eval | 20× costo |
| Citations | structured output (non citations native) finché si usa `output_config.format` | incompatibilità 400; JEV valida comunque |
| Reranker baseline | no-rerank + bge-reranker-v2-m3 (Apache-2.0) | licenza ok, multilingue, locale |

---

## 6. Domande all'utente (armate)

1. **Dati verso JEV/LLM cloud.** Finding: JEV ospitato in USA, retention "as long as necessary", ZDR solo enterprise; Anthropic 30 gg (ZDR su accordo). Il corpus del portfolio sarà **sintetico/pubblico** (cloud ok) o simula documenti riservati reali (allora LLM locale default e JEV solo su corpus non sensibile)?
2. **Pin versione JEV.** Finding: `jev-latest`→`jev-1.13.0`, `jev-preview`→`jev-1.14.0` (14-10-2026); alias cambia senza preavviso; docs consigliano ID versionato per soglie calibrate. Confermi `jev-1.13.0` pinnato (DEPENDENCIES oggi dice `jev-latest`)?
3. **LLM default.** Finding: 6 GB VRAM → 8B Q4 al limite (qwen3:8b ~5.5 GB, llama3.1:8b 4.9 GB) e non convive con reranker; Haiku 5.5 ≈ $0.0008/query con structured output. Default API Haiku 5.5 + adapter locale 4B, oppure locale-first?
4. **Citazioni.** Finding: Citations native Claude (`search_result_location`) incompatibili con `output_config.format` (400). Preferisci claim strutturati con `chunk_ids` validati da JEV (portabile su locale) o citazioni native (solo Claude, più affidabili, mapping in codice)?

---

## 7. Piano spike di misura

| # | Spike | Misura | Criterio |
|---|---|---|---|
| S1 | JEV rank A vs B su 30 query eval × top-20 | nDCG@5, recall@8, p50/p95 latenza, token, 429 osservati, varianza su 3 ripetizioni | A default se Δ qualità ≥ 0 e p95 ≤ budget |
| S2 | Concorrenza JEV 4/8/16 | p95, tasso 429/529, throughput | scegliere max senza 429 |
| S3 | Alias | `response.model` per `jev-latest`, `jev-1.13.0`, `jev-preview` | confermare risoluzione |
| S4 | Cancellazione | drop a 50/200/500 ms con server mock (wiremock/axum lento) h1 vs h2: connessioni aperte, riuso pool, RST_STREAM, leak task; poi su JEV reale: `usage` addebitato? | nessun leak su 10k cicli |
| S5 | Deadline globale | `buffer_unordered(8)`+`timeout_at` vs `JoinSet`+`Semaphore`: tempo di uscita dopo deadline | uscita < 10 ms dopo deadline |
| S6 | LLM locale | qwen3:4b, gemma3:4b, qwen3:8b, llama3.1:8b, gemma4:e2b su 3050: tok/s, TTFT, VRAM (nvidia-smi), % JSON valido, faithfulness IT (claim supportati da JEV citation check) su 30 domande | ≥ 98% JSON valido, faithfulness misurata vs Haiku |
| S7 | LLM API | Haiku 5.5 vs Sonnet 5.5: stessa eval, costo reale da `usage`, latenza | Haiku default se gap faithfulness < soglia |
| S8 | Reranker | no-rerank vs bge-reranker-v2-m3 (TEI GPU, ort CPU) vs JEV: nDCG@5, p95 top-20 | baseline documentata |
Tutti gli spike: solo corpus sintetico/eval; nessun chunk reale fuori dalla macchina finché Q1 non risolta.

---

**Modifica:** Claude (Anthropic) · 2026-10-18 · ricerca decisionale discuss 1
