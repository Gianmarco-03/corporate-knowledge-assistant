#!/usr/bin/env bash
# spikes/tools/session.sh — unico script di orchestrazione Compose degli spike, sull'host
# (Git Bash). Proprietario: piano 01-02 (WIRING W9/W10/W20), chiuso dopo la wave 0.
#
# Contratto completo in 01-02-PLAN.md <interfaces>. In breve:
#   new-id <piano>                          stampa un run_id nuovo
#   tree-id                                 stampa l'id del worktree (nome del volume /target)
#   acquire <piano> <run_id> [--wait <s>]   prenota la finestra di misura (exit 3 se occupata)
#   release <run_id>                        rilascia, solo se il run_id è il proprietario
#   with <piano> [--wait <s>] -- <cmd…>     new-id + acquire + cmd + down + release
#   status | reclaim | vram
#   up <run_id> <svc…>                      avvia i soli servizi nominati, senza profili
#   run <run_id> <nome> -- <cmd…>           job del runner in background, nominato
#   oneshot <run_id> -- <cmd…>              job del runner in primo piano (--rm)
#   run-paid <run_id> <nome> -- <cmd…>      come run, sul servizio runner-paid (chiavi a pagamento)
#   oneshot-paid <run_id> -- <cmd…>         come oneshot, sul servizio runner-paid
#   init-volumes <run_id>                   chown dei volumi del runner (unico passo root,
#                                           servizio runner-init)
#   exec <run_id> <svc|container> -- <cmd…> docker exec su un container della sessione
#   cid <run_id> <svc>                      id dell'unico container del servizio
#   stop <run_id> <container>               ferma e rimuove UN container della sessione
#   down <run_id>                           ferma e rimuove tutti e soli i container della sessione
#   ps <run_id> [--assert-only <svc…>]      container della sessione; con --assert-only
#                                           exit 1 se gira un servizio del progetto fuori elenco
#   docling-models <run_id>                 job dei pesi Docling (volume docling-models-min),
#                                           fino a 12 tentativi; stampa tentativi=<n>
#   docling-models-help <run_id>            modelli ammessi da docling-tools nel job
#
# Regole: il run_id si valida con ^[a-z0-9][a-z0-9-]{0,62}$ (e `none`, valore di default
# dell'etichetta, è riservato) prima di qualsiasi comando docker; nessun filtro di etichetta
# vuoto; mai il down di compose, mai rimozione di volumi; nessun argomento del chiamante
# diventa un'opzione di `compose run`; nessun socket del daemon montato. Nessun valore
# dell'env viene stampato. Compose legge il file segreti con --env-file, e dc() toglie
# dall'ambiente del solo processo compose ogni variabile dell'host che ha il nome,
# maiuscole comprese, di una chiave del file: così l'interpolazione usa sempre il valore
# del file (fase 1.1). Del file si leggono solo i nomi delle chiavi; restano escluse
# CKA_RUN_ID, che imposta dc() stesso, e le variabili interne di session.sh (DC_ENV_KEEP).
# Le chiavi a pagamento stanno solo nel servizio runner-paid (oneshot-paid, run-paid);
# up rifiuta runner-paid e runner-init: niente container a lunga vita con quelle chiavi o
# root con CHOWN (gap-closure 1.6, D-09).
#
# Modifica: Claude (gsd-executor 01.1-01) · 2026-10-21 · dc(): i segreti del file prevalgono sull'host (gap-closure 1.1)
# Modifica: Claude (gsd-executor 01.6-03) · 2026-10-27 · runner senza capability, runner-init, runner-paid (gap-closure 1.6, D-09)
# Modifica: Claude (gsd-executor 01.6-04) · 2026-10-29 · pesi Docling ridotti ai modelli usati, volume docling-models-min (gap-closure 1.6, D-09)
# Modifica: Claude (Anthropic) · 2026-11-05 · volume /target per worktree (/verify 1, decisione utente)

set -euo pipefail

# Git Bash: gli argomenti che iniziano con / sono percorsi del container, non dell'host.
export MSYS_NO_PATHCONV=1
export MSYS2_ARG_CONV_EXCL='*'
unset COMPOSE_PROFILES

