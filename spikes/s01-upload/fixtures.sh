#!/usr/bin/env bash
# spikes/s01-upload/fixtures.sh — fixture dello spike S-01, NEL RUNNER (piano 01-04).
#
# Uso (dall'host, dentro una sessione prenotata):
#   bash spikes/tools/session.sh oneshot "$run_id" -- bash s01-upload/fixtures.sh
#
# Output in /work/tmp/fixtures (volume Linux work-tmp, mai versionato):
# - pool di 200 PDF validi distinti per 1, 10, 24 e 25 MiB (`probe gen-pdf`, semi 1..200);
# - 10 MiB, semi 201..211: corpo del carico dedup (201) e riscaldamento new (202..211);
# - PDF di confine 26_214_399 / 26_214_400 / 26_214_401 byte;
# - corpo artificiale da 100 MiB (unico corpo non PDF valido, per il 413);
# - casi di formato in fmt/: Markdown, PDF e DOCX sintetici, polyglot, zip bomb, DOCX con
#   DTD/entità, PDF cifrato (qpdf), PDF con /JavaScript, /J#61vaScript, /Launch e
#   /EmbeddedFile, magic mismatch (MZ davanti a un PDF), PDF troncato. Un PDF e un DOCX
#   reali da /src/data/corpus/raw se presenti (real.pdf, real.docx), altrimenti solo
#   i casi `synthetic`.
# Per ogni fixture una riga di manifest.tsv: percorso, `stat -c %s`, `sha256sum`.
# Idempotente: una fixture già presente con la dimensione giusta non si rigenera.

set -euo pipefail

OUT=/work/tmp/fixtures
PROBE=/target/release/probe
MIB=1048576
POOL=200

[[ -x $PROBE ]] || { echo "fixtures.sh: $PROBE assente (compilare prima)" >&2; exit 2; }
mkdir -p "$OUT"

# Spazio: 200 × (1 + 10 + 24 + 25) MiB ≈ 11,7 GiB più il resto; sotto 20 GiB liberi il
# pool scende a 100 (deviazione da annotare in RISULTATI).
avail_kb=$(df -Pk /work/tmp | awk 'NR==2 {print $4}')
if (( avail_kb < 20 * 1024 * 1024 )); then
  POOL=100
  echo "fixtures.sh: DEVIAZIONE spazio libero ${avail_kb} KiB < 20 GiB: pool da $POOL" >&2
fi
echo "fixtures.sh: spazio libero ${avail_kb} KiB, pool $POOL"

gen_pool() { # <etichetta> <byte> <seme_da> <quanti>
  local label=$1 size=$2 from=$3 count=$4 dir="$OUT/$1" seed missing=0
  mkdir -p "$dir"
  for (( seed = from; seed < from + count; seed++ )); do
    local f; f=$(printf '%s/%s-%03d.pdf' "$dir" "$label" "$seed")
    if [[ ! -f $f || $(stat -c %s "$f") -ne $size ]]; then missing=1; break; fi
  done
  if (( missing )); then
    "$PROBE" gen-pdf --size "$size" --seed-from "$from" --count "$count" --dir "$dir" --prefix "$label"
  fi
}

gen_pool 1m $((1 * MIB)) 1 "$POOL"
gen_pool 10m $((10 * MIB)) 1 "$POOL"
gen_pool 24m $((24 * MIB)) 1 "$POOL"
gen_pool 25m $((25 * MIB)) 1 "$POOL"
gen_pool 10m $((10 * MIB)) 201 11
gen_pool edge-399 26214399 1 1
gen_pool edge-400 26214400 1 1
gen_pool edge-401 26214401 1 1

mkdir -p "$OUT/fmt"
cd "$OUT/fmt"

# 100 MiB artificiale (413 atteso).
if [[ ! -f art-100m.pdf || $(stat -c %s art-100m.pdf) -ne $((100 * MIB)) ]]; then
  { printf '%%PDF-1.7\n'; head -c $((100 * MIB - 9)) /dev/zero; } > art-100m.pdf
fi

printf '# Relazione trimestrale\n\nUtile netto in crescita del 4 %%.\n' > nota.md
"$PROBE" gen-pdf --size 200000 --seed-from 900 --count 1 --dir . --prefix synthetic
mv -f synthetic-900.pdf synthetic.pdf

