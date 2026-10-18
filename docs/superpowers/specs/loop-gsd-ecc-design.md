# Loop GSD+ECC — design del livello di orchestrazione di Corporate Knowledge Assistant

**Data**: 2026-10-18 · **Stato**: adottato per decisione utente, porting del metodo di Gestionale Light
**Ambito**: tutte le fasi GSD di Corporate Knowledge Assistant (CKA). Nessuna fase esiste ancora: il bootstrap è `/gsd:ingest-docs` sui documenti già scritti (`README.md`, `DEPENDENCIES.md`, `docs/architecture/`, `docs/adr/`, `deliverables/checklist.md`), senza una fase 0 di ricerca di mercato.
**Contratti operativi**: `skills/<verbo>/SKILL.md` (canonici) con mirror markdown in `.claude/skills/`.

## 1. Problema e principio

Convivono due plugin: **GSD** (requisiti, roadmap, fasi, piani, stato) ed **ECC** (agenti di implementazione e verifica ingegneristica). Senza un livello di collegamento gli esecutori chiudono piani che GSD non vede (nessun SUMMARY → ri-esecuzioni duplicate) e i due verifier possono contraddirsi. Questo livello home-made li collega senza toccare gli interni di nessuno dei due.

**Tabella di comando**:

| Funzione | Chi comanda |
| --- | --- |
| Requisiti, roadmap, fasi, piani, stato; redazione dei documenti di ricerca | **GSD / orchestratore Claude** |
| Ricerca esterna (specifiche, standard, librerie, UX) | **Claude** per default (WebSearch/WebFetch, context7, `anthropic-skills:deep-research`), con fonti citate per URL e data di accesso; **ChatGPT Deep Research** via AgentBridge solo su richiesta dell'utente |
| Architettura locale, implementazione, TDD, build fix, code review, security review, e2e | **ECC** (metodo e reviewer) con esecutori Claude (`gsd-executor`) |
| Revisione dei rischi e disegno dei test sulle fasi critiche; verificatore esterno degli oracoli di evaluation | **GPT** via AgentBridge |
| Gate di progetto sui piani | **skill home-made** (né GSD né ECC) |
| Valori di dominio, oracoli, confini di prodotto, giudizi grafici | **Utente**, sempre |

**Codex**: lo slot di implementatore e di scrittore dei test rossi esiste nel metodo, ma in CKA è **spento per decisione utente**. Nessun verbo lo invoca.

**Ordine dei verifier (mai invertire, mai doppiare)**: ECC → verifica ingegneristica (test/lint/build, code review, security, e2e, regressione di evaluation) → GSD → verifica di obiettivo («abbiamo davvero completato la fase?»).

## 2. Architettura

**4 verbi** — `discuss | plan | execute | verify` — che ripiegano su skill esistenti (GSD, ECC, gate di progetto), più le skill di servizio:
- `onde-slice`, `update-orchestrator-state`, `ricerca-punti-aperti`, `deep-research-handoff`;
- la triade di wiring `wiring-search` → `wiring-design` → `wiring-build`;
- i gate `solid-check`, `crate-boundary-check`, `security-scanner`, `eval-oracle-check`, `perimetro-check` (`scripts/gate-perimetro.cjs`);
- il contratto agente `integratore-onda` (`.claude/agents/integratore-onda.md`, unico agente vendorizzato).

Gli agenti `gsd-*` **non sono vendorizzati**: si usano quelli dell'installazione utente (`~/.claude/agents/gsd-*`, GSD 1.42.3, `gsd-tools` in `$HOME/.claude/gsd-core/bin/gsd-tools.cjs`). Il bridge SUMMARY è un **passo interno di `execute`**, non una skill.

Collocazione: codice canonico in `skills/`, mirror solo-markdown in `.claude/skills/` (MAI script duplicati). Stato dell'orchestratore in `.planning/ORCHESTRATOR-STATUS.md`, registro dei seam inter-fase in `.planning/WIRING-MAP.md`, entrambi versionati.

Aggancio a GSD: **solo via `gsd-tools`** — mai toccare gli interni di gsd-core; `/gsd-execute-phase` resta intatto ma il loop non lo usa.

