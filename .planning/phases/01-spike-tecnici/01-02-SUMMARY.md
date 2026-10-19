---
phase: 01-spike-tecnici
plan: 02
subsystem: infra
tags: [docker-compose, wsl2, gpu, nvidia, tei, docling, pgvector, garage, bash, powershell, session-lock, spike-harness]

# Dependency graph
requires: []
provides:
  - "Harness Compose chiuso degli spike (spikes/compose.yaml, progetto cka-spikes): 10 servizi, profili solo raggruppamenti, avvio per nome, etichetta cka.run_id ovunque, porte solo su 127.0.0.1, segreti solo da ${VAR}"
  - "spikes/tools/session.sh: prenotazione atomica della finestra di misura (mkdir + owner via mv), run_id validato prima di ogni comando docker, up/run/oneshot/exec/cid/stop/down/ps/vram/with/reclaim/init-volumes secondo il contratto <interfaces> del piano"
  - "Gate GPU di wave 0 eseguito per primo e VERDE (RTX 3050, nbody, TEI Qwen3-Embedding-0.6B fp16 con /embed a 1024 valori)"
  - "Pesi Docling offline nel volume docling-models (1192 file, 28,6 GB, --all) con inventario atteso di layout + TableFormer accurate, digest delle immagini e sha256 di ogni file; prove positive CPU e GPU senza rete e prove negative diagnostiche"
  - "Immagine runner Linux (rust 1.99.1-slim-trixie, oha v1.16.0 verificato con sha256, uid 10001) e smoke dei volumi runner-smoke.sh"
  - "hw-manifest.sh per l'hw.json di ogni sessione di misura (criterio 5), con run_id, progetto e container della sessione"
  - "File segreti esterno $HOME/.cka-spikes/spikes.env con ACL del solo utente e chiavi JEV e Anthropic inserite dall'utente (Task 4)"
affects: [01-03, 01-04, 01-05, 01-06, 01-07, 01-08, 01-09, 01-10, 01-11, 01-12, 01-13, 01-14, 01-15, 01-16, 01-17, 01-18]

# Tech tracking
tech-stack:
  added: [pgvector/pgvector 0.8.7-pg18-trixie, dxflrs/garage v2.4.1, text-embeddings-inference 86-1.9.4, docling-serve-cu128 v1.37.0, docling-serve-cpu v1.37.0, rust 1.99.1-slim-trixie, oha v1.16.0, busybox 1.37 (solo prove), nvcr cuda-sample nbody (solo gate)]
  patterns:
    - "Avvio solo per nome di servizio con session.sh up dentro una sessione prenotata; COMPOSE_PROFILES mai impostata"
    - "Ogni container (compose, job del runner, docker run fuori compose) porta cka.run_id; pulizia solo per etichetta, mai compose down né volumi"
    - "Segreti in un file esterno a repo e worktree, passato con --env-file; nessun valore stampato"
    - "Pesi dei modelli scaricati da un job con egress e montati in sola lettura nei servizi senza rete"
    - "Git Bash: MSYS_NO_PATHCONV=1 e cygpath -m per i percorsi host passati a docker.exe"

key-files:
  created:
    - spikes/compose.yaml
    - spikes/.env.example
    - spikes/.gitignore
    - spikes/.dockerignore
    - spikes/docker/runner.Dockerfile
    - spikes/tools/session.sh
    - spikes/tools/runner-smoke.sh
    - spikes/tools/hw-manifest.sh
    - spikes/tools/gpu-check.ps1
    - spikes/results/gpu-check.txt
    - spikes/results/docling-models.txt
    - spikes/results/session-negative.txt
    - spikes/data/corpus/.gitkeep
    - spikes/data/s02/.gitkeep
    - spikes/data/s03/.gitkeep
    - spikes/data/s06/.gitkeep
    - spikes/data/hostile/.gitkeep
  modified: []