readonly PROJECT=cka-spikes
readonly RUN_ID_RE='^[a-z0-9][a-z0-9-]{0,62}$'
readonly CKA_HOME="$HOME/.cka-spikes"
readonly LOCKS="$CKA_HOME/locks"
readonly LOCK="$LOCKS/measure"
readonly GRACE_S="${CKA_LOCK_GRACE_S:-30}"
CKA_SPIKES_ENV="${CKA_SPIKES_ENV:-$CKA_HOME/spikes.env}"
# Variabili che dc() non toglie mai all'ambiente di compose, anche se il file segreti
# ne contiene una omonima: CKA_RUN_ID la imposta dc() stesso (etichette cka.run_id);
# le altre servono al funzionamento di session.sh; COMPOSE_PROFILES è già rifiutata da
# check_env e tolta qui sopra.
readonly DC_ENV_KEEP=(CKA_RUN_ID CKA_TREE_ID CKA_SPIKES_ENV CKA_LOCK_GRACE_S CKA_OWNER_PID
  MSYS_NO_PATHCONV MSYS2_ARG_CONV_EXCL COMPOSE_PROFILES)

err() { printf 'session.sh: %s\n' "$*" >&2; }
die() { local code=$1; shift; err "$*"; exit "$code"; }

# Percorso host in forma accettata da docker.exe (con MSYS_NO_PATHCONV la conversione
# automatica è spenta).
hostpath() {
  if command -v cygpath >/dev/null 2>&1; then cygpath -m "$1"; else printf '%s\n' "$1"; fi
}

valid_run_id() {
  local rid=${1-}
  if [[ -z $rid || ! $rid =~ $RUN_ID_RE || $rid == none ]]; then
    die 2 "run_id non valido: atteso ^[a-z0-9][a-z0-9-]{0,62}$, diverso da 'none'"
  fi
}

valid_name() {
  [[ ${1-} =~ ^[a-zA-Z0-9][a-zA-Z0-9_.-]{0,127}$ ]] || die 2 "nome di container non valido"
}

compose_file() {
  local top
  top=$(git rev-parse --show-toplevel 2>/dev/null) || die 2 "non sono in un repository git"
  printf '%s/spikes/compose.yaml\n' "$top"
}

# Id del worktree: dà il nome al volume /target (un volume di build per worktree, /verify 1).
tree_id() {
  local top
  top=$(git rev-parse --show-toplevel 2>/dev/null) || die 2 "non sono in un repository git"
  printf '%s' "$top" | sha1sum | cut -c1-12
}

check_env() {
  [[ -f $CKA_SPIKES_ENV ]] || die 2 "file segreti assente (CKA_SPIKES_ENV): crearlo come da 01-02 Task 1"
  if grep -qE '^[[:space:]]*(export[[:space:]]+)?COMPOSE_PROFILES[[:space:]]*=' "$CKA_SPIKES_ENV"; then
    die 2 "il file segreti contiene COMPOSE_PROFILES: rimuoverla (i profili non si usano)"
  fi
}

# secret_keys: stampa i soli NOMI delle chiavi del file segreti, uno per riga.
# sed tiene solo il gruppo 2 (il nome) e scarta il valore dopo `=` senza salvarlo né
# stamparlo; accetta il prefisso `export` come check_env; commenti e righe vuote non
# producono nomi; tr toglie il CR dei file scritti su Windows.
secret_keys() {
  sed -nE 's/^[[:space:]]*(export[[:space:]]+)?([A-Za-z_][A-Za-z0-9_]*)[[:space:]]*=.*/\2/p' \
    "$CKA_SPIKES_ENV" | tr -d '\r' | sort -u
}

