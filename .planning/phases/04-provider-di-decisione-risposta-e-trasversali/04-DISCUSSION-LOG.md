# Phase 4: Provider di decisione, risposta e trasversali - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-21
**Phase:** 04-provider-di-decisione-risposta-e-trasversali
**Areas discussed:** Porte e baseline, Resilienza, AuditSink, Cache e test reali

Ogni domanda è arrivata armata del finding del brief `04-RICERCA-DISCUSSIONE.md` e di un esempio concreto (preferenza dell'utente).

---

## Porte e baseline

**Split delle porte** (S-07 vs criterio 1 del ROADMAP)

| Option | Description | Selected |
|--------|-------------|----------|
| Split S-07 | `Reranker` (JEV + baseline) separato da `DecisionEngine` (JEV + fake); criterio 1 riformulato | ✓ |
| Porta unica a 4 op | la baseline dovrebbe implementare anche le tre op semantiche | |

**Baseline senza JEV** per `validate_query`, `assess_evidence`, `validate_citations`

| Option | Description | Selected |
|--------|-------------|----------|
| Nessuna, errore tipizzato | baseline solo sul `Reranker`; policy alla fase 8 | ✓ |
| Engine conservativo | `Complex` / `Unassessed`, mai positivo | |
| Euristiche lessicali | sembrano una validazione senza esserlo | |

**Soglie sui punteggi JEV**

| Option | Description | Selected |
|--------|-------------|----------|
| Nel caso d'uso fase 8 | adapter solo segnali; calibrazione 13/14 | ✓ |
| Nell'adapter | la calibrazione toccherebbe il perimetro della fase 4 | |

**Routing Haiku/Sonnet**

| Option | Description | Selected |
|--------|-------------|----------|
| `Complexity` di dominio | mapping a model id da config nell'adapter | ✓ |
| Model id in firma | il nome del provider attraversa la porta | |

**Multi-turno (Q-05 aperta)**

| Option | Description | Selected |
|--------|-------------|----------|
| Single-turn ora | storia via riconciliazione se Q-05 = sì | ✓ |
| Predisporre la storia | parametro speculativo | |

## Resilienza

**Implementazione del circuit breaker**

| Option | Description | Selected |
|--------|-------------|----------|
| Scritto a mano | ~100 righe, clock tokio, nessuna dipendenza | ✓ |
| `failsafe` 1.3.0 | policy pronte, fermo da luglio 2024 | |
| `recloser` | piccolo, manutenzione non verificata | |

**Errori che contano come fallimento**

| Option | Description | Selected |
|--------|-------------|----------|
| Solo guasti del provider | timeout, 429, 5xx/529, connessione, malformati; non 4xx di contratto né cancellazioni | ✓ |
| Ogni errore | un 400 aprirebbe il circuito | |

**Granularità**

| Option | Description | Selected |
|--------|-------------|----------|
| Uno per provider | JEV condiviso da Reranker e DecisionEngine; uno per Anthropic | ✓ |
| Uno per operazione | tre breaker vedono lo stesso guasto in ritardo | |
| Decidi tu | scelta in plan | |

**Limite globale di concorrenza**

| Option | Description | Selected |
|--------|-------------|----------|
| Sì, semaforo globale | valore da ADR-0008 | ✓ |
| Solo limite per query | 8×N richieste con N query | |

## AuditSink

**Forma provvisoria (Q-04 al discuss 11)**

| Option | Description | Selected |
|--------|-------------|----------|
| Adapter PG per servizio | append-only, REVOKE + trigger, ON CONFLICT DO NOTHING | ✓ |
| Audit centrale nel rag-service | anticipa Q-04, dipendenza runtime | |

**Contenuto di AuditEvent**

| Option | Description | Selected |
|--------|-------------|----------|
| Tutto tipizzato | nessun payload libero | ✓ |
| Tipizzato + details JSON | la regola no-leak diventa disciplina | |

**Catena di hash**

| Option | Description | Selected |
|--------|-------------|----------|
| No, fuori dall'MVP (raccomandata) | REVOKE + trigger bastano; la catena serializza gli insert | |
| Sì, già in fase 4 | integrità verificabile | ✓ |

**Stesso event_id con contenuto diverso**

| Option | Description | Selected |
|--------|-------------|----------|
| Errore tipizzato | confronto del digest; `IdempotencyConflict` | ✓ |
| AlreadyApplied comunque | un id riusato passa in silenzio | |

**Serializzazione degli append con la catena** (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Riga di testa FOR UPDATE | `audit_chain_head` per servizio, tx breve | ✓ |
| Advisory lock | stesso effetto, meno visibile | |
| Decidi tu | in plan | |

**Ancoraggio esterno** (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| No, limite documentato | l'owner DB può ricalcolare la catena: scritto nel threat model | ✓ |
| Sì, testa nei log strutturati | rilevamento retroattivo | |

**Verifica della catena** (follow-up)

| Option | Description | Selected |
|--------|-------------|----------|
| Operazione con consumatore fase 11 | ritirata in riconciliazione se il discuss 11 non la conferma | ✓ |
| Solo test e comando di manutenzione | nessuna operazione di porta | |

## Cache e test reali

**Cache**

| Option | Description | Selected |
|--------|-------------|----------|
| NoOp + fake in memoria | fake testkit con scope e invalidazione per la fase 10 | ✓ |
| Solo NoOpCache | canale cache della fase 10 vuoto | |

**Dove girano gli adapter reali**

| Option | Description | Selected |
|--------|-------------|----------|
| Replay in CI + gate locale | wiremock con risposte registrate; provider veri in verify | ✓ |
| Provider veri in CI | segreti, flakiness, rate limit | |
| Solo locale | CI cieca | |

**Tetto del gate locale**

| Option | Description | Selected |
|--------|-------------|----------|
| 0,50 USD per esecuzione | cancellate contate come addebitate | ✓ |
| 1 USD | più margine | |
| Decidi tu | in plan | |

**Geografia dei dati**

| Option | Description | Selected |
|--------|-------------|----------|
| `"global"`, nessun dato utente | solo query e passaggi autorizzati | ✓ |
| `"us"` | geografia fissa | |

## Criticità

| Option | Description | Selected |
|--------|-------------|----------|
| Critica | GPT `implementation-risk-audit` + `test-design` in plan | ✓ |
| Normale | test design da agente Claude separato | |

## Claude's Discretion

- Etichette posizionali `S1..Sn` nel prompt; system prompt versionato e passaggi delimitati; effort/thinking/`max_tokens`/fallbacks secondo ADR-0011; forma di `CacheValue`; nomi delle tabelle; forma canonica per l'hash; struttura dei moduli; soglie del breaker.
- Snapshot dell'OpenAPI di JEV con hash e test di validazione delle request (D-11), annunciato all'utente senza domanda.

## Deferred Ideas

- Ancoraggio esterno dell'hash di testa (fase 11 o 15).
- Multi-turno (Q-05, discuss 8).
- Baseline semantica per `assess_evidence`/`validate_citations`.

**Modifica:** Claude (Anthropic) · 2026-10-21 · creazione (discuss fase 04)
