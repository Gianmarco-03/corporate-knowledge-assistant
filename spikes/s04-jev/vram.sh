#!/usr/bin/env bash
# spikes/s04-jev/vram.sh — convivenza in VRAM di tei-embed, tei-rerank e docling sotto
# carico concorrente (piano 01-15, Task 1). Gira sull'host (Git Bash), dove nvidia-smi
# vede la GPU, e solo dentro una sessione prenotata:
#
#   CKA_SPIKES_ENV=<file> bash spikes/tools/session.sh with 01-15 -- bash spikes/s04-jev/vram.sh
#
# Una configurazione di batch per invocazione: TEI_MAX_BATCH_TOKENS e
# TEI_RERANK_MAX_BATCH_TOKENS vengono dal file di CKA_SPIKES_ENV, lo stesso che session.sh
# passa a compose (dc() fa prevalere i valori del file sull'ambiente). Per ridurre il
# batch si rilancia lo script con un file in cui i due valori sono più bassi.
#
# Fasi (colonna scenario = <configurazione>-<fase>), campionamento continuo ogni 500 ms:
#   desktop       nessun servizio avviato: quota del desktop
#   warm-<svc>    un servizio alla volta, avviato per nome e caricato da solo per VRAM_WARM_S
#   load          i tre carichi insieme per VRAM_DURATION_S (default 180 s)
#   after         qualche secondo a riposo
# La quota di un servizio è picco − minimo della sua fase warm (su WSL2 nvidia-smi non vede
# i processi dei container); oom = true su tutte le righe se log o carichi riportano un
# errore di memoria o un servizio non è più in esecuzione.
#
# Servizi solo con session.sh up/ps/cid/oneshot; lo spegnimento lo fa session.sh with
# (down per etichetta cka.run_id), anche su errore o interruzione. Nessun endpoint letterale:
# il runner legge TEI_EMBED_URL, TEI_RERANK_URL e DOCLING_URL dal proprio ambiente.
# Variabili facoltative: VRAM_DURATION_S, VRAM_WARM_S, VRAM_SCENARIO, VRAM_OUT,
# VRAM_ACCEPT_SHA (sha dei candidati rigenerati), VRAM_DOCLING_INPUT, VRAM_DOCLING_PAGES,
# VRAM_DOCLING_JOBS, VRAM_DOCLING_RUNS, VRAM_EMBED_MODEL_ARG.
#
# Exit: 0 ok; 1 errore (servizio non pronto, container di altri, carico fallito); 2 uso.
#
# **Modifica:** Claude (gsd-executor 01-15) · 2026-10-25 · misura di convivenza VRAM

set -euo pipefail

if [[ -z ${CKA_RUN_ID:-} ]]; then
  printf 'vram.sh: CKA_RUN_ID assente: lanciare dentro session.sh with 01-15\n' >&2
  exit 2
fi
RID=$CKA_RUN_ID

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SPIKES=$(cd "$HERE/.." && pwd)
T=$SPIKES/tools/session.sh

CKA_SPIKES_ENV=${CKA_SPIKES_ENV:-$HOME/.cka-spikes/spikes.env}
if [[ ! -f $CKA_SPIKES_ENV ]]; then
  printf 'vram.sh: file dei segreti assente (CKA_SPIKES_ENV)\n' >&2
  exit 2
fi
set -a
# shellcheck disable=SC1090
. "$CKA_SPIKES_ENV"
set +a

B_EMBED=${TEI_MAX_BATCH_TOKENS:-16384}
B_RERANK=${TEI_RERANK_MAX_BATCH_TOKENS:-8192}
DURATION_S=${VRAM_DURATION_S:-180}
WARM_S=${VRAM_WARM_S:-30}
SCENARIO=${VRAM_SCENARIO:-b${B_EMBED}-${B_RERANK}}
OUT=${VRAM_OUT:-$HERE/results/rerank/vram.csv}
RAWDIR=$HERE/results/raw/rerank/vram-$SCENARIO
EMBED_ARG=${VRAM_EMBED_MODEL_ARG:-bgem3}
DOC_INPUT=${VRAM_DOCLING_INPUT:-/data/corpus/raw/pub-it-01-bdi-relazione-annuale-2025.pdf}
DOC_PAGES=${VRAM_DOCLING_PAGES:-12-12}
DOC_JOBS=${VRAM_DOCLING_JOBS:-/src/s02-parser/results/raw/jobs/s04-vram.jsonl}
DOC_RUNS=${VRAM_DOCLING_RUNS:-/src/s02-parser/results/raw/extract/s04-vram-runs.jsonl}
ACCEPT=()
if [[ -n ${VRAM_ACCEPT_SHA:-} ]]; then ACCEPT=(--accept-sha "$VRAM_ACCEPT_SHA"); fi
HEADER=t_ms,scenario,max_batch_tokens_embed,max_batch_tokens_rerank,used_mib_total,used_mib_tei_embed,used_mib_tei_rerank,used_mib_docling,oom
SERVICES=(tei-embed tei-rerank docling)

