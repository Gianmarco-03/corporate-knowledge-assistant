#!/bin/sh
#
# .githooks/controlli-comuni.sh — i controlli condivisi da pre-commit e
# pre-push, scritti UNA volta sola.
#
# Non e' un hook: git esegue solo i file che si chiamano come un hook. Lo
# includono (`. .githooks/controlli-comuni.sh`) i due hook versionati, cosi'
# lo stesso controllo non esiste in due copie destinate a divergere.
#
# Ogni passo stampa ESPLICITAMENTE uno di quattro esiti:
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
#                    .planning/ per lint-stato), oppure lo scope non contiene
#                    nulla che il controllo giudica (nessun file Rust degli
#                    spike per i passi degli spike). E' dichiarato a video,
#                    mai silenzioso;
#   NON COPERTO      il controllo esiste ma gira FUORI dall'hook, nel runner
#                    Docker degli spike (cargo test degli spike): e' dichiarato
#                    a video con il comando del runner e non blocca. Non e' un
#                    «NON APPLICABILE»: il manifest c'e', e' il controllo che
#                    sta altrove.
#
# Storia: fino al piano 01.6-06 gli esiti erano tre (PASS, FAIL, NON
# APPLICABILE) e i passi Rust guardavano solo il Cargo.toml alla radice, che
# non esiste ancora. Il codice degli spike (spikes/Cargo.toml) non passava mai
# da fmt e clippy, e il riepilogo diceva «NON APPLICABILE» senza dire che gli
# spike erano scoperti (01-REVIEW.md §B.2). Con D-08 gli spike hanno i loro
# passi (cargo fmt (spikes), cargo clippy (spikes)) e i loro test, che
# richiedono il runner, sono dichiarati NON COPERTO.
#
# NESSUNA VIA D'USCITA: nessuna variabile d'ambiente di scavalcamento,
# nessun elenco di percorsi esentati, nessun ramo sull'autore, sul ref o sul
# messaggio. CKA_PYTHON (letta da scripts/py.sh) sceglie QUALE interprete
# usare, non se eseguire i controlli.

MANIFEST_RUST="Cargo.toml"
MANIFEST_SPIKE="spikes/Cargo.toml"
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

# esito <PASS|FAIL|NON APPLICABILE|NON COPERTO> <nome del passo> [motivo]
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

# --- Passi Rust degli spike (D-08) ------------------------------------------
# Gli spike esistono: un spikes/Cargo.toml assente con codice degli spike
# nello scope e' un guasto, non un «ancora», e da' FAIL come cargo assente.
# I comandi: cargo fmt --manifest-path spikes/Cargo.toml --all -- --check e
# cargo clippy --manifest-path spikes/Cargo.toml --workspace --all-targets
# --locked --offline -- -D warnings (MANIFEST_SPIKE qui sotto).

