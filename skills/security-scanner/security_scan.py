#!/usr/bin/env python3
"""
security_scan.py - scanner euristico di sicurezza per CKA. Zero dipendenze.

Tarato su: OWASP ASVS L1; OWASP Top 10 for LLM Applications (prompt
injection, sensitive information disclosure, insecure output handling,
excessive agency); JWT RFC 7519 / RFC 8725; Argon2id; la regola no-leak di
CKA (nessun chunk o contenuto documentale verso log, telemetria, cache, JEV o
LLM fuori dal percorso autorizzato); segreti del progetto (TYPESAFE_API_KEY,
DATABASE_URL, chiavi JWT); upload (dimensione, tipo, path traversal, zip
bomb, PDF non fidati); SSRF sulle fonti online; GDPR generico per i dati
personali nei log.

Uso:
    python security_scan.py <file_o_cartella> [<file_o_cartella> ...] [--json]
                            [--fail-on {MEDIUM,HIGH,CRITICAL}]

    --fail-on  soglia di blocco (default MEDIUM): solo i rilevamenti di
               severita' pari o superiore alla soglia rendono l'esito 1. Quelli
               sotto soglia si stampano comunque, come avvisi. L'hook
               pre-commit usa HIGH; i gate di plan e verify il default.

Esito:
    0  nessun rilevamento, oppure solo rilevamenti sotto soglia (avvisi)
    1  almeno un rilevamento di severita' >= soglia (FAIL del gate)
    2  non eseguito: nessun file scansionabile nello scope (percorsi
       inesistenti o scope vuoto). Uno scope vuoto non e' mai verde.

Nessuna soppressione e' silenziosa: una directory FIGLIA con il marcatore
FIXTURE_MARKER_FILENAME e' esclusa e il suo percorso e' sempre riportato.
Una directory passata esplicitamente come radice e' sempre scansionata.

Gap-closure 1.6 (piano 01.6-01):
  D-01  le letture fino all'EOF (`read_to_end`, `read_until`, `io::copy` verso un
        Vec) si classificano sull'istruzione: conta solo un `.take(<n>)` sull'OUTPUT
        del decoder (CWE-409); le forme indistinguibili danno «da verificare»
        (`upload-in-memory-unverified`), mai silenzio; la capacita' presa
        dall'input ha l'id `upload-in-memory-capacity-unverified`.
  D-05  una macro di log spezzata su piu' righe si valuta sul testo unito fino
        alla parentesi bilanciata, con il finding sulla riga d'apertura.
  D-06  sono handler anche gli `async fn` registrati in `.route(..)` dello stesso
        file e quelli pubblici di un modulo di handler, anche senza argomenti.
"""
import argparse
import json
import os
import re
import sys

EXIT_OK, EXIT_FINDINGS, EXIT_NOT_RUN = 0, 1, 2

# --------------------------------------------------------------------------
# Famiglie di file: ogni regola dichiara a quali si applica
# --------------------------------------------------------------------------
RS, TS, CFG, SCRIPT, DOC, SQL = "rs", "ts", "cfg", "script", "doc", "sql"
ALL = frozenset({RS, TS, CFG, SCRIPT, DOC, SQL})
CODE = frozenset({RS, TS})

_EXT_KIND = {
    ".rs": RS,
    ".ts": TS, ".tsx": TS, ".js": TS, ".jsx": TS, ".mjs": TS, ".cjs": TS,
    ".toml": CFG, ".yaml": CFG, ".yml": CFG, ".json": CFG, ".env": CFG,
    ".conf": CFG, ".ini": CFG,
    ".sh": SCRIPT, ".bash": SCRIPT, ".py": SCRIPT, ".ps1": SCRIPT,
    ".md": DOC,
    ".sql": SQL,
}


def file_kind(path):
    base = os.path.basename(path).lower()
    if base.startswith(".env"):
        return CFG
    if base == "dockerfile" or base.startswith("dockerfile.") or base.endswith(".dockerfile"):
        return CFG
    if base.startswith(("compose.", "docker-compose")) and base.endswith((".yml", ".yaml")):
        return CFG
    return _EXT_KIND.get(os.path.splitext(base)[1])


# --------------------------------------------------------------------------
# Segnaposto: un valore che non e' un segreto (non fa scattare le regole
# di segreto). Lista chiusa e dichiarata, mai allargata per far tacere un
# finding reale.
# --------------------------------------------------------------------------
_PLACEHOLDER = re.compile(
    r"""^(?:<[^>]*>|\$\{[^}]*\}|\$\w+|\{\{[^}]*\}\}|%\w+%|\*+|x{3,}|\.{3}|"""
    r"""changeme|change[-_]me|replace[-_]?me|example\w*|placeholder|dummy|"""
    r"""your[-_]\w+|redacted|todo|none|null|password|secret|test|fake\w*)$""", re.I)


def is_placeholder(val):
    return val is None or not val.strip() or bool(_PLACEHOLDER.match(val.strip()))


# --------------------------------------------------------------------------
# Prefissi di log
# --------------------------------------------------------------------------
_LOG = (r"""(?:\b(?:tracing::)?(?:trace|debug|info|warn|error|event)!\s*\(|"""
        r"""\b(?:e?println|e?print|dbg|panic)!\s*\(|"""
        r"""\b(?:console|logger|log)\.\w+\s*\()[^;]*?""")

# Contenuto documentale e di generazione: la regola no-leak di CKA.
_LEAK_TERMS = (r"""chunks?|chunk_text|prompt|system_prompt|user_prompt|completion|"""
               r"""document_text|page_text|passage|snippet|evidence|extracted_text|"""
               r"""ocr_text|raw_text|content|embedding|answer_text|context_window""")

# Dati personali generici (GDPR art. 5 par. 1 lett. c, art. 32).
_PII_TERMS = (r"""email|e_mail|phone|phone_number|telefono|address|indirizzo|"""
              r"""date_of_birth|birth_date|full_name|first_name|last_name|"""
              r"""nome|cognome|tax_id|iban""")

_SECRET_TERMS = (r"""password|passwd|pin|token|access_token|refresh_token|secret|"""
                 r"""api_key|apikey|jwt|authorization|bearer|private_key|cookie""")

