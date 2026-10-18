# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Cos'è questo progetto

Corporate Knowledge Assistant (CKA): assistente RAG aziendale per documenti finanziari e societari, con autenticazione, RBAC, citazioni verificabili, viewer in-app, audit e aggiornamento incrementale della knowledge base. Progetto portfolio.

**Stato:** definizione dell'architettura. Non esiste ancora codice, build system né test runner: le directory di codice contengono solo `.gitkeep`. Quando parte l'implementazione, aggiungere qui i comandi reali.

**Lingua:** documenti, skill e commit in italiano; codice e identificatori in inglese (convenzione Rust).

## Riferimenti obbligatori

Fonti di verità, da leggere prima di proporre o scrivere codice:

- `README.md`: scope MVP, i 3 servizi, target prestazionali (Time To Queryable), principi.
- `DEPENDENCIES.md`: ogni scelta di stack è **Decisa**, **Proposta** (default da validare con uno spike) o **Aperta**. Una Proposta o un'Aperta non è una decisione presa. Le dipendenze nell'elenco «da evitare nell'MVP» non si introducono.
- `docs/architecture/interfaces-and-concurrency.md`: endpoint HTTP per servizio, trait-porta Rust, modello di concorrenza, race condition da testare.
- `docs/architecture/architecture.md`: vista dei container, flussi di ingestion e query, ownership dei dati.
- `docs/adr/`: decisioni accettate. Una decisione irreversibile o costosa diventa un nuovo ADR da `0000-template.md`.
- `docs/superpowers/specs/loop-gsd-ecc-design.md`: spec del loop di sviluppo.

Alcuni documenti (`api-contract.md`, `data-model.md`, `test-plan.md`, …) sono stub che rimandano a template in `../../00-shared-templates/`, fuori dal repo.

## Architettura

Esattamente **tre microservizi di dominio** in Rust; nessun quarto bounded context.

1. **Identity & Access** (`services/identity-service/`): utenti, hash Argon2id, ruoli (`admin`, `manager`, `employee`, `guest`), JWT firmati asimmetricamente, JWKS. Niente MFA nell'MVP.
2. **Document & Indexing** (`services/document-service/`): upload, fonti online con quarantena, parsing/OCR, chunking, embedding, versionamento, ACL, indice `pgvector`, retrieval autorizzato, contenuto per il viewer. Il worker di ingestion è una modalità di esecuzione di questo servizio.
3. **RAG & Answer** (`services/rag-service/`): flusso query, `DecisionEngine` JEV, reranking, evidence gate, generazione strutturata, validazione delle citazioni, rifiuto, audit, evaluation.

Frontend (`apps/web/`, React + TypeScript), reverse proxy, worker e database sono componenti di deployment. `crates/` contiene librerie Rust interne (`contracts`, `telemetry`); i modelli di dominio **non** si condividono fra servizi.

Regole trasversali:

- **Ownership dei dati**: un cluster PostgreSQL, ma schema/database e credenziali separati per servizio. Nessuna join fra servizi; migrazioni proprie per servizio. I dati attraversano i confini solo via contratti HTTP o transactional outbox con consumer idempotenti (`event_id` univoco).
- **Porte e adapter**: i casi d'uso dipendono da trait interni (`DocumentSource`, `DocumentStore`, `DocumentParser`, `Chunker`, `Embedder`, `VectorIndex`, `DecisionEngine`, `AnswerGenerator`, `AccessPolicy`, `AuditSink`, `Cache`). Tipi di provider, SDK o database non attraversano una porta di dominio. Primo adapter di `Cache`: `NoOpCache`; Redis solo dopo una misura.
- **JEV** (decision engine typesafe.ai): HTTPS (`POST /v1/systemone`) tramite adapter Rust dietro `DecisionEngine`, senza SDK. Solo decisioni semantiche (validazione query, rerank, sufficienza/contraddizione delle evidenze, validazione claim–citazione). RBAC, provenance, mapping delle citazioni, validazione dei file e deduplicazione restano deterministici. Nei test unitari si usa un fake deterministico. Un guasto JEV non bypassa mai un controllo di sicurezza.
- **Sicurezza**: deny by default. Nessun chunk non autorizzato raggiunge reranker, JEV, LLM, cache o log. L'autorizzazione si ricontrolla prima di restituire citazioni e link, e di nuovo nel viewer.
- **Convenzioni API**: rotte pubbliche sotto `/api/v1`, interne sotto `/internal/v1`; UUID opachi; UTC RFC 3339; idempotency key sulle operazioni mutanti; propagazione di `request_id` e trace context; errori con `code`, `message`, `request_id`.
- **Concorrenza**: async per l'I/O, pool limitati per il lavoro CPU-bound (`spawn_blocking` o processi isolati per parsing/OCR). Ogni fan-out ha limite, deadline globale, cancellazione e budget di retry. Upload in streaming. Worker con lease + `FOR UPDATE SKIP LOCKED`. Nessuna chiamata esterna dentro una transazione. Le versioni di documento si pubblicano atomicamente.
- **Corpus**: knowledge corpus (dinamico; la discovery online passa da quarantena e approvazione) separato dall'evaluation corpus (versionato, immutabile).

## Flusso di lavoro: loop GSD+ECC a 4 verbi

Metodologia portata da Gestionale Light il 2026-10-18 (decisione utente). Spec: `docs/superpowers/specs/loop-gsd-ecc-design.md`.