# Documenti reali, se il corpus è già stato popolato (altrimenti nessun file real.*).
rm -f real.pdf real.docx
for ext in pdf docx; do
  src=$(find /src/data/corpus/raw -maxdepth 2 -type f -name "*.$ext" 2>/dev/null | sort | head -n 1 || true)
  if [[ -n $src ]]; then cp -f "$src" "real.$ext"; echo "fixtures.sh: real.$ext da corpus"; fi
done

# Mini PDF con un'azione nel catalogo (prefiltro → quarantena).
mini_pdf() { # <extra nel catalogo>
  printf '%%PDF-1.7\n1 0 obj\n<< /Type /Catalog /Pages 2 0 R %s >>\nendobj\n2 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\ntrailer\n<< /Root 1 0 R >>\n%%%%EOF\n' "$1"
}
mini_pdf '/OpenAction << /S /JavaScript /JS (app.alert(1)) >>' > javascript.pdf
mini_pdf '/OpenAction << /S /J#61vaScript /JS (app.alert(1)) >>' > javascript-offuscato.pdf
mini_pdf '/OpenAction << /S /Launch /F (calc.exe) >>' > launch.pdf
mini_pdf '/Names << /EmbeddedFiles << /Names [(a) << /Type /EmbeddedFile >>] >> >>' > embedded.pdf

# Magic mismatch: nome .pdf, intestazione eseguibile (MZ) prima di un PDF valido.
{ printf 'MZ\220\000'; mini_pdf ''; } > magic-mismatch.pdf

# PDF cifrato davvero (qpdf, AES-256), dal PDF sintetico.
qpdf --encrypt utente proprietario 256 -- synthetic.pdf encrypted.pdf

# PDF troncato: metà di una fixture da 1 MiB (niente %%EOF in coda).
head -c $((MIB / 2)) "$OUT/1m/1m-001.pdf" > truncated.pdf

python3 - <<'PY'
import zipfile, io

CT = ('<?xml version="1.0" encoding="UTF-8"?><Types xmlns="http://schemas.openxmlformats.org/'
      'package/2006/content-types"><Override PartName="/word/document.xml" ContentType="'
      'application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"/></Types>')
DOC = ('<?xml version="1.0" encoding="UTF-8"?><w:document xmlns:w="http://schemas.openxmlformats.'
       'org/wordprocessingml/2006/main"><w:body><w:p><w:r><w:t>Relazione</w:t></w:r></w:p>'
       '</w:body></w:document>')
DTD = ('<?xml version="1.0"?><!DOCTYPE w [<!ENTITY a "aaaaaaaaaa">]><w:document xmlns:w='
       '"http://schemas.openxmlformats.org/wordprocessingml/2006/main"><w:body>&a;</w:body>'
       '</w:document>')

def docx(path, doc, extra=None):
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        z.writestr("[Content_Types].xml", CT)
        z.writestr("word/document.xml", doc)
        if extra:
            extra(z)

docx("synthetic.docx", DOC)
docx("dtd.docx", DTD)

def bomb(z):
    # 1 GiB di zeri dichiarati: la somma dichiarata supera 200 MiB.
    with z.open("word/media/bomb.bin", "w", force_zip64=True) as f:
        block = b"\0" * (1 << 20)
        for _ in range(1024):
            f.write(block)
docx("zipbomb.docx", DOC, bomb)

# Polyglot: archivio ZIP (magic PK) con nome .pdf e un PDF dentro, senza parti DOCX.
with zipfile.ZipFile("polyglot.pdf", "w") as z:
    z.writestr("doc.pdf", "%PDF-1.7\n%%EOF\n")
PY

cd "$OUT"
find . -type f \( -name '*.pdf' -o -name '*.docx' -o -name '*.md' \) -printf '%P\n' | sort \
  | while read -r f; do printf '%s\t%s\t%s\n' "$f" "$(stat -c %s "$f")" "$(sha256sum "$f" | cut -d' ' -f1)"; done \
  > manifest.tsv
echo "fixtures.sh: $(wc -l < manifest.tsv) fixture in manifest.tsv"

# Validatore indipendente su un campione di 5 PDF per dimensione.
for label in 1m 10m 24m 25m; do
  for seed in 1 50 100 150 200; do
    (( seed <= POOL )) || continue
    f=$(printf '%s/%s-%03d.pdf' "$label" "$label" "$seed")
    qpdf --check "$f" > /dev/null || { echo "fixtures.sh: qpdf --check fallito su $f" >&2; exit 1; }
  done
done
echo "fixtures.sh: qpdf --check verde sul campione"
