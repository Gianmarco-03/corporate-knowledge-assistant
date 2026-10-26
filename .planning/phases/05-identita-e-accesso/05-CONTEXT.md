# Phase 5: Identità e accesso - Context

**Gathered:** 2026-10-26
**Status:** Ready for planning
**Criticità:** critica. È proposta dal ROADMAP («identità e autenticazione») e l'utente l'ha ratificata con questo CONTEXT il 2026-10-26. Il goal è autenticazione pura, e il threat model assegna a questa fase T-01, T-02 e T-04. In `plan`: `implementation-risk-audit` e `test-design` di GPT via AgentBridge.

<domain>
## Phase Boundary

La fase costruisce l'Identity & Access Service e il lato di verifica negli altri due servizi:

- **login** con email e password, emissione di access token JWT firmati asimmetricamente, **refresh** con rotazione, **logout** e `GET /api/v1/auth/me`;
- **JWKS** pubblicato su `/.well-known/jwks.json` e un **verificatore riusabile del token utente**, montato negli hook di autenticazione che la Fondazione pre-registra in document-service e rag-service;
- **utenti e ruoli**:
  - creazione da parte dell'admin;
  - assegnazione e revoca dei ruoli `admin`, `manager`, `employee`, `guest`;
  - elenco utenti, disattivazione, cambio della propria password, password temporanea (D-17);
  - bootstrap del primo admin da CLI;
- **feature auth di `apps/web`**: pagina di login, sessione, pagina admin degli utenti;
- **eventi di audit** di autenticazione e gestione utenti, tramite `AuditSink` (fase 4).

La fase non costruisce:
- **autenticazione servizio-servizio** su `/internal/v1`: va alla fase 7 (D-05);
- **policy di risorsa sui documenti** (`AccessPolicy`, predicato di retrieval): fasi 6, 7 e 10;
- **rate limit per utente sulle query** (T-24): fase 14;
- **MFA**: fuori dall'MVP per README;
- **reset della password via email**: fuori, perché richiede SMTP.

Requisiti: AUTH-01, AUTH-02, AUTH-03. Criteri di successo 1-5 del ROADMAP, fase 5. Lo scope allargato di D-17 non ha ancora righe di requisito (TODO).

Ricerca a supporto: `05-RICERCA-DISCUSSIONE.md`, Aree A-K, con fonti (URL e data di accesso 2026-10-26, oppure file e sezione del repo). Ogni decisione qui sotto cita l'area che la fonda.

**Rischio di pipeline (spec §8).**
- La fase 1 è in verify, con la gap-closure 1.6 da pianificare.
- Le fasi 2, 3 e 4 sono discusse ma non pianificate.
- I seam che questa fase consuma esistono solo come promesse nei CONTEXT 02 e 04, e la WIRING-MAP non ha ancora righe: hook di autenticazione in document/rag, riga del route registry, fascia di migrazioni `05`, slot `AuditSink` in identity.
- Se `wiring-design` della fase 2 ne cambiasse la forma, i vincoli sotto si aggiornano. Le decisioni di questa fase non dipendono dalla forma esatta.

</domain>

<decisions>
## Implementation Decisions

### Token e firma (Area A)
- **D-01: JWT firmati in EdDSA (Ed25519).** La riga «Token» di DEPENDENCIES.md passa da Proposta a Decisa, con un paragrafo *Storia*. Il lavoro si fa in `plan`, come per le Proposte della fase 2.
  - L'algoritmo atteso è fissato lato verifica: si rifiutano `alg none`, HS* e qualsiasi algoritmo diverso.
  - Se il crate scelto non supporta EdDSA con JWK in modo solido (verifica in `plan` con context7 o sul sorgente), si ripiega su ES256 senza riaprire il discuss.
  - Fonte: RFC 8725 §3.1; threat model T-02; `security-scanner` (`jwt-symmetric`, `jwt-alg-none`); Area A. Decisione utente.
- **D-02: Access token da 10 minuti.** È anche il limite superiore della finestra in cui un ruolo revocato o un logout restano visibili ai verificatori locali.
  - Fonte: OWASP Microservices Security CS (la validazione locale non vede la revoca prima di `exp`); Area A.
  - Decisione utente: 10 minuti, contro i 5 raccomandati.
