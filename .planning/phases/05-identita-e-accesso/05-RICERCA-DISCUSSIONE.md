# 05 — Ricerca decisionale per il discuss (Identità e accesso)

**Modifica:** Claude (Anthropic) · 2026-10-26 · brief di ricerca del discuss della fase 5 (canale Claude, nessuna ricerca GPT richiesta)

Stile caveman full. Fonti esterne con URL + accesso 2026-10-26. Fonti repo con file:sezione: prevalgono su esterno finché utente non riapre.

## 0. Preflight

- Goal ROADMAP: utente creato da admin entra con email+password; document/rag riconoscono identità e ruoli senza chiamare identity. Req AUTH-01..03. Dipende fasi 2, 4.
- Decisioni da chiudere (ROADMAP): conferma Proposta JWT asimmetrici (se no → rivedere C-02); storage token browser (vincolo security-scanner); forma propagazione identità fra servizi (usata da fase 7).
- Criticità: ROADMAP già «critica (identità e autenticazione)». Regola skill: goal tocca autenticazione → critica. In plan: GPT `implementation-risk-audit` + `test-design`.
- Pipeline sfalsata: fase 1 in verify (gap 1.6 da pianificare), fasi 2/3/4 discusse non pianificate. Rischio: seam Fondazione (hook auth, slot AuditSink, fascia migrazioni) ancora promessi a parole, WIRING-MAP vuota.
- deferred-items.md: nessuno. Punti ereditati veri da CONTEXT/threat model:
  - P1 · 02 D-14: slot verificatore token di servizio su `/internal/v1` → «5 o 7», da fissare in wiring-design.
  - P2 · 03 D-08 + 01-CONTRATTO §6 riga 103 + ADR-0012 §6 e conseguenze: fase identity/RBAC (5, 10) fa crescere `acl_version` del soggetto a ogni cambio ruolo.
  - P3 · ADR-0012 conseguenze: derivazione `AuthorizationScope`/predicato da `Principal` verificato (JWT) → fase identity/RBAC.
  - P4 · threat model T-01, T-02, T-04 test previsti fase 5; T-03 fasi 5 e 9.
  - P5 · 04 D-14: AuditSink PG append-only anche schema `identity`, fascia `04NNNN`; slot AuditSink nel composition root identity (Fondazione).
  - P6 · 02 D-13: Caddy unico origin, `/api/v1/auth/*` e `/api/v1/users/*` → identity. 02 D-09: fascia migrazioni `05NNNN_*.sql`. 02 D-05/D-06: problem+json, 429 rate limit.
- WIRING-MAP: zero righe. Da riempire: nessuna riga esistente. Da consumare (promessi in CONTEXT 02/04, righe nasceranno in wiring-design di plan 02): hook di autenticazione in document e rag (criterio 5 ROADMAP «pre-registrati»), route registry riga fase 5 (02 D-23), fascia migrazioni 05, slot AuditSink identity.

## Area A — Firma JWT (conferma Proposta «JWT asimmetrici»)

Fonte autorevole:
- RFC 8725 §3.1: libreria deve permettere al chiamante di fissare insieme algoritmi; ogni chiave legata a un solo algoritmo. https://www.rfc-editor.org/rfc/rfc8725.html (2026-10-26)
- RFC 8725 §3.9: issuer con più relying party → `aud` MUST, destinatario rifiuta se assente/non sua. §3.11 typing esplicito `typ` RECOMMENDED. §3.12 tipi diversi di JWT → regole mutuamente esclusive (es. chiavi diverse).
- RFC 9068 §2.1 `typ` = `at+jwt`; §2.2 claim richiesti `iss exp aud sub client_id iat jti`; §4 resource server rifiuta `typ` diverso. https://www.rfc-editor.org/rfc/rfc9068.html (2026-10-26). Nota: unico client = web app → `client_id` costante.
- Repo: DEPENDENCIES.md riga «Token | JWT firmati asimmetricamente | Proposta»; architecture.md §Identity («altri servizi verificano localmente firma, issuer, audience e scadenza»); threat model T-02 (alg none, HS↔RS, kid → 401); security-scanner `jwt-symmetric` HIGH, `jwt-alg-none` CRITICAL; .env.example `JWT_ISSUER`, `JWT_AUDIENCE`, `JWT_PUBLIC_KEY`.
Pratica consolidata:
- Asimmetrico = verificatori non possono firmare → document/rag senza segreto condiviso. Simmetrico HS256 = ogni servizio può forgiare → contrario a goal e scanner.
- Algoritmo: EdDSA (Ed25519) firma piccola, deterministica, niente scelta parametri; ES256 più diffuso fra librerie; RS256 chiavi grandi. Crate `jsonwebtoken` 11.0.0 (docs.rs, 2026-10-26): backend `aws_lc_rs` o `rust_crypto` (al più uno), algoritmi HS/RS/PS/ES/EdDSA, JWK/JwkSet. https://docs.rs/crate/jsonwebtoken/latest — supporto EdDSA da fonte secondaria, verificare in plan con context7/sorgente.
- Access token breve (5-15 min) perché verifica locale non vede revoca (OWASP Microservices CS: validazione locale «cannot detect server-side revocation before expiry without an additional mechanism»). https://cheatsheetseries.owasp.org/cheatsheets/Microservices_Security_Cheat_Sheet.html (2026-10-26)
- JWKS: `kid` in header, verificatore tiene cache JWKS, ricarica su `kid` ignoto con limite di frequenza; `kid` ignoto dopo ricarica → 401. Rotazione: pubblica nuova chiave prima di firmare con essa.
Proposta: Proposta → Decisa. EdDSA Ed25519, `typ: at+jwt`, claim `iss sub aud exp iat jti` + `roles` + `acl_version`; TTL access 5 min; `kid`; chiave privata solo in identity da segreto env/file non versionato.

