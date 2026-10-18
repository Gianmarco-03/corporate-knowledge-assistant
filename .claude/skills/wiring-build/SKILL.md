---
name: wiring-build
description: >
  Wave 0 di `execute` in Corporate Knowledge Assistant: realizza UNA volta i seam
  progettati da wiring-design in §5 di `0X-WIRING.md`, intra-fase (innesti
  per-slice pre-registrati, registri espliciti, mount point del Router Axum,
  funzioni di composizione per feature, membri del workspace, servizi Compose)
  e inter-fase (trait, registri, mount point e segnaposto nudi per le fasi
  successive, creati SOLO vuoti). Verifica che i PLAN restino plug-in-only, porta
  build e test al verde, committa la fondazione e segna i seam `created` nella
  WIRING-MAP. USARE come wave 0 di `execute`, prima delle onde delle slice; o
  quando l'utente chiede "crea i wiring point", "realizza i seam", "prepara il
  wiring per il parallelo".
---

# wiring-build — realizzare i wiring point

Terzo passo della triade di wiring, eseguito come **wave 0 di `execute`** (scrive codice, quindi non sta in `plan`). Lo esegue un esecutore Claude (`gsd-executor`) con il brief stretto: §5 e §4 di `0X-WIRING.md`, le righe della WIRING-MAP con `created-in = X`, e i comandi di verifica sotto. Scopo: creare i seam una volta sola, così che ogni slice successiva sia un plug-in additivo che tocca soltanto file propri.

## Input

- `0X-WIRING.md` §5 (seam da creare, con la forma vuota esatta e le righe di innesto) e §4 (vincoli);
- `.planning/WIRING-MAP.md`, righe non `retired` con `created-in = X` e stato `promised`;
- i `PLAN.md` della fase, committati, per il controllo plug-in-only.

## Procedura

### Step 1 — Preflight: i piani sono plug-in-only?
Ricalcola la matrice `file → {piani}` sui `files_modified` reali (`node scripts/estrai.cjs piani <dir-fase>`). Ogni file condiviso da due o più piani deve essere:
- un seam di §5 che la wave 0 crea e pre-registra, dopo il quale i piani toccano solo i propri file per-slice; oppure
- un append dichiarato in §3, con i piani già in onde diverse in `0X-ONDE.md`; oppure
- un artefatto `regenerate` con un solo contributore per onda.

Se un piano edita righe condivise non coperte da §3-§5, **non** si riscrive il piano qui: dopo gate e audit il PLAN è congelato (lo sha conta per `plan-gate` sulle fasi critiche). È una deviazione (b): STOP e rimbalzo a `plan` con il punto scoperto.

### Step 2 — Seam intra-fase (create-once)
Per ogni voce intra-fase di §5 realizza lo scaffolding vuoto-ma-pronto secondo la convenzione di `wiring-design`. Esempi tipici della forma Rust/Axum/React:
- membri nuovi in `[workspace] members` e dipendenze note della fase in `[workspace.dependencies]` del `Cargo.toml` radice; crate nuovo con `Cargo.toml` e `src/lib.rs` o `src/main.rs` minimi;
- composition root del servizio: `src/main.rs` minimo e modulo `app` con una riga `.merge(<feature>::compose(&infra))` per ogni slice della fase, e per ciascuna il file `src/app/<feature>.rs` con la funzione di composizione che restituisce un `Router` vuoto;
- mount point `.nest("/api/v1/<area>", …)` per le aree della fase;
- registri trait+chiave vuoti (tipo, `register`, `get`), con le sole registrazioni che la wave 0 stessa può fare senza implementare logica di slice;
- `services/<svc>/migrations/` con la convenzione di numerazione annotata (la cartella, non migrazioni di slice);
- in `apps/web`: `src/app/routes.tsx` con lo spread di `features/<feature>/routes.tsx` per ogni feature della fase, ciascuno con un array vuoto; indice MSW analogo, se MSW è adottato;
- registro degli eventi del servizio (`src/events/registry.rs`) vuoto o con i soli eventi già fissati nel contratto di fase;
- servizi Compose in `infra/` e sezioni di `.env.example` per i container che la fase introduce.

### Step 3 — Seam inter-fase (solo vuoti)
Per ogni voce inter-fase di §5 (riga `WM-NNN` con `created-in = X`) crea **soltanto la forma vuota**: il trait con le firme promesse, il tipo registro, il mount point con la funzione di composizione vuota, la cartella o la convenzione, al più un commento nudo `// fase Y: <seam> (WM-NNN)`. È il confine con `perimetro-check`: il contenuto della fase futura è vietato. Quindi niente adapter, fake, test, DTO, rotte, colonne, migrazioni, chiavi di configurazione, voci di menu o copy per l'area futura. Se la forma vuota non compila senza contenuto futuro (per esempio un trait che richiede tipi non ancora definiti), fermati e rimbalza a `plan`: la forma promessa va rivista, non riempita.

### Step 4 — Verifica
Sulla fondazione, a worktree pulito:
```bash
# Rust: solo se esiste Cargo.toml alla radice, altrimenti stampa "Rust: non applicabile (nessun Cargo.toml)"
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
# Web: solo se esiste apps/web/package.json, altrimenti stampa "Web: non applicabile (nessun apps/web/package.json)"
(cd apps/web && npm run lint && npx tsc --noEmit && npx vitest run)
```
Uno step si salta **solo** se il manifest relativo non esiste ancora, e lo si stampa. Nessun bypass, né variabili d'ambiente né percorsi esentati. Poi `crate-boundary-check` e `solid-check` sulla fondazione: un seam che dipende da un altro servizio, o un registro sostituito da un `match` sul tipo, non è pronto.

Ricalcola infine la matrice dello Step 1: ogni punto deve risultare **creato** (wave 0) o **posseduto da una sola slice**; l'unico residuo condiviso ammesso sono gli append e gli artefatti `regenerate` dichiarati in §3.

### Step 5 — Commit e registro
1. Commit della fondazione, in italiano: `chore(0X-wiring): seam create-once intra e inter-fase`. Mai `--no-verify`.
2. WIRING-MAP: le righe create passano da `promised` a `created`, con lo sha del commit nelle note; una riga promessa che non si è potuta creare resta `promised` con il motivo. Variazione registrata in coda al file.
3. §6 di `0X-WIRING.md`: commit, seam creati, matrice ricalcolata, slice dichiarate plug-in-ready.
4. Commit docs: `docs(0X-wiring): seam creati, WIRING-MAP aggiornata`.

Lo stato `filled` non lo imposta `wiring-build`: lo riconcilia `verify` confrontando i seam reali con la WIRING-MAP.

## Vincoli e confini

- `wiring-build` crea soltanto seam e innesti; non implementa la logica delle slice, che resta alle onde parallele. La fondazione è deliberatamente sottile.
- Ogni seam rispetta OCP e i confini dei crate: una slice o una fase futura si innesta aggiungendo un file e una registrazione, mai modificando il seam.
- Divieti dell'esecutore di wave 0: `STATE.md`, `ROADMAP.md`, `ORCHESTRATOR-STATUS.md`, `--no-verify`, file fuori da §5 senza deviazione registrata.
- A fine fase `verify` riconcilia seam reali e WIRING-MAP; i punti di giunzione emersi in corsa entrano nella mappa da lì, non con un ritocco silenzioso.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
