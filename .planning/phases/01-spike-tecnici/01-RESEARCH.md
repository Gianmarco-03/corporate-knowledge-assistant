# 01 — Ricerca implementativa (plan 1: Spike tecnici)

Scopo: colmare SOLO i buchi per scrivere piani spike eseguibili. Decisioni = `01-CONTEXT.md` D-01..D-22 (vincolanti). Ricerca decisionale = `01-RICERCA-DISCUSSIONE.md` (non ripetuta qui: si rimanda per §).
Accesso fonti: **2026-10-18** salvo diversa nota. Legenda: **[V]** verificato su fonte primaria oggi · **[S]** secondaria · **[NV]** non verificato, da confermare nello spike · **[I]** inferenza.
Stile caveman. Codice/comandi/versioni verbatim.

---

## 0. Versioni pin (verificate oggi)

### Crate (crates.io API `max_stable_version`, https://crates.io/api/v1/crates/{name}) [V]

| Crate | Versione | Feature per spike |
|---|---|---|
| axum | 0.8.9 | `multipart` |
| tower-http | 0.7.1 | `limit`, `timeout`, `trace` |
| tokio | 1.53.3 | `full` |
| tokio-util | 0.7.20 | `io` |
| object_store | 0.14.2 | `aws` (= aws-base + reqwest/rustls + aws-lc-rs) |
| sqlx | 0.9.0 | `runtime-tokio`, `tls-rustls-ring-webpki` (o `tls-none` in rete compose), `postgres`, `uuid`, `migrate` |
| pgvector | 0.4.2 | `sqlx`, `halfvec` (supporta sqlx `>=0.8,<0.10`, 0.4.2 = "Added support for SQLx 0.9") |
| pdfium-render | 0.9.4 | default (`pdfium_latest`=`pdfium_7881`, `thread_safe`, `image_latest`) |
| reqwest | 0.13.5 | default già = `rustls` + `http2`; aggiungere `json`, `stream`, `zstd` |
| wiremock | 0.6.5 | — |
| dhat | 0.3.3 | — |
| tempfile | 3.27.0 | — |
| sha2 | 0.11.1 | — |
| rlimit | 0.11.0 | — |
| nix | 0.31.3 | `signal`, `process` |
| futures | 0.3.34 | — |
| hdrhistogram | 7.6.0 | — |
| serde_json | 1.0.154 | — |
| clap | 4.6.7 | `derive` |
| anyhow | 1.0.104 | (ok negli spike: codice usa-e-getta) |
| sysinfo | 0.39.6 | — |
| bytes | 1.12.1 | — |
| lopdf / flate2 / zip | lopdf 0.45.0 (da RICERCA §Parser) · flate2, zip **[NV versione]** | prescan bomb |

Fonte feature: `https://crates.io/api/v1/crates/{name}/{ver}` campo `features` [V]: reqwest 0.13.5 `default-tls = ["rustls"]`, default = `default-tls, charset, http2, system-proxy`; object_store 0.14.2 `aws = [aws-base, reqwest, reqwest/rustls, aws-lc-rs]`, default = `fs`; sqlx 0.9.0 feature TLS: `tls-native-tls, tls-none, tls-rustls, tls-rustls-aws-lc-rs, tls-rustls-ring, tls-rustls-ring-native-roots, tls-rustls-ring-webpki`.
Nota: reqwest 0.13 default TLS = rustls → coerente con Proposta "reqwest + Rustls" (DEPENDENCIES §Backend Rust).

### Immagini container [V salvo nota]

| Componente | Tag pin | Fonte |
|---|---|---|
| Postgres+pgvector | `pgvector/pgvector:0.8.7-pg18-trixie` (anche `0.8.7-pg17-trixie`), aggiornati 2026-10-01 | Docker Hub API `hub.docker.com/v2/repositories/pgvector/pgvector/tags` |
| Garage | `dxflrs/garage:v2.4.1` (2026-09-08) | Docker Hub API + https://garagehq.deuxfleurs.fr/documentation/quick-start/ |
| TEI GPU RTX 3050 (sm_86) | `ghcr.io/huggingface/text-embeddings-inference:86-1.9.4` (manifest 200; anche `86-1.9`) | README TEI tabella "Ampere 8.6 (A10, A40, ...) → `86-1.9`"; release v1.9.4 2026-09-15; manifest ghcr verificato |
| TEI CPU (fallback) | `ghcr.io/huggingface/text-embeddings-inference:cpu-1.9` | README TEI |
| Docling GPU | `quay.io/docling-project/docling-serve-cu128:v1.37.0` (release 2026-10-13; immagini CUDA solo tag esplicito, niente `latest`) | README docling-serve §Container Images; quay API tag list |
| Docling CPU | `quay.io/docling-project/docling-serve-cpu:v1.37.0` **[NV tag cpu con prefisso v]** | idem |
| Rust build | `rust:1.99.1-slim-trixie` | Docker Hub API library/rust |
| Runtime worker | `debian:trixie-slim` (glibc: serve ai binari PDFium linux-x64; musl "failing" — RICERCA §Parser) | Docker Hub |
| Verifica GPU | `nvcr.io/nvidia/k8s/cuda-sample:nbody` | https://docs.docker.com/desktop/features/gpu/ |
| Load test | `oha` v1.16.0 (2026-08-23) binario; `grafana/k6:1.8.2` | GitHub releases API; Docker Hub |
| PDFium binari | `bblanchon/pdfium-binaries` release `chromium/8100` (2026-10-12), asset `pdfium-linux-x64.tgz` | GitHub releases API |
| Tesseract | Debian trixie `tesseract-ocr` **5.5.0-1**; upstream ultimo 5.5.3 (2026-07-24); Alpine edge 5.5.3-r0, v3.24 5.5.2-r0 | sources.debian.org API; GitHub releases; pkgs.alpinelinux.org |
| Docling (lib) | `docling` 2.140.0 PyPI | pypi.org/pypi/docling/json |
| PaddleOCR | `paddleocr` 3.7.0 (Python ≥3.8), `paddlepaddle` 3.3.1 | PyPI JSON API |

**Deviazione da segnalare al planner:** D-09 cita "Tesseract 5.5.3" (build Windows UB Mannheim). In container Debian trixie = 5.5.0 (bugfix-only di distanza). Opzioni: (a) usare 5.5.0 apt e registrarlo nel manifest [raccomandato, costo zero]; (b) build da sorgente 5.5.3 (+ leptonica) nel Dockerfile [costo build]; (c) container OCR separato Alpine edge 5.5.3 [glibc≠musl irrilevante: Tesseract è CLI]. Decide plan; RISULTATI registra versione effettiva (`tesseract --version`).
**Deviazione 2:** PaddleOCR 3.7 default = **PP-OCRv6_medium** ("+5.1% recognition … over PP-OCRv5_server", 46 lingue latine) — https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/docs/version3.x/pipeline_usage/OCR.en.md §intro [V]. D-09 nomina PP-OCRv5 come sfidante: misurare v5 (`--ocr_version PP-OCRv5`) e, a costo marginale, v6 default. Segnalare.
**Deviazione 3:** pdfium-render 0.9.4 binding max `pdfium_7881`; binario più recente `chromium/8100` (fix CVE 2026, RICERCA §Isolamento). API PDFium additive → binario nuovo con feature `pdfium_7881` dovrebbe funzionare **[NV]**: spike prova 8100, fallback release `chromium/7881`.

---

## 1. Layout harness spike (S-07 escluso: firme in §9)

### Vincoli da repo [V]
- Hook `.githooks/controlli-comuni.sh`: passi cargo `NON APPLICABILE` finché manca `Cargo.toml` **alla radice**; con manifest presente girano `cargo fmt --all`, `clippy --workspace`, `test --workspace` → un `Cargo.toml` in radice trasformerebbe gli spike in "prodotto" e i gate li giudicherebbero. **Vietato** (01-CONTEXT §code_context).
- `.github/workflows/` vuota → oggi nessuna CI. Regola: nessun workflow futuro deve includere `spikes/**` (Fase 2 Fondazione: root `Cargo.toml` con `[workspace] exclude = ["spikes"]` per cintura+bretelle [I]).
- Hook pre-commit lancia `skills/security-scanner/security_scan.py` sul contenuto staged → **scansiona anche `spikes/`**. Regole rilevanti: `http-url` HIGH su literal `"http://<host instradabile>"` (eccetto 127.0.0.1/localhost/[::1]/0.0.0.0), regole segreti (`hardcoded-password`, `env-secret-literal`, `connstring-inline-pwd`…), TLS disabilitato. Marker `.security-scan-fixture` esclude una dir (riportata a video). Fonte: `skills/security-scanner/security_scan.py` righe 156-166, 303-312.
  - Conseguenza: endpoint compose (`http://garage:3900`, `http://tei-embed:80`) **solo da env var**, mai literal nel codice Rust/YAML; password Postgres/Garage via `${VAR}` da `spikes/.env` (ignorato). Marker fixture SOLO su `spikes/data/hostile/` (file ostili, non codice) — dichiararlo nel piano, mai su dir di codice.
- `.gitignore` radice già ignora `target/` ma non dati spike; modificarlo è fuori perimetro (perimetro = `spikes/**`, ADR, DEPENDENCIES, threat-model) → usare **`spikes/.gitignore`** proprio.
- `crate-boundary-check` cerca `Cargo.toml` in radice (`skills/crate-boundary-check/boundary_check.py` r.220) → spike invisibili al gate. OK.

