#!/usr/bin/env bash
# spikes/data/corpus/fetch.sh — ricostruisce raw/ del corpus comune degli spike.
# Proprietario: piano 01-03 (WIRING W11). Idempotente: ogni worktree lo rilancia come
# precondizione prima di misurare.
#
# Cosa fa, nell'ordine:
#   1. legge manifest.json con node e valida ogni voce (doc_id, sha256, URL solo https);
#   2. per ogni voce `public`: se raw/<doc_id>.<format> c'è e lo sha256 coincide lo salta
#      («già presente, sha256 ok»); se c'è ma lo sha256 non coincide lo scarta e lo
#      riscarica; scarica con curl solo https, redirect, tempo e dimensione limitati, in un
#      file .part, verifica lo sha256 e solo allora lo rinomina; mismatch = file scartato,
#      uscita 1;
#   3. per ogni voce `derived` in PDF: la produce con qpdf nel runner (session.sh oneshot
#      se il chiamante ha già una sessione e ha esportato CKA_RUN_ID, altrimenti
#      session.sh with 01-03, che prenota la finestra di misura e in uscita fa down +
#      release) e ne verifica lo sha256; se nessun derivato manca, nessun container parte;
#   4. verifica lo sha256 dei file versionati in synthetic/ (sorgenti md e DOCX derivati).
#
# Il manifest è immutabile: gli hash si sono fissati al primo download riuscito. Se un
# emittente sostituisce un file, qui si esce 1; la correzione è una voce nuova con motivo
# in CORPUS.md, mai un hash riscritto.
#
# Uso: bash spikes/data/corpus/fetch.sh
# Variabili: CKA_SPIKES_ENV (file segreti di 01-02; serve solo a session.sh, qui se ne
# controlla la presenza e non si legge), CKA_RUN_ID (sessione già prenotata, opzionale),
# CKA_FETCH_WAIT_S (attesa massima della prenotazione, default 900 s).

set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
# Radice del repo dalla posizione fissa del file (spikes/data/corpus/), non da `git -C`:
# dentro session.sh with la conversione dei percorsi MSYS è spenta e git.exe non
# risolverebbe un percorso /c/....
REPO=$(cd "$HERE/../../.." && pwd)
MANIFEST=$HERE/manifest.json
RAW=$HERE/raw
SYN=$HERE/synthetic
SESSION=$REPO/spikes/tools/session.sh
CKA_SPIKES_ENV=${CKA_SPIKES_ENV:-$HOME/.cka-spikes/spikes.env}
WAIT_S=${CKA_FETCH_WAIT_S:-900}
RETRIES=3

