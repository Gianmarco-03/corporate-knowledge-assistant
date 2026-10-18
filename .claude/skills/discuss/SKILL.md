---
name: discuss
description: Primo verbo del loop GSD+ECC di Corporate Knowledge Assistant — apre una fase con decisioni fondate su fonti. Legge stato, punti ereditati e WIRING-MAP, esegue la ricerca decisionale a tre lenti (fonte autorevole, pratica consolidata, riferimenti di prodotto) PRIMA di qualsiasi domanda, poi delega la discussione a gsd-discuss-phase con ogni domanda già armata del finding, classifica la fase come critica o no, e chiude registrando lo stato. USARE per aprire una fase ("discuti la fase X", "/discuss X"), quando una fase va discussa prima della pianificazione, o quando l'utente chiede di ridiscutere una fase con basi documentate.
---

# discuss

Primo verbo del loop GSD+ECC (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md` §3). Scopo: aprire la fase X con decisioni **fondate su fonti** e lasciare uno stato da cui chiunque riparte.

## Confini di delega

- **GSD**: la discussione vera e propria e il CONTEXT (`gsd-discuss-phase`).
- **Ricerca esterna — canale di default: Claude.** Claude ricerca in proprio con WebSearch/WebFetch, con context7 per librerie e crate (versioni, API, esempi ufficiali) e con la skill `anthropic-skills:deep-research` quando un'area richiede una sintesi multi-fonte. Ogni affermazione porta la fonte: URL e data di accesso.
- **ChatGPT Deep Research (GPT)**: solo **su richiesta esplicita dell'utente**, via AgentBridge (`ask-deepsearch`, ID `deepsearch_N`) con la skill `deep-research-handoff`. Su un rapporto GPT vale la sua regola: non si riverificano le fonti, si attribuisce il rapporto con il `content_id`.
- **Home-made**: vincolo «decisione ⇒ fonte», redazione del brief, lettura della WIRING-MAP, stato.
- **ECC**: nulla — questo è l'unico verbo interamente GSD+utente.

## Passi

1. **Preflight**
   - Leggi `.planning/ORCHESTRATOR-STATUS.md` (se manca, crealo via `update-orchestrator-state`). Se manca l'intera `.planning/`, il progetto non è inizializzato: il bootstrap è `/gsd:ingest-docs` sui documenti esistenti (`README.md`, `DEPENDENCIES.md`, `docs/architecture/`, `docs/adr/`, `deliverables/checklist.md`), non questo verbo.
   - Verifica che la fase X esista: `node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" find-phase X`. Se X-1 non è ancora verificata, **annota il rischio** nello stato (pipeline sfalsata, spec §8) ma non bloccare.
   - **Carico minimo**: goal, requisiti e criteri di successo della fase si prendono con `gsd-tools roadmap get-phase X` — MAI `cat` dell'intero `ROADMAP.md`. Le decisioni precedenti arrivano da `.planning/DECISIONS-INDEX.md` (una riga per decisione + puntatore), che `gsd-discuss-phase` legge al posto dei CONTEXT precedenti; il rationale si apre dal puntatore SOLO se una domanda lo richiede.
   - **Punti ereditati**: raccogli da `.planning/phases/*/deferred-items.md` delle fasi precedenti le voci che nominano la fase X («Fase X», «prima della Fase X», «→ Fase X»). Sono decisioni rinviate QUI: entrano nella ricerca come aree obbligatorie e nella discussione come domande già armate.
   - **WIRING-MAP**: leggi `.planning/WIRING-MAP.md` (registro persistente dei seam inter-fase; lo scrive `wiring-design`). Estrai (a) i seam che la mappa assegna alla fase X da **riempire** — sono vincoli di forma già decisi: la discussione non li ridisegna, salvo che l'utente li riapra; (b) i seam che fasi precedenti hanno promesso e la fase X deve **predisporre**. Entrambi vanno nel CONTEXT come vincoli con il riferimento `WM-NNN` della riga. Se il file non esiste ancora, annotalo e prosegui: lo crea `wiring-design` dal template `skills/wiring-design/WIRING-MAP.template.md` al primo `plan`.
   - **Classificazione di criticità**: la fase è **critica** se il suo goal tocca anche solo in parte autenticazione/identity, RBAC con retrieval autorizzato, oppure ingestion (upload, parsing, fonti online). Proponi la classificazione con il motivo e falla ratificare insieme al CONTEXT: decide se in `plan` interviene GPT (`implementation-risk-audit` + `test-design`) o un agente Claude separato per il test-design.
2. **Ricerca decisionale a tre lenti** — per ogni area che richiede nuova evidenza:
   - **Fonte autorevole**: specifica, RFC, standard di sicurezza o norma applicabile, citata al punto (es. RFC 7519/8725 per JWT, OWASP ASVS L1, OWASP Top 10 for LLM Applications, GDPR per i dati personali nei log, documentazione ufficiale di PostgreSQL/pgvector, contratto OpenAPI di JEV), oppure le decisioni già scritte in `DEPENDENCIES.md`, `docs/architecture/` e `docs/adr/`, che prevalgono sull'esterno finché l'utente non le riapre;
   - **Pratica consolidata**: il modo standard di risolvere quel tipo di problema nello stack (Rust/Axum/sqlx, React/TypeScript, RAG con citazioni verificabili);
   - **Riferimenti di prodotto**: cosa fanno assistenti RAG aziendali comparabili. Lente facoltativa a scala portfolio: si usa solo quando l'area è una scelta di scope.
   Claude esegue la ricerca nel canale di default (sopra) e scrive `0X-RICERCA-DISCUSSIONE.md` nella directory di fase: un brief per area, ogni affermazione con la sua fonte (URL + data di accesso, oppure file e sezione del repo). Se l'utente ha chiesto GPT per un'area, quella parte passa da `deep-research-handoff` e nel brief si attribuisce a **ChatGPT Deep Research (OpenAI)** con il `content_id`; finché la richiesta non è rientrata e confermata con `ACK-RESPONSE`, non avviare la discussione che ne dipende. **Stile caveman full** per il brief (Regola caveman, spec §2): terso, sostanza e citazioni intatte. Il CONTEXT e le domande all'utente restano in prosa normale.
3. **Discussione fondata** — delega a `gsd-discuss-phase X`. Ogni domanda all'utente arriva **già armata del finding**, in uno di questi formati:
   - «La fonte X (RFC/standard/ADR) dice così. Mi allineo? Sì/No»
   - «I sistemi comparabili offrono X e qui non è previsto. Lo aggiungo allo scope e lo discutiamo?»
   - «La pratica consolidata per questa architettura è X. La usiamo anche noi?»
   **Mai una domanda nuda**: se manca il finding, ricerca il punto prima di chiedere. **Eccezione:** ciò che la regola dei dubbi di `CLAUDE.md` dichiara sicuro non si chiede affatto. Se `gsd-discuss-phase` entra in advisor mode, usa le tabelle sintetizzate dal brief già scritto e salta `advisor_research` per le aree già coperte; uno spawn di `gsd-advisor-researcher` è ammesso solo per una zona grigia che il brief non copre, e il suo esito entra nel brief con le fonti.
4. **Raccolta** — a discussione chiusa verifica che `0X-CONTEXT.md` esista e contenga: ogni decisione bloccata con la **fonte che la fonda**; i vincoli dalla WIRING-MAP con puntatore; la classificazione di criticità ratificata. Decisione senza fonte → TO-DO con assunzione provvisoria, mai scelta silenziosa (regola CLAUDE.md, qui meccanica).
5. **Stato** — `update-orchestrator-state --discuss`: posizione `fase X / — / — / discuss completato`; findings ← decisioni bloccate con fonte (una riga + puntatore al CONTEXT) e criticità della fase; TODO ← questioni aperte; prossima azione → `/plan X`.
6. **Commit** — CONTEXT, discussion log e brief di ricerca: `docs(0X): contesto di fase discusso` (+ ORCHESTRATOR-STATUS.md nello stesso commit).

## Fallimenti e ripresa

Discussione interrotta a metà → lo stato registra `discuss in corso` con l'istruzione di ripresa. La skill è idempotente: rilanciarla riprende da dove si era fermata e non duplica ricerca già fatta (il brief esistente si aggiorna, non si riscrive).

## Cosa NON fa

Niente ricerca implementativa (è di `plan`, che consuma questa senza ripeterla), niente piani, niente codice, niente ECC, nessuna modifica alla WIRING-MAP (la scrive `wiring-design`), nessuna decisione al posto dell'utente.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
