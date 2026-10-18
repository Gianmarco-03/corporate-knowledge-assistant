---
name: update-orchestrator-state
description: Il notaio del loop GSD+ECC di Corporate Knowledge Assistant — unica skill autorizzata a scrivere .planning/ORCHESTRATOR-STATUS.md, lo stato persistente dell'orchestratore. USARE al termine di ogni verbo del loop (discuss, plan, execute, verify — con il flag corrispondente), oppure standalone con --evento (accadimento fuori banda - rate limit, trappola scoperta, intervento manuale) o --uat (esito di un giudizio umano). MAI scrivere ORCHESTRATOR-STATUS.md direttamente da un'altra skill o a mano.
---

# update-orchestrator-state

Aggiorna `.planning/ORCHESTRATOR-STATUS.md` — lo stato persistente dell'orchestratore del loop GSD+ECC (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md` §10). Se il file non esiste, crealo dalla struttura a 8 sezioni descritta sotto.

## Invocazione

`update-orchestrator-state --<flag>` con esattamente UN flag: `--discuss | --plan | --exec | --verify-ecc | --verify | --evento | --uat`.

## Sezioni di competenza per flag

Ogni flag aggiorna **solo** le sezioni di sua competenza. `## Posizione` e `## Prossima azione` si aggiornano SEMPRE, con qualunque flag.

| Flag | Oltre a Posizione + Prossima azione aggiorna |
| --- | --- |
| `--discuss` | `## Findings` (decisioni bloccate, ognuna con la fonte; criticità della fase), `## TODO` (questioni aperte / assunzioni provvisorie) |
| `--plan` | `## Findings` (esiti del wiring, seam aggiunti alla WIRING-MAP, verdetti dei gate sul piano, contratto emesso), `## TODO` |
| `--exec` | `## Findings` (onda completata: slice chiuse, deviazioni di livello a; rimbalzi di livello b) |
| `--verify-ecc` | `## Gate & verdetti` (colonna «Review ECC» per piano: esito, finding gravi corretti, residui accettati con motivo) |
| `--verify` | `## Gate & verdetti` (colonna «Verifier GSD»), `## UAT` (pendenti `human_judgment: true`), `## Findings` (esito della riconciliazione con la WIRING-MAP, una riga) |
| `--evento` | `## Limiti trovati` (una riga: cosa, quando, impatto, riferimento) |
| `--uat` | `## UAT` (esito del giudizio umano, con data) |

## Regole di scrittura

1. **Riscrittura per sezione, mai append cieco**: leggi il file, riscrivi per intero le sole sezioni di competenza del flag. Rilanciare con gli stessi dati non deve duplicare nulla (idempotenza).
2. **Timestamp**: ogni aggiornamento timbra `## Posizione` con la data da `node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" current-timestamp`.
3. **Indice, non archivio**: ogni voce è UNA riga con puntatore all'artefatto di fase (`0X-CONTEXT.md`, `0X-RICERCA-DISCUSSIONE.md`, `0X-CONTRATTO.md`, `0X-ONDE.md`, `0X-VERIFICATION.md`…). Il dettaglio vive nell'artefatto, mai copiato qui.
4. **Mai fatti GSD**: avanzamento piani e completamento fasi stanno in `STATE.md`/`ROADMAP.md` e qui si referenziano soltanto. Se un aggiornamento richiederebbe di scrivere un fatto GSD, il chiamante sta sbagliando punto di scrittura — rifiuta e segnala. Lo stesso vale per i seam inter-fase: vivono in `.planning/WIRING-MAP.md` (la scrive `wiring-design`, `verify` ne aggiorna lo stato) e qui si referenziano con una riga e il puntatore, mai copiati.
5. **`## Posizione`** ha formato fisso: `fase XX / onda N / slice 0X-YY / <verbo in corso o completato> · aggiornato: <data>` (i campi non applicabili restano `—`).
6. **`## Prossima azione`** è UNA riga imperativa che permette a una sessione fredda di ripartire senza leggere altro.
7. **Stile caveman full** (Regola caveman, spec §2): ogni voce tersa — niente articoli né riempitivi, ID/percorsi/numeri verbatim.

## Commit

- Invocata da un verbo del loop: **non committare** — il file entra nel commit di chiusura del verbo chiamante.
- Invocata standalone (`--evento`, `--uat`): committa da sola solo ORCHESTRATOR-STATUS.md, messaggio `docs(stato): <una riga su cosa è accaduto>`.

## Struttura del file (per la creazione da zero)

Intestazione con scopo e le due regole di sanità, poi le 8 sezioni nell'ordine: `## Posizione`, `## Prossima azione`, `## Findings`, `## Limiti trovati`, `## Gate & verdetti` (tabella `Piano | Gate sul PLAN | Review ECC | Verifier GSD`), `## UAT`, `## TODO`, `## Decisioni ratificate in corsa` (chi ha ratificato, quando). Le sezioni vuote portano `*(vuoto)*`.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
