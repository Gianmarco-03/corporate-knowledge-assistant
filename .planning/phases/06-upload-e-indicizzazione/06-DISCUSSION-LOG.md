# Phase 6: Upload e indicizzazione - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-26 (prima area, dal checkpoint) e 2026-11-05
**Phase:** 06-upload-e-indicizzazione
**Areas discussed:** Ruoli e classificazione, Quarantena e stati, Coda/retry/audit, Docling e TTQ, Perimetro della fase

**Modifica:** Claude (Anthropic) · 2026-11-05 · log del discuss della fase 6

---

## Ruoli e classificazione (2026-10-26)

| Domanda | Opzioni | Scelta |
|---|---|---|
| Livelli di classificazione | 4 livelli · 3 livelli · Nessun livello, ACL libera | 4 livelli |
| Utenti nominati | Solo con restricted · Su ogni livello · Mai | Solo con restricted, uploader sempre incluso |
| Chi carica | Tutti tranne guest (racc.) · Solo manager e admin · Solo admin | **Solo manager e admin**, contro la raccomandazione |
| Chi cambia livello/ACL | Uploader manager + admin · Uploader + admin · Solo admin | Uploader + admin |
| Manager carica restricted | Sì · No, solo admin | Sì |
| Chi vede i job | Uploader + admin · Chiunque veda il documento | Uploader + admin |

## Quarantena e stati

| Domanda | Opzioni | Scelta |
|---|---|---|
| Q-06, PDF attivo su upload manuale | Terminale, si ricarica (racc.) · Terminale + rilascio admin · Approvazione per ogni upload | Terminale, si ricarica |
| Criterio 2 contro ADR | Segui gli ADR (racc.) · Limite a 10 MB | Segui gli ADR |
| File non valido all'upload | Due livelli (racc.) · Sempre un job | Due livelli |
| Diagnostica | Codice + testo fisso (racc.) · Codice + dettaglio tecnico | Codice + testo fisso |
| Altro sull'area | Prossima area · Altre domande | Prossima area: stati come proposti |

## Coda, retry, audit

| Domanda | Opzioni | Scelta |
|---|---|---|
| Coda | Tabella scritta a mano (racc.) · Crate esistente | Tabella scritta a mano |
| Retry | Permanenti subito, 5 transitori (racc.) · Più tentativi (come River) | Permanenti subito, 5 transitori |
| Stesso uploader, stesso hash | Restituisce l'esistente (racc.) · Nuovo documento | Restituisce l'esistente |
| Audit | Outbox + relay (racc.) · Append diretto dopo il commit | Outbox + relay |
| Crash, timeout, limite del parser | Un solo nuovo tentativo (racc.) · Permanente subito · Transitorio (5 tentativi) | Un solo nuovo tentativo |
| Dopo `failed` | Si ricarica, blob al GC (racc.) · Pulsante «riprova» | Si ricarica, blob al GC |
| Altro sull'area | Prossima area · Altre domande | **Altre domande** |
| Worker morto a metà | Da zero, conto al claim (racc.) · Riparte dalla fase raggiunta | Da zero, conto al claim |
| Coda piena | Tetto per utente + globale (racc.) · Solo tetto globale · Claim a turno fra utenti | Tetto per utente + globale |
| Granularità dell'audit | A ogni transizione di stato (racc.) · Solo inizio ed esito | A ogni transizione di stato |
| Codice del tetto | 429 utente, 503 globale (racc.) · 503 per entrambi | 429 utente, 503 globale |

**Notes:** il tetto era stato proposto con 503 per entrambi i casi; Claude ha corretto la proposta su 02 D-06 prima della chiusura.

## Docling e TTQ

| Domanda | Opzioni | Scelta |
|---|---|---|
| Docling nella pipeline, primo giro | Sincrono con interruttore (racc.) · Due passate · Spento di default | Risposta libera: «non si può fare docling in background dopo aver caricato il PDF?» |
| Docling, secondo giro | Background, stessa versione (racc.) · Background, versione nuova · Sincrono con interruttore | Background, stessa versione |

**Notes:** nel primo giro Claude aveva sopravvalutato i costi delle due passate («doppio embedding», «citazioni che puntano a chunk sostituiti»). Sulla domanda dell'utente ha ricontrollato: l'embedding costa circa 1 s (ADR-0007) e il locator PDF resta valido sugli stessi byte. Ha quindi corretto la descrizione e cambiato raccomandazione.

## Perimetro della fase

| Domanda | Opzioni | Scelta |
|---|---|---|
| Tombstone | In fase 6 (racc.) · In fase 12 | In fase 6 |
| Nuove versioni | Solo il modello ora (racc.) · Anche l'endpoint | Solo il modello ora |
| Avanzamento | Polling ogni 1-2 s (racc.) · SSE | Polling |
| UI | Upload + i miei caricamenti (racc.) · Solo upload e stato · Anche elenco visibili | Upload + i miei caricamenti |
| Uploader declassato | No, serve ancora il ruolo (racc.) · Sì, resta proprietario | No, serve ancora il ruolo |

## Criticità e chiusura

- Classificazione critica: ratificata.
- Ripresa dal checkpoint del 2026-10-26: «Riprendi».
- Chiusura: «Scrivi il CONTEXT».

## Claude's Discretion

Fingerprint e TTL dell'idempotency key; curva del backoff, lease, heartbeat e tetto dei tentativi per lease scaduto; valori dei tetti; 404 o 403 su un job non visibile; dettagli del frontend; forma tecnica della generazione di indice.

## Deferred Ideas

Pulsante «riprova»; rilascio admin dalla quarantena; claim a turno fra uploader; SSE; euristica pagine-tabella più precisa.