# (severity, id, regex, message, kinds)
# Le regole di segreto catturano il valore nel gruppo `val`: un segnaposto
# non scatta.
RULES = [
    # ---- Segreti (ASVS V14, regola segreti di CKA) ----
    ("CRITICAL", "hardcoded-password",
     re.compile(r"""(?:password|passwd|pwd)\w*["']?\s*[:=]\s*["'](?P<val>[^"'\s]{3,})["']""", re.I),
     "Password hard-coded. Usa variabili d'ambiente / secret store.", ALL),
    ("CRITICAL", "hardcoded-apikey",
     re.compile(r"""(?:api[_-]?key|apikey|secret|token|client[_-]?secret|private[_-]?key)\w*["']?"""
                r"""\s*[:=]\s*["'](?P<val>[^"'\s]{8,})["']""", re.I),
     "Chiave/segreto hard-coded. Sposta in variabile d'ambiente o secret store.", ALL),
    ("CRITICAL", "rust-const-secret",
     re.compile(r"""\b(?:const|static)\s+\w*(?:KEY|SECRET|TOKEN|PASSWORD)\w*\s*:\s*&(?:'static\s+)?str"""
                r"""\s*=\s*"(?P<val>[^"]{6,})\""""),
     "Segreto come costante Rust. Leggilo dalla configurazione a runtime.", frozenset({RS})),
    ("CRITICAL", "env-secret-literal",
     re.compile(r"""^\s*(?:export\s+|-\s+)?(?:TYPESAFE_API_KEY|JWT_\w*(?:KEY|SECRET)\w*|\w*_API_KEY|"""
                r"""\w*_SECRET|\w*_PASSWORD)\s*[=:]\s*["']?(?P<val>[^"'\s#]+)"""),
     "Segreto con valore letterale in un file versionato (env, Compose, script). Solo "
     "segnaposto (`${VAR}`, `<...>`, `changeme`) nei file versionati; i valori reali in "
     "`.env` ignorato da git o nel secret store.", frozenset({CFG, SCRIPT, DOC})),
    ("CRITICAL", "connstring-inline-pwd",
     re.compile(r"""\b(?:postgres(?:ql)?|mysql|redis|rediss|amqp|mongodb(?:\+srv)?)://[^:/\s"'@]+:"""
                r"""(?P<val>[^@\s"'/]+)@""", re.I),
     "Password dentro una connection string (es. DATABASE_URL). Componi l'URL a runtime "
     "da variabili d'ambiente.", ALL),
    ("CRITICAL", "private-key-block",
     re.compile(r"""-----BEGIN (?:[A-Z]+ )?PRIVATE KEY-----"""),
     "Chiave privata versionata (es. chiave di firma JWT). Rimuovila, ruotala e caricala "
     "dal secret store.", ALL),
    ("CRITICAL", "sql-role-password",
     re.compile(r"""\bPASSWORD\s+'(?P<val>[^']+)'""", re.I),
     "Password di ruolo DB in una migrazione/script SQL. Gestisci le credenziali fuori "
     "dalle migrazioni.", frozenset({SQL})),
    ("HIGH", "jwt-literal",
     re.compile(r"""\beyJ[A-Za-z0-9_-]{10,}\.eyJ[A-Za-z0-9_-]{10,}\.[A-Za-z0-9_-]{10,}"""),
     "JWT letterale committato. Se e' reale va revocato; nei test generalo con una chiave "
     "di test a runtime.", ALL),

    # ---- Iniezione SQL (ASVS V5) ----
    ("CRITICAL", "sql-format",
     re.compile(r"""format!\s*\(\s*r?#*"\s*(?:SELECT|INSERT|UPDATE|DELETE|WITH)\b[^"]*\{""", re.I),
     "SQL costruito con format!: rischio SQL injection. Usa query parametrizzate sqlx "
     "(`$1`, `bind`) o le macro `query!`.", frozenset({RS})),
    ("CRITICAL", "sql-query-dynamic",
     re.compile(r"""\bquery(?:_as|_scalar|_with)?(?:::<[^>]*>)?\s*\(\s*&?\s*(?:format!|&\w+\s*\+)"""),
     "sqlx::query su stringa dinamica: parametrizza con `bind`.", frozenset({RS})),
    ("CRITICAL", "sql-template",
     re.compile(r"""`\s*(?:SELECT|INSERT|UPDATE|DELETE)\b[^`]*\$\{""", re.I),
     "SQL in template literal con interpolazione: parametrizza.", frozenset({TS})),
    ("CRITICAL", "sql-concat",
     re.compile(r"""["']\s*(?:SELECT|INSERT|UPDATE|DELETE)\b[^"']*["']\s*\+\s*\w""", re.I),
     "SQL costruito per concatenazione: parametrizza.", CODE),

    # ---- TLS (ASVS V9) ----
    ("CRITICAL", "tls-bypass",
     re.compile(r"""danger_accept_invalid_certs\s*\(\s*true|danger_accept_invalid_hostnames\s*\(\s*true|"""
                r"""\.dangerous\(\)\s*\.\s*set_certificate_verifier|NODE_TLS_REJECT_UNAUTHORIZED\s*[=:]\s*["']?0|"""
                r"""rejectUnauthorized\s*:\s*false|\bverify\s*=\s*False\b|\bcurl\b[^\n]*\s(?:-k|--insecure)\b"""),
     "Validazione dei certificati TLS disabilitata. Ripristinala.", frozenset({RS, TS, CFG, SCRIPT})),
    ("HIGH", "http-url",
     re.compile(r"""["']http://(?!127\.0\.0\.1|localhost|\[::1\]|0\.0\.0\.0)[^"'\s]+["']""", re.I),
     "URL in chiaro verso un host instradabile. Usa https:// (JEV, LLM, embedding, fonti "
     "online) e metti gli URL in configurazione.", frozenset({RS, TS, CFG})),

    # ---- JWT (RFC 7519 / RFC 8725) ----
    ("CRITICAL", "jwt-alg-none",
     re.compile(r"""Algorithm::None\b|["']alg["']\s*:\s*["']none["']|algorithms\s*:\s*\[[^\]]*["']none["']""", re.I),
     "JWT con algoritmo `none`: RFC 8725 §3.1 vieta di accettarlo.", CODE),
    ("CRITICAL", "jwt-no-verify",
     re.compile(r"""insecure_disable_signature_validation|dangerous_insecure_decode|"""
                r"""dangerous::insecure_decode|validate_exp\s*=\s*false|validate_aud\s*=\s*false|"""
                r"""ignoreExpiration\s*:\s*true"""),
     "JWT senza verifica di firma, scadenza o audience. RFC 8725 §3: verifica sempre "
     "firma, `exp`, `iss`, `aud` e l'algoritmo atteso.", CODE),
    ("HIGH", "jwt-symmetric",
     re.compile(r"""Algorithm::HS(?:256|384|512)\b|["']HS(?:256|384|512)["']|"""
                r"""(?:Encoding|Decoding)Key::from_secret"""),
     "JWT con firma simmetrica: CKA usa firma asimmetrica con JWKS (verifica locale in "
     "ogni servizio senza condividere il segreto di firma).", frozenset({RS, TS, CFG})),
    ("MEDIUM", "jwt-decode-unverified",
     re.compile(r"""\bjwtDecode\s*\(|\bjwt_decode\s*\(|\bjwt\.decode\s*\("""),
     "Decodifica JWT senza verifica: ammessa solo per UX, mai per decisioni di "
     "autorizzazione (che restano lato server).", frozenset({TS})),

    # ---- Password e crittografia (ASVS V2, V6) ----
    ("HIGH", "argon2-variant",
     re.compile(r"""Algorithm::Argon2(?:i|d)\b|Variant::Argon2(?:i|d)\b"""),
     "Variante Argon2 diversa da Argon2id. La decisione di progetto e' Argon2id.",
     frozenset({RS})),
    ("HIGH", "weak-crypto",
     re.compile(r"""\b(?:md5|sha1)::|\b(?:Md5|Sha1)::|\bmd5\s*\(|createHash\s*\(\s*["'](?:md5|sha1)["']|"""
                r"""\b(?:Des|TripleDes|Rc4)\b"""),
     "Algoritmo crittografico debole. Usa SHA-256+ per hashing, AEAD per cifratura, "
     "Argon2id per le password.", CODE),
    ("HIGH", "plaintext-pwd-compare",
     re.compile(r"""\b(?:password|pwd|pass)\s*[!=]==?\s*|[!=]==?\s*(?:password|pwd)\b""", re.I),
     "Confronto password in chiaro. Verifica con Argon2id (`verify_password`), tempo "
     "costante.", CODE),
    ("MEDIUM", "insecure-random",
     re.compile(r"""\bMath\.random\s*\(|\bSmallRng\b|\bfastrand::"""),
     "Generatore non crittografico. Per token, nonce e id di sessione usa un CSPRNG "
     "(`OsRng`, `crypto.getRandomValues`).", CODE),

    # ---- No-leak e log (OWASP LLM02, GDPR) ----
    ("CRITICAL", "no-leak-in-log",
     re.compile(_LOG + r"""\b(?:""" + _LEAK_TERMS + r""")\b""", re.I),
     "Contenuto documentale o di generazione nei log. Regola no-leak di CKA: nessun "
     "chunk, prompt o testo estratto in log/telemetria; logga id e conteggi "
     "(document_id, chunk_id, versione).", CODE),
    ("CRITICAL", "no-leak-telemetry",
     re.compile(r"""(?:KeyValue::new|set_attribute|setAttribute|\.record)\s*\([^)]*\b(?:"""
                + _LEAK_TERMS + r""")\b""", re.I),
     "Contenuto documentale o di generazione come attributo di span/metrica: la "
     "telemetria e' un archivio con un pubblico piu' ampio. Solo id e conteggi.", CODE),
    ("CRITICAL", "pii-in-log",
     re.compile(_LOG + r"""\b(?:""" + _PII_TERMS + r""")\b""", re.I),
     "Dato personale nei log (GDPR art. 5 par. 1 lett. c, art. 32). Logga un id "
     "pseudonimo (user_id), mai il dato.", CODE),
    ("HIGH", "log-secret",
     re.compile(_LOG + r"""\b(?:""" + _SECRET_TERMS + r""")\b""", re.I),
     "Possibile credenziale/token/header Authorization nei log.", CODE),

    # ---- Browser (ASVS V8, LLM05) ----
    ("CRITICAL", "credentials-in-webstorage",
     re.compile(r"""(?:local|session)Storage\s*(?:\.\s*setItem\s*\(\s*|\.\s*|\[\s*)["'`]?[A-Za-z0-9_.\-]*"""
                r"""(?:token|password|passwd|pwd|secret|apikey|api[_-]?key|jwt|credential|session|auth)""", re.I),
     "Token o credenziale nello storage del browser (ASVS V8): leggibile da qualunque "
     "script della pagina. Refresh token in cookie HttpOnly/Secure/SameSite, access token "
     "solo in memoria.", frozenset({TS})),
    ("HIGH", "token-in-js-cookie",
     re.compile(r"""document\.cookie\s*=\s*[^;]*(?:token|jwt|session|auth)""", re.I),
     "Cookie di sessione scritto da JavaScript: non puo' essere HttpOnly. Lo imposta il "
     "server.", frozenset({TS})),
    ("HIGH", "html-injection",
     re.compile(r"""dangerouslySetInnerHTML|\.innerHTML\s*=|\.outerHTML\s*=|insertAdjacentHTML\s*\("""),
     "HTML non sanificato nel DOM: risposte LLM e contenuti dei documenti sono input non "
     "fidati (OWASP LLM05, XSS). Renderizza come testo.", frozenset({TS})),
    ("HIGH", "creds-in-querystring",
     re.compile(r"""[?&](?:password|pwd|token|access_token|refresh_token|jwt|api_key|apikey)=""", re.I),
     "Credenziale in query string: finisce nei log dei proxy e nella cronologia. Header "
     "Authorization o body su TLS.", frozenset({RS, TS, CFG})),
    ("HIGH", "cors-permissive",
     re.compile(r"""CorsLayer::(?:very_)?permissive\s*\(|allow_origin\s*\(\s*(?:Any\b|AllowOrigin::any)|"""
                r"""AllowOrigin::mirror_request"""),
     "CORS permissivo: con credenziali espone le API a qualunque origine. Allow-list "
     "esplicita delle origini.", frozenset({RS})),

    # ---- Upload e parsing (ASVS V12) ----
    ("CRITICAL", "upload-body-limit-disabled",
     re.compile(r"""DefaultBodyLimit::disable\s*\("""),
     "Limite del body disabilitato: un upload senza tetto e' un DoS. Imposta un limite "
     "esplicito per la rotta di upload.", frozenset({RS})),
    # `read_to_end`, `read_until` e `io::copy` sono regole di file (D-01, sotto
    # FILE_RULES): qui restano le sole forme di riga.
    ("HIGH", "upload-in-memory",
     re.compile(r"""\.bytes\(\)\s*\.await|to_bytes\s*\([^)]*usize::MAX"""),
     "File letto interamente in memoria: upload e download di fonti vanno in streaming "
     "con limite di dimensione.", frozenset({RS})),
    ("HIGH", "upload-path-traversal",
     re.compile(r"""\.join\s*\([^)]*\b(?:file_name|filename|original_name|originalname)\b|"""
                r"""Path::new\s*\([^)]*\b(?:file_name|filename)\b""", re.I),
     "Percorso costruito dal nome file del client: path traversal. Salva con un id "
     "generato; il nome originale e' solo metadato.", CODE),
    ("MEDIUM", "upload-mime-trust",
     re.compile(r"""\.content_type\s*\(\s*\)"""),
     "Content-Type dichiarato dal client: non e' una validazione. Verifica per magic "
     "bytes con allow-list (PDF, DOCX).", frozenset({RS})),
    ("MEDIUM", "archive-decompress",
     re.compile(r"""ZipArchive::new|tar::Archive::new|\b\w*Decoder::new|\.extract\s*\("""),
     "Decompressione (DOCX e' uno zip): limita dimensione totale decompressa, numero di "
     "entry e rapporto di compressione (zip bomb), usa `enclosed_name()`.", frozenset({RS})),
    ("MEDIUM", "pdf-untrusted-parse",
     re.compile(r"""lopdf::|pdf_extract::|pdfium_render::|tesseract::|leptess::"""),
     "Parsing di PDF/OCR non fidato: processo isolato o spawn_blocking con timeout, "
     "limiti su pagine/oggetti, nessuna esecuzione di JavaScript o azioni embedded.",
     frozenset({RS})),

    # ---- SSRF (fonti online) ----
    ("HIGH", "ssrf-dynamic-url",
     re.compile(r"""reqwest::get\s*\(\s*(?!["'])|\.(?:get|post|put|request)\s*\(\s*(?:Method::\w+\s*,\s*)?"""
                r"""&?\s*(?:url|source_url|target_url|link|href|uri|feed_url|\w+\.url|\w+\.source_url)\b"""),
     "Richiesta verso un URL non costante: SSRF. Allow-list di schema (https) e host, "
     "blocco di IP privati/loopback/link-local/metadata dopo la risoluzione DNS, "
     "redirect limitati e riverificati, timeout e tetto di dimensione.", frozenset({RS})),

    # ---- LLM (OWASP LLM01, LLM06) ----
    ("MEDIUM", "prompt-interpolation",
     re.compile(r"""(?:system_prompt|SYSTEM_PROMPT|system_message|instructions)\w*\s*(?:=|\+=|:)\s*[^;]*"""
                r"""(?:format!|\+\s*\w|\$\{|push_str)""", re.I),
     "Prompt di sistema costruito per interpolazione (OWASP LLM01): istruzioni e "
     "contenuti non fidati in ruoli separati, chunk delimitati come dati, istruzioni "
     "non sovrascrivibili dal testo dei documenti.", CODE),
    ("HIGH", "dynamic-exec",
     re.compile(r"""Command::new\s*\(\s*(?!["'])|\bexecSync\s*\(|\bchild_process\b|\beval\s*\(|"""
                r"""\bnew\s+Function\s*\("""),
     "Esecuzione di comandi o codice dinamico: mai su output di un LLM o input utente "
     "(OWASP LLM06, command injection).", CODE),
]