**Regola strutturale anti-trappola**: ogni verbo committa i propri artefatti alla chiusura. I documenti di fase non restano mai untracked, perché i worktree degli esecutori non vedrebbero i PLAN.

**Regola caveman**: ogni **risultato intermedio** del loop — brief di ricerca (`0X-RICERCA-DISCUSSIONE.md`), rapporti degli agenti tecnici (integratore-onda, campi liberi del rapporto YAML degli esecutori), `0X-WIRING.md`, `0X-ONDE.md`, verdetti dei gate, voci di ORCHESTRATOR-STATUS — si scrive in **stile caveman full** (plugin `caveman`): niente articoli né riempitivi, una idea per frase, sostanza tecnica intatta; codice, comandi, numeri, citazioni ed errori restano VERBATIM. Restano in prosa normale: i deliverable dei verbi (CONTEXT, PLAN, test design, CONTRATTO, SUMMARY, VERIFICATION), i messaggi di commit e ogni testo rivolto all'utente. `.caveman.json` alla radice tiene la sessione principale in `off`: i verbi parlano in prosa con l'utente, gli agenti tecnici caricano la skill da soli.

**Modelli degli agenti**: esecutori per natura del piano — `sonnet` per i piani meccanici, Opus per autenticazione/RBAC, regola no-leak, oracoli e soglie di evaluation, confini fra crate, migrazioni, seam della WIRING-MAP (in dubbio Opus, con la ragione nel brief). `gsd-verifier` e `gsd-plan-checker` restano al profilo GSD.

**Contesto minimo e decisioni come indice**: `STATE.md` ≤ 20 KB; `state add-decision` porta solo un sommario ≤ 200 caratteri + puntatore; `.planning/DECISIONS-INDEX.md` ha una riga per decisione ed è letto da `gsd-discuss-phase` al posto dei CONTEXT precedenti; il goal di fase si prende con `gsd-tools roadmap get-phase X`, mai con `cat` dell'intera ROADMAP. Guardiano: `scripts/lint-stato.cjs`, in `verify` e nel bridge. Checker, gate e verifier leggono la parte strutturata degli artefatti con `scripts/estrai.cjs` (frontmatter, `<tasks>`, `coverage`, blocchi nominati); il corpo intero si apre solo per motivare un rosso.

## 3. `discuss`

Scopo: aprire una fase con decisioni **fondate su fonti** e lasciare uno stato ripartibile.

1. **Preflight** — legge/crea ORCHESTRATOR-STATUS; `find-phase`; X-1 non verificata → rischio annotato, non blocca (§8). Goal via `roadmap get-phase X`; decisioni da `DECISIONS-INDEX.md`; **punti ereditati** dai `deferred-items.md` delle fasi precedenti che nominano X; **WIRING-MAP**: seam che la mappa assegna a X da riempire o da predisporre, portati nel CONTEXT come vincoli. **Classificazione di criticità**: la fase è critica se tocca identity/autenticazione, RBAC con retrieval autorizzato o ingestion (upload, parsing, fonti online); ratificata dall'utente con il CONTEXT.
2. **Ricerca decisionale a tre lenti** — per ogni area che richiede nuova evidenza: **fonte autorevole** (RFC, OWASP, specifiche, norma, e le decisioni già scritte del repo), **pratica consolidata**, **riferimenti di prodotto** (facoltativa, solo per scelte di scope). Canale di default Claude, con fonti per URL e data di accesso; GPT su richiesta dell'utente. Esce `0X-RICERCA-DISCUSSIONE.md` in caveman.
3. **Discussione fondata** — delega a `gsd-discuss-phase X`; ogni domanda arriva armata del finding («La fonte X dice così. Mi allineo?» · «I sistemi comparabili offrono X. In scope?» · «La pratica consolidata è X. La usiamo?»). Mai una domanda nuda; ciò che la regola dei dubbi di `CLAUDE.md` dichiara sicuro non si chiede.
4. **Raccolta** — `0X-CONTEXT.md`; ogni decisione bloccata con la fonte che la fonda; senza fonte → TO-DO con assunzione provvisoria, mai scelta silenziosa.
5. **Stato** — `update-orchestrator-state --discuss`; prossima azione `/plan X`.
6. **Commit** — `docs(0X): contesto di fase discusso`.

