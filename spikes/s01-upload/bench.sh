#!/usr/bin/env bash
# spikes/s01-upload/bench.sh — benchmark dello spike S-01 (piano 01-04).
#
# Modalità HOST (Git Bash sull'host, dalla radice del worktree; RISK-001 di risk_audit_2):
#   bash spikes/s01-upload/bench.sh host vendor            crate vendorizzate (vedi DNS)
#   bash spikes/s01-upload/bench.sh host tests             test Linux (dhat, pdfgen, admission, gc_race)
#   bash spikes/s01-upload/bench.sh host matrix <run>      run 1..3 della matrice adapter × dimensione ×
#                                                          carico × concorrenza (server nuovo per riga)
#   bash spikes/s01-upload/bench.sh host dedup <sessione>  sessione 1..3 del canale laterale (TEST-7)
#   bash spikes/s01-upload/bench.sh host mitig             mitigazioni di Garage (parti del multipart) dopo la matrice
#   bash spikes/s01-upload/bench.sh host dedup-mitig <n>   canale laterale delle configurazioni mitigate (stesso protocollo)
#   bash spikes/s01-upload/bench.sh host correctness       casi di correttezza (correctness.csv)
#   bash spikes/s01-upload/bench.sh host extras            storage.csv e gc.csv
#   bash spikes/s01-upload/bench.sh host finalize          CSV a header fisso dai TSV grezzi
# Ogni modalità host apre una propria sessione: new-id + acquire --wait (exit 3 dopo
# l'attesa = finestra occupata da un altro piano: si ferma senza toccare nulla), trap che
# fa down + release, servizi avviati per nome con `session.sh up`, nessun profilo.
#
# Modalità RUNNER (lanciate dall'host con session.sh run|oneshot, mai a mano):
#   bench.sh build | serve <args server> | client <args> | dedup-client <args> | correctness-client
# Nessun comando docker nel runner: niente socket del daemon.
#
# DNS: il resolver interno di Docker Desktop oggi fallisce in modo intermittente sui nomi
# dei servizi e sempre sui nomi esterni (index.crates.io). Mitigazioni, senza toccare
# l'harness chiuso di 01-02: (1) crate vendorizzate dall'host in spikes/target/ (ignorata)
# e build nel runner con `--offline`; (2) ogni processo del runner risolve una volta i nomi
# di postgres, garage e cka-s01-server con tentativi limitati e usa l'IP (pin_env).

set -euo pipefail
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

MIB=1048576
SIZES_LABELS=(1m 10m 24m 25m)
declare -A SIZE_BYTES=([1m]=$((1 * MIB)) [10m]=$((10 * MIB)) [24m]=$((24 * MIB)) [25m]=$((25 * MIB)))
ADAPTERS=(garage pgbytea null)
WORKLOADS=(new dedup)
CONCURRENCY=(1 4 8 16)
REQ=200
SERVER=cka-s01-server
VENDOR_DIR=target/vendor-s01

die() { echo "bench.sh: $*" >&2; exit 1; }

# ============================================================== modalità RUNNER

# Risolve un nome con tentativi limitati (120 × 0,5 s) e stampa l'IPv4. Con 20 tentativi
# il resolver di Docker ha lasciato il nome `postgres` irrisolto (gc_race, 2026-10-20).
resolve() {
  local name=$1 ip i
  for (( i = 0; i < 120; i++ )); do
    ip=$(getent ahostsv4 "$name" 2>/dev/null | awk 'NR==1 {print $1}') || true
    if [[ -n ${ip-} ]]; then printf '%s\n' "$ip"; return 0; fi
    sleep 0.5
  done
  return 1
}

