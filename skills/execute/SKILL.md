---
name: execute
description: Terzo verbo del loop GSD+ECC di Corporate Knowledge Assistant — esegue le onde di una fase con esecutori Claude (gsd-executor, metodo ecc:tdd-guide) in worktree isolati, al massimo 2 concorrenti; prima di ogni piano un agente Claude fresco, distinto dall'esecutore, scrive i test del design e li porta al rosso; wave 0 crea i seam di wiring (anche inter-fase); integra ogni onda e chiude OGNI piano col bridge che lo rende visibile a GSD (SUMMARY + STATE + ROADMAP). USARE dopo plan X ("esegui la fase X", "/execute X", anche "--onda N" per una sola onda, "--gaps" per i piani di gap), o per riprendere un'esecuzione interrotta.
---

# execute

Terzo verbo del loop GSD+ECC (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md` §5 e §7). L'implementazione è di agenti Claude con metodo ECC; GSD non tocca palla fino al bridge. gsd-tools: `node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs"`.

## Confini di delega

- **Implementazione**: esecutori `gsd-executor` (agenti Claude, contratto utente in `~/.claude/agents/`), uno per slice, ciascuno nel proprio worktree, con `ecc:tdd-guide` come protocollo. Build fix: `ecc:rust-build-resolver` per il workspace Cargo, `ecc:react-build-resolver` per `apps/web`, sempre a diff minimo.
- **`test-write`**: un agente Claude **fresco e distinto dall'esecutore** del piano, che scrive solo nei `allowed_test_paths`.
- **Codex**: lo slot di implementazione e di `test-write` esiste nel metodo ma in questo progetto è **spento per decisione utente**; nessun passo lo invoca.
- **Home-made**: worktree, orchestrazione onde, `integratore-onda`, bridge, stato.
- **GSD**: riceve — SUMMARY, STATE e ROADMAP li scrive SOLO il bridge, coi verbi gsd-tools.

## Passi

1. **Preflight** — STATUS letto; pretende: PLAN **committati** (i worktree devono vederli), gate tutti `conforme` (dal `plan`), un `0X-YY-TEST-DESIGN.md` committato per ogni piano, `0X-ONDE.md` presente. Sulle **fasi critiche** (classificazione nel CONTEXT) aggiunge `READ-GATE` su AgentBridge per ogni piano dell'onda: si procede solo con `TEST_SPEC_READY` o `EXEC_READY`; con `STALE`, `REVISION_REQUIRED`, `RESEARCH_PENDING` o `BLOCKED` il piano torna a `plan`, mai forzato. **Safe-resume**: per ogni piano con commit nel log (`git log --grep="(0X-YY)"`) ma senza SUMMARY → instradalo al bridge in modalità `--da-prove`, MAI ri-eseguirlo.
2. **Worktree** — creati a mano dal `main` locale, perché `isolation: "worktree"` parte da `origin/HEAD` e non dal main locale:
   - fase critica: il branch del piano esiste già da `plan` → `git worktree add .claude/worktrees/0X-YY task/PLAN-<fase>-<piano>`;
   - fase non critica: `git worktree add -b exec/0X-YY .claude/worktrees/0X-YY main`.
   Ogni brief porta il controllo «HEAD = <hash atteso>, altrimenti fermati».
3. **`test-write`** — prima dell'implementazione, per ogni piano, un agente Claude FRESCO (mai `fork`, mai l'esecutore del piano, mai una ripresa) riceve il test design del piano e gli `allowed_test_paths` (i soli percorsi di test fra i `files_modified` del PLAN). Scrive i test `TEST-N` nel worktree del piano, li porta al rosso e committa con scope `test(0X-YY)`. Esiti, registrati in `.planning/tmp/test-write-0X-YY.yaml`:
   - `RED_EXPECTED` → parte l'implementazione;
   - `RED_UNEXPECTED` (rosso per il motivo sbagliato: non compila, fixture rotta) → nuovo `test-write` mirato, con un agente nuovo;
   - `GREEN_UNEXPECTED` (verde prima di implementare) → Claude lo esamina: test debole → nuovo test design in `plan`; comportamento già esistente → annotato, e il test resta come regressione.
   Sulle fasi critiche i commit del `test-write` sono attesi e **non** si ri-registra il piano per loro, altrimenti l'audit diventa `STALE`. L'esito si registra sul `plan-gate` secondo `C:\Agent Bridge\docs\contracts.md`; se il contratto non ammette un `test-write` eseguito in locale da Claude, il gate resta in `TEST_SPEC_READY` e l'evidenza dell'esito è il file yaml sopra, citato nel SUMMARY.
