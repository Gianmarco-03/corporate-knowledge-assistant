#!/bin/sh
#
# scripts/backup.sh — la copia verificabile della memoria di progetto su un
# SECONDO supporto.
#
# ---------------------------------------------------------------------------
# LA DIVISIONE DEI RUOLI
#
# Il remoto condiviso su GitHub e' la copia PRIMARIA fuori macchina, e si
# aggiorna a ogni insieme di modifiche funzionanti. Questo comando non lo
# sostituisce: aggiunge la cintura di sicurezza, cioe' un pacchetto della
# storia completa (`git bundle --all`) piu' una copia della directory di
# pianificazione (.planning/) su un supporto DIVERSO da quello che ospita il
# repository.
#
# ---------------------------------------------------------------------------
# A COMANDO, NON PIANIFICATO
#
# Il backup si lancia a comando, con prassi dichiarata a fine fase. Questo
# script non crea, non registra e non genera alcun compito pianificato,
# servizio o timer di sistema.
#
# ---------------------------------------------------------------------------
# LA VERIFICA E' L'ESITO
#
# Il pacchetto viene verificato con la verifica di integrita' del proprio
# formato, e il codice di uscita ne dipende: un pacchetto prodotto ma non
# verificabile e' un FALLIMENTO, non un successo. Un backup che mente sul
# proprio esito e' peggio dell'assenza di backup, perche' sostituisce una
# preoccupazione con una falsa sicurezza — e la falsa sicurezza non produce
# alcun segnale finche' non serve davvero.
#
# ---------------------------------------------------------------------------
# COSA NON PROTEGGE
#
# Il database applicativo (PostgreSQL + pgvector nel container Docker), i
# documenti caricati e i volumi Docker NON sono coperti. Qui viaggia la
# memoria di PROGETTO — storia del repository e documenti di pianificazione —
# mai le credenziali della postazione: i file .env, le configurazioni locali
# e le chiavi sono esclusi dalla copia.
#
# ---------------------------------------------------------------------------
# USO
#
#   sh scripts/backup.sh <directory-di-destinazione>
#   CKA_BACKUP_DEST=<directory> sh scripts/backup.sh
#
# Codici di uscita: 0 riuscito · 2 destinazione inutilizzabile · 3 pacchetto
# non prodotto · 4 pacchetto non verificabile · 5 copia della pianificazione
# non riuscita.

set -eu

# Il nome della variabile d'ambiente vive in UN SOLO posto: qui.
VARIABILE_DESTINAZIONE="CKA_BACKUP_DEST"

# Il nome del pacchetto e' fisso, non derivato dalla cartella del clone
# (che puo' chiamarsi in qualunque modo, per esempio con prefisso numerico).
NOME_PROGETTO="corporate-knowledge-assistant"

DIRECTORY_PIANIFICAZIONE=".planning"

USCITA_DESTINAZIONE=2
USCITA_PACCHETTO=3
USCITA_VERIFICA=4
USCITA_PIANIFICAZIONE=5

errore() {
    printf 'backup: %s\n' "$1" >&2
}

uso() {
    printf '\nUso:\n' >&2
    printf '  sh %s <directory-di-destinazione>\n' "scripts/backup.sh" >&2
    printf '  %s=<directory> sh %s\n\n' \
        "$VARIABILE_DESTINAZIONE" "scripts/backup.sh" >&2
    printf 'La destinazione deve stare su un supporto DIVERSO da quello del\n' >&2
    printf 'repository: una copia che vive dentro cio che protegge non\n' >&2
    printf 'protegge da nulla.\n' >&2
}

# I nomi che non lasciano il repository. Ricalcano i segreti esclusi da
# .gitignore: il backup porta con se' la memoria di progetto, non le
# credenziali della postazione.
e_escluso() {
    case "$1" in
        .env|.env.*)    return 0 ;;
        *.local.json)   return 0 ;;
        secrets.json)   return 0 ;;
        *.pfx|*.p12|*.key|*.pem) return 0 ;;
    esac
    return 1
}

# --- 1. La radice del repository -------------------------------------------

if ! RADICE_GREZZA=$(git rev-parse --show-toplevel 2>/dev/null); then
    errore "questa directory non appartiene a un repository git."
    exit "$USCITA_DESTINAZIONE"
fi
# Canonicalizzata passando dalla shell: `git rev-parse` restituisce la forma
# nativa della piattaforma (C:/...), `pwd -P` quella della shell (/c/...).
# Confrontarle senza normalizzare farebbe fallire il controllo di
# contenimento proprio dove serve.
RADICE=$(cd "$RADICE_GREZZA" && pwd -P)

# --- 2. La destinazione, e i quattro modi in cui non e' accettabile ---------