# Sostituisce gli host di DATABASE_URL e S3_ENDPOINT con l'IP risolto (valori mai stampati).
# Un nome non risolto ferma il processo: niente ripiego silenzioso sul resolver.
# Guardia: DATABASE_URL e S3_ENDPOINT devono puntare ai servizi del compose (postgres,
# garage). Il 2026-10-20 una DATABASE_URL dell'ambiente host di un altro progetto ha
# scavalcato il file segreti nell'interpolazione di compose: i test con TRUNCATE non
# devono mai partire contro un database esterno.
pin_env() {
  local ip
  if [[ -n ${DATABASE_URL-} && ! $DATABASE_URL =~ @postgres[:/] ]]; then
    die "DATABASE_URL non punta al servizio postgres del compose: mi fermo"
  fi
  if [[ -n ${S3_ENDPOINT-} && ! $S3_ENDPOINT =~ ^http://garage: ]]; then
    die "S3_ENDPOINT non punta al servizio garage del compose: mi fermo"
  fi
  if [[ -n ${DATABASE_URL-} ]]; then
    ip=$(resolve postgres) || die "risoluzione di postgres fallita dopo 60 s"
    export DATABASE_URL=${DATABASE_URL/@postgres:/@$ip:}
    export DATABASE_URL=${DATABASE_URL/@postgres\//@$ip/}
  fi
  if [[ -n ${S3_ENDPOINT-} ]]; then
    ip=$(resolve garage) || die "risoluzione di garage fallita dopo 60 s"
    export S3_ENDPOINT=${S3_ENDPOINT/\/\/garage:/\/\/$ip:}
  fi
}

# Attende che Garage accetti connessioni S3 (bucket di default creato all'avvio).
wait_garage() {
  [[ -n ${S3_ENDPOINT-} ]] || return 0
  local i code
  for (( i = 0; i < 120; i++ )); do
    code=$(curl -s -o /dev/null -w '%{http_code}' "$S3_ENDPOINT" 2>/dev/null || true)
    [[ $code != 000 ]] && return 0
    sleep 0.5
  done
  die "Garage non raggiungibile entro 60 s"
}

cargo_v() { # cargo con le crate vendorizzate, se presenti
  local sub=$1; shift
  if [[ -d /src/$VENDOR_DIR ]]; then
    cargo --config 'source.crates-io.replace-with="vendored-sources"' \
      --config "source.vendored-sources.directory=\"/src/$VENDOR_DIR\"" "$sub" --offline "$@"
  else
    cargo "$sub" "$@"
  fi
}

runner_build() {
  cd /src
  cargo_v build --locked --release -p s01-upload --bins
}

runner_tests() {
  cd /src
  pin_env
  wait_garage
  cargo_v build --locked --release -p s01-upload --all-targets
  local rc=0 t
  # dhat_25m legge i JSON di 1m e 10m: l'ordine conta.
  for t in dhat_1m dhat_10m dhat_25m pdfgen admission gc_race; do
    echo "=== TEST $t"
    if cargo_v test --locked --release -p s01-upload --test "$t" -- --test-threads=1; then
      echo "=== EXIT $t 0"
    else
      echo "=== EXIT $t 1"; rc=1
    fi
  done
  return $rc
}

runner_serve() {
  pin_env
  mkdir -p /work/tmp/srv
  exec /target/release/s01-upload --tmp-dir /work/tmp/srv "$@"
}

# URL del server con IP risolto (curl --resolve), da S01_TARGET_URL.
target_setup() {
  [[ -n ${S01_TARGET_URL-} ]] || die "S01_TARGET_URL assente"
  local ip
  ip=$(resolve "$SERVER") || die "risoluzione di $SERVER fallita"
  RESOLVE=(--resolve "$SERVER:8080:$ip")
  BASE=${S01_TARGET_URL%/upload}
  local i
  for (( i = 0; i < 120; i++ )); do
    if curl -sf "${RESOLVE[@]}" "$BASE/healthz" > /dev/null 2>&1; then return 0; fi
    sleep 0.5
  done
  die "healthz del server non pronta entro 60 s"
}

json_field() { # <json> <campo>
  python3 -c 'import json,sys; v=json.loads(sys.argv[1]).get(sys.argv[2]); print("" if v is None else v)' "$1" "$2"
}

upload_one() { # <file> → "<http_code> <time_total> <size_upload>"
  curl -sS -o /dev/null -w '%{http_code} %{time_total} %{size_upload}\n' "${RESOLVE[@]}" \
    -F "file=@$1" "$S01_TARGET_URL" || true
}
export -f upload_one

# client --adapter A --size L --workload W --c C --run R --rid RID [--label E]
# --label: nome della configurazione nei CSV (default = adapter); serve alle mitigazioni
# misurate dopo la matrice, che usano lo stesso adapter con un'altra configurazione.
runner_client() {
  local adapter size workload c run rid label=
  while (( $# )); do
    case $1 in
      --adapter) adapter=$2; shift 2 ;; --size) size=$2; shift 2 ;;
      --workload) workload=$2; shift 2 ;; --c) c=$2; shift 2 ;;
      --run) run=$2; shift 2 ;; --rid) rid=$2; shift 2 ;; --label) label=$2; shift 2 ;;
      *) die "client: $1" ;;
    esac
  done
  label=${label:-$adapter}
  cd /src
  pin_env
  wait_garage
  target_setup
  local probe=/target/release/probe raw=/src/s01-upload/results/raw tag="$label-$size-$workload-c$c-r$run"
  mkdir -p "$raw/$rid"
  "$probe" reset --adapter "$adapter" > /dev/null
  local before; before=$(curl -sf "${RESOLVE[@]}" "$BASE/bench/stats")
  local fx=/work/tmp/fixtures/$size list="$raw/$rid/$tag.list"
  if [[ $workload == new ]]; then
    for (( i = 1; i <= REQ; i++ )); do printf '%s/%s-%03d.pdf\n' "$fx" "$size" "$i"; done > "$list"
  else
    local body; body=$(printf '%s/%s-%03d.pdf' "$fx" "$size" 1)
    upload_one "$body" > "$raw/$rid/$tag.prime"   # innesco escluso dalle statistiche
    for (( i = 1; i <= REQ; i++ )); do printf '%s\n' "$body"; done > "$list"
  fi
  export RESOLVE_STR="${RESOLVE[*]}" S01_TARGET_URL
  # xargs lancia una shell per richiesta: RESOLVE ricostruito da RESOLVE_STR.
  xargs -P "$c" -I{} bash -c 'RESOLVE=($RESOLVE_STR); upload_one "$1"' _ {} < "$list" > "$raw/$rid/$tag.times"
  local after; after=$(curl -sf "${RESOLVE[@]}" "$BASE/bench/stats")
  curl -sf "${RESOLVE[@]}" "$BASE/bench/phases" > "$raw/$rid/$tag.phases.json" || true
  printf '%s\n%s\n' "$before" "$after" > "$raw/$rid/$tag.stats.json"
  local agg cnt
  agg=$("$probe" aggregate --input "$raw/$rid/$tag.times")
  cnt=$("$probe" count --adapter "$adapter")
  read -r req p50 p95 p99 s2 s4 s5 <<< "$agg"
  read -r rows distinct objects <<< "$cnt"
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' \
    "$rid" "$label" "$workload" "${SIZE_BYTES[$size]}" "$c" "$run" "$req" "$p50" "$p95" "$p99" \
    "$s2" "$s4" "$s5" "$distinct" "$objects" >> "$raw/adapters.tsv"
  if [[ $workload == new && $c == 1 ]]; then
    printf '%s\t%s\tfull\t%s\t%s\t\t%s\t%s\n' "$rid" "$label" "${SIZE_BYTES[$size]}" "$run" \
      "$(json_field "$after" vm_hwm_kb)" "$(json_field "$after" cgroup_peak_bytes)" >> "$raw/mem_full.tsv"
  fi
  echo "CLIENT $tag: req=$req p50=$p50 p95=$p95 p99=$p99 2xx=$s2 4xx=$s4 5xx=$s5 distinct=$distinct objects=$objects rows=$rows"
}