4. **Wave 0 — fondazione di wiring** — un esecutore realizza `wiring-build` sui seam elencati in §5 di `0X-WIRING.md`: i seam condivisi della fase creati UNA volta e committati (registry espliciti, mount point del router Axum, trait-porta, membri del workspace, sequenza delle migrazioni, route del frontend, tipi in `crates/contracts`), più i seam **inter-fase** che la WIRING-MAP assegna alla fase X da predisporre — questi **solo vuoti**. `wiring-build` compila §6 (esito di wave 0) e porta le righe della WIRING-MAP da `promised` a `created` con lo sha. **Non riscrive i PLAN**: un piano che si scopre non plug-in-only rispetto a §3-§4 è una deviazione (b), rimbalzo a `plan`. Da qui ogni slice è un plug-in additivo.
5. **Onde** — per ogni onda di `0X-ONDE.md`, esecutori concorrenti con tetto `workflow.max_esecutori_onda`, **default 2**, uno per slice, ciascuno nel proprio worktree.
   **Modello per natura del piano**: `model: "sonnet"` per i piani meccanici — configurazione, DTO, mappature, componenti UI senza regole di dominio; **Opus** (nessun `model=`, eredita) per i piani che toccano autenticazione o RBAC, la regola no-leak, un oracolo di evaluation o una soglia, un confine fra crate, una migrazione di dati o un seam della WIRING-MAP. In dubbio → Opus, e la ragione va scritta nel brief. Ogni esecutore carica `Skill("caveman")` (livello full) all'avvio: rapporto YAML e narrazione finale in caveman, codice, commit e SUMMARY in stile repo.
   **Brief d'ingresso stretto**: il PLAN integrale, **§4 di `0X-WIRING.md`** (vincoli per gli esecutori: l'unica sezione del WIRING che entra nel brief), il **test design** del piano (e, sulle fasi critiche, i rischi MEDIUM/LOW dell'audit `APPROVED_WITH_WARNINGS`), il «Next Phase Readiness» del SUMMARY dell'onda prima. Blueprint, CONTRATTO, WIRING-MAP e le altre sezioni del WIRING si leggono **solo per puntatore**, alla sezione che il PLAN cita.
   **Metodo**: TDD (`ecc:tdd-guide`: rosso → verde → refactor) partendo dai test del `test-write`; **commit atomici per task** in italiano, scope `(0X-YY)`. Il piano è verde solo quando lo sono i test del design: nessuno si toglie o si indebolisce per far passare l'implementazione — farlo è una deviazione (b). Build rossa → build-resolver dedicato, diff minimo.
   **Correzioni: agente nuovo, mai ripresa.** Un difetto da correggere dopo il rapporto si affida a un esecutore **nuovo** con un compito stretto: la ripresa ricarica l'intero transcript.
   **Dipendenze d'onda**: se un piano dell'onda N dipende da codice dell'onda N−1, si fonde `main` nel branch del piano (merge, mai rebase; sulle fasi critiche senza ri-registrarlo). Se quel codice contraddice le premesse del piano, è deviazione (b).
   **Contratto d'uscita**: rapporto su file `.planning/tmp/rapporto-0X-YY.yaml` (schema sotto) scritto PRIMA di ogni narrazione finale, così sopravvive ai crash; campi liberi in caveman full, hash, test ref ed errori verbatim.
   **Divieti per gli esecutori**: `STATE.md`/`ROADMAP.md`/`ORCHESTRATOR-STATUS.md`/`WIRING-MAP.md`, `--no-verify`, uscire dai `files_modified` senza deviazione registrata nel rapporto.
6. **Fine onda** — **onda con PIÙ slice**: spawn di `integratore-onda` (contratto in `.claude/agents/integratore-onda.md`): merge nell'ordine di `0X-ONDE.md`, conflitti motivati, gate d'onda completo sul main fuso, pulizia dei worktree. Rosso di merito → rimbalzo alla slice colpevole. **Onda con UNA sola slice**: nessun integratore; il merge `--no-ff` lo fa l'orchestratore e il gate d'onda si esegue **una sola volta, sul `main` fuso**.
   **Gate d'onda** (ogni passo si salta SOLO se il relativo manifest non esiste ancora, e lo si stampa come «non applicabile»; nessun bypass, né variabili d'ambiente né percorsi esentati):
   - Rust, se esiste `Cargo.toml` alla radice: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`;
   - web, se esiste `apps/web/package.json`, in `apps/web`: `npm run lint`, `npx tsc --noEmit`, `npx vitest run`.
   I test d'integrazione che richiedono PostgreSQL+pgvector girano sul database locale di Docker Desktop (Compose del repo), mai su un remoto.
7. **Bridge per piano** — SOLO a merge verde, per ogni slice dell'onda (sezione sotto).
8. **Stato** — `update-orchestrator-state --exec` a fine onda, più `APPEND-STATUS` sulla dashboard AgentBridge a ogni tappa reale; prossima azione: onda successiva o `/verify X`.

## Il bridge (per piano)

Rende il piano visibile a GSD. Cinque passi, o tutto o niente; idempotente (SUMMARY presente → no-op).

1. **Raccolta prove** — commit del piano dal log (scope `(0X-YY)`); diff reale vs `files_modified` del PLAN; esiti test. **Rifiuta** (senza scrivere nulla, con `rifiutato: <motivi>`) se: zero commit, file dichiarati non toccati, test rossi.
2. **Rapporto** — leggi `.planning/tmp/rapporto-0X-YY.yaml` e **incrocia ogni affermazione con le prove** del passo 1 — mai fidarsi del racconto. Rapporto assente → modalità `--da-prove`: ricostruisci da git + PLAN + esiti test; i deliverable non dimostrabili diventano `human_judgment: true`.
3. **Compilazione SUMMARY** — `0X-YY-SUMMARY.md` dal template GSD (`~/.claude/gsd-core/templates/summary.md`): frontmatter completo (`requirements-completed` copiati dal PLAN), blocco `coverage` popolato dai must_haves con `verification[].ref` e `status` reali. Il `coverage` collega ogni `TEST-N` del design al proprio `ref` e al rischio `RISK-N` che cattura; un `TEST-N` senza prova è `human_judgment: true`, mai verde implicito. In dubbio → `human_judgment: true` con rationale (fail-safe GSD).
4. **Autovalidazione** — `gsd-tools verify-summary <path>`; se boccia: correggi e ripeti.
5. **Registrazione atomica** — `gsd-tools roadmap update-plan-progress 0X YY complete`; aggiornamento posizione via verbi `state`; commit docs: `docs(0X-YY): completato il piano YY — <one-liner sostanziale>`. A registrazione avvenuta, elimina i file tmp del piano.
   **Decisioni = una riga**: `state add-decision` porta SOLO il sommario (≤ 200 caratteri) col puntatore `→ \`file#sezione\`` all'artefatto che porta il rationale; la stessa riga si accoda a `.planning/DECISIONS-INDEX.md` sotto la fase. Mai rationale in STATE.md. Prima del commit: `node scripts/lint-stato.cjs` deve essere verde.

