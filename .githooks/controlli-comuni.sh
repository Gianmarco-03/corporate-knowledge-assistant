#!/bin/sh
#
# .githooks/controlli-comuni.sh — i controlli condivisi da pre-commit e
# pre-push, scritti UNA volta sola.
#
# Non e' un hook: git esegue solo i file che si chiamano come un hook. Lo
# includono (`. .githooks/controlli-comuni.sh`) i due hook versionati, cosi'
# lo stesso controllo non esiste in due copie destinate a divergere.
#
# Ogni passo stampa ESPLICITAMENTE uno di tre esiti:
#   PASS             il controllo e' stato eseguito ed e' uscito 0;
#   FAIL             il controllo e' uscito diverso da 0, oppure NON e' stato
#                    eseguito per un motivo che non e' l'assenza del manifest
#                    (strumento mancante, script mancante, exit 2 dello
#                    script con il manifest presente). Un controllo che non
#                    gira blocca esattamente come un controllo rosso, perche'
#                    un controllo che si salta e' indistinguibile da uno che
#                    passa;
#   NON APPLICABILE  il manifest di cui il controllo ha bisogno non esiste
#                    ANCORA (niente Cargo.toml alla radice per i passi Rust,
#                    niente apps/web/package.json per i passi web, niente
#                    .planning/ per lint-stato). E' l'unico salto ammesso, ed
#                    e' dichiarato a video, mai silenzioso.
#
# NESSUNA VIA D'USCITA: nessuna variabile d'ambiente di scavalcamento,
# nessun elenco di percorsi esentati, nessun ramo sull'autore, sul ref o sul
# messaggio. CKA_PYTHON (letta da scripts/py.sh) sceglie QUALE interprete
# usare, non se eseguire i controlli.

MANIFEST_RUST="Cargo.toml"
DIRECTORY_WEB="apps/web"
MANIFEST_WEB="$DIRECTORY_WEB/package.json"
DIRECTORY_PIANIFICAZIONE=".planning"

SCRIPT_CONFINI="skills/crate-boundary-check/boundary_check.py"
SCRIPT_SICUREZZA="skills/security-scanner/security_scan.py"
SCRIPT_LINT_STATO="scripts/lint-stato.cjs"

FALLIMENTI=""
RIEPILOGO=""

annota() {
    FALLIMENTI="${FALLIMENTI}${1}
"
}

titolo() {
    printf "\n--- %s\n" "$1"
}

# esito <PASS|FAIL|NON APPLICABILE> <nome del passo> [motivo]
esito() {
    stato="$1"
    nome="$2"
    motivo="${3:-}"
    if [ -n "$motivo" ]; then
        riga="$stato — $nome ($motivo)"
    else
        riga="$stato — $nome"
    fi
    RIEPILOGO="${RIEPILOGO}${riga}
"
    if [ "$stato" = "FAIL" ]; then
        printf "%s\n" "$riga" >&2
        annota "$nome${motivo:+ ($motivo)}"
    else
        printf "%s\n" "$riga"
    fi
}

# esegui <nome del passo> <comando...>: PASS se il comando esce 0, FAIL altrimenti.
esegui() {
    nome="$1"
    shift
    if "$@"; then
        esito "PASS" "$nome"
    else
        esito "FAIL" "$nome" "rc=$?"
    fi
}

# --- Gli strumenti ----------------------------------------------------------
# Uno strumento assente con il manifest presente e' FAIL, mai un salto.

PYTHON_OK=0
if sh scripts/py.sh -c 'import sys' >/dev/null 2>&1; then
    PYTHON_OK=1
fi

ha_comando() {
    command -v "$1" >/dev/null 2>&1
}

# --- Passi Rust -------------------------------------------------------------

passo_cargo_fmt() {
    titolo "cargo fmt --all -- --check"
    if [ ! -f "$MANIFEST_RUST" ]; then
        esito "NON APPLICABILE" "cargo fmt" "manifest assente: $MANIFEST_RUST"
    elif ! ha_comando cargo; then
        esito "FAIL" "cargo fmt" "cargo non trovato nel PATH con $MANIFEST_RUST presente"
    else
        esegui "cargo fmt" cargo fmt --all -- --check
    fi
}

passo_cargo_clippy() {
    titolo "cargo clippy --workspace --all-targets -- -D warnings"
    if [ ! -f "$MANIFEST_RUST" ]; then
        esito "NON APPLICABILE" "cargo clippy" "manifest assente: $MANIFEST_RUST"
    elif ! ha_comando cargo; then
        esito "FAIL" "cargo clippy" "cargo non trovato nel PATH con $MANIFEST_RUST presente"
    else
        esegui "cargo clippy" cargo clippy --workspace --all-targets -- -D warnings
    fi
}