# dedup-client --adapter A --session N --rid RID [--label E]
runner_dedup_client() {
  local adapter session rid label=
  while (( $# )); do
    case $1 in
      --adapter) adapter=$2; shift 2 ;; --session) session=$2; shift 2 ;;
      --rid) rid=$2; shift 2 ;; --label) label=$2; shift 2 ;; *) die "dedup-client: $1" ;;
    esac
  done
  label=${label:-$adapter}
  cd /src
  pin_env
  wait_garage
  target_setup
  local probe=/target/release/probe raw=/src/s01-upload/results/raw fx=/work/tmp/fixtures/10m
  mkdir -p "$raw/dedup"
  "$probe" reset --adapter "$adapter" > /dev/null
  local body; body=$(printf '%s/10m-%03d.pdf' "$fx" 201)
  upload_one "$body" > /dev/null                      # innesco escluso
  local i
  for (( i = 202; i <= 211; i++ )); do upload_one "$(printf '%s/10m-%03d.pdf' "$fx" "$i")" > /dev/null; done
  for (( i = 0; i < 10; i++ )); do upload_one "$body" > /dev/null; done
  local out="$raw/dedup/$label-s$session.tsv" pair blk n=0 first
  : > "$out"
  run_block() { # <gruppo> <blocco>
    local group=$1 b=$2 k f line code t
    for (( k = 0; k < 10; k++ )); do
      if [[ $group == new ]]; then n=$((n + 1)); f=$(printf '%s/10m-%03d.pdf' "$fx" "$n"); else f=$body; fi
      line=$(upload_one "$f"); read -r code t _ <<< "$line"
      printf '%s\t%s\ts%s-b%02d-%d\t%s\t%s\n' "$label" "$([[ $group == dedup ]] && echo true || echo false)" \
        "$session" "$b" "$k" "$(awk -v t="$t" 'BEGIN {printf "%.3f", t * 1000}')" "$code" >> "$out"
    done
  }
  blk=0
  for (( pair = 0; pair < 20; pair++ )); do
    first=$(printf 's01-dedup%s%s' "$session" "$pair" | sha256sum | cut -c64)
    if (( 0x$first % 2 == 0 )); then
      run_block new $blk; run_block dedup $((blk + 1))
    else
      run_block dedup $blk; run_block new $((blk + 1))
    fi
    blk=$((blk + 2))
  done
  echo "DEDUP-CLIENT $label sessione $session: $(wc -l < "$out") richieste, non 2xx: $(awk -F'\t' '$5 !~ /^2/' "$out" | wc -l)"
}