log() { printf 'fetch.sh: %s\n' "$*"; }
die() { local code=$1; shift; printf 'fetch.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

# session.sh risolve compose.yaml dalla radice git della directory corrente.
cd "$REPO"

[[ -f $SESSION ]] || die 2 "session.sh assente: atteso in $SESSION"
[[ -f $MANIFEST ]] || die 2 "manifest assente: $MANIFEST"
[[ $WAIT_S =~ ^[0-9]+$ ]] || die 2 "CKA_FETCH_WAIT_S non numerico"
mkdir -p "$RAW"

# Percorso per programmi nativi Windows (node.exe): sotto session.sh la conversione MSYS
# automatica è spenta, quindi si converte qui; su Linux cygpath non c'è e il percorso resta.
native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

sha_of() { sha256sum "$1" | cut -d' ' -f1 | tr -d '\\'; }

# Voci del manifest, una per riga, campi separati da TAB:
# doc_id kind format source_url sha256 derived_from pages.
# La validazione sta qui, prima di qualsiasi accesso a rete o file system.
ENTRIES=$(node -e '
const fs = require("fs");
const m = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
if (!Array.isArray(m)) { console.error("manifest: atteso un array"); process.exit(2); }
const ids = new Set();
for (const d of m) {
  const bad = (w) => { console.error("manifest: voce " + JSON.stringify(d.doc_id) + ": " + w); process.exit(2); };
  if (typeof d.doc_id !== "string" || !/^[a-z0-9][a-z0-9-]{0,95}$/.test(d.doc_id)) bad("doc_id non valido");
  if (ids.has(d.doc_id)) bad("doc_id duplicato");
  ids.add(d.doc_id);
  if (!/^[0-9a-f]{64}$/.test(d.sha256 || "")) bad("sha256 non valido");
  if (!["public", "synthetic", "derived"].includes(d.kind)) bad("kind non valido");
  if (!["pdf", "docx", "md"].includes(d.format)) bad("format non valido");
  if (d.kind === "public" && !/^https:\/\/[^\s]+$/.test(d.source_url || "")) bad("source_url non https");
  if (d.kind === "derived" && !ids.has(d.derived_from)) bad("derived_from assente o successivo alla voce");
  if (!Number.isInteger(d.pages) || d.pages < 1) bad("pages non valido");
  console.log([d.doc_id, d.kind, d.format, d.source_url || "-", d.sha256, d.derived_from || "-", d.pages].join("\t"));
}
' "$(native "$MANIFEST")" | tr -d '\r') || die 2 "manifest non valido"

failed=0

# --- 2. documenti pubblici --------------------------------------------------------------
download() {
  local url=$1 out=$2 i
  for ((i = 1; i <= RETRIES; i++)); do
    if curl -fsSL --proto '=https' --max-redirs 3 --max-time 300 --max-filesize 104857600 \
         -o "$out" "$url"; then
      return 0
    fi
    rm -f "$out"
    log "tentativo $i/$RETRIES fallito: $url"
    (( i < RETRIES )) && sleep $(( i * 5 ))
  done
  return 1
}

while IFS=$'\t' read -r id kind fmt url sha from pages; do
  [[ $kind == public ]] || continue
  dest=$RAW/$id.$fmt
  if [[ -f $dest ]]; then
    if [[ $(sha_of "$dest") == "$sha" ]]; then
      log "già presente, sha256 ok: $id"
      continue
    fi
    log "sha256 errato su file presente, scartato e riscaricato: $id"
    rm -f "$dest"
  fi
  part=$RAW/.part-$id.$fmt
  if ! download "$url" "$part"; then
    log "download fallito: $id"
    failed=1
    continue
  fi
  got=$(sha_of "$part")
  if [[ $got != "$sha" ]]; then
    rm -f "$part"
    log "sha256 NON coincide, file scartato: $id (atteso $sha, ottenuto $got)"
    failed=1
    continue
  fi
  mv -f "$part" "$dest"
  log "scaricato, sha256 ok: $id"
done <<< "$ENTRIES"

(( failed == 0 )) || die 1 "almeno un documento pubblico non è disponibile con lo sha256 del manifest"

# --- 3. derivati PDF (qpdf nel runner) --------------------------------------------------
missing=()
while IFS=$'\t' read -r id kind fmt url sha from pages; do
  [[ $kind == derived && $fmt == pdf ]] || continue
  dest=$RAW/$id.pdf
  if [[ -f $dest && $(sha_of "$dest") == "$sha" ]]; then
    log "già presente, sha256 ok: $id"
    continue
  fi
  [[ -f $RAW/$from.pdf ]] || die 1 "sorgente del derivato assente: $from"
  rm -f "$dest"
  missing+=("$id"$'\t'"$from"$'\t'"$pages"$'\t'"$sha")
done <<< "$ENTRIES"

if (( ${#missing[@]} > 0 )); then
  [[ -f $CKA_SPIKES_ENV ]] || die 2 "file segreti assente (CKA_SPIKES_ENV): crearlo come da 01-02 Task 1"
  if [[ -z ${CKA_RUN_ID-} ]]; then
    # Nessuna sessione del chiamante: se ne prenota una sola per tutti i derivati e ci si
    # rilancia dentro; il ramo dei pubblici sopra non riscarica nulla.
    [[ -z ${CKA_FETCH_IN_SESSION-} ]] || die 2 "dentro la sessione manca CKA_RUN_ID"
    log "derivati mancanti: ${#missing[@]}, prenoto la finestra con session.sh with 01-03"
    CKA_FETCH_IN_SESSION=1 exec bash "$SESSION" with 01-03 --wait "$WAIT_S" -- \
      bash "$HERE/fetch.sh"
  fi
  for row in "${missing[@]}"; do
    IFS=$'\t' read -r id from pages sha <<< "$row"
    part=.part-$id.pdf
    # Percorsi del container: il runner monta spikes/ in /src. --deterministic-id rende
    # l'output riproducibile byte per byte con la stessa versione di qpdf.
    bash "$SESSION" oneshot "$CKA_RUN_ID" -- \
      qpdf --deterministic-id "/src/data/corpus/raw/$from.pdf" --pages . "1-$pages" -- \
      "/src/data/corpus/raw/$part"
    got=$(sha_of "$RAW/$part")
    if [[ $got != "$sha" ]]; then
      rm -f "$RAW/$part"
      die 1 "sha256 del derivato NON coincide, file scartato: $id (atteso $sha, ottenuto $got)"
    fi
    mv -f "$RAW/$part" "$RAW/$id.pdf"
    log "derivato prodotto, sha256 ok: $id"
  done
fi

# --- 4. file versionati in synthetic/ ---------------------------------------------------
while IFS=$'\t' read -r id kind fmt url sha from pages; do
  # Sorgente sintetico: synthetic/<doc_id>.md. DOCX derivato: synthetic/<derived_from>.docx
  # (stesso nome del sorgente, doc_id distinto con suffisso -docx).
  case $kind/$fmt in
    synthetic/md) name=$id.md ;;
    derived/docx) name=$from.docx ;;
    *) continue ;;
  esac
  f=$SYN/$name
  [[ -f $f ]] || die 1 "file versionato assente: synthetic/$name"
  [[ $(sha_of "$f") == "$sha" ]] || die 1 "sha256 NON coincide: synthetic/$name (rilanciare synthetic/build.sh o correggere il manifest con motivo in CORPUS.md)"
  log "versionato, sha256 ok: synthetic/$name ($id)"
done <<< "$ENTRIES"

log "corpus completo: ogni file coincide con lo sha256 del manifest"