# dc <run_id> <argomenti compose…>: compose del progetto con CKA_RUN_ID interpolato.
# Nell'interpolazione di compose una variabile dell'ambiente vince su --env-file: per
# questo il processo compose (e solo lui: nessun unset nella shell di session.sh) parte
# senza le variabili dell'host che hanno il nome di una chiave del file. Il confronto è
# in maiuscolo perché docker.exe è un processo Windows, dove i nomi d'ambiente non
# distinguono le maiuscole (Database_Url vale come DATABASE_URL). Diagnostica su stderr,
# solo nomi: lo stdout di dc lo leggono all_services e cmd_up.
dc() {
  local rid=$1; shift
  check_env
  local -A keys=() keep=()
  local -a unset_args=()
  local k n
  for k in "${DC_ENV_KEEP[@]}"; do keep[${k^^}]=1; done
  while IFS= read -r k; do
    [[ -n $k && -z ${keep[${k^^}]-} ]] && keys[${k^^}]=1
  done < <(secret_keys)
  while IFS= read -r n; do
    if [[ -n ${keys[${n^^}]-} && -z ${keep[${n^^}]-} ]]; then
      unset_args+=(-u "$n")
      err "variabile dell'host $n non passata a compose: vale il file segreti"
    fi
  done < <(compgen -e)
  env ${unset_args[@]+"${unset_args[@]}"} CKA_RUN_ID="$rid" CKA_TREE_ID="$(tree_id)" docker compose \
    --env-file "$(hostpath "$CKA_SPIKES_ENV")" -f "$(compose_file)" "$@"
}

# Tutti i servizi dichiarati (tutti i profili attivi solo per leggere la configurazione).
all_services() {
  dc "${1:-none}" --profile '*' config --services
}

# ---------------------------------------------------------------- prenotazione

owner_field() { sed -n "s/^$1=//p" "$LOCK/owner" 2>/dev/null | head -n 1; }

pid_alive() { [[ -n ${1-} ]] && kill -0 "$1" 2>/dev/null; }

dir_age_s() {
  local m now
  m=$(stat -c %Y "$LOCK" 2>/dev/null) || { echo 0; return; }
  now=$(date +%s)
  echo $((now - m))
}

# Stato della prenotazione: free | live | stale-pid | creating | stale-noowner
lock_state() {
  if [[ ! -d $LOCK ]]; then echo free; return; fi
  if [[ -f $LOCK/owner ]]; then
    if pid_alive "$(owner_field pid)"; then echo live; else echo stale-pid; fi
    return
  fi
  if (( $(dir_age_s) > GRACE_S )); then echo stale-noowner; else echo creating; fi
}

describe_lock() {
  local st; st=$(lock_state)
  case $st in
    free) echo "PRENOTAZIONE: nessuna" ;;
    live|stale-pid)
      local tag=vivo; [[ $st == stale-pid ]] && tag=STANTIO
      echo "PRENOTAZIONE: run_id=$(owner_field run_id) piano=$(owner_field plan) pid=$(owner_field pid) ($tag) worktree=$(owner_field worktree) git_rev=$(owner_field git_rev) dal=$(owner_field started_utc)" ;;
    creating) echo "PRENOTAZIONE: IN CREAZIONE (owner non ancora pubblicato, età $(dir_age_s) s)" ;;
    stale-noowner) echo "PRENOTAZIONE: STANTIO (owner assente da $(dir_age_s) s, grazia ${GRACE_S} s)" ;;
  esac
}

# Il run_id deve essere il proprietario della prenotazione attiva.
require_owner() {
  local rid=$1
  [[ -f $LOCK/owner && $(owner_field run_id) == "$rid" ]] \
    || die 2 "il run_id non possiede la prenotazione attiva: nessuna azione ($(describe_lock))"
}

acquire_impl() {
  local plan=$1 rid=$2 pid=$3 wait_s=$4
  mkdir -p "$LOCKS"
  chmod 700 "$CKA_HOME" "$LOCKS" 2>/dev/null || true
  local deadline=$(( $(date +%s) + wait_s ))
  while :; do
    if mkdir "$LOCK" 2>/dev/null; then
      local tmp="$LOCK/.owner.tmp.$$"
      {
        printf 'run_id=%s\n' "$rid"
        printf 'plan=%s\n' "$plan"
        printf 'pid=%s\n' "$pid"
        printf 'worktree=%s\n' "$(git rev-parse --show-toplevel 2>/dev/null || pwd)"
        printf 'git_rev=%s\n' "$(git rev-parse --short HEAD 2>/dev/null || echo unknown)"
        printf 'started_utc=%s\n' "$(date -u +%Y-%m-%dT%H:%M:%SZ)"
      } > "$tmp"
      mv -f "$tmp" "$LOCK/owner"
      echo "PRENOTATO run_id=$rid piano=$plan"
      return 0
    fi
    if (( $(date +%s) < deadline )); then sleep 5; continue; fi
    echo "OCCUPATO — $(describe_lock)"
    return 3
  done
}

