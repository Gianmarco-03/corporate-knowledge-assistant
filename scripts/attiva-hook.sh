#!/bin/sh
#
# scripts/attiva-hook.sh — rende una postazione conforme: attiva gli hook
# versionati di .githooks/.
#
# La directory interna di git (.git/hooks) NON e' versionata: un hook scritto
# li' esisterebbe su una sola postazione, e nessuno se ne accorgerebbe,
# perche' l'assenza di un hook e' silenziosa per definizione. Gli hook vivono
# quindi in .githooks/ (versionati) e si attivano puntandoci `core.hooksPath`.
# Sono DUE: pre-commit (controlli statici sull'indice: cargo fmt, clippy,
# crate-boundary-check, security-scanner, lint e tsc del frontend, lint-stato,
# segreti nel diff) e pre-push (gli stessi controlli sull'albero tracciato piu'
# le suite complete: cargo test --workspace e vitest).
#
# Idempotente: eseguirlo due volte lascia la postazione nello stesso stato.
# Da eseguire dopo ogni clone fresco.

set -eu

RADICE=$(git rev-parse --show-toplevel)
cd "$RADICE"

DIRECTORY_HOOK=".githooks"
HOOKS="$DIRECTORY_HOOK/pre-commit $DIRECTORY_HOOK/pre-push"

for HOOK in $HOOKS; do
    if [ ! -f "$HOOK" ]; then
        printf "attiva-hook: %s non esiste.\n" "$HOOK" >&2
        printf "Non c'e' nulla da attivare: la postazione resta non conforme.\n" >&2
        exit 1
    fi
done

# 1. Il percorso configurabile degli hook.
ATTUALE=$(git config --get core.hooksPath || true)
if [ "$ATTUALE" = "$DIRECTORY_HOOK" ]; then
    printf "core.hooksPath: gia' impostato su %s\n" "$DIRECTORY_HOOK"
else
    git config core.hooksPath "$DIRECTORY_HOOK"
    printf "core.hooksPath: impostato su %s (era: %s)\n" \
        "$DIRECTORY_HOOK" "${ATTUALE:-<non impostato>}"
fi

for HOOK in $HOOKS; do
    # 2. Il bit di eseguibilita' sul filesystem, dove e' significativo.
    chmod +x "$HOOK" 2>/dev/null || true

    # 3. Il modo REGISTRATO in git: e' quello che un clone fresco su un
    #    sistema POSIX riceve, ed e' l'unico controllabile su un filesystem
    #    dove il bit non e' significativo (NTFS). Un hook registrato 100644
    #    arriverebbe non eseguibile su ogni altra postazione.
    MODO=$(git ls-files -s "$HOOK" | awk '{print $1}')
    if [ "$MODO" = "100755" ]; then
        printf "%s: modo registrato 100755 (eseguibile anche in un clone fresco)\n" "$HOOK"
    elif [ -z "$MODO" ]; then
        printf "attiva-hook: %s non e' ancora tracciato da git.\n" "$HOOK" >&2
        printf "Va aggiunto con modo 100755 (git add %s && git update-index --chmod=+x %s),\n" "$HOOK" "$HOOK" >&2
        printf "altrimenti le altre postazioni lo riceveranno non eseguibile.\n" >&2
        exit 1
    else
        printf "attiva-hook: %s e' registrato in git col modo %s invece di 100755.\n" \
            "$HOOK" "$MODO" >&2
        printf "Correggere con: git update-index --chmod=+x %s\n" "$HOOK" >&2
        exit 1
    fi
done

printf "\nPostazione conforme: ogni commit esegue i controlli statici sull'indice,\n"
printf "ogni push ripete i controlli statici e le suite complete (cargo test, vitest).\n"
printf "Gli hook non hanno vie d'uscita proprie: nessuna variabile di\n"
printf "scavalcamento, nessun percorso esentato. Saltarli resta possibile con\n"
printf "'git commit --no-verify' e 'git push --no-verify', che git offre a\n"
printf "chiunque: per questo gli stessi controlli vanno ripetuti in CI sulla\n"
printf "pull request, dove il ramo protetto non ammette l'equivalente.\n"

# **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
