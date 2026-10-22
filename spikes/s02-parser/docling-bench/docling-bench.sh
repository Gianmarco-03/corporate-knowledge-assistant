#!/usr/bin/env bash
# spikes/s02-parser/docling-bench/docling-bench.sh — misure di Docling (CPU e GPU) e prove di
# confine del servizio, sull'host (il runner non vede Docker). Proprietario: piano 01-08
# (WIRING W18, cartella-strumento del piano).
#
# Gira solo dentro una sessione prenotata:
#   bash spikes/tools/session.sh with 01-08 -- bash spikes/s02-parser/docling-bench/docling-bench.sh <cmd> …
#
# Sottocomandi:
#   hash-inputs <jobs.jsonl>          sha256 degli input dei job extract_pdf (inputs.sha256)
#   measure <gpu|cpu> <pages.tsv>     pagine-tabella: servizio ricreato a ogni documento,
#                                     riscaldamento escluso, 3 ripetizioni per pagina
#   probes <gpu|cpu>                  timeout, disconnessione, page_range e marcatore di stato
#
# Regole (piano 01-08, deepsearch_2): il servizio Docling si riporta a riposo solo con
# `session.sh stop` del suo container (da `session.sh cid`) seguito da `session.sh up` del
# solo servizio, a ogni documento e subito dopo ogni timeout, disconnessione o errore; il
# timeout del client e la chiusura della connessione non fermano il lavoro già accettato.
# Lo spegnimento di fine sessione lo fa `session.sh with`. Nessun volume toccato.
# Ogni input passa prima da extract_pdf con esito ok (lo verifica docling-client) e deve
# avere ancora lo sha256 registrato da hash-inputs (lo verifica questo script).
#
# **Modifica:** Claude (gsd-executor 01-08) · 2026-10-22 · misure Docling CPU/GPU e prove di confine

set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SPIKES=$(cd "$HERE/../.." && pwd)
TOOLS=$SPIKES/tools
RID=${CKA_RUN_ID:?docling-bench.sh gira dentro session.sh with (CKA_RUN_ID assente)}

# Percorsi lato host e lato runner (/src = spikes/).
HOST_RAW=$SPIKES/s02-parser/results/raw
HOST_OUT=$SPIKES/s02-parser/results/extract
JOBS=/src/s02-parser/results/raw/jobs/extract.jsonl
RUNS=/src/s02-parser/results/raw/extract/runs.jsonl
SHAS=$HOST_RAW/jobs/inputs.sha256
CLIENT=/target/release/docling-client
DOCLING_CSV=$HOST_OUT/docling.csv
ISO_CSV=$HOST_OUT/docling_isolation.csv
DOCLING_HEADER=device,doc_id,page_range,run,seconds,seconds_per_page,vram_peak_mib,tables_found,status
ISO_HEADER=probe,device,run,pages_sent,page_range,client_exit,status,cpu_pct_0s,cpu_pct_10s,mem_mib_0s,mem_mib_10s,container_peak_mib,restarted,next_benign_status,next_benign_s

# Input di riferimento delle prove (percorsi del worker), sovrascrivibili dall'ambiente.
WARMUP_INPUT=${BENCH_WARMUP_INPUT:-}
BENIGN_INPUT=${BENCH_BENIGN_INPUT:-}
BIG_INPUT=${BENCH_BIG_INPUT:-}
OUTSIDE_INPUT=${BENCH_OUTSIDE_INPUT:-}
BIG_PAGES=${BENCH_BIG_PAGES:-400}
REPS=${BENCH_REPS:-3}
# Numero della ripetizione delle prove di confine (colonna run di docling_isolation.csv).
PROBE_RUN=${BENCH_PROBE_RUN:-1}

log() { printf 'docling-bench: %s\n' "$*" >&2; }
die() { local code=$1; shift; printf 'docling-bench: ERRORE: %s\n' "$*" >&2; exit "$code"; }

service_of() {
  case $1 in
    gpu) echo docling ;;
    cpu) echo docling-cpu ;;
    *) die 2 "dispositivo sconosciuto: $1 (gpu|cpu)" ;;
  esac
}

