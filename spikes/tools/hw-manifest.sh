#!/usr/bin/env bash
# spikes/tools/hw-manifest.sh <out.json> [--run-id <run_id>] [--image <ref>]… [--tool nome=versione]…
#
# Manifest hardware di una sessione di misura (RESEARCH §8.2; WIRING W9). Proprietario:
# piano 01-02, chiuso. Un CSV senza hw.json gemello è una misura non valida.
#
# Campi: timestamp_utc, git_rev, cpu, ram_total, gpu, cuda_driver, wsl_version,
# docker_version, docker_desktop_resources, images, rustc, tools, power_plan, run_id,
# compose_project, containers. Un campo non rilevabile vale null, mai un errore bloccante.
# Non legge il file segreti né stampa variabili d'ambiente.

set -euo pipefail
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'

usage() {
  echo "uso: hw-manifest.sh <out.json> [--run-id <run_id>] [--image <ref>]… [--tool nome=versione]…" >&2
  exit 2
}

out=${1-}
[[ -n $out ]] || usage
shift

rid=${CKA_RUN_ID-}
images=()
tools=()
while (( $# > 0 )); do
  case $1 in
    --run-id) rid=${2-}; shift 2 || usage ;;
    --image) images+=("${2-}"); shift 2 || usage ;;
    --tool) tools+=("${2-}"); shift 2 || usage ;;
    *) usage ;;
  esac
done

# run_id valido o null: mai un filtro di etichetta vuoto
if [[ -n $rid && ( ! $rid =~ ^[a-z0-9][a-z0-9-]{0,62}$ || $rid == none ) ]]; then
  echo "hw-manifest.sh: run_id non valido, registrato come null" >&2
  rid=""
fi

# Esegue un comando e ne restituisce l'output ripulito; vuoto se fallisce o manca.
try() { "$@" 2>/dev/null | tr -d '\r' || true; }
have() { command -v "$1" >/dev/null 2>&1; }

