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

## Legenda

- **kind**: `trait-port` · `registry` · `router-mount` · `migration-sequence` · `openapi-tag` · `ts-client` · `frontend-route` · `audit/outbox-event-registry` · `workspace-member` · `compose-service` · `config-key`
- **contributo**: `create-once` · `append` · `register` · `regenerate` · `shared-artifact`
- **stato**: `promised` (progettato, non ancora nel codice) · `created` (esiste vuoto, wave 0 fusa) · `filled` (almeno una fase `filled-by` ha fuso il proprio contributo; le altre restano nelle note) · `retired` (superato o abbandonato, con motivo)
- **owner**: servizio o area proprietaria (`identity-service`, `document-service`, `rag-service`, `crates/contracts`, `crates/telemetry`, `apps/web`, `infra`, `workspace`)

## Seam

| id | kind | file / posizione | owner | created-in | filled-by | contributo | stato | note |
|---|---|---|---|---|---|---|---|---|
| WM-001 | <kind> | `<percorso>::<simbolo>` | <owner> | <fase> | <fase>, <fase> | <contributo> | promised | <goal citato; convenzione in `0X-WIRING.md` §3; sha di creazione> |

<!-- Esempio di forma, da cancellare alla creazione del file reale:
| WM-001 | registry | `services/document-service/src/application/parsing.rs::ParserRegistry` | document-service | 03 | 03, 07 | register | created | 03 registra PDF; 07 (goal: "supporto DOCX") registra DOCX; sha a1b2c3d |
| WM-002 | router-mount | `services/rag-service/src/app.rs` nest `/api/v1/audit-events` | rag-service | 04 | 08 | create-once | promised | solo mount e `compose` vuota; rotte in 08 |
| WM-003 | migration-sequence | `services/identity-service/migrations/` | identity-service | 02 | 02, 05 | append | filled | VERSION = fase3+piano2+progr2; monotona |
-->

## Registro delle variazioni

| data | fase | skill | righe | variazione e motivo |
|---|---|---|---|---|
| AAAA-MM-GG | 0X | wiring-design | WM-001 | creata (promised): <motivo> |

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
