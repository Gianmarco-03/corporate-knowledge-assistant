#!/usr/bin/env bash
# spikes/s02-parser/scan-gen/make_scans.sh — scansioni sintetiche del piano 01-13 (Task 2), sull'host.
#
# Gira solo dentro una sessione prenotata (esce 2 se CKA_RUN_ID manca):
#   bash spikes/tools/session.sh with 01-13 -- bash spikes/s02-parser/scan-gen/make_scans.sh
#
# Precondizioni: `results/ocr/limits-ocr_render.json` (calibrate --op ocr_render) e le pagine
# di gen_geom.py in `results/raw/geom/<CKA_RUN_ID>/`; corpus scaricato (`data/corpus/fetch.sh`).
#
# Passi:
#  1. nel runner, `qpdf` estrae le pagine IT dell'oracolo (`parser-it.json`, voci page-text) in
#     PDF di una pagina in `results/raw/ocr-src/<CKA_RUN_ID>/`;
#  2. render via job `ocr_render` (`parse-parent batch` nel worker) delle pagine dell'oracolo,
#     del documento IT da 25 pagine e dei 4 PDF di gen_geom.py, a 300 DPI (buone) e a 200 DPI
#     (base della degradata); `job_id` = `render-<doc>-<dpi>-<CKA_RUN_ID>`, `out_dir` esattamente
#     `/results/raw/ocr/<job_id>`, creato dal padre (un `job_error` ferma lo script, mai un
#     nuovo tentativo con un altro nome); job `extract_pdf` sugli stessi PDF digitali (riferimento
#     PDFium delle bbox) in `/results/raw/ocr-extract/<job_id>`;
#  3. indice `results/raw/ocr-index/<CKA_RUN_ID>.json` (render ed extract: job_id → out_dir), da
#     cui leggono i passi successivi: nessun percorso di PNG scritto a mano;
#  4. nel runner, degradazione dichiarata nelle Soglie sui PNG a 200 DPI con `magick`, input
#     `png:` e uscite `jpg:`/`png:` esplicite (nessun coder scelto dal contenuto), seme fisso del
#     rumore per la riproducibilità, in `results/raw/ocr-degraded/<CKA_RUN_ID>/<job_id>/`, con un
#     `<pagina>.transform.json` accanto a ogni PNG;
#  5. PDF misto (pagine 1-5 di testo del documento da 25 pagine + pagine 6-10 come immagini
#     JPEG a 300 DPI) composto con `qpdf` in `results/raw/ocr-degraded/<CKA_RUN_ID>/mixed.pdf`;
#     sha256 di immagini e PDF misto in `results/raw/ocr-degraded/<CKA_RUN_ID>/sha256.txt`.
#
# Rischio accettato (Task 2 del piano): il runner è sulle reti default e parse (compose di
# 01-02), ma ImageMagick legge solo PNG prodotti dal nostro render (input già passato da
# prescan::check_pdf) e scrive JPEG, PNG e PDF senza delegati di rete.
#
# Nessun container lanciato fuori da session.sh, nessun profilo, mai il down di compose.
#
# **Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · scansioni sintetiche e degradazione dichiarata (Task 2)

set -euo pipefail

if [[ -z ${CKA_RUN_ID:-} ]]; then
  printf 'make_scans.sh: CKA_RUN_ID assente: gira dentro session.sh with 01-13\n' >&2
  exit 2
fi

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
SPIKES=$(cd "$HERE/../.." && pwd)
TOOLS=$SPIKES/tools
RID=$CKA_RUN_ID
RAW=$SPIKES/s02-parser/results/raw
SESSION=(bash "$TOOLS/session.sh")

DOC25=der-it-01-relazione-annuale-2025-p1-25
RENDER_LIMITS=/results/ocr/limits-ocr_render.json
EXTRACT_LIMITS=/results/extract/limits-extract_pdf.json
MIXED_TEXT_PAGES=1-5
MIXED_IMAGE_PAGES=(6 7 8 9 10)

