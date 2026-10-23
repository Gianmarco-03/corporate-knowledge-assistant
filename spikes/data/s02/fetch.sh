#!/usr/bin/env bash
# spikes/data/s02/fetch.sh — ricostruisce raw/ dei dataset di misura di S-02 (estrazione).
# Proprietario: piano 01-08 (WIRING W11, cartella data/s02). Idempotente: ogni worktree lo
# rilancia come precondizione prima di misurare.
#
# Cosa fa, nell'ordine:
#   1. legge manifest.json con node e valida ogni voce (item_id, file relativo senza «..»,
#      sha256, URL solo https, membro d'archivio facoltativo);
#   2. per ogni voce: se raw/<file> c'è e lo sha256 coincide lo salta; altrimenti lo mette
#      nell'elenco da scaricare (un file con sha256 diverso si scarta e si riscarica);
#   3. scarica solo i sottoinsiemi: per le voci con `member` legge con richieste HTTP Range
#      la directory centrale dell'archivio zip remoto e poi i soli byte del membro (TAT-DQA,
#      DocLayNet: gli archivi interi pesano da 155 MB a 30 GB); per le altre il file intero.
#      Ogni richiesta è un curl con solo https, al più 3 redirect, 300 s e 100 MiB;
#   4. scrive <file>.part, ne verifica lo sha256 con sha256sum e solo allora lo rinomina;
#      un mismatch scarta il file ed esce 1.
#
# Il manifest è immutabile: gli hash si sono fissati al primo download (2026-10-22). Se un
# archivio cambia, qui si esce 1; la correzione è una voce nuova con motivo in DATASET.md.
#
# Uso: bash spikes/data/s02/fetch.sh
# Variabili: CKA_FETCH_DOH_URL (facoltativa, solo https): risolutore DNS-over-HTTPS per curl,
# quando il DNS di sistema non risolve un host (successo il 2026-10-22 con l'object storage
# di DocLayNet).
#
# **Modifica:** Claude (gsd-executor 01-08) · 2026-10-22 · dataset TAT-DQA, DocLayNet, FinanceBench per S-02

set -euo pipefail

HERE=$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)
MANIFEST=$HERE/manifest.json
RAW=$HERE/raw
DOH=${CKA_FETCH_DOH_URL:-}

log() { printf 'fetch.sh: %s\n' "$*"; }
die() { local code=$1; shift; printf 'fetch.sh: ERRORE: %s\n' "$*" >&2; exit "$code"; }