BURNED_SECRETS = []  # vuota per costruzione: si ripopola solo dopo un incidente reale
_BURNED_RE = (re.compile("|".join(map(re.escape, BURNED_SECRETS)), re.I)
              if BURNED_SECRETS else None)
_BURNED_MSG = "Segreto bruciato di questo progetto nel codice: ruotalo e rimuovilo dalla storia."

FIXTURE_MARKER_FILENAME = ".security-scan-fixture"
_PRUNED_DIRS = ("target", "node_modules", ".git", "dist", "build", "coverage")

_SECRET_RULES = {"hardcoded-password", "hardcoded-apikey", "rust-const-secret",
                 "env-secret-literal", "connstring-inline-pwd", "private-key-block",
                 "sql-role-password", "jwt-literal"}
# Su una riga commentata restano attive solo le regole di segreto: un segreto
# commentato e' committato lo stesso; il resto descrive comportamento runtime.
_COMMENT_SCANNED_RULES = _SECRET_RULES
_TEST_SKIPPED_RULES = {"http-url", "jwt-literal", "insecure-random"}
# Regole sui log valutate a stringhe svuotate: conta CIO' CHE SI LOGGA (gli
# identificatori), non il testo del messaggio. `info!("chunk indicizzati: {}", n)`
# non e' una fuga; `info!("{}", chunk.text)` lo e'.
_STRIP_STRING_RULES = {"no-leak-in-log", "no-leak-telemetry", "pii-in-log", "log-secret"}
_STRING_LIT = re.compile(r'''"(?:\\.|[^"\\])*"|`(?:\\.|[^`\\])*`|'(?:\\.|[^'\\]){2,}?\'''')


