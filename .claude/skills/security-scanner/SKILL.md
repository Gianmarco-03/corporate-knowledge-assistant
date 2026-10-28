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
python skills/security-scanner/security_scan.py <percorso> [<percorso> ...] [--json]     [--fail-on {MEDIUM,HIGH,CRITICAL}]
```

`--fail-on` fissa la soglia di blocco: l'esito è `1` solo se c'è almeno un
rilevamento di severità pari o superiore. I rilevamenti sotto soglia si
stampano comunque, marcati `AVVISO`, con una riga che dice che non bloccano;
l'ultima riga dichiara la soglia effettiva. Il default è `MEDIUM`, e lo usano
i gate di `plan` e `verify`. L'hook `pre-commit` (e `pre-push`, che passa
dalla stessa `passo_sicurezza` di `.githooks/controlli-comuni.sh`) lancia lo
scanner con `--fail-on HIGH`.

Lo script vive solo in `skills/security-scanner/` (in `.claude/skills/` c'è
solo questo markdown); su questa postazione si può lanciare anche con
`sh scripts/py.sh`. Si passano i file toccati, oppure la directory di uno
snapshot (indice o albero in uscita, come fanno gli hook), mai l'antenato
comune di due aree del repo.

File analizzati: `.rs`; `.ts .tsx .js .jsx .mjs .cjs`; configurazione
(`.toml .yaml .yml .json .env .env.* .conf .ini`, `Dockerfile*`, `compose*`);
script (`.sh .bash .py .ps1`); `.md`; `.sql`; e la famiglia testuale
(gap-closure 1.6, D-03): `.txt .csv .tsv .jsonl .ndjson .log .pem .key .crt` e
i file senza estensione, compresi gli hook (`pre-commit`) e i dotfile come
`.gitignore`. Ogni regola dichiara a quali famiglie si applica: su markdown,
script e SQL valgono le regole di segreto (e TLS per gli script), non quelle
di comportamento runtime; sulla famiglia testuale valgono **solo** le regole
dei segreti (con le regole TLS e HTTP un CSV di evidenza darebbe falsi
allarmi), e una riga che inizia con `#` o `//` non vale come commento. I
binari con estensione (`.pdf`, `.docx`, `.png`, …) restano fuori. Directory
saltate: solo `target`, `node_modules`, `.git`. Un file che non si apre o non
si legge dà `unreadable-file` (CRITICAL, riga 0): un file non letto non è un
file pulito.

**Esiti**: `0` nessun rilevamento o solo rilevamenti sotto soglia, `1`
almeno un rilevamento pari o sopra la soglia, `2` non eseguito, mai `0`
(gap-closure 1.6, D-07 e D-04):

- nessun percorso passato: `NON ESEGUITO [security-no-paths]`;
- un percorso inesistente fra quelli passati, anche uno solo mescolato a
  percorsi esistenti: `NON ESEGUITO [security-missing-paths]`, perché un
  refuso nell'elenco di un gate darebbe un verde parziale;
- scope vuoto, nessun file scansionabile: `security-scope-empty`;
- allowlist del marcatore assente o illeggibile, JSON non valido, voce senza
  `path` o `motivo`, `path` assoluto o con `..`:
  `NON ESEGUITO [fixture-allowlist-unreadable]`.

Uno scope vuoto non è mai verde. Il riepilogo testuale stampa sempre
`Escluse per marcatore (<n>)`, anche con zero. In `--json` lo stdout è
`{findings, fail_on, blocking_count, warning_count, excluded_dirs,
excluded_count, missing_paths, scanned_files}`, dove ogni finding porta
`blocking`, oppure, in ognuno dei casi `2`, `{error, message, status:
"not-run"}` (con `missing_paths` per `security-missing-paths`).

### 2. Le regole e la loro motivazione

**Segreti (CRITICAL; ASVS V14).** `hardcoded-password`, `hardcoded-apikey`
(nomi con `api_key`, `apikey`, `secret`, `token`, `client_secret`,
`private_key`, `access_key`), `rust-const-secret`, `env-secret-literal`,
`connstring-inline-pwd` (password dentro `DATABASE_URL` e simili),
`private-key-block` (chiavi di firma JWT), `sql-role-password`,
`known-key-prefix`; `jwt-literal` (HIGH).