key-decisions:
  - "docling e docling-cpu girano con user 1001, l'utente non root dichiarato da entrambe le immagini (.Config.User), non 10001"
  - "docling-models gira come 0:0 con cap_drop ALL e no-new-privileges: il volume nuovo nasce di root e 1001 non ci scrive"
  - "Limite di memoria Docling con mem_limit 8g: compose rifiuta pids_limit insieme a deploy.resources.limits senza pids"
  - "session.sh up ricava il set risolto da config --format json <svc…> (ripiego [NV] del piano): config --services ignora i nomi in compose v5.6.0"
  - "Pesi Docling scaricati con --all (28,6 GB) e mantenuti per decisione dell'utente; gli obbligatori per la misura sono circa 550 MB"
  - "Separatore | nel --format di project_running, identico all'IFS dei due lettori (fix 37c68ee)"

patterns-established:
  - "Sessione di misura: session.sh with <piano> -- <passo>, che prenota, esporta CKA_RUN_ID e in uscita fa down + release"
  - "Inventario atteso dei pesi: repository, revisione, file obbligatori, byte e sha256 confrontati con l'etag LFS di Hugging Face"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Il passthrough GPU di Docker Desktop/WSL2 e l'avvio di TEI sulla RTX 3050 sono verificati PER PRIMI, con esito scritto, prima di ogni misura GPU (D-13)"
    verification:
      - {test: TEST-8, risks: [RISK-001], ref: "spikes/results/gpu-check.txt (sezioni 1-5, ultima riga VERDETTO: VERDE)", status: pass, evidence: "nvidia-smi RTX 3050 6144 MiB compute_cap 8.6, WSL 2.6.3.0, Docker 29.8.2, nbody 183 Ginteractions/s, TEI /embed 1 vettore da 1024; run_id 01-02-20261019t155805z-0709. git_rev registrato 41d98ef con modifiche locali: lo script era quello poi committato in ead4a87, che introduce proprio quella marcatura"}
      - {test: Task-4, ref: "risposta utente del 2026-10-19 riportata dall'orchestratore", status: pass, evidence: "gate GPU VERDE confermato dall'utente"}
  - must_have: "Il compose degli spike contiene tutti i servizi delle slice con porte solo su 127.0.0.1 e segreti solo da env"
    verification:
      - {test: TEST-1, risks: [RISK-001], ref: "docker compose --env-file spikes/.env.example -f spikes/compose.yaml config -q ; config --services con tutti i profili ; config --services senza profili", status: pass, evidence: "rieseguito dal bridge: config -q exit 0 (anche col solo profilo db); con tutti i profili esattamente docling docling-cpu docling-models garage paddle postgres runner tei-embed tei-rerank worker; senza profili nessun servizio; 0 depends_on, 0 env_file"}
      - {test: TEST-7, risks: [RISK-001], ref: "spikes/compose.yaml:42,65,84,105 ; grep -v '^ *#' compose.yaml .env.example | grep -c 'http://' ; security_scan.py sull'elenco esplicito", status: pass, evidence: "rieseguito dal bridge: 4 ports tutti 127.0.0.1, 0 http:// non commentati, 0 segreti valorizzati in .env.example, scanner exit 0 su compose, env, ignore, docker, tools, data, results"}
  - must_have: "I profili Compose si uniscono: nessuna sessione si avvia per profili; ogni avvio nomina i servizi, controlla prima il set risolto e dopo il set in esecuzione"
    verification:
      - {test: TEST-1, risks: [RISK-001], ref: "spikes/tools/session.sh:281-292 (set risolto), :306 (up per nome), :308-313 (assert-only) ; spikes/results/session-negative.txt prova a", status: pass, evidence: "dopo il fix 37c68ee: up postgres exit 0 con SET RISOLTO = NOMINATO; ps --assert-only garage con postgres attivo exit 1 (servizio fuori elenco); ps --assert-only postgres exit 0"}
      - {test: TEST-1 (set del passo 4 oltre postgres), risks: [RISK-001], ref: ".planning/tmp/rapporto-01-02.yaml coverage TEST-1 (Task 3 passo 4)", status: human_judgment, human_judgment: true, rationale: "l'avvio per nome di {docling-cpu}, {docling}, {tei-embed} è attestato solo dal rapporto; il controllo assert-only eseguito allora era inattivo per il difetto del separatore (ogni riga veniva scartata, quindi passava sempre). Indiretto: le prove positive di docling-models.txt mostrano docling-cpu e docling avviati con up. Nessun log versionato che provi l'assenza di TEI nei set Docling con il codice corretto"}
  - must_have: "Ogni sessione di misura è prenotata in modo atomico con un run_id validato; i container portano cka.run_id e la pulizia tocca solo i container della propria sessione"
    verification:
      - {test: TEST-2 (lock concorrente e owner assente), risks: [RISK-001], ref: "spikes/tools/session.sh:131-155 (acquire_impl), :103-110 (lock_state), :251-271 (reclaim)", status: pass, evidence: "rieseguito dal bridge con HOME fittizio nello scratchpad, senza container: due acquire in background -> uno 0, l'altro 3 (OCCUPATO, IN CREAZIONE); dir di lock senza owner -> status IN CREAZIONE, reclaim exit 3, dir intatta; owner con pid morto -> STANTIO"}
      - {test: TEST-2 (rifiuto di servizio di altra sessione), risks: [RISK-001], ref: "spikes/tools/session.sh:294-303 ; spikes/results/session-negative.txt prova b", status: pass, evidence: "postgres della sessione A in esecuzione, up B postgres -> exit 1 'già in esecuzione con un'altra sessione'; container di A con stesso ID e stato prima e dopo"}
      - {test: TEST-2 (reclaim di sessione stantia), risks: [RISK-001], ref: ".planning/tmp/rapporto-01-02.yaml coverage TEST-2 (passo 2c)", status: human_judgment, human_judgment: true, rationale: "with + up postgres terminata con kill -9, status STANTIO, reclaim che rimuove solo postgres della sessione morta lasciando cka-foreign vivo: attestato solo dal rapporto, nessun log versionato. Il meccanismo (down_impl per sola etichetta, session.sh:175-185) non dipende dal difetto del separatore"}
      - {test: TEST-3, risks: [RISK-001], ref: "spikes/tools/session.sh:52-57 (valid_run_id), :124-129 (require_owner)", status: pass, evidence: "rieseguito dal bridge senza container: down \"\", down A_B, down none, up '', cid A_B, release A_B, acquire A_B, ps '' -> exit 2; up/down/stop/run/oneshot con run_id che non possiede la prenotazione -> exit 2; init-volumes con argomento in più -> exit 2. Le parti con container (cka-foreign, cka-label-only, cka-label-stop) sono attestate dal rapporto"}
  - must_have: "Il runner (uid 10001) scrive su volumi nuovi /target, registry Cargo, /work/tmp e sotto /src, provato da uno smoke test anche dopo la ricreazione del container"
    verification:
      - {test: TEST-4, risks: [RISK-001], ref: "spikes/tools/runner-smoke.sh:63-79 ; .planning/tmp/rapporto-01-02.yaml coverage TEST-4", status: human_judgment, human_judgment: true, rationale: "SMOKE: OK (uid 10001, mount 10001:10001 755, /work/tmp 920 GiB liberi, secondo container, getent hosts cka-smoke-dns) attestato solo dal rapporto, nessun log versionato; il bridge non avvia container"}
  - must_have: "Docling CPU e GPU convertono senza egress fin dal primo job con pesi offline in sola lettura, con elenco file e SHA-256 versionati; senza pesi la conversione fallisce in modo diagnostico"
    verification:
      - {test: TEST-5, risks: [RISK-001], ref: "spikes/results/docling-models.txt sezioni Inventario atteso e Validazione", status: pass, evidence: "5 file obbligatori presenti, i due pesi LFS con sha256 = etag Hugging Face; DOCLING_ARTIFACTS_PATH=/models/assente -> LocalEntryNotFoundError, Application startup failed, exit 3; volume temporaneo senza tableformer_accurate.safetensors -> ValueError per TableModel04_rs, exit 3; digest del volume buono invariati. 1192 righe sha256 rieseguite al conteggio dal bridge"}
      - {test: TEST-6, risks: [RISK-001], ref: "spikes/results/docling-models.txt sezione Validazione (prove positive CPU e GPU) ; spikes/compose.yaml:126-129,156-157", status: pass, evidence: "rete parse interna, /models in sola lettura, uid 1001; PDF con tabella 6x4, do_ocr=false table_mode=accurate -> success, 24 celle (CPU 1,74 s, GPU 2,24 s), log 'No model weights will be downloaded at runtime'. Il file è un resoconto versionato scritto dall'esecutore, non il log grezzo: accettato perché riporta run_id, messaggi d'errore e codici d'uscita verificabili"}
  - must_have: "Nessun segreto né endpoint letterale http:// è versionato; .env.example ha valori vuoti per i segreti, JEV_MODEL=jev-1.13.0 e i modelli Haiku/Sonnet"
    verification:
      - {test: TEST-7, risks: [RISK-001], ref: "spikes/.env.example:54,68-69 ; grep segreti valorizzati ; security_scan.py", status: pass, evidence: "rieseguito dal bridge: 0 segreti valorizzati, JEV_MODEL=jev-1.13.0, LLM_MODEL_SIMPLE/COMPLEX Haiku e Sonnet 5.5, COMPOSE_PROFILES e CKA_RUN_ID assenti, scanner exit 0"}
      - {test: TEST-7 (hook), risks: [RISK-001], ref: ".planning/tmp/rapporto-01-02.yaml coverage TEST-7", status: human_judgment, human_judgment: true, rationale: "«pre-commit segreti PASS su 4 commit» (più il commit del fix) è un'affermazione del rapporto: il pre-commit non lascia traccia versionata. Indiretto: lo scanner rieseguito oggi sugli stessi file è verde"}
  - must_have: "I segreti locali stanno in $HOME/.cka-spikes/spikes.env, fuori da repo e worktree, passato con --env-file"
    verification:
      - {test: Task-4, risks: [RISK-001], ref: "grep -cE '^(TYPESAFE_API_KEY|ANTHROPIC_API_KEY)=.+' sul file segreti esterno", status: pass, evidence: "rieseguito dal bridge: 2 (solo conteggio, nessun valore stampato); COMPOSE_PROFILES/CKA_RUN_ID nel file: 0; spikes/.env assente e ignorato; session.sh:69-82 passa il file con --env-file"}
  - must_have: "Ogni sessione di misura può produrre un hw.json valido con hw-manifest.sh, con run_id, progetto Compose e immagini dei container"
    verification:
      - {test: Task-2-verify, ref: "bash spikes/tools/hw-manifest.sh <scratchpad>/hw.json", status: pass, evidence: "rieseguito dal bridge: JSON valido con timestamp_utc, git_rev, gpu, docker_version, run_id, compose_project, containers; cuda_driver 13.3"}