Confine: `discuss` = cosa e perché, con quale fonte; `plan` = come, in che ordine, con quali contratti (consuma questa ricerca, non la ripete).

## 4. `plan`

Richiede `discuss X` completato. Se X-1 è in exec, pianifica **contro `0(X-1)-CONTRATTO.md`**, non contro la codebase di X-1.

1. **Preflight** — STATUS; CONTEXT con classificazione di criticità; contratto X-1; WIRING-MAP.
2. **Ricerca implementativa** — lacune su API, versioni di crate, specifiche: canale di default Claude (context7 prima di tutto); `0X-RESEARCH.md` con fonti. Una voce **Proposta** o **Aperta** di `DEPENDENCIES.md` non si decide qui: zona grigia → rimbalzo a `discuss`.
3. **Wiring prima dei piani** — `wiring-search` + `wiring-design`, tra slice e tra fasi (§4.2). `wiring-build` scrive codice: è la wave 0 di `execute`.
4. **Architettura locale (ECC)** — `ecc:code-architect` per slice grossa: blueprint (crate, trait-porta, file, flusso, ordine).
5. **Piani** — `gsd-plan-phase X --skip-research` in un **subagente fresco** (mai `fork`), vincolato a CONTEXT + RESEARCH + `0X-WIRING.md` + blueprint + contratto X-1; il subagente non risponde al posto dell'utente, riporta in caveman e non scrive lo stato. I piani li scrive sempre GSD.
6. **Gate paralleli sui piani** — `solid-check`, `crate-boundary-check`, `security-scanner`, `perimetro-check` sempre; `eval-oracle-check` solo se un piano tocca dataset di evaluation, valori attesi o soglie (JEV, retrieval). Verdetto per piano `conforme`/`da correggere`; ricircolo fino a conformità; 3 giri falliti → stop e presentazione all'utente.
   **6-bis. Rischi e test design** — fase **critica**: GPT su AgentBridge, `implementation-risk-audit` poi `test-design`, fino a `plan-gate = TEST_SPEC_READY` (limite proprio di 3 giri d'audit; branch `task/PLAN-…` mai ribasato dopo l'audit). Fase **non critica**: un agente Claude separato (fresco, distinto da planner ed esecutori) scrive `0X-YY-TEST-DESIGN.md` nello stesso formato (`RISK-N`, `TEST-N` con `catches`, `uncovered` motivati).
7. **Contratto e onde** — `0X-CONTRATTO.md`: rotte+DTO, tabelle e migrazioni per servizio, tipi di `crates/contracts`, trait-porta, route e componenti di `apps/web`, **seam della WIRING-MAP** creati o riempiti; `onde-slice`; `update-orchestrator-state --plan`; commit di tutti gli artefatti di fase.

**Regole anti-falso-verde dei gate**: `skipped` non è mai verde; lo status deriva dall'exit code grezzo dello script (0 pass, 1 fail, 2 non eseguito, che blocca come 1); uno scope vuoto dà `skipped`, mai un fallback silenzioso sull'intero repo; un test ancorato a un TO-DO aperto resta *pending*; **lo script trova, l'agente giudica**, e le righe giudicate legittime vanno in `0X-GATE.md` perché il re-gate non le ridiscuta.

### 4.1 Mappa dei gate

I gate agiscono **una volta, sul piano**; a codice scritto lo stesso rischio è coperto da enforcement meccanico e dai reviewer:

| Gate | In `plan` (sul PLAN) | A codice scritto |
| --- | --- | --- |
| solid-check | responsabilità; trait-porta dove serve un'astrazione; niente `match` sul tipo sostituibile da trait; struct/impl e componenti non sovradimensionati | `ecc:rust-reviewer`, `ecc:typescript-reviewer`, `ecc:react-reviewer` in `verify` |
| crate-boundary-check | nessun servizio dipende da un altro servizio; dipendenze interne solo verso `crates/contracts` e `crates/telemetry`; `domain` senza `sqlx`, `reqwest`, `axum`, SDK provider; wiring solo nel composition root | controllo meccanico su `cargo metadata` e sugli `use` di `src/domain/**` + test architetturali |
| security-scanner (ASVS L1, OWASP LLM Top 10, RFC 7519/8725) | authz deny-by-default per endpoint; regola no-leak verso reranker/JEV/LLM/cache/log/telemetria; JWT e Argon2id; segreti; token nello storage del browser; upload; SSRF sulle fonti online; prompt injection | `ecc:security-reviewer` sul diff in `verify` |
| eval-oracle-check (solo se il piano tocca dataset, valori attesi o soglie) | provenienza degli oracoli citata nel piano | test di evaluation marcati + triplo guardrail + regressione sull'evaluation corpus in `verify` |
| perimetro-check | niente ponti per fasi successive: ciò che copre una fase futura compare solo come seam vuoto o segnaposto nudo | filtro in `verify` 3-bis |

**Valvola d'eccezione in `verify`** — i gate si riaccendono **solo su segnale**: dipendenza fra crate o import nel `domain` non previsti → `crate-boundary-check`; valore atteso, citazione attesa o soglia non previsti → `eval-oracle-check` **e rimbalzo al planning**; superfici sensibili non pianificate (auth/JWT, RBAC, log e telemetria, parser e upload, fonti online, prompt verso LLM o JEV, segreti) → `security-scanner` sui soli file toccati.

**Oracoli di evaluation (triplo guardrail)** — un valore atteso (domanda, risposta, citazione attesa, numero estratto, soglia) è legittimo solo se viene da: (1) un estratto verbatim, con pagina, del documento sorgente; (2) una derivazione documentata dal documento sorgente; (3) una decisione utente ratificata. **Mai generato eseguendo la pipeline RAG.** Il guardrail ha tre passi: (1) Claude deriva il valore con un agente separato dall'implementatore; (2) GPT, contratto `double-check` in sessione pulita, riceve solo documento sorgente e derivazione, mai il codice — **sempre**; (3) l'utente ratifica guardando derivazione e fonte. Servono tre verdi; un disaccordo qualsiasi diventa TO-DO. Tolleranze: citazione = documento + versione + pagina esatti, span con overlap; numeri estratti esatti.

### 4.2 Wiring inter-fase e WIRING-MAP

Oltre ai seam tra le slice di una fase, il loop gestisce i **seam tra fasi**. `.planning/WIRING-MAP.md` è il registro persistente dei seam di estensione promessi o creati e della fase che li riempirà: trait-porta, registry espliciti, mount point del router Axum, convenzione di numerazione delle migrazioni per servizio, tag OpenAPI, route del frontend, registry degli eventi di audit e dell'outbox, membri del workspace.