[[ -f $MANIFEST ]] || die 2 "manifest assente: $MANIFEST"
if [[ -n $DOH && ! $DOH =~ ^https://[^[:space:]]+$ ]]; then
  die 2 "CKA_FETCH_DOH_URL deve essere un URL https"
fi
mkdir -p "$RAW"

# Percorso per node.exe su Windows; su Linux resta com'è.
native() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

sha_of() { sha256sum "$1" | cut -d' ' -f1 | tr -d '\\'; }

# Voci validate, una per riga, TAB: file sha256 source_url member ("-" se assente).
ENTRIES=$(node -e '
const fs = require("fs");
const m = JSON.parse(fs.readFileSync(process.argv[1], "utf8"));
if (!Array.isArray(m)) { console.error("manifest: atteso un array"); process.exit(2); }
const ids = new Set(), files = new Set();
for (const d of m) {
  const bad = (w) => { console.error("manifest: voce " + JSON.stringify(d.item_id) + ": " + w); process.exit(2); };
  if (typeof d.item_id !== "string" || !/^[a-z0-9][a-z0-9-]{0,63}$/.test(d.item_id)) bad("item_id non valido");
  if (ids.has(d.item_id)) bad("item_id duplicato");
  ids.add(d.item_id);
  if (!["TAT-DQA", "DocLayNet", "FinanceBench"].includes(d.dataset)) bad("dataset non previsto");
  if (!["pdf", "annotation"].includes(d.role)) bad("role non valido");
  if (typeof d.file !== "string" || !/^[a-z0-9-]+\/[A-Za-z0-9_.-]+$/.test(d.file) || d.file.includes("..")) bad("file non valido");
  if (files.has(d.file)) bad("file duplicato");
  files.add(d.file);
  if (!/^[0-9a-f]{64}$/.test(d.sha256 || "")) bad("sha256 non valido");
  if (!/^https:\/\/[^\s]+$/.test(d.source_url || "")) bad("source_url non https");
  if (d.member !== null && (typeof d.member !== "string" || !/^[A-Za-z0-9_.\/-]+$/.test(d.member) || d.member.includes(".."))) bad("member non valido");
  if (!d.license || !d.oracle_author) bad("licenza o autori assenti");
  console.log([d.file, d.sha256, d.source_url, d.member || "-"].join("\t"));
}
' "$(native "$MANIFEST")") || die 2 "manifest non valido"

TODO=$(mktemp)
trap 'rm -f "$TODO"' EXIT
total=0 present=0
while IFS=$'\t' read -r file sha url member; do
  [[ -n $file ]] || continue
  total=$((total + 1))
  dest=$RAW/$file
  if [[ -f $dest ]]; then
    if [[ $(sha_of "$dest") == "$sha" ]]; then
      present=$((present + 1))
      continue
    fi
    log "$file: sha256 diverso dal manifest, lo riscarico"
    rm -f "$dest"
  fi
  mkdir -p "$(dirname "$dest")"
  printf '%s\t%s\t%s\n' "$(native "$dest.part")" "$url" "$member" >> "$TODO"
done <<< "$ENTRIES"
log "$present/$total già presenti con sha256 ok"

if [[ -s $TODO ]]; then
  # Scaricamento: una sola lettura della directory centrale per archivio.
  node -e '
const fs = require("fs");
const zlib = require("zlib");
const { execFileSync } = require("child_process");
const [list, doh] = process.argv.slice(1);
const base = ["-fsSL", "--proto", "=https", "--max-redirs", "3", "--max-time", "300", "--max-filesize", "104857600"];
if (doh) base.push("--doh-url", doh);
function curl(args, attempt = 0) {
  try { return execFileSync("curl", [...base, ...args], { maxBuffer: 128 * 1024 * 1024 }); }
  catch (e) { if (attempt >= 4) throw e; execFileSync("sleep", [String(2 + attempt * 2)]); return curl(args, attempt + 1); }
}
const range = (url, a, b) => curl(["-r", `${a}-${b}`, url]);
function size(url) {
  const h = curl(["-r", "0-0", "-o", require("path").join(require("os").tmpdir(), "s02-fetch-probe-" + process.pid), "-D", "-", url]).toString("latin1");
  const m = [...h.matchAll(/content-range:\s*bytes\s+\d+-\d+\/(\d+)/gi)].pop();
  if (!m) throw new Error("dimensione dell'\''archivio ignota: " + url);
  return Number(m[1]);
}
function directory(url) {
  const n = size(url);
  const tail = range(url, Math.max(0, n - 65536), n - 1);
  const i = tail.lastIndexOf(Buffer.from([0x50, 0x4b, 0x05, 0x06]));
  if (i < 0) throw new Error("fine della directory centrale non trovata: " + url);
  let cdSize = tail.readUInt32LE(i + 12), cdOff = tail.readUInt32LE(i + 16);
  const z = tail.lastIndexOf(Buffer.from([0x50, 0x4b, 0x06, 0x06]));
  if (z >= 0) { cdSize = Number(tail.readBigUInt64LE(z + 40)); cdOff = Number(tail.readBigUInt64LE(z + 48)); }
  const cd = range(url, cdOff, cdOff + cdSize - 1);
  const dir = new Map();
  for (let p = 0; p + 46 <= cd.length && cd.readUInt32LE(p) === 0x02014b50;) {
    const method = cd.readUInt16LE(p + 10);
    let csize = cd.readUInt32LE(p + 20), usize = cd.readUInt32LE(p + 24), off = cd.readUInt32LE(p + 42);
    const fl = cd.readUInt16LE(p + 28), el = cd.readUInt16LE(p + 30), cl = cd.readUInt16LE(p + 32);
    const name = cd.subarray(p + 46, p + 46 + fl).toString("utf8");
    const ex = cd.subarray(p + 46 + fl, p + 46 + fl + el);
    for (let q = 0; q + 4 <= ex.length;) {
      const id = ex.readUInt16LE(q), len = ex.readUInt16LE(q + 2);
      if (id === 1) {
        let r = q + 4;
        if (usize === 0xffffffff) { usize = Number(ex.readBigUInt64LE(r)); r += 8; }
        if (csize === 0xffffffff) { csize = Number(ex.readBigUInt64LE(r)); r += 8; }
        if (off === 0xffffffff) off = Number(ex.readBigUInt64LE(r));
      }
      q += 4 + len;
    }
    dir.set(name, { method, csize, usize, off });
    p += 46 + fl + el + cl;
  }
  return dir;
}
const dirs = new Map();
for (const line of fs.readFileSync(list, "utf8").split("\n").filter(Boolean)) {
  const [part, url, member] = line.split("\t");
  if (member === "-") { fs.writeFileSync(part, curl([url])); continue; }
  if (!dirs.has(url)) dirs.set(url, directory(url));
  const e = dirs.get(url).get(member);
  if (!e) throw new Error("membro assente nell'\''archivio: " + member);
  if (![0, 8].includes(e.method)) throw new Error("compressione non supportata: " + member);
  if (e.usize > 104857600) throw new Error("membro oltre 100 MiB: " + member);
  const buf = range(url, e.off, e.off + 30 + 65535 + 65535 + e.csize);
  if (buf.readUInt32LE(0) !== 0x04034b50) throw new Error("intestazione locale non valida: " + member);
  const start = 30 + buf.readUInt16LE(26) + buf.readUInt16LE(28);
  const data = buf.subarray(start, start + e.csize);
  const out = e.method === 8 ? zlib.inflateRawSync(data, { maxOutputLength: e.usize }) : data;
  if (out.length !== e.usize) throw new Error("lunghezza decompressa diversa dalla directory: " + member);
  fs.writeFileSync(part, out);
}
' "$(native "$TODO")" "$DOH" || die 1 "download fallito"

  fail=0
  while IFS=$'\t' read -r file sha url member; do
    [[ -n $file ]] || continue
    dest=$RAW/$file
    [[ -f $dest.part ]] || continue
    if [[ $(sha_of "$dest.part") == "$sha" ]]; then
      mv -f "$dest.part" "$dest"
      log "$file: scaricato, sha256 ok"
    else
      rm -f "$dest.part"
      printf 'fetch.sh: ERRORE: %s: sha256 diverso dal manifest, file scartato\n' "$file" >&2
      fail=1
    fi
  done <<< "$ENTRIES"
  (( fail == 0 )) || exit 1
fi

# Verifica finale: tutte le voci presenti con lo sha256 del manifest.
while IFS=$'\t' read -r file sha url member; do
  [[ -n $file ]] || continue
  [[ -f $RAW/$file && $(sha_of "$RAW/$file") == "$sha" ]] || die 1 "$file: assente o sha256 diverso"
done <<< "$ENTRIES"
log "ok: $total voci verificate in raw/"