## Area B — Storage dei token nel browser

Fonte autorevole:
- Repo, vincolante: threat model T-03 «refresh token in cookie HttpOnly, Secure, SameSite; access token solo in memoria; CSP restrittiva», test fasi 5 e 9; security-scanner regola token-in-storage (`localStorage`/`sessionStorage` vietati) e `token-in-js-cookie`.
- draft-ietf-oauth-browser-based-apps-26 (dic 2025): pattern in ordine di sicurezza decrescente, BFF «strongly recommended for business applications» §6.1.4.3; cookie BFF MUST Secure + HttpOnly, SHOULD SameSite=Strict, SHOULD prefisso `__Host`, path `/` §6.1.3.2; CSRF defense MUST §6.1.3.3, custom request header SHOULD §6.1.3.3.2; anche con token fuori portata del JS, codice malevolo può chiedere token via flussi normali §6.2.4.3.1. https://www.ietf.org/archive/id/draft-ietf-oauth-browser-based-apps-26.txt (2026-10-26). Nota: draft per client OAuth; CKA = identity first-party, nessun authorization server esterno → BFF completo sovradimensionato, regole cookie/CSRF valgono per cookie di refresh.
Pratica consolidata:
- Variante T-03: access in memoria JS (perso a reload → `POST /auth/refresh` all'avvio), refresh opaco in cookie HttpOnly Secure SameSite=Strict. Caddy unico origin (02 D-13) → same-site, SameSite=Strict funziona.
- CSRF su `/auth/refresh` e `/auth/logout` (unici endpoint che usano cookie): SameSite=Strict + header custom obbligatorio + controllo `Origin`.
- Alternativa «tutto cookie» (access in cookie HttpOnly): niente token in JS, ma ogni rotta API cookie-autenticata → CSRF ovunque, document/rag leggono cookie. Più superficie, contro T-03 già scritto.
- `__Host-` richiede `Path=/` → cookie inviato a ogni rotta (ignorato fuori identity). Alternativa `Path=/api/v1/auth` senza prefisso: invio limitato, perde garanzia `__Host-`. Dettaglio a discrezione in plan.
Proposta: allinearsi a T-03.

## Area C — Refresh, rotazione, logout

Fonte autorevole:
- RFC 9700 §2.2.2: refresh token per client pubblici MUST sender-constrained o rotazione (§4.14). §4.14.2: token invalidato ripresentato = breach, server revoca refresh attivo. https://www.rfc-editor.org/rfc/rfc9700.html (2026-10-26)
- Repo: interfaces-and-concurrency.md §Identity: `POST /auth/refresh` «ruota il refresh token», `POST /auth/logout` «revoca la sessione»; criterio 4 ROADMAP.
Pratica consolidata:
- Refresh opaco (256 bit casuali), in DB solo hash SHA-256; tabella sessioni con `family_id`; ogni refresh: vecchio marcato usato, nuovo emesso in una tx (`FOR UPDATE` sulla riga); riuso di un usato → revoca intera famiglia + evento audit.
- Gara legittima: due tab che rinfrescano insieme → secondo vede token usato → famiglia revocata = logout spurio. Mitigazioni: finestra di grazia breve (es. 10 s, ritorna stesso successore) oppure serializzare refresh nel client (Web Locks API). Da decidere.
- Logout: revoca famiglia + cancella cookie. Access già emesso valido fino a `exp` (≤ TTL). Denylist `jti` richiederebbe stato condiviso → contro goal «senza chiamare identity».
- Durata sessione: assoluta + inattività. Valori da decidere.

## Area D — Password e Argon2id

Fonte autorevole:
- OWASP Password Storage CS: Argon2id minimi equivalenti `m=47104 t=1 p=1`, `m=19456 t=2 p=1`, `m=12288 t=3 p=1`…; upgrade parametri = rehash al login successivo; pepper solo difesa in profondità, in vault/HSM. https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html (2026-10-26)
- RFC 9106 §4: prima raccomandazione `t=1 p=4 m=2 GiB`; seconda `t=3 p=4 m=64 MiB`; salt 128 bit, tag 256 bit. https://www.rfc-editor.org/rfc/rfc9106.html (2026-10-26)
- NIST SP 800-63B-4 §3.1.1.2: password solo fattore → min 15 caratteri; max consentito ≥ 64 (SHOULD); blocklist password comuni/compromesse SHALL. https://pages.nist.gov/800-63-4/sp800-63b.html (2026-10-26)
- OWASP Authentication CS: niente regole di composizione; tutti i caratteri inclusi unicode e spazi; min 15 senza MFA; max ≥ 64; DoS da password lunghe. https://cheatsheetseries.owasp.org/cheatsheets/Authentication_Cheat_Sheet.html (2026-10-26)
- Repo: AUTH-03/NFR-002 «hash Argon2id versionato»; scanner `argon2-variant`; CLAUDE.md lavoro CPU-bound in pool limitati (`spawn_blocking`).
Pratica consolidata:
- Formato PHC (`$argon2id$v=19$m=…,t=…,p=…$salt$hash`) = parametri versionati dentro l'hash; login con parametri ≠ correnti → rehash. Crate RustCrypto `argon2` (versione in plan).
- Costo 19-64 MiB per verifica → semaforo dedicato dentro `spawn_blocking`: login concorrenti = memoria limitata.
- Tetto byte prima di hashare (es. 1024), anti-DoS.
- Blocklist: lista locale versionata (top N password comuni) vs API HIBP k-anonymity (chiamata esterna, contro toolchain semplice).
Opzioni parametri: `m=19456 t=2 p=1` (minimo OWASP, leggero, desktop condiviso con GPU/Docling) vs `m=65536 t=3 p=4` (RFC 9106 seconda).

## Area E — Enumerazione utenti e rate limit sul login

Fonte autorevole:
- OWASP Authentication CS: risposta generica identica per utente inesistente, password errata, account bloccato/disabilitato; tempo può rivelare → stesso processo sempre. Lockout: soglia, finestra, durata; contatore legato all'account, non all'IP; rischio DoS del lockout. (URL sopra)
- NIST 800-63B-4 §3.2.2: al massimo 100 tentativi falliti consecutivi per autenticatore; limiti più bassi ammessi.
- Repo: criterio 2 ROADMAP (stesso errore generico); T-01 «rate limit per IP e per account… oltre soglia 429»; 02 D-06 429 solo rate limit; fase 14 rate limit per utente sulle query (T-24) → separato.
Pratica consolidata:
- Utente inesistente → verifica contro hash fittizio con parametri correnti (tempo uguale).
- Throttling progressivo per account (es. 5 falliti in 15 min → 429 con `Retry-After`) + tetto per IP. Niente lockout permanente (nessun reset self-service → lockout = DoS).
- Stato contatori: tabella PG (sopravvive al riavvio) vs memoria in-process (`governor`, si azzera). CLAUDE.md: Redis solo dopo misura.
- Contatore anche per email inesistenti → 429 non rivela esistenza.

## Area F — Propagazione dell'identità fra servizi (per fase 7) e auth servizio-servizio

Fonte autorevole:
- OWASP Identity Propagation Patterns CS: (1) inoltro del token esterno ai destinatari che lo validano = più semplice, diffonde bearer, nessun isolamento fra destinatari; (2) attributi firmati da servizio qualsiasi sconsigliato; (3) token exchange RFC 8693 per audience diversa/delega, aggiunge dipendenza STS; (4) asserzione interna da issuer fidato. Raccomandazioni: validare firma, alg, iss, aud, typ, exp; non allargare `aud` solo per far funzionare l'inoltro; ogni destinatario verifica audience e autorizza azione; «start with a token every intended recipient can validate». https://cheatsheetseries.owasp.org/cheatsheets/Identity_Propagation_Patterns_Cheat_Sheet.html (2026-10-26)
- OWASP Microservices CS: contesto utente validabile da ogni destinatario; chiamante autenticato separatamente (mTLS o token di servizio); rifiutare se validazione non completabile. (URL sopra)
- Repo: ADR-0012 punto 5 e conseguenze: `RetrievalSearchResponse{subject_id, acl_version,…}` controllata lato rag da `authorize_response` con scope del principal verificato; threat model confine 2 «/internal/v1 … autenticate»; 02 D-14 slot verificatore token di servizio «5 o 7».
Opzioni:
- F1: rag inoltra l'access token utente a document su `/internal/v1/...`; `aud` = insieme fisso `["cka-document","cka-rag"]` deciso all'emissione. Document deriva `subject_id`/`acl_version` dal token verificato, mai dal body.
- F2: token exchange via identity → chiamata a identity sul percorso query, contro goal.
- Servizio-servizio: rete interna + Caddy blocca `/internal` (02 D-14); autenticazione del chiamante nasce con prima rotta interna = fase 7. Fase 5 fornisce verificatore del token utente riusabile.
Proposta: F1 + slot servizio-servizio → fase 7.

## Area G — Ruoli, revoca e `acl_version`

Fonte autorevole:
- Repo: AUTH-02, criterio 3; T-04 (403 + evento deny; cambio da admin → evento con attore e versione; ogni cambio incrementa `AclVersion`); ADR-0012 §6 `acl_version` monotona, cambia a ogni cambio ruoli; interfaces `PATCH /api/v1/users/{id}/roles`.
- OWASP Microservices CS: verifica locale non vede revoca prima di `exp`. (URL sopra)
Pratica consolidata:
- Insieme di ruoli per utente (non gerarchia); PATCH sostituisce insieme in una tx che incrementa `acl_version` e scrive audit.
- Invariante «ultimo admin»: rifiuto rimozione admin all'ultimo admin attivo.
- Revoca efficace entro TTL access. Rafforzo: al cambio ruoli revocare sessioni refresh dell'utente. Notifica a document via outbox fuori fase 5.
- Token porta `acl_version`; consumatori (rag `authorize_response`, cache) confrontano.

## Area H — Primo admin (bootstrap)

Riferimenti di prodotto:
- Keycloak: `KC_BOOTSTRAP_ADMIN_USERNAME`/`_PASSWORD` letti solo al primo avvio, ignorati dopo; admin temporaneo. https://www.keycloak.org/nightly/getting-started/getting-started-openshift + https://pub.nethence.com/identity/keycloak (2026-10-26)
- Alternative: sottocomando CLI (`identity-service create-admin`, password da stdin); seed in migrazione scartato (segreto in file versionato, scanner).
Pratica: env one-shot o CLI; password mai in log; evento audit.

## Area I — Scope del ciclo di vita utente (lente prodotto)

- Requisiti: FR-001 login, FR-002 ruoli; README: nessun MFA. Niente su cambio password, reset, disattivazione.
- Comparabili (Keycloak, assistenti RAG enterprise con login locale): creazione da admin con password temporanea + cambio obbligatorio al primo accesso; disattivazione utente; reset via email (SMTP = nuova dipendenza → fuori MVP).
- Minimo sensato da decidere: cambio password proprio; disattivazione da admin con revoca sessioni.

## Area J — Audit e dati personali

- Repo: FR-009; criterio 2 «login riusciti e falliti emessi tramite AuditSink»; 04 D-14 AuditSink schema `identity`; 02 D-20 redazione; T-22 no-leak (niente token nei log).
- GDPR art. 5(1)(c) minimizzazione: https://eur-lex.europa.eu/eli/reg/2016/679/oj (2026-10-26). OWASP Logging CS: mai password, token, session id nei log. https://cheatsheetseries.owasp.org/cheatsheets/Logging_Cheat_Sheet.html (2026-10-26)
- Login fallito per email inesistente: nessun user_id. Opzioni: solo IP + esito vs HMAC email normalizzata (correla attacchi senza email in chiaro).
- Eventi minimi: login_succeeded, login_failed, login_throttled, refresh_reuse_detected, logout, user_created, roles_changed, role_change_denied, bootstrap_admin.

## Area K — Frontend auth

- Repo: 02 D-23 React Router 8 data mode, registro route con riga per fase; TanStack Query; T-03.
- Pratica: al boot `POST /auth/refresh` → access in memoria (modulo, non persistito); loader protetti con redirect a `/login`; fetch wrapper con un solo refresh in volo su 401; JWT mai decodificato per decisioni (scanner `jwt-decode-unverified`); ruoli UI da `GET /auth/me`.
- Dettagli = discrezione in plan.