# correctness-client --phase normal|fault --rid RID
runner_correctness_client() {
  local phase rid
  while (( $# )); do
    case $1 in --phase) phase=$2; shift 2 ;; --rid) rid=$2; shift 2 ;; *) die "correctness: $1" ;; esac
  done
  cd /src
  pin_env
  wait_garage
  target_setup
  local probe=/target/release/probe raw=/src/s01-upload/results/raw fx=/work/tmp/fixtures out
  out="$raw/correctness.tsv"
  tmpfiles() { find /work/tmp/srv -type f 2>/dev/null | wc -l; }
  # Massimo di byte del campo letti dal server prima di un 413 del contatore manuale
  # (GET /bench/phases, massimo dall'avvio del processo).
  srv_rejected() { json_field "$(curl -sf "${RESOLVE[@]}" "$BASE/bench/phases")" max_bytes_read_rejected; }
  case_row() { # <caso> <file> <stato atteso> [<status json atteso>]
    local name=$1 f=$2 exp=$3 exp_status=${4-} body code up sha size st pass=true rej0 rej1 read_b
    "$probe" reset --adapter garage > /dev/null
    body=$(mktemp)
    rej0=$(srv_rejected)
    read -r code up <<< "$(curl -sS -o "$body" -w '%{http_code} %{size_upload}' "${RESOLVE[@]}" -F "file=@$f" "$S01_TARGET_URL" || echo '000 0')"
    sleep 0.5
    rej1=$(srv_rejected)
    # 413: byte letti dal server (contatore lato server; invariato = rifiuto prima di
    # leggere il campo, per Content-Length oltre il limite di corpo), soglia D-04 con un
    # chunk da al più 64 KiB. Altri casi: byte inviati da curl.
    if [[ $code == 413 ]]; then
      if (( rej1 > rej0 )); then read_b=$rej1; else read_b=0; fi
      (( read_b <= 26214400 + 65536 + 65536 )) || pass=false
    else
      read_b=$up
    fi
    read -r rows _ objects <<< "$("$probe" count --adapter garage)"
    [[ $code == "$exp" ]] || pass=false
    if [[ $code == 200 ]]; then
      sha=$(json_field "$(cat "$body")" sha256)
      st=$(json_field "$(cat "$body")" status)
      [[ $sha == "$(sha256sum "$f" | cut -d' ' -f1)" ]] || pass=false
      [[ -z $exp_status || $st == "$exp_status" ]] || pass=false
      [[ $rows == 1 && $objects == 1 ]] || pass=false
    else
      [[ $rows == 0 && $objects == 0 ]] || pass=false
    fi
    [[ $(tmpfiles) == 0 ]] || pass=false
    printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$(stat -c %s "$f")" "$exp${exp_status:+/$exp_status}" \
      "$code${st:+/$st}" "$read_b" "$rows" "$objects" "$(tmpfiles)" "$pass" >> "$out"
    echo "CASE $name: atteso $exp${exp_status:+/$exp_status}, ottenuto $code${st:+/$st}, letti $read_b (curl $up), righe $rows, oggetti $objects, pass $pass"
    rm -f "$body"
  }
  if [[ $phase == normal ]]; then
    : > "$out"
    case_row 25MiB-1 "$fx/edge-399/edge-399-001.pdf" 200 accepted
    case_row 25MiB "$fx/edge-400/edge-400-001.pdf" 200 accepted
    # 100 MiB prima di 25MiB+1: il contatore lato server è un massimo dall'avvio.
    case_row 100MiB "$fx/fmt/art-100m.pdf" 413
    case_row 25MiB+1 "$fx/edge-401/edge-401-001.pdf" 413
    case_row pdf-sintetico "$fx/fmt/synthetic.pdf" 200 accepted
    case_row docx-sintetico "$fx/fmt/synthetic.docx" 200 accepted
    case_row markdown "$fx/fmt/nota.md" 200 accepted
    case_row polyglot "$fx/fmt/polyglot.pdf" 415
    case_row magic-mismatch "$fx/fmt/magic-mismatch.pdf" 415
    case_row zip-bomb "$fx/fmt/zipbomb.docx" 422
    case_row dtd "$fx/fmt/dtd.docx" 422
    case_row encrypted "$fx/fmt/encrypted.pdf" 422
    case_row javascript "$fx/fmt/javascript.pdf" 200 quarantined
    case_row javascript-offuscato "$fx/fmt/javascript-offuscato.pdf" 200 quarantined
    case_row launch "$fx/fmt/launch.pdf" 200 quarantined
    case_row embedded "$fx/fmt/embedded.pdf" 200 quarantined
    if [[ -f $fx/fmt/real.pdf ]]; then case_row pdf-reale "$fx/fmt/real.pdf" 200; fi
    if [[ -f $fx/fmt/real.docx ]]; then case_row docx-reale "$fx/fmt/real.docx" 200; fi
    case_row troncato "$fx/fmt/truncated.pdf" 422
    # Duplicato: due upload dello stesso corpo, un solo oggetto fisico, stessa forma.
    "$probe" reset --adapter garage > /dev/null
    local f="$fx/10m/10m-001.pdf" b1 b2 c1 c2 k1 k2 pass=true
    b1=$(mktemp); b2=$(mktemp)
    c1=$(curl -sS -o "$b1" -w '%{http_code}' "${RESOLVE[@]}" -F "file=@$f" "$S01_TARGET_URL")
    c2=$(curl -sS -o "$b2" -w '%{http_code}' "${RESOLVE[@]}" -F "file=@$f" "$S01_TARGET_URL")
    k1=$(python3 -c 'import json,sys; print(",".join(sorted(json.load(open(sys.argv[1])))))' "$b1")
    k2=$(python3 -c 'import json,sys; print(",".join(sorted(json.load(open(sys.argv[1])))))' "$b2")
    read -r rows _ objects <<< "$("$probe" count --adapter garage)"
    [[ $c1 == 200 && $c2 == 200 && $k1 == "$k2" && $rows == 2 && $objects == 1 ]] || pass=false
    [[ $(json_field "$(cat "$b1")" upload_id) != "$(json_field "$(cat "$b2")" upload_id)" ]] || pass=false
    printf 'duplicato\t%s\t200/200\t%s/%s\t%s\t%s\t%s\t%s\t%s\n' "$(stat -c %s "$f")" "$c1" "$c2" \
      "$((2 * $(stat -c %s "$f")))" "$rows" "$objects" "$(tmpfiles)" "$pass" >> "$out"
    echo "CASE duplicato: $c1/$c2 campi [$k1] [$k2] righe $rows oggetti $objects pass $pass"
    rm -f "$b1" "$b2"
    # Interrotto: il client chiude a metà (2 s a 1 MiB/s su 10 MiB).
    "$probe" reset --adapter garage > /dev/null
    local up
    up=$(curl -sS -o /dev/null -w '%{size_upload}' --limit-rate 1M --max-time 2 "${RESOLVE[@]}" \
      -F "file=@$f" "$S01_TARGET_URL" 2>/dev/null || true)
    sleep 3
    read -r rows _ objects <<< "$("$probe" count --adapter garage)"
    pass=true; [[ $rows == 0 && $objects == 0 && $(tmpfiles) == 0 ]] || pass=false
    printf 'interrotto\t%s\tchiusura client\tnessuna risposta\t%s\t%s\t%s\t%s\t%s\n' "$(stat -c %s "$f")" \
      "${up:-0}" "$rows" "$objects" "$(tmpfiles)" "$pass" >> "$out"
    echo "CASE interrotto: letti $up, righe $rows, oggetti $objects, tempfile $(tmpfiles), pass $pass"
  else
    # Server avviato con --fault after_put: abort dopo il PUT, prima della transazione.
    "$probe" reset --adapter garage > /dev/null
    local f="$fx/1m/1m-001.pdf" code
    code=$(curl -sS -o /dev/null -w '%{http_code}' "${RESOLVE[@]}" -F "file=@$f" "$S01_TARGET_URL" 2>/dev/null || true)
    sleep 1
    read -r rows _ before <<< "$("$probe" count --adapter garage)"
    "$probe" gc --adapter garage --n-hours 0
    read -r rows2 _ objects <<< "$("$probe" count --adapter garage)"
    local pass=true; [[ $rows == 0 && $before == 1 && $rows2 == 0 && $objects == 0 ]] || pass=false
    printf 'crash-fra-put-e-tx\t%s\tnessuna riga; orfano raccolto da gc N=0\t%s; orfani prima del gc %s\t%s\t%s\t%s\t%s\t%s\n' \
      "$(stat -c %s "$f")" "${code:-000}" "$before" "$(stat -c %s "$f")" "$rows2" "$objects" "$(tmpfiles)" "$pass" >> "$out"
    echo "CASE crash-fra-put-e-tx: http $code, righe $rows, oggetti prima del gc $before, dopo $objects, tempfile lasciati dall'abort $(tmpfiles), pass $pass"
    # Il tempfile lasciato dall'abort è un risultato (registrato sopra), poi si rimuove.
    find /work/tmp/srv -type f -delete 2>/dev/null || true
  fi
}

