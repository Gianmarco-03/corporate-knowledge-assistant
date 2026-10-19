# Racconto del piano 01-02: l'harness infrastrutturale e il gate GPU di wave 0

## Cosa

Prima di questo piano gli spike avevano solo il workspace Cargo di 01-01: niente database, niente modelli, niente modo di misurare. Adesso c'è un ambiente Docker chiuso con dieci servizi (PostgreSQL con pgvector, Garage, i due TEI per embedding e reranking, Docling su GPU e su CPU, il job che scarica i pesi di Docling, il worker isolato, PaddleOCR e un runner Linux). Si può avviare soltanto per nome di servizio, dentro una «sessione di misura» prenotata con un identificativo (`run_id`) che marca ogni container e ne delimita la pulizia. In più sappiamo che la GPU funziona da dentro Docker: il gate è verde. Docling converte un PDF senza rete con pesi verificati, e senza pesi fallisce in modo chiaro.

Il piano porta il requisito PLAT-07 per la parte degli spike: segreti fuori dal repository, porte solo su loopback, container non root e senza rete dove servono. Le prove di misura vere e proprie le faranno le slice delle onde successive, usando queste primitive senza più toccarle.

## Come

### Percorso di lettura consigliato

1. **`spikes/compose.yaml`**: è la mappa di tutto. Le regole d'uso sono in testa (`spikes/compose.yaml:6-17`), poi vengono le quattro deviazioni dichiarate rispetto alla ricerca (`:19-28`).
   - Ogni servizio ha `profiles` e l'etichetta `cka.run_id: "${CKA_RUN_ID:-none}"` (es. `:35-37`). La sintassi `${VAR:-default}` è l'interpolazione di Compose: prende il valore dall'ambiente o dal file passato con `--env-file`, e se manca usa `none`. Lo script di sessione esclude proprio quel valore, così un container avviato fuori da una sessione non si confonde mai con uno di sessione.
   - I due Docling (`:113-161`) hanno un utente non root, la sola rete `parse` e i pesi montati con `:ro`, cioè in sola lettura. La rete `parse` è dichiarata `internal: true` (`:276`), quindi non ha uscita verso Internet e non può pubblicare porte.
   - La GPU si prenota con `deploy.resources.reservations.devices` (`:85-88`), il modo in cui Compose chiede al runtime NVIDIA di passare il dispositivo al container.
   - Il job `docling-models` (`:163-177`) è l'unico con rete esterna e volume scrivibile.
   - Worker e paddle (`:179-211`) hanno `network_mode: none`, cioè nessuna interfaccia di rete. Il worker ha anche il filesystem in sola lettura e un `tmpfs` per `/work`: un filesystem in RAM, che però conta nella memoria del container.
   - Il runner (`:213-272`) riceve le chiavi dell'env una per una, senza `env_file`, e monta il repository su `/src`.

