# Phase 2: Fondazione - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-21
**Phase:** 02-fondazione
**Areas discussed:** Contratti API, Dati e migrazioni, Ambiente/build/CI, Shell frontend, Telemetria in locale, Log e no-leak, Health e CI, Rotte /internal/v1, Criticità

---

## Contratti API

### Tooling OpenAPI (A-01)

| Option | Description | Selected |
|--------|-------------|----------|
| utoipa code-first | Spec generata dagli handler; rotta e spec dalla stessa fonte | |
| Contract-first + generatore Rust | YAML fonte; xtask con typify, trait e router axum generati | |
| Contract-first Python + typify | Generatore Python con template; typify CLI per i tipi | |
| Contract-first, tipi generati | Solo tipi generati, handler a mano | |
| Servizio generatore a runtime | Proposta dell'utente: un servizio produce le istanze | |
| Classi a mano + generatore verificatore | Generatore Python di fixture valide/non valide | (scelta intermedia, poi superata) |
| Classi a mano, senza generatore | YAML + trait Contract + struct a mano; Schemathesis verifica | ✓ |

**User's choice:** contract-first semplificato. Le classi Rust si scrivono a mano e implementano il trait `Contract` (costruttore `from_json` che valida). Nessun generatore.

**Notes:**
- L'utente ha chiesto contesto due volte (cos'è OpenAPI, cos'è un contract test) e ha chiesto se fosse fattibile un generatore nostro, anche in Python.
- Ha proposto un'interfaccia `Contract` concretizzata dalle classi Rust e un servizio che restituisce istanze. Claude ha chiarito:
  - i tipi Rust esistono solo a compile time;
  - un servizio in più violerebbe la regola dei tre servizi;
  - il JSON degli oggetti vivi arriva dalla rete, non dal generatore (tipo e valore).
- Scelto prima «a mano, il generatore verifica». Poi l'utente ha chiesto un approccio più semplice senza Python: Claude ha verificato che Schemathesis `--mode all` genera dati validi e non validi (docs, consultate 2026-10-21) e il generatore è stato tolto.

### Contract test PLAT-03

| Option | Description | Selected |
|--------|-------------|----------|
| Snapshot + oasdiff + Schemathesis | Tre livelli | (prima risposta «1») |
| Solo snapshot + oasdiff | Senza servizio avviato | |
| Pact | Consumer-driven, broker | |
| oasdiff + Schemathesis --mode all + unit test su validate() | Versione semplificata | ✓ |

**User's choice:** versione semplificata.

### Formato errori

| Option | Description | Selected |
|--------|-------------|----------|
| problem+json + estensioni | RFC 9457 con code, message, request_id | ✓ |
| Busta JSON propria | {code, message, request_id} | |

### Backpressure

| Option | Description | Selected |
|--------|-------------|----------|
| 429 ovunque | Fedele ai documenti attuali | |
| 503 saturazione, 429 rate limit | Allineato alle RFC | ✓ |

### Client TS

| Option | Description | Selected |
|--------|-------------|----------|
| openapi-typescript + fetch + react-query | Tipi generati dallo YAML | ✓ |
| @hey-api/openapi-ts | Pre-1.0 | |
| Tipi TS a mano | | |

### Generatore Python (superata)

| Option | Description | Selected |
|--------|-------------|----------|
| tools/contracts/ | Python pinnato, job CI | (scelta, poi superata) |
| Altra collocazione | | |

**Notes:** superata dalla versione senza generatore.

---

## Dati e migrazioni

### Isolamento

| Option | Description | Selected |
|--------|-------------|----------|
| Database per servizio | Raccomandato dal brief | |
| Schema per servizio | Un db, tre schemi e tre ruoli | ✓ |

**Notes:** confermate le conseguenze: `search_path` per ruolo, lock delle migrazioni unico, pgvector nello schema document, cataloghi visibili come rischio residuo, test 42501.

### Fascia migrazioni

| Option | Description | Selected |
|--------|-------------|----------|
| PPNNNN + gate | Intero unico, gate CI | ✓ |
| Timestamp sqlx | | |

### Primitive trasversali

| Option | Description | Selected |
|--------|-------------|----------|
| Meccanismo in Fondazione | Raccomandato dal brief | |
| Fasi proprietarie | Come il CONTRATTO di fase 1 | ✓ |
| Tutto in Fondazione, tabelle incluse | | |
| Solo seam vuoti | | |

**Notes:** confermate le conseguenze: duplicazione fra servizi accettata, riuso senza modifica nello stesso servizio, ROADMAP da annotare.

---

## Ambiente, build, CI

| Domanda | Scelta |
|---|---|
| Stack Rust (versioni, config, immagini) | Confermo tutto (crate `config`, distroless debian13) |
| Reverse proxy | Caddy in Fondazione |
| TLS verso Garage | HTTP su rete interna, TLS in fase 15 |
| Repo su C:\ / WSL2 | Immagini build, niente bind mount |

---

## Shell frontend

| Domanda | Scelta |
|---|---|
| React Router | v8 data mode |
| TypeScript | TS 7 se il lint regge, altrimenti 6.x |
| Stile | Tailwind 4 + shadcn/Radix |
| Tipi in crates/contracts | Trasversali ora, dominio come file vuoti |

---

## Aree aggiuntive

| Domanda | Scelta |
|---|---|
| Telemetria in locale | Export OTLP opzionale + Jaeger v2 in override (passa alla fase 14) |
| Log e no-leak | Redazione di default nello scheletro |
| Health e CI | /livez + /readyz; contract test a ogni push |
| Rotte /internal/v1 | Isolamento di rete ora, slot vuoto per l'autenticazione (fase 5/7) |
| Criticità | Normale |

---

## Claude's Discretion

- Struttura dei job CI, cache, modalità offline di sqlx.
- Porte, nomi dei ruoli, default di pool, timeout e limiti di concorrenza.
- Tag esatti di Jaeger v2, Garage e cargo-chef.
- Posizione dello YAML OpenAPI per servizio.

## Deferred Ideas

- Rate limit per client: fase 14.
- Autenticazione servizio-servizio: fase 5 o 7.
- TLS per deployment non locali: fase 15.
- Dashboard di osservabilità: fase 14.
- Node 26 LTS dopo il 2026-10-28.

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 02)