# ============================================================== modalità HOST

S=spikes/tools/session.sh
RESULTS=spikes/s01-upload/results

host_session() { # <etichetta> : prenota, trap, servizi
  [[ -n ${CKA_SPIKES_ENV-} ]] || CKA_SPIKES_ENV=$HOME/.cka-spikes/spikes.env
  [[ -f $CKA_SPIKES_ENV ]] || die "file segreti assente: $CKA_SPIKES_ENV"
  # Nell'interpolazione di compose l'ambiente della shell vince su --env-file: una
  # variabile omonima dell'host (es. DATABASE_URL di un altro progetto) arriverebbe al
  # runner al posto del valore degli spike. Si tolgono dall'ambiente le chiavi del file
  # segreti; si stampano solo i nomi.
  local k
  for k in $(grep -oE '^[A-Za-z_][A-Za-z0-9_]*' "$CKA_SPIKES_ENV" | sort -u); do
    if [[ -n ${!k+x} ]]; then echo "bench.sh: tolta dall'ambiente host la variabile $k (vale il file segreti)"; unset "$k"; fi
  done
  RID=$(bash "$S" new-id 01-04)
  local rc=0
  CKA_OWNER_PID=$$ bash spikes/tools/session.sh acquire 01-04 "$RID" --wait 3600 || rc=$?
  if (( rc == 3 )); then echo "bench.sh: finestra di misura occupata da un altro piano, nessuna azione"; exit 3; fi
  (( rc == 0 )) || die "acquire fallito ($rc)"
  trap 'bash "$S" down "$RID" || true; bash "$S" release "$RID" || true' EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  echo "bench.sh: sessione $RID ($1)"
}

