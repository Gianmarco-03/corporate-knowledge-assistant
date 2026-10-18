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

Esito:
    0  nessun rilevamento
    1  almeno un rilevamento (qualsiasi CRITICAL => FAIL del gate)
    2  non eseguito: nessun file scansionabile nello scope (percorsi
       inesistenti o scope vuoto). Uno scope vuoto non e' mai verde.

Nessuna soppressione e' silenziosa: una directory FIGLIA con il marcatore
FIXTURE_MARKER_FILENAME e' esclusa e il suo percorso e' sempre riportato.
Una directory passata esplicitamente come radice e' sempre scansionata.
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
    ("HIGH", "upload-in-memory",
     re.compile(r"""\.bytes\(\)\s*\.await|to_bytes\s*\([^)]*usize::MAX|\.read_to_end\s*\("""),
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


def axum_handlers_without_authz(lines, path):
    """Deny by default (ASVS V4): ogni handler Axum (fn async con estrattori
    Axum) porta una decisione di autorizzazione: un estrattore di identita'
    nella firma, oppure un commento sentinella MOTIVATO subito sopra:
    `// security-scanner: public-endpoint - <motivazione>` (login, JWKS,
    health) o `// security-scanner: authz-layer - <dove sta il layer>`.
    Una rinuncia senza motivazione e' essa stessa una violazione."""
    if not path.lower().endswith(".rs"):
        return []
    out = []
    i = 0
    while i < len(lines):
        if not _HANDLER_START.match(lines[i]):
            i += 1
            continue
        sig, j = lines[i], i
        while j + 1 < len(lines) and "{" not in lines[j] and j - i < 25:
            j += 1
            sig += lines[j]
        if _AXUM_EXTRACTOR.search(sig) and not _AUTH_EXTRACTOR.search(sig):
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
    args = ap.parse_args()

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

    if args.json:
        print(json.dumps({
            "findings": [dict(severity=s, rule=r, file=f, line=l, message=m)
                         for (s, r, f, l, m) in findings],
            "excluded_dirs": excluded_dirs,
            "missing_paths": missing,
            "scanned_files": len(files),
        }, indent=2, ensure_ascii=False))
        return EXIT_FINDINGS if findings else EXIT_OK

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
        print(f"[{sev}] {rid}  {f}:{l}\n        {m}")
    print("\nRegola: qualsiasi CRITICAL => FAIL. Risolvi prima di consegnare.")
    return EXIT_FINDINGS


if __name__ == "__main__":
    sys.exit(main())
