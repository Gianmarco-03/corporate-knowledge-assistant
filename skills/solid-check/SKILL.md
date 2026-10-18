---
name: solid-check
description: Verifica che un piano, un file o un crate/modulo di CKA rispetti i principi SOLID (SRP, OCP, Liskov, Interface Segregation, DIP) e le regole architetturali del progetto — trait-porta invece di dipendenze concrete, niente dispatch sul tipo dove un trait o un registry lo sostituisce, niente stato globale mutabile, niente I/O bloccante in async. Copre Rust (servizi e crate del workspace) e TypeScript/React (apps/web). USARE come gate sui PLAN in `plan`, prima di considerare completo un lavoro di codice, prima di una PR, quando si rivede un design, o quando l'utente chiede "controlla i principi", "è SOLID?", "rivedi l'architettura", "check design". Combina uno scan euristico (script) con una review guidata per principio.
---

# SOLID Check — CKA

Scopo: tenere i casi d'uso dipendenti da trait-porta (`DocumentSource`,
`DocumentStore`, `DocumentParser`, `Chunker`, `Embedder`, `VectorIndex`,
`DecisionEngine`, `AnswerGenerator`, `AccessPolicy`, `AuditSink`, `Cache`),
gli adapter sostituibili (JEV, fake deterministico, baseline; `NoOpCache`
prima di Redis) e il frontend leggibile per responsabilità. Riferimenti:
`CLAUDE.md`, `docs/architecture/interfaces-and-concurrency.md`. I confini fra
crate li giudica `crate-boundary-check`.

## Sul PLAN (gate in `plan`)

Si leggono frontmatter e `<tasks>` (`node scripts/estrai.cjs piani <dir-fase>`)
e si giudica il disegno, non il codice: ogni tipo nuovo ha una sola ragione di
cambiare; una variante nuova (fonte online, parser, provider LLM, ruolo) si
aggiunge con un'implementazione di trait o una voce di registry, non toccando
un `match`/`switch`; i casi d'uso ricevono trait-porta, il wiring sta nel
composition root (`src/main.rs`, `src/app.rs`, `src/app/**`); nessun trait
largo che costringe il fake JEV a implementare metodi che non usa. Verdetto
per piano: `conforme` / `da correggere`, con il motivo.

## A codice scritto

### 1. Scan euristico

```bash
python skills/solid-check/solid_scan.py <percorso> [<percorso> ...] [--json]
```

Lo script vive solo in `skills/solid-check/` (in `.claude/skills/` c'è solo
questo markdown); su questa postazione si può lanciare anche con
`sh scripts/py.sh`. Si passano i file o le directory toccati, mai il loro
antenato comune. Commenti, stringhe e moduli `#[cfg(test)]` sono tolti prima
dell'analisi; file di test (`tests/`, `__tests__/`, `e2e/`, `*_test.rs`,
`*.test.ts(x)`, `*.spec.ts(x)`) e generati (`@generated`, `*.gen.ts`, client
API generato) sono esclusi e contati in output.

| Principio | Rust | TypeScript/React |
|---|---|---|
| SRP | `impl` > 300 righe, `fn` > 60, `struct` > 12 campi | file `.tsx` > 300 righe, componente con > 6 `useState`/`useEffect`/`useReducer` |
| OCP | `match` sullo stesso enum in ≥ 3 siti; `match` su `.as_str()`; `downcast`/`TypeId` | `switch` su `type`/`kind`/`role`/`status`; `if (x.type === …)`; controlli di ruolo inline; `instanceof` |
| LSP | `unimplemented!`/`todo!` fuori dai test | `instanceof` su classe concreta |
| ISP | trait con > 6 metodi | `interface`/`type` con > 10 membri |
| DIP | `PgPool`, `reqwest::Client`, `redis::Client` fuori da adapter e composition root; `static mut`, `lazy_static`, `static` con `Mutex` | `fetch`/`axios` fuori dal client API |
| Arch | `std::thread::sleep`, `block_on`, `std::fs` sincrono in codice async | — |

Le soglie si cambiano con le opzioni (`--max-fn-lines`, `--min-enum-match-sites`, …).

**Match sul tipo, solo dove sostituibile.** Un `match` esaustivo su un enum
chiuso è Rust idiomatico e non è un difetto. Il candidato nasce quando lo
stesso enum si ramifica in più punti: ogni variante nuova tocca N file. Lì la
correzione è un trait (strategy) o un registry esplicito (mai
auto-discovery). Lo stesso vale per il frontend: una mappa
tipo → componente, non `switch` ripetuti.

Esiti: `0` nessun candidato, `1` candidati, `2` non eseguito (percorso
inesistente o nessun `.rs`/`.ts`/`.tsx` di produzione nello scope). Uno scope
vuoto non è mai verde; con `--json` il non eseguito esce come oggetto
`{error, message, status}`. Le directory con il marcatore
`.solid-check-fixture` sono escluse e dichiarate (`[ESCLUSA]`).

### 2. Review per principio (obbligatoria)

Lo script trova, l'agente giudica.

- **S** — Il tipo o componente ha una sola ragione di cambiare? Un handler
  Axum che valida, autorizza, interroga il DB e formatta la risposta va
  spezzato (estrattore, caso d'uso, adapter, DTO).
- **O** — Aggiungere una fonte, un parser, un provider o un ruolo richiede di
  modificare codice esistente? Allora serve un trait o un registry.
- **L** — Ogni implementazione di un trait-porta rispetta il contratto
  (errori, idempotenza, deadline, cancellazione)? Il fake di `DecisionEngine`
  deve comportarsi come l'adapter JEV su timeout ed errori, altrimenti i test
  non provano nulla. Nessun `unimplemented!()` dietro un trait.
- **I** — I trait sono piccoli e per client? `DecisionEngine` non deve
  assorbire autorizzazione o provenance (restano deterministiche e fuori dalla
  porta).
- **D** — I casi d'uso dipendono da `Arc<dyn Trait>` o generici vincolati,
  non da `PgPool` o `reqwest::Client`; il wiring sta solo nel composition root.

Regole architetturali oltre SOLID: nessuno stato globale mutabile; async per
l'I/O, `spawn_blocking` o processi isolati per il lavoro CPU-bound
(parsing/OCR); errori di dominio espliciti (`thiserror`), `anyhow` solo ai bordi.

### 3. Report

**Violazioni** (principio, `file:riga`, perché, fix), **odori da valutare**,
**conformità**. Verdetto PASS / PASS con note / FAIL; sul FAIL, il refactoring
minimo.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