# Percorso del worker → percorso dell'host.
host_path() {
  case $1 in
    /data/*) printf '%s/data/%s\n' "$SPIKES" "${1#/data/}" ;;
    /results/raw/*) printf '%s/%s\n' "$HOST_RAW" "${1#/results/raw/}" ;;
    *) die 2 "input fuori dalle radici: $1" ;;
  esac
}

ensure_header() {
  local file=$1 header=$2
  if [[ ! -s $file ]]; then
    printf '%s\n' "$header" > "$file"
  elif [[ $(head -n 1 "$file" | tr -d '\r') != "$header" ]]; then
    die 1 "header inatteso in $file"
  fi
}

# Lo sha256 dell'input coincide con quello registrato all'estrazione.
check_sha() {
  local input=$1 want got
  want=$(awk -v p="$input" '$2 == p {print $1}' "$SHAS" | head -n 1)
  [[ -n $want ]] || die 1 "input senza sha256 registrato: $input"
  got=$(sha256sum "$(host_path "$input")" | cut -d' ' -f1 | tr -d '\\')
  [[ $got == "$want" ]] || die 1 "sha256 cambiato dopo l'estrazione: $input"
}

# docling-client nel runner; stampa la riga JSON dell'esito, ritorna il codice del client.
client() {
  bash "$TOOLS"/session.sh oneshot "$RID" -- "$CLIENT" \
    --extract-runs "$RUNS" --extract-jobs "$JOBS" \
    --data-root /src/data --results-raw /src/s02-parser/results/raw "$@" 2>/dev/null </dev/null | tail -n 1
  return "${PIPESTATUS[0]}"
}

# Campo di una riga JSON (stdin) con node.
json_field() { node -e 'let s="";process.stdin.on("data",d=>s+=d).on("end",()=>{try{const v=JSON.parse(s)[process.argv[1]];console.log(v===null||v===undefined?"":v)}catch(e){console.log("")}})' "$1"; }

# CPU% e memoria (MiB) del container, una lettura di docker stats.
stats_of() {
  local cid=$1 line
  line=$(docker stats --no-stream --format '{{.CPUPerc}}|{{.MemUsage}}' "$cid" 2>/dev/null || true)
  awk -F'|' '{
    cpu=$1; sub(/%/,"",cpu);
    split($2, m, " / "); v=m[1]; n=v; sub(/[A-Za-z]+$/,"",n); u=v; sub(/^[0-9.]+/,"",u);
    f=1; if(u=="GiB")f=1024; else if(u=="KiB")f=1/1024; else if(u=="B")f=1/1048576; else if(u=="kB")f=1/1024; else if(u=="MB")f=1; else if(u=="GB")f=1000;
    printf "%s %.1f\n", cpu, n*f
  }' <<< "${line:-0%|0MiB / 0MiB}"
}

# Picco di memoria del cgroup del container (MiB), se il kernel espone memory.peak.
peak_of() {
  local svc=$1 v
  v=$(bash "$TOOLS"/session.sh exec "$RID" "$svc" -- cat /sys/fs/cgroup/memory.peak 2>/dev/null | tr -d '\r' || true)
  if [[ $v =~ ^[0-9]+$ ]]; then awk -v b="$v" 'BEGIN{printf "%.1f", b/1048576}'; else echo ""; fi
}

# Attende che il servizio risponda (GET /version) e annota la versione una volta.
wait_ready() {
  local device=$1 i
  for i in $(seq 1 60); do
    if bash "$TOOLS"/session.sh oneshot "$RID" -- "$CLIENT" --extract-runs "$RUNS" --extract-jobs "$JOBS" --device "$device" \
        --get version --data-root /src/data --results-raw /src/s02-parser/results/raw \
        --out-prefix "/src/s02-parser/results/raw/docling/$device/version" >/dev/null 2>&1; then
      return 0
    fi
    sleep 3
  done
  die 1 "servizio $device non pronto dopo 60 tentativi"
}

# Fine delle misure del dispositivo: il container si ferma, il down lo fa session.sh with.
finish() {
  local svc cid
  svc=$(service_of "$1")
  cid=$(bash "$TOOLS"/session.sh cid "$RID" "$svc")
  bash "$TOOLS"/session.sh stop "$RID" "$cid" >&2
}

# Pronto e riscaldato: attesa di GET /version e una richiesta di riscaldamento esclusa.
warm() {
  local device=$1
  wait_ready "$device"
  if [[ -n $WARMUP_INPUT ]]; then
    client --device "$device" --input "$WARMUP_INPUT" --page-range 1-1 >/dev/null || true
  fi
}

# Avvio del solo servizio del dispositivo; stampa i secondi fino al servizio riscaldato.
start() {
  local device=$1 svc t0
  svc=$(service_of "$device")
  t0=$(date +%s.%N)
  bash "$TOOLS"/session.sh up "$RID" "$svc" >&2
  bash "$TOOLS"/session.sh ps "$RID" --assert-only "$svc" >&2
  warm "$device"
  awk -v a="$t0" -v b="$(date +%s.%N)" 'BEGIN{printf "%.2f", b-a}'
}

# Riavvio: stop del container del servizio (id da cid) e up del solo servizio; stampa i
# secondi dallo stop al servizio riscaldato e li annota in restarts.tsv.
restart() {
  local device=$1 svc cid t0 dt
  svc=$(service_of "$device")
  t0=$(date +%s.%N)
  cid=$(bash "$TOOLS"/session.sh cid "$RID" "$svc")
  bash "$TOOLS"/session.sh stop "$RID" "$cid" >&2
  bash "$TOOLS"/session.sh up "$RID" "$svc" >&2
  bash "$TOOLS"/session.sh ps "$RID" --assert-only "$svc" >&2
  warm "$device"
  dt=$(awk -v a="$t0" -v b="$(date +%s.%N)" 'BEGIN{printf "%.2f", b-a}')
  printf '%s	%s	%s
' "$device" "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$dt" >> "$HOST_RAW/docling/$device/restarts.tsv"
  printf '%s' "$dt"
}

vram_idle() {
  nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits | head -n 1 | tr -d ' \r'
}

cmd_hash_inputs() {
  local jobs=${1:?uso: hash-inputs <jobs.jsonl lato host>} input
  mkdir -p "$(dirname "$SHAS")"
  : > "$SHAS"
  while IFS= read -r input; do
    printf '%s  %s\n' "$(sha256sum "$(host_path "$input")" | cut -d' ' -f1 | tr -d '\\')" "$input" >> "$SHAS"
  done < <(node -e 'require("fs").readFileSync(0,"utf8").split("\n").filter(Boolean).forEach(l=>{const j=JSON.parse(l);if(j.op==="extract_pdf")console.log(j.input)})' < "$jobs")
  log "sha256 di $(wc -l < "$SHAS") input in $SHAS"
}

cmd_measure() {
  local device=${1:?uso: measure <gpu|cpu> <pages.tsv>} pages=${2:?pages.tsv}
  local svc idle=0 first=1 doc input page prev="" rows run line st sec tf exit_code n
  svc=$(service_of "$device")
  mkdir -p "$HOST_RAW/docling/$device"
  ensure_header "$DOCLING_CSV" "$DOCLING_HEADER"
  [[ $device == gpu ]] && idle=$(vram_idle)
  # Documenti nell'ordine del file: righe contigue con lo stesso input = un documento.
  local -a docs=()
  mapfile -t docs < <(cut -f2 "$pages" | awk '!seen[$0]++')
  for input in "${docs[@]}"; do
    check_sha "$input"
    if (( first )); then start "$device" >/dev/null; first=0; else restart "$device" >/dev/null; fi
    local sampler="" vfile="$HOST_RAW/docling/$device/vram-$$.txt"
    if [[ $device == gpu ]]; then
      nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits -lms 500 > "$vfile" &
      sampler=$!
    fi
    rows=""
    # Il file delle pagine si legge dal descrittore 3: compose run legge lo stdin e
    # altrimenti consumerebbe le righe successive.
    while IFS=$'\t' read -r doc in2 page <&3; do
      [[ $in2 == "$input" ]] || continue
      for run in $(seq 1 "$REPS"); do
        exit_code=0
        line=$(client --device "$device" --input "$input" --page-range "$page-$page" \
          --out-prefix "/src/s02-parser/results/raw/docling/$device/${doc}__p${page}__r${run}") || exit_code=$?
        st=$(json_field status <<< "$line"); sec=$(json_field seconds <<< "$line"); tf=$(json_field tables_found <<< "$line")
        [[ -n $st ]] || st=client_exit_$exit_code
        rows+="$device,$doc,$page-$page,$run,${sec:-},${sec:-},@VRAM@,${tf:-},$st"$'\n'
        if (( exit_code == 3 )) || [[ $st == error ]]; then
          log "$doc p$page r$run: $st (exit $exit_code), riavvio del servizio"
          restart "$device" >/dev/null
        fi
      done
    done 3< "$pages"
    local peak=""
    if [[ -n $sampler ]]; then
      kill "$sampler" 2>/dev/null || true
      wait "$sampler" 2>/dev/null || true
      peak=$(awk -v i="$idle" 'BEGIN{m=0} {gsub(/[ \r]/,""); if($1+0>m)m=$1+0} END{printf "%d", m-i}' "$vfile")
      rm -f "$vfile"
    fi
    printf '%s' "${rows//@VRAM@/$peak}" >> "$DOCLING_CSV"
    n=$(printf '%s' "$rows" | grep -c . || true)
    log "$input: $n righe, VRAM di picco ${peak:-n/d} MiB"
  done
  finish "$device"
}

# Una prova di confine: esegue il client, registra CPU e memoria a 0 s e 10 s, riavvia e
# misura una pagina benigna. Argomenti: probe device pages_sent page_range [opzioni client…]
probe() {
  local probe=$1 device=$2 sent=$3 range=$4; shift 4
  local svc cid exit_code=0 line st sec s0 s10 peak restart_s nb nb_st nb_s
  svc=$(service_of "$device")
  line=$(client --device "$device" "$@") || exit_code=$?
  st=$(json_field status <<< "$line"); [[ -n $st ]] || st=client_exit_$exit_code
  sec=$(json_field seconds <<< "$line")
  cid=$(bash "$TOOLS"/session.sh cid "$RID" "$svc")
  s0=$(stats_of "$cid")
  sleep 10
  s10=$(stats_of "$cid")
  peak=$(peak_of "$svc")
  log "$probe/$device: client $st (exit $exit_code); CPU/MiB 0 s = $s0, 10 s = $s10; picco $peak MiB"
  restart_s=$(restart "$device")
  nb=$(client --device "$device" --input "$BENIGN_INPUT" --page-range 1-1) || true
  nb_st=$(json_field status <<< "$nb"); nb_s=$(json_field seconds <<< "$nb")
  printf '%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,%s,true,%s,%s\n' \
    "$probe" "$device" "$PROBE_RUN" "$sent" "$range" "$exit_code" "$st" \
    "${s0%% *}" "${s10%% *}" "${s0##* }" "${s10##* }" "$peak" "${nb_st:-}" "${nb_s:-}" >> "$ISO_CSV"
  log "$probe/$device: riavvio in $restart_s s, pagina benigna $nb_st in $nb_s s"
  # Tempo della richiesta della prova e del riavvio (docling_isolation.csv non ha la colonna).
  printf '%s\t%s\t%s\t%s\t%s\t%s\t%s\t%s\n' "$probe" "$PROBE_RUN" "$sent" "$range" "$exit_code" "$st" "${sec:-}" "$restart_s" \
    >> "$HOST_RAW/docling/$device/probes.tsv"
}

cmd_probes() {
  local device=${1:?uso: probes <gpu|cpu>} svc cid input
  svc=$(service_of "$device")
  [[ -n $BENIGN_INPUT && -n $BIG_INPUT && -n $OUTSIDE_INPUT ]] \
    || die 2 "BENCH_BENIGN_INPUT, BENCH_BIG_INPUT e BENCH_OUTSIDE_INPUT sono obbligatorie"
  for input in "$BENIGN_INPUT" "$BIG_INPUT" "$OUTSIDE_INPUT"; do check_sha "$input"; done
  mkdir -p "$HOST_RAW/docling/$device"
  ensure_header "$ISO_CSV" "$ISO_HEADER"
  start "$device" >/dev/null

  # Marcatore di stato (TEST-4): scritto nel container del documento precedente, cercato nel
  # container ricreato, con processi e mount enumerati prima e dopo.
  local mark="cka-marker-$RID" before after mounts_b mounts_a pr_b pr_a present nb nb_st nb_s
  client --device "$device" --input "$BENIGN_INPUT" --page-range 1-1 >/dev/null || true
  bash "$TOOLS"/session.sh exec "$RID" "$svc" -- sh -c "echo 1 > /tmp/$mark && echo 1 > \"\$HOME/$mark\"" >&2
  cid=$(bash "$TOOLS"/session.sh cid "$RID" "$svc")
  before=$cid
  mounts_b=$(docker inspect --format '{{range .Mounts}}{{.Name}}:{{.Destination}}:{{.RW}} {{end}}' "$cid")
  pr_b=$(bash "$TOOLS"/session.sh exec "$RID" "$svc" -- sh -c 'ls -d /proc/[0-9]* | wc -l' | tr -d '\r ')
  restart "$device" >/dev/null
  after=$(bash "$TOOLS"/session.sh cid "$RID" "$svc")
  mounts_a=$(docker inspect --format '{{range .Mounts}}{{.Name}}:{{.Destination}}:{{.RW}} {{end}}' "$after")
  pr_a=$(bash "$TOOLS"/session.sh exec "$RID" "$svc" -- sh -c 'ls -d /proc/[0-9]* | wc -l' | tr -d '\r ')
  present=$(bash "$TOOLS"/session.sh exec "$RID" "$svc" -- sh -c "ls /tmp/$mark \"\$HOME/$mark\" 2>/dev/null | wc -l" | tr -d '\r ')
  nb=$(client --device "$device" --input "$BENIGN_INPUT" --page-range 1-1) || true
  nb_st=$(json_field status <<< "$nb"); nb_s=$(json_field seconds <<< "$nb")
  local mstatus=marker_absent
  [[ $present == 0 ]] || mstatus=marker_present
  [[ $before != "$after" ]] || mstatus=same_container
  printf 'state_marker,%s,%s,1,1-1,0,%s,,,,,,true,%s,%s\n' "$device" "$PROBE_RUN" "$mstatus" "${nb_st:-}" "${nb_s:-}" >> "$ISO_CSV"
  printf '%s\tcontainer %s -> %s\tprocessi %s -> %s\tmount prima: %s\tmount dopo: %s\tmarcatori trovati: %s\n' \
    "$device" "${before:0:12}" "${after:0:12}" "$pr_b" "$pr_a" "$mounts_b" "$mounts_a" "$present" \
    >> "$HOST_RAW/docling/$device/state-marker.tsv"
  log "state_marker/$device: $mstatus (processi $pr_b -> $pr_a; mount $mounts_a)"

  # (a) timeout del documento a 5 s su BIG_PAGES pagine.
  probe timeout "$device" "$BIG_PAGES" all --input "$BIG_INPUT" --document-timeout 5
  # (b) disconnessione del client dopo 3 s.
  probe disconnect "$device" "$BIG_PAGES" all --input "$BIG_INPUT" --abort-after 3
  # (c) page_range: la pagina da sola, poi la stessa pagina seguita da BIG_PAGES-1 pagine,
  # ciascuna su un container appena ricreato (il picco di memoria del cgroup parte da zero).
  probe outside_range "$device" 1 1-1 --input "$BENIGN_INPUT" --page-range 1-1
  probe outside_range "$device" "$BIG_PAGES" 1-1 --input "$OUTSIDE_INPUT" --page-range 1-1
  finish "$device"
}

main() {
  local sub=${1-}
  [[ -n $sub ]] || die 2 "uso: docling-bench.sh hash-inputs|measure|probes …"
  shift
  case $sub in
    hash-inputs) cmd_hash_inputs "$@" ;;
    measure) cmd_measure "$@" ;;
    probes) cmd_probes "$@" ;;
    *) die 2 "sottocomando sconosciuto: $sub" ;;
  esac
}

main "$@"