passo_cargo_test() {
    titolo "cargo test --workspace"
    if [ ! -f "$MANIFEST_RUST" ]; then
        esito "NON APPLICABILE" "cargo test" "manifest assente: $MANIFEST_RUST"
    elif ! ha_comando cargo; then
        esito "FAIL" "cargo test" "cargo non trovato nel PATH con $MANIFEST_RUST presente"
    else
        esegui "cargo test" cargo test --workspace
    fi
}

# crate-boundary-check: 0 verde, 1 violazioni, 2 non eseguito. Il 2 vale
# NON APPLICABILE solo se Cargo.toml e' davvero assente; con il manifest
# presente un 2 e' un controllo non eseguito, e blocca.
passo_confini() {
    titolo "crate-boundary-check ($SCRIPT_CONFINI)"
    if [ ! -f "$SCRIPT_CONFINI" ]; then
        esito "FAIL" "crate-boundary-check" "script assente: $SCRIPT_CONFINI"
        return
    fi
    if [ "$PYTHON_OK" -ne 1 ]; then
        esito "FAIL" "crate-boundary-check" "nessun interprete Python 3 (scripts/py.sh)"
        return
    fi
    rc=0
    sh scripts/py.sh "$SCRIPT_CONFINI" . || rc=$?
    if [ "$rc" -eq 0 ]; then
        esito "PASS" "crate-boundary-check"
    elif [ "$rc" -eq 2 ] && [ ! -f "$MANIFEST_RUST" ]; then
        esito "NON APPLICABILE" "crate-boundary-check" "manifest assente: $MANIFEST_RUST"
    elif [ "$rc" -eq 2 ]; then
        esito "FAIL" "crate-boundary-check" "rc=2: non eseguito con $MANIFEST_RUST presente"
    else
        esito "FAIL" "crate-boundary-check" "rc=$rc"
    fi
}

# security-scanner su una directory-istantanea (il contenuto da giudicare,
# non l'albero di lavoro). Sempre applicabile: 0 verde, qualsiasi altro
# codice blocca.
passo_sicurezza() {
    istantanea="$1"
    descrizione="$2"
    titolo "security-scanner su $descrizione ($SCRIPT_SICUREZZA)"
    if [ ! -f "$SCRIPT_SICUREZZA" ]; then
        esito "FAIL" "security-scanner" "script assente: $SCRIPT_SICUREZZA"
        return
    fi
    if [ "$PYTHON_OK" -ne 1 ]; then
        esito "FAIL" "security-scanner" "nessun interprete Python 3 (scripts/py.sh)"
        return
    fi
    rc=0
    sh scripts/py.sh "$SCRIPT_SICUREZZA" "$istantanea" || rc=$?
    if [ "$rc" -eq 0 ]; then
        esito "PASS" "security-scanner"
    else
        esito "FAIL" "security-scanner" "rc=$rc"
    fi
}

# --- Passi web --------------------------------------------------------------

# Il manifest presente senza node_modules e' FAIL: le dipendenze mancanti
# non sono un manifest assente.
web_pronto() {
    nome="$1"
    if [ ! -f "$MANIFEST_WEB" ]; then
        esito "NON APPLICABILE" "$nome" "manifest assente: $MANIFEST_WEB"
        return 1
    fi
    if ! ha_comando npm || ! ha_comando npx; then
        esito "FAIL" "$nome" "npm/npx non trovati nel PATH con $MANIFEST_WEB presente"
        return 1
    fi
    if [ ! -d "$DIRECTORY_WEB/node_modules" ]; then
        esito "FAIL" "$nome" "dipendenze non installate: eseguire 'npm ci' in $DIRECTORY_WEB"
        return 1
    fi
    return 0
}

passo_web_lint() {
    titolo "web: npm run lint ($DIRECTORY_WEB)"
    web_pronto "web lint" || return
    esegui "web lint" sh -c "cd '$DIRECTORY_WEB' && npm run lint"
}

passo_web_tsc() {
    titolo "web: npx tsc --noEmit ($DIRECTORY_WEB)"
    web_pronto "web tsc" || return
    esegui "web tsc" sh -c "cd '$DIRECTORY_WEB' && npx tsc --noEmit"
}

passo_web_vitest() {
    titolo "web: npx vitest run ($DIRECTORY_WEB)"
    web_pronto "web vitest" || return
    esegui "web vitest" sh -c "cd '$DIRECTORY_WEB' && npx vitest run"
}