# Metrics
duration: 6h29m
completed: 2026-10-19
---

# Phase 1 Plan 02: Harness infrastrutturale e gate GPU di wave 0 Summary

**Compose chiuso degli spike a 10 servizi avviabili solo per nome dentro una sessione prenotata (session.sh con lock atomico e run_id validato), gate GPU VERDE sulla RTX 3050 con TEI, pesi Docling offline con inventario atteso e prove positive e negative, runner Linux non root e segreti solo nel file esterno**

## Performance

- **Duration:** circa 6 h 29 min di calendario (da `ea09d68` alle 15:12 al merge `a43a0fa` alle 21:41, ora italiana), comprese l'attesa del checkpoint del Task 4, il download di 28,6 GB con 14 riprese e la correzione di session.sh
- **Started:** 2026-10-19T15:12:35+02:00
- **Completed:** 2026-10-19T21:41:19+02:00
- **Tasks:** 4 di 4 (Task 4 checkpoint umano)
- **Files modified:** 17 creati: i 16 di files_modified più `spikes/results/session-negative.txt`

## Accomplishments

- `spikes/compose.yaml` con `name: cka-spikes` e i 10 servizi: ognuno ha `profiles` e l'etichetta `cka.run_id`, nessun `depends_on` né `env_file`, le 4 porte pubblicate solo su 127.0.0.1, Docling solo sulla rete interna `parse` con i pesi in sola lettura, worker e paddle senza rete e con utente 10001.
- `spikes/tools/session.sh` con tutti i sottocomandi del contratto: prenotazione `mkdir` + `owner` pubblicato con `mv`, stati vivo, stantio, in creazione e stantio senza owner, validazione del run_id prima di ogni comando docker, `down` solo per etichetta, `reclaim` solo su prenotazione stantia.
- Gate GPU di wave 0 eseguito per primo: `VERDETTO: VERDE`, con VRAM a 1840 MiB a riposo e 3158 MiB con TEI caricato, su 6144.
- Pesi Docling scaricati nel volume `docling-models` e registrati in `spikes/results/docling-models.txt`: digest delle due immagini, versioni (docling-slim 2.138.0, docling-serve 1.37.0), inventario atteso a 5 file obbligatori, sha256 dei 1192 file; conversione offline riuscita su CPU e GPU; fallimento diagnostico con artefatti assenti e con volume incompleto.
- Immagine runner con oha verificato tramite sha256, utente 10001, `TMPDIR=/work/tmp`; `runner-smoke.sh` e `hw-manifest.sh` pronti per i consumatori.
- File segreti esterno con ACL del solo utente, segreti locali generati con un CSPRNG senza passare dalla shell, chiavi dei provider inserite dall'utente.