### Struttura raccomandata
Workspace Cargo **dentro** `spikes/` (un lockfile, un `target/` condiviso, build più rapide), crate per spike:

```
spikes/
  Cargo.toml              # [workspace] resolver="3", members=[...]; NON in radice repo
  Cargo.lock
  .gitignore              # target/, data/raw/**, results/**/raw/, .env
  .env.example            # chiavi senza valori: POSTGRES_PASSWORD=, GARAGE_*=, TYPESAFE_API_KEY=, ANTHROPIC_API_KEY=, S3_ENDPOINT=, TEI_EMBED_URL=, ...
  compose.yaml            # postgres, garage, tei-embed, tei-rerank, docling, worker (profili)
  docker/
    worker.Dockerfile     # rust build → debian:trixie-slim + libpdfium + tesseract + tessdata_best
    garage.toml           # rpc_secret/admin_token da env, non literal
    paddle.Dockerfile     # python:3.12-slim + paddleocr (solo sfidante OCR)
  tools/
    hw-manifest.sh        # §8.2
    gpu-check.ps1         # §6
  common/                 # crate lib: CSV writer, timer, hdrhistogram→CSV, manifest, env
  s01-upload/             # S-01 + A-02 (bin: server, bin: bench)
  s02-parser/             # S-02/A-05/A-10 (bin: parse-parent, parse-child, docling-client, ocr)
  s03-pgvector/           # S-03/A-03 (bin: load, query-bench, ingest-under-load)
  s04-jev/                # S-04/A-09 + A-06 reranker
  s05-cancel/             # S-05 deadline/cancellazione
  s06-llm/                # A-04 Haiku/Sonnet structured output
  s07-ports/              # S-07: solo src/lib.rs con trait candidati (compila, nessuna impl)
  data/                   # manifest corpus (versionato) + raw/ ignorato; hostile/ con marker fixture
  <ogni spike>/RISULTATI.md, <ogni spike>/results/*.csv|json
```
- Nessun crate di prodotto dipende da `spikes/*` (nessun path dep verso `../spikes`). Verifica meccanica nel piano: `grep -r "spikes" --include=Cargo.toml services crates apps` vuoto.
- Python (Docling client, PaddleOCR) solo dentro container; nessun `pyproject` in radice.
- Alternativa scartata: un Cargo project per spike senza workspace → N lockfile, N `target/` (GB ciascuno), nessun guadagno di isolamento.

`spikes/Cargo.toml` minimo:
```toml
[workspace]
resolver = "3"
members = ["common", "s01-upload", "s02-parser", "s03-pgvector", "s04-jev", "s05-cancel", "s06-llm", "s07-ports"]

[workspace.dependencies]
axum = { version = "=0.8.9", features = ["multipart"] }
tower-http = { version = "=0.7.1", features = ["limit", "timeout", "trace"] }
tokio = { version = "=1.53.3", features = ["full"] }
object_store = { version = "=0.14.2", features = ["aws"] }
sqlx = { version = "=0.9.0", default-features = false, features = ["runtime-tokio", "tls-none", "postgres", "uuid", "migrate", "macros"] }
pgvector = { version = "=0.4.2", features = ["sqlx", "halfvec"] }
pdfium-render = "=0.9.4"
reqwest = { version = "=0.13.5", features = ["json", "stream", "zstd"] }
wiremock = "=0.6.5"
dhat = "=0.3.3"
```
(`resolver = "3"` richiede edition 2024 / Rust ≥1.84 [I]; ok con rust 1.99.)

---

## 2. Compose condiviso `spikes/compose.yaml`