release_impl() {
  local rid=$1
  if [[ -f $LOCK/owner && $(owner_field run_id) == "$rid" ]]; then
    rm -f "$LOCK/owner" "$LOCK"/.owner.tmp.* 2>/dev/null || true
    rmdir "$LOCK"
    echo "RILASCIATO run_id=$rid"
    return 0
  fi
  err "rilascio rifiutato: il run_id non è il proprietario ($(describe_lock))"
  return 3
}

# ---------------------------------------------------------------- container

label_of() {
  docker inspect --format '{{index .Config.Labels "cka.run_id"}}' "$1" 2>/dev/null
}

down_impl() {
  local rid=$1 ids id
  ids=$(docker ps -aq --filter "label=cka.run_id=$rid")
  for id in $ids; do
    docker stop "$id" >/dev/null 2>&1 || true
    docker rm "$id" >/dev/null 2>&1 || true
  done
  local left; left=$(docker ps -aq --filter "label=cka.run_id=$rid" | wc -l)
  echo "DOWN run_id=$rid: rimossi $(printf '%s\n' $ids | grep -c . || true), residui $left"
  [[ $left -eq 0 ]]
}

# Servizi del progetto in esecuzione, uno per riga: "<servizio>|<run_id>|<nome>"
# (separatore | come l'IFS dei lettori: ps_impl --assert-only e cmd_up passo (2))
project_running() {
  docker ps --filter "label=com.docker.compose.project=$PROJECT" \
    --format '{{.Label "com.docker.compose.service"}}|{{.Label "cka.run_id"}}|{{.Names}}'
}

ps_impl() {
  local rid=$1; shift
  echo "CONTAINER della sessione $rid (nome, servizio, run_id, stato):"
  docker ps -a --filter "label=cka.run_id=$rid" \
    --format '  {{.Names}}  {{.Label "com.docker.compose.service"}}  {{.Label "cka.run_id"}}  {{.State}}'
  [[ ${1-} == --assert-only ]] || return 0
  shift
  local known allowed=" $* " bad=0 svc r name
  known=" $(all_services "$rid" | tr '\n' ' ') "
  while IFS='|' read -r svc r name; do
    [[ -n ${svc-} ]] || continue
    [[ $known == *" $svc "* ]] || continue   # nessun servizio del compose: non è del set
    if [[ $allowed != *" $svc "* ]]; then
      err "in esecuzione un servizio fuori elenco: $svc ($name, run_id=$r)"
      bad=1
    fi
  done < <(project_running)
  if (( bad )); then return 1; fi
  echo "ASSERT-ONLY ok: in esecuzione solo {$*}"
}

# ---------------------------------------------------------------- sottocomandi

cmd_new_id() {
  local plan=${1-}
  [[ $plan =~ ^[a-z0-9][a-z0-9-]{0,40}$ ]] || die 2 "piano non valido (es. 01-02)"
  local hex rid
  hex=$(od -An -N2 -tx1 /dev/urandom | tr -d ' \n')
  rid="$plan-$(date -u +%Y%m%dT%H%M%SZ | tr 'A-Z' 'a-z')-$hex"
  valid_run_id "$rid"
  printf '%s\n' "$rid"
}