2. **`spikes/tools/session.sh`**: è l'unico punto da cui si orchestra. Il contratto riassunto è in testa (`spikes/tools/session.sh:5-26`).
   - `set -euo pipefail` (`:28`) fa uscire lo script al primo comando fallito, a ogni variabile non definita e a ogni errore dentro una pipeline.
   - `MSYS_NO_PATHCONV=1` (`:31`) spegne una conversione di Git Bash: senza, un argomento come `/target` arriverebbe a `docker.exe` trasformato in `C:/Program Files/Git/target`. Per questo i percorsi dell'host si convertono a mano con `cygpath -m` (`:48-50`).
   - `valid_run_id` (`:52-57`) usa `${1-}`, che vale stringa vuota se l'argomento manca (con `set -u` un `$1` assente farebbe abortire con un messaggio poco chiaro). Usa anche `[[ $rid =~ $RUN_ID_RE ]]` con la regex in una variabile non quotata: in bash è il modo corretto, perché quotarla la trasformerebbe in un confronto letterale.
   - La prenotazione è in `acquire_impl` (`:131-155`). `mkdir "$LOCK"` (`:137`) è atomico su ogni filesystem: di due richieste simultanee una sola crea la directory. L'owner si scrive in un file temporaneo e si pubblica con `mv -f` (`:146-147`): un rename nella stessa directory è atomico, quindi chi legge vede o il file completo o nessun file.
   - `lock_state` (`:103-110`) distingue fra prenotazione libera, viva, stantia per pid morto (`kill -0` alla riga `:93` controlla solo che il processo esista, senza mandargli segnali), in creazione, e stantia senza owner oltre la grazia (età letta con `stat -c %Y`, `:95-100`).
   - `down_impl` (`:175-185`) filtra sempre per `label=cka.run_id=<run_id>` su tutti i container, anche quelli lanciati fuori compose, e non tocca mai i volumi.
   - **`project_running` (`:187-192`) e i suoi due lettori: qui c'era il difetto.** La funzione stampa una riga per container nella forma `servizio|run_id|nome`. I lettori sono `ps_impl --assert-only` (`:203`) e il passo 2 di `cmd_up` (`:296`), e la leggono con `while IFS='|' read -r svc r name`. `IFS` è la lista dei caratteri con cui `read` spezza la riga in campi: deve contenere esattamente il separatore che il `--format` di `docker ps` mette fra i campi. Nella prima versione il `--format` usava gli spazi mentre l'IFS era `|`. Così `read` non trovava nessun `|`, l'intera riga finiva in `svc`, non coincideva mai con il nome di un servizio e veniva scartata. Risultato: `--assert-only` passava sempre e `up` non rifiutava mai un servizio già avviato da un'altra sessione. Lo spazio poi sarebbe sbagliato comunque: un container fuori compose ha l'etichetta del servizio vuota, e con un separatore di spazi i campi scivolerebbero di una posizione. Il fix (`37c68ee`) mette `|` nel `--format` e lo annota nel commento.
   - Il costrutto `done < <(project_running)` (`:303`) è una *process substitution*: il ciclo legge l'output della funzione come se fosse un file. A differenza di `project_running | while …`, il ciclo resta nella shell principale, così il `die 1` alla riga `:300` termina davvero lo script e non solo una subshell.
   - `cmd_up` (`:273-314`) fa quattro passi: confronta il set nominato con quello che Compose risolverebbe davvero (`:281-292`, tramite `config --format json` e `node`), rifiuta i servizi già attivi con un altro run_id (`:294-303`), avvia con `up -d --no-deps --wait` senza profili (`:306`) e infine verifica che sia in esecuzione solo il set della sessione (`:308-313`).
   - `cmd_run` e `cmd_oneshot` (`:316-342`) copiano il comando del chiamante in un array e lo passano come `"${cmd[@]}"` dopo il nome del servizio `runner`. Ogni argomento resta intatto, spazi compresi, e nessuno può diventare un'opzione di `compose run`.
   - `cmd_init_volumes` (`:344-352`) è l'unico punto con `--user 0:0`, e ha una firma fissa.
   - `cmd_with` (`:432-466`) è la forma consigliata. `trap cleanup EXIT` (`:456`) esegue `down` e `release` all'uscita, e `trap 'exit 130' INT` (`:457`) trasforma un Ctrl-C in un'uscita normale, così anche in quel caso parte il trap di EXIT. Qui il pid dell'owner è `$$`, la shell di `with` che resta viva per tutta la sessione. In `cmd_acquire` è invece `$PPID` (`:238`), il chiamante, perché `acquire` da solo termina subito.

3. **`spikes/docker/runner.Dockerfile`**: `oha` si scarica e si verifica con `sha256sum -c` su un digest fissato in un `ARG` (`spikes/docker/runner.Dockerfile:12-13,21-24`). Le directory `/target`, `/work/tmp` e il registry Cargo nascono di proprietà di 10001 (`:29-32`), perché un volume nominato vuoto eredita proprietario e permessi dalla directory dell'immagine su cui viene montato. `TMPDIR` punta al volume (`:34-35`), poi `USER 10001` (`:37`).

