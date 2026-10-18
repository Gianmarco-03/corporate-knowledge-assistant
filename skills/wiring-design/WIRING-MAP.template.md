# WIRING-MAP — registro dei seam di progetto

> Template: `skills/wiring-design/WIRING-MAP.template.md`. Il file reale è `.planning/WIRING-MAP.md`, creato da `wiring-design` al primo `plan` dopo il bootstrap GSD.
> Registro **persistente** dei seam di estensione del progetto: punti in cui una fase prepara l'aggancio di un'altra (trait-porta, registri, mount point, sequenze di migrazioni, tag OpenAPI, route, registri di eventi, membri del workspace, servizi Compose, chiavi di configurazione).

## Chi lo legge e chi lo scrive

| Skill | Uso |
|---|---|
| `discuss` | legge: seam che la fase deve riempire o predisporre, portati nel CONTEXT come vincoli con il riferimento `WM-NNN` |
| `wiring-search` | legge: righe con `created-in` o `filled-by` uguali alla fase, e controlla che le righe `created` esistano nel codice |
| `wiring-design` | **scrive**: righe nuove (`promised`), aggiornamenti di forma e di fasi, ritiri |
| `wiring-build` | **scrive**: `promised` → `created` con lo sha della wave 0 |
| `execute` | consulta: toccare un seam di questa mappa fuori da quanto §4 del WIRING consente è deviazione (b), rimbalzo a `plan` |
| `verify` | **scrive**: riconcilia seam reali e mappa; `created` → `filled` quando la fase `filled-by` ha fuso il proprio contributo |
| `plan` (contratto) | `0X-CONTRATTO.md` elenca i seam promessi dalla fase con il loro `WM-NNN` |

## Regole

1. Una riga per seam, identificata da `WM-NNN` progressivo, mai riusato.
2. **Mai cancellare una riga.** Se il disegno cambia, la riga vecchia passa a `retired` con «sostituita da WM-NNN» nelle note, e se ne apre una nuova. Ogni variazione si registra in coda, nel registro delle variazioni.
3. Un seam creato in anticipo per una fase futura esiste **solo vuoto** (trait con firme, registro, mount point, cartella, segnaposto nudo): il contenuto della fase futura è vietato (`perimetro-check`).
4. Niente auto-discovery né registrazione magica; niente `match` sul tipo dove serve una variante estendibile: trait + registro esplicito.
5. Il goal di una fase futura si cita come motivo; i dettagli delle fasi non ancora discusse non entrano qui.
6. **Insiemi di file chiusi** (decisione utente 2026-10-18): un file d'innesto ha **una sola** `fase-proprietaria`. La fase `created-in` (di norma la Fondazione) lo crea vuoto; dopo, solo la `fase-proprietaria` lo modifica. `scripts/gate-proprieta-file.cjs` legge questa colonna per ammettere il passaggio fra le due fasi; ogni terza fase sullo stesso file è violazione. `—` = il file resta chiuso nella fase che lo crea (es. il composition root con le righe di registrazione). Un registro alimentato da più fasi ha un file d'innesto per fase, ciascuno su una riga propria.

## Legenda

- **kind**: `trait-port` · `registry` · `router-mount` · `migration-sequence` · `openapi-tag` · `ts-client` · `frontend-route` · `audit/outbox-event-registry` · `workspace-member` · `compose-service` · `config-key`
- **contributo**: `create-once` · `append` · `register` · `regenerate` · `shared-artifact`
- **stato**: `promised` (progettato, non ancora nel codice) · `created` (esiste vuoto, wave 0 fusa) · `filled` (almeno una fase `filled-by` ha fuso il proprio contributo; le altre restano nelle note) · `retired` (superato o abbandonato, con motivo)
- **fase-proprietaria**: l'unica fase che modifica il file d'innesto dopo la sua creazione (`—` se il file resta chiuso nella fase `created-in`); il percorso nella colonna file è il primo valore tra backtick, senza `::simbolo`
- **owner**: servizio o area proprietaria (`identity-service`, `document-service`, `rag-service`, `crates/contracts`, `crates/telemetry`, `apps/web`, `infra`, `workspace`)

## Seam

| id | kind | file / posizione | owner | created-in | filled-by | fase-proprietaria | contributo | stato | note |
|---|---|---|---|---|---|---|---|---|---|
| WM-001 | <kind> | `<percorso>::<simbolo>` | <owner> | <fase> | <fase>, <fase> | <fase o —> | <contributo> | promised | <goal citato; convenzione in `0X-WIRING.md` §3; sha di creazione> |

<!-- Esempio di forma, da cancellare alla creazione del file reale (01 = Fondazione):
| WM-001 | registry | `services/document-service/src/application/parsing.rs::ParserRegistry` | document-service | 01 | 03, 07 | — | create-once | created | tipo registro chiuso nella Fondazione; le registrazioni stanno negli innesti WM-004 e WM-005 |
| WM-002 | router-mount | `services/rag-service/src/app/audit.rs::compose` | rag-service | 01 | 08 | 08 | create-once | promised | `.merge(audit::compose(&infra))` pre-registrata in `app.rs` (Fondazione); rotte in 08 |
| WM-003 | migration-sequence | `services/identity-service/migrations/` | identity-service | 01 | 02, 05 | — | append | filled | cartella della Fondazione; ogni file di migrazione è della fase che lo scrive, per nome; VERSION = fase3+piano2+progr2; monotona |
| WM-004 | registry | `services/document-service/src/app/parsers_pdf.rs` | document-service | 01 | 03 | 03 | register | created | innesto PDF pre-registrato in `app.rs` |
| WM-005 | registry | `services/document-service/src/app/parsers_docx.rs` | document-service | 01 | 07 | 07 | register | promised | 07 (goal: "supporto DOCX") |
-->

## Registro delle variazioni

| data | fase | skill | righe | variazione e motivo |
|---|---|---|---|---|
| AAAA-MM-GG | 0X | wiring-design | WM-001 | creata (promised): <motivo> |

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
**Modifica:** Claude (Anthropic) · 2026-10-18 · strategia provider-first e insiemi di file chiusi (decisione utente)
