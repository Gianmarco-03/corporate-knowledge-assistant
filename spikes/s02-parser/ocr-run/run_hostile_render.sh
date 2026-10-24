#!/usr/bin/env bash
# spikes/s02-parser/ocr-run/run_hostile_render.sh <runs> — prova ostile sul render (piano 01-13,
# D-34, e obbligo del contratto della 1.5), sull'host.
#
# Gira solo dentro una sessione prenotata (esce 2 se CKA_RUN_ID manca):
#   bash spikes/tools/session.sh with 01-13 -- bash spikes/s02-parser/ocr-run/run_hostile_render.sh 3
#
# Casi e attesi dai manifest in sola lettura (nessun atteso nuovo):
#   - spikes/data/hostile/manifest.json (01-18): jbig2-malformed, deep-nesting;
#   - spikes/data/hostile/sof/manifest.json (1.5): dct-sof-huge-image, jpx-siz-huge-image,
#     jpx-siz-huge-jp2 (rifiuto del prescan con detail dct_sof / jpx_siz).
# Precondizione: le fixture esistono in spikes/data/hostile/generated/ (e generated/sof/) con gli
# sha256 dei manifest, confrontati qui con sha256sum: file assente o diverso → exit 2, nessun job.
#
# Per caso e ripetizione, un job `ocr_render` con `parse-parent run --job <file>` nel worker,
# concorrenza 1: `job_id` = `hr-<case_id>-<run>-<CKA_RUN_ID>`, `input` = `/data/hostile/generated/…`,
# `params` `{"dpi": 300}`, `out_dir` esattamente `/results/raw/ocr/<job_id>`. Il job si passa come
# file (`results/raw/hostile-render/<CKA_RUN_ID>/<job_id>.job.json`) perché `session.sh exec` non
# inoltra lo stdin. Dopo ogni job si salvano lo stdout di parse-parent (`<job_id>.stdout`, con la
# riga RunReport), l'exit e lo stato del worker (`session.sh cid` e `ps --assert-only worker`) in
# `<job_id>.meta`; a fine batch le voci rimaste in /work del worker (`work-leftover.txt`, atteso
# 0). Un `job_error` ferma lo script senza ritentare. Poi `ocr hostile-verdict` nel runner scrive
# `results/ocr/hostile-render.csv`.
#
# **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · driver della prova ostile sul render (Task 2)

set -euo pipefail

if [[ -z ${CKA_RUN_ID:-} ]]; then
  printf 'run_hostile_render.sh: CKA_RUN_ID assente: gira dentro session.sh with 01-13\n' >&2
  exit 2
fi
RUNS=${1:-}
if [[ ! $RUNS =~ ^[1-9][0-9]*$ ]]; then
  printf 'uso: run_hostile_render.sh <runs>\n' >&2
  exit 2
fi

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SPIKES=$(cd "$HERE/../.." && pwd)
TOOLS=$SPIKES/tools
RID=$CKA_RUN_ID
RAW=$SPIKES/s02-parser/results/raw
SESSION=(bash "$TOOLS/session.sh")
LIMITS=/results/ocr/limits-ocr_render.json
OUT=$RAW/hostile-render/$RID
OCR=/target/release/ocr

log() { printf 'run_hostile_render.sh: %s\n' "$*" >&2; }
die() { local code=$1; shift; printf 'run_hostile_render.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

# case_id → manifest e sottocartella delle fixture.
CASES=(jbig2-malformed deep-nesting dct-sof-huge-image jpx-siz-huge-image jpx-siz-huge-jp2)
manifest_of() {
  case $1 in
    jbig2-malformed|deep-nesting) echo "$SPIKES/data/hostile/manifest.json" ;;
    *) echo "$SPIKES/data/hostile/sof/manifest.json" ;;
  esac
}
subdir_of() {
  case $1 in
    jbig2-malformed|deep-nesting) echo "" ;;
    *) echo "sof/" ;;
  esac
}
field_of() {
  node -e '
    const m = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
    const c = m.find(x => x.case_id === process.argv[2]);
    if (!c) process.exit(3);
    console.log(c[process.argv[3]]);
  ' "$1" "$2" "$3" | tr -d '\r'
}

