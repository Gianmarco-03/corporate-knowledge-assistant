---
name: crate-boundary-check
description: Verifica che i confini fra i crate del workspace Cargo di CKA siano rispettati — un servizio non dipende mai da un altro servizio, le dipendenze interne vanno solo verso `crates/contracts` e `crates/telemetry`, i moduli `domain` dei servizi (e l'intero `contracts`) non usano sqlx, reqwest, axum, tower, hyper né SDK di provider, il wiring sta solo nel composition root (`src/main.rs`, `src/app.rs`, `src/app/**`). USARE come gate sui PLAN in `plan` (modalità plan), quando si aggiunge una dipendenza a un `Cargo.toml`, si crea un crate, si tocca codice che attraversa i confini, in `verify` quando compare una dipendenza fra crate non prevista, o quando l'utente chiede "controlla i confini", "dipendenze tra servizi", "il dominio è pulito?".
---

# Crate Boundary Check — CKA

Garantisce l'isolamento dei tre bounded context (identity, document, rag) e la
regola porte-e-adapter: i tipi di database, HTTP e provider non attraversano
una porta di dominio. Fonti: `CLAUDE.md` (Architettura, Porte e adapter),
`docs/architecture/interfaces-and-concurrency.md`, `docs/adr/0001-*`.
L'unico posto in cui si dichiara un crate è `boundaries.json`: un crate nuovo
richiede prima un ADR (esattamente tre servizi, nessun quarto bounded context).

## Regole

1. **Servizio → servizio: mai.** Nessuna dipendenza Cargo (normal, dev o
   build) da un crate `kind: service` a un altro. I dati attraversano i confini
   solo via contratti HTTP o transactional outbox; i payload cross-service
   vivono in `crates/contracts`.
2. **Dipendenze interne ammesse**: per ogni crate, solo quelle elencate in
   `allowed_internal` (servizi → `contracts`, `telemetry`; le librerie sono
   foglie).
3. **Moduli puri** (`pure_paths`: `src/domain` dei servizi, `src` di
   `contracts`): nessun uso dei crate in `pure_forbidden_crates` (sqlx,
   reqwest, axum, tower, hyper, client DB/Redis, SDK LLM/S3, parser PDF/OCR…)
   né dei moduli di adapter dello stesso crate (`crate::infrastructure`,
   `crate::adapters`, `crate::http`, `crate::app`, …).
4. **`contracts` è un crate di DTO**: non dichiara in `Cargo.toml` le
   dipendenze in `forbidden_cargo_deps` (sqlx, reqwest, axum, tower, hyper).
5. **Wiring solo nel composition root**: il modulo `app` di ogni servizio,
   cioè `src/main.rs`, `src/app.rs` e l'intero sottoalbero `src/app/**`
   (un `compose()` per feature). Lo script segnala la costruzione di pool,
   listener e server (`wiring_patterns`) fuori da lì; il resto del giudizio
   sulla DI (adapter istanziati dentro un caso d'uso) è review, vedi
   `solid-check`. Il registro degli eventi sta in `src/events/registry.rs`
   del servizio.
6. Ogni membro del workspace è dichiarato in `boundaries.json`; un membro non
   dichiarato è una violazione.

## Modalità e invocazione

Lo script vive solo in `skills/crate-boundary-check/` (in `.claude/skills/`
c'è solo questo markdown). Su questa postazione si lancia con `python`
(Python 3.13) o con `sh scripts/py.sh`, che risolve l'interprete.

**Scan del workspace** (a codice scritto, hook git, `verify`):

```bash
python skills/crate-boundary-check/boundary_check.py .          # radice come posizionale
python skills/crate-boundary-check/boundary_check.py --root . --json
```

Legge i membri con `cargo metadata --format-version 1 --no-deps`
(dipendenze dichiarate, con il loro `kind`) e scansiona i `.rs` dei moduli
puri dopo aver tolto commenti, stringhe e char: un `sqlx::` in un commento non
è una violazione. Senza argomenti la radice è quella del repo, ricavata dalla
posizione dello script e mai dalla directory corrente.

**Plan** (gate sui PLAN in `plan`, senza cargo):

```bash
python skills/crate-boundary-check/boundary_check.py --plan .planning/phases/<fase>/*-PLAN.md
```

Dal piano legge: `files_modified` (frontmatter; file in un crate non
dichiarato → violazione, file nel composition root → nota di wiring point);
la mappa opzionale `crate_deps` (frontmatter, `crate: [dep, …]`); i comandi
`cargo add … -p <crate>`; i blocchi ```toml preceduti dal percorso di un
`Cargo.toml`; i blocchi ```rust preceduti dal percorso di un file sotto un
modulo puro. Sono candidati estratti dal testo: l'agente-gate giudica ogni
riga e scrive in `0X-GATE.md` quelle legittime con il motivo, così il re-gate
non le ridiscute. Se le dipendenze nuove non sono scritte in nessuna di
queste forme, il gate le chiede al piano: una dipendenza non dichiarata non
è giudicabile.

## Esiti (regola anti-falso-verde)

| Situazione | Esito | Codice |
|---|---|---|
| nessuna violazione | `OK` / `pass` | 0 |
| almeno una violazione | `FAIL` con l'elenco (`rule`, sorgente → destinazione, file:riga o origine nel piano) | 1 |
| `boundaries.json` assente, illeggibile o senza `crates` | `NON ESEGUITO [boundaries-config-*]` | 2 |
| nessun `Cargo.toml` nella radice | `NON ESEGUITO [cargo-workspace-missing]` | 2 |
| `cargo` non nel PATH, `cargo metadata` fallito o oltre 180 s | `NON ESEGUITO [cargo-*]` | 2 |
| workspace senza membri | `NON ESEGUITO [cargo-workspace-empty]` | 2 |
| PLAN assente o senza `files_modified` | `NON ESEGUITO [plan-missing / plan-scope-empty]` | 2 |

Solo 0 è verde; 2 blocca come 1. Con più PLAN l'esito è il peggiore (2
domina 1). In `--json` lo stdout è il contratto macchina: lista di violazioni
(scan), lista di risultati per piano (plan) oppure oggetto
`{error, message, status: "not-run"}`; le note escono sul canale diagnostico.

**Note mai silenziose.** Ogni crate dichiarato ma non ancora nel workspace
esce come `[NON PRESENTE]`; il numero di file analizzati è sempre stampato;
una directory con il marcatore `.boundary-check-fixture` (fixture che violano
di proposito) è esclusa e dichiarata come `[ESCLUSA]`. Un'esclusione taciuta
sarebbe indistinguibile da una directory pulita.

## Verifica manuale (complemento dello script)

- Un tipo di dominio di un servizio compare in un payload verso un altro? →
  DTO in `crates/contracts`.
- Un caso d'uso istanzia un adapter (`PgVectorIndex::new`, `JevClient::new`)
  invece di ricevere il trait-porta? → il wiring va in `src/app/**`.
- Una porta espone tipi di provider (`sqlx::Row`, `reqwest::Response`)? →
  violazione della regola delle porte, anche fuori da `src/domain`.

Report: violazioni come `sorgente -> destinazione : motivo`, verdetto
PASS/FAIL e, sul FAIL, il refactoring minimo (es. «spostare il DTO in
`crates/contracts` e dipendere da quello»).

## Il file di configurazione

| Chiave | Uso |
|---|---|
| `crates.<nome>.path` / `kind` | percorso del crate e `service` o `library` (obbligatori) |
| `crates.<nome>.allowed_internal` | crate interni ammessi come dipendenza |
| `crates.<nome>.pure_paths` | percorsi (relativi al crate) da scansionare come moduli puri |
| `crates.<nome>.forbidden_cargo_deps` | dipendenze Cargo vietate a livello di crate |
| `pure_forbidden_crates` / `pure_forbidden_modules` | ciò che un modulo puro non può usare |
| `composition_roots` | file o sottoalberi (voce che finisce con `/`) del composition root |
| `wiring_patterns` | costruzioni ammesse solo nel composition root |
| `non_crate_areas` | aree del repo fuori dai crate, accettate in `files_modified` |

Ogni chiave ha un consumatore nello script: una chiave che nessuno legge è
la forma documentale del falso verde, e non si aggiunge.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