## Task Commits

1. **Task 1: compose, env, ignore, runner.Dockerfile e cartelle dati** - `584e20b` (feat)
2. **Task 2: strumenti di sessione, orchestrazione e misura** - `c3cce23` (feat)
3. **Task 3: gate GPU, pesi Docling offline e smoke dell'harness** - `41d98ef` (fix: set risolto da config json e gate GPU robusto ai DNS transitori), `ead4a87` (test: gate GPU verde e pesi Docling con inventario atteso)
4. **Task 4: chiavi API e decisione sul gate GPU** - checkpoint umano, nessun commit (file esterno al repo)
5. **Correzione dopo il rifiuto del primo bridge** - `37c68ee` (fix: separatore `|` in project_running, prove negative in `spikes/results/session-negative.txt`)

Merge su main: `ad19dc6` (harness) e `a43a0fa` (correzione).

## Files Created/Modified

- `spikes/compose.yaml` - servizi, profili, reti, volumi ed etichette di sessione
- `spikes/.env.example` - chiavi per sezione con segreti e URL interni vuoti, pin JEV e modelli LLM
- `spikes/.gitignore`, `spikes/.dockerignore` - esclusioni W7
- `spikes/docker/runner.Dockerfile` - ambiente Linux non root degli spike
- `spikes/tools/session.sh` - orchestrazione host, prenotazione e pulizia per etichetta
- `spikes/tools/runner-smoke.sh` - inizializzazione e smoke dei volumi del runner
- `spikes/tools/hw-manifest.sh` - hw.json di sessione
- `spikes/tools/gpu-check.ps1` - gate GPU di wave 0
- `spikes/results/gpu-check.txt` - esito del gate
- `spikes/results/docling-models.txt` - manifest e prove dei pesi Docling
- `spikes/results/session-negative.txt` - prove negative di session.sh dopo il fix (file in più, vedi deviazioni)
- `spikes/data/{corpus,s02,s03,s06,hostile}/.gitkeep` - cartelle dati W11