def strip_strings(line):
    return _STRING_LIT.sub('""', line)


# Char literal di un carattere (`')'`, `'\''`, `'\n'`, `'\u{..}'`): `_STRING_LIT`
# svuota solo i letterali fra apici di almeno due caratteri. Un lifetime (`'a`)
# non ha l'apice di chiusura e resta com'e'.
_CHAR_LIT = re.compile(r"""b?'(?:\\(?:x[0-9A-Fa-f]{2}|u\{[0-9A-Fa-f]{1,6}\}|.)|[^\\'\n])'""")


def strip_strings_and_chars(line):
    return _CHAR_LIT.sub("''", strip_strings(line))


def _masked_rust(text):
    """Il testo con commenti e contenuto dei letterali (stringhe, stringhe raw,
    char literal) sostituiti da spazi: stessa lunghezza, stessi a capo, cosi'
    gli offset restano numeri di riga validi. I delimitatori dei letterali
    restano, il contenuto no."""
    out = list(text)
    n = len(text)
    i = 0

    def blank(a, b):
        for k in range(a, b):
            if out[k] != "\n":
                out[k] = " "

    while i < n:
        c = text[i]
        if c == "/" and i + 1 < n and text[i + 1] == "/":
            j = text.find("\n", i)
            j = n if j < 0 else j
            blank(i, j)
            i = j
            continue
        if c == "/" and i + 1 < n and text[i + 1] == "*":
            depth, j = 1, i + 2
            while j < n and depth:
                if text.startswith("/*", j):
                    depth, j = depth + 1, j + 2
                elif text.startswith("*/", j):
                    depth, j = depth - 1, j + 2
                else:
                    j += 1
            blank(i, j)
            i = j
            continue
        if c in "rb" and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == "_")):
            m = re.match(r"""(?:br|rb|r)(#*)\"""", text[i:i + 300])
            if m:
                close = '"' + m.group(1)
                start = i + m.end()
                j = text.find(close, start)
                j = n if j < 0 else j
                blank(start, j)
                i = j + len(close)
                continue
        if c == '"':
            j = i + 1
            while j < n and text[j] != '"':
                j += 2 if text[j] == "\\" else 1
            blank(i + 1, min(j, n))
            i = j + 1
            continue
        if c == "'":
            m = _CHAR_LIT.match(text, i - 1 if i > 0 and text[i - 1] == "b" else i)
            if m and m.end() > i:
                blank(i + 1, m.end() - 1)
                i = m.end()
                continue
        i += 1
    return "".join(out)


_OPEN, _CLOSE = "([{", ")]}"


def _match_paren(text, open_at):
    """Indice della parentesi che chiude quella in `open_at`, oppure None."""
    depth = 0
    for k in range(open_at, len(text)):
        ch = text[k]
        if ch in _OPEN:
            depth += 1
        elif ch in _CLOSE:
            depth -= 1
            if depth == 0:
                return k
    return None


def _split_args(text):
    """Argomenti separati dalle virgole di profondita' zero."""
    args, depth, cur = [], 0, []
    for ch in text:
        if ch in _OPEN:
            depth += 1
        elif ch in _CLOSE:
            depth -= 1
        if ch == "," and depth == 0:
            args.append("".join(cur))
            cur = []
        else:
            cur.append(ch)
    tail = "".join(cur)
    if tail.strip():
        args.append(tail)
    return args


# --------------------------------------------------------------------------
# D-01: letture fino all'EOF limitate sull'output del decoder (CWE-409)
# --------------------------------------------------------------------------