cmd_acquire() {
  local plan=${1-} rid=${2-}
  valid_run_id "$rid"
  [[ $plan =~ ^[a-z0-9][a-z0-9-]{0,40}$ ]] || die 2 "piano non valido"
  shift 2
  local wait_s=0
  if [[ ${1-} == --wait ]]; then
    [[ ${2-} =~ ^[0-9]+$ ]] || die 2 "--wait richiede i secondi"
    wait_s=$2; shift 2
  fi
  (( $# == 0 )) || die 2 "argomenti in più per acquire"
  acquire_impl "$plan" "$rid" "${CKA_OWNER_PID:-$PPID}" "$wait_s"
}

cmd_release() {
  local rid=${1-}
  valid_run_id "$rid"
  release_impl "$rid"
}

cmd_status() {
  describe_lock
}

cmd_reclaim() {
  local st; st=$(lock_state)
  case $st in
    free) echo "PRENOTAZIONE: nessuna, niente da recuperare" ;;
    live|creating) echo "RECLAIM rifiutato — $(describe_lock)"; return 3 ;;
    stale-pid)
      local rid; rid=$(owner_field run_id)
      valid_run_id "$rid"
      echo "RECLAIM di $(describe_lock)"
      down_impl "$rid" || true
      rm -f "$LOCK/owner" "$LOCK"/.owner.tmp.* 2>/dev/null || true
      rmdir "$LOCK"
      echo "RECLAIM completato: prenotazione di $rid rimossa"
      ;;
    stale-noowner)
      echo "RECLAIM di $(describe_lock): solo rimozione della prenotazione, nessun down"
      rm -f "$LOCK"/.owner.tmp.* 2>/dev/null || true
      rmdir "$LOCK"
      ;;
  esac
}