- **D-03: Forma del token.**
  - Header: `typ: at+jwt` e `kid`.
  - Claim: `iss`, `sub` (UUID opaco dell'utente), `aud`, `exp`, `iat`, `jti`, `client_id` costante della web app, `roles` (insieme) e `acl_version` (monotona del soggetto, ADR-0012 §6).
  - Il verificatore rifiuta un `typ` diverso.
  - Il JWKS si tiene in cache nel verificatore e si ricarica su un `kid` ignoto, con un limite di frequenza. Se il `kid` resta ignoto anche dopo la ricarica → 401.
  - Fonte: RFC 9068 §2.1, §2.2 e §4; RFC 8725 §3.11; Area A. Chiuso come raccomandato, approvato dall'utente.

### Propagazione dell'identità (Area F)
- **D-04: Inoltro del token utente (pattern F1).** Per chiamare il Document Service, il RAG inoltra l'access token dell'utente.
  - `aud` è fissato all'emissione all'insieme `["cka-document", "cka-rag"]`: entrambi i destinatari previsti, non un allargamento successivo.
  - Ogni servizio controlla di essere fra gli `aud`.
  - Il Document Service prende `subject_id` e `acl_version` **solo dal token verificato, mai dal corpo della richiesta**. È il complemento di `authorize_response` lato RAG (ADR-0012 punto 5).
  - Scartato il token exchange (RFC 8693): richiede una chiamata a Identity per ogni query, contro il goal.
  - Fonte: OWASP Identity Propagation Patterns CS; Area F. Decisione utente.
- **D-05: Autenticazione servizio-servizio su `/internal/v1` alla fase 7.** Lo slot vuoto di 02 D-14 si assegna alla fase 7, che introduce la prima rotta interna e può provarlo davvero. La fase 5 consegna solo il verificatore riusabile del token utente.
  - Fonte: OWASP Microservices Security CS (il chiamante si autentica separatamente); regola provider-first di CLAUDE.md (nessuna operazione senza consumatore); Area F. Decisione utente.
  - Da registrare in WIRING-MAP da `wiring-design`.

### Sessione nel browser (Aree B e C)
- **D-06: Storage dei token, come da T-03.**
  - L'access token vive solo in memoria (un modulo JS, mai `localStorage` o `sessionStorage`).
  - Il refresh token è opaco, a 256 bit casuali, in un cookie `HttpOnly`, `Secure` e `SameSite=Strict`.
  - Al caricamento della pagina, `POST /auth/refresh` ricostruisce l'access token.
  - Fonte: threat model T-03 (vincolante); `security-scanner`; draft-ietf-oauth-browser-based-apps-26 §6.1.3.2; Area B.
- **D-07: CSRF sugli endpoint che usano il cookie** (`/auth/refresh`, `/auth/logout`): `SameSite=Strict`, un header custom obbligatorio e il controllo di `Origin` contro l'unica origine servita da Caddy (02 D-13).
  - Fonte: draft-ietf-oauth-browser-based-apps-26 §6.1.3.3; Area B. Chiuso come raccomandato.
- **D-08: Rotazione del refresh token con rilevamento del riuso.**
  - In database si salva solo lo SHA-256 del refresh token.
  - La sessione appartiene a una famiglia (`family_id`). A ogni refresh, in una transazione con `FOR UPDATE` sulla riga, il token vecchio diventa «usato» e se ne emette uno nuovo.
  - La ripresentazione di un token usato revoca l'intera famiglia ed emette un evento di audit (`refresh_reuse_detected`). Il server è severo: non c'è finestra di grazia.
  - Fonte: RFC 9700 §2.2.2 e §4.14.2; Area C. Decisione utente.
- **D-09: Schede multiple coordinate nel client.** Il frontend serializza il refresh fra le schede con la Web Locks API: una sola scheda alla volta rinfresca, le altre rileggono il risultato.
  - Il caso di due tab concorrenti va nei test (race condition da elencare in `interfaces-and-concurrency.md`).
  - Fonte: Area C. Decisione utente, contro la grazia lato server.
- **D-10: Durata della sessione:** 8 ore assolute dal login e 1 ora di inattività, cioè senza refresh.
  - Fonte: Area C (valori tipici per applicazioni aziendali con dati sensibili). Decisione utente.
- **D-11: Il logout revoca solo la famiglia della sessione corrente** e cancella il cookie. Le revoche globali vengono dal cambio ruoli (D-16), dalla disattivazione e dal cambio password (D-17). Non c'è un'azione «esci ovunque».
  - Fonte: criterio 4 del ROADMAP; Area C. Decisione utente.

### Password e login (Aree D, E, J)
- **D-12: Argon2id `m=65536` (64 MiB), `t=3`, `p=4`**, con salt da 128 bit e tag da 256 bit.
  - L'hash si salva in formato PHC, quindi i parametri sono versionati dentro l'hash. Al login, un hash con parametri diversi dai correnti viene ricalcolato.
  - La verifica gira in `spawn_blocking` sotto un semaforo dedicato da 4 permessi, così il picco resta sotto i 256 MiB.
  - Obiettivo di latenza da misurare in `plan`/`execute`: < 500 ms per login.
  - Fonte: RFC 9106 §4 (seconda raccomandazione); OWASP Password Storage CS (upgrade al login); CLAUDE.md (CPU-bound in pool limitati); Area D. Decisione utente.
- **D-13: Policy NIST completa.**
  - Minimo 15 caratteri, massimo 128, con un tetto in byte prima dell'hash contro il DoS.
  - Nessuna regola di composizione; ammessi unicode e spazi.
  - Blocklist locale di password comuni o compromesse, versionata nel repo e senza chiamate esterne. Vale per ogni password scelta da admin o utente.
  - Fonte: NIST SP 800-63B-4 §3.1.1.2; OWASP Authentication CS; Area D. Decisione utente.
- **D-14: Nessuna enumerazione degli utenti.**
  - Utente inesistente, password errata e utente disattivato danno la stessa risposta (stesso codice, corpo e header).
  - Per un utente inesistente la verifica gira contro un hash fittizio con i parametri correnti, così anche il tempo è lo stesso.
  - Fonte: OWASP Authentication CS; criterio 2 del ROADMAP; threat model T-01; Area E.
- **D-15: Throttling del login in PostgreSQL** (schema `identity`).
  - **Per account:** il contatore vale anche per le email inesistenti, normalizzate, così il 429 non rivela nulla. Dopo 5 fallimenti in 15 minuti → 429 con `Retry-After` e attesa crescente.
  - **Per IP:** tetto più alto, nell'ordine di 50 ogni 15 minuti, da fissare in `plan`.
  - Nessun blocco permanente. Entro il tetto NIST di 100.
  - Fonte: OWASP Authentication CS (contatore per account, rischio DoS del lockout); NIST SP 800-63B-4 §3.2.2; threat model T-01; 02 D-06 (429 solo per rate limit); Area E. Decisione utente.
- **D-15b: Audit dei login.** Un tentativo su un utente esistente registra lo `user_id`. Un tentativo su un'email inesistente registra l'HMAC-SHA256 dell'email normalizzata, con chiave segreta da variabile d'ambiente, più IP ed esito.
  - Mai password, token o cookie nei log o nell'audit.
  - Eventi minimi: `login_succeeded`, `login_failed`, `login_throttled`, `refresh_reuse_detected`, `logout`, `user_created`, `user_disabled`, `roles_changed`, `role_change_denied`, `password_changed`, `bootstrap_admin`.
  - Fonte: GDPR art. 5(1)(c); OWASP Logging CS; criterio 2 del ROADMAP; 04 D-14; Area J. Decisione utente.

### Utenti e ruoli (Aree G, H, I)
- **D-16: Cambio dei ruoli.** `PATCH /api/v1/users/{id}/roles` sostituisce l'insieme dei ruoli dell'utente (insieme piatto, nessuna gerarchia). In una sola transazione:
  - controlla l'autorizzazione: solo `admin`; altrimenti 403 con evento `role_change_denied`;
  - rifiuta con 409 se resterebbe zero admin attivi;
  - incrementa `acl_version` del soggetto;
  - revoca tutte le famiglie refresh dell'utente;
  - scrive l'audit con attore e versione.

  I ruoli nuovi arrivano al login successivo; l'access token in volo vale al massimo 10 minuti.
  - Fonte: threat model T-04; ADR-0012 §6 e conseguenze; 01-CONTRATTO §6; 03 D-08; Area G. Decisione utente.
- **D-17: Scope del ciclo di vita allargato** su riferimenti di prodotto (Keycloak e assistenti aziendali con login locale). Entrano:
  1. **Elenco utenti** per l'admin (`GET /api/v1/users`) e pagina admin web per creare utenti, cambiare ruoli e disattivare.
  2. **Disattivazione** da parte dell'admin: login rifiutato con l'errore generico di D-14, famiglie revocate, `acl_version` incrementata, audit. Si applica la protezione dell'ultimo admin.
  3. **Cambio della propria password**: richiede quella attuale, applica D-13, revoca le altre famiglie dell'utente e scrive l'audit.
  4. **Password temporanea**: quella impostata dall'admin vale solo per il primo accesso. Il login riesce ma la sessione è limitata al cambio password: con un claim o un flag, nessun'altra rotta finché la password non cambia.

  Le rotte nuove non sono in `interfaces-and-concurrency.md`: aggiornarlo, con lo YAML OpenAPI di identity, in `plan`.
  - Fonte: Area I. Decisione utente.
- **D-18: Bootstrap del primo admin da CLI.** Un sottocomando del binario identity (es. `create-admin`, eseguito con `docker compose run`) legge la password da stdin e applica D-13.
  - Rifiuta se esiste già un admin attivo, ed emette l'evento `bootstrap_admin`.
  - La password non passa mai da variabili d'ambiente, file o log.
  - Scartato il seed in migrazione: segreto in un file versionato, bloccato dal `security-scanner`.
  - Fonte: Area H (Keycloak bootstrap admin come riferimento contrario). Decisione utente.

### Vincoli dalla WIRING-MAP e dalle fasi precedenti
WIRING-MAP: zero righe al 2026-10-26, quindi nessun `WM-NNN` da citare ancora. Seam che la fase **consuma**, promessi nei CONTEXT e da registrare in `wiring-design` (plan della fase 2):
- **Hook di autenticazione** pre-registrati in document-service e rag-service (criterio 5 del ROADMAP): la fase 5 li riempie con il verificatore di D-01, D-03 e D-04.
- **Riga del route registry** di `apps/web` per la fase 5 (02 D-23): login e pagina admin.
- **Fascia di migrazioni** `05NNNN_*.sql` nello schema `identity` (02 D-09).
- **Slot `AuditSink`** nel composition root di identity, con la tabella append-only della fascia `04` (04 D-14).
- **Slot del verificatore del token di servizio** (02 D-14): assegnato alla fase 7 (D-05). La fase 5 non lo tocca.

Altri vincoli già decisi:
- Errori problem+json (02 D-05).
- Caddy: `/api/v1/auth/*` e `/api/v1/users/*` → identity (02 D-13). `/.well-known/jwks.json` deve essere raggiungibile dai servizi sulla rete interna; se esposto anche pubblicamente, va instradato in `plan`.
- Schema `identity` con ruolo DB proprio (02 D-07).
- Redazione dei log (02 D-20).
- `acl_version` del soggetto incrementata a ogni cambio di ruolo (ADR-0012), qui estesa anche a disattivazione (D-17).

### Claude's Discretion
- **Crate e versioni** (`jsonwebtoken` 11 o alternativa, RustCrypto `argon2`), con la verifica di EdDSA/JWK in `plan`.
- **Cookie**:
  - nome e path: prefisso `__Host-` con `Path=/`, oppure path ristretto a `/api/v1/auth` senza prefisso;
  - nome dell'header CSRF.
- **Chiave di firma**:
  - gestione: chiave privata da segreto file o env non versionato, `kid` derivato dalla chiave;
  - procedura di rotazione documentata: pubblicare la nuova chiave nel JWKS prima di firmare con essa.
- **Throttling**: soglia esatta per IP e progressione di `Retry-After`.
- **Normalizzazione dell'email**: minuscole e trim; nessuna manipolazione specifica dei provider.
- **Frontend auth**:
  - wrapper di fetch con un solo refresh in volo su 401;
  - loader protetti con redirect a `/login`;
  - ruoli dell'interfaccia da `GET /auth/me`, mai dalla decodifica del JWT (scanner `jwt-decode-unverified`).
- **Blocklist**: fonte e dimensione della lista locale, con licenza compatibile.

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Ricerca di fase
- `.planning/phases/05-identita-e-accesso/05-RICERCA-DISCUSSIONE.md` — brief Aree A-K con fonti.

### Decisioni di progetto
- `DEPENDENCIES.md` — riga «Token» (Proposta → Decisa con D-01), riga «Password hashing».
- `docs/adr/0012-firme-porte-candidate.md` — `acl_version` del soggetto (§6), `authorize_response` (punto 5), conseguenze sulla fase identity/RBAC.
- `.planning/phases/01-spike-tecnici/01-CONTRATTO.md` §3 e §6 — `AuthorizationScope`, `acl_version` al cambio ruoli.
- `.planning/phases/02-fondazione/02-CONTEXT.md` — D-05/D-06 (errori, 429), D-07 (schema per servizio), D-09 (fascia migrazioni), D-13 (Caddy), D-14 (slot servizio-servizio), D-20 (redazione), D-23 (route registry).
- `.planning/phases/03-provider-di-ingestion-e-indice/03-CONTEXT.md` — D-08 (versione ACL del soggetto alla fase 5/10).
- `.planning/phases/04-provider-di-decisione-risposta-e-trasversali/04-CONTEXT.md` — D-14 (AuditSink per servizio, schema `identity`).

### Architettura e requisiti
- `docs/architecture/interfaces-and-concurrency.md` §Identity & Access — endpoint (da estendere con D-17).
- `docs/architecture/architecture.md` §Identity & Access Service.
- `.planning/REQUIREMENTS.md` — AUTH-01, AUTH-02, AUTH-03.
- `docs/product/requirements.md` — FR-001, FR-002, FR-009, NFR-002.
- `README.md` — scope MVP (niente MFA).

### Sicurezza
- `docs/security/threat-model.md` — T-01, T-02, T-03, T-04 (test previsti alla fase 5), T-22 (no-leak).
- `skills/security-scanner/SKILL.md` — regole JWT, Argon2 e storage dei token.

### Standard esterni
- RFC 7519, RFC 8725 (JWT BCP), RFC 9068 (JWT access token), RFC 9106 (Argon2), RFC 9700 (OAuth Security BCP).
- NIST SP 800-63B-4; OWASP Password Storage, Authentication, Logging, Microservices Security, Identity Propagation Patterns Cheat Sheet; draft-ietf-oauth-browser-based-apps-26 (URL nel brief).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- Nessun codice di prodotto: `services/identity-service/` contiene solo `.gitkeep`. Gli spike non si importano (01-CONTRATTO §6); le firme si trascrivono.
- `spikes/s07-ports` — forma candidata di `AuthorizationScope`/`acl_version` e redazione del `Debug` (`t5_debug_redacts_sentinel_text`), da trascrivere.

### Established Patterns
- Fasce di migrazione per fase e servizio, errori problem+json, route registry esplicito, adapter dietro porte con fake deterministici (fasi 2-4).
- `CallContext` con deadline (ADR-0010) per ogni chiamata verso l'esterno, incluso il recupero del JWKS da parte dei verificatori.

### Integration Points
- Hook di autenticazione di document/rag (Fondazione) ← verificatore della fase 5.
- `AuditSink` di identity (fase 4) ← eventi di D-15b.
- Fase 6 (upload): consuma `Principal` e ruoli per i permessi per ruolo (Q-01).
- Fase 7: consuma D-04 (token inoltrato) e riempie lo slot servizio-servizio (D-05).
- Fasi 9 e 10: test T-03 (header CSP, nessun token nello storage) e matrice di leak per ruolo.

</code_context>

<specifics>
## Specific Ideas

- L'utente vuole capire i meccanismi: il racconto didattico delle slice dovrebbe spiegare rotazione e riuso del refresh, hash fittizio contro l'enumerazione e PHC/rehash.
- Il caso «due schede che rinfrescano insieme» e il caso «riuso di un refresh rubato» devono essere test distinti e leggibili.

</specifics>

<deferred>
## Deferred Ideas

- Reset della password via email (richiede SMTP): fuori MVP.
- «Esci da tutti i dispositivi» come azione utente: non adottato (D-11).
- MFA: fuori MVP (README).
- Notifica del cambio ruoli al Document Service via outbox, per una revoca sotto i 10 minuti: eventualmente fase 10 o 14.

</deferred>

## TODO aperti dal discuss

- **REQUIREMENTS:** valutare nuove righe di requisito (es. AUTH-04 ciclo di vita utente) per lo scope di D-17, via `gsd-tools`, prima di `plan`.
- **Contratti:** estendere `interfaces-and-concurrency.md` (elenco utenti, disattivazione, cambio password, password temporanea, race del refresh fra schede) in `plan`.
- **DEPENDENCIES.md:** «Token» da Proposta a Decisa (EdDSA) con *Storia*, in `plan`.
- **`.env.example`:** nuove chiavi (chiave privata di firma, chiave HMAC dell'audit, `kid`), nell'insieme di file della fase proprietaria o in whitelist, da verificare con `gate-proprieta-file`.
- **Assunzione provvisoria:** verifica del supporto EdDSA/JWK nel crate in `plan`; se manca, ES256 (D-01).
- **Pipeline:** gli hook di autenticazione e gli altri seam consumati diventano righe `WM-NNN` solo con il `plan` della fase 2. Riallineare i vincoli sopra a quel punto.

**Modifica:** Claude (Anthropic) · 2026-10-26 · CONTEXT della fase 5 dal discuss (decisioni utente 2026-10-26)