# Letta con `printenv` e non con `eval`: il nome resta in un posto solo (la
# riga di VARIABILE_DESTINAZIONE), ma non si costruisce piu' una stringa da
# far interpretare alla shell dentro uno script che maneggia percorsi di
# destinazione. `|| true` perche' `printenv` esce 1 quando la variabile non
# c'e', che qui e' il caso normale.
DA_AMBIENTE=$(printenv "$VARIABILE_DESTINAZIONE" || true)
DESTINAZIONE="${1:-$DA_AMBIENTE}"

if [ -z "$DESTINAZIONE" ]; then
    errore "nessuna destinazione indicata."
    uso
    exit "$USCITA_DESTINAZIONE"
fi

if [ ! -e "$DESTINAZIONE" ]; then
    errore "la destinazione «$DESTINAZIONE» non esiste."
    errore "Un supporto non collegato non e' una destinazione: collegarlo, o"
    errore "creare la directory, prima di rieseguire."
    exit "$USCITA_DESTINAZIONE"
fi

if [ ! -d "$DESTINAZIONE" ]; then
    errore "la destinazione «$DESTINAZIONE» non e' una directory."
    exit "$USCITA_DESTINAZIONE"
fi

# Il `cd` puo' fallire da solo quando i permessi negano l'attraversamento, e
# sotto `set -eu` un fallimento non intercettato uscirebbe con il codice
# della shell e il SUO messaggio: l'utente leggerebbe «Permission denied» su
# una riga di questo script invece dell'esito esplicito che il comando deve
# dare. Un esito opaco su un backup e' esattamente cio che il requisito
# vieta.
if ! DESTINAZIONE_ASSOLUTA=$(cd "$DESTINAZIONE" 2>/dev/null && pwd -P); then
    errore "la destinazione «$DESTINAZIONE» non e' accessibile: i permessi"
    errore "non ne consentono nemmeno l'attraversamento."
    exit "$USCITA_DESTINAZIONE"
fi

if [ "$DESTINAZIONE_ASSOLUTA" = "$RADICE" ]; then
    errore "la destinazione «$DESTINAZIONE» E' il repository stesso."
    errore "Un backup che vive dentro cio che protegge non protegge da nulla."
    exit "$USCITA_DESTINAZIONE"
fi

