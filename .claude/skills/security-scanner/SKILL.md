---
name: security-scanner
description: Analizza piani e codice di CKA per vulnerabilità di sicurezza — segreti (TYPESAFE_API_KEY, DATABASE_URL, chiavi JWT) nel codice o nei file versionati, SQL injection, TLS disabilitato, JWT non verificati o simmetrici, Argon2 diverso da Argon2id, fughe di chunk/prompt/contenuti verso log, telemetria, cache, JEV o LLM (regola no-leak), dati personali nei log, token nello storage del browser, handler Axum senza decisione di autorizzazione, upload (dimensione, tipo, path traversal, zip bomb, PDF non fidati), SSRF sulle fonti online, prompt injection e output LLM non sanificato. USARE come gate sui PLAN in `plan`, prima di consegnare codice che tocchi auth, RBAC, retrieval, ingestion, upload, rete, segreti, log o telemetria, prima di una PR, o quando l'utente chiede "controlla la sicurezza", "security review", "è sicuro?", "scan vulnerabilità". Tarato su OWASP ASVS L1, OWASP Top 10 for LLM Applications, RFC 7519/8725 e GDPR.
---

# Security Scanner — CKA

CKA indicizza documenti finanziari e societari riservati e risponde con
citazioni. Il rischio principale non è un exploit esotico: è un chunk non
autorizzato che esce dal perimetro (verso il reranker, JEV, l'LLM, la cache,
un log o uno span di telemetria). Le regole nascono dagli **standard** e
dalle **decisioni di progetto**, e ognuna porta la sua motivazione.

Riferimenti: OWASP ASVS 4.0 livello 1 (blocco su `high`); OWASP Top 10 for
LLM Applications (LLM01 prompt injection, LLM02 sensitive information
disclosure, LLM05 improper output handling, LLM06 excessive agency); JWT RFC
7519 e RFC 8725 (best practice); GDPR art. 5 par. 1 lett. c e art. 32 per i
dati personali; `CLAUDE.md` (deny by default, no-leak, JWT asimmetrici,
Argon2id) e `docs/security/threat-model.md`.

## Sul PLAN (gate in `plan`)

Per ogni piano si leggono frontmatter, `<tasks>` e il blocco `threat_model`
(`node scripts/estrai.cjs blocco <plan> threat_model`). Il piano è
`conforme` se ogni superficie sensibile che introduce ha una decisione
scritta: quale ruolo autorizza ogni rotta nuova (e dove si ricontrolla prima
di citazioni, link e viewer); dove passano chunk e prompt e cosa
garantisce che nessun chunk non autorizzato li raggiunga; quali segreti usa
e da dove li legge; limiti di upload e validazione per magic bytes; allow-list
e blocco degli IP privati per le fonti online; cosa succede al timeout JEV
(rifiuto o degrado, mai bypass). Una superficie senza decisione è
`da correggere`.

## A codice scritto

### 1. Scansione automatica

```bash
python skills/security-scanner/security_scan.py <percorso> [<percorso> ...] [--json]
```

Lo script vive solo in `skills/security-scanner/` (in `.claude/skills/` c'è
solo questo markdown); su questa postazione si può lanciare anche con
`sh scripts/py.sh`. Si passano i file toccati, oppure la directory di uno
snapshot (indice o albero in uscita, come fanno gli hook), mai l'antenato
comune di due aree del repo.

File analizzati: `.rs`; `.ts .tsx .js .jsx .mjs .cjs`; configurazione
(`.toml .yaml .yml .json .env .env.* .conf .ini`, `Dockerfile*`, `compose*`);
script (`.sh .bash .py .ps1`); `.md`; `.sql`. Ogni regola dichiara a quali
famiglie si applica: su markdown, script e SQL valgono le regole di segreto
(e TLS per gli script), non quelle di comportamento runtime. Directory
saltate: `target`, `node_modules`, `.git`, `dist`, `build`, `coverage`.

**Esiti**: `0` nessun rilevamento, `1` almeno un rilevamento, `2` non
eseguito — nessun file scansionabile nello scope (percorsi inesistenti o
scope vuoto). Uno scope vuoto non è mai verde. In `--json` lo stdout è
`{findings, excluded_dirs, missing_paths, scanned_files}`, oppure
`{error, message, status: "not-run"}`.

### 2. Le regole e la loro motivazione

**Segreti (CRITICAL; ASVS V14).** `hardcoded-password`, `hardcoded-apikey`,
`rust-const-secret`, `env-secret-literal` (`TYPESAFE_API_KEY`, `JWT_*KEY*`,
`*_API_KEY`, `*_SECRET`, `*_PASSWORD` con valore letterale in env, Compose,
script o documenti), `connstring-inline-pwd` (password dentro
`DATABASE_URL` e simili), `private-key-block` (chiavi di firma JWT),
`sql-role-password`; `jwt-literal` (HIGH). Nei file versionati solo
segnaposto: `${VAR}`, `<...>`, `changeme`, `example…`, `your-…`. La lista dei
segnaposto è chiusa e non si allarga per far tacere un finding.
`BURNED_SECRETS` è vuota per costruzione (repo greenfield) e si ripopola solo
dopo un incidente reale, insieme alla rotazione; una lista vuota lascia la
regex a `None`, mai `re.compile("")`, che corrisponderebbe a ogni riga.

**Iniezione SQL (CRITICAL; ASVS V5).** `sql-format`, `sql-query-dynamic`,
`sql-template`, `sql-concat`: mai SQL composto con `format!`, concatenazione
o template literal; sqlx con `bind` o macro `query!`.