host_services() {
  bash "$S" up "$RID" postgres garage
  bash "$S" ps "$RID" --assert-only postgres garage
}

# hw.json dal manifest di 01-02, normalizzato allo schema letto da common::manifest
# (difetto fra 01-01 e 01-02: cpu, ram_total, docker_desktop_resources e images hanno
# forme diverse). Copia originale in results/raw/hw-<run_id>.raw.json.
host_hw() {
  local curlv
  curlv=$(bash "$S" oneshot "$RID" -- curl --version 2>/dev/null | head -1 | awk '{print $2}' | tr -d '\r')
  mkdir -p "$RESULTS/raw"
  bash spikes/tools/hw-manifest.sh "$RESULTS/raw/hw-$RID.raw.json" --run-id "$RID" \
    --image pgvector/pgvector:0.8.7-pg18-trixie --image dxflrs/garage:v2.4.1 --tool "curl=$curlv"
  node -e '
    const fs = require("fs");
    const [src, dst] = process.argv.slice(1);
    const j = JSON.parse(fs.readFileSync(src, "utf8"));
    const out = Object.assign({}, j);
    const str = v => (v === null || v === undefined) ? null : (typeof v === "string" ? v : JSON.stringify(v));
    if (j.cpu && typeof j.cpu === "object") { out.cpu = `${j.cpu.model} (${j.cpu.cores} core, ${j.cpu.threads} thread)`; out.cpu_detail = j.cpu; }
    if (typeof j.ram_total === "number") { out.ram_total = String(j.ram_total); out.ram_total_bytes = j.ram_total; }
    for (const k of ["gpu", "cuda_driver", "wsl_version", "docker_version", "docker_desktop_resources", "rustc", "power_plan", "git_rev"]) out[k] = str(j[k]);
    if (j.docker_desktop_resources && typeof j.docker_desktop_resources === "object") out.docker_desktop_resources_detail = j.docker_desktop_resources;
    if (Array.isArray(j.images)) { out.images = Object.fromEntries(j.images.map(i => [i.ref, i.digest || ""])); out.images_detail = j.images; }
    out.normalized_by = "spikes/s01-upload/bench.sh host_hw (schema di common::manifest; originale in results/raw/)";
    fs.writeFileSync(dst, JSON.stringify(out, null, 2) + "\n");
  ' "$RESULTS/raw/hw-$RID.raw.json" "$RESULTS/hw.json"
  echo "bench.sh: hw.json sha256 $(sha256sum "$RESULTS/hw.json" | cut -d' ' -f1) (run_id $RID)"
}

host_build() {
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh build
}

start_server() { # <argomenti del server>
  bash "$S" run "$RID" "$SERVER" -- bash s01-upload/bench.sh serve "$@" > /dev/null
}

stop_server() {
  bash "$S" stop "$RID" "$SERVER" > /dev/null
}

record_run() { # <adapter> <size_bytes> <workload> <c> <run>
  local cid img
  cid=$(docker inspect --format '{{.Id}}' "$SERVER")
  img=$(docker inspect --format '{{.Image}}' "$SERVER")
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$RID" "$1" "$2" "$3" "$4" "$5" "$cid" "$img" \
    "$(git rev-parse --short HEAD)" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" >> "$RESULTS/raw/runs.tsv"
}

host_vendor() {
  local i
  for (( i = 1; i <= 15; i++ )); do
    if (cd spikes && cargo vendor --locked "$VENDOR_DIR" > /dev/null); then echo "VENDOR ok al tentativo $i"; return 0; fi
    echo "VENDOR fallito al tentativo $i"; sleep 20
  done
  die "cargo vendor fallito dopo 15 tentativi"
}