# --- Stato GSD --------------------------------------------------------------

passo_lint_stato() {
    titolo "lint-stato ($SCRIPT_LINT_STATO)"
    if [ ! -d "$DIRECTORY_PIANIFICAZIONE" ]; then
        esito "NON APPLICABILE" "lint-stato" "directory assente: $DIRECTORY_PIANIFICAZIONE"
    elif ! ha_comando node; then
        esito "FAIL" "lint-stato" "node non trovato nel PATH con $DIRECTORY_PIANIFICAZIONE presente"
    else
        esegui "lint-stato" node "$SCRIPT_LINT_STATO"
    fi
}

# --- Segreti ----------------------------------------------------------------

# Nomi di file che non entrano MAI nella storia. .env.example e' il solo
# file .env* ammesso, ed e' un modello senza valori.
e_nome_segreto() {
    base=${1##*/}
    case "$base" in
        .env.example) return 1 ;;
        .env|.env.*) return 0 ;;
        *.pem|*.key|*.p12|*.pfx) return 0 ;;
        id_rsa|id_rsa.*|id_ed25519|id_ed25519.*) return 0 ;;
        *.local.json) return 0 ;;
    esac
    return 1
}

# Elenco di percorsi su stdin -> quelli con nome da segreto su stdout.
filtra_nomi_segreti() {
    while IFS= read -r percorso; do
        [ -z "$percorso" ] && continue
        if e_nome_segreto "$percorso"; then
            printf "%s\n" "$percorso"
        fi
    done
}

# Righe AGGIUNTE di un diff su stdin -> righe con valori segreti su stdout.
# Regole: chiavi private PEM, TYPESAFE_API_KEY valorizzata, chiavi/segreti
# JWT valorizzati, URL Postgres con password reale (i segnaposto change-me,
# ${VAR} e <...> sono ammessi), token di provider noti.
filtra_righe_segrete() {
    grep -E '^\+' | grep -vE '^\+\+\+ ' | grep -E \
        -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' \
        -e 'TYPESAFE_API_KEY[[:space:]]*[=:][[:space:]]*["'"'"']?[A-Za-z0-9_./+-]{8,}' \
        -e 'JWT_(SECRET|PRIVATE_KEY|SIGNING_KEY)[[:space:]]*[=:][[:space:]]*["'"'"']?[A-Za-z0-9_./+=-]{8,}' \
        -e 'postgres(ql)?://[^:/@[:space:]]+:[^@[:space:]]+@' \
        -e '(^|[^A-Za-z0-9_-])sk-[A-Za-z0-9_-]{20,}' \
        -e 'AKIA[0-9A-Z]{16}' \
        -e 'gh[pousr]_[A-Za-z0-9]{36}' \
    | grep -vE 'postgres(ql)?://[^:/@[:space:]]+:(change-me|changeme|\$\{[^}]*\}|<[^>]*>)@' \
    | cut -c1-200
}

# --- Il verdetto ------------------------------------------------------------

# verdetto <pre-commit|pre-push> <COMMIT|PUSH> <comando di scavalcamento git>
verdetto() {
    hook="$1"
    azione="$2"
    scavalcamento="$3"
    printf "\n=== %s: riepilogo ===\n" "$hook"
    printf "%s" "$RIEPILOGO" | sed "s/^/  /"
    if [ -n "$FALLIMENTI" ]; then
        printf "\n=====================================================================\n" >&2
        printf "%s RIFIUTATO — controlli falliti:\n\n" "$azione" >&2
        printf "%s" "$FALLIMENTI" | sed "s/^/  - /" >&2
        printf "\nQuesto hook non ha alcuna via d'uscita PROPRIA: nessuna variabile\n" >&2
        printf "di scavalcamento, nessun percorso esentato, nessun ramo sull'autore,\n" >&2
        printf "sul ref o sul messaggio. Saltarlo resta pero' possibile con\n" >&2
        printf "'%s', che git offre a chiunque: per questo gli stessi\n" "$scavalcamento" >&2
        printf "controlli vanno ripetuti in CI sulla pull request, dove il ramo\n" >&2
        printf "protetto non ammette l'equivalente.\n" >&2
        printf "Si corregge cio' che e' rosso e si riprova.\n" >&2
        printf "=====================================================================\n" >&2
        exit 1
    fi
    printf "\n%s: nessun controllo rosso (i NON APPLICABILE sono elencati sopra).\n" "$hook"
    exit 0
}

# **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