*Nomi (gap-closure 1.6, D-02).* `env-secret-literal` riconosce, ancorato a
inizio riga e sensibile alle maiuscole, `TYPESAFE_API_KEY`, `JWT_*KEY*`,
`JWT_*SECRET*` e ogni nome `\w*(SECRET|PASSWORD|API_KEY|ACCESS_KEY|TOKEN)\w*`
con un valore letterale in env, Compose, script, documenti e file testuali:
quindi anche `GARAGE_SECRET_KEY`, `*_ACCESS_KEY` e `*_TOKEN`.

*Prefissi noti (D-02).* `known-key-prefix` segnala in qualsiasi file dei
segreti, commenti e file di test compresi, `sk-` o `sk-ant-` seguiti da almeno
20 caratteri `[A-Za-z0-9_-]`, con un confine a sinistra (nessuna lettera,
cifra, `_` o `-` prima: `syn-05-risk-…` non scatta), e le chiavi Garage `GK`
seguite da esattamente 24 cifre esadecimali (maiuscole o minuscole) con
confine di parola su entrambi i lati (23 o 25 cifre non scattano). Un
`sk-test-{SENTINEL}` composto a runtime non scatta.

*Segnaposto (D-02).* Nei file versionati solo segnaposto. La lista è chiusa e
non si allarga per far tacere un finding; così com'è nel codice
(`_PLACEHOLDER`, senza distinzione di maiuscole), un valore è un segnaposto se
è vuoto oppure è interamente uno di questi: `<…>`, `${…}`, `$NOME`, `{{…}}`,
`%NOME%`, una serie di `*`, almeno tre `x`, `...`, `changeme`, `change-me`,
`change_me`, `replaceme`, `replace-me`, `replace_me`, `example…` (lettere,
cifre e `_` dopo `example`), `placeholder`, `dummy`, `your-…`, `your_…`,
`redacted`, `todo`, `none`, `null`, `test`, `fake…`. `password` e `secret`
non sono segnaposto. Un valore uguale al nome della variabile, o a una delle
sue parole separate da `_` o `-`, non è mai un segnaposto: il confronto si fa
sul valore grezzo, quindi `${SERVICE_TOKEN}` e `$SERVICE_TOKEN` restano
segnaposto, mentre `token` o il nome intero ripetuto sotto `SERVICE_TOKEN`
fanno scattare il finding.

