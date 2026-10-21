#!/usr/bin/env bash
# spikes/data/corpus/synthetic/build.sh — converte quattro sorgenti sintetici in DOCX.
# Proprietario: piano 01-03 (WIRING W11).
#
# syn-01, syn-02, syn-05 e syn-09 diventano DOCX con pandoc da un'immagine fissata per tag
# e digest, stili di intestazione Word standard (Heading1, Heading2, …) e interruzioni di
# pagina manuali: ogni riga `\newpage` del Markdown diventa un <w:br w:type="page"/> grazie
# a un filtro Lua scritto qui sotto. SOURCE_DATE_EPOCH fisso rende l'output riproducibile
# byte per byte, così lo sha256 del manifest resta valido a ogni ricostruzione.
#
# Contro il manifest (immutabile):
#   - lo sha256 di ogni sorgente .md deve coincidere, altrimenti uscita 1 prima di pandoc;
#   - il DOCX prodotto si installa in synthetic/ solo se coincide con lo sha256 registrato;
#     se non coincide si esce 1 senza toccare il file versionato;
#   - un DOCX non ancora registrato si installa e se ne stampa lo sha256, con uscita 1:
#     la registrazione nel manifest è un passo esplicito, mai automatico.
#
# Il container gira dentro una sessione di session.sh: se il chiamante ha già una sessione
# ed esporta CKA_RUN_ID la usa, altrimenti si rilancia in `session.sh with 01-03`. Il
# container porta l'etichetta cka.run_id, così `down` della sessione lo ripulisce anche in
# caso di interruzione; gira senza rete, senza capability e con uid 10001.
#
# Uso: bash spikes/data/corpus/synthetic/build.sh

set -euo pipefail
export MSYS_NO_PATHCONV=1

readonly PANDOC_IMAGE='pandoc/core:3.11.0.0@sha256:62518bfb6833705dc90a1634d8d879299bd820b3f74ad3b32fba8b618452553a'
readonly SOURCE_DATE_EPOCH_FIXED=1767225600   # 2026-01-01T00:00:00Z
readonly DOCX_SOURCES=(syn-01-budget-interno syn-02-piano-industriale-riservato
  syn-05-risk-report-confidential-en syn-09-trappola-restricted)

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
CORPUS=$(cd "$HERE/.." && pwd)
REPO=$(cd "$HERE/../../../.." && pwd)
MANIFEST=$CORPUS/manifest.json
SESSION=$REPO/spikes/tools/session.sh
CKA_SPIKES_ENV=${CKA_SPIKES_ENV:-$HOME/.cka-spikes/spikes.env}
WAIT_S=${CKA_FETCH_WAIT_S:-900}

log() { printf 'build.sh: %s\n' "$*"; }
die() { local code=$1; shift; printf 'build.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }
native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}
sha_of() { sha256sum "$1" | cut -d' ' -f1 | tr -d '\\'; }

cd "$REPO"
[[ -f $SESSION ]] || die 2 "session.sh assente: atteso in $SESSION"
[[ -f $MANIFEST ]] || die 2 "manifest assente: $MANIFEST"
[[ $WAIT_S =~ ^[0-9]+$ ]] || die 2 "CKA_FETCH_WAIT_S non numerico"

# sha256 registrato per doc_id ("-" se la voce non esiste).
expected_sha() {
  node -e '
const m = JSON.parse(require("fs").readFileSync(process.argv[1], "utf8"));
const d = m.find((x) => x.doc_id === process.argv[2]);
console.log(d && /^[0-9a-f]{64}$/.test(d.sha256) ? d.sha256 : "-");
' "$(native "$MANIFEST")" "$1" | tr -d '\r'
}

# 1. sorgenti Markdown contro il manifest, prima di avviare qualsiasi container.
for src in "${DOCX_SOURCES[@]}"; do
  f=$HERE/$src.md
  [[ -f $f ]] || die 1 "sorgente assente: synthetic/$src.md"
  want=$(expected_sha "$src")
  [[ $want != - ]] || die 1 "sorgente non registrato nel manifest: $src"
  [[ $(sha_of "$f") == "$want" ]] || die 1 "sha256 del sorgente NON coincide: synthetic/$src.md"
done
log "sorgenti md: sha256 ok"

# 2. sessione di misura.
if [[ -z ${CKA_RUN_ID-} ]]; then
  [[ -z ${CKA_BUILD_IN_SESSION-} ]] || die 2 "dentro la sessione manca CKA_RUN_ID"
  [[ -f $CKA_SPIKES_ENV ]] || die 2 "file segreti assente (CKA_SPIKES_ENV): crearlo come da 01-02 Task 1"
  log "prenoto la finestra con session.sh with 01-03"
  CKA_BUILD_IN_SESSION=1 exec bash "$SESSION" with 01-03 --wait "$WAIT_S" -- bash "$HERE/build.sh"
fi
[[ $CKA_RUN_ID =~ ^[a-z0-9][a-z0-9-]{0,62}$ ]] || die 2 "CKA_RUN_ID non valido"

# 3. conversione in una cartella temporanea fuori dal repository.
WORK=$(mktemp -d)
trap 'rm -rf "$WORK"' EXIT
mkdir -p "$WORK/in" "$WORK/out"
cat > "$WORK/pagebreak.lua" <<'LUA'
-- Una riga `\newpage` (blocco TeX grezzo) diventa un'interruzione di pagina Word.
function RawBlock(el)
  if (el.format == 'tex' or el.format == 'latex') and el.text:match('^%s*\\newpage%s*$') then
    return pandoc.RawBlock('openxml', '<w:p><w:r><w:br w:type="page"/></w:r></w:p>')
  end
end
LUA
for src in "${DOCX_SOURCES[@]}"; do cp "$HERE/$src.md" "$WORK/in/"; done

for src in "${DOCX_SOURCES[@]}"; do
  docker run --rm --network none --cap-drop ALL --security-opt no-new-privileges:true \
    --user 10001:10001 --label "cka.run_id=$CKA_RUN_ID" \
    -e SOURCE_DATE_EPOCH="$SOURCE_DATE_EPOCH_FIXED" \
    -v "$(native "$WORK"):/work" -w /work \
    "$PANDOC_IMAGE" \
    --from markdown --to docx --lua-filter /work/pagebreak.lua \
    --output "/work/out/$src.docx" "/work/in/$src.md"
done

# 4. confronto con il manifest e installazione.
unregistered=0
for src in "${DOCX_SOURCES[@]}"; do
  out=$WORK/out/$src.docx
  [[ -s $out ]] || die 1 "pandoc non ha prodotto $src.docx"
  got=$(sha_of "$out")
  want=$(expected_sha "$src-docx")
  if [[ $want == - ]]; then
    cp "$out" "$HERE/$src.docx"
    log "NON REGISTRATO: synthetic/$src.docx sha256=$got bytes=$(wc -c < "$out" | tr -d ' ')"
    unregistered=1
  elif [[ $got == "$want" ]]; then
    cp "$out" "$HERE/$src.docx"
    log "DOCX ok, sha256 coincide: synthetic/$src.docx"
  else
    die 1 "sha256 del DOCX NON coincide: $src.docx (atteso $want, ottenuto $got); file versionato non toccato"
  fi
done

(( unregistered == 0 )) || die 1 "DOCX non registrati nel manifest: registrarli con kind derived e derived_from"
log "DOCX ricostruiti e coincidenti con il manifest"
