#!/usr/bin/env bash
# spikes/s02-parser/measure/run_oom.sh <out.json>
#
# Prova OOM controllata (piano 01-18, Task 3; soglia M11). Gira dentro
# `bash spikes/tools/session.sh with 01-18 -- …` dalla radice del worktree e usa CKA_RUN_ID.
#   1. compila nel runner il test `tests/oom.rs` (`session.sh oneshot … cargo test
#      --no-run --message-format=json`) e ne estrae il percorso dell'eseguibile (campo
#      `executable` del messaggio `compiler-artifact` del target `oom`);
#   2. trova il volume `/target` del servizio runner per etichetta compose (esattamente uno,
#      altrimenti exit 2); mai la configurazione risolta del progetto, che stamperebbe i
#      segreti del runner;
#   3. lancia l'eseguibile in un container usa e getta a memoria nota (256 MiB, swap uguale),
#      senza rete, read-only, senza capability, utente 10001, con l'etichetta della sessione
#      (lo rimuove `session.sh down` se resta appeso);
#   4. scrive <out.json> dalla riga `OOM_RESULT` con `container_memory`, `container_exit` e
#      `image`; senza riga (es. 137: il cgroup ha ucciso il test stesso) scrive
#      {"esito":"fallito","motivo":"parent_killed_or_no_result",…} ed esce 1.
# Il JSON non contiene variabili d'ambiente. Exit 0 con esito `verificato` o
# `non_esercitato`, 1 con `fallito`.
#
# **Modifica:** Claude (Anthropic) · 2026-10-22 · prova OOM nel container a memoria nota (01-18)

set -euo pipefail
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

out=${1-}
if [[ -z $out ]]; then
  echo "uso: run_oom.sh <out.json>" >&2
  exit 2
fi
if [[ -z ${CKA_RUN_ID-} ]]; then
  echo "run_oom.sh: CKA_RUN_ID assente (lanciare dentro session.sh with 01-18)" >&2
  exit 2
fi

S=spikes/tools/session.sh
MEM=256m
raw=spikes/s02-parser/results/raw/isolation-oom
mkdir -p "$raw"
rm -f "$out"

# (1) compilazione nel runner ed eseguibile del test.
bash $S oneshot "$CKA_RUN_ID" -- \
  cargo test -p s02-parser --test oom --no-run --message-format=json \
  > "$raw/cargo-messages.jsonl" 2> "$raw/cargo-stderr.txt"
exe=$(node -e '
const lines = require("fs").readFileSync(0, "utf8").split("\n");
const hits = [];
for (const l of lines) {
  if (!l.startsWith("{")) continue;
  let m; try { m = JSON.parse(l); } catch { continue; }
  if (m.reason === "compiler-artifact" && m.target && m.target.name === "oom" && m.executable) hits.push(m.executable);
}
if (hits.length !== 1) { console.error("eseguibili del target oom: " + hits.length); process.exit(2); }
console.log(hits[0]);
' < "$raw/cargo-messages.jsonl" | tr -d '\r')
[[ $exe == /target/* ]] || { echo "run_oom.sh: eseguibile fuori da /target: $exe" >&2; exit 2; }

# (2) volume /target del runner, per etichetta.
vols=$(docker volume ls -q --filter label=com.docker.compose.project=cka-spikes \
  --filter label=com.docker.compose.volume=cargo-target | tr -d '\r')
if [[ $(printf '%s\n' $vols | grep -c .) -ne 1 ]]; then
  echo "run_oom.sh: volume cargo-target non univoco o assente" >&2
  exit 2
fi
image=$(docker image inspect cka-spikes-runner --format '{{.Id}}' | tr -d '\r')

# (3) container usa e getta a memoria nota.
code=0
docker run --rm --label cka.run_id="$CKA_RUN_ID" --network none --memory 256m --memory-swap 256m \
  --pids-limit 256 --cap-drop ALL --security-opt no-new-privileges --read-only \
  --user 10001:10001 --tmpfs /work/tmp:size=16m,uid=10001,gid=10001 \
  -v "$vols":/target:ro cka-spikes-runner \
  "$exe" --ignored --exact oom_kills_child_not_parent --nocapture --test-threads 1 \
  > "$raw/test-stdout.txt" 2> "$raw/test-stderr.txt" || code=$?

# (4) oom.json dalla riga OOM_RESULT.
node -e '
const fs = require("fs");
const [outPath, mem, code, image] = process.argv.slice(1);
const text = fs.readFileSync(0, "utf8").replace(/\r/g, "");
// libtest con --nocapture stampa `test <nome> ... ` sulla stessa riga, prima del risultato:
// si cerca il marcatore ovunque nella riga, e deve comparire una volta sola.
const tag = "OOM_RESULT ";
const lines = text.split("\n").filter((l) => l.includes(tag));
let o;
if (lines.length === 1) {
  o = JSON.parse(lines[0].slice(lines[0].indexOf(tag) + tag.length));
} else {
  o = { esito: "fallito", motivo: "parent_killed_or_no_result" };
}
o.container_memory = mem;
o.container_exit = Number(code);
o.image = image;
fs.writeFileSync(outPath, JSON.stringify(o, null, 2) + "\n");
console.log("run_oom.sh: esito " + o.esito + (o.motivo ? " (" + o.motivo + ")" : "") + ", exit del container " + code);
process.exit(o.esito === "fallito" ? 1 : 0);
' "$out" "$MEM" "$code" "$image" < "$raw/test-stdout.txt"