*Varco dichiarato (T-01.6-02-08).* Un nome di segreto minuscolo senza
virgolette in un file env non scatta: i nomi delle variabili d'ambiente sono
maiuscoli per convenzione e `env-secret-literal` è sensibile alle maiuscole;
con le virgolette lo copre `hardcoded-apikey`, che non lo è.
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
confronto). Una macro di log (`tracing::*!`, `log::*!`, `println!`,
`eprintln!`, `dbg!`, `panic!`, `console.*`) che rustfmt spezza su più righe si
valuta sul testo unito fino alla parentesi di chiusura bilanciata (al più 60
righe; oltre il tetto vale il testo unito fin lì, mai la sola riga
d'apertura): il bilanciamento conta le parentesi dopo aver svuotato stringhe e
char literal (`')'`), e il finding va sulla riga d'apertura, senza doppioni
(gap-closure 1.6, D-05). La cache non ha una regola automatica: in review si verifica che
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
ricontrollo prima di citazioni, link e viewer. Sono handler anche senza
argomenti (gap-closure 1.6, D-06): gli `async fn` il cui nome è registrato
nello stesso file in `.route(<path>, <metodo>(<nome>))`, comprese le catene
del `MethodRouter` (`get(a).post(b)`), `on(<filtro>, <nome>)` e i nomi
qualificati (`handlers::upload`), cercati sul testo con le righe unite; e ogni
`async fn` pubblico di un modulo di handler (`handler.rs`, `handlers.rs`,
cartella `handlers/`). Un `.get(<nome>)` fuori dal secondo argomento di
`.route(` (`cache.get(key)`) non è una rotta, e una closure
(`get(|| async { "ok" })`) non è un nome. Il waiver motivato resta l'unica
eccezione; senza motivo è una violazione anche sopra un handler registrato.

**Browser (ASVS V8, LLM05).** `credentials-in-webstorage` (CRITICAL: access
token solo in memoria, refresh token in cookie `HttpOnly`/`Secure`/`SameSite`),
`token-in-js-cookie`, `html-injection` (risposte LLM e contenuti dei
documenti sono input non fidati), `creds-in-querystring`, `cors-permissive` (HIGH).

**Upload e parsing (ASVS V12).** `upload-body-limit-disabled` (CRITICAL),
`upload-in-memory` e `upload-path-traversal` (HIGH), `upload-mime-trust`,
`archive-decompress` (zip bomb: DOCX è uno zip) e `pdf-untrusted-parse`
(MEDIUM: processo isolato o `spawn_blocking` con timeout e limiti).

*Letture fino all'EOF (gap-closure 1.6, D-01; CWE-409, ASVS 5.0 V5.2.3).*
`.bytes().await` e `to_bytes(.., usize::MAX)` restano regole di riga
`upload-in-memory` (HIGH). `read_to_end`, `read_until` (anche con `.await`), la
forma UFCS (`Read::read_to_end(&mut r, ..)`, `AsyncReadExt::…`) e `io::copy`
(`std::io`, `tokio::io`) sono invece regole di file con un classificatore
comune, su testo a commenti e stringhe svuotati:

- **il ricevitore** è l'espressione prima della chiamata, risalendo fino al
  delimitatore dell'istruzione (`;`, `{`, `}`, `,`, `=`, `=>`, `|` di una
  closure, una parentesi aperta che la contiene) e togliendo `if`, `match`,
  `while`, `return`. Così anche una lettura senza `;` di chiusura (condizione
  di `if`, scrutinio di `match`, `while let`, corpo di una closure) ha sempre
  un esito;
- **conta come limite solo `.take(<argomento>)`**: `.take()` vuoto è
  `Option::take` e non limita nulla;
- **con un decoder** (`<Nome>::new(` con `<Nome>` che finisce in `Decoder` o
  `Decompressor`) il `take` deve stare *dopo* la parentesi di chiusura del
  costruttore più esterno, cioè sull'output decompresso:
  `ZlibDecoder::new(src).take(N)` è limitato, `ZlibDecoder::new(src.take(N))`
  limita solo i byte compressi e dà `upload-in-memory` (HIGH), come
  `GzDecoder::new(ZlibDecoder::new(src).take(N))`;
- **senza decoder**, un `.take(` sulla catena limita i byte letti
  (`r.take(MAX + 1)`, `(&mut stdout).take(..)`, `BufReader::new(f.take(N))`);
  senza `take`, una catena con una chiamata (`File::open(p)?`,
  `BufReader::new(f)`) è `upload-in-memory` (HIGH), un identificatore nudo è
  `upload-in-memory-unverified` (MEDIUM: reader costruito altrove), come la
  forma UFCS senza `take` nel primo argomento;
- **su più righe** (ricevitore o argomenti della chiamata a capo) l'esito è
  `upload-in-memory-unverified` solo se c'è un `.take(<argomento>)`, perché la
  sua posizione rispetto al decoder non si determina; senza alcun `take` si
  classifica come una riga sola, quindi un decoder senza limite resta HIGH. I
  combinatori in coda (`.map_err(..)`, `?`, `.await`, `.unwrap()`) non contano;
- **`io::copy(&mut <src>, <dst>)`**: con una sorgente limitata nessun
  finding; con una sorgente non limitata, `sink()`, `stdout()` e `stderr()`
  non accumulano e non danno finding; un `Vec` dichiarato nel file
  (`let [mut] v[: Vec<..>] = Vec::new()`, `vec![..]`, `Vec::with_capacity(..)`)
  dà `unbounded-copy-to-vec` (HIGH), o `upload-in-memory-unverified` se la
  sorgente è un identificatore nudo; ogni altra destinazione (turbofish
  `Vec::<u8>::new()`, parametro `&mut Vec<u8>`, `Cursor`, identificatore non
  dichiarato) dà `upload-in-memory-unverified`, mai silenzio.

