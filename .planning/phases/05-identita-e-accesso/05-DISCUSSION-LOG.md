# Phase 5: Identità e accesso - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-26
**Phase:** 05-identita-e-accesso
**Areas discussed:** Token e propagazione, Sessione nel browser, Password e login, Utenti e ruoli

**Modifica:** Claude (Anthropic) · 2026-10-26 · log del discuss della fase 5

---

## Token e propagazione

| Domanda | Opzioni | Scelta |
|---|---|---|
| Firma JWT (conferma della Proposta) | EdDSA Ed25519 (racc.) · ES256 · RS256 | EdDSA Ed25519 |
| Durata dell'access token | 5 min (racc.) · 15 min · 1 min | **10 min** (risposta libera) |
| Propagazione RAG → Document | F1 inoltro del token (racc.) · F2 token exchange | F1 |
| Slot servizio-servizio (02 D-14) | Fase 7 (racc.) · Fase 5 | Fase 7 |
| Altro sull'area | Prossima area · Altre domande | Prossima area: `typ`, claim e JWKS come nel brief |

**Notes:** 10 minuti, contro i 5 raccomandati: la finestra di revoca accettata è ≤ 10 minuti.

## Sessione nel browser

| Domanda | Opzioni | Scelta |
|---|---|---|
| Schede multiple con rotazione severa | Lock lato client (racc.) · Grazia 10 s lato server · Entrambe | Lock lato client |
| Durata della sessione | 8 h + 1 h di inattività · 7 gg + 24 h · Decidi tu | 8 h + 1 h |
| Scope del logout | Solo la corrente (racc.) · Anche «esci ovunque» | Solo la corrente |

**Notes:** CSRF (`SameSite=Strict` + header custom + `Origin`) presentato come informativa dal brief, nessuna obiezione.

## Password e login

| Domanda | Opzioni | Scelta |
|---|---|---|
| Parametri Argon2id | 64 MiB t=3 p=4 (racc.) · 19 MiB t=2 p=1 | 64 MiB t=3 p=4 |
| Policy password | NIST completo (racc.) · NIST senza blocklist · Min 12 + blocklist | NIST completo |
| Throttling | Postgres 5/15 min (racc.) · In memoria 5/15 min · Decidi tu | Postgres 5/15 min |
| Audit email inesistente | HMAC dell'email (racc.) · Solo IP ed esito | HMAC dell'email |

## Utenti e ruoli

| Domanda | Opzioni | Scelta |
|---|---|---|
| Bootstrap primo admin | Comando CLI (racc.) · Env al primo avvio | Comando CLI |
| Cambio ruoli | Ultimo admin + revoca sessioni (racc.) · Solo ultimo admin · Nessuna | Entrambe |
| Scope del ciclo di vita (multi) | Elenco utenti · Disattivazione · Cambio password proprio · Password temporanea | Tutte e quattro |

## Criticità e chiusura

- Classificazione critica: ratificata («Sì, critica»).
- Chiusura: «Scrivi il CONTEXT».

## Claude's Discretion

Crate e versioni; nome e path del cookie e header CSRF; gestione e rotazione della chiave di firma; soglia per IP e progressione di `Retry-After`; normalizzazione dell'email; dettagli del frontend auth; fonte della blocklist.

## Deferred Ideas

Reset via email (SMTP); «esci ovunque»; MFA; notifica del cambio ruoli via outbox per una revoca sotto i 10 minuti.