log() { printf 'vram.sh: %s\n' "$*" >&2; }
die() { local code=$1; shift; printf 'vram.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

[[ $DURATION_S =~ ^[0-9]+$ && $WARM_S =~ ^[0-9]+$ ]] || die 2 "VRAM_DURATION_S e VRAM_WARM_S interi"
[[ $SCENARIO =~ ^[a-z0-9][a-z0-9.-]{0,40}$ ]] || die 2 "VRAM_SCENARIO non valido"

WORK=$(mktemp -d)
SAMPLER_PID=""
LOAD_PIDS=()
cleanup() {
  local p
  for p in "${LOAD_PIDS[@]}" $SAMPLER_PID; do
    kill "$p" 2>/dev/null || true
  done
  wait 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT
trap 'log "interrotto: i container della sessione li ferma session.sh with"; exit 130' INT TERM

# Nessun container di altri piani (build o misure): GPU non condivisa.
others=$(docker ps --format '{{.Names}} {{.Label "cka.run_id"}}' | awk -v r="$RID" '$2 != r')
[[ -z $others ]] || die 1 "container estranei alla sessione in esecuzione: $(tr '\n' ' ' <<< "$others")"

mkdir -p "$RAWDIR"
T0=$(date +%s%3N)
phase() { printf '%s\n' "$1" > "$WORK/phase"; log "fase $1"; }

sample_loop() {
  local v now
  while :; do
    v=$(nvidia-smi --query-gpu=memory.used --format=csv,noheader,nounits 2>/dev/null | head -n 1 | tr -d ' \r')
    now=$(date +%s%3N)
    [[ $v =~ ^[0-9]+$ ]] && printf '%s %s %s\n' "$((now - T0))" "$v" "$(cat "$WORK/phase")" >> "$WORK/samples"
    sleep 0.5
  done
}

# Processi visti da nvidia-smi (documenta che su WSL2 i container non compaiono).
apps_snapshot() {
  { printf '## %s\n' "$1"; nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv 2>&1 || true; } \
    >> "$RAWDIR/compute-apps.txt"
}

# Comando di carico nel runner per un servizio, per `secs` secondi.
load_cmd() {
  local svc=$1 secs=$2
  case $svc in
    tei-embed)
      printf '%s' "mkdir -p /work/tmp/s04-vram/emb && end=\$((SECONDS+$secs)) && while (( SECONDS < end )); do /target/release/s03-pgvector --out /work/tmp/s04-vram/emb --raw /work/tmp/s04-vram/emb-raw throughput --model $EMBED_ARG --batch 32 --max-batch-tokens $B_EMBED --run 1 --queries 200 || exit 1; done" ;;
    tei-rerank)
      printf '%s' "mkdir -p /work/tmp/s04-vram && /target/release/rerank --methods tei --duration-s $secs --out /work/tmp/s04-vram/rerank-load.csv ${ACCEPT[*]}" ;;
    docling)
      printf '%s' "mkdir -p /work/tmp/s04-vram/docling && end=\$((SECONDS+$secs)) && n=0 && while (( SECONDS < end )); do /target/release/docling-client --extract-runs $DOC_RUNS --extract-jobs $DOC_JOBS --data-root /src/data --results-raw /src/s02-parser/results/raw --device gpu --input $DOC_INPUT --page-range $DOC_PAGES --out-prefix /work/tmp/s04-vram/docling/page || exit 1; n=\$((n+1)); done; echo conversioni \$n" ;;
  esac
}

run_load() {
  local svc=$1 secs=$2
  bash "$T" oneshot "$RID" -- bash -c "$(load_cmd "$svc" "$secs")" >> "$RAWDIR/load-$svc.log" 2>&1 </dev/null
}

wait_ready() {
  local svc=$1 i probe
  case $svc in
    tei-embed) probe='curl -fsS "$TEI_EMBED_URL/health"' ;;
    tei-rerank) probe='curl -fsS "$TEI_RERANK_URL/health"' ;;
    docling) probe="/target/release/docling-client --extract-runs $DOC_RUNS --extract-jobs $DOC_JOBS --data-root /src/data --results-raw /src/s02-parser/results/raw --device gpu --get version" ;;
  esac
  for i in $(seq 1 90); do
    if bash "$T" oneshot "$RID" -- bash -c "$probe" >/dev/null 2>&1 </dev/null; then
      log "$svc pronto (tentativo $i)"
      return 0
    fi
    sleep 5
  done
  die 1 "$svc non pronto"
}

printf 'desktop\n' > "$WORK/phase"
sample_loop &
SAMPLER_PID=$!

