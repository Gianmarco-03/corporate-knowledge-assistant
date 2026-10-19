#!/usr/bin/env bash
# spikes/tools/runner-smoke.sh <run_id> — inizializza i volumi del runner per uid 10001 e
# prova la scrittura come 10001, anche dopo la ricreazione del container, più la
# risoluzione DNS fra job del runner. Proprietario: piano 01-02 (WIRING W9), chiuso.
#
# Si lancia dentro una sessione prenotata (session.sh with … oppure acquire); è
# idempotente: i consumatori lo rilanciano se un volume del runner è stato ricreato.
# Non tocca i volumi del worker. Stampa `SMOKE: OK` o `SMOKE: FAIL — <passo>`.

set -euo pipefail
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

rid=${1-}
if [[ -z $rid || ! $rid =~ ^[a-z0-9][a-z0-9-]{0,62}$ || $rid == none ]]; then
  echo "runner-smoke.sh: run_id non valido" >&2
  exit 2
fi

SESSION="$(cd "$(dirname "$0")" && pwd)/session.sh"
DNS_JOB=cka-smoke-dns

fail() {
  echo "SMOKE: FAIL — $1"
  exit 1
}

cleanup() {
  bash "$SESSION" stop "$rid" "$DNS_JOB" >/dev/null 2>&1 || true
}
trap cleanup EXIT

# Sonda eseguita nel runner come 10001 (nessuna opzione docker: solo il comando).
# shellcheck disable=SC2016
PROBE='
set -eu
rid=$1
echo "uid=$(id -u) gid=$(id -g)"
[ "$(id -u)" = 10001 ] || { echo "PROBE-FAIL uid"; exit 11; }
t=$(mktemp -p /work/tmp) || { echo "PROBE-FAIL mktemp /work/tmp"; exit 12; }
rm -f "$t"
avail=$(df -BG --output=avail /work/tmp | tail -n 1 | tr -dc 0-9)
echo "work_tmp_avail_gib=$avail"
[ "$avail" -ge 20 ] || { echo "PROBE-FAIL spazio /work/tmp ${avail}G < 20G"; exit 13; }
for d in /target /usr/local/cargo/registry; do
  f="$d/.smoke-$rid"
  echo smoke > "$f" && rm -f "$f" || { echo "PROBE-FAIL scrittura $d"; exit 14; }
done
mkdir -p /src/results/raw
f="/src/results/raw/smoke-$rid.csv"
echo "a,b" > "$f" && rm -f "$f" || { echo "PROBE-FAIL scrittura /src/results/raw"; exit 15; }
rmdir /src/results/raw 2>/dev/null || true
rmdir /src/results 2>/dev/null || true
for tool in curl xargs qpdf pdftotext python3 oha; do
  command -v "$tool" >/dev/null || { echo "PROBE-FAIL strumento assente: $tool"; exit 16; }
done
echo "oha=$(oha --version 2>/dev/null | head -n 1)"
echo "mount (uid:gid permessi percorso):"
stat -c "  %u:%g %a %n" /target /usr/local/cargo/registry /work/tmp /src
echo PROBE-OK
'

echo "== 1. init-volumes (unico passo root, firma fissa)"
bash "$SESSION" init-volumes "$rid" || fail "init-volumes"

echo "== 2. sonda come 10001 (container nuovo)"
bash "$SESSION" oneshot "$rid" -- bash -c "$PROBE" probe "$rid" || fail "sonda 10001 (primo container)"

echo "== 3. sonda come 10001 (container ricreato)"
bash "$SESSION" oneshot "$rid" -- bash -c "$PROBE" probe "$rid" || fail "sonda 10001 (container ricreato)"

echo "== 4. DNS fra job del runner"
bash "$SESSION" run "$rid" "$DNS_JOB" -- sleep 120 >/dev/null || fail "avvio job $DNS_JOB"
if ! bash "$SESSION" oneshot "$rid" -- getent hosts "$DNS_JOB"; then
  fail "risoluzione DNS di $DNS_JOB"
fi
bash "$SESSION" stop "$rid" "$DNS_JOB" || fail "arresto job $DNS_JOB"

echo "SMOKE: OK"