### Schema del rapporto (`.planning/tmp/rapporto-0X-YY.yaml`)

```yaml
piano: 0X-YY
commit:                      # nell'ordine, hash + messaggio
  - {hash: abc123f, msg: "test(0X-YY): ..."}
accomplishments:             # cosa è stato costruito, righe sostanziali
  - "..."
decisioni:                   # prese durante l'esecuzione, con motivo
  - "..."
deviazioni:                  # livello a (auto-fix) / b (MAI qui — va in rimbalzo) / c (interruzione)
  - {livello: a, cosa: "...", fix: "...", commit: abc123f}
coverage:                    # deliverable → prova
  - {deliverable: "...", ref: "services/document-service/tests/retrieval.rs#nome_test", esito: pass}
note: "..."
```

## Deviazioni a tre livelli

- **(a) dentro il piano** (fix necessario, dipendenza mancante) → auto-fix, registrato nel rapporto e nel SUMMARY, si prosegue.
- **(b) tocca `0X-CONTRATTO.md`, un oracolo o una soglia di evaluation, oppure un seam della WIRING-MAP** fuori da quanto §4 di `0X-WIRING.md` consente (crearlo, cambiarne la forma, riempirlo), o il piano risulta non plug-in-only → STOP della slice, rimbalzo a `plan` — mai assorbita in silenzio.
- **(c) interruzione esterna** (rate limit, crash) → `update-orchestrator-state --evento`; alla ripresa il preflight (safe-resume) riparte dal primo task non committato.

## Cosa NON fa

Non giudica la qualità (è di `verify`), non chiude la fase, non fa UAT, non decide nulla.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