4. **`spikes/tools/runner-smoke.sh`**: la sonda è uno script fra apici singoli in una variabile (`spikes/tools/runner-smoke.sh:35-61`), eseguito con `bash -c "$PROBE" probe "$rid"` (`:67`). In `bash -c` il primo argomento dopo lo script diventa `$0` e il secondo `$1`, quindi dentro la sonda `rid=$1` è il run_id. La sonda gira due volte in container diversi (`:66-70`), per provare che i permessi sopravvivono alla ricreazione. Poi un job nominato e un `getent hosts` provano il DNS fra container (`:72-77`). Il `trap cleanup EXIT` (`:28-31`) ferma il job DNS anche se un passo fallisce.

5. **`spikes/tools/hw-manifest.sh`**: `"${images[@]+"${images[@]}"}"` (`spikes/tools/hw-manifest.sh:83`) è il modo di iterare un array forse vuoto con `set -u`: se l'array è vuoto, l'espansione `+` non produce nulla invece di un errore. `try` (`:44`) restituisce l'output ripulito o la stringa vuota, quindi un campo non rilevabile diventa `null` e non blocca. Il JSON lo scrive `node` da un heredoc `<<'NODE'` fra apici (`:112-146`): gli apici impediscono a bash di espandere i `$` del JavaScript, e i valori arrivano tramite variabili d'ambiente invece che per sostituzione nel testo. Il file viene poi riletto e validato (`:148`).

6. **`spikes/tools/gpu-check.ps1`**: `$ErrorActionPreference = 'Continue'` (`spikes/tools/gpu-check.ps1:13`) evita che lo stderr di un comando nativo interrompa lo script. Gli errori si raccolgono con `Fail`, che scrive in `$script:failed` (`:41`): il prefisso `script:` serve perché una funzione PowerShell altrimenti creerebbe una variabile locale. `[Console]::OutputEncoding` UTF-8 (`:16`) decodifica correttamente l'output di `docker logs`. Con `2>&1` le righe di stderr arrivano come oggetti `ErrorRecord`: `Text` (`:39`) le riporta a stringhe semplici. Gli URL di loopback si costruiscono con `UriBuilder` (`:108-109`), così nel file non compare alcun `http://` letterale. TEI gira in un `try/finally` (`:100-159`) che lo ferma anche in caso di errore, con un massimo di tre tentativi (`:117-138`). Il trattino lungo del verdetto è `[char]0x2014` (`:164`), perché PowerShell 5.1 legge gli script senza BOM come ANSI. L'esito si scrive con `WriteAllLines` in UTF-8 senza BOM (`:168`).

### Flusso dei dati

Il file segreti `$HOME/.cka-spikes/spikes.env` entra solo come `--env-file` nella funzione `dc` (`spikes/tools/session.sh:77-82`). Il `run_id` nasce in `new-id` (`:217-225`), finisce nel file di owner e arriva a Compose come `CKA_RUN_ID`, cioè come etichetta di ogni container. Da lì lo leggono `down`, `cid`, `stop`, `exec`, `ps` e `hw-manifest.sh` (`spikes/tools/hw-manifest.sh:97-103`). I risultati versionati stanno in `spikes/results/`.

## Perché