- **Verbi**: `discuss | plan | execute | verify`, skill in `skills/<verbo>/SKILL.md`. GSD comanda requisiti, roadmap, piani e stato; ECC (plugin installato a scope progetto) comanda implementazione e verifica ingegneristica. Ordine della verifica: ECC → GSD, mai inverso né in parallelo.
- **Skill**: codice canonico in `skills/`; `.claude/skills/` contiene solo il mirror markdown, mai script duplicati. Gate sui piani: `solid-check`, `crate-boundary-check`, `security-scanner`, `eval-oracle-check`, `perimetro-check` (`scripts/gate-perimetro.cjs`), `proprieta-file` (`scripts/gate-proprieta-file.cjs`, su tutti i piani della roadmap). Wiring: `wiring-search` → `wiring-design` → `wiring-build`, **intra e inter-fase**, con registro di progetto `.planning/WIRING-MAP.md`. Un seam per una fase futura si crea solo vuoto: il suo contenuto resta vietato dal perimetro.
- **Stato**: `.planning/` è versionata. `STATE.md` e `ROADMAP.md` si scrivono solo via `gsd-tools` (`node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs"`), mai direttamente. `.planning/ORCHESTRATOR-STATUS.md` lo scrive solo `update-orchestrator-state`. Agenti `gsd-*` dall'installazione utente (GSD 1.42.3); vendorizzato solo `.claude/agents/integratore-onda.md`.
- **Ricerca esterna**: di default la fa Claude (WebSearch/WebFetch, context7, `anthropic-skills:deep-research`), citando URL e data. Quando l'utente lo chiede, va a ChatGPT Deep Research tramite `skills/deep-research-handoff` su AgentBridge: in quel caso le fonti non si riverificano e la ricerca si attribuisce a GPT con il suo ID.
- **AgentBridge** (`http://127.0.0.1:8765`, solo loopback; progetto `CorporateKnowledgeAssistant`; contratti in `C:\Agent Bridge\docs\contracts.md`):
  - `ask-deepsearch`, su richiesta dell'utente;
  - `implementation-risk-audit` + `test-design`, **solo sulle fasi critiche**: identity, RBAC + retrieval autorizzato, ingestion. Altrove il test design lo fa un agente Claude separato;
  - `double-check`, **sempre**, come verificatore esterno del triplo guardrail sugli oracoli dell'evaluation corpus;
  - `status-tracker`, una voce `APPEND-STATUS` a ogni tappa reale.
  Ogni `READ-RESPONSE` va seguito da `ACK-RESPONSE`.
- **Codex**: operativo ma non usato in questo progetto (decisione utente). Implementano i `gsd-executor` in worktree isolati, tetto di 2 concorrenti. I test rossi (`test-write`) li scrive un agente Claude fresco, distinto dall'esecutore.
- **Strategia di consegna provider-first** (decisione utente 2026-10-18):
  - **Ordine:** `discuss` di tutte le fasi → esecuzione della fase Spike → `plan` di Fondazione e Provider → `plan` di tutte le fasi consumatrici contro i trait-porta → riconciliazione dei piani provider → esecuzione per onde di dipendenza.
  - **Fondazione:** costruisce completa le parti comuni e pre-registra gli innesti vuoti di ogni fase successiva.
  - **Provider:** gli adapter dietro le porte si implementano completi nella propria fase, prima dei consumatori. Ogni operazione cita almeno un piano consumatore; senza consumatore non si scrive.
  - **Insiemi di file chiusi:** ogni fase possiede un insieme chiuso di file che le altre non modificano, salvo la whitelist di `scripts/proprieta-file.whitelist.json`. Gate meccanico: `scripts/gate-proprieta-file.cjs`. Un difetto trovato in un file chiuso si registra con motivo e si corregge in una fase decimale di gap-closure (`riapre_perimetro: [N]`), mai in silenzio. Gli innesti creati dalla Fondazione passano alla fase indicata nella colonna `fase-proprietaria` della WIRING-MAP.
- **Racconto didattico** (decisione utente 2026-10-18): al termine di ogni slice il bridge di `execute` scrive `0X-YY-RACCONTO.md` in prosa: cosa è stato fatto, come (percorso di lettura del codice con `percorso:riga`, costrutti non ovvi spiegati), perché (alternative scartate), test, deviazioni. Scopo: l'utente mantiene il controllo e impara.
- **Stile**: gli intermedi fra agenti (brief, manifest, rapporti YAML) sono in caveman; i deliverable (CONTEXT, PLAN, CONTRATTO, SUMMARY, VERIFICATION), i commit e tutto ciò che legge l'utente sono in prosa. `.caveman.json` tiene la sessione principale in prosa.
- **Firma**: ogni nuovo artefatto o voce di stato porta `**Modifica:** <Autore> · <data> · <cosa>`. Una regola superata resta scritta con un paragrafo *Storia*, mai cancellata in silenzio.
- **Regola dei dubbi** (elenco vivo, lo aggiorna l'utente): su ciò che è qui elencato si procede senza chiedere. Elenco attualmente vuoto.
- **Hook git**: `.githooks/pre-commit` e `pre-push`, attivati con `scripts/attiva-hook.sh`, senza bypass. Backup: `scripts/backup.sh`.
- Questo è l'**unico CLAUDE.md** del repo: niente `.claude/CLAUDE.md`.

## Configurazione

`.env.example` elenca le variabili attese (`DATABASE_URL`, `JWT_*`, `JEV_API_URL`, `TYPESAFE_API_KEY`, `LLM_PROVIDER`, `LLM_MODEL`, `EMBEDDING_MODEL`, …). I file `.env*` reali sono ignorati da git. Ambiente locale: Docker Desktop + Compose per PostgreSQL con `pgvector` (decisione utente 2026-10-18).