_READ_METHOD = re.compile(r"""\.\s*(?:read_to_end|read_until)\s*\(""")
_READ_UFCS = re.compile(r"""(?<![\w.:])(?:[A-Za-z_]\w*\s*::\s*)+(?:read_to_end|read_until)\s*\(""")
_IO_COPY = re.compile(r"""\bio\s*::\s*copy\s*\(""")
_DECODER_NEW = re.compile(r"""\b\w*(?:Decoder|Decompressor)\s*::\s*new\s*\(""")
_TAKE_LIMIT = re.compile(r"""\.\s*take\s*\(\s*[^\s)]""")
_LEAD_KEYWORDS = re.compile(r"""^(?:(?:if|match|while|return|else|in|move|unsafe|break)\b\s*|!\s*)+""")
_BARE_READER = re.compile(r"""^(?:\(\s*)?(?:&\s*(?:mut\s+)?)?(?:\*\s*)?[A-Za-z_]\w*(?:\s*\.\s*[A-Za-z_]\w*)*"""
                          r"""(?:\s*\))?$""")
_SINK_DST = re.compile(r"""^(?:&\s*(?:mut\s+)?)?(?:(?:std|tokio)\s*::\s*)?(?:io\s*::\s*)?"""
                       r"""(?:sink|stdout|stderr)\s*\(\s*\)(?:\s*\.\s*lock\s*\(\s*\))?$""")
_VEC_DECL = r"""\blet\s+(?:mut\s+)?{id}\s*(?::\s*Vec\s*<[^=;]*>\s*)?=\s*(?:Vec\s*::\s*new\s*\(\s*\)|vec!\s*[\[(]|""" \
            r"""Vec\s*::\s*with_capacity\s*\()"""
_WITH_CAPACITY = re.compile(r"""\b(?:Vec|String|BytesMut)\s*(?:::\s*<[^<>()]*(?:<[^<>()]*>)?[^<>()]*>\s*)?"""
                            r"""::\s*with_capacity\s*\(""")
_CONST_EXPR = re.compile(r"""^(?:\s|\d[\d_]*(?:[iu](?:8|16|32|64|128|size))?(?!\w)|0x[0-9A-Fa-f_]+(?!\w)|"""
                         r"""(?:[A-Za-z_]\w*\s*::\s*)*[A-Z][A-Z0-9_]*\b|"""
                         r"""\bas\s+[iu](?:8|16|32|64|128|size)\b|[-+*/%()<>&|^])*$""")

LIMITED, UNBOUNDED, UNVERIFIED = "limited", "unbounded", "unverified"


def _classify_reader(expr):
    """Classificazione del ricevitore (contratto di 01.6-01):
    1. con un costruttore di decoder: limitato solo se un `.take(<n>)` segue la
       parentesi di chiusura del costruttore piu' esterno (quello che si chiude
       per ultimo), mai per un `take` dentro i suoi argomenti;
    2. senza decoder, con un `.take(<n>)` sulla catena: limitato;
    3. senza decoder ne' `take`, con una chiamata nella catena: non limitato;
    4. un identificatore nudo (anche `&mut x`, `(&mut x)`): da verificare.
    `.take()` senza argomenti e' `Option::take` e non limita nulla."""
    expr = expr.strip()
    if not expr:
        return UNVERIFIED
    last_close = None
    for m in _DECODER_NEW.finditer(expr):
        close = _match_paren(expr, m.end() - 1)
        if close is None:
            return UNVERIFIED
        if last_close is None or close > last_close:
            last_close = close
    if last_close is not None:
        return LIMITED if _TAKE_LIMIT.search(expr[last_close + 1:]) else UNBOUNDED
    if _TAKE_LIMIT.search(expr):
        return LIMITED
    if _BARE_READER.match(expr):
        return UNVERIFIED
    return UNBOUNDED if "(" in expr else UNVERIFIED


def _receiver_start(text, dot):
    """Inizio del ricevitore della chiamata che comincia in `dot`: si risale
    oltre le parentesi bilanciate fino a un delimitatore di profondita' zero
    (`;`, `{`, `}`, `,`, `=`, `|`, `&&`, una parentesi aperta che contiene la
    chiamata)."""
    depth = 0
    k = dot - 1
    while k >= 0:
        ch = text[k]
        if ch in _CLOSE:
            depth += 1
        elif ch in _OPEN:
            if depth == 0:
                return k + 1
            depth -= 1
        elif depth == 0:
            if ch in ";{},=|":
                return k + 1
            if ch == "&" and k > 0 and text[k - 1] == "&":
                return k + 1
            if ch == ">" and k > 0 and text[k - 1] == "=":
                return k + 1
        k -= 1
    return 0


def _strip_lead(expr):
    s = expr.lstrip()
    return s[len(_LEAD_KEYWORDS.match(s).group(0)):] if _LEAD_KEYWORDS.match(s) else s


def _strip_ref(arg):
    return re.sub(r"""^\s*&\s*(?:mut\s+)?""", "", arg).strip()


def _is_const_expr(arg):
    return bool(arg.strip()) and _CONST_EXPR.match(arg) is not None


_D01_CACHE = {"key": None, "value": None}