## Decisions Made

- **Utente di Docling:** `user: "1001"`, perché entrambe le immagini dichiarano già quell'utente non root; parte e legge `/models` in sola lettura (uid 1001, gid 0). Il ripiego del piano su 10001 non è servito.
- **Job dei pesi come root senza capability:** il volume nuovo nasce di root e 1001 non può scriverci. Restano `cap_drop: [ALL]` e `no-new-privileges`.
- **mem_limit invece di deploy.resources.limits:** con `pids_limit` compose rifiuta un blocco `deploy.resources.limits` senza pids (`config -q` rosso).
- **Set risolto da `config --format json`:** compose v5.6.0 ignora i nomi passati a `config --services`; si usa il ripiego [NV] previsto dal piano.
- **Avvio per nome senza profilo:** funziona (postgres, docling-cpu, docling e tei-embed avviati con `up <svc>` senza `--profile`), quindi il ripiego sui profili non è servito.
- **Persistenza di pgdata con PG18:** `PGDATA=/var/lib/postgresql/18/docker` sotto il volume montato su `/var/lib/postgresql`; tabella ed estensione presenti dopo un secondo `up`; pgvector 0.8.7, quindi almeno 0.8.4 (D-12). Fonte: rapporto dell'esecutore.
- **Pesi Docling con `--all`:** 28,6 GB, mantenuti per decisione dell'utente; per la misura ne bastano circa 550 MB.

