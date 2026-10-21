# S-04 — Forma della chiamata `rank` a JEV, concorrenza, pin e alias · RISULTATI

**Modifica:** Claude (gsd-executor 01-11) · 2026-10-21 · domanda e soglie, scritte prima di qualsiasi misura

## Domanda e decisione da chiudere

Spike S-04, parte A-09 (piano 01-11). Le decisioni di 01-CONTEXT coinvolte sono D-17 (pin di una versione JEV esplicita, mai un alias mobile) e D-22 (forma della chiamata, concorrenza e retry misurati prima di fissare l'adapter).

La domanda ha quattro parti:

1. **Forma di `rank`.** Conviene una richiesta per candidato con domande Noul (forma A), uno `state` unico con N domande Noul (forma B) o una sola domanda Choice (forma C)? Si confrontano nDCG@5, recall@8, latenza, token e varianza su 3 ripetizioni.
2. **Concorrenza.** Quale livello fra 4, 8 e 16 richieste in volo regge senza 429, e come si comporta il client con `retry-after`?
3. **Alias.** A quale versione risolvono `jev-latest`, `jev-1.13.0` e `jev-preview` secondo `response.model`?
4. **Pin.** Il pin `jev-1.13.0` letto da `JEV_MODEL` risolve a sé stesso, con la versione risolta registrata in ogni riga?

La decisione va in ADR-0008. La riga «Modello testuale» di `DEPENDENCIES.md` (`jev-latest` → `jev-1.13.0`) la scrive il consolidamento (01-17).

## Soglie (scritte PRIMA della misura)

| Metrica | Soglia | Fonte soglia |
|---|---|---|
| Scelta della forma | La forma A resta il default se nDCG@5(A) ≥ nDCG@5 della migliore fra B e C **e** la latenza p95 del rank di 20 candidati per query è ≤ 2.000 ms alla concorrenza scelta. Altrimenti vince la forma con nDCG@5 più alto entro lo stesso tetto di latenza | RICERCA Area D §7 riga S1; piano 01-11 Task 1 |
| Latenza p95 del rank (20 candidati, una query) | ≤ 2.000 ms alla concorrenza scelta | piano 01-11 Task 1 (Claude's Discretion) |
| Concorrenza scelta | Il livello più alto fra 4, 8 e 16 con **0 risposte 429 su 3 run** | RICERCA Area D §7 riga S2 |
| Varianza del segnale | Deviazione standard del Noul `relevant` sullo stesso candidato fra 3 ripetizioni ≤ 0,05. Sopra soglia, nell'Esito la varianza diventa un vincolo per la calibrazione delle soglie | piano 01-11 Task 1; RICERCA Area D §7 riga S1 |
| Pin | `jev-1.13.0` risolve a sé stesso in `response.model` | D-17; RICERCA Area D §1.4 |
| Passaggi non visibili al ruolo inviati | **0** (hard gate): `unauthorized_sent` = 0 su ogni riga | D-16; threat model T-14 |
| Spesa della sessione | ≤ `S04_MAX_COST_USD` | piano 01-11 Task 1; T-01-11-03 |

Regole di lettura fissate insieme alle soglie:

- Gli aggregati usano le ripetizioni 1..3. La prima richiesta di ciascuna forma è un warm-up (`rep` = 0): resta nel CSV ed è esclusa dagli aggregati.
- La deviazione standard è quella campionaria (n − 1) del segnale per candidato fra le 3 ripetizioni: Noul `relevant` per A e B, probabilità della Choice per C. La soglia di 0,05 si legge sulla distribuzione per candidato (media, p95, massimo). Nel CSV, `noul_relevant_std` riporta il massimo per query.
- I percentili sono nearest-rank, calcolati su tutte le righe riuscite.