- **Avvio per nome invece che per profili.** In Compose un servizio parte se è attivo *uno qualsiasi* dei suoi profili (deepsearch_4), quindi avviare `--profile s02` porterebbe con sé tutto ciò che condivide il profilo, TEI compreso, e falserebbe le misure. Si è scelto di nominare i servizi e di controllare il set prima e dopo l'avvio (RISK-001 di risk_audit_1, WIRING W5). Il ripiego sui profili non è servito: l'avvio per nome senza `--profile` funziona.
- **`config --format json` invece di `config --services <nomi>`.** In compose v5.6.0 `config --services` ignora i nomi e restituisce sempre tutti i servizi. La variante JSON con i nomi è la stessa risoluzione che fa `up`.
- **`mkdir` + `mv` invece di `flock` o `O_EXCL`.** Su Git Bash sopra NTFS `flock` non è affidabile, mentre `mkdir` è atomico ovunque. Pubblicare l'owner con un rename evita che un lettore veda un file a metà (RISK-004 di risk_audit_1). Un owner assente si considera «in creazione», mai stantio prima della grazia: si scarta così il rischio di sottrarre una prenotazione proprio mentre nasce.
- **Pulizia per etichetta invece di `docker compose down`.** Il daemon Docker è condiviso fra worktree: `down` del progetto fermerebbe anche le sessioni degli altri. Il run_id si valida prima di ogni comando docker perché un filtro `label=cka.run_id=` vuoto selezionerebbe ogni container che ha quell'etichetta.
- **Orchestrazione sull'host invece del socket Docker nel runner.** Il runner riceve le chiavi dei provider. Dargli anche il socket significherebbe dargli il controllo completo dell'host (T-01-02-07).
- **Volume `work-tmp` invece di un tmpfs per i tempfile degli upload.** Un tmpfs si addebita alla memoria del container e falserebbe le misure di memoria di S-01 (D-03).
- **Job `docling-models` invece di un primo avvio di Docling con la rete aperta.** Il compose è chiuso e i download al primo job falserebbero i tempi (RISK-003 di risk_audit_1). Il job gira come root perché il volume nuovo nasce di root, ma senza capability e con `no-new-privileges`, e non partecipa mai alle misure.
- **Inventario atteso dei pesi.** È la risposta a RISK-001 di risk_audit_8, l'unico rischio MEDIUM della revisione: non basta un elenco di sha256 di ciò che c'è, serve anche l'elenco di ciò che *deve* esserci (repository, revisione, file, byte, digest confrontati con l'etag LFS di Hugging Face). Le due prove negative (artefatti assenti, volume senza TableFormer) mostrano che un'installazione incompleta si ferma all'avvio invece che al primo documento.
- **`mem_limit` invece di `deploy.resources.limits`.** Compose rifiuta `pids_limit` insieme a `deploy.resources.limits` senza pids. Il blocco `deploy` resta solo per la GPU.
- **Utente 1001 per Docling.** È l'utente non root che le immagini dichiarano già. Imporre 10001 avrebbe richiesto di verificare i permessi interni dell'immagine senza alcun vantaggio.
- **PDF di prova con python stdlib invece di ImageMagick.** ImageMagick produce un'immagine raster senza testo: con l'OCR spento la tabella risulterebbe vuota e la prova direbbe poco. Con il testo nativo escono 24 celle verificabili.
- **Segreti generati con `secrets.token_hex(32)` di python invece di `openssl rand`.** L'entropia è la stessa, ma i valori vanno direttamente nel file senza passare per la shell o per l'output. I permessi del file si impostano con `icacls` e non con `chmod`, perché in Git Bash `chmod` non cambia le ACL NTFS.
- **Gate GPU con tre tentativi e senza `--rm`.** Il primo gate è risultato ROSSO per un errore DNS transitorio di Docker Desktop, e `--rm` aveva cancellato i log. Un errore di rete non è un guasto della GPU, e i log devono restare leggibili.

## Test

Le prove di questo piano sono verifiche di integrazione, non test unitari: files_modified non contiene percorsi di test, quindi test-write ha esito NOT_APPLICABLE e il plan-gate PLAN-010-02 resta `TEST_SPEC_READY`. Ogni TEST-N del design cattura RISK-001 di risk_audit_8.