`upload-in-memory-unverified` è MEDIUM di proposito: nell'hook (soglia HIGH) è
un avviso, nei gate di `plan` e `verify` (soglia MEDIUM) blocca e va motivato
nel SUMMARY. `upload-in-memory-capacity-unverified` (MEDIUM) segnala la
capacità del buffer presa da un'espressione non costante:
`Vec::with_capacity(<arg>)`, `String::with_capacity(..)`,
`BytesMut::with_capacity(..)`, anche in forma turbofish, con un argomento che
non è un letterale intero né un'espressione di soli letterali e costanti in
maiuscolo (`MAX_LEN`, `4 * MIB`). Ha un id proprio perché i gate separano
questi MEDIUM dagli altri filtrando sul campo `rule`, mai sul testo del
`message`. Il rimedio delle tre regole è lo stesso: `.take(quota + 1)`
sull'output, rifiuto oltre la quota, e dalla fase 3 il lettore limitato
unico.

*Varchi dichiarati.* La regola è lessicale e non vede: `read_to_string`,
`fs::read` e i cicli di `read` manuali; un `copy` importato nudo (senza
`io::`); un reader passato fra funzioni (il `take` sta nel chiamante, lo
scanner vede solo un identificatore nudo e dice «da verificare»); un handler
registrato in un altro file. Li chiude la fase 3 (D-19 di `03-CONTEXT.md`:
lettore limitato unico e `disallowed-methods` di clippy; CodeQL
`rust/uncontrolled-allocation-size` in `01-DEEPSEARCH-7.md`); fino ad allora
la garanzia sostanziale resta dei test ostili e della review.

*Storia.* Fino alla gap-closure 1.6 `upload-in-memory` conteneva anche
l'alternativa di riga `\.read_to_end\s*\(`, che scattava su ogni
`read_to_end` senza guardare il `take`. Premiava chi evitava la parola: il
prescan della fase 1.5 usava `io::copy` al posto di `take` + `read_to_end` per
non far scattare la regola, e la bomba con il `take` sul lato sbagliato
(`ZlibDecoder::new(src.take(N))`) passava per una lettura qualsiasi. La banda
ECC di /verify 1 (`01-REVIEW.md` §B.1, D-41, `deepsearch_7`) ha chiesto una
regola sensibile alla posizione del `take`: è la classificazione sopra, con
il «da verificare» al posto del silenzio sulle forme che una regola lessicale
non sa distinguere.

**SSRF (fonti online).** `ssrf-dynamic-url` (HIGH): allow-list di schema e
host, blocco di IP privati, loopback, link-local e metadata dopo la
risoluzione DNS, redirect limitati e riverificati, timeout e tetto di
dimensione; la discovery passa da quarantena e approvazione.

**LLM (LLM01, LLM06).** `prompt-interpolation` (MEDIUM): istruzioni e
contenuti non fidati in ruoli separati, chunk delimitati come dati.
`dynamic-exec` (HIGH): nessun comando o codice dinamico, mai su output LLM.

### 3. Esclusioni — mai silenziose

1. **Marcatore di directory** `.security-scan-fixture` (con la motivazione
   dentro), solo in allowlist (gap-closure 1.6, D-04): esclude una directory
   **figlia** incontrata nella camminata solo se il suo percorso del progetto
   è una voce di `skills/security-scanner/fixture-allowlist.json` (array di
   `{ "path", "motivo" }`, letto accanto allo script; oggi la sola
   `spikes/data/hostile`, le fixture ostili di S-02). Il percorso del progetto
   è relativo alla radice del repo (due livelli sopra la cartella dello
   script) quando la radice passata sta dentro il repo, altrimenti alla radice
   passata, con separatore `/`; mai alla cartella corrente. Il confronto è
   esatto, mai per prefisso: `spikes/data/hostile-x` non è in allowlist, e una
   copia annidata (`altro/spikes/data/hostile`) resta fuori da qualunque
   cartella si lanci. Un'istantanea degli hook (radice temporanea fuori dal
   repo) dà lo stesso `spikes/data/hostile`; un'istantanea passata da una sua
   sottocartella dà un percorso più corto e quindi il finding: è il lato
   sicuro. Un marcatore fuori allowlist dà `fixture-marker-not-allowlisted`
   (CRITICAL) e la directory si scansiona lo stesso. Una directory passata
   esplicitamente come radice si scansiona sempre. Ogni esclusa è riportata
   in output, in `excluded_dirs` e nel conteggio `excluded_count`. Aggiungere
   una voce all'allowlist è una modifica versionata, con il motivo, da
   giudicare in review.