# Percorsi su stdin -> exit 0 se almeno uno e' codice Rust degli spike (un
# .rs, un Cargo.toml o spikes/Cargo.lock sotto spikes/), 1 altrimenti.
tocca_rust_spike() {
    trovato=1
    while IFS= read -r percorso || [ -n "$percorso" ]; do
        case "$percorso" in
            spikes/*.rs|spikes/*Cargo.toml|spikes/Cargo.lock) trovato=0 ;;
        esac
    done
    return "$trovato"
}

passo_spike_fmt() {
    titolo "cargo fmt --manifest-path $MANIFEST_SPIKE --all -- --check"
    if [ ! -f "$MANIFEST_SPIKE" ]; then
        esito "FAIL" "cargo fmt (spikes)" "manifest assente: $MANIFEST_SPIKE con codice degli spike nello scope"
    elif ! ha_comando cargo; then
        esito "FAIL" "cargo fmt (spikes)" "cargo non trovato nel PATH con codice degli spike nello scope"
    else
        esegui "cargo fmt (spikes)" cargo fmt --manifest-path "$MANIFEST_SPIKE" --all -- --check
    fi
}

passo_spike_clippy() {
    titolo "cargo clippy --manifest-path $MANIFEST_SPIKE --workspace --all-targets --locked --offline -- -D warnings"
    if [ ! -f "$MANIFEST_SPIKE" ]; then
        esito "FAIL" "cargo clippy (spikes)" "manifest assente: $MANIFEST_SPIKE con codice degli spike nello scope"
    elif ! ha_comando cargo; then
        esito "FAIL" "cargo clippy (spikes)" "cargo non trovato nel PATH con codice degli spike nello scope"
    else
        esegui "cargo clippy (spikes)" cargo clippy --manifest-path "$MANIFEST_SPIKE"             --workspace --all-targets --locked --offline -- -D warnings
    fi
}

# I test degli spike girano solo nel runner Docker (servizi, modelli, GPU):
# l'hook li dichiara NON COPERTO con il comando da usare, e non blocca.
esito_non_coperto_spike() {
    esito "NON COPERTO" "cargo test (spikes)" "richiede il runner Docker: bash spikes/tools/session.sh …"
    printf "  nel runner: bash spikes/tools/session.sh with <piano> -- bash -c 'bash spikes/tools/session.sh oneshot <run_id> -- cargo test --manifest-path %s …'
" "$MANIFEST_SPIKE"
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
# codice blocca. Soglia --fail-on HIGH (decisione utente 2026-10-21): bloccano
# CRITICAL e HIGH; i MEDIUM si stampano come avvisi e la giustificazione va
# nel SUMMARY del piano. Vale per pre-commit e pre-push, che usano entrambi
# questa funzione.
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
    sh scripts/py.sh "$SCRIPT_SICUREZZA" --fail-on HIGH "$istantanea" || rc=$?
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
#
# Due gruppi di regole, in unione: una riga segnalata da uno dei due esce.
#
# 1. Regole di D-02, in parita' con lo scanner (skills/security-scanner/
#    security_scan.py dopo 01.6-02; la parita' la prova il test
#    test_secret_parity_with_scanner, non la lettura). Le implementa il
#    programma awk qui sotto, con regex POSIX senza intervalli:
#    - env-secret-literal: a inizio riga (dopo `export ` o `- ` opzionali) un
#      nome con SECRET, PASSWORD, API_KEY, ACCESS_KEY o TOKEN (maiuscole
#      distinte), TYPESAFE_API_KEY o JWT_*KEY/SECRET, poi `=` o `:` e un
#      valore di qualunque lunghezza;
#    - hardcoded-password e hardcoded-apikey: senza distinzione di
#      maiuscole, valore fra virgolette di almeno 3 e 8 caratteri;
#    - known-key-prefix: sk-/sk-ant- con il confine a sinistra, GK piu' 24
#      esadecimali con i confini.
#    Le esenzioni sono quelle dello scanner, e valgono SOLO per queste regole:
#    segnaposto della lista chiusa (ma un valore uguale al nome, o a una sua
#    parola, non lo e'), sole cifre sotto un nome da contatore (suffisso in
#    coda e nessun termine da segreto nel resto del nome), numero con unita'
#    di misura sotto un nome con token senza termini forti, e nei file .py (dall'intestazione `+++ b/...` del diff) un
#    valore di env-secret-literal senza virgolette. L'hook applica le regole
#    di D-02 a ogni tipo di file: dove lo scanner non applica
#    env-secret-literal (.rs, .ts, .sql) l'hook resta piu' severo.
# 2. Regole storiche dell'hook, invariate e piu' severe dove gia' lo erano
#    (nessun segnaposto dello scanner si applica a loro): chiavi private
#    PEM, TYPESAFE_API_KEY valorizzata, chiavi/segreti JWT valorizzati, URL
#    Postgres con password reale (i soli segnaposto change-me, ${VAR} e
#    <...> sono ammessi), token di provider noti.
#
# Il programma awk marca ogni riga aggiunta con D (segnalata da D-02) o N;
# le righe N passano poi dalle regole storiche. Il marcatore si toglie prima
# dell'uscita, tagliata a 200 colonne.
filtra_righe_segrete() {
    awk '
    BEGIN {
        q = sprintf("%c", 39)
        dq = "\""
        qc = "[" dq q "]"
        nv = "[^" dq q "[:space:]]"
        w = "[a-z0-9_]"
        sep = qc "?[ [:space:]]*[:=][ [:space:]]*" qc
        re_pwd = w "*(password|passwd|pwd)" w "*" sep nv nv nv nv "*" qc
        re_key = w "*(api[_-]?key|apikey|secret|token|client[_-]?secret|private[_-]?key|access[_-]?key)" w "*" sep
        for (i = 0; i < 8; i++) re_key = re_key nv
        re_key = re_key nv "*" qc
        re_sk = "(^|[^A-Za-z0-9_-])sk-"
        for (i = 0; i < 20; i++) re_sk = re_sk "[A-Za-z0-9_-]"
        re_gk = "(^|[^A-Za-z0-9_])GK"
        for (i = 0; i < 24; i++) re_gk = re_gk "[0-9a-fA-F]"
        re_gk = re_gk "([^A-Za-z0-9_]|$)"
        re_val = "^[^" dq q "#[:space:]]+"
        re_virg = "^[" dq q "]+|[" dq q "]+$"
        file = ""
    }
    function pulito(v) {
        v = tolower(v)
        gsub(/^[ [:space:]]+|[ [:space:]]+$/, "", v)
        gsub(re_virg, "", v)
        return v
    }
    function uguale_nome(v, n,    k, parti, i) {
        v = pulito(v)
        if (v == "") return 0
        n = tolower(n)
        if (v == n) return 1
        k = split(n, parti, /[_-]+/)
        for (i = 1; i <= k; i++) if (parti[i] != "" && parti[i] == v) return 1
        return 0
    }
    function segnaposto(v, n) {
        gsub(/^[ [:space:]]+|[ [:space:]]+$/, "", v)
        if (v == "") return 1
        if (uguale_nome(v, n)) return 0
        return tolower(v) ~ /^(<[^>]*>|\$[{][^}]*[}]|\$[a-z0-9_]+|[{][{][^}]*[}][}]|%[a-z0-9_]+%|\*+|xxx+|\.\.\.|changeme|change[-_]me|replace[-_]?me|example[a-z0-9_]*|placeholder|dummy|your[-_][a-z0-9_]+|redacted|todo|none|null|test|fake[a-z0-9_]*)$/
    }
    function numero_unita(v, n) {
        n = tolower(n)
        if (n !~ /token/ || n ~ /password|passwd|pwd|secret|api[_-]?key|access[_-]?key|private[_-]?key|jwt/) return 0
        return pulito(v) ~ /^[0-9]+(\.[0-9]+)?(ns|us|µs|μs|ms|s|sec|min|h|d|b|kb|mb|gb|tb|kib|mib|gib|tib|%)$/
    }
    function contatore(n) {
        if (!match(n, /(_TOKENS|_MAX|_SIZE|_LEN|_LIMIT|_COUNT|_TTL)$/)) return 0
        return substr(n, 1, RSTART - 1) !~ /SECRET|PASSWORD|API_KEY|ACCESS_KEY|TOKEN/
    }
    function env_letterale(c,    r, nome, v, virg, ch) {
        r = c
        sub(/^[ [:space:]]+/, "", r)
        if (match(r, /^(export[ [:space:]]+|-[ [:space:]]+)/)) r = substr(r, RLENGTH + 1)
        if (!match(r, /^[A-Za-z0-9_]+/)) return 0
        nome = substr(r, 1, RLENGTH)
        r = substr(r, RLENGTH + 1)
        if (!(nome == "TYPESAFE_API_KEY" || nome ~ /^JWT_[A-Za-z0-9_]*(KEY|SECRET)/ || nome ~ /SECRET|PASSWORD|API_KEY|ACCESS_KEY|TOKEN/)) return 0
        if (!match(r, /^[ [:space:]]*[=:][ [:space:]]*/)) return 0
        r = substr(r, RLENGTH + 1)
        virg = 0
        ch = substr(r, 1, 1)
        if (ch == dq || ch == q) { virg = 1; r = substr(r, 2) }
        if (!match(r, re_val)) return 0
        v = substr(r, 1, RLENGTH)
        if (segnaposto(v, nome)) return 0
        if (v ~ /^[0-9]+$/ && contatore(nome)) return 0
        if (numero_unita(v, nome)) return 0
        if (!virg && tolower(file) ~ /\.py$/) return 0
        return 1
    }
    function valore_fra_virgolette(lc, re,    resto, m, nome, v, i, ch) {
        resto = lc
        while (match(resto, re)) {
            m = substr(resto, RSTART, RLENGTH)
            resto = substr(resto, RSTART + RLENGTH)
            match(m, /^[a-z0-9_-]+/)
            nome = substr(m, 1, RLENGTH)
            v = substr(m, 1, length(m) - 1)
            for (i = length(v); i > 0; i--) {
                ch = substr(v, i, 1)
                if (ch == dq || ch == q) break
            }
            v = substr(v, i + 1)
            if (!segnaposto(v, nome) && !numero_unita(v, nome)) return 1
        }
        return 0
    }
    function regole_d02(c,    lc) {
        lc = tolower(c)
        if (env_letterale(c)) return 1
        if (valore_fra_virgolette(lc, re_pwd)) return 1
        if (valore_fra_virgolette(lc, re_key)) return 1
        if (c ~ re_sk || c ~ re_gk) return 1
        return 0
    }
    /^\+\+\+ / {
        file = substr($0, 5)
        sub(/^b\//, "", file)
        next
    }
    /^\+/ {
        c = substr($0, 2)
        sub(/\r$/, "", c)
        print (regole_d02(c) ? "D" : "N") $0
    }
    ' | grep -E \
        -e '^D' \
        -e '-----BEGIN [A-Z ]*PRIVATE KEY-----' \
        -e 'TYPESAFE_API_KEY[ [:space:]]*[=:][ [:space:]]*["'"'"']?[A-Za-z0-9_./+-]{8,}' \
        -e 'JWT_(SECRET|PRIVATE_KEY|SIGNING_KEY)[ [:space:]]*[=:][ [:space:]]*["'"'"']?[A-Za-z0-9_./+=-]{8,}' \
        -e 'postgres(ql)?://[^:/@[:space:]]+:[^@[:space:]]+@' \
        -e '(^|[^A-Za-z0-9_-])sk-[A-Za-z0-9_-]{20,}' \
        -e 'AKIA[0-9A-Z]{16}' \
        -e 'gh[pousr]_[A-Za-z0-9]{36}' \
    | grep -vE '^N.*postgres(ql)?://[^:/@[:space:]]+:(change-me|changeme|\$\{[^}]*\}|<[^>]*>)@' \
    | cut -c2- \
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
    printf "\n%s: nessun controllo rosso (i NON APPLICABILE e i NON COPERTO sono elencati sopra).\n" "$hook"
    exit 0
}

# **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
# **Modifica:** Claude (gsd-executor 01.6-06) · 2026-10-29 · fmt e clippy degli spike, esito NON COPERTO, segreti allineati a D-02 (gap-closure 1.6, D-08)