def _d01_findings(lines, path):
    """Mappa {id di regola: [righe]} per le quattro regole di D-01, calcolata
    una volta per file e condivisa dalle quattro voci di FILE_RULES."""
    key = (path, id(lines), len(lines))
    if _D01_CACHE["key"] == key:
        return _D01_CACHE["value"]
    res = {"upload-in-memory": [], "upload-in-memory-unverified": [],
           "upload-in-memory-capacity-unverified": [], "unbounded-copy-to-vec": []}
    if path.lower().endswith(".rs"):
        text = _masked_rust("\n".join(lines))
        starts = [0]
        for k, ch in enumerate(text):
            if ch == "\n":
                starts.append(k + 1)

        def line_at(off):
            lo, hi = 0, len(starts) - 1
            while lo < hi:
                mid = (lo + hi + 1) // 2
                if starts[mid] <= off:
                    lo = mid
                else:
                    hi = mid - 1
            return lo + 1

        def add(rule, off):
            ln = line_at(off)
            if ln not in res[rule]:
                res[rule].append(ln)

        def multiline_with_take(span):
            return "\n" in span.strip() and _TAKE_LIMIT.search(span) is not None

        # 1. metodi `.read_to_end(` e `.read_until(`
        for m in _READ_METHOD.finditer(text):
            open_at = m.end() - 1
            close = _match_paren(text, open_at)
            start = _receiver_start(text, m.start())
            raw = text[start:m.start()]
            receiver = _strip_lead(raw)
            name_off = m.start() + text[m.start():].find("read_")
            if close is None:
                add("upload-in-memory-unverified", name_off)
                continue
            span = receiver + text[m.start():close + 1]
            if multiline_with_take(span):
                add("upload-in-memory-unverified", name_off)
                continue
            cls = _classify_reader(receiver)
            if cls == UNBOUNDED:
                add("upload-in-memory", name_off)
            elif cls == UNVERIFIED:
                add("upload-in-memory-unverified", name_off)

        # 2. forma UFCS `AsyncReadExt::read_to_end(&mut r, ..)`
        for m in _READ_UFCS.finditer(text):
            open_at = m.end() - 1
            close = _match_paren(text, open_at)
            name_off = m.start() + text[m.start():].find("read_")
            if close is None:
                add("upload-in-memory-unverified", name_off)
                continue
            body = text[open_at + 1:close]
            args = _split_args(body)
            if not args or multiline_with_take(body):
                add("upload-in-memory-unverified", name_off)
                continue
            first = _strip_ref(args[0])
            if _DECODER_NEW.search(first):
                cls = _classify_reader(first)
            else:
                cls = LIMITED if _TAKE_LIMIT.search(first) else UNVERIFIED
            if cls == UNBOUNDED:
                add("upload-in-memory", name_off)
            elif cls == UNVERIFIED:
                add("upload-in-memory-unverified", name_off)

        # 3. `io::copy(&mut <src>, &mut <dst>)`
        for m in _IO_COPY.finditer(text):
            open_at = m.end() - 1
            close = _match_paren(text, open_at)
            if close is None:
                add("upload-in-memory-unverified", m.start())
                continue
            body = text[open_at + 1:close]
            args = _split_args(body)
            if len(args) < 2 or multiline_with_take(body):
                add("upload-in-memory-unverified", m.start())
                continue
            cls = _classify_reader(_strip_ref(args[0]))
            if cls == LIMITED:
                continue
            dst = args[1].strip()
            if _SINK_DST.match(dst):
                continue
            ident = _strip_ref(dst)
            if (re.fullmatch(r"""[A-Za-z_]\w*""", ident)
                    and re.search(_VEC_DECL.format(id=re.escape(ident)), text)):
                add("unbounded-copy-to-vec" if cls == UNBOUNDED else "upload-in-memory-unverified",
                    m.start())
            else:
                add("upload-in-memory-unverified", m.start())

        # 4. capacita' del buffer presa dall'input
        for m in _WITH_CAPACITY.finditer(text):
            open_at = m.end() - 1
            close = _match_paren(text, open_at)
            if close is None or not _is_const_expr(text[open_at + 1:close]):
                add("upload-in-memory-capacity-unverified", m.start())
    for v in res.values():
        v.sort()
    _D01_CACHE["key"], _D01_CACHE["value"] = key, res
    return res


def unbounded_read_to_end(lines, path):
    return _d01_findings(lines, path)["upload-in-memory"]


def unverified_read(lines, path):
    return _d01_findings(lines, path)["upload-in-memory-unverified"]


def capacity_from_input(lines, path):
    return _d01_findings(lines, path)["upload-in-memory-capacity-unverified"]


def unbounded_copy_to_vec(lines, path):
    return _d01_findings(lines, path)["unbounded-copy-to-vec"]

# --------------------------------------------------------------------------
# Regole di file (contesto multi-riga)
# --------------------------------------------------------------------------

_HANDLER_START = re.compile(r"""^\s*(?:pub(?:\([^)]*\))?\s+)?async\s+fn\s+\w+\s*(?:<[^>]*>)?\s*\(""")
_AXUM_EXTRACTOR = re.compile(r"""\b(?:Json|Path|Query|Form|Extension|State|TypedHeader)\s*<|"""
                             r"""\bMultipart\b|\bHeaderMap\b""")
_AUTH_EXTRACTOR = re.compile(r"""\b(?:AuthUser|AuthenticatedUser|Authenticated|Claims|Subject|Principal|"""
                             r"""CurrentUser|RequireRole|RequirePermission|Authorized\w*|ServiceCaller|"""
                             r"""InternalCaller|ServiceIdentity)\b""")
_WAIVER_ANY = re.compile(r"""//\s*security-scanner\s*:\s*(?:public-endpoint|authz-layer)\b""", re.I)
_WAIVER_OK = re.compile(r"""//\s*security-scanner\s*:\s*(?:public-endpoint|authz-layer)\s*[-—–:]+\s*\S.{2,}""",
                        re.I)
_HANDLER_NAME = re.compile(r"""^\s*(?:pub(?:\([^)]*\))?\s+)?async\s+fn\s+(?P<name>\w+)""")
_PUB_ASYNC = re.compile(r"""^\s*pub(?:\([^)]*\))?\s+async\s+fn\b""")
_ROUTE_CALL = re.compile(r"""\.\s*route\s*\(""")
_ROUTE_METHOD = re.compile(r"""(?<![\w.])(?:\w+\s*::\s*)*(get|post|put|patch|delete|head|options|trace|any|on)"""
                           r"""\s*\(|\.\s*(get|post|put|patch|delete|head|options|trace|any|on)\s*\(""")
_PLAIN_PATH = re.compile(r"""^(?:[A-Za-z_]\w*\s*::\s*)*(?P<last>[A-Za-z_]\w*)$""")


def _is_handler_module(path):
    n = path.replace("\\", "/").lower()
    base = n.rsplit("/", 1)[-1]
    return base in ("handler.rs", "handlers.rs") or "/handlers/" in n


def _routed_handler_names(lines):
    """Nomi registrati come handler in `.route(<path>, <method_router>)`: la
    ricerca e' sul testo del file a commenti e stringhe svuotati, con le righe
    unite (`.route(` e `post(upload)` possono stare su righe diverse)."""
    text = _masked_rust("\n".join(lines))
    names = set()
    for m in _ROUTE_CALL.finditer(text):
        open_at = m.end() - 1
        close = _match_paren(text, open_at)
        if close is None:
            continue
        args = _split_args(text[open_at + 1:close])
        if len(args) < 2:
            continue
        router = args[1]
        pos = 0
        # Solo la catena del MethodRouter: le chiamate annidate negli argomenti
        # (il corpo di una closure) non sono registrazioni.
        while True:
            mm = _ROUTE_METHOD.search(router, pos)
            if not mm:
                break
            inner_open = mm.end() - 1
            inner_close = _match_paren(router, inner_open)
            if inner_close is None:
                break
            pos = inner_close + 1
            inner = _split_args(router[inner_open + 1:inner_close])
            if not inner:
                continue
            method = mm.group(1) or mm.group(2)
            cand = (inner[-1] if method == "on" else inner[0]).strip()
            pm = _PLAIN_PATH.match(cand)
            if pm:
                names.add(pm.group("last"))
    return names