2. **Regole saltate nei file di test** (`tests/`, `__tests__/`, `e2e/`,
   `*_test.rs`, `*.test.ts(x)`, `*.spec.ts(x)`): solo `http-url`,
   `jwt-literal`, `insecure-random`.
3. **Righe commentate**: restano attive le sole regole di segreto, perché un
   segreto commentato è committato lo stesso. Sulla famiglia testuale non
   esistono righe commentate.
4. **Contatori numerici (D-02).** Per la sola regola `env-secret-literal`, un
   valore di sole cifre (`^\d+$`) non è un segreto solo sotto un nome da
   contatore, deciso in due passi: il nome finisce con il suffisso
   `(_TOKENS|_MAX|_SIZE|_LEN|_LIMIT|_COUNT|_TTL)$` (un `MAX` o un `_SIZE` in
   mezzo al nome non conta), e il resto del nome, tolto il suffisso, non
   contiene un termine da segreto (`SECRET`, `PASSWORD`, `API_KEY`,
   `ACCESS_KEY`, `TOKEN`). Il termine da segreto ha sempre la precedenza:
   `MAX_ADMIN_PASSWORD`, `ADMIN_PASSWORD_MAX` e `API_TOKEN_TTL` con un valore
   di sole cifre restano finding, come ogni altro nome del gruppo. Motivo: un
   tetto di token come `TEI_MAX_BATCH_TOKENS` di `spikes/.env.example` non è
   un segreto. `_TOKENS` è un suffisso da contatore solo al plurale, in coda.
   Le altre regole (`hardcoded-apikey` su un `api_key` fra virgolette di sole
   cifre) non hanno questa esenzione.