# Percorso host per node (Windows): con MSYS_NO_PATHCONV la conversione automatica è spenta.
nodepath() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}
log() { printf 'make_scans.sh: %s\n' "$*" >&2; }
die() { local code=$1; shift; printf 'make_scans.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

[[ -f $SPIKES/s02-parser/results/ocr/limits-ocr_render.json ]] || die 1 "limits-ocr_render.json assente: prima calibrate --op ocr_render"
[[ -f $RAW/geom/$RID/geom-cases.json ]] || die 1 "pagine di gen_geom.py assenti in results/raw/geom/$RID"
[[ -f $SPIKES/data/corpus/raw/$DOC25.pdf ]] || die 1 "corpus assente: bash spikes/data/corpus/fetch.sh"

mkdir -p "$RAW/ocr" "$RAW/ocr-extract" "$RAW/ocr-index" "$RAW/ocr-jobs/$RID" "$RAW/ocr-runs/$RID" \
  "$RAW/ocr-src/$RID" "$RAW/ocr-degraded/$RID"

# Worker della sessione (avviato se non c'è).
if ! "${SESSION[@]}" cid "$RID" worker >/dev/null 2>&1; then
  "${SESSION[@]}" up "$RID" worker >&2
fi

# ---- 1. pagine dell'oracolo --------------------------------------------------------------
# Voci page-text di parser-it.json: oracle_id, documento, pagina 1-based.
mapfile -t ORACLE < <(node -e '
  const o = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
  for (const e of o) if (e.parser_kind === "page-text")
    console.log([e.oracle_id, e.source_ref.document_id, e.source_ref.page].join(" "));
' "$(nodepath "$SPIKES/data/corpus/oracles/parser-it.json")" | tr -d '\r')
(( ${#ORACLE[@]} > 0 )) || die 1 "nessuna voce page-text in parser-it.json"

QPDF_CMDS=""
for line in "${ORACLE[@]}"; do
  read -r oid doc page <<< "$line"
  [[ $oid =~ ^P-[0-9]{3}$ && $doc =~ ^[a-z0-9-]+$ && $page =~ ^[0-9]+$ ]] || die 1 "voce dell'oracolo inattesa: $line"
  name=$(tr 'A-Z' 'a-z' <<< "$oid")
  QPDF_CMDS+="qpdf --deterministic-id --empty --pages /src/data/corpus/raw/$doc.pdf $page -- /src/s02-parser/results/raw/ocr-src/$RID/$name.pdf && "
done
"${SESSION[@]}" oneshot "$RID" -- bash -c "${QPDF_CMDS}true" >&2

# ---- 2. job di render ed extract ----------------------------------------------------------
RENDER_JOBS=$RAW/ocr-jobs/$RID/render.jsonl
EXTRACT_JOBS=$RAW/ocr-jobs/$RID/extract.jsonl
: > "$RENDER_JOBS"
: > "$EXTRACT_JOBS"
RENDERS=()
EXTRACTS=()

add_doc() {
  local set=$1 doc=$2 input=$3 oid=$4 dpi job out
  for dpi in 300 200; do
    job="render-$doc-$dpi-$RID"
    out="/results/raw/ocr/$job"
    printf '{"job_id":"%s","op":"ocr_render","input":"%s","out_dir":"%s","params":{"dpi":%d}}\n' \
      "$job" "$input" "$out" "$dpi" >> "$RENDER_JOBS"
    RENDERS+=("$(printf '{"job_id":"%s","set":"%s","doc":"%s","oracle_id":%s,"dpi":%d,"input":"%s","out_dir":"%s"}' \
      "$job" "$set" "$doc" "$oid" "$dpi" "$input" "$out")")
  done
  if [[ $set != geom ]]; then
    job="x-$doc-$RID"
    out="/results/raw/ocr-extract/$job"
    printf '{"job_id":"%s","op":"extract_pdf","input":"%s","out_dir":"%s","params":{}}\n' \
      "$job" "$input" "$out" >> "$EXTRACT_JOBS"
    EXTRACTS+=("$(printf '{"job_id":"%s","doc":"%s","input":"%s","out_dir":"%s"}' "$job" "$doc" "$input" "$out")")
  fi
}

for line in "${ORACLE[@]}"; do
  read -r oid _ _ <<< "$line"
  name=$(tr 'A-Z' 'a-z' <<< "$oid")
  add_doc oracle "$name" "/results/raw/ocr-src/$RID/$name.pdf" "\"$oid\""
done
add_doc doc25 doc25 "/data/corpus/raw/$DOC25.pdf" null
for g in geom-1 geom-2 geom-3 geom-4; do
  add_doc geom "$g" "/results/raw/geom/$RID/$g.pdf" null
done

# Il padre esce 2 se un job ha dato job_error (per esempio out_dir_exists): lo script si ferma.
log "render: $(wc -l < "$RENDER_JOBS") job ocr_render"
set +e
"${SESSION[@]}" exec "$RID" worker -- parse-parent batch --jobs "/results/raw/ocr-jobs/$RID/render.jsonl" \
  --concurrency 2 --limits "$RENDER_LIMITS" > "$RAW/ocr-runs/$RID/render.jsonl"
rc=$?
set -e
grep -q '"job_error"' "$RAW/ocr-runs/$RID/render.jsonl" && die 1 "job_error nel render: $(grep '"job_error"' "$RAW/ocr-runs/$RID/render.jsonl" | head -n 1)"
(( rc == 0 )) || die 1 "parse-parent batch (render) uscito con $rc"
grep -q '"outcome":{"status":"ok"' "$RAW/ocr-runs/$RID/render.jsonl" || die 1 "nessun render ok"
if grep -q '"outcome":{"status":"\(failed\|quarantined\)"' "$RAW/ocr-runs/$RID/render.jsonl"; then
  die 1 "render non riusciti: $(grep -c '"outcome":{"status":"\(failed\|quarantined\)"' "$RAW/ocr-runs/$RID/render.jsonl")"
fi

log "extract: $(wc -l < "$EXTRACT_JOBS") job extract_pdf"
set +e
"${SESSION[@]}" exec "$RID" worker -- parse-parent batch --jobs "/results/raw/ocr-jobs/$RID/extract.jsonl" \
  --concurrency 2 --limits "$EXTRACT_LIMITS" > "$RAW/ocr-runs/$RID/extract.jsonl"
rc=$?
set -e
grep -q '"job_error"' "$RAW/ocr-runs/$RID/extract.jsonl" && die 1 "job_error nell'extract"
(( rc == 0 )) || die 1 "parse-parent batch (extract) uscito con $rc"

# ---- 3. indice della run ------------------------------------------------------------------
join_json() { local IFS=,; printf '%s' "$*"; }
printf '{"run_id":"%s","renders":[%s],"extract":[%s]}\n' "$RID" "$(join_json "${RENDERS[@]}")" \
  "$(join_json "${EXTRACTS[@]}")" > "$RAW/ocr-index/$RID.json"
node -e 'JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"))' "$(nodepath "$RAW/ocr-index/$RID.json")" \
  || die 1 "indice non valido"
log "indice: results/raw/ocr-index/$RID.json"

# ---- 4. degradazione dichiarata (runner) ---------------------------------------------------
# Parametri immutabili delle Soglie: 200 DPI, -attenuate 0.5 +noise Gaussian, -distort SRT 1.5
# (rotazione attorno al centro a tela invariata), JPEG qualità 60, riletto come PNG a 200 DPI.
DEGRADE='set -euo pipefail
rid=$1; shift
for job in "$@"; do
  src=/src/s02-parser/results/raw/ocr/$job
  dst=/src/s02-parser/results/raw/ocr-degraded/$rid/$job
  mkdir -p "$dst"
  for png in "$src"/page-*.png; do
    stem=$(basename "$png" .png)
    magick "png:$png" -seed 1301 -attenuate 0.5 +noise Gaussian -distort SRT 1.5 -quality 60 "jpg:$dst/$stem.jpg"
    magick "jpg:$dst/$stem.jpg" -density 200 -units PixelsPerInch "png:$dst/$stem.png"
    rm -f "$dst/$stem.jpg"
    python3 - "$src/$stem.render.json" "$dst/$stem.png" "$dst/$stem.transform.json" <<PY
import json, struct, sys
r = json.load(open(sys.argv[1]))
with open(sys.argv[2], "rb") as f:
    head = f.read(24)
w, h = struct.unpack(">II", head[16:24])
assert (w, h) == (r["width_px"], r["height_px"]), "tela cambiata dalla degradazione"
t = {"dpi": float(r["dpi"]), "width_px": w, "height_px": h, "page_h_pt": r["page_h_pt"],
     "rotation_deg": 1.5, "center_px": [w / 2.0, h / 2.0], "noise": "-attenuate 0.5 +noise Gaussian (seed 1301)",
     "jpeg_quality": 60, "source_render": sys.argv[1].rsplit("/raw/", 1)[1], "source_sha256": r["source_sha256"]}
json.dump(t, open(sys.argv[3], "w"))
PY
  done
done'
JOBS200=()
for r in "${RENDERS[@]}"; do
  [[ $r == *'"dpi":200'* ]] && JOBS200+=("$(sed -E 's/.*"job_id":"([^"]+)".*/\1/' <<< "$r")")
done
log "degradazione: ${#JOBS200[@]} render a 200 DPI"
"${SESSION[@]}" oneshot "$RID" -- bash -c "$DEGRADE" _ "$RID" "${JOBS200[@]}" >&2

# ---- 5. PDF misto e sha256 -----------------------------------------------------------------
MIXED='set -euo pipefail
rid=$1; doc=$2; text_pages=$3; shift 3
src=/src/s02-parser/results/raw/ocr/render-doc25-300-$rid
dst=/src/s02-parser/results/raw/ocr-degraded/$rid
imgs=()
for p in "$@"; do
  n=$(printf "%04d" "$p")
  magick "png:$src/page-$n.png" -compress jpeg -quality 85 -density 300 -units PixelsPerInch "pdf:$dst/mixed-img-$n.pdf"
  imgs+=("$dst/mixed-img-$n.pdf")
done
qpdf --deterministic-id --empty --pages "/src/data/corpus/raw/$doc.pdf" "$text_pages" "${imgs[@]}" -- "$dst/mixed.pdf"
rm -f "${imgs[@]}"
cd /src/s02-parser/results/raw
{ find ocr-degraded/$rid -name "*.png" -o -name mixed.pdf; find ocr -path "*-$rid/*.png"; } | sort | xargs sha256sum > ocr-degraded/$rid/sha256.txt'
"${SESSION[@]}" oneshot "$RID" -- bash -c "$MIXED" _ "$RID" "$DOC25" "$MIXED_TEXT_PAGES" "${MIXED_IMAGE_PAGES[@]}" >&2

log "fatto: $(grep -c . "$RAW/ocr-degraded/$RID/sha256.txt") file con sha256, PDF misto ocr-degraded/$RID/mixed.pdf"