case "$DESTINAZIONE_ASSOLUTA/" in
    "$RADICE"/*)
        errore "la destinazione «$DESTINAZIONE» e' contenuta nel repository."
        errore "Un secondo supporto che sta sul primo non e' un secondo"
        errore "supporto: la perdita del disco porta via entrambe le copie."
        exit "$USCITA_DESTINAZIONE"
        ;;
esac

# Scrivibilita' provata SCRIVENDO. Su NTFS il bit di permesso non e'
# significativo, quindi `[ -w ]` risponderebbe di si' anche dove una
# scrittura fallisce: il comando scoprirebbe il problema a meta' lavoro,
# lasciando sulla destinazione un pacchetto incompleto.
SONDA="$DESTINAZIONE_ASSOLUTA/.cka-backup-sonda.$$"
if ! (: > "$SONDA") 2>/dev/null; then
    errore "la destinazione «$DESTINAZIONE» non e' scrivibile."
    exit "$USCITA_DESTINAZIONE"
fi
rm -f "$SONDA"

# --- 3. I nomi: esecuzioni successive COESISTONO ----------------------------

NOME_REPOSITORY="$NOME_PROGETTO"
MARCATORE=$(date +%Y%m%d-%H%M%S)
SUFFISSO=""
PROGRESSIVO=1
while [ -e "$DESTINAZIONE_ASSOLUTA/$NOME_REPOSITORY-$MARCATORE$SUFFISSO.bundle" ] \
   || [ -e "$DESTINAZIONE_ASSOLUTA/pianificazione-$MARCATORE$SUFFISSO" ]; do
    # Il marcatore ha la risoluzione del secondo: due esecuzioni ravvicinate
    # sceglierebbero lo stesso nome, e la seconda distruggerebbe la prima.
    # Il progressivo rende la coesistenza una proprieta' del comando invece
    # che una conseguenza fortunata del tempo trascorso.
    PROGRESSIVO=$((PROGRESSIVO + 1))
    SUFFISSO="-$PROGRESSIVO"
done
PACCHETTO="$DESTINAZIONE_ASSOLUTA/$NOME_REPOSITORY-$MARCATORE$SUFFISSO.bundle"
COPIA="$DESTINAZIONE_ASSOLUTA/pianificazione-$MARCATORE$SUFFISSO"

# --- 4. Il pacchetto della storia completa ----------------------------------

printf 'Backup di %s\n' "$RADICE"
printf '  destinazione : %s\n' "$DESTINAZIONE_ASSOLUTA"
printf '  pacchetto della storia completa (tutti i riferimenti)…\n'

if ! git -C "$RADICE" bundle create "$PACCHETTO" --all >/dev/null; then
    errore "il pacchetto non e' stato prodotto."
    exit "$USCITA_PACCHETTO"
fi

# --- 5. La verifica, che E' l'esito del comando ----------------------------

printf '  verifica di integrita del pacchetto…\n'
if ! git -C "$RADICE" bundle verify "$PACCHETTO" >/dev/null 2>&1; then
    errore "il pacchetto «$PACCHETTO» e' stato prodotto ma NON supera la"
    errore "verifica di integrita: e' un FALLIMENTO, non un successo."
    errore "Il file e' lasciato sulla destinazione perche' possa essere"
    errore "esaminato; non consideralo una copia valida."
    exit "$USCITA_VERIFICA"
fi

# --- 6. La copia della directory di pianificazione --------------------------

ORIGINE="$RADICE/$DIRECTORY_PIANIFICAZIONE"
if [ ! -d "$ORIGINE" ]; then
    errore "«$DIRECTORY_PIANIFICAZIONE» non esiste nel repository: la copia"
    errore "della memoria di progetto non e' stata prodotta (la crea il"
    errore "bootstrap GSD: /gsd:ingest-docs)."
    exit "$USCITA_PIANIFICAZIONE"
fi

printf '  copia di %s (esclusi segreti e configurazioni locali)…\n' \
    "$DIRECTORY_PIANIFICAZIONE"
mkdir -p "$COPIA"

# I due elenchi temporanei si rimuovono da un trap: senza, restavano su disco
# a ogni uscita anticipata dello script.
ELENCO_ESCLUSI=$(mktemp)
ELENCO_FILE=$(mktemp)
trap 'rm -f "${ELENCO_ESCLUSI:-}" "${ELENCO_FILE:-}"' EXIT

# Il ciclo NON e' piu' il lato destro di una pipe. Li' girava in una subshell:
# sotto `set -eu` una `cp` fallita — destinazione piena, permesso negato,
# percorso troppo lungo sul filesystem di destinazione — abortiva la subshell,
# la pipeline tornava non-zero e lo script usciva con 1 e senza alcun
# messaggio. Mai con USCITA_PIANIFICAZIONE, e senza mai raggiungere la
# diagnostica qui sotto, che scatta solo quando i file copiati sono ZERO e non
# quando la copia si e' fermata a meta'. Una copia parziale violava cosi' il
# contratto dei codici d'uscita dichiarato in testa al file, ed e' lo stesso
# ragionamento gia' applicato al `cd` della destinazione.
(cd "$ORIGINE" && find . -type f -print) > "$ELENCO_FILE"

while IFS= read -r RELATIVO; do
    NOME=${RELATIVO##*/}
    if e_escluso "$NOME"; then
        printf '%s\n' "$RELATIVO" >> "$ELENCO_ESCLUSI"
        continue
    fi
    mkdir -p "$COPIA/$(dirname "$RELATIVO")"
    if ! cp "$ORIGINE/$RELATIVO" "$COPIA/$RELATIVO"; then
        errore "copia di «$RELATIVO» non riuscita: la copia della"
        errore "pianificazione e' PARZIALE e non va considerata valida."
        exit "$USCITA_PIANIFICAZIONE"
    fi
done < "$ELENCO_FILE"

NUMERO_COPIATI=$(find "$COPIA" -type f | wc -l | tr -d ' ')
NUMERO_ESCLUSI=$(wc -l < "$ELENCO_ESCLUSI" | tr -d ' ')

if [ "$NUMERO_COPIATI" -eq 0 ]; then
    errore "la copia della pianificazione e' vuota: nessun file e' stato"
    errore "prodotto in «$COPIA»."
    exit "$USCITA_PIANIFICAZIONE"
fi

# --- 7. Il riepilogo --------------------------------------------------------

DIMENSIONE=$(wc -c < "$PACCHETTO" | tr -d ' ')

printf '\nBackup RIUSCITO.\n'
printf '  pacchetto della storia    : %s\n' "$PACCHETTO"
printf '  dimensione                : %s byte\n' "$DIMENSIONE"
printf '  verifica di integrita     : SUPERATA\n'
printf '  copia della pianificazione: %s\n' "$COPIA"
printf '  file copiati              : %s\n' "$NUMERO_COPIATI"
printf '  file esclusi (segreti e configurazioni locali): %s\n' \
    "$NUMERO_ESCLUSI"
if [ "$NUMERO_ESCLUSI" -gt 0 ]; then
    while IFS= read -r ESCLUSO; do
        printf '      %s\n' "$ESCLUSO"
    done < "$ELENCO_ESCLUSI"
fi

printf '\nIl remoto condiviso resta la copia primaria fuori macchina: questo\n'
printf 'pacchetto e la cintura di sicurezza aggiuntiva. Il database\n'
printf 'applicativo (Postgres/pgvector in Docker) e i documenti caricati\n'
printf 'NON sono coperti.\n'

exit 0

# **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
