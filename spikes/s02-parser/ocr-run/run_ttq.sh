#!/usr/bin/env bash
# spikes/s02-parser/ocr-run/run_ttq.sh <doc> <runs> [warm|cold] — TTQ-parse sul percorso composto
# reale (piano 01-13, D-07, M1), sull'host.
#
# Gira solo dentro una sessione prenotata (esce 2 se CKA_RUN_ID manca):
#   bash spikes/tools/session.sh with 01-13 -- bash spikes/s02-parser/ocr-run/run_ttq.sh \
#     /data/corpus/raw/der-it-01-relazione-annuale-2025-p1-25.pdf 21
#
# `<doc>` è il percorso del PDF nel worker (`/data/...` o `/results/raw/...`). Worker e Docling
# (`docling`, GPU; `TTQ_DOCLING_DEVICE=cpu` per `docling-cpu`) si avviano una volta con
# `session.sh up`, il runner con `session.sh run "$CKA_RUN_ID" cka-s02-ocr-runner -- sleep
# infinity`; tutto si pilota con `session.sh exec`, fuori dalla finestra misurata. Con `cold` i
# container di worker e Docling si fermano e si riavviano prima dell'unica run (`warm=false`).
#
# Per ogni run: `run_id` = `t<ms epoch>-<4 hex>`, sha256 del documento, timestamp esterni
# `date +%s%N` attorno all'intera sequenza e a ogni passo:
#   1. job `extract_pdf` (prescan + PDFium), `job_id` `<run_id>-extract`, `out_dir`
#      `/results/raw/ttq/<run_id>-extract`;
#   2. selezione delle pagine-tabella con l'euristica di 01-08 (`ocr ttq-select`, parametri
#      congelati `results/extract/table_heuristic_params.json`) e `docling-client` su quelle
#      pagine, un intervallo contiguo per richiesta;
#   3. sulle pagine con zero caratteri in uscita da `extract_pdf`: `qpdf` ne fa un PDF nel runner,
#      job `ocr_render` (`<run_id>-render`, `out_dir` `/results/raw/ttq/<run_id>-render`) e job
#      `ocr_tesseract` (`<run_id>-tess-<pagina>`, `out_dir` null) con `parse-parent batch`.
# Un `job_error` (per esempio `out_dir_exists`) scarta la run, che si registra in
# `results/raw/ttq-logs/anomalies.txt` e non si rilancia con lo stesso `run_id`. Log della run in
# `results/raw/ttq-logs/<run_id>/` (area distinta, mai `out_dir` di un job), poi `ocr ttq-merge`
# scrive una riga di `results/ocr/ttq_parse.csv`.
#
# `TTQ_INJECT_DELAY_MS` (default 0) inserisce un ritardo dentro la finestra misurata, solo per
# l'autocontrollo del cronometro.
#
# **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · driver del TTQ-parse composto (Task 2)

set -euo pipefail

if [[ -z ${CKA_RUN_ID:-} ]]; then
  printf 'run_ttq.sh: CKA_RUN_ID assente: gira dentro session.sh with 01-13\n' >&2
  exit 2
fi
DOC=${1:-}
RUNS=${2:-}
MODE=${3:-warm}
if [[ ! $DOC =~ ^/(data|results/raw)/[A-Za-z0-9/_.-]+\.pdf$ || ! $RUNS =~ ^[1-9][0-9]*$ ]]; then
  printf 'uso: run_ttq.sh <pdf nel worker> <runs> [warm|cold]\n' >&2
  exit 2
fi
case $MODE in
  warm) WARM=true ;;
  cold) WARM=false; [[ $RUNS == 1 ]] || { printf 'run_ttq.sh: cold vuole una sola run\n' >&2; exit 2; } ;;
  *) printf 'run_ttq.sh: modo sconosciuto: %s\n' "$MODE" >&2; exit 2 ;;
esac

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SPIKES=$(cd "$HERE/../.." && pwd)
TOOLS=$SPIKES/tools
RID=$CKA_RUN_ID
RAW=$SPIKES/s02-parser/results/raw
SESSION=(bash "$TOOLS/session.sh")
RUNNER=cka-s02-ocr-runner
OCR=/target/release/ocr
CLIENT=/target/release/docling-client
DELAY=${TTQ_INJECT_DELAY_MS:-0}
DEVICE=${TTQ_DOCLING_DEVICE:-gpu}
[[ $DELAY =~ ^[0-9]+$ ]] || { printf 'run_ttq.sh: TTQ_INJECT_DELAY_MS non intero\n' >&2; exit 2; }
case $DEVICE in
  gpu) DOCLING=docling ;;
  cpu) DOCLING=docling-cpu ;;
  *) printf 'run_ttq.sh: TTQ_DOCLING_DEVICE gpu|cpu\n' >&2; exit 2 ;;