### Task 4 (checkpoint umano)

Risposta dell'utente del 2026-10-19, riportata dall'orchestratore: chiavi `TYPESAFE_API_KEY` e `ANTHROPIC_API_KEY` inserite in `$HOME/.cka-spikes/spikes.env` (controllate per lunghezza senza stamparle: compilate, senza virgolette né spazi); gate GPU VERDE confermato; pesi Docling con `--all` mantenuti. Il bridge ha ricontato le chiavi: 2.

## Rischi MEDIUM (01-02-RISK-AUDIT-v2.md, risk_audit_8) e trattamento

| Rischio | Trattamento | Prova | Residuo |
|---------|-------------|-------|---------|
| RISK-001 MEDIUM, supply chain: il manifest dei pesi non vincola un inventario atteso | sezione «Inventario atteso» con repository, revisione, file, byte e sha256 dei 5 file obbligatori; digest LFS confrontati con l'etag di Hugging Face; revisioni di tutti i 15 repository; validazione di presenza, digest e caricamento prima di chiudere il Task 3 | `spikes/results/docling-models.txt`; prove negative su artefatti assenti e su volume incompleto, entrambe exit 3 diagnostico | il caso «preload interrotto» del test_hint non è stato provato come scenario a sé (il download si è interrotto 14 volte per DNS, ma il gate è stato provato solo su volume incompleto); i pesi non obbligatori di `--all` non hanno inventario atteso |

Tutti i TEST-1..8 di `01-02-TEST-DESIGN.md` catturano RISK-001 di risk_audit_8 e sono collegati nel blocco coverage. Il plan-gate PLAN-010-02 resta `TEST_SPEC_READY`: test-write ha esito NOT_APPLICABLE (`.planning/tmp/test-write-01-02.yaml`), perché files_modified non contiene percorsi di test e le prove sono le verifiche dei Task 1-3.

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Rule 3 - Blocking] docling-models senza utente esplicito non scrive sul volume**
- **Found during:** Task 3 · **Fix:** `user: "0:0"` più `cap_drop: [ALL]` e `no-new-privileges`, con commento motivato (`spikes/compose.yaml:170-177`) · **Committed in:** `584e20b`

**2. [Rule 3 - Blocking] limite di memoria Docling via deploy.resources.limits rifiutato da compose**
- **Fix:** `mem_limit: 8g`; il blocco `deploy` resta solo per la reservation GPU (`spikes/compose.yaml:133-139`) · **Committed in:** `584e20b`

**3. [Rule 1 - Bug] il commento di testa del compose con la parola depends_on rompeva il conteggio a 0**
- **Fix:** commento riformulato · **Committed in:** `584e20b`

**4. [Rule 2 - Missing] TMPDIR del runner**
- **Fix:** `ENV TMPDIR=/work/tmp` (`spikes/docker/runner.Dockerfile:34-35`): i tempfile di default vanno sul volume work-tmp (D-03) · **Committed in:** `584e20b`

**5. [Rule 1 - Bug] cuda_driver null in hw-manifest con il driver 610.88**
- **Fix:** regex `CUDA[A-Z ]* Version` (`spikes/tools/hw-manifest.sh:66`), ora 13.3 · **Committed in:** `c3cce23`

**6. [Rule 1 - Bug] config --services ignora i nomi: up postgres rifiutato**
- **Fix:** ripiego [NV] del piano con `config --format json <svc…>` (`spikes/tools/session.sh:281-292`) · **Committed in:** `41d98ef`

**7. [Rule 1 - Bug] primo gate GPU ROSSO falso per un DNS transitorio, log persi per --rm**
- **Fix:** TEI senza `--rm`, fino a 3 tentativi, log ripuliti da ANSI e da URL firmati (`spikes/tools/gpu-check.ps1:110-138`); nessun intervento su driver o WSL · **Committed in:** `41d98ef`