**TLS (ASVS V9).** `tls-bypass` (CRITICAL: `danger_accept_invalid_certs`,
verificatori custom, `NODE_TLS_REJECT_UNAUTHORIZED=0`, `curl -k`) e `http-url`
(HIGH) verso host instradabili; il loopback è escluso perché il traffico non
lascia la macchina.

**JWT (RFC 7519 / RFC 8725).** `jwt-alg-none` e `jwt-no-verify` (CRITICAL:
firma, `exp`, `aud` sempre verificati, algoritmo atteso fissato);
`jwt-symmetric` (HIGH: CKA firma in modo asimmetrico e pubblica il JWKS);
`jwt-decode-unverified` (MEDIUM, frontend: decodifica solo per UX).

**Password e crittografia (ASVS V2, V6).** `argon2-variant` (Argon2id è la
decisione), `weak-crypto`, `plaintext-pwd-compare` (HIGH), `insecure-random`
(MEDIUM).

**No-leak e log (OWASP LLM02, GDPR).** `no-leak-in-log` e `no-leak-telemetry`
(CRITICAL): nessun chunk, prompt, completion, testo estratto o embedding in
log o attributi di span; si loggano id e conteggi. `instrument-without-skip`
(CRITICAL, regola di file): `#[instrument]` registra tutti gli argomenti, e su
una fn che riceve chunk, prompt, query o credenziali serve `skip`/`skip_all`.
`pii-in-log` (CRITICAL): email, telefono, indirizzo, data di nascita,
nominativo, IBAN; si logga un id pseudonimo. `log-secret` (HIGH): token,
header `Authorization`, cookie. Queste regole guardano gli identificatori
loggati, non il testo del messaggio (le stringhe sono svuotate prima del
confronto). La cache non ha una regola automatica: in review si verifica che
la chiave includa identità, versione della policy e versioni dei documenti.

**Autorizzazione (ASVS V4, deny by default).**
`endpoint-without-authz-decision` (CRITICAL, regola di file): ogni handler
Axum (fn `async` con estrattori `Json`, `Path`, `Query`, `State`,
`Multipart`, …) porta un estrattore di identità (`AuthenticatedUser`,
`Claims`, `Subject`, `ServiceCaller`, …) oppure, subito sopra, un commento
sentinella **motivato**: `// security-scanner: public-endpoint - <motivazione>`
(login, refresh, JWKS, health) o `// security-scanner: authz-layer - <dove sta
il layer>`. Una rinuncia senza motivazione è essa stessa una violazione. La
regola non verifica *quale* ruolo: quello lo giudica la review, insieme al
ricontrollo prima di citazioni, link e viewer.

**Browser (ASVS V8, LLM05).** `credentials-in-webstorage` (CRITICAL: access
token solo in memoria, refresh token in cookie `HttpOnly`/`Secure`/`SameSite`),
`token-in-js-cookie`, `html-injection` (risposte LLM e contenuti dei
documenti sono input non fidati), `creds-in-querystring`, `cors-permissive` (HIGH).

**Upload e parsing (ASVS V12).** `upload-body-limit-disabled` (CRITICAL),
`upload-in-memory` e `upload-path-traversal` (HIGH), `upload-mime-trust`,
`archive-decompress` (zip bomb: DOCX è uno zip) e `pdf-untrusted-parse`
(MEDIUM: processo isolato o `spawn_blocking` con timeout e limiti).

**SSRF (fonti online).** `ssrf-dynamic-url` (HIGH): allow-list di schema e
host, blocco di IP privati, loopback, link-local e metadata dopo la
risoluzione DNS, redirect limitati e riverificati, timeout e tetto di
dimensione; la discovery passa da quarantena e approvazione.

**LLM (LLM01, LLM06).** `prompt-interpolation` (MEDIUM): istruzioni e
contenuti non fidati in ruoli separati, chunk delimitati come dati.
`dynamic-exec` (HIGH): nessun comando o codice dinamico, mai su output LLM.

### 3. Esclusioni — mai silenziose

1. **Marcatore di directory** `.security-scan-fixture` (con la motivazione
   dentro): esclude una directory **figlia** incontrata nella camminata; una
   directory passata esplicitamente come radice si scansiona sempre. Ogni
   esclusa è riportata in output e in `excluded_dirs`.
2. **Regole saltate nei file di test** (`tests/`, `__tests__/`, `e2e/`,
   `*_test.rs`, `*.test.ts(x)`, `*.spec.ts(x)`): solo `http-url`,
   `jwt-literal`, `insecure-random`.
3. **Righe commentate**: restano attive le sole regole di segreto, perché un
   segreto commentato è committato lo stesso.

Non esistono altre soppressioni. Una regola non si restringe mai per far
tacere un finding scomodo: un falso positivo si risolve correggendo il codice
o con la riga giudicata legittima, motivata, nel report di sicurezza.

### 4. Verdetto e report

Lo script trova, l'agente giudica. Si elencano CRITICAL, HIGH, MEDIUM con
`file:riga`, spiegazione e fix. **Qualsiasi CRITICAL ⇒ FAIL** e blocca la
consegna. HIGH e MEDIUM si chiudono con una correzione o con una
motivazione scritta. Segreti toccati, decisioni prese, vulnerabilità trovate
e risolte, esito del gate e rischi residui vanno nello stesso giro nel report
di sicurezza della fase (`.planning/phases/<fase>/<fase>-SECURITY.md`, quello
di `gsd-secure-phase`).

## Estendere lo scanner

Una regola nuova dichiara le famiglie di file a cui si applica, ha un
esempio che deve scattare e uno che non deve (fixture sotto una directory con
il marcatore), e si conferma con una scansione reale, non solo a occhio: una
regola su un'estensione che lo scanner non vede è una regola morta.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