esac
EXTRACT_LIMITS=/results/extract/limits-extract_pdf.json
RENDER_LIMITS=/results/ocr/limits-ocr_render.json
TESS_LIMITS=/results/ocr/limits-ocr_tesseract.json
TESS_CONCURRENCY=${TTQ_TESS_CONCURRENCY:-8}

log() { printf 'run_ttq.sh: %s\n' "$*" >&2; }
die() { local code=$1; shift; printf 'run_ttq.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

# Percorso del worker → host e → runner (/src = spikes/).
host_of() {
  case $1 in
    /data/*) printf '%s/data/%s\n' "$SPIKES" "${1#/data/}" ;;
    /results/raw/*) printf '%s/%s\n' "$RAW" "${1#/results/raw/}" ;;
  esac
}
runner_of() {
  case $1 in
    /data/*) printf '/src/data/%s\n' "${1#/data/}" ;;
    /results/raw/*) printf '/src/s02-parser/results/raw/%s\n' "${1#/results/raw/}" ;;
  esac
}

HOST_DOC=$(host_of "$DOC")
[[ -f $HOST_DOC ]] || die 1 "documento assente: $HOST_DOC"
DOC_ID=$(basename "$DOC" .pdf)
SHA=$(sha256sum "$HOST_DOC" | cut -d' ' -f1 | tr -d '\\')
mkdir -p "$RAW/ttq" "$RAW/ttq-logs" "$RAW/ttq-src" "$SPIKES/s02-parser/results/ocr"

wexec() { "${SESSION[@]}" exec "$RID" worker -- "$@"; }
rexec() { "${SESSION[@]}" exec "$RID" "$RUNNER" -- "$@"; }

docling_ready() {
  local i
  for i in $(seq 1 100); do
    if rexec "$CLIENT" --extract-runs /dev/null --extract-jobs /dev/null --device "$DEVICE" --get version \
        --data-root /src/data --results-raw /src/s02-parser/results/raw >/dev/null 2>&1; then
      return 0
    fi
    sleep 3
  done
  die 1 "Docling ($DEVICE) non pronto"
}

# ---- servizi, fuori dalla finestra ---------------------------------------------------------
for svc in worker "$DOCLING"; do
  "${SESSION[@]}" cid "$RID" "$svc" >/dev/null 2>&1 || "${SESSION[@]}" up "$RID" "$svc" >&2
done
if ! rexec true >/dev/null 2>&1; then
  "${SESSION[@]}" run "$RID" "$RUNNER" -- sleep infinity >&2
fi
rexec cargo build --release --locked -p s02-parser --bin ocr --bin docling-client >&2
if [[ $MODE == cold ]]; then
  for svc in worker "$DOCLING"; do
    "${SESSION[@]}" stop "$RID" "$("${SESSION[@]}" cid "$RID" "$svc")" >&2
    "${SESSION[@]}" up "$RID" "$svc" >&2
  done
fi
docling_ready

# ---- run --------------------------------------------------------------------------------------
now() { date +%s%N; }
step() { printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$1" "$RUN_ID" "$SHA" "$2" "$3" "$4" >> "$LOG/steps.tsv"; }
anomaly() { printf '%s\t%s\t%s\t%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)" "$RUN_ID" "$DOC_ID" "$1" >> "$RAW/ttq-logs/anomalies.txt"; log "run $RUN_ID scartata: $1"; }
ok_report() { grep -q '"outcome":{"status":"ok"' "$1" && ! grep -q '"job_error"' "$1"; }

# Intervalli contigui di una lista `a,b,c`.
ranges() {
  awk -F, '{ s=$1; p=$1; for (i=2; i<=NF; i++) { if ($i == p+1) { p=$i } else { print s "-" p; s=$i; p=$i } } print s "-" p }' <<< "$1"
}

for r in $(seq 1 "$RUNS"); do
  RUN_ID="t$(date +%s%3N)-$(od -An -N2 -tx1 /dev/urandom | tr -d ' \n')"
  LOG=$RAW/ttq-logs/$RUN_ID
  WLOG=/results/raw/ttq-logs/$RUN_ID
  RLOG=/src/s02-parser/results/raw/ttq-logs/$RUN_ID
  mkdir "$LOG"
  : > "$LOG/steps.tsv"
  printf '{"job_id":"%s-extract","op":"extract_pdf","input":"%s","out_dir":"/results/raw/ttq/%s-extract","params":{}}\n' \
    "$RUN_ID" "$DOC" "$RUN_ID" > "$LOG/extract.jobs.jsonl"

  T0=$(now)
  if (( DELAY > 0 )); then
    sleep "$(awk -v d="$DELAY" 'BEGIN{printf "%.3f", d/1000}')"
  fi
  # 1. extract_pdf
  s=$(now)
  wexec parse-parent run --job "$WLOG/extract.jobs.jsonl" --limits "$EXTRACT_LIMITS" > "$LOG/extract.out" 2>/dev/null || true
  e=$(now)
  if ! ok_report "$LOG/extract.out"; then anomaly "extract_pdf non ok: $(head -c 200 "$LOG/extract.out")"; continue; fi
  PAGES=$(sed -nE 's/.*"outcome":\{"status":"ok","pages":([0-9]+).*/\1/p' "$LOG/extract.out" | head -n 1)
  [[ $PAGES =~ ^[0-9]+$ ]] || { anomaly "pagine non lette dal RunReport"; continue; }
  step extract "$(seq -s, 1 "$PAGES")" "$s" "$e"

  # 2. pagine-tabella → Docling
  SEL=$(rexec "$OCR" ttq-select --pages-dir "/src/s02-parser/results/raw/ttq/$RUN_ID-extract" \
    --params /src/s02-parser/results/extract/table_heuristic_params.json | tr -d '\r')
  TABLES=$(sed -nE 's/.*table_pages=([0-9,]*).*/\1/p' <<< "$SEL")
  OCRP=$(sed -nE 's/.*ocr_pages=([0-9,]*).*/\1/p' <<< "$SEL")
  if [[ -n $TABLES ]]; then
    s=$(now)
    dl_ok=1
    while read -r rg; do
      if ! rexec "$CLIENT" --extract-runs "$RLOG/extract.out" --extract-jobs "$RLOG/extract.jobs.jsonl" \
          --input "$DOC" --data-root /src/data --results-raw /src/s02-parser/results/raw \
          --device "$DEVICE" --page-range "$rg" --out-prefix "$RLOG/docling-$rg" >> "$LOG/docling.out" 2>/dev/null; then
        dl_ok=0
      fi
    done < <(ranges "$TABLES")
    e=$(now)
    (( dl_ok )) || { anomaly "docling-client non ok: $(tail -n 1 "$LOG/docling.out")"; continue; }
    step docling "$TABLES" "$s" "$e"
  fi

  # 3. pagine senza testo → render + tesseract
  if [[ -n $OCRP ]]; then
    s=$(now)
    rexec qpdf --deterministic-id --empty --pages "$(runner_of "$DOC")" "$OCRP" -- \
      "/src/s02-parser/results/raw/ttq-src/$RUN_ID-sub.pdf" >/dev/null
    printf '{"job_id":"%s-render","op":"ocr_render","input":"/results/raw/ttq-src/%s-sub.pdf","out_dir":"/results/raw/ttq/%s-render","params":{"dpi":300}}\n' \
      "$RUN_ID" "$RUN_ID" "$RUN_ID" > "$LOG/render.job.json"
    wexec parse-parent run --job "$WLOG/render.job.json" --limits "$RENDER_LIMITS" > "$LOG/render.out" 2>/dev/null || true
    e=$(now)
    if ! ok_report "$LOG/render.out"; then anomaly "ocr_render non ok: $(head -c 200 "$LOG/render.out")"; continue; fi
    step render "$OCRP" "$s" "$e"
    : > "$LOG/tess.jsonl"
    i=0
    IFS=, read -r -a plist <<< "$OCRP"
    for p in "${plist[@]}"; do
      i=$((i + 1))
      printf '{"job_id":"%s-tess-%s","op":"ocr_tesseract","input":"/results/raw/ttq/%s-render/page-%04d.png","out_dir":null,"params":{}}\n' \
        "$RUN_ID" "$p" "$RUN_ID" "$i" >> "$LOG/tess.jsonl"
    done
    s=$(now)
    wexec parse-parent batch --jobs "$WLOG/tess.jsonl" --concurrency "$TESS_CONCURRENCY" --limits "$TESS_LIMITS" \
      > "$LOG/tess.out" 2>/dev/null || true
    e=$(now)
    if grep -q '"job_error"' "$LOG/tess.out" || grep -q '"outcome":{"status":"\(failed\|quarantined\)"' "$LOG/tess.out"; then
      anomaly "ocr_tesseract non ok"; continue
    fi
    step tesseract "$OCRP" "$s" "$e"
  fi
  T1=$(now)

  cat "$LOG/extract.out" "$LOG/render.out" "$LOG/tess.out" 2>/dev/null | grep '^{' > "$LOG/reports.jsonl" || true
  printf 'run_id\t%s\ndoc_id\t%s\ndoc_sha256\t%s\npages\t%s\nrun\t%s\nwarm\t%s\ndocling_device\t%s\nt_start_ns\t%s\nt_end_ns\t%s\ninject_delay_ms\t%s\n' \
    "$RUN_ID" "$DOC_ID" "$SHA" "$PAGES" "$r" "$WARM" "$DEVICE" "$T0" "$T1" "$DELAY" > "$LOG/meta.tsv"
  rexec "$OCR" ttq-merge --run-dir "$RLOG" --csv /src/s02-parser/results/ocr/ttq_parse.csv | tr -d '\r'
done