5. **Espressioni Python (D-02, precisazione dell'utente del 2026-10-29).**
   Nei file `.py` `env-secret-literal` conta un valore come letterale solo se
   è aperto da una virgoletta (`"` o `'`): un valore nudo è un'espressione del
   linguaggio (tupla, lista, insieme, dizionario, chiamata, identificatore,
   `os.environ.get(...)`). I letterali fra virgolette restano CRITICAL; negli
   altri tipi di file (env, shell, Compose, documenti, testuali) un valore
   nudo resta un letterale.
6. **Numeri con unità di misura (D-02, precisazione dell'utente del
   2026-10-29).** In tutte le regole dei segreti un valore che è interamente
   un numero (intero o decimale) seguito da un'unità di misura non è un
   segreto: durate (`ns`, `us`, `µs`, `ms`, `s`, `sec`, `min`, `h`, `d`),
   dimensioni (`b`, `kb`, `mb`, `gb`, `tb`, `kib`, `mib`, `gib`, `tib`) e
   percentuali (`%`), senza distinzione di maiuscole. L'unità è obbligatoria
   e chiude il valore: un numero seguito da un suffisso che non è un'unità, o
   con una coda alfanumerica dopo l'unità, resta un finding; le sole cifre
   senza unità seguono il punto 4. Motivo: una durata in un log di misura
   (`tokenization_time` con un valore in millisecondi) non è un segreto.

Non esistono altre soppressioni. Una regola non si restringe mai per far
tacere un finding scomodo: un falso positivo si risolve correggendo il codice
o con la riga giudicata legittima, motivata, nel report di sicurezza.

*Storia.* Fino alla gap-closure 1.6 (piano 01.6-02, banda ECC di /verify 1,
`01-REVIEW.md` §B.1) lo scanner potava anche `dist`, `build` e `coverage`, non
leggeva i file testuali (`.txt`, `.csv`, `.jsonl`, `.pem`, gli hook senza
estensione), trattava un file illeggibile come pulito, accettava il marcatore
`.security-scan-fixture` ovunque, contava `password` e `secret` fra i
segnaposto e, senza percorsi, usciva 2 per argparse con un errore su stderr e
senza JSON. `env-secret-literal` conosceva solo `*_API_KEY`, `*_SECRET` e
`*_PASSWORD` in coda al nome, quindi `GARAGE_SECRET_KEY`, `*_ACCESS_KEY` e i
token passavano. Le esclusioni 4, 5 e 6 sono le sole aggiunte: la 4 viene da
D-02 (CONTEXT riformulata il 2026-10-26); la 5 e la 6 le ha decise l'utente
il 2026-10-29, dopo lo stop del primo esecutore di 01.6-02, che sull'albero
intero aveva trovato sei CRITICAL nuovi senza alcun segreto: cinque costanti
Python dello scanner stesso, il cui valore è un'espressione, e una durata in
millisecondi in un log di misura.

### 4. Verdetto e report

Lo script trova, l'agente giudica. Si elencano CRITICAL, HIGH, MEDIUM con
`file:riga`, spiegazione e fix. **Qualsiasi CRITICAL ⇒ FAIL** e blocca la
consegna. HIGH e MEDIUM si chiudono con una correzione o con una
motivazione scritta.

**Regola dell'hook (decisione utente 2026-10-21).** Nel `pre-commit` e nel
`pre-push` bloccano CRITICAL e HIGH. Un MEDIUM è un avviso: non ferma il
commit, ma la sua giustificazione va scritta nel SUMMARY del piano che lo
introduce (regola, `file:riga`, perché è accettabile, quale mitigazione lo
contiene). Un MEDIUM non motivato nel SUMMARY è un difetto del piano, che il
gate di `verify` (soglia di default `MEDIUM`) fa riemergere.

*Storia.* Fino al 2026-10-21 l'hook chiamava lo scanner senza soglia, quindi
bloccava anche i MEDIUM. I parser PDF e ZIP (lopdf, pdfium_render,
ZipArchive, decoder) producono per natura `pdf-untrusted-parse` e
`archive-decompress`, richiesti dai piani 01-07, 01-08, 01-09, 01-13 e 01-18:
con quella regola nessun commit con quel codice passava. La riga finale dello
script diceva «qualsiasi CRITICAL => FAIL», mentre l'esito era 1 per
qualsiasi rilevamento; ora dichiara la soglia effettiva. Segreti toccati, decisioni prese, vulnerabilità trovate
e risolte, esito del gate e rischi residui vanno nello stesso giro nel report
di sicurezza della fase (`.planning/phases/<fase>/<fase>-SECURITY.md`, quello
di `gsd-secure-phase`).

## Estendere lo scanner

Una regola nuova dichiara le famiglie di file a cui si applica, ha un
esempio che deve scattare e uno che non deve (fixture sotto una directory con
il marcatore), e si conferma con una scansione reale, non solo a occhio: una
regola su un'estensione che lo scanner non vede è una regola morta. I test
dello scanner stanno in `skills/security-scanner/tests/test_security_scan.py`
(`unittest` della libreria standard, a subprocess su una cartella
temporanea): `sh scripts/py.sh -m unittest discover -s
skills/security-scanner/tests -p 'test_security_scan.py'`.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
**Modifica:** Claude (Anthropic) · 2026-10-21 · opzione `--fail-on`; l'hook blocca solo CRITICAL e HIGH, i MEDIUM sono avvisi da motivare nel SUMMARY (decisione utente)
**Modifica:** Claude (gsd-executor 01.6-01) · 2026-10-26 · regole upload-in-memory, upload-in-memory-unverified, upload-in-memory-capacity-unverified, unbounded-copy-to-vec; log su più righe; handler senza argomenti (D-01, D-05, D-06)
**Modifica:** Claude (gsd-executor 01.6-02) · 2026-10-29 · segreti, file testuali, allowlist del marcatore, esito 2 senza percorsi (D-02, D-03, D-04, D-07), con le esclusioni decise dall'utente il 2026-10-29 per le espressioni Python e i numeri con unità
