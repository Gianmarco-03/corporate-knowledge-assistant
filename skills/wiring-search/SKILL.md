---
name: wiring-search
description: >
  Trova TUTTI i wiring point (punti di giunzione) di una fase di Corporate
  Knowledge Assistant, su due assi. Intra-fase: i file e i simboli condivisi che
  più slice della stessa fase toccano (membri del workspace Cargo, composition
  root dei servizi, mount point del Router Axum, migrazioni sqlx per servizio,
  spec OpenAPI e client TS generato, route React, handler MSW, servizi Compose,
  chiavi di `.env.example`, registri degli eventi di audit e outbox). Inter-fase:
  i seam che la fase deve predisporre per le fasi successive, letti dai soli goal
  del ROADMAP, e quelli già promessi nella WIRING-MAP che la fase deve riempire.
  Produce l'inventario (§1-§2 di `0X-WIRING.md`) con contributori, tipo di
  contributo e rischio. USARE come primo passo del wiring dentro `plan` (passo 3,
  prima dei piani), o quando l'utente chiede "cerca i wiring point", "trova le
  giunzioni tra slice", "quali seam deve preparare la fase X", "wire search".
---

# wiring-search — trovare i wiring point di una fase

Primo passo della triade di wiring (`wiring-search` → `wiring-design` → `wiring-build`), dentro il verbo `plan` al passo 3, **prima** che i piani esistano (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md`, §4 e §5). Scopo: rendere esplicito ogni punto in cui più contributi devono convergere sullo stesso file o simbolo, così che `wiring-design` ne progetti la convenzione additiva e `wiring-build` (wave 0 di `execute`) li crei prima delle slice.

Lavora su due assi:
- **intra-fase**: slice della fase X che, eseguite in parallelo, toccherebbero le stesse righe;
- **inter-fase**: seam che X deve **predisporre** perché una fase successiva vi si innesti senza riaprire il codice di X, e seam che una fase precedente ha già **promesso** e che X deve **riempire**. Il registro di progetto di questi seam è `.planning/WIRING-MAP.md`.

`wiring-search` trova e classifica soltanto: non progetta, non scrive codice, non modifica la WIRING-MAP né i piani.

## Perché esiste

In un workspace con tre servizi, un frontend e un database condiviso nel cluster, N slice verticali condividono inevitabilmente dei seam: il `Cargo.toml` del workspace, il composition root che costruisce il `Router` e inietta gli adapter dietro i trait, la sequenza delle migrazioni di un servizio, la spec OpenAPI e il client TypeScript rigenerato, il registro delle route React. Se ogni slice edita le stesse righe, l'esecuzione parallela collide al merge. Lo stesso vale fra fasi: se la fase che costruisce l'ingestion dei PDF non predispone il punto di estensione delle fonti, la fase delle fonti online dovrà riaprire i casi d'uso dell'ingestion, violando Open-Closed. Non si possono progettare convenzioni per punti che non sono stati prima **trovati e classificati**.

## Input

- `0X-CONTEXT.md` (decisioni e vincoli già estratti dalla WIRING-MAP in `discuss`);
- `0X-RESEARCH.md`, in particolare le **slice probabili** e i file che toccherebbero;
- `0(X-1)-CONTRATTO.md`, se esiste (pipeline sfalsata: si pianifica contro il contratto, non contro la codebase di X-1);
- `.planning/WIRING-MAP.md` (registro persistente; se manca, si annota e si prosegue: la crea `wiring-design` dal template);
- i **goal** delle fasi successive, uno per fase, mai il ROADMAP intero;
- la codebase reale, se esiste già codice (`Cargo.toml`, `services/*`, `crates/*`, `apps/web`, `infra/`);
- i `PLAN.md` della fase, **solo** se esistono già (`plan --gaps`, rilancio dopo un rimbalzo).

## Procedura

### Step 0 — Preflight
- Cartella di fase: `node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" find-phase X`.
- WIRING-MAP: presente o assente (annotalo nell'intestazione del manifest).
- Stato del codice: `Cargo.toml` alla radice e `apps/web/package.json` esistono? In loro assenza i seam corrispondenti sono tutti da creare (create-once), mai «già fatti».

### Step 1 — Raccogli le slice
- **PLAN esistenti**: leggi frontmatter e `<tasks>` con `node scripts/estrai.cjs piani <dir-fase>` (`files_modified`, `depends_on`, blocchi «shared-file touches» nel corpo solo se il piano li cita).
- **PLAN non ancora scritti** (caso normale): usa le slice probabili di `0X-RESEARCH.md`, correggendole dove CONTEXT e CONTRATTO X-1 le contraddicono, e scrivi per ciascuna l'elenco dei file e simboli che toccherebbe. Le correzioni si annotano con il motivo.

### Step 2 — Calcola l'intersezione intra-fase
Costruisci la matrice `file o simbolo → {slice che lo toccano}`. Un punto toccato da **≥ 2 slice** è un wiring point candidato. Glob compresi: `services/document-service/migrations/*` è un punto unico, non N file.

### Step 3 — Aggiungi i seam che lo sono per natura
Alcuni punti sono wiring point anche se oggi li tocca una sola slice, perché ogni slice o fase futura dovrà contribuirvi. Includili sempre, con il percorso reale se esiste o quello previsto da `README.md` §Struttura prevista:
- **membri del workspace Cargo**: `Cargo.toml` radice (`[workspace] members`, `[workspace.dependencies]`) e `Cargo.lock` (rigenerato);
- **composition root per servizio**: `services/<svc>/src/main.rs` e il modulo `app` (`src/app.rs`, `src/app/*.rs`), dove si costruisce il `Router` e si iniettano gli adapter dietro i trait-porta (`DocumentParser`, `Embedder`, `VectorIndex`, `DecisionEngine`, `AccessPolicy`, `AuditSink`, `Cache`, …; elenco in `docs/architecture/interfaces-and-concurrency.md` §Porte Rust interne);
- **mount point del Router Axum**: `nest`/`merge` per area (`/api/v1/auth`, `/api/v1/documents`, `/api/v1/queries`, `/internal/v1/...`);
- **registri trait+chiave** per varianti di uno stesso comportamento (parser per tipo di media, fonti di documenti, adapter del `DecisionEngine`): ovunque una slice o una fase futura aggiunga una variante;
- **sequenza delle migrazioni** per servizio: `services/<svc>/migrations/` (sqlx), più la cache offline `.sqlx/` se adottata (rigenerata);
- **OpenAPI per servizio** (spec e tag) e **client TS generato** in `apps/web` (rigenerato);
- **route React Router** e navigazione data-driven in `apps/web`, e il **registro degli handler MSW** se il progetto usa MSW;
- **`infra/` Compose**: un servizio per container (Postgres+pgvector, i tre servizi);
- **`.env.example`** e la struttura di configurazione di ogni servizio (chiavi come `DATABASE_URL`, `TYPESAFE_API_KEY`, chiavi JWT);
- **registro degli eventi di audit** e **registro degli eventi di outbox** (tipi evento, versioni, consumatori);
- **`crates/contracts`** e **`crates/telemetry`**: l'unico seam fra servizi ammesso dai confini (`crate-boundary-check`);
- `apps/web/package-lock.json` (rigenerato) quando più slice aggiungono dipendenze npm.

### Step 4 — Asse inter-fase
1. **Promesse da riempire o predisporre.** Dalla WIRING-MAP estrai le righe non `retired` con `created-in = X` (seam che X deve **creare vuoto** in wave 0) e quelle con X fra i `filled-by` (seam che X deve **riempire**). Per ognuna: esiste nel codice (o nel contratto X-1) con il percorso dichiarato? Una riga `created` che non si trova nel codice è un **segnale** da riportare (la riconcilia `verify`), mai da correggere qui.
2. **Goal delle fasi successive — solo i goal.** Elenca i numeri delle fasi successive a X e leggi per ciascuna il solo goal:
   ```bash
   node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" roadmap analyze --pick phases   # solo per i numeri delle fasi
   node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" roadmap get-phase N --pick goal  # una chiamata per fase N > X
   ```
   Mai `cat`/`Read` di `ROADMAP.md`, mai i success criteria o i dettagli delle fasi future: il wiring prepara la **forma** dell'aggancio, non il contenuto di fasi non ancora discusse.
3. **Candidati.** Un seam inter-fase è candidato solo se **(a)** un goal futuro estende un comportamento che X costruisce (nuova variante, nuova rotta sullo stesso servizio, nuova tabella nello stesso servizio, nuovo tipo di evento sullo stesso registro) e **(b)** senza il seam quella fase dovrebbe modificare l'interno del codice di X. Ogni candidato cita il goal letterale che lo motiva e il file di X dove vivrebbe. Niente candidati speculativi senza un goal che li nomini.
4. Se un goal futuro tocca un'area che X non costruisce affatto, **non** è un seam di X: sarà la fase futura a creare il proprio punto di aggancio. Annotalo come «nessun seam per X».

### Step 5 — Classifica ogni punto
- **Kind** (tassonomia della WIRING-MAP): `trait-port | registry | router-mount | migration-sequence | openapi-tag | ts-client | frontend-route | audit/outbox-event-registry | workspace-member | compose-service | config-key`.
- **Tipo di contributo**:
  - `create-once`: creato una volta (wave 0 o slice proprietaria), gli altri consumano (trait-porta, registro vuoto, membro del workspace, servizio Compose);
  - `append`: ogni slice aggiunge una voce in coda (route, handler MSW, tipo evento, chiave di configurazione);
  - `register`: ogni slice registra un adapter o una variante nel composition root (`registry.register(...)`, `.merge(router)`);
  - `regenerate`: artefatto prodotto da uno strumento e mai editato a mano (client TS, `Cargo.lock`, `package-lock.json`, `.sqlx/`); la rigenerazione concorrente è un punto di serializzazione;
  - `shared-artifact`: file condiviso mutato da più slice (spec OpenAPI del servizio, `Cargo.toml` radice, `compose.yaml`).

### Step 6 — Assegna il rischio di collisione
ALTO (stesse righe, o rigenerazione concorrente), MEDIO (append nello stesso blocco dello stesso file), BASSO (file distinti per slice, o create-once già fatto). Ricorda che `onde-slice` serializza **qualunque** coppia di piani con un file in comune: un append condiviso, anche a rischio di merge MEDIO, toglie comunque il parallelo.

### Step 7 — Scrivi l'inventario
Output: `.planning/phases/<dir-fase>/0X-WIRING.md`, in **caveman full** (regola degli intermedi, spec §2). La struttura del file è fissa e la condividono le tre skill; `wiring-search` scrive l'intestazione, §1 e §2 e lascia le altre sezioni come titoli vuoti.

```markdown
# 0X — WIRING
Redazione · input letti (con sha di main) · WIRING-MAP presente/assente · stato del codice

## §1 — Inventario intra-fase
| id | punto (file/simbolo) | kind | contributo | slice | rischio |
|---|---|---|---|---|---|
| W1 | ... | ... | ... | ... | ... |
Riepilogo: n ALTO/MEDIO/BASSO; slice genuinamente file-disgiunte.

## §2 — Seam inter-fase
| rif | punto | kind | azione per X (crea vuoto / riempie / consuma) | fase futura o d'origine | motivo (goal citato o riga WIRING-MAP) |
|---|---|---|---|---|---|
| WM-007 o cand-1 | ... | ... | ... | ... | ... |
Segnali: righe WIRING-MAP non trovate nel codice; goal futuri senza seam in X.

## §3 — Convenzioni e plug-in contract   (wiring-design)
## §4 — Vincoli per gli esecutori        (wiring-design)
## §5 — Seam da creare in wave 0          (wiring-design → wiring-build)
## §6 — Esito della wave 0                (wiring-build)
```

## Output e passo successivo

L'inventario è l'input di **`wiring-design`**, che sceglie la convenzione per ogni punto, decide quali candidati inter-fase diventano promesse e aggiorna la WIRING-MAP. Qui non si progetta, non si crea nulla e non si toccano WIRING-MAP, piani o codice.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