**8. [Rule 1 - Bug] log TEI con caratteri corrotti e git_rev che non segnalava lo script modificato**
- **Fix:** `[Console]::OutputEncoding` UTF-8 e marcatura «con modifiche locali in spikes/tools» (`spikes/tools/gpu-check.ps1:16,52-55`) · **Committed in:** `ead4a87`

**9. [Rule 3 - Blocking] il pre-commit rifiuta i commit di soli .txt (security-scanner rc=2 su scope vuoto)**
- **Fix:** risultati committati insieme al fix reale di gpu-check.ps1, gate rilanciato con lo script di quel commit, nessun `--no-verify` · **Committed in:** `ead4a87`

**10. [Rule 1 - Bug] separatore spazio nel --format di project_running contro IFS='|' dei lettori**
- **Issue:** `ps --assert-only` e il rifiuto di un servizio in esecuzione con un altro run_id in `up` non scattavano mai: con lo spazio come separatore l'intera riga finiva nel primo campo, non coincideva con alcun servizio e veniva scartata. Il primo rapporto dichiarava questo fix, ma nel codice non c'era: per questo il primo bridge è stato rifiutato.
- **Fix:** separatore `|` con commento (`spikes/tools/session.sh:187-192`); unici lettori `ps_impl` (`:203`) e `cmd_up` passo 2 (`:296`), entrambi `IFS='|'`, verificato dal bridge con una ricerca su tutto il file
- **Verification:** `spikes/results/session-negative.txt` prove a e b · **Committed in:** `37c68ee`

**11. [Deviazione a] file in più fuori files_modified: `spikes/results/session-negative.txt`**
- Prova negativa versionata della correzione 10, sotto `spikes/results/` (cartella di 01-02). Va aggiunta all'insieme chiuso di 01-02 quando si riconcilia il piano o la WIRING-MAP.

**12. [Deviazione a] PDF di prova scritto con python3 stdlib invece di ImageMagick**
- ImageMagick produce un raster senza testo: con `do_ocr=false` TableFormer vedrebbe celle vuote. Il PDF con testo nativo dà una tabella 6x4 con 24 celle verificabili. Lo script di prova è rimasto fuori dal repo.

**13. [Deviazione a] ordine del Task 3:** download dei pesi (passo 5) prima dei set Docling del passo 4, perché Docling senza pesi non regge l'avvio.

**14. [Deviazione a] `pip show docling` non trova il pacchetto:** nell'immagine la libreria si chiama docling-slim 2.138.0; le versioni sono state registrate da `pip list`.

**15. [Non nel rapporto, trovata dal bridge] `tei-rerank` riceve `--dtype float16` fisso** (`spikes/compose.yaml:100-101`), non previsto dal piano per il reranker. Effetto: dtype non variabile da env per S-04. Da valutare in 01-08/01-10.

---

**Total deviations:** 15 (10 auto-fix, 5 di processo o documentali); nessuna di livello b o c.
**Impact on plan:** le correzioni servono al funzionamento sull'host Windows e alla correttezza dei controlli di sessione. L'unica estensione di perimetro è il file di prove in più.

## Issues Encountered

- DNS di Docker Desktop instabile durante il download dei pesi: 14 riprese (4 + 12 tentativi), riuscito al tentativo 11 del secondo giro; ogni tentativo riparte dai file già scaricati. L'utente lo considera transitorio. Utile saperlo per i download HF di 01-08, 01-10 e 01-15.
- La VRAM del passo 4 è stata letta subito dopo `up`, con i modelli ancora da caricare: i valori sotto carico reale sono in `spikes/results/gpu-check.txt` e nella prova GPU di docling-models.txt (2644 MiB).

## Discrepanze fra rapporto e prove