cmd_up() {
  local rid=${1-}
  valid_run_id "$rid"; shift
  require_owner "$rid"
  (( $# > 0 )) || die 2 "up richiede almeno un servizio"
  local s
  for s in "$@"; do [[ $s =~ ^[a-z0-9][a-z0-9-]*$ ]] || die 2 "nome di servizio non valido: $s"; done
  # runner-paid (chiavi a pagamento) e runner-init (root con CHOWN) non si avviano mai a
  # lunga vita: il controllo guarda ogni nome e precede ogni chiamata a docker
  # (gap-closure 1.6, D-09).
  for s in "$@"; do
    case $s in
      runner-paid|runner-init)
        die 2 "up non avvia $s: usare oneshot-paid/run-paid o init-volumes" ;;
    esac
  done

  # (1) set risolto = set nominato. `config --services` ignora i nomi (verificato in
  # 01-02 Task 3, compose v5.6.0): ripiego del piano, `config --format json <svc…>` senza
  # flag di profilo, cioè la stessa risoluzione di `up <svc…>`, e chiavi di .services.
  # `--no-interpolate` (gap-closure 1.6): alla risoluzione i valori non servono, quindi la
  # pipe non li porta; un parsing fallito esce con un messaggio fisso, senza il testo
  # dell'eccezione, che potrebbe riportare un frammento dell'input. Con --no-interpolate
  # compose non filtra per servizio e rende il modello intero (avviso «service filtering is
  # not applied», verificato in 01.6-03): il set risolto lo calcola node, come chiusura dei
  # nomi dichiarati su depends_on, network_mode service: e volumes_from, cioè quello che
  # `up <svc…>` tirerebbe dentro. Un nome assente dal modello resta fuori dal set.
  local want got
  want=$(printf '%s\n' "$@" | sort -u | tr '\n' ' ')
  got=$(dc "$rid" config --no-interpolate --format json \
    | node -e 'let j;try{j=JSON.parse(require("fs").readFileSync(0))}catch(e){console.error("config non JSON");process.exit(1)}const s=j.services||{},seen=new Set(),q=process.argv.slice(1);while(q.length){const n=q.pop();if(seen.has(n)||!Object.hasOwn(s,n))continue;seen.add(n);const v=s[n],d=v.depends_on||{};for(const k of Array.isArray(d)?d:Object.keys(d))q.push(k);const m=String(v.network_mode||"");if(m.startsWith("service:"))q.push(m.slice(8));for(const f of v.volumes_from||[]){const t=String(f);if(!t.startsWith("container:"))q.push(t.split(":")[0])}}for(const k of seen)console.log(k)' "$@" \
    | tr -d '\r' | sort -u | tr '\n' ' ')
  if [[ $want != "$got" ]]; then
    die 1 "set risolto diverso dal nominato: nominato {$want} risolto {$got}"
  fi
  echo "SET RISOLTO = NOMINATO: {$want}"

  # (2) nessun servizio nominato già in esecuzione con un'altra sessione
  local svc r name
  while IFS='|' read -r svc r name; do
    [[ -n ${svc-} ]] || continue
    for s in "$@"; do
      if [[ $svc == "$s" && $r != "$rid" ]]; then
        die 1 "servizio $s già in esecuzione con un'altra sessione ($name, run_id=$r): non lo tocco"
      fi
    done
  done < <(project_running)

  # (3) avvio per nome, senza flag di profilo e senza dipendenze
  dc "$rid" up -d --no-deps --wait --wait-timeout 900 "$@"

  # (4) il set in esecuzione è quello della sessione
  local session_svcs
  session_svcs=$( { printf '%s\n' "$@"; docker ps --filter "label=cka.run_id=$rid" \
      --format '{{.Label "com.docker.compose.service"}}'; } | grep . | sort -u | tr '\n' ' ')
  # shellcheck disable=SC2086
  ps_impl "$rid" --assert-only $session_svcs
}

cmd_run() {
  local rid=${1-}
  valid_run_id "$rid"
  local name=${2-}
  valid_name "$name"
  shift 2
  [[ ${1-} == -- ]] || die 2 "uso: run <run_id> <container_name> -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "run richiede un comando"
  require_owner "$rid"
  if docker inspect "$name" >/dev/null 2>&1; then
    die 1 "nome già occupato: $name (run_id=$(label_of "$name"))"
  fi
  local cmd=("$@")
  dc "$rid" run -d --no-deps --name "$name" --label "cka.run_id=$rid" runner "${cmd[@]}"
}

cmd_oneshot() {
  local rid=${1-}
  valid_run_id "$rid"; shift
  [[ ${1-} == -- ]] || die 2 "uso: oneshot <run_id> -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "oneshot richiede un comando"
  require_owner "$rid"
  local cmd=("$@")
  dc "$rid" run --rm --no-deps -T --label "cka.run_id=$rid" runner "${cmd[@]}"
}

# run-paid e oneshot-paid: copie di run e oneshot sul servizio runner-paid, con gli stessi
# controlli. Le chiavi a pagamento arrivano solo dall'interpolazione di compose (file
# segreti), mai da argv (gap-closure 1.6, D-09).
cmd_run_paid() {
  local rid=${1-}
  valid_run_id "$rid"
  local name=${2-}
  valid_name "$name"
  shift 2
  [[ ${1-} == -- ]] || die 2 "uso: run-paid <run_id> <container_name> -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "run-paid richiede un comando"
  require_owner "$rid"
  if docker inspect "$name" >/dev/null 2>&1; then
    die 1 "nome già occupato: $name (run_id=$(label_of "$name"))"
  fi
  local cmd=("$@")
  dc "$rid" run -d --no-deps --name "$name" --label "cka.run_id=$rid" runner-paid "${cmd[@]}"
}

cmd_oneshot_paid() {
  local rid=${1-}
  valid_run_id "$rid"; shift
  [[ ${1-} == -- ]] || die 2 "uso: oneshot-paid <run_id> -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "oneshot-paid richiede un comando"
  require_owner "$rid"
  local cmd=("$@")
  dc "$rid" run --rm --no-deps -T --label "cka.run_id=$rid" runner-paid "${cmd[@]}"
}

# Unico passo root: servizio runner-init (root con le sole CHOWN, DAC_OVERRIDE e FOWNER,
# senza rete né ambiente); il runner non ha più capability (gap-closure 1.6, D-09).
cmd_init_volumes() {
  (( $# == 1 )) || die 2 "uso: init-volumes <run_id> (firma fissa, nessun altro argomento)"
  local rid=$1
  valid_run_id "$rid"
  require_owner "$rid"
  dc "$rid" run --rm --no-deps -T --label "cka.run_id=$rid" runner-init \
    chown -R 10001:10001 /target /usr/local/cargo/registry /work/tmp
  echo "INIT-VOLUMES ok: /target /usr/local/cargo/registry /work/tmp di 10001:10001"
}

# Job dei pesi Docling (deviazione (d) del compose, gap-closure 1.6, D-09): firma fissa, il
# comando di download e i modelli stanno solo nel compose, nessun argomento del chiamante
# arriva a compose. Gli errori DNS transitori di Docker Desktop fanno fallire il job (01-02
# è riuscito all'undicesimo tentativo); un nuovo tentativo riprende dai file già scaricati.
# Tetto di 12 tentativi a 15 s l'uno dall'altro; l'uscita è quella dell'ultimo tentativo.
readonly DOCLING_MODELS_TRIES=12
readonly DOCLING_MODELS_PAUSE_S=15

cmd_docling_models() {
  (( $# == 1 )) || die 2 "uso: docling-models <run_id> (firma fissa, nessun altro argomento)"
  local rid=$1
  valid_run_id "$rid"
  require_owner "$rid"
  local n=0 code=1
  while (( n < DOCLING_MODELS_TRIES )); do
    n=$((n + 1))
    if dc "$rid" --profile docling-models run --rm --no-deps -T         --label "cka.run_id=$rid" docling-models; then
      code=0
    else
      code=$?
    fi
    (( code == 0 )) && break
    err "docling-models: tentativo $n di $DOCLING_MODELS_TRIES fallito (exit $code)"
    (( n < DOCLING_MODELS_TRIES )) && sleep "$DOCLING_MODELS_PAUSE_S"
  done
  echo "DOCLING-MODELS exit=$code tentativi=$n run_id=$rid"
  return "$code"
}

# Elenco dei modelli che `docling-tools models download` accetta nell'immagine del job:
# stesso servizio, comando --help al posto del download, nessun argomento del chiamante.
cmd_docling_models_help() {
  (( $# == 1 )) || die 2 "uso: docling-models-help <run_id> (firma fissa, nessun altro argomento)"
  local rid=$1
  valid_run_id "$rid"
  require_owner "$rid"
  dc "$rid" --profile docling-models run --rm --no-deps -T     --label "cka.run_id=$rid" docling-models docling-tools models download --help
}

cmd_cid() {
  local rid=${1-} svc=${2-}
  valid_run_id "$rid"
  [[ $svc =~ ^[a-z0-9][a-z0-9-]*$ ]] || die 2 "servizio non valido"
  local ids n
  ids=$(docker ps -q --filter "label=cka.run_id=$rid" --filter "label=com.docker.compose.service=$svc")
  n=$(printf '%s\n' $ids | grep -c . || true)
  if [[ $n -ne 1 ]]; then
    err "cid: trovati $n container del servizio $svc per la sessione $rid (atteso 1)"
    return 1
  fi
  printf '%s\n' "$ids"
}

cmd_exec() {
  local rid=${1-}
  valid_run_id "$rid"
  local target=${2-}
  [[ -n $target ]] || die 2 "uso: exec <run_id> <servizio|container> -- <comando…>"
  shift 2
  [[ ${1-} == -- ]] || die 2 "uso: exec <run_id> <servizio|container> -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "exec richiede un comando"
  require_owner "$rid"
  local known
  known=" $(all_services "$rid" | tr '\n' ' ') "
  if [[ $known == *" $target "* ]]; then
    target=$(cmd_cid "$rid" "$target") || die 1 "servizio non risolto per questa sessione"
  else
    valid_name "$target"
  fi
  local lbl; lbl=$(label_of "$target") || die 1 "container inesistente: $target"
  [[ $lbl == "$rid" ]] || die 1 "exec rifiutato: $target non appartiene alla sessione (run_id=${lbl:-assente})"
  local cmd=("$@")
  docker exec "$target" "${cmd[@]}"
}

cmd_stop() {
  local rid=${1-} target=${2-}
  valid_run_id "$rid"
  valid_name "$target"
  require_owner "$rid"
  local lbl; lbl=$(label_of "$target") || die 1 "container inesistente: $target"
  [[ $lbl == "$rid" ]] || die 1 "stop rifiutato: $target non appartiene alla sessione (run_id=${lbl:-assente})"
  docker stop "$target" >/dev/null 2>&1 || true
  docker rm "$target" >/dev/null 2>&1 || true
  if docker inspect "$target" >/dev/null 2>&1; then die 1 "stop: $target ancora presente"; fi
  echo "STOP $target ok"
}

cmd_down() {
  local rid=${1-}
  valid_run_id "$rid"
  require_owner "$rid"
  down_impl "$rid"
}

cmd_ps() {
  local rid=${1-}
  valid_run_id "$rid"; shift
  if [[ ${1-} == --assert-only ]]; then
    shift
    (( $# > 0 )) || die 2 "--assert-only richiede almeno un servizio"
    ps_impl "$rid" --assert-only "$@"
  else
    (( $# == 0 )) || die 2 "uso: ps <run_id> [--assert-only <servizio…>]"
    ps_impl "$rid"
  fi
}

cmd_vram() {
  if ! command -v nvidia-smi >/dev/null 2>&1; then echo "VRAM: nvidia-smi non disponibile"; return 0; fi
  echo "VRAM (GPU):"
  nvidia-smi --query-gpu=memory.used,memory.total --format=csv
  echo "VRAM (processi):"
  nvidia-smi --query-compute-apps=pid,process_name,used_memory --format=csv
}

cmd_with() {
  local plan=${1-}
  [[ $plan =~ ^[a-z0-9][a-z0-9-]{0,40}$ ]] || die 2 "uso: with <piano> [--wait <s>] -- <comando…>"
  shift
  local wait_s=0
  if [[ ${1-} == --wait ]]; then
    [[ ${2-} =~ ^[0-9]+$ ]] || die 2 "--wait richiede i secondi"
    wait_s=$2; shift 2
  fi
  [[ ${1-} == -- ]] || die 2 "uso: with <piano> [--wait <s>] -- <comando…>"
  shift
  (( $# > 0 )) || die 2 "with richiede un comando"
  check_env
  WITH_RID=$(cmd_new_id "$plan")
  local rid=$WITH_RID
  acquire_impl "$plan" "$rid" "$$" "$wait_s" || exit 3
  export CKA_RUN_ID=$rid
  cleanup() {
    local code=$?
    trap - EXIT INT TERM
    down_impl "$WITH_RID" || true
    release_impl "$WITH_RID" || true
    exit "$code"
  }
  trap cleanup EXIT
  trap 'exit 130' INT
  trap 'exit 143' TERM
  echo "SESSIONE run_id=$rid piano=$plan"
  ps_impl "$rid"
  cmd_vram || true
  local code=0
  "$@" || code=$?
  echo "SESSIONE run_id=$rid: comando terminato con exit $code"
  exit "$code"
}

usage() {
  sed -n '2,30p' "$0" >&2
  exit 2
}

main() {
  local sub=${1-}
  [[ -n $sub ]] || usage
  shift
  case $sub in
    new-id) cmd_new_id "$@" ;;
    tree-id) tree_id; echo ;;
    acquire) cmd_acquire "$@" ;;
    release) cmd_release "$@" ;;
    with) cmd_with "$@" ;;
    status) cmd_status ;;
    reclaim) cmd_reclaim ;;
    up) cmd_up "$@" ;;
    run) cmd_run "$@" ;;
    oneshot) cmd_oneshot "$@" ;;
    run-paid) cmd_run_paid "$@" ;;
    oneshot-paid) cmd_oneshot_paid "$@" ;;
    init-volumes) cmd_init_volumes "$@" ;;
    exec) cmd_exec "$@" ;;
    cid) cmd_cid "$@" ;;
    stop) cmd_stop "$@" ;;
    down) cmd_down "$@" ;;
    ps) cmd_ps "$@" ;;
    vram) cmd_vram ;;
    docling-models) cmd_docling_models "$@" ;;
    docling-models-help) cmd_docling_models_help "$@" ;;
    -h|--help|help) usage ;;
    *) die 2 "sottocomando sconosciuto: $sub" ;;
  esac
}

main "$@"