Profili per non avviare tutto insieme (VRAM 6 GB condivisa: TEI embed + TEI rerank + Docling GPU vanno misurati in convivenza, D-13/D-21, Claude's Discretion "budget VRAM").

```yaml
name: cka-spikes
services:
  postgres:
    image: pgvector/pgvector:0.8.7-pg18-trixie
    environment:
      POSTGRES_USER: ${POSTGRES_USER}
      POSTGRES_PASSWORD: ${POSTGRES_PASSWORD}
      POSTGRES_DB: spikes
    ports: ["127.0.0.1:55432:5432"]
    volumes: [pgdata:/var/lib/postgresql]     # [NV] PG18 image: path dati cambiato rispetto a /var/lib/postgresql/data
    shm_size: 1g
    command: ["postgres", "-c", "shared_buffers=2GB", "-c", "max_parallel_workers_per_gather=4", "-c", "track_io_timing=on"]
    profiles: [db, s01, s03]

  garage:
    image: dxflrs/garage:v2.4.1
    volumes:
      - ./docker/garage.toml:/etc/garage.toml:ro
      - garage-meta:/var/lib/garage/meta
      - garage-data:/var/lib/garage/data
    environment:
      GARAGE_RPC_SECRET: ${GARAGE_RPC_SECRET}            # [NV] override env per rpc_secret
      GARAGE_DEFAULT_ACCESS_KEY: ${GARAGE_ACCESS_KEY}
      GARAGE_DEFAULT_SECRET_KEY: ${GARAGE_SECRET_KEY}
      GARAGE_DEFAULT_BUCKET: cka-originals
    command: ["/garage", "server", "--single-node", "--default-bucket"]
    ports: ["127.0.0.1:3900:3900"]
    profiles: [s01]

  tei-embed:
    image: ghcr.io/huggingface/text-embeddings-inference:86-1.9.4
    command: ["--model-id", "${EMBED_MODEL:-Qwen/Qwen3-Embedding-0.6B}", "--dtype", "float16", "--max-batch-tokens", "${TEI_MAX_BATCH_TOKENS:-16384}", "--max-client-batch-size", "64"]
    volumes: [hf-cache:/data]
    ports: ["127.0.0.1:8081:80"]
    deploy:
      resources:
        reservations:
          devices: [{ driver: nvidia, count: 1, capabilities: [gpu] }]
    profiles: [s03, gpu]

  tei-rerank:
    image: ghcr.io/huggingface/text-embeddings-inference:86-1.9.4
    command: ["--model-id", "BAAI/bge-reranker-v2-m3", "--dtype", "float16", "--max-batch-tokens", "8192"]
    volumes: [hf-cache:/data]
    ports: ["127.0.0.1:8082:80"]
    deploy:
      resources:
        reservations:
          devices: [{ driver: nvidia, count: 1, capabilities: [gpu] }]
    profiles: [s04, gpu]

  docling:
    image: quay.io/docling-project/docling-serve-cu128:v1.37.0
    environment:
      DOCLING_DEVICE: cuda
      DOCLING_NUM_THREADS: "8"
      DOCLING_SERVE_ENG_LOC_NUM_WORKERS: "1"
      DOCLING_SERVE_MAX_NUM_PAGES: "400"
      DOCLING_SERVE_MAX_FILE_SIZE: "26214400"
    ports: ["127.0.0.1:5001:5001"]
    networks: [parse]              # rete internal: niente egress
    deploy:
      resources:
        limits: { memory: 8g }
        reservations:
          devices: [{ driver: nvidia, count: 1, capabilities: [gpu] }]
    profiles: [s02, gpu]

  worker:
    build: { context: ., dockerfile: docker/worker.Dockerfile }
    network_mode: none            # parse-child: nessuna rete (D-08)
    read_only: true
    tmpfs: ["/work:size=2g"]
    mem_limit: 2g
    memswap_limit: 2g             # = mem_limit → niente swap
    cpus: 8
    pids_limit: 256
    cap_drop: [ALL]
    security_opt: ["no-new-privileges:true"]
    user: "10001:10001"
    volumes: ["./data:/data:ro", "./s02-parser/results:/results"]
    profiles: [s02]

networks:
  parse: { internal: true }
volumes: { pgdata: {}, garage-meta: {}, garage-data: {}, hf-cache: {} }
```
Fonti: Compose GPU `deploy.resources.reservations.devices` con `driver`, `count`, `capabilities: [gpu]` obbligatorio — https://raw.githubusercontent.com/docker/docs/main/content/manuals/compose/how-tos/gpu-support.md [V]; `--memory`/`--memory-swap` uguali = no swap — https://docs.docker.com/engine/containers/resource_constraints/ (RICERCA §Isolamento) [V]; flag TEI `--dtype {float16,float32}`, `--max-batch-tokens` default 16384, `--max-client-batch-size` default 32, `--max-concurrent-requests` default 512 — README TEI [V].
Note:
- Porte solo su `127.0.0.1` (scanner `http-url` tollera localhost; superficie minima).
- Docling con rete `internal` non scarica modelli: immagine base "with all packages", slim "coming soon" → pesi inclusi [S README]; se un modello manca, primo avvio su rete normale poi `internal` **[NV]**.
- Docling è sidecar HTTP: lo spike S-02 lo misura anche come confine di isolamento (crash Docling ≠ crash worker).
- Volume `hf-cache` condiviso fra TEI: modelli scaricati una volta.

### Garage — config e bootstrap
`spikes/docker/garage.toml` (chiavi da quick-start ufficiale [V]):
```toml
metadata_dir = "/var/lib/garage/meta"
data_dir = "/var/lib/garage/data"
db_engine = "sqlite"
replication_factor = 1
rpc_bind_addr = "[::]:3901"
rpc_public_addr = "127.0.0.1:3901"
# rpc_secret: da env GARAGE_RPC_SECRET (openssl rand -hex 32), mai literal nel repo [NV nome env]

[s3_api]
s3_region = "garage"
api_bind_addr = "[::]:3900"
root_domain = ".s3.garage.localhost"

[admin]
api_bind_addr = "[::]:3903"
```
Bootstrap:
- Rapido (v2.x quick-start [V]): `/garage server --single-node --default-bucket` + env `GARAGE_DEFAULT_ACCESS_KEY`, `GARAGE_DEFAULT_SECRET_KEY`, `GARAGE_DEFAULT_BUCKET` → layout, chiave e bucket automatici.
- Manuale (se flag single-node non basta) [V comandi quick-start]:
```sh
docker compose exec garage /garage status                     # prende <node_id>
docker compose exec garage /garage layout assign -z dc1 -c 10G <node_id>
docker compose exec garage /garage layout apply --version 1
docker compose exec garage /garage key create cka-spike-key   # stampa Key ID + Secret
docker compose exec garage /garage bucket create cka-originals
docker compose exec garage /garage bucket allow --read --write --owner cka-originals --key cka-spike-key
```
- Regione `garage`, porta S3 3900. Path-style vs vhost: pagina non esplicita; `root_domain` serve al vhost; path-style funziona su endpoint IP/host [S/NV] → client in path-style (default object_store).

---

## 3. S-01 + A-02: upload streaming e storage

### Handler axum 0.8.9 (forma)
Basi verificate in RICERCA §1 (F1.1–F1.7: `field.chunk()`, `DefaultBodyLimit` locale, `RequestBodyLimitLayer` globale). Snippet di riferimento per il piano:
```rust
use axum::{Router, routing::post, extract::{Multipart, DefaultBodyLimit}, http::StatusCode};
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tower_http::limit::RequestBodyLimitLayer;

const MAX_FILE: usize = 25 * 1024 * 1024;           // D-04
const MP_OVERHEAD: usize = 64 * 1024;                // boundary + header campi

async fn upload(mut mp: Multipart) -> Result<String, StatusCode> {
    let mut field = mp.next_field().await.map_err(|e| e.status())?   // MultipartError::status() → 413 su limite [NV metodo]
        .ok_or(StatusCode::BAD_REQUEST)?;
    if field.name() != Some("file") { return Err(StatusCode::BAD_REQUEST); }
    let tmp = tempfile::NamedTempFile::new_in("/work/tmp").map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (std_file, tmp_path) = tmp.into_parts();
    let mut out = tokio::fs::File::from_std(std_file);
    let (mut hasher, mut n, mut head) = (Sha256::new(), 0usize, Vec::with_capacity(8192));
    while let Some(chunk) = field.chunk().await.map_err(|e| e.status())? {
        n += chunk.len();
        if n > MAX_FILE { return Err(StatusCode::PAYLOAD_TOO_LARGE); }   // tmp_path drop → file rimosso
        hasher.update(&chunk);
        if head.len() < 8192 { head.extend_from_slice(&chunk[..chunk.len().min(8192 - head.len())]); }
        out.write_all(&chunk).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    out.flush().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    if mp.next_field().await.map_err(|e| e.status())?.is_some() { return Err(StatusCode::BAD_REQUEST); } // 1 solo campo
    let sha = hex::encode(hasher.finalize());
    // sniff(head) → validate(tmp_path) → store.put(sha) → tx metadati+outbox (D-02/D-03)
    Ok(sha)
}

let app = Router::new()
    .route("/upload", post(upload).layer(DefaultBodyLimit::max(MAX_FILE + MP_OVERHEAD)))
    .layer(RequestBodyLimitLayer::new(MAX_FILE + MP_OVERHEAD));
```
- Tempdir su tmpfs/volume Linux del container, mai bind NTFS (D-03).
- Misura i 3 adapter (D-06) dietro un trait locale allo spike `BlobSink { async fn put(&self, key, path) }`: `GarageSink`, `PgByteaSink`, `NullSink`.
- Risposta identica dedup sì/no (D-02): misurare anche **tempo** risposta dedup vs nuovo (side-channel temporale) → colonna `dedup_hit` nel CSV, confronto distribuzioni [I: requisito D-02 esteso al timing].

### object_store → Garage
```rust
use object_store::{aws::AmazonS3Builder, path::Path, ObjectStore, buffered::BufWriter};
use std::sync::Arc;
let s3: Arc<dyn ObjectStore> = Arc::new(AmazonS3Builder::new()
    .with_endpoint(std::env::var("S3_ENDPOINT")?)     // es. http://garage:3900 via env
    .with_allow_http(true)                            // solo rete compose dello spike
    .with_region("garage")
    .with_bucket_name("cka-originals")
    .with_access_key_id(std::env::var("GARAGE_ACCESS_KEY")?)
    .with_secret_access_key(std::env::var("GARAGE_SECRET_KEY")?)
    .build()?);
let key = Path::from(format!("sha256/{sha}"));
let mut w = BufWriter::new(s3.clone(), key);          // AsyncWrite: single PUT o multipart automatico
let mut f = tokio::fs::File::open(&tmp_path).await?;
tokio::io::copy(&mut f, &mut w).await?;
tokio::io::AsyncWriteExt::shutdown(&mut w).await?;    // completa upload
```
Fonte [V] docs.rs `AmazonS3Builder` 0.14.2: `with_virtual_hosted_style_request` default **false = path style**; `with_allow_http` default false ("Only HTTPS"); `with_region` default `us-east-1`; `with_conditional_put` default `S3ConditionalPut::ETagMatch`; `from_env()` legge `AWS_ACCESS_KEY_ID`, `AWS_SECRET_ACCESS_KEY`, `AWS_DEFAULT_REGION`, `AWS_ENDPOINT`. `BufWriter`/`WriteMultipart` da RICERCA §2 F2.8.
[NV] In object_store ≥0.13 alcuni metodi di comodo (`put`, `get`) potrebbero stare in un trait estensione → seguire errori del compilatore.
[NV] Supporto Garage a `If-None-Match` (put condizionale): non necessario — chiave = sha256 → overwrite idempotente.
GC orfani (D-02): spike misura solo `list` prefisso `sha256/` + `head` per `last_modified`; N ore = Claude's Discretion nel plan.

### Baseline PG `bytea` (D-06)
```sql
CREATE TABLE blob_chunk (object_key text NOT NULL, seq int NOT NULL, data bytea NOT NULL, PRIMARY KEY (object_key, seq));
ALTER TABLE blob_chunk ALTER COLUMN data SET STORAGE EXTERNAL;  -- niente compressione TOAST su PDF già compressi [I]
```
Insert a chunk da 1 MiB letti dal tempfile, una tx per oggetto; misurare tempo, WAL generato (`pg_current_wal_lsn()` prima/dopo → `pg_wal_lsn_diff`), dimensione tabella (`pg_total_relation_size`).

### Misura memoria (criterio S-01 "picco costante rispetto alla dimensione")
Tre sorgenti, tutte nel CSV:
1. `dhat` in test integrazione (RICERCA F1.7): `let _p = dhat::Profiler::builder().testing().build();` … `dhat::HeapStats::get().max_bytes` → asserzione `max_bytes(25MB) - max_bytes(1MB) < 2 MiB` [I soglia]. Un test per file in `tests/` (Profiler globale). Gira **in container Linux** (dhat "much worse on Windows").
2. RSS di picco processo: `/proc/self/status` riga `VmHWM` letta a fine run (Linux) [I standard procfs].
3. cgroup container: `cat /sys/fs/cgroup/memory.peak` (cgroup v2; kernel WSL2 6.x) **[NV disponibilità memory.peak su Docker Desktop]**, fallback `docker stats --no-stream --format '{{.MemUsage}}'` campionato.
File test: `printf '%%PDF-1.7\n' > f; head -c $((S)) /dev/urandom >> f` per S = 1, 10, 24, 25 MiB, + 25 MiB+1 (413 atteso), + 100 MiB (413 senza leggere tutto: misurare byte letti prima del rifiuto).

### Carico
```sh
oha -n 200 -c 8 --no-tui --output-format json -m POST -F 'file=@/data/up/10m.pdf' http://127.0.0.1:8080/upload > s01-upload/results/oha_10m_c8.json
oha -z 60s -c 16 --no-tui --output-format csv -m POST -F 'file=@/data/up/25m.pdf' http://127.0.0.1:8080/upload > s01-upload/results/oha_25m_c16.csv
```
Flag [V] README oha: `-n`, `-c`, `-z`, `-F, --form` (multipart curl-compatibile), `--no-tui`, `--output-format json|csv`, `--latency-correction`, `--http2`. k6 (`grafana/k6:1.8.2`) solo se serve scenario a rampa; oha basta.
Metriche CSV S-01: `adapter,size_bytes,concurrency,req,p50_ms,p95_ms,p99_ms,rss_hwm_kb,cgroup_peak_bytes,dhat_max_bytes,status_counts,dedup_hit`.
Target README: upload acknowledgment p95 ≤ 500 ms [V README §TTQ].

---

## 4. S-02 / A-05 / A-10: parser, OCR, isolamento

### PDFium in container
`docker/worker.Dockerfile` (forma):
```dockerfile
FROM rust:1.99.1-slim-trixie AS build
WORKDIR /src
COPY . .
RUN cargo build --release -p s02-parser

FROM debian:trixie-slim
ARG PDFIUM_TAG=chromium%2F8100
ADD https://github.com/bblanchon/pdfium-binaries/releases/download/${PDFIUM_TAG}/pdfium-linux-x64.tgz /tmp/pdfium.tgz
RUN mkdir -p /opt/pdfium && tar -xzf /tmp/pdfium.tgz -C /opt/pdfium && rm /tmp/pdfium.tgz \
 && apt-get update && apt-get install -y --no-install-recommends tesseract-ocr ca-certificates && rm -rf /var/lib/apt/lists/*
ADD https://github.com/tesseract-ocr/tessdata_best/raw/main/ita.traineddata /opt/tessdata_best/
ADD https://github.com/tesseract-ocr/tessdata_best/raw/main/eng.traineddata /opt/tessdata_best/
ADD https://github.com/tesseract-ocr/tessdata_best/raw/main/osd.traineddata /opt/tessdata_best/
COPY --from=build /src/target/release/parse-parent /src/target/release/parse-child /usr/local/bin/
ENV PDFIUM_DIR=/opt/pdfium/lib
```
- Asset `pdfium-linux-x64.tgz` presente in `chromium/8100` e `chromium/8086` [V GitHub API]. Usare build **non-V8** (niente JS engine: superficie minore; anche RLIMIT_AS meno problematico) [I].
- URL tessdata_best branch `main`, HTTP 200 per `ita`, `eng`, `osd` [V]. Licenza Apache-2.0 (RICERCA §OCR).
- Pin checksum: plan registra `sha256sum` dei tre traineddata e del tgz nel manifest (ADD non verifica) [I].

Binding + estrazione per carattere (API [V] docs.rs pdfium-render 0.9.4 + README):
```rust
use pdfium_render::prelude::*;
let pdfium = Pdfium::new(Pdfium::bind_to_library(
    Pdfium::pdfium_platform_library_name_at_path(std::env::var("PDFIUM_DIR")?))?);
let doc = pdfium.load_pdf_from_file(&path, None)?;   // PDF cifrato → Err(PdfiumError::…) [NV variante esatta: password error]
for (i, page) in doc.pages().iter().enumerate() {
    let (w, h) = (page.width().value, page.height().value);   // PdfPoints
    let text = page.text()?;
    for ch in text.chars().iter() {
        let c = ch.unicode_char();                  // Option<char>
        let r = ch.loose_bounds()?;                 // PdfRect, origine bottom-left, punti
        // emit {page: i+1, char, left, bottom, right, top}; tight_bounds() alternativa
    }
    for seg in text.segments().iter() { let b = seg.bounds(); let t = seg.text(); /* run stessa baseline+font */ }
}
```
Metodi [V]: `PdfPage::{text, width, height, page_size, boundaries}`, `PdfPageText::{all, chars, segments, search, len}`, `PdfPageTextChar::{unicode_char, unicode_string, loose_bounds, tight_bounds, origin, scaled_font_size, font_name}`. Rotazione/CropBox: `boundaries()` + rotazione pagina → normalizzazione per PDF.js (RICERCA M7) **[NV nome metodo rotazione]**.
Render per OCR 300 DPI: `page.render_with_config(&PdfRenderConfig::new().scale_page_by_factor(300.0 / 72.0))?.as_image()` (feature `image`) **[NV firma esatta]**; bbox OCR px → pt `pt = px × 72 / 300` (RICERCA §OCR).
Mutex globale `thread_safe` → parallelismo solo multi-processo (RICERCA §Isolamento) → misura M9: 1 processo vs N processi figli.

### Tesseract CLI (TSV con bbox per parola)
```sh
OMP_THREAD_LIMIT=1 tesseract page-0007.png stdout --tessdata-dir /opt/tessdata_best -l ita+eng --oem 1 --psm 3 --dpi 300 -c preserve_interword_spaces=1 tsv
```
- Flag `-l ita+eng`, `--psm`, `-c preserve_interword_spaces=1`, configfile `tsv` [V tessdoc Command-Line-Usage, RICERCA §OCR]; `--oem 1` = solo LSTM (tessdata_best è solo LSTM) [V tessdata_best README]; `--tessdata-dir`, `--dpi` **[NV ripresa oggi, noti da tessdoc]**.
- `OMP_THREAD_LIMIT=1` quando si lanciano N processi in parallelo (evita oversubscription OpenMP) **[NV/S consiglio tessdoc FAQ]**.
- TSV colonne: `level page_num block_num par_num line_num word_num left top width height conf text` (level 5 = parola) **[NV ripresa oggi]**. Origine top-left px → flip Y per PDF space.
- Misura: 1/4/8/16 processi paralleli su 25 pp; CER vs oracolo manuale (RICERCA M6).

### PP-OCRv5 sfidante (percorso minimo)
`docker/paddle.Dockerfile`: `python:3.12-slim` + `pip install paddlepaddle==3.3.1 paddleocr==3.7.0` (CPU) [V versioni PyPI]. GPU: `paddlepaddle-gpu` da indice Paddle **[NV comando]** — opzionale; VRAM già contesa.
```sh
paddleocr ocr -i /data/scan/page-0007.png --lang it --ocr_version PP-OCRv5 \
  --use_doc_orientation_classify False --use_doc_unwarping False --use_textline_orientation False \
  --device cpu --save_path /results/paddle/
```
- CLI e flag [V] doc OCR pipeline (`paddleocr ocr -i … --use_doc_orientation_classify False --use_doc_unwarping False --use_textline_orientation False --save_path ./output --device gpu:0`; `--ocr_version PP-OCRv4` esempio); lingua `it` = Italian in tabella appendice [V]; output JSON con `rec_texts`, `rec_scores`, `rec_boxes`, `rec_polys` [V esempio output].
- Variante senza flag `--ocr_version` = PP-OCRv6_medium (default 3.7) → seconda riga di misura (Deviazione 2).

### Docling solo pagine con tabelle (D-07)
Endpoint [V docs/usage.md docling-serve]: `POST /v1/convert/file` multipart; `POST /v1/convert/source` JSON. Parametri utili [V]: `to_formats` (`json`, `md`, `chunks`, …), `do_ocr` (default true), `force_ocr`, `pdf_backend` (`pypdfium2`, `docling_parse`, `dlparse_v2`, `dlparse_v4`; default `docling_parse`), `table_mode` (`fast`|`accurate`, default accurate), `do_table_structure` (default true), **`page_range` (tuple, pagine da 1)**, `document_timeout` (s), `ocr_lang` (BCP-47).
```sh
curl -sS -X POST http://127.0.0.1:5001/v1/convert/file \
  -H 'accept: application/json' \
  -F 'files=@/data/pdf/relazione-2025.pdf;type=application/pdf' \
  -F 'to_formats=json' -F 'do_ocr=false' -F 'table_mode=accurate' -F 'do_table_structure=true' \
  -F 'page_range=41' -F 'page_range=47' -F 'document_timeout=120' \
  -o s02-parser/results/docling/relazione-2025_p41-47.json -w '%{time_total}\n'
```
- Encoding form di `page_range` (due campi ripetuti) **[NV]**: verificare su `http://127.0.0.1:5001/docs` (OpenAPI del servizio); fallback `/v1/convert/source` JSON con `"page_range": [41, 47]`.
- Env server [V docs/configuration.md]: `DOCLING_DEVICE` (`cpu|cuda|cuda:N`), `DOCLING_NUM_THREADS` (default 4), `DOCLING_SERVE_ENG_LOC_NUM_WORKERS` (default 2), `DOCLING_SERVE_MAX_NUM_PAGES`, `DOCLING_SERVE_MAX_FILE_SIZE`, `DOCLING_SERVE_MAX_DOCUMENT_TIMEOUT`.
- CPU vs GPU: stesso comando contro due istanze (`docling-serve-cpu`, `docling-serve-cu128`), misurare s/pagina tabella e VRAM (`nvidia-smi --query-gpu=memory.used --format=csv -l 1`).
- Output JSON = DoclingDocument: tabelle con `prov[].page_no`, `prov[].bbox` (`l,t,r,b`, `coord_origin`), celle in `data.table_cells` **[NV nomi campi esatti: leggere un output reale]**.
- Selezione pagine-tabella (input a Docling): euristica PDFium (densità numeri + allineamenti colonna) vs elenco manuale nell'oracolo → misurare recall dell'euristica [I].

### Isolamento processo (D-08) — approccio pratico per lo spike
Modello: `parse-parent` (Rust, tokio) lancia un `parse-child` per documento; I/O via file nel tmpfs `/work/<job>/` + JSON su stdout (cap).
```rust
use tokio::process::Command;
use std::{process::Stdio, time::Duration};
let mut cmd = Command::new("/usr/local/bin/parse-child");
cmd.arg("--input").arg(&input).arg("--out").arg(&out_dir)
   .env_clear().env("PDFIUM_DIR", "/opt/pdfium/lib")
   .current_dir(&job_dir)
   .stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped())
   .kill_on_drop(true)
   .process_group(0);                     // pgid = pid figlio → kill di tutto il gruppo
unsafe {
    cmd.pre_exec(|| {                     // solo syscall async-signal-safe: niente alloc
        use rlimit::{setrlimit, Resource};
        setrlimit(Resource::AS, 1 << 30, 1 << 30)?;       // 1 GiB spazio indirizzi (calibrare)
        setrlimit(Resource::CPU, 60, 60)?;                // s CPU → SIGXCPU/SIGKILL
        setrlimit(Resource::FSIZE, 256 << 20, 256 << 20)?;// file scritti ≤256 MiB
        setrlimit(Resource::NOFILE, 64, 64)?;
        setrlimit(Resource::CORE, 0, 0)?;
        Ok(())
    });
}
let child = cmd.spawn()?;
let pid = child.id().expect("pid") as i32;
match tokio::time::timeout(Duration::from_secs(30), child.wait_with_output()).await {
    Ok(Ok(out)) => classify(out.status),  // ExitStatusExt::signal(): 9 KILL(OOM/timeout), 11 SEGV, 6 ABRT, 24 XCPU
    Ok(Err(e)) => io_err(e),
    Err(_) => { let _ = nix::sys::signal::killpg(nix::unistd::Pid::from_raw(pid), nix::sys::signal::Signal::SIGKILL); /* future droppato → kill_on_drop; reaping best-effort tokio */ }
}
```
Fonti [V] docs.rs tokio 1.53.3 `Command`: `kill_on_drop` (default false; caveat zombie: tokio reap "best-effort"), `unsafe fn pre_exec` (Unix, eseguita nel figlio dopo fork, ambiente vincolato: niente malloc/lock), `process_group(pgroup)` (0 = pid come pgid).
- `wait_with_output()` consuma `child` → per kill dopo timeout servono pid salvato prima + `killpg`; alternativa: `child.wait()` in `select!` con lettura stdout cap (`take(1 MiB)`) [I].
- RLIMIT_AS: conta spazio virtuale (mmap riservati, allocatori) → falsi OOM possibili; misurare baseline VmPeak su PDF benigno, poi limite = 2–3× [I]. Alternativa RLIMIT_DATA **[NV comportamento su mmap anonimi]**.
- cgroup v2: `mem_limit`/`memswap_limit` del container = rete di sicurezza **per tutto il worker** (OOM killer sceglie il processo più grande, di norma il figlio, ma non garantito) → cgroup per-figlio richiede delega cgroup nel container, non disponibile di default su Docker Desktop **[NV]** → per lo spike: rlimit per figlio + cgroup container come backstop; registrare quale processo muore (`dmesg` non accessibile su Docker Desktop: dedurre da exit signal) [I].
- Container worker: `network_mode: none`, `read_only`, `tmpfs /work`, `cap_drop: [ALL]`, `no-new-privileges`, utente non-root, `pids_limit` (§2).
- Windows Job Objects (D-08 cita "Job Object su Windows"): **fuori dallo spike** se tutto gira in container Linux; registrarlo come nota nell'ADR (dev su Windows usa Docker) [I] — segnalare al planner.

Tetto byte decompressi (PDF FlateDecode):
- PDFium: nessuna API pubblica nota per limitare l'output Flate **[NV]** → prescan nel **figlio** prima di PDFium (anche il prescan è input ostile):
  1. `lopdf::Document::load_mem` (stream non decompressi al load **[NV]**; ObjStm compressi sì → rischio: anche il load nel figlio sotto rlimit).
  2. per ogni `Object::Stream` con `/Filter /FlateDecode`: `flate2::read::ZlibDecoder::new(&s.content[..]).take(cap + 1)` → `io::copy(&mut r, &mut io::sink())` conta byte senza tenerli in RAM.
  3. rifiuto se `somma > 512 MiB` o `stream > 64 MiB` o `ratio > 100× con output > 16 MiB` → `failed: decompression_limit` (soglie [I], calibrare su corpus benigno: misurare max ratio reale).
- DOCX (anti zip-bomb, D-05): crate `zip` central directory: n. entry ≤ 1000, somma `size()` dichiarata ≤ 200 MiB, poi lettura streaming con `take()` per non fidarsi del dichiarato; rifiuto `vbaProject.bin` / `.docm` (RICERCA §1 pipeline) — soglie [I].
- Rete di sicurezza indipendente dal prescan: RLIMIT_AS + timeout + cgroup → criterio M8 "100% contenuti" vale anche se il prescan manca un caso.
File ostili: elenco RICERCA §"Spike: cosa MISURARE" (cifrato user/owner pwd, troncato, Flate bomb ~1 MB→≥1 GB, 10k pagine, nesting profondo, JBIG2 malformato, DOCX zip bomb, macro, magic mismatch). Generazione bomb Flate [I]: script Python in container `zlib.compress(b"\0" * (1<<30), 9)` inserito come stream in PDF minimale scritto a mano (offset xref calcolati) — generatore versionato in `spikes/data/hostile/gen/`, file generati ignorati.

---

## 5. S-03 / A-03: pgvector + embedding

### Schema minimo spike
```sql
CREATE EXTENSION IF NOT EXISTS vector;
CREATE TABLE doc (id uuid PRIMARY KEY, classification text NOT NULL);           -- public/internal/confidential/restricted (D-16 sintetico)
CREATE TABLE doc_acl (doc_id uuid REFERENCES doc(id), role text NOT NULL, PRIMARY KEY (doc_id, role));
CREATE TABLE chunk (
  id uuid PRIMARY KEY, doc_id uuid NOT NULL REFERENCES doc(id), page int, lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL,
  emb_h halfvec(1024)                                                           -- variante misurata
);
CREATE INDEX ON chunk USING gin (tsv);
CREATE INDEX ON doc_acl (role, doc_id);
```
(`to_tsvector(regconfig, text)` immutabile → ammesso in colonna generata [I, standard PG]; config `italian`/`english` built-in — RICERCA F8.)
Dim: Qwen3-Embedding-0.6B 1024, bge-m3 1024, multilingual-e5-large-instruct 1024 (RICERCA F10 / D-14) → stessa colonna, una tabella per modello o colonna per modello.

### Ricerca esatta con predicato ACL nella stessa query (D-12)
```sql
-- $1 vector query, $2 text[] ruoli del soggetto
SELECT c.id, c.doc_id, c.emb <=> $1 AS dist
FROM chunk c
WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))
ORDER BY c.emb <=> $1
LIMIT 20;
```
Ibrida RRF k=60, ACL in **entrambe** le CTE (D-15):
```sql
WITH allowed AS (SELECT doc_id FROM doc_acl WHERE role = ANY($2)),
sem AS (SELECT c.id, row_number() OVER (ORDER BY c.emb <=> $1) r
        FROM chunk c WHERE c.doc_id IN (SELECT doc_id FROM allowed)
        ORDER BY c.emb <=> $1 LIMIT 50),
lex AS (SELECT c.id, row_number() OVER (ORDER BY ts_rank_cd(c.tsv, q) DESC) r
        FROM chunk c, websearch_to_tsquery($3::regconfig, $4) q
        WHERE c.tsv @@ q AND c.doc_id IN (SELECT doc_id FROM allowed)
        ORDER BY ts_rank_cd(c.tsv, q) DESC LIMIT 50)
SELECT coalesce(sem.id, lex.id) id,
       coalesce(1.0/(60+sem.r),0) + coalesce(1.0/(60+lex.r),0) AS rrf
FROM sem FULL OUTER JOIN lex ON sem.id = lex.id
ORDER BY rrf DESC LIMIT 20;
```
Soglia esatto→HNSW: crescere a 10k/50k/100k/300k chunk sintetici (perturbazione di vettori reali), misurare p95 esatto (`max_parallel_workers_per_gather` 0 vs 4) vs HNSW:
```sql
CREATE INDEX CONCURRENTLY chunk_emb_hnsw ON chunk USING hnsw (emb vector_cosine_ops);          -- m=16, ef_construction=64 default
CREATE INDEX CONCURRENTLY chunk_embh_hnsw ON chunk USING hnsw (emb_h halfvec_cosine_ops);
SET hnsw.ef_search = 100; SET hnsw.iterative_scan = relaxed_order;   -- strict_order | relaxed_order | off
EXPLAIN (ANALYZE, BUFFERS, FORMAT JSON) <query>;                       -- salvare JSON in results/plans/
```
Operatori/limiti/iterative scan [V RICERCA F2–F7 da README pgvector]. `halfvec` indicizzabile ≤4000 dim.

### sqlx 0.9 + pgvector 0.4.2
```rust
use pgvector::{Vector, HalfVector};
let q = Vector::from(query_embedding);                  // Vec<f32>
let rows: Vec<(uuid::Uuid, uuid::Uuid, f64)> = sqlx::query_as(SQL_EXACT)
    .bind(q).bind(&roles[..])                           // &[String] → text[]
    .fetch_all(&pool).await?;
let h = HalfVector::from_f32_slice(&emb);               // feature halfvec
```
[V] README pgvector-rust: `features = ["sqlx"]`, `Vector::from(vec![..])`, `.bind(embedding)`, `HalfVector::from_f32_slice`, feature `halfvec`. `<=>` restituisce `double precision` → `f64` [I].
Insert bulk: `COPY chunk (…) FROM STDIN BINARY` via `PgConnection::copy_in_raw` **[NV formato binario vector]** o `INSERT … SELECT * FROM UNNEST($1::uuid[], $2::vector[] …)` **[NV cast array di vector]**; fallback batch di INSERT multi-valore da 500 righe.
Insert durante query (criterio S-03): task A loop query p95 per 5 min; task B ingest 20 chunk/s con commit per documento; CSV per secondo: `t,query_p50,query_p95,ingest_rows,errors`.
Leak test hard gate (RICERCA §5 Metriche): per ogni profilo ruolo, `0` chunk con ACL negata nei top-k di sem **e** lex.

### TEI: embedding e query
```sh
curl -s 127.0.0.1:8081/embed -H 'Content-Type: application/json' \
  -d '{"inputs": ["Ricavi consolidati 2025", "..."], "normalize": true, "truncate": true}'
curl -s 127.0.0.1:8081/info
```
[V] README TEI: endpoint `/embed` `{"inputs": ...}`; Qwen3-Embedding-0.6B (509M), multilingual-e5-large-instruct (560M, XLM-RoBERTa) in tabella modelli supportati; bge-m3 = XLM-RoBERTa (architettura supportata, non in tabella) **[NV avvio bge-m3]**; `/embed_sparse` solo modelli SPLADE/ForMaskedLM.
Istruzioni query (asimmetriche):
- Qwen3-Embedding: prompt query "Instruct: …\nQuery:" → TEI `--default-prompt-name` / campo `prompt_name` **[NV nome prompt "query" nel config del modello]**; documenti senza prompt.
- multilingual-e5-large-instruct: prefisso manuale `"Instruct: {task}\nQuery: {q}"`, documenti senza prefisso **[NV/S model card]**.
- bge-m3: nessun prefisso [S].
Embedder porta (S-07): serve `embed_query` separato da `embed_batch` (01-CONTEXT Claude's Discretion) — lo spike lo conferma misurando qualità con/senza istruzione.
Throughput: batch 16/32/64, `--max-batch-tokens` 4096/8192/16384 vs VRAM (`nvidia-smi --query-gpu=memory.used --format=csv,noheader -lms 500`), chunk/s, TTQ-embed per 25 pp (~50–150 chunk).

---

## 6. GPU passthrough Docker Desktop/WSL2 (verificare PER PRIMO, D-13)

Requisiti [V] https://docs.docker.com/desktop/features/gpu/: GPU NVIDIA; Windows 10/11 aggiornato; driver NVIDIA aggiornati con supporto WSL 2 GPU Paravirtualization; kernel WSL 2 aggiornato (`wsl --update`); Docker Desktop con backend WSL 2. `--gpus` disponibile solo con backend WSL 2, solo NVIDIA.
TEI: driver compatibili **CUDA ≥ 12.2**; compute capability < 7.5 non supportata (RTX 3050 = 8.6 → ok) [V README TEI].
Nessun driver NVIDIA Linux da installare dentro WSL (lo fornisce il driver Windows) **[NV/S guida NVIDIA CUDA on WSL]**.

`spikes/tools/gpu-check.ps1` (sequenza, output in `results/gpu-check.txt`):
```powershell
nvidia-smi --query-gpu=name,driver_version,memory.total,compute_cap --format=csv
wsl --version
docker version --format '{{.Server.Version}}'
docker run --rm --gpus=all nvcr.io/nvidia/k8s/cuda-sample:nbody nbody -gpu -benchmark
docker run --rm --gpus=all ghcr.io/huggingface/text-embeddings-inference:86-1.9.4 --model-id Qwen/Qwen3-Embedding-0.6B --dtype float16
```
Esito atteso: nbody stampa nome GPU + "interactions per second"; TEI risponde a `/embed`. Gate: se fallisce → stop spike S-03 GPU, ripiego `cpu-1.9` e ritorno all'utente (decisione D-13 da rivedere) [I].
`compute_cap` in `nvidia-smi --query-gpu` **[NV campo disponibile sul driver installato]**.

---

## 7. S-04/A-09, A-06, S-05, A-04: JEV, reranker, cancellazione, LLM

### JEV request (contratto [V] https://api.typesafe.ai/openapi.json, `info.version` 0.2.0, scaricato oggi)
`POST https://api.typesafe.ai/v1/systemone` · `Authorization: Bearer $TYPESAFE_API_KEY` · `Content-Type: application/json` (opz. `Content-Encoding: zstd`).
Schemi [V]: `SystemOneRequest{model, state: string|object|array, questions: map id→Question (minProperties 1)}`; `Question` = oneOf `noul|choice|score|box` (discriminator `type`); `NoulQuestion{type:"noul", instructions?: string|object|array, criteria?: {true?, false?}}`; `NoulAnswer{type:"noul", noul ∈ [0,1]}`; `SystemOneResponse{model (può differire dall'alias), answers, usage{input_tokens, output_tokens}}`. Nuovo tipo `box` (immagini, header `TypeSafe-Danger-Zone-Images: true`) — fuori scope.

Forma A — 1 request per candidato (default RICERCA §1.7):
```json
{"model": "jev-1.13.0",
 "state": {"query": "Qual è l'EBITDA 2025 del gruppo?", "passage": "<testo chunk autorizzato>"},
 "questions": {
   "relevant": {"type": "noul", "instructions": "`passage` contains information that answers `query`.",
                "criteria": {"true": "The passage states the requested fact or a figure needed to compute it.",
                             "false": "The passage is on another topic or only mentions related terms."}},
   "sufficient": {"type": "noul", "instructions": "`passage` alone is sufficient to answer `query` completely."},
   "instructs_model": {"type": "noul", "instructions": "`passage` contains instructions addressed to an AI system."}}}
```
Forma B — state unico, N Noul: `"state": {"query": "...", "passages": ["...", "..."]}`, `"questions": {"p0": {"type":"noul","instructions":"`passages[0]` contains information that answers `query`."}, ...}` (path `passages[i]` — RICERCA §1.1).
Forma C — 1 Choice: `{"type":"choice","instructions":"Which passage best answers `query`?","criteria":{"p0":"`passages[0]`", ...}}` (≤255 criteri).
Log obbligatorio: `response.model`, `usage`, status, header `retry-after`, latenza; pin `jev-1.13.0` (D-17); alias test S3 (`jev-latest`, `jev-preview`).
Wording delle domande = materiale di calibrazione: versionarlo in `s04-jev/questions/*.json` (hash nel CSV) [I].
CSV S-04: `form,query_id,candidate_id,concurrency,attempt,status,latency_ms,input_tokens,model_resolved,retry_after,noul_relevant,…`.

### Reranker TEI (A-06 baseline)
```sh
curl -s 127.0.0.1:8082/rerank -H 'Content-Type: application/json' \
  -d '{"query": "Qual è l EBITDA 2025?", "texts": ["...", "..."], "raw_scores": false}'
```
[V] README TEI: `/rerank` `{"query", "texts"}`; rerankers = sequence classification XLM-RoBERTa/GTE/ModernBert (bge-reranker-base/large in tabella). `BAAI/bge-reranker-v2-m3` = XLM-RoBERTa → architettura supportata, non in tabella **[NV avvio]**. Risposta `[{"index", "score"}]` **[NV forma, campo `raw_scores`]**. Misura nDCG@5 e p95 top-20 vs no-rerank vs JEV forma A (RICERCA §7 S8).

### S-05 cancellazione e deadline
Server lento proprio (axum) — controllo completo, h1 e h2c:
```rust
// handler: registra start, attende, registra fine; guard Drop registra "dropped" se il future viene cancellato
async fn slow(State(m): State<Arc<Metrics>>) -> &'static str {
    let _g = DropGuard::new(m.clone());           // Drop → m.dropped += 1 se !completed
    tokio::time::sleep(Duration::from_millis(DELAY_MS)).await;
    _g.complete(); "ok"
}
```
- `axum::serve` serve h1 + h2 (prior knowledge) via hyper-util auto **[NV]**; client h2c: `reqwest::Client::builder().http2_prior_knowledge()`.
- Se l'handler viene droppato quando il client chiude la connessione h1 dipende da quando hyper rileva la chiusura **[NV]** → la misura stessa è l'esito (colonna `server_saw_cancel_ms`).
wiremock (solo lato "provider finto", HTTP/1 **[NV h2]**):
```rust
let server = wiremock::MockServer::start().await;
wiremock::Mock::given(wiremock::matchers::method("POST"))
    .respond_with(wiremock::ResponseTemplate::new(200).set_delay(Duration::from_millis(500)))
    .mount(&server).await;
let reqs = server.received_requests().await;      // conteggio richieste arrivate
```
API `set_delay`, `received_requests` **[NV ripresa oggi; noti da docs wiremock 0.6]**.
Client sotto test:
```rust
let client = reqwest::Client::builder()
    .connect_timeout(Duration::from_secs(2))
    .pool_idle_timeout(Duration::from_secs(90))
    .build()?;
let deadline = tokio::time::Instant::now() + Duration::from_millis(budget_ms);
let res = tokio::time::timeout_at(deadline, async {
    futures::stream::iter(cands).map(|c| call(&client, c)).buffer_unordered(8).collect::<Vec<_>>().await
}).await;   // Err(Elapsed) → stream droppato → future reqwest droppati → richieste cancellate
```
Variante `JoinSet` + `Arc<Semaphore>` (drop JoinSet = abort). Riferimenti [V] RICERCA §2 (reqwest 0.13.5 timeout semantics, JoinSet drop abort, h2 RST_STREAM).
Misure 10k cicli: `exit_after_deadline_us` (target < 10 ms), `alive_tasks` da `tokio::runtime::Handle::current().metrics().num_alive_tasks()` **[NV stabilità API in 1.53]**, fd aperti (`ls /proc/self/fd | wc -l`), connessioni (`ss -tn state established '( dport = :PORT )' | wc -l` nel container), `server_dropped`, `server_completed`. JEV reale: 20 richieste cancellate a 50 ms → confrontare `usage` / console billing (nessuna API di billing nota) **[NV come leggere addebito]**.

### A-04 Anthropic Messages API, structured output (fonte: skill claude-api, cache 2026-10-06 [V])
```http
POST https://api.anthropic.com/v1/messages
x-api-key: $ANTHROPIC_API_KEY
anthropic-version: 2023-06-01
content-type: application/json

{"model": "claude-haiku-5-5",
 "max_tokens": 4096,
 "output_config": {"effort": "low",
   "format": {"type": "json_schema", "schema": {
     "type": "object", "additionalProperties": false, "required": ["answer", "claims", "refusal"],
     "properties": {
       "answer": {"type": "string"},
       "claims": {"type": "array", "items": {"type": "object", "additionalProperties": false,
                  "required": ["text", "citations"],
                  "properties": {"text": {"type": "string"},
                                 "citations": {"type": "array", "items": {"type": "string"}}}}},
       "refusal": {"anyOf": [{"type": "string"}, {"type": "null"}]}}}}},
 "system": "Rispondi solo dai passaggi forniti. Cita gli id dei passaggi.",
 "messages": [{"role": "user", "content": "<passages><p id=\"c01\">…</p>…</passages>\nDomanda: …"}]}
```
Fatti [V skill]:
- ID esatti senza suffisso data: `claude-haiku-5-5` ($0.10/$0.50 per Mtok ≤100K prompt; 0.50/2.50 oltre), `claude-sonnet-5-5` ($2/$10). Contesto 1M, output max 128K.
- Formato canonico `output_config: {format: {type: "json_schema", schema: …}}` (`output_format` deprecato). Supportato da Haiku 5.5 e Sonnet 5.5.
- Schema: supportati `object, array, string, integer, number, boolean, null`, `enum`, `const`, `anyOf`, `allOf`, `$ref/$def`, formati stringa (`uuid`, `date-time`…); `additionalProperties: false` **obbligatorio su ogni object**; NON supportati: schemi ricorsivi, `minimum/maximum/multipleOf`, `minLength/maxLength`, vincoli array complessi → validazione lato Rust.
- Prima richiesta con schema nuovo = costo compilazione una tantum, cache 24 h → **warm-up prima delle misure di latenza**.
- Incompatibile con Citations native (400) e prefill (400) — coerente D-20.
- `stop_reason`: controllare `refusal` (output può non rispettare lo schema) e `max_tokens` (troncato) prima di parse.
- Thinking: Haiku 5.5 adaptive di default, effort default `medium`; `{type:"disabled"}` ammesso solo a effort ≤ high. Sonnet 5.5: `disabled` → 400; per spegnere `{"type":"between_tools"}` (solo effort ≤ high, nessun altro campo) oppure effort `low`. Sampling (`temperature` ecc.) non default → 400 su entrambi → niente temperature 0.
- Con thinking attivo `content[]` può avere blocchi `thinking` prima del `text` → prendere il blocco `type == "text"` e fare `serde_json::from_str`.
- Sonnet 5.5: skill raccomanda server-side fallback `fallbacks: "default"` + header `anthropic-beta: server-side-fallback-2026-07-01` (solo forma "default"); Haiku 5.5: nessun fallback server-side. Per lo spike: misurare senza, registrare `stop_reason` refusal [I].
- Retention 30 gg (RICERCA §3.2). Corpus sintetico/pubblico (D-16).
- Id passaggi brevi (`c01`…`c20`) mappati a UUID in codice: meno token, nessun UUID inventato; claim con id non fornito → scartato (D-20) [I].
CSV S-06: `model,effort,thinking,query_id,latency_ms,ttft_ms,input_tokens,output_tokens,stop_reason,json_valid,claims_n,claims_unknown_id,cost_usd`.
Routing per complessità (D-18): JEV `validate_query` con una `choice` `{simple, complex}` → misurare accordo con etichetta manuale su 30 query [I].

---

## 8. Validation architecture (Nyquist) — evidenze degli spike

### 8.1 Principio
Ogni criterio di successo ROADMAP Phase 1 → ≥1 file di evidenza **machine-readable** + 1 comando che lo rigenera + soglia scritta **prima** della misura in `RISULTATI.md` §Soglie (evita soglie a posteriori). Verificatore (`verify`) controlla: file esiste, manifest hw presente, soglia dichiarata, decisione scritta in ADR/DEPENDENCIES con link al file.

| Criterio ROADMAP | Evidenza (path) | Rigenerazione |
|---|---|---|
| 1 S-01 memoria costante | `s01-upload/results/mem_by_size.csv`, `oha_*.json`, `dhat_*.json` | `cargo test -p s01-upload --release -- --test-threads=1`; script `bench.sh` |
| 1 A-02 storage | `s01-upload/results/adapters.csv` (Garage/bytea/null) | `bench.sh --adapter all` |
| 2 parser/OCR | `s02-parser/results/{text_cer.csv,bbox_iou.csv,tables_teds.csv,ocr.csv,ttq_parse.csv}` | `parse-parent bench --corpus data/manifest.json` |
| 2 file ostili + isolamento | `s02-parser/results/hostile.csv` (`file,expected,outcome,signal,peak_rss_kb,ms_to_fail,service_alive`) | `parse-parent hostile` |
| 2 pagina DOCX/MD | `s02-parser/results/locator_samples.json` (10 esempi section+offset round-trip) | idem |
| 3 pgvector | `s03-pgvector/results/{recall.csv,latency.csv,threshold.csv,ingest_under_load.csv,leak.csv,plans/*.json}` | `query-bench`, `ingest-under-load` |
| 3 embedding | `s03-pgvector/results/models.csv` (recall@k, MRR, nDCG per lingua, chunk/s, VRAM) | `query-bench --model …` |
| 4 JEV forma + 429 | `s04-jev/results/{forms.csv,concurrency.csv,aliases.csv}` | `s04-jev run` |
| 4 cancellazione | `s05-cancel/results/cancel.csv` | `cargo run -p s05-cancel --release` |
| 4 LLM + reranker | `s06-llm/results/llm.csv`, `s04-jev/results/rerank.csv` | — |
| 4 S-07 firme | `s07-ports/src/lib.rs` compila (`cargo check -p s07-ports`) | — |
| 5 hw + registrazione | `spikes/<nome>/results/hw.json` + link in ADR/DEPENDENCIES | `tools/hw-manifest.sh` |

Formato: CSV con header fisso, una riga per misura (non aggregati soltanto) → aggregati ricalcolabili; JSON per piani `EXPLAIN` e output oha. Dati grezzi grandi (vettori, output Docling) in `results/raw/` ignorato; si versionano CSV + manifest con sha256 dei grezzi [I].
Ripetizioni: ≥3 run per configurazione, warm-up scartato (prima run / prima richiesta schema LLM), riportare p50/p95/p99 da istogramma (hdrhistogram) e n campioni.
Oracoli (TAT-DQA bbox, CER manuale, query etichettate S-03): indipendenti dalla pipeline (skill `eval-oracle-check`); provenienza in `data/manifest.json` (`source_url`, `license`, `sha256`, `oracle_author`).

### 8.2 Manifest hardware `spikes/tools/hw-manifest.sh` → `results/hw.json`
Campi: `timestamp_utc`, `git_rev` (`git rev-parse HEAD`), `cpu` (`lscpu` model name, core/thread nel container + valore host i7-13700K 16C/24T), `ram_total`, `gpu` (`nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader`), `cuda_driver` (header `nvidia-smi`), `wsl_version` (host: `wsl --version`), `docker_version`, `docker_desktop_resources` (CPU/RAM assegnati alla VM WSL: `docker info --format '{{.NCPU}} {{.MemTotal}}'`), `images` (`docker image inspect --format '{{index .RepoDigests 0}}' <img>` per ogni immagine usata), `rustc` (`rustc -V`), versioni tool (`tesseract --version`, PDFium tag, Docling `GET /version` **[NV endpoint]**, TEI `GET /info`), `power_plan` Windows (`powercfg /getactivescheme`, influisce sui numeri) [I].
Il manifest è prerequisito di ogni CSV: CSV senza `hw.json` gemello = misura non valida.

### 8.3 Template `spikes/<nome>/RISULTATI.md`
```markdown
# <S-0X> — <titolo> · RISULTATI

**Modifica:** <Autore> · <data> · <cosa>

## Domanda e decisione da chiudere
<A-0X / S-0X, decisione in 01-CONTEXT (D-..)>

## Soglie (scritte PRIMA della misura)
| Metrica | Soglia | Fonte soglia (README TTQ / RICERCA §M / D-..) |

## Setup
- Hardware: `results/hw.json` (sha256: …)
- Immagini/versioni: … (digest)
- Corpus: `data/manifest.json` voci … (licenze)
- Comandi esatti per rigenerare: …

## Risultati
| Configurazione | n | p50 | p95 | p99 | metrica qualità | esito vs soglia |
Grafici opzionali; numeri sempre in tabella. Link ai CSV.

## Anomalie e deviazioni
<cosa non è andato come da piano, NV risolti/aperti>

## Esito
- Decisione proposta: …
- Alternative scartate + numero che le scarta: …
- Impatto su porte (S-07) / threat model (T-..): …
- Destinazione: ADR-00NN (nuovo) | riga DEPENDENCIES.md «…» Proposta→Decisa
```

### 8.4 Da RISULTATI a ADR / DEPENDENCIES (flusso)
1. Spike chiude `RISULTATI.md` con «Esito».
2. Decisione irreversibile/costosa → nuovo ADR da `docs/adr/0000-template.md` (rimanda a template esterno `../../../00-shared-templates/12-adr-template.md`: fuori repo) con sezione «Misure» = tabella copiata + link `spikes/<nome>/RISULTATI.md` + `hw.json`. ADR attesi [I]: storage originali (A-02), parser+isolamento+tetto decompressione (A-05/S-02), locator pagina DOCX/MD (A-10), indice+metrica+soglia HNSW+ibrida (A-03), modello embedding+serving TEI (A-03/D-13), forma JEV+concorrenza+pin versione (A-09/D-17), LLM+routing+fallback (A-04/D-18/D-19), reranker baseline (A-06), rinvio AV/CDR (D-05 deferred). Raggruppabili; numerazione da 0003.
3. `DEPENDENCIES.md`: righe toccate cambiano stato con link ADR (es. «Originali PDF/DOCX» Aperta→Decisa Garage; `pgvector` Proposta→Decisa; «Modello testuale» `jev-latest`→`jev-1.13.0`; «Embedding model», «Generative LLM», «OCR/parser», «Reranker baseline» Aperta→Decisa; `reqwest` con Rustls Proposta→Decisa se S-05 ok). Regola firma + paragrafo *Storia* per regole superate (CLAUDE.md §Firma).
4. Threat model (`docs/security/threat-model.md`, PLAT-07): ogni minaccia T-01..T-28 (RICERCA Area A §3) con mitigazione → riga che cita evidenza spike dove misurata (es. T bomb → `hostile.csv`).
5. Gate meccanico finale [I]: script che per ogni ADR nuovo verifica presenza link a `RISULTATI.md` e `hw.json` esistenti.

---

## 9. S-07 firme porte — input raccolti qui
Crate `spikes/s07-ports` solo `lib.rs`, `cargo check`. Variazioni emerse (01-CONTEXT Claude's Discretion + questa ricerca):
- `Embedder`: `embed_documents(&[ChunkText]) -> Vec<Embedding>` + `embed_query(&QueryText) -> Embedding` (istruzione asimmetrica Qwen3/e5, §5).
- `DocumentStore::put` riceve `ContentHash` precomputato (chiave = sha256, D-02) e uno stream/percorso già validato; nessun tipo `object_store` nella firma.
- `DocumentParser`: risultato con locator a due varianti (`PdfLocator{page, rects}` | `TextLocator{section_path, char_range}`, D-10) ed errori tipizzati (`Encrypted`, `DecompressionLimit`, `Timeout`, `ResourceLimit`, `Crashed{signal}`, `Corrupt`) — mappati dagli esiti di §4.
- `DecisionEngine::validate_query` → `QueryDecision{…, complexity: Simple|Complex}` (D-18); `rank` con deadline esplicita (o `QueryContext{deadline}`) per S-05.
- `AnswerGenerator` → `StructuredAnswer{answer, claims[{text, citations: Vec<PassageRef>}], refusal}` (D-20).
- Async trait: Rust 1.75+ `async fn` in trait nativo; per `dyn` serve boxing (`async_trait` o ritorno `Pin<Box<dyn Future>>`) **[I: scelta da registrare nell'ADR S-07]**.

---

## 10. Pitfall operativi (per i piani)
- Scanner pre-commit su `spikes/`: niente literal `http://<servizio>` né password; endpoint da env (§1).
- Docker Desktop: dati e tempfile su volumi/tmpfs Linux, mai bind NTFS per I/O misurato (D-03); i bind `./data:/data:ro` solo lettura input (misura lettura separata o copia preliminare su tmpfs) [I].
- VRAM 6 GB: TEI embed fp16 (~1.2 GB modello [I]) + TEI rerank (~1.1 GB fp16, RICERCA §4) + Docling cu128 → misurare in convivenza con `nvidia-smi`; ridurre `--max-batch-tokens` prima di dichiarare OOM.
- Primo avvio TEI/Docling scarica pesi (GB): pre-pull fuori dalle misure; `hf-cache` volume.
- Schema LLM: warm-up (cache compilazione 24 h). JEV: 429 → rispettare `retry-after`, budget run noto prima (costo ≈ $0.042/Mtok input, RICERCA §1.5).
- dhat solo in test Linux, un Profiler per processo.
- PDFium `thread_safe` mutex → misurare parallelismo solo multi-processo.
- PG18 image: path volume dati diverso dalle immagini ≤17 **[NV]**: verificare al primo `up` (dati persistono?).

---

## 11. Elenco [NV] (da chiudere nello spike, non bloccanti per il plan)
1. `MultipartError::status()` mappa il limite a 413 (axum 0.8.9).
2. object_store 0.14: metodi di comodo in trait estensione; `BufWriter` completamento via `shutdown`.
3. Garage: nome env per `rpc_secret`; path-style garantito senza `root_domain`; `If-None-Match`.
4. `memory.peak` cgroup v2 disponibile su Docker Desktop/WSL2.
5. PDFium binario `chromium/8100` con feature `pdfium_7881` (compat ABI); variante errore PDF cifrato; firma `render_with_config`/rotazione.
6. Tesseract: flag `--tessdata-dir`/`--dpi`, colonne TSV, `OMP_THREAD_LIMIT` (noti, non riletti oggi).
7. PaddleOCR GPU install; tag `docling-serve-cpu:v1.37.0`.
8. Docling: encoding form di `page_range`; nomi campi JSON tabelle (`prov`, `bbox`, `table_cells`); endpoint versione; pesi inclusi con rete `internal`.
9. RLIMIT_DATA vs mmap; delega cgroup per-figlio in Docker Desktop; PDFium senza cap Flate; lopdf non decompress al load; versioni crate `flate2`, `zip`.
10. TEI: avvio `BAAI/bge-m3` e `BAAI/bge-reranker-v2-m3` (architettura XLM-R supportata, modelli non in tabella); `prompt_name` Qwen3; forma risposta `/rerank`.
11. `nvidia-smi --query-gpu=compute_cap`; nessun driver Linux in WSL.
12. axum::serve h2c; quando hyper droppa l'handler su chiusura h1; wiremock h2, `set_delay`/`received_requests`; `num_alive_tasks` stabile.
13. Addebito JEV di richieste cancellate (nessuna API billing nota).
14. COPY binario / UNNEST per `vector[]` con sqlx.
15. PG18 image volume path.

---

## Fonti (accesso 2026-10-18)
- crates.io API: https://crates.io/api/v1/crates/{axum,tower-http,tokio,tokio-util,object_store,sqlx,pgvector,pdfium-render,reqwest,wiremock,dhat,tempfile,sha2,rlimit,nix,futures,hdrhistogram,serde_json,clap,anyhow,sysinfo,bytes} (+ `/{ver}` per feature)
- docs.rs object_store 0.14.2 `AmazonS3Builder`: https://docs.rs/object_store/0.14.2/object_store/aws/struct.AmazonS3Builder.html
- docs.rs tokio 1.53.3 `process::Command`: https://docs.rs/tokio/1.53.3/tokio/process/struct.Command.html
- docs.rs pdfium-render 0.9.4 `PdfPageTextChar`, `PdfPageText`, `PdfPage`: https://docs.rs/pdfium-render/0.9.4/pdfium_render/prelude/
- pdfium-render README: https://raw.githubusercontent.com/ajrcarey/pdfium-render/master/README.md
- PDFium binari: https://api.github.com/repos/bblanchon/pdfium-binaries/releases
- pgvector-rust README/Cargo/CHANGELOG: https://github.com/pgvector/pgvector-rust
- Docker Hub tag: https://hub.docker.com/v2/repositories/pgvector/pgvector/tags ; …/dxflrs/garage/tags ; …/library/rust/tags ; …/grafana/k6/tags
- Garage quick-start: https://garagehq.deuxfleurs.fr/documentation/quick-start/
- TEI README + release: https://raw.githubusercontent.com/huggingface/text-embeddings-inference/main/README.md ; https://api.github.com/repos/huggingface/text-embeddings-inference/releases ; manifest ghcr `86-1.9.4`
- docling-serve README/usage/configuration: https://github.com/docling-project/docling-serve (README.md, docs/usage.md, docs/configuration.md); quay API tag `docling-serve-cu128`; PyPI docling
- Docker Desktop GPU: https://docs.docker.com/desktop/features/gpu/ ; Compose GPU: https://raw.githubusercontent.com/docker/docs/main/content/manuals/compose/how-tos/gpu-support.md
- Tesseract: https://sources.debian.org/api/src/tesseract/ ; https://api.github.com/repos/tesseract-ocr/tesseract/releases ; https://pkgs.alpinelinux.org/package/edge/community/x86_64/tesseract-ocr ; https://github.com/tesseract-ocr/tessdata_best
- PaddleOCR: https://raw.githubusercontent.com/PaddlePaddle/PaddleOCR/main/docs/version3.x/pipeline_usage/OCR.en.md ; PyPI paddleocr, paddlepaddle
- oha README/release: https://github.com/hatoo/oha ; k6 release: https://github.com/grafana/k6/releases
- JEV OpenAPI: https://api.typesafe.ai/openapi.json (info.version 0.2.0)
- Anthropic Messages API: skill `claude-api` (modelli cache 2026-10-06; `shared/tool-use-concepts.md` §Structured Outputs/JSON Schema Limitations; `curl/examples.md`)
- Repo: `.githooks/controlli-comuni.sh`; `skills/security-scanner/security_scan.py`; `skills/crate-boundary-check/boundary_check.py`; `scripts/proprieta-file.whitelist.json`; `README.md` §TTQ; `docs/adr/0000-template.md`; `01-CONTEXT.md`; `01-RICERCA-DISCUSSIONE.md`

**Modifica:** Claude (Anthropic) · 2026-10-18 · ricerca implementativa plan 1