- **TEST-1 (servizi risolti, nessun profilo implicito):** `docker compose --env-file spikes/.env.example -f spikes/compose.yaml config -q`, poi `config --services` con e senza tutti i profili (10 e 0 servizi). Per il controllo dopo l'avvio vedi `spikes/results/session-negative.txt`, prova a.
- **TEST-2 (lock concorrente, rifiuto di sessioni altrui, reclaim):** due `bash spikes/tools/session.sh acquire 01-02 <id>` in background (uno esce 0, l'altro 3). `spikes/results/session-negative.txt` prova b mostra `up` rifiutato su un postgres di un'altra sessione. Il bridge ha ripetuto le prove di lock con un HOME fittizio. Il reclaim con `cka-foreign` intatto è documentato solo nel rapporto.
- **TEST-3 (run_id non valido):** `bash spikes/tools/session.sh down ""` e `down A_B` escono 2 senza toccare docker.
- **TEST-4 (volumi del runner):** `bash spikes/tools/session.sh with 01-02 -- bash -c 'bash spikes/tools/runner-smoke.sh "$CKA_RUN_ID"'` (gli apici singoli rimandano l'espansione di `CKA_RUN_ID` alla shell interna, dove `with` l'ha già esportata) deve stampare `SMOKE: OK`. L'esito è solo nel rapporto.
- **TEST-5 e TEST-6 (Docling offline):** sono descritti in `spikes/results/docling-models.txt`, sezione Validazione, con run_id, messaggi ed exit code.
- **TEST-7 (segreti e porte):** `python skills/security-scanner/security_scan.py spikes/compose.yaml spikes/.env.example spikes/.gitignore spikes/.dockerignore spikes/docker spikes/tools spikes/data spikes/results`.
- **TEST-8 (gate GPU):** `bash spikes/tools/session.sh with 01-02 -- powershell -NoProfile -ExecutionPolicy Bypass -File spikes/tools/gpu-check.ps1`, poi `tail -1 spikes/results/gpu-check.txt`.

Controlli statici che non richiedono container: `bash -n spikes/tools/session.sh` (e lo stesso per gli altri due script bash), `bash spikes/tools/hw-manifest.sh <file>.json`.

## Deviazioni e punti aperti

**Deviazioni.**
- Il difetto del separatore è il punto più istruttivo. Il primo rapporto dichiarava il fix, ma il codice non lo conteneva, e il bridge di allora è stato rifiutato. Il fix vero è `37c68ee`, con prove negative versionate in `spikes/results/session-negative.txt`: un file in più rispetto a files_modified, sotto la cartella dei risultati di 01-02, da aggiungere all'insieme chiuso del piano. Una conseguenza da non dimenticare: gli «assert-only ok» del passo 4 del Task 3 erano stati ottenuti con il controllo inattivo, quindi per i set Docling e TEI l'assenza di servizi estranei non è dimostrata dal codice corretto.
- Altre deviazioni:
  - `docling-models` gira come root, senza capability (threat flag nel SUMMARY);
  - `mem_limit` al posto di `deploy.resources.limits`;
  - `TMPDIR` nel Dockerfile;
  - regex CUDA aggiornata per il driver 610.88;
  - PDF di prova generato con python;
  - download dei pesi anticipato rispetto al passo 4;
  - il pacchetto si chiama docling-slim;
  - `tei-rerank` riceve un `--dtype float16` fisso, non previsto dal piano.
- `spikes/results/gpu-check.txt` riporta `git_rev 41d98ef (con modifiche locali)`: lo script usato è quello committato poi in `ead4a87`.

**Punti aperti.**
- Il DNS di Docker Desktop è stato instabile durante il download (14 riprese). L'utente lo ritiene transitorio, ma conviene ricordarlo per i prossimi download da Hugging Face.
- Lo script delle prove negative Docling aspetta `/health` fino a 6 minuti anche a servizio già uscito: è una miglioria possibile.
- Il pre-commit rifiuta i commit composti solo di `.txt`.
- Un `acquire` lanciato da una subshell che termina diventa subito STANTIO (owner `$PPID`). Per una sessione intera si usa `with`.
- `S06_MAX_COST_USD` va portato a 2 per la decisione dell'utente su 01-12 (spesa Anthropic dimezzata). La modifica va applicata con un ripasso di plan prima dell'onda 7, e basta il valore locale in `spikes.env`, perché `.env.example` è chiuso. Oggi il file locale è ancora a 5.
- Resta da decidere se ridurre i 28,6 GB di `--all` ai soli pesi necessari (circa 550 MB).

**Modifica:** Claude (Anthropic) · 2026-10-19 · racconto didattico del piano 01-02 dal bridge di execute, letto sul diff reale e sul fix 37c68ee