# ---- precondizione: fixture con gli sha256 dei manifest -----------------------------------
declare -A FILE
for c in "${CASES[@]}"; do
  m=$(manifest_of "$c")
  f=$(field_of "$m" "$c" file) || die 2 "caso $c assente da $m"
  want=$(field_of "$m" "$c" sha256) || die 2 "sha256 di $c assente"
  host=$SPIKES/data/hostile/generated/$(subdir_of "$c")$f
  [[ -f $host ]] || die 2 "fixture assente: $host (rigenerarla con il generatore del manifest)"
  got=$(sha256sum "$host" | cut -d' ' -f1 | tr -d '\\')
  [[ $got == "$want" ]] || die 2 "sha256 diverso dal manifest per $c: nessun job"
  FILE[$c]=/data/hostile/generated/$(subdir_of "$c")$f
done
log "fixture verificate: ${#CASES[@]} casi con lo sha256 dei manifest"

[[ -f $SPIKES/s02-parser/results/ocr/limits-ocr_render.json ]] || die 1 "limits-ocr_render.json assente"
mkdir -p "$OUT" "$SPIKES/s02-parser/results/ocr"
if ! "${SESSION[@]}" cid "$RID" worker >/dev/null 2>&1; then
  "${SESSION[@]}" up "$RID" worker >&2
fi

worker_alive() {
  "${SESSION[@]}" cid "$RID" worker >/dev/null 2>&1 \
    && "${SESSION[@]}" ps "$RID" --assert-only worker >/dev/null 2>&1
}

# ---- job ostili, concorrenza 1 -------------------------------------------------------------
DEAD=0
for c in "${CASES[@]}"; do
  for run in $(seq 1 "$RUNS"); do
    job="hr-$c-$run-$RID"
    (( ${#job} <= 64 )) || die 1 "job_id oltre 64 caratteri: $job"
    printf '{"job_id":"%s","op":"ocr_render","input":"%s","out_dir":"/results/raw/ocr/%s","params":{"dpi":300}}\n' \
      "$job" "${FILE[$c]}" "$job" > "$OUT/$job.job.json"
    set +e
    "${SESSION[@]}" exec "$RID" worker -- parse-parent run --job "/results/raw/hostile-render/$RID/$job.job.json" \
      --limits "$LIMITS" > "$OUT/$job.stdout" 2> "$OUT/$job.stderr"
    rc=$?
    set -e
    alive=false
    worker_alive && alive=true
    printf 'case_id\t%s\nrun\t%s\nparent_exit\t%s\nworker_alive\t%s\n' "$c" "$run" "$rc" "$alive" > "$OUT/$job.meta"
    if grep -q '"job_error"' "$OUT/$job.stdout"; then
      die 1 "job_error per $job: $(head -c 300 "$OUT/$job.stdout")"
    fi
    log "$job: exit $rc, worker vivo $alive"
    if [[ $alive != true ]]; then
      # Contenimento fallito: niente altri job, ma il verdetto si scrive comunque.
      log "worker non più in esecuzione dopo $job: contenimento fallito"
      DEAD=1
      break 2
    fi
  done
done

if (( DEAD == 0 )); then
  "${SESSION[@]}" exec "$RID" worker -- sh -c 'find /work -mindepth 1 -maxdepth 1 | wc -l' \
    | tr -d '\r ' > "$OUT/work-leftover.txt"
  log "voci rimaste in /work: $(cat "$OUT/work-leftover.txt")"
fi

# ---- verdetto (runner) ----------------------------------------------------------------------
"${SESSION[@]}" oneshot "$RID" -- cargo build --release --locked -p s02-parser --bin ocr >&2
"${SESSION[@]}" oneshot "$RID" -- "$OCR" hostile-verdict \
  --manifest /src/data/hostile/manifest.json --manifest /src/data/hostile/sof/manifest.json \
  --reports "/src/s02-parser/results/raw/hostile-render/$RID" \
  --out /src/s02-parser/results/ocr/hostile-render.csv
(( DEAD == 0 )) || exit 1