def axum_handlers_without_authz(lines, path):
    """Deny by default (ASVS V4): ogni handler Axum (fn async con estrattori
    Axum) porta una decisione di autorizzazione: un estrattore di identita'
    nella firma, oppure un commento sentinella MOTIVATO subito sopra:
    `// security-scanner: public-endpoint - <motivazione>` (login, JWKS,
    health) o `// security-scanner: authz-layer - <dove sta il layer>`.
    Una rinuncia senza motivazione e' essa stessa una violazione.

    D-06: e' un handler anche, senza argomenti, un `async fn` il cui nome e'
    registrato in `.route(<path>, <metodo>(<nome>))` dello stesso file (catene
    `get(a).post(b)` comprese, nome anche qualificato), oppure ogni `async fn`
    pubblico di un modulo di handler (`handler.rs`, `handlers.rs`, cartella
    `handlers/`). Le closure non sono nomi e non contano."""
    if not path.lower().endswith(".rs"):
        return []
    routed = _routed_handler_names(lines)
    module = _is_handler_module(path)
    out = []
    i = 0
    while i < len(lines):
        m = _HANDLER_START.match(lines[i])
        if not m:
            i += 1
            continue
        sig, j = lines[i], i
        while j + 1 < len(lines) and "{" not in lines[j] and j - i < 25:
            j += 1
            sig += lines[j]
        name = _HANDLER_NAME.match(lines[i]).group("name")
        is_handler = (_AXUM_EXTRACTOR.search(sig) is not None or name in routed
                      or (module and _PUB_ASYNC.match(lines[i]) is not None))
        if is_handler and not _AUTH_EXTRACTOR.search(sig):
            k, decided, unmotivated = i - 1, False, False
            while k >= 0 and re.match(r"""^\s*(?://|#\[|$)""", lines[k]):
                if _WAIVER_ANY.search(lines[k]):
                    if _WAIVER_OK.search(lines[k]):
                        decided = True
                    else:
                        unmotivated = True
                k -= 1
            if unmotivated or not decided:
                out.append(i + 1)
        i = j + 1
    return out


_INSTRUMENT = re.compile(r"""#\[\s*(?:tracing::)?instrument\b([^\]]*)\]""")
_SENSITIVE_PARAM = re.compile(
    r"""\b(?:chunks?|prompt|evidence|content|document|query|question|password|token|"""
    r"""claims|credentials)\s*:|:\s*&?(?:\w+::)*(?:\w*Chunk\w*|\w*Prompt\w*|\w*Evidence\w*|"""
    r"""ParsedDocument|DocumentStream|UserQuery|GroundedAnswerRequest|LoginRequest)\b""")


def instrument_without_skip(lines, path):
    """`#[instrument]` registra come campi dello span TUTTI gli argomenti
    (Debug): su una fn che riceve chunk, prompt, query o credenziali e' una
    fuga verso la telemetria. Serve `skip(...)`/`skip_all`."""
    if not path.lower().endswith(".rs"):
        return []
    out = []
    for i, line in enumerate(lines):
        m = _INSTRUMENT.search(line)
        if not m or "skip" in m.group(1):
            continue
        sig = ""
        for k in range(i + 1, min(i + 15, len(lines))):
            sig += lines[k]
            if "{" in lines[k] or ";" in lines[k]:
                break
        if _SENSITIVE_PARAM.search(sig):
            out.append(i + 1)
    return out


FILE_RULES = [
    ("CRITICAL", "endpoint-without-authz-decision", axum_handlers_without_authz,
     "Handler Axum senza decisione di autorizzazione (deny by default): aggiungi un "
     "estrattore di identita' oppure un commento MOTIVATO `// security-scanner: "
     "public-endpoint - <motivazione>` / `// security-scanner: authz-layer - <dove>`."),
    ("CRITICAL", "instrument-without-skip", instrument_without_skip,
     "#[instrument] su una fn con chunk/prompt/query/credenziali senza `skip`: gli "
     "argomenti finiscono nella telemetria (regola no-leak)."),
    # ---- D-01: letture fino all'EOF (CWE-409, ASVS 5.0 V5.2.3) ----
    ("HIGH", "upload-in-memory", unbounded_read_to_end,
     "Lettura fino all'EOF senza limite sull'output (D-01, CWE-409): `read_to_end`/"
     "`read_until` su un decoder o un reader non limitato e' una bomba di decompressione. "
     "Metti `.take(quota + 1)` sull'OUTPUT del decoder (dopo la parentesi del costruttore, "
     "non sui byte compressi) e rifiuta oltre la quota; nella fase 3 il lettore limitato unico."),
    ("MEDIUM", "upload-in-memory-unverified", unverified_read,
     "Lettura fino all'EOF da verificare (D-01, CWE-409): il reader e' costruito altrove, "
     "l'istruzione e' su piu' righe con un `.take(` di posizione non determinabile, la forma "
     "e' UFCS o la destinazione di `io::copy` non si riconosce. Verifica che l'output del "
     "decoder sia limitato da `.take(quota + 1)` e motivalo nel SUMMARY; nella fase 3 il "
     "lettore limitato unico."),
    ("MEDIUM", "upload-in-memory-capacity-unverified", capacity_from_input,
     "Capacita' del buffer presa da un'espressione non costante (D-01): se viene dall'input "
     "e' un'allocazione controllata dall'attaccante (CWE-789, CWE-409). Limita la capacita' "
     "con la quota (`min(n, QUOTA)`) o motivala nel SUMMARY: la capacita' iniziale non e' "
     "un limite di lettura, il limite sta nel lettore (`.take(quota + 1)`)."),
    ("HIGH", "unbounded-copy-to-vec", unbounded_copy_to_vec,
     "`io::copy` verso un Vec da una sorgente non limitata (D-01, CWE-409): il Vec cresce "
     "quanto l'output del decoder. Metti `.take(quota + 1)` sull'OUTPUT della sorgente e "
     "rifiuta oltre la quota; nella fase 3 il lettore limitato unico."),
]

# --------------------------------------------------------------------------
# Scansione
# --------------------------------------------------------------------------


def find_files(path, excluded_dirs):
    if os.path.isfile(path):
        return [path] if file_kind(path) else []
    out = []
    for root, dirs, files in os.walk(path):
        kept = []
        for d in dirs:
            if d in _PRUNED_DIRS:
                continue
            child = os.path.join(root, d)
            if os.path.isfile(os.path.join(child, FIXTURE_MARKER_FILENAME)):
                excluded_dirs.append(child)
                continue
            kept.append(d)
        dirs[:] = kept
        out.extend(os.path.join(root, f) for f in files if file_kind(os.path.join(root, f)))
    return out


def is_test_file(path):
    n = path.replace("\\", "/").lower()
    base = n.rsplit("/", 1)[-1]
    return ("/tests/" in n or "/__tests__/" in n or "/e2e/" in n or n.startswith("tests/")
            or base.endswith(("_test.rs", "_tests.rs"))
            or re.search(r"\.(test|spec)\.[jt]sx?$", base) is not None)


def is_comment(line, kind):
    s = line.strip()
    if kind in (CFG, SCRIPT, SQL):
        return s.startswith(("#", "--", "//", ";"))
    if kind == DOC:
        return False
    return s.startswith(("//", "/*", "*", "#!"))


_LOG_OPEN = re.compile(_LOG[:-len(r"""[^;]*?""")])
_LOG_JOIN_CAP = 60


def _paren_balance(text):
    depth = 0
    for ch in text:
        if ch == "(":
            depth += 1
        elif ch == ")":
            depth -= 1
            if depth == 0:
                return 0
    return depth