winpath() {
  if have cygpath; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

HWM_TIMESTAMP=$(date -u +%Y-%m-%dT%H:%M:%SZ)
HWM_GIT_REV=$(try git rev-parse HEAD)

HWM_CPU_NAME=""; HWM_CPU_CORES=""; HWM_CPU_THREADS=""; HWM_RAM=""; HWM_POWER=""
if have powershell.exe; then
  HWM_CPU_NAME=$(try powershell.exe -NoProfile -Command "(Get-CimInstance Win32_Processor | Select-Object -First 1).Name")
  HWM_CPU_CORES=$(try powershell.exe -NoProfile -Command "(Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfCores -Sum).Sum")
  HWM_CPU_THREADS=$(try powershell.exe -NoProfile -Command "(Get-CimInstance Win32_Processor | Measure-Object -Property NumberOfLogicalProcessors -Sum).Sum")
  HWM_RAM=$(try powershell.exe -NoProfile -Command "(Get-CimInstance Win32_ComputerSystem).TotalPhysicalMemory")
fi
if have powercfg.exe; then HWM_POWER=$(try powercfg.exe /getactivescheme); fi

HWM_GPU=""; HWM_CUDA=""
if have nvidia-smi; then
  HWM_GPU=$(try nvidia-smi --query-gpu=name,driver_version,memory.total --format=csv,noheader)
  HWM_CUDA=$(try nvidia-smi | sed -n 's/.*CUDA[A-Z ]* Version: *\([0-9.]*\).*/\1/p' | head -n 1)
fi

HWM_WSL=""
if have wsl.exe; then HWM_WSL=$(WSL_UTF8=1 try wsl.exe --version); fi

HWM_DOCKER=""; HWM_DOCKER_RES=""
if have docker; then
  HWM_DOCKER=$(try docker version --format '{{.Server.Version}}')
  HWM_DOCKER_RES=$(try docker info --format '{{.NCPU}} {{.MemTotal}}')
fi

HWM_RUSTC=""
if have rustc; then HWM_RUSTC=$(try rustc -V); fi

# immagini: "ref<TAB>digest" per riga
HWM_IMAGES=""
for ref in "${images[@]+"${images[@]}"}"; do
  [[ -n $ref ]] || continue
  dg=""
  if have docker; then dg=$(try docker image inspect --format '{{index .RepoDigests 0}}' "$ref"); fi
  HWM_IMAGES+="${ref}"$'\t'"${dg}"$'\n'
done

HWM_TOOLS=""
for t in "${tools[@]+"${tools[@]}"}"; do
  [[ $t == *=* ]] || continue
  HWM_TOOLS+="$t"$'\n'
done

# container della sessione: "nome<TAB>servizio<TAB>immagine<TAB>image_id" per riga
HWM_CONTAINERS=""
if [[ -n $rid ]] && have docker; then
  for id in $(try docker ps -aq --filter "label=cka.run_id=$rid"); do
    line=$(try docker inspect --format '{{.Name}}{{"\t"}}{{index .Config.Labels "com.docker.compose.service"}}{{"\t"}}{{.Config.Image}}{{"\t"}}{{.Image}}' "$id")
    [[ -n $line ]] && HWM_CONTAINERS+="${line#/}"$'\n'
  done
fi

export HWM_TIMESTAMP HWM_GIT_REV HWM_CPU_NAME HWM_CPU_CORES HWM_CPU_THREADS HWM_RAM HWM_POWER \
  HWM_GPU HWM_CUDA HWM_WSL HWM_DOCKER HWM_DOCKER_RES HWM_RUSTC HWM_IMAGES HWM_TOOLS HWM_CONTAINERS
export HWM_RUN_ID=$rid

mkdir -p "$(dirname "$out")"
outw=$(winpath "$out")

node - "$outw" <<'NODE'
const fs = require('fs');
const e = process.env;
const s = (k) => { const v = (e[k] || '').trim(); return v === '' ? null : v; };
const n = (k) => { const v = s(k); return v !== null && /^\d+$/.test(v) ? Number(v) : null; };
const lines = (k) => (e[k] || '').split('\n').map((l) => l.trim()).filter(Boolean);
let res = null;
const r = s('HWM_DOCKER_RES');
if (r) { const [c, m] = r.split(/\s+/); res = { ncpu: Number(c) || null, mem_total_bytes: Number(m) || null }; }
const tools = {};
for (const l of lines('HWM_TOOLS')) { const i = l.indexOf('='); tools[l.slice(0, i)] = l.slice(i + 1); }
const m = {
  timestamp_utc: s('HWM_TIMESTAMP'),
  git_rev: s('HWM_GIT_REV'),
  cpu: s('HWM_CPU_NAME') === null ? null :
    { model: s('HWM_CPU_NAME'), cores: n('HWM_CPU_CORES'), threads: n('HWM_CPU_THREADS') },
  ram_total: n('HWM_RAM'),
  gpu: s('HWM_GPU'),
  cuda_driver: s('HWM_CUDA'),
  wsl_version: s('HWM_WSL'),
  docker_version: s('HWM_DOCKER'),
  docker_desktop_resources: res,
  images: lines('HWM_IMAGES').map((l) => { const [ref, digest] = l.split('\t'); return { ref, digest: digest || null }; }),
  rustc: s('HWM_RUSTC'),
  tools,
  power_plan: s('HWM_POWER'),
  run_id: s('HWM_RUN_ID'),
  compose_project: 'cka-spikes',
  containers: lines('HWM_CONTAINERS').map((l) => {
    const [name, service, image, image_id] = l.split('\t');
    return { name, service: service || null, image: image || null, image_id: image_id || null };
  }),
};
fs.writeFileSync(process.argv[2], JSON.stringify(m, null, 2) + '\n');
NODE

node -e "JSON.parse(require('fs').readFileSync(process.argv[1]))" "$outw"
echo "hw-manifest: $out"
