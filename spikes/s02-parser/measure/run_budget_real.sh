#!/usr/bin/env bash
# spikes/s02-parser/measure/run_budget_real.sh <classe> <N> <run-tag>
#
# Ciclo di misura di un batch di job `parse` reali (piano 01-18, Task 2; soglia M10). Unico
# proprietario di riavvio del worker, letture del cgroup, `parse-parent batch` e controllo
# del padre. Gira dentro `bash spikes/tools/session.sh with 01-18 -- …` dalla radice del
# worktree, dopo `budget_real.py jobs`, e usa solo CKA_RUN_ID; non stampa valori dell'env.
#
# Passi, scritti in spikes/s02-parser/results/raw/isolation-budget-real/<classe>-c<N>/
# (ignorata da git; nel worker /results/raw/isolation-budget-real/...):
#   1. riavvio del worker (stop del container via `session.sh cid`, poi `up` e
#      `ps --assert-only worker`), così memory.peak riparte da un cgroup nuovo;
#   2. memory.peak, memory.max e memory.events del worker in cg-before.txt;
#   3. `parse-parent batch --jobs … --concurrency <N>`: stdout in reports.jsonl, stderr in
#      stderr.txt, exit code in exit_code (un exit non zero non interrompe i passi 4-5);
#   4. stesse letture del cgroup in cg-after.txt;
#   5. exit di `session.sh ps --assert-only worker` in parent_alive.
# Classe, N e run-tag si validano prima di qualsiasi comando: altrimenti exit 2.
#
# **Modifica:** Claude (Anthropic) · 2026-10-22 · ciclo di misura dei batch reali (01-18)

set -euo pipefail
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

cls=${1-}
n=${2-}
tag=${3-}
if [[ ! $cls =~ ^(benign|hostile)$ || ! $n =~ ^(1|4|8|16)$ || ! $tag =~ ^[a-z0-9]{8}$ ]]; then
  echo "uso: run_budget_real.sh <benign|hostile> <1|4|8|16> <run-tag [a-z0-9]{8}>" >&2
  exit 2
fi
if [[ -z ${CKA_RUN_ID-} ]]; then
  echo "run_budget_real.sh: CKA_RUN_ID assente (lanciare dentro session.sh with 01-18)" >&2
  exit 2
fi

S=spikes/tools/session.sh
rel=isolation-budget-real/$cls-c$n
dir=spikes/s02-parser/results/raw/$rel
if [[ ! -s $dir/jobs.jsonl ]]; then
  echo "run_budget_real.sh: $dir/jobs.jsonl assente (prima budget_real.py jobs)" >&2
  exit 2
fi
rm -f "$dir/cg-before.txt" "$dir/cg-after.txt" "$dir/reports.jsonl" "$dir/stderr.txt" \
  "$dir/exit_code" "$dir/parent_alive"

cgroup() {
  bash $S exec "$CKA_RUN_ID" worker -- \
    cat /sys/fs/cgroup/memory.peak /sys/fs/cgroup/memory.max /sys/fs/cgroup/memory.events
}

# (1) worker nuovo: memory.peak non si azzera scrivendolo come uid 10001.
bash $S stop "$CKA_RUN_ID" "$(bash $S cid "$CKA_RUN_ID" worker)"
bash $S up "$CKA_RUN_ID" worker >/dev/null
bash $S ps "$CKA_RUN_ID" --assert-only worker >/dev/null

# (2) cgroup prima del batch.
cgroup > "$dir/cg-before.txt"

# (3) batch: l'exit si registra, non interrompe.
code=0
bash $S exec "$CKA_RUN_ID" worker -- parse-parent batch \
  --jobs "/results/raw/$rel/jobs.jsonl" --concurrency "$n" \
  > "$dir/reports.jsonl" 2> "$dir/stderr.txt" || code=$?
printf '%s\n' "$code" > "$dir/exit_code"

# (4) cgroup dopo il batch.
cgroup > "$dir/cg-after.txt"

# (5) padre e worker: exit di ps --assert-only.
alive=0
bash $S ps "$CKA_RUN_ID" --assert-only worker >/dev/null 2>&1 || alive=$?
printf '%s\n' "$alive" > "$dir/parent_alive"

echo "run_budget_real.sh: $cls c$n exit_batch=$code ps=$alive"