host_tests() {
  host_session tests
  host_services
  host_hw
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh tests
}

host_matrix() {
  local run=$1
  [[ $run =~ ^[123]$ ]] || die "run 1..3"
  host_session "matrix run $run"
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  local order=("${SIZES_LABELS[@]}")
  if (( run == 2 )); then order=(25m 24m 10m 1m); fi   # crescente, decrescente, crescente
  local size adapter workload c
  for size in "${order[@]}"; do
    for adapter in "${ADAPTERS[@]}"; do
      for workload in "${WORKLOADS[@]}"; do
        for c in "${CONCURRENCY[@]}"; do
          bash "$S" ps "$RID" --assert-only postgres garage runner > /dev/null
          start_server --adapter "$adapter" --bench
          record_run "$adapter" "${SIZE_BYTES[$size]}" "$workload" "$c" "$run"
          bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh client --adapter "$adapter" --size "$size" \
            --workload "$workload" --c "$c" --run "$run" --rid "$RID" || echo "bench.sh: client fallito ($adapter $size $workload c$c)"
          stop_server
        done
      done
    done
  done
}

host_dedup() {
  local session=$1
  [[ $session =~ ^[123]$ ]] || die "sessione 1..3"
  host_session "dedup sessione $session"
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  local adapter
  for adapter in garage pgbytea null; do
    start_server --adapter "$adapter" --bench
    bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh dedup-client --adapter "$adapter" --session "$session" --rid "$RID"
    stop_server
  done
}

# Mitigazioni misurate DOPO la matrice (soglie e protocollo invariati, RISK-001): stesso
# adapter con un'altra configurazione, etichetta propria nei CSV. Dopo ogni riga il log
# del proprio server va in results/raw/<run_id>/<tag>.log (diagnosi dei 500 di Garage).
# Configurazioni: etichetta|argomenti del server.
MITIG_CONFIGS=(
  "garage-p5k1|--adapter garage --bench --garage-part-mib 5 --garage-part-concurrency 1"
  "garage-p2k1|--adapter garage --bench --garage-part-mib 2 --garage-part-concurrency 1"
)

mitig_row() { # <etichetta> <argomenti server> <size> <workload> <c> <run>
  local label=$1 sargs=$2 size=$3 workload=$4 c=$5 run=$6
  bash "$S" ps "$RID" --assert-only postgres garage runner > /dev/null
  # shellcheck disable=SC2086  # argomenti del server: parole fisse di MITIG_CONFIGS
  start_server $sargs
  record_run "$label" "${SIZE_BYTES[$size]}" "$workload" "$c" "$run"
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh client --adapter garage --label "$label" \
    --size "$size" --workload "$workload" --c "$c" --run "$run" --rid "$RID" \
    || echo "bench.sh: client fallito ($label $size $workload c$c)"
  docker logs "$SERVER" > "$RESULTS/raw/$RID/$label-$size-$workload-c$c-r$run.log" 2>&1 || true
  stop_server
}