- `spikes/results/gpu-check.txt` riporta `git_rev: 41d98ef (con modifiche locali in spikes/tools)`: il gate è stato lanciato con lo script poi committato in `ead4a87`. È coerente, perché la marcatura stessa è introdotta da quel commit.
- La prima sezione del rapporto elenca fra le scelte il separatore `|` in project_running, ma il codice di `ad19dc6` usava lo spazio. Il fix reale è `37c68ee`. Ne segue che gli «assert-only ok» del Task 3 passo 4 e il controllo del passo 2 di `up` sono stati eseguiti con un controllo inattivo: dopo il fix sono provati solo su postgres (session-negative.txt). Per gli altri set il coverage è `human_judgment`.
- La pulizia del reclaim e il «pre-commit PASS su 4 commit» non hanno traccia versionata: vedi coverage.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: elevation | spikes/compose.yaml | `docling-models` gira come 0:0 con egress (rete default) e volume `docling-models` in scrittura; il piano prevedeva solo `cap_drop` e `no-new-privileges` (T-01-02-04), non l'utente root. Mitigato da `cap_drop: [ALL]`, `no-new-privileges`, uso solo fuori dalle misure e digest dei pesi verificati dopo il download |
| threat_flag: info | spikes/compose.yaml | docling e docling-cpu girano come uid 1001 ma gid 0 (gruppo root dell'immagine); il volume dei pesi è in sola lettura e la rete è interna |
| threat_flag: tampering | spikes/tools/gpu-check.ps1 | crea il volume `cka-spikes_hf-cache` fuori compose con le etichette del progetto, condiviso con tei-embed e tei-rerank: contenuto della cache scaricato da HF senza inventario atteso |

## Known Stubs

Nessuno stub di dati. Il compose riferisce tre file che non esistono ancora, come seam intenzionali assegnati ai rispettivi proprietari: `docker/worker.Dockerfile` (01-07), `docker/paddle.Dockerfile` (01-13) e `docker/garage.toml` (01-04). Finché non esistono, worker, paddle e garage non si avviano: il passo 4 non li ha mai costruiti.

## User Setup Required

Già fatto: chiavi dei provider nel file segreti esterno (Task 4). Resta aperto solo il punto su `S06_MAX_COST_USD` qui sotto.

## Punti aperti

- `S06_MAX_COST_USD` va portato a 2 per la decisione dell'utente su 01-12 (spesa Anthropic dimezzata), applicando la modifica con un ripasso di plan prima dell'onda 7. `.env.example` è un file chiuso di 01-02 e resta a 5: basta il valore locale. Il bridge ha verificato che il file locale è ancora a 5 (non coincide con 2) e non l'ha modificato.
- Lo script delle prove negative Docling aspetta `/health` fino a 6 minuti anche quando il servizio è già uscito: miglioria possibile.
- Il pre-commit rifiuta i commit composti solo di `.txt`.
- Un `acquire` lanciato da una subshell che termina diventa subito STANTIO, perché l'owner è `$PPID`; confermato dal bridge con la prova di concorrenza. Per una sessione intera la forma giusta resta `with`.
- `--all` porta 28,6 GB di modelli VLM che la misura non usa: ridurlo a layout + tableformer richiede una decisione dell'utente.

## Next Phase Readiness

- L'harness è chiuso: le slice variano solo i parametri da env e avviano servizi e job per nome con session.sh dentro una sessione prenotata.
- PLAT-07 è copiato dal frontmatter del piano: qui si realizzano le mitigazioni T-01-02-01..09, non il threat model di progetto. Se il requisito sia chiuso lo decide il verifier di fase.

## Self-Check: PASSED

- I 16 file di files_modified sono presenti nel diff `ea09d68..a43a0fa` sotto `spikes/`; l'unico file in più di 01-02 è `spikes/results/session-negative.txt`. Gli altri file nuovi del diff sono di 01-01.
- I commit `584e20b`, `c3cce23`, `41d98ef`, `ead4a87`, `37c68ee` e i merge `ad19dc6`, `a43a0fa` sono presenti nella storia.
- Rieseguiti dal bridge: `config -q` exit 0 (anche col solo profilo db), 10 servizi con tutti i profili e 0 senza, controlli JSON su worker, paddle e hardening Docling verdi; `bash -n` exit 0 sui tre script bash; i grep del Task 2 a 0; scanner exit 0; hw-manifest JSON valido; prove di lock e di run_id con HOME fittizio verdi; conteggio delle chiavi = 2.

---
*Phase: 01-spike-tecnici*
*Completed: 2026-10-19*

**Modifica:** Claude (Anthropic) · 2026-10-19 · SUMMARY del piano 01-02 dal bridge di execute, prove incrociate con git, fix 37c68ee verificato, verifiche statiche e di lock rieseguite