def _multiline_log_findings(lines, i, bare, kind, findings, path):
    """D-05: una macro di log che si apre sulla riga `i` (da 1) e non si chiude
    li' si valuta sul testo unito, a stringhe e char literal svuotati, fino alla
    parentesi bilanciata (al piu' `_LOG_JOIN_CAP` righe: oltre il tetto vale il
    testo unito fin li'). Il finding va sulla riga d'apertura, senza doppioni."""
    opening = strip_strings_and_chars(bare)
    m = _LOG_OPEN.search(opening)
    if not m:
        return
    depth = _paren_balance(opening[m.end() - 1:])
    if depth <= 0:
        return
    joined = opening
    k = i
    while depth > 0 and k < len(lines) and k - i < _LOG_JOIN_CAP:
        nxt = strip_strings_and_chars(strip_strings(lines[k]))
        joined += " " + nxt
        depth = _paren_balance(joined[m.end() - 1:])
        k += 1
    already = {rid for (_, rid, _, ln, _) in findings if ln == i}
    for sev, rid, rx, msg, kinds in RULES:
        if rid not in _STRIP_STRING_RULES or kind not in kinds or rid in already:
            continue
        if rx.search(joined):
            findings.append((sev, rid, path, i, msg))


def scan_file(path):
    findings = []
    try:
        with open(path, encoding="utf-8", errors="replace") as fh:
            text = fh.read()
    except OSError:
        return findings
    kind = file_kind(path)
    lines = text.splitlines()
    test_file = is_test_file(path)
    for i, line in enumerate(lines, 1):
        comment = is_comment(line, kind)
        bare = strip_strings(line)
        for sev, rid, rx, msg, kinds in RULES:
            if kind not in kinds:
                continue
            if test_file and rid in _TEST_SKIPPED_RULES:
                continue
            if comment and rid not in _COMMENT_SCANNED_RULES:
                continue
            for m in rx.finditer(bare if rid in _STRIP_STRING_RULES else line):
                if "val" in rx.groupindex and is_placeholder(m.group("val")):
                    continue
                findings.append((sev, rid, path, i, msg))
                break
        if kind in CODE and not comment:
            _multiline_log_findings(lines, i, bare, kind, findings, path)
        if _BURNED_RE is not None and _BURNED_RE.search(line):
            findings.append(("CRITICAL", "burned-secret", path, i, _BURNED_MSG))
    if kind in CODE:
        for sev, rid, check, msg in FILE_RULES:
            if test_file and rid in _TEST_SKIPPED_RULES:
                continue
            for ln in check(lines, path):
                findings.append((sev, rid, path, ln, msg))
    return findings


def collect_files(paths, excluded_dirs):
    seen, out = set(), []
    for p in paths:
        for f in find_files(p, excluded_dirs):
            k = os.path.normcase(os.path.abspath(f))
            if k not in seen:
                seen.add(k)
                out.append(f)
    return out


SEV_ORDER = {"CRITICAL": 0, "HIGH": 1, "MEDIUM": 2}


def main():
    ap = argparse.ArgumentParser(prog="security_scan.py")
    ap.add_argument("path", nargs="+")
    ap.add_argument("--json", action="store_true")
    ap.add_argument("--fail-on", choices=list(SEV_ORDER), default="MEDIUM",
                    help="severita' minima che fa fallire (default MEDIUM)")
    args = ap.parse_args()
    soglia = SEV_ORDER[args.fail_on]

    missing = [p for p in args.path if not os.path.exists(p)]
    excluded_dirs = []
    files = collect_files([p for p in args.path if os.path.exists(p)], excluded_dirs)

    if not files:
        msg = ("NON ESEGUITO [security-scope-empty] nessun file scansionabile nello scope"
               + (f"; percorsi inesistenti: {missing}" if missing else "")
               + (f"; escluse per marcatore: {excluded_dirs}" if excluded_dirs else "")
               + ". Uno scope vuoto non e' un verde.")
        if args.json:
            print(json.dumps({"error": "security-scope-empty", "message": msg,
                              "status": "not-run", "excluded_dirs": excluded_dirs},
                             indent=2, ensure_ascii=False))
        else:
            print(msg, file=sys.stderr)
        return EXIT_NOT_RUN

    findings = []
    for f in files:
        findings.extend(scan_file(f))
    findings = [(s, r, f.replace("\\", "/"), l, m) for (s, r, f, l, m) in findings]
    # Severita' sconosciuta => bloccante (fail-closed).
    def blocca(sev):
        return SEV_ORDER.get(sev, -1) <= soglia
    bloccanti = [x for x in findings if blocca(x[0])]
    esito = EXIT_FINDINGS if bloccanti else EXIT_OK

    if args.json:
        print(json.dumps({
            "findings": [dict(severity=s, rule=r, file=f, line=l, message=m,
                              blocking=blocca(s))
                         for (s, r, f, l, m) in findings],
            "fail_on": args.fail_on,
            "blocking_count": len(bloccanti),
            "warning_count": len(findings) - len(bloccanti),
            "excluded_dirs": excluded_dirs,
            "missing_paths": missing,
            "scanned_files": len(files),
        }, indent=2, ensure_ascii=False))
        return esito

    if excluded_dirs:
        print(f"Escluse per marcatore {FIXTURE_MARKER_FILENAME} ({len(excluded_dirs)}): "
              + ", ".join(excluded_dirs))
    else:
        print(f"Nessuna directory esclusa per marcatore {FIXTURE_MARKER_FILENAME}.")
    for p in missing:
        print(f"[INESISTENTE] {p}")
    if not findings:
        print(f"OK - nessun problema di sicurezza rilevato su {len(files)} file.")
        return EXIT_OK
    findings.sort(key=lambda x: (SEV_ORDER.get(x[0], 9), x[2], x[3]))
    counts = {}
    for s, *_ in findings:
        counts[s] = counts.get(s, 0) + 1
    print(f"== Security scan: {len(findings)} rilevamenti (CRITICAL={counts.get('CRITICAL', 0)}, "
          f"HIGH={counts.get('HIGH', 0)}, MEDIUM={counts.get('MEDIUM', 0)}) su {len(files)} file ==\n")
    for sev, rid, f, l, m in findings:
        tag = sev if blocca(sev) else f"{sev} - AVVISO"
        print(f"[{tag}] {rid}  {f}:{l}\n        {m}")
    sopra = [k for k, v in SEV_ORDER.items() if v <= soglia]
    print(f"\nRegola: soglia --fail-on {args.fail_on}: qualsiasi {' o '.join(sopra)} => FAIL.")
    if len(findings) > len(bloccanti):
        print(f"AVVISO: {len(findings) - len(bloccanti)} rilevamenti sotto la soglia "
              f"{args.fail_on} non bloccano; vanno giustificati nel SUMMARY del piano.")
    if bloccanti:
        print(f"FAIL: {len(bloccanti)} rilevamenti bloccanti. Risolvi prima di consegnare.")
    else:
        print("PASS con avvisi: nessun rilevamento bloccante.")
    return esito


if __name__ == "__main__":
    sys.exit(main())