phase desktop
apps_snapshot desktop
sleep 10

for svc in "${SERVICES[@]}"; do
  phase "warm-$svc"
  bash "$T" up "$RID" "$svc" >&2 || die 1 "session.sh up $svc non riuscito (servizio di un'altra sessione?)"
  wait_ready "$svc"
  run_load "$svc" "$WARM_S" || die 1 "carico di riscaldamento di $svc non riuscito (vedi $RAWDIR/load-$svc.log)"
  sleep 3
  apps_snapshot "warm-$svc"
done

bash "$T" ps "$RID" --assert-only tei-embed tei-rerank docling runner >&2 \
  || die 1 "servizi della sessione diversi da tei-embed tei-rerank docling"

phase load
status=()
for svc in "${SERVICES[@]}"; do
  run_load "$svc" "$DURATION_S" &
  LOAD_PIDS+=($!)
done
for i in "${!LOAD_PIDS[@]}"; do
  if wait "${LOAD_PIDS[$i]}"; then status+=("${SERVICES[$i]}=0"); else status+=("${SERVICES[$i]}=$?"); fi
done
LOAD_PIDS=()
apps_snapshot load

phase after
sleep 5
kill "$SAMPLER_PID" 2>/dev/null || true
wait "$SAMPLER_PID" 2>/dev/null || true
SAMPLER_PID=""

# OOM: log dei servizi e dei carichi, servizi ancora in esecuzione.
oom=false
for svc in "${SERVICES[@]}"; do
  cid=$(bash "$T" cid "$RID" "$svc" 2>/dev/null || true)
  if [[ -z $cid ]]; then
    log "$svc non più in esecuzione"; oom=true; continue
  fi
  docker logs "$cid" > "$RAWDIR/service-$svc.log" 2>&1 || true
  if [[ $(docker inspect -f '{{.State.Running}}' "$cid" 2>/dev/null) != true ]]; then
    log "$svc fermo alla fine"; oom=true
  fi
done
if grep -Eqi 'out of memory|outofmemory|CUDA_ERROR_OUT_OF_MEMORY' "$RAWDIR"/*.log 2>/dev/null; then
  oom=true
fi
for s in "${status[@]}"; do
  [[ ${s#*=} == 0 ]] || { log "carico ${s%%=*} uscito con ${s#*=}"; }
done

if [[ ! -s $OUT ]]; then
  mkdir -p "$(dirname "$OUT")"
  printf '%s\n' "$HEADER" > "$OUT"
elif [[ $(head -n 1 "$OUT" | tr -d '\r') != "$HEADER" ]]; then
  die 1 "header inatteso in $OUT"
fi

# Quote per servizio (picco − minimo della fase warm) e righe del CSV.
awk -v sc="$SCENARIO" -v be="$B_EMBED" -v br="$B_RERANK" -v oom="$oom" '
  NR == FNR {
    p = $3
    if (!(p in mx) || $2 > mx[p]) mx[p] = $2
    if (!(p in mn) || $2 < mn[p]) mn[p] = $2
    next
  }
  FNR == 1 {
    q["tei-embed"] = mx["warm-tei-embed"] - mn["warm-tei-embed"]
    q["tei-rerank"] = mx["warm-tei-rerank"] - mn["warm-tei-rerank"]
    q["docling"] = mx["warm-docling"] - mn["warm-docling"]
    order["desktop"] = 0; order["warm-tei-embed"] = 1; order["warm-tei-rerank"] = 2
    order["warm-docling"] = 3; order["load"] = 4; order["after"] = 5
  }
  {
    p = $3; o = order[p]
    e = (o > 1) ? q["tei-embed"] : (o == 1 ? $2 - mn[p] : 0)
    r = (o > 2) ? q["tei-rerank"] : (o == 2 ? $2 - mn[p] : 0)
    d = (o > 3) ? q["docling"] : (o == 3 ? $2 - mn[p] : 0)
    printf "%s,%s-%s,%s,%s,%s,%s,%s,%s,%s\n", $1, sc, p, be, br, $2, e, r, d, oom
  }
' "$WORK/samples" "$WORK/samples" >> "$OUT"

cp "$WORK/samples" "$RAWDIR/samples.txt"
awk -v sc="$SCENARIO" '
  { p = $3; if (!(p in mx) || $2 > mx[p]) mx[p] = $2; if (!(p in mn) || $2 < mn[p]) mn[p] = $2
    if (!(p in t0)) t0[p] = $1; t1[p] = $1 }
  END {
    for (p in mx) printf "%s %s: min %d MiB, max %d MiB, durata %d ms\n", sc, p, mn[p], mx[p], t1[p] - t0[p]
  }
' "$WORK/samples" | sort >&2
log "carichi: ${status[*]}; oom=$oom; righe in $OUT"