host_mitig() {
  host_session "mitigazioni Garage"
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  mkdir -p "$RESULTS/raw/$RID"
  # Diagnosi: configurazione della matrice, riga con più 500 (25 MiB, dedup, c = 16).
  mitig_row garage-diag "--adapter garage --bench" 25m dedup 16 1
  local run cfg label sargs size order
  for run in 1 2 3; do
    order=("${SIZES_LABELS[@]}")
    if (( run == 2 )); then order=(25m 24m 10m 1m); fi
    for cfg in "${MITIG_CONFIGS[@]}"; do
      label=${cfg%%|*}; sargs=${cfg#*|}
      for size in "${order[@]}"; do mitig_row "$label" "$sargs" "$size" new 1 "$run"; done
      mitig_row "$label" "$sargs" 10m new 4 "$run"
      mitig_row "$label" "$sargs" 10m dedup 16 "$run"
      mitig_row "$label" "$sargs" 25m dedup 16 "$run"
    done
  done
}

# Canale laterale delle configurazioni mitigate, con il protocollo pre-registrato
# (sessione pulita per ogni <sessione>, stesse regole di host_dedup). Garage con parti da
# 2 MiB e una parte in volo: l'unica configurazione mitigata che sta sotto la soglia di
# VmHWM in `host mitig` (sessione 303e), quindi quella che l'Esito propone.
host_dedup_mitig() {
  local session=$1
  [[ $session =~ ^[123]$ ]] || die "sessione 1..3"
  host_session "dedup mitigato sessione $session"
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  start_server --adapter pgbytea --bench --pg-conflict update
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh dedup-client --adapter pgbytea \
    --label pgbytea-update --session "$session" --rid "$RID"
  stop_server
  start_server --adapter garage --bench --garage-part-mib 2 --garage-part-concurrency 1
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh dedup-client --adapter garage \
    --label garage-p2k1 --session "$session" --rid "$RID"
  stop_server
}

host_correctness() {
  host_session correctness
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  start_server --adapter garage --bench
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh correctness-client --phase normal --rid "$RID"
  stop_server
  start_server --adapter garage --bench --fault after_put
  bash "$S" oneshot "$RID" -- bash s01-upload/bench.sh correctness-client --phase fault --rid "$RID"
  stop_server
}

host_extras() {
  host_session extras
  host_services
  host_hw
  host_build
  bash "$S" oneshot "$RID" -- bash s01-upload/fixtures.sh
  bash "$S" oneshot "$RID" -- bash -c '. s01-upload/bench.sh lib && pin_env && /target/release/probe storage --fixtures /work/tmp/fixtures/10m --docs 30 --out /src/s01-upload/results/storage.csv && /target/release/probe gc-bench --objects 1000 --orphans 100 --out /src/s01-upload/results/gc.csv'
}

host_finalize() {
  local p="cargo run -q --manifest-path spikes/Cargo.toml --locked --release -p s01-upload --bin probe --"
  local r=$RESULTS raw=$RESULTS/raw
  $p tsv2csv --input "$raw/runs.tsv" --out "$r/runs.csv" \
    --header run_id,adapter,size_bytes,workload,concurrency,run,server_container_id,runner_image_digest,git_rev,started_utc
  $p tsv2csv --input "$raw/adapters.tsv" --out "$r/adapters.csv" \
    --header run_id,adapter,workload,size_bytes,concurrency,run,req,p50_ms,p95_ms,p99_ms,status_2xx,status_4xx,status_5xx,distinct_sha,objects_after
  # Memoria: righe streaming dai JSON dhat, righe full dalla matrice (carico new, c = 1).
  : > "$raw/mem.tsv"
  local l
  for l in 1m 10m 25m; do
    node -e 'const j=JSON.parse(require("fs").readFileSync(process.argv[1]));
      process.stdout.write(["dhat-test","null","streaming",j.size_bytes,1,j.max_bytes,"",""].join("\t")+"\n")' \
      "$r/dhat_$l.json" >> "$raw/mem.tsv"
  done
  cat "$raw/mem_full.tsv" >> "$raw/mem.tsv"
  $p tsv2csv --input "$raw/mem.tsv" --out "$r/mem_by_size.csv" \
    --header run_id,adapter,mode,size_bytes,run,dhat_max_bytes,rss_hwm_kb,cgroup_peak_bytes
  # Canale laterale: solo 2xx (criterio di esclusione pre-registrato).
  cat "$raw"/dedup/*.tsv | awk -F'\t' -v OFS='\t' '$5 ~ /^2/ {print $1, $2, $3, $4}' > "$raw/dedup.tsv"
  $p tsv2csv --input "$raw/dedup.tsv" --out "$r/dedup_timing.csv" --header adapter,dedup_hit,req_id,latency_ms
  $p tsv2csv --input "$raw/correctness.tsv" --out "$r/correctness.csv" \
    --header case,size_bytes,expected_status,actual_status,bytes_read_before_reject,db_rows_after,objects_after,tmpfiles_after,pass
  local a s
  for a in garage pgbytea null pgbytea-update garage-p2k1; do
    for s in 1 2 3; do $p dedup-ci --csv "$r/dedup_timing.csv" --adapter "$a" --session "$s" || true; done
  done
}

# ============================================================== dispatch

mode=${1-}
shift || true
case $mode in
  lib) return 0 2>/dev/null || exit 0 ;;   # solo funzioni, per `. bench.sh lib`
  build) runner_build ;;
  tests) runner_tests ;;
  serve) runner_serve "$@" ;;
  client) runner_client "$@" ;;
  dedup-client) runner_dedup_client "$@" ;;
  correctness-client) runner_correctness_client "$@" ;;
  host)
    cd "$(git rev-parse --show-toplevel)"
    sub=${1-}; shift || true
    case $sub in
      vendor) host_vendor ;;
      tests) host_tests ;;
      matrix) host_matrix "$@" ;;
      dedup) host_dedup "$@" ;;
      mitig) host_mitig ;;
      dedup-mitig) host_dedup_mitig "$@" ;;
      correctness) host_correctness ;;
      extras) host_extras ;;
      finalize) host_finalize ;;
      *) die "host: vendor|tests|matrix <run>|dedup <sessione>|mitig|dedup-mitig <sessione>|correctness|extras|finalize" ;;
    esac
    ;;
  *) die "uso: bench.sh host <...> | build | tests | serve | client | dedup-client | correctness-client" ;;
esac