- `discuss` la legge in preflight e porta nel CONTEXT i seam che riguardano la fase;
- `wiring-search` legge anche i goal delle fasi successive (`gsd-tools roadmap get-phase N`, solo il goal) per capire quali seam la fase X deve predisporre;
- `wiring-design` scrive `0X-WIRING.md` (§1 inventario intra-fase, §2 seam inter-fase, §3 convenzioni, §4 vincoli per gli esecutori — l'unica sezione nel brief di execute —, §5 seam di wave 0, §6 esito di wave 0) e aggiorna la mappa (righe `WM-NNN` in stato `promised`), creandola dal template `skills/wiring-design/WIRING-MAP.template.md` se manca;
- `wiring-build` crea i seam nella wave 0 (`promised` → `created`) e compila §6; non riscrive i PLAN: un piano congelato non plug-in-only è deviazione (b);
- `0X-CONTRATTO.md` elenca i seam promessi;
- in `execute`, toccare un seam della mappa fuori da quanto il PLAN dichiara è una deviazione (b);
- `verify` riconcilia i seam reali con la mappa e porta a `filled` le righe riempite dalla fase (lo stato `filled` lo imposta solo `verify`).

**Confine con `perimetro-check`**: un seam per una fase futura è ammesso SOLO vuoto (trait senza implementazione, registry senza voci, punto di montaggio senza rotte, segnaposto nudo). Il contenuto della fase futura resta vietato.

**Default dei seam**: append-registry esplicito in file noti, ogni slice appende in coda; mai auto-discovery, salvo richiesta esplicita; mai `match` sul tipo dove serve un registry.

## 5. `execute`

1. **Preflight** — piani committati, gate `conforme`, test design presente per ogni piano, onde presenti; fasi critiche: `plan-gate` in `TEST_SPEC_READY`/`EXEC_READY`. **Safe-resume**: commit di piano senza SUMMARY → bridge in modalità `--da-prove`, mai ri-esecuzione.
2. **Worktree** creati a mano dal `main` locale (`isolation: "worktree"` parte da `origin/HEAD`), con il controllo «HEAD = <hash>, altrimenti fermati» nel brief.
3. **`test-write`** — un agente Claude **fresco e distinto dall'esecutore** scrive i test del design nei soli `allowed_test_paths` e li porta al rosso prima dell'implementazione. `RED_EXPECTED` → si procede; `RED_UNEXPECTED` → nuovo test-write mirato; `GREEN_UNEXPECTED` → test debole (nuovo test design) o comportamento già esistente (il test resta come regressione).
4. **Wave 0** — `wiring-build`: seam condivisi della fase e seam inter-fase della WIRING-MAP, creati una volta, vuoti dove servono a fasi future, e committati.
5. **Onde** — esecutori `gsd-executor` concorrenti, **tetto 2** (`workflow.max_esecutori_onda`), uno per slice e ciascuno nel proprio worktree. Brief stretto: PLAN integrale, §4 di `0X-WIRING.md`, test design, «Next Phase Readiness» dell'onda prima; il resto per puntatore. Metodo `ecc:tdd-guide`, commit atomici per task con scope `(0X-YY)`; build rossa → `ecc:rust-build-resolver` o `ecc:react-build-resolver` a diff minimo. Uscita: rapporto `.planning/tmp/rapporto-0X-YY.yaml` scritto prima di ogni narrazione. Divieti: STATE/ROADMAP/STATUS/WIRING-MAP, `--no-verify`, uscire dai `files_modified` senza deviazione registrata. Correzioni sempre con un esecutore **nuovo**, mai con una ripresa.
6. **Fine onda** — onda con più slice → `integratore-onda`; onda con una sola slice → merge `--no-ff` dell'orchestratore e gate eseguito **una volta** sul main fuso. Gate d'onda: `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`; in `apps/web` `npm run lint`, `npx tsc --noEmit`, `npx vitest run`. Ogni passo si salta SOLO se il relativo manifest (`Cargo.toml`, `apps/web/package.json`) non esiste ancora, e lo si stampa; nessun bypass.
7. **Bridge per piano** (solo a merge verde) — §7.
8. **Stato** — `--exec` a ogni fine onda, più `APPEND-STATUS` sulla dashboard a ogni tappa.

**Deviazioni a tre livelli**: (a) dentro il piano → auto-fix registrato nel rapporto e nel SUMMARY; (b) **tocca il contratto di fase, un oracolo o una soglia di evaluation, oppure un seam della WIRING-MAP → STOP della slice, rimbalzo al planning, mai in silenzio**; (c) interruzione esterna → `--evento` + safe-resume dal primo task non committato.

## 6. `verify`

1. **Banda ECC** — test/lint/build completi (gli stessi comandi del gate d'onda, condizione d'ingresso); `lint-stato.cjs`; code review sul **diff di fase**: `ecc:rust-reviewer`, `ecc:database-reviewer` (migrazioni, `sqlx`, `pgvector`), `ecc:typescript-reviewer`, `ecc:react-reviewer` — finding gravi corretti qui; `ecc:security-reviewer` + valvola d'eccezione (§4.1); `ecc:e2e-runner` (`npx playwright test`) sui flussi critici quando c'è UI; **regressione sull'evaluation corpus** quando la fase tocca retrieval o generazione: confronto con la baseline ratificata in `evaluation/results/`, peggioramento oltre soglia = gap, soglia mancante = TO-DO utente, baseline mai aggiornata in silenzio.
2. **Cerniera** — `update-orchestrator-state --verify-ecc`: verdetto ECC nello STATUS **prima** che GSD parli.
3. **Banda GSD** — `gsd-verifier` → `0X-VERIFICATION.md`. `passed` → `phase complete` + **UAT umana solo sui deliverable `human_judgment: true`**; `gaps_found` → `plan --gaps` → `execute --gaps` → `verify`.
   **3-bis. Filtro del perimetro** — `gate-perimetro.cjs voci`: i punti su aree di fasi successive si scartano d'ufficio e non arrivano in UAT né in un giro gap.
4. **Ricerca pre-UAT** — `ricerca-punti-aperti` (§9-bis): triage, ricerca, presentazione per specie.
5. **Riconciliazione** — SUMMARY reali contro `0X-CONTRATTO.md` (onorato → piani di X+1 validi; deviato → ritocco dei soli punti divergenti) e seam reali contro WIRING-MAP (mancante o di forma diversa → gap; concordante → stato aggiornato nella mappa).
6. **Stato finale** — `--verify`: fase verificata, UAT pendenti già classificate, prossima azione.

**Invarianti**: niente review del codice storico; niente UAT auto-chiuse; niente fase completa senza `passed`; niente baseline di evaluation aggiornata senza ratifica.

## 7. Bridge (passo interno di `execute`)

Ciò che GSD esige per piano chiuso: SUMMARY col frontmatter obbligatorio (`requirements-completed` copiati dal PLAN, `key-files`, `status: complete`) e col **blocco `coverage`** (matrice per deliverable con `verification[].ref`/`status`, ogni `TEST-N` del design collegato al suo `RISK-N`): guida il routing della UAT — l'automazione provata si auto-passa, all'utente restano i giudizi di merito.

**5 passi per piano**: (1) raccolta prove — commit dal log per scope, diff vs `files_modified`, esiti test; rifiuto su zero commit, file non toccati, test rossi; (2) rapporto dell'esecutore **incrociato con le prove** — mai fidarsi del racconto; (3) SUMMARY dal template GSD, `human_judgment: true` fail-safe; (4) autovalidazione con `gsd-tools verify-summary`; (5) registrazione atomica — `roadmap update-plan-progress`, verbi `state`, riga in `DECISIONS-INDEX.md`, `lint-stato.cjs` verde, commit `docs(0X-YY): completato il piano YY — <one-liner>`. O tutto o niente; idempotente.

**Due modalità**: primaria **dal rapporto**; degradata **`--da-prove`** (ricostruzione da git + PLAN + test, deliverable non dimostrabili → `human_judgment: true`), che è anche lo strumento di recovery di ogni esecuzione fuori protocollo.

## 8. Contratto di fase e pipeline sfalsata

Obiettivo: `plan X` senza attendere `exec X-1`. Le catene che li legano sono la codebase reale, i SUMMARY come contesto e le decisioni prese in corsa. Soluzione: `0X-CONTRATTO.md` prodotto da `plan` (rotte+DTO, schema per servizio, tipi di `crates/contracts`, trait-porta, componenti esportati, seam della WIRING-MAP) → `plan X+1` consuma il contratto invece dei SUMMARY; il contratto è **congelato in exec** (deviazione che lo tocca → rimbalzo al planning); `verify` riconcilia. Limite deliberato: la UI di dettaglio resta late-bound, valutata dal vivo su localhost.

## 9. `onde-slice`

Input: frontmatter dei PLAN (`files_modified`, `depends_on`), `0X-WIRING.md`, contratto. Algoritmo: grafo delle dipendenze → livelli; sovrapposizione `files_modified` a coppie, glob-aware (anche un solo file in comune, `Cargo.lock` e `package-lock.json` compresi → niente parallelo); vincoli meccanici: un solo piano con superficie API per onda (rigenerazione OpenAPI e client per intero), un solo piano per servizio e per onda che aggiunge migrazioni, un solo piano per onda che aggiunge membri al workspace; tetto di concorrenza **2**. Output: `0X-ONDE.md` committato. Rifiuta su cicli o piani senza `files_modified` → rimbalzo a `plan`.

## 9-bis. `ricerca-punti-aperti`

Skill di servizio invocata da `verify` prima di portare i punti aperti all'utente, o standalone su qualsiasi UAT. **Triage** a tavolino: giudizio puro (estetica, gusto, verifica visiva) contro punto ricercabile (specifica, standard, comportamento documentato, coerenza con decisioni ratificate; in dubbio → ricercabile). Ricerca di Claude per default, GPT via `deep-research-handoff` solo su richiesta dell'utente. Lenti: **fonte autorevole**, **pratica consolidata**, **decisioni del progetto** (che prevalgono sull'esterno). Esito per punto: chiuso dalla fonte (conferma a un tocco) / orientato (raccomandazione con fonte) / giudizio puro (nudo). Presentazione nell'ordine **conferme → orientati → giudizi puri**; i finding entrano nelle voci UAT come proposte con fonte, mai come decisioni. Guardrail: mai auto-risolvere; triplo guardrail degli oracoli intatto; fonte non trovata ≠ fonte contraria.

## 10. `update-orchestrator-state` e ORCHESTRATOR-STATUS.md

Unica skill autorizzata a scrivere `.planning/ORCHESTRATOR-STATUS.md`. Flag: `--discuss | --plan | --exec | --verify-ecc | --verify | --evento | --uat`. Idempotente; timbra data e ora; ogni flag aggiorna solo le sezioni di competenza (Posizione e Prossima azione sempre). Il commit del file è del verbo chiamante; `--evento`/`--uat` standalone committano da soli.

Sezioni: `Posizione` · `Prossima azione` (una riga, per ripartire a freddo) · `Findings` · `Limiti trovati` · `Gate & verdetti` (per piano: gate sul PLAN, review ECC, verifier GSD) · `UAT` · `TODO` · `Decisioni ratificate in corsa`.

Regole di sanità: **indice, non archivio** (una riga + puntatore all'artefatto); **mai fatti GSD** né contenuto della WIRING-MAP (una sola fonte di verità per fatto).

## 11. `integratore-onda` (contratto agente)

`.claude/agents/integratore-onda.md`. Solo per onde con più slice: merge dei worktree sul main nell'ordine di `0X-ONDE.md`; conflitti residui risolti con motivazione (un conflitto su un seam condiviso è anche una violazione da segnalare); gate d'onda completo post-merge; rosso → build-resolver o rimbalzo alla slice colpevole. Non scrive SUMMARY (bridge), non aggiorna lo stato (notaio), non giudica la qualità (verify).

## 12. Configurazione e avvio

- `.planning/config.json`: `mode: yolo`, `model_profile: quality`, `parallelization: true`, `commit_docs: false` (committa il verbo), `workflow.code_review: false` (la review è di ECC, niente doppio verdetto), `workflow.plan_check`, `workflow.verifier`, `workflow.nyquist_validation: true`, `workflow.max_esecutori_onda: 2`, `security_enforcement: true` (ASVS livello 1, blocco su `high`), `human_verify_mode: end-of-phase`, `claude_md_path: ./CLAUDE.md`. I motori di ricerca integrati in GSD restano spenti: i researcher di GSD non si usano (`--skip-research`) e la ricerca la fa Claude direttamente.
- Database di sviluppo e test: PostgreSQL + pgvector su **Docker Desktop** (Compose del repo), mai un remoto.
- AgentBridge: progetto `CorporateKnowledgeAssistant`, contratti in `C:\Agent Bridge\docs\contracts.md`.
- Lingua: documenti, skill e commit in italiano; codice e identificatori in inglese.
- **Avvio**: `/gsd:ingest-docs` sui documenti esistenti → roadmap a fasi → la prima fase multi-slice fa da **pilota** del wiring (anche inter-fase); gli attriti si registrano come correzioni di procedura datate. Da lì `/discuss 1` → `/plan 1` → `/execute 1` → `/verify 1`.

## 13. Fuori scope (YAGNI)

Codex come implementatore o scrittore di test (slot spento per decisione utente); piani scritti da GPT; estensione degli interni di gsd-core; parallelismo fra fasi in exec (solo plan sfalsato); UI di dettaglio pianificata in anticipo; una fase 0 di ricerca di mercato.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
