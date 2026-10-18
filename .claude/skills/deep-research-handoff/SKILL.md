---
name: deep-research-handoff
description: Invia al controller AgentBridge le ricerche esterne del loop di Corporate Knowledge Assistant che l'utente vuole affidare a ChatGPT Deep Research, come richieste ask-deepsearch con content_id, e riprende il lavoro solo dopo la risposta di GPT e il suo ACK. Descrive anche i contratti vicini (double-check degli oracoli di evaluation, status-tracker, revisione dei piani delle fasi critiche). USARE solo quando l'utente chiede esplicitamente GPT per una ricerca, quando un audit GPT apre una external_research, o per i contratti double-check e plan-gate; la ricerca di default la fa Claude.
---

# Handoff della ricerca esterna a GPT

## Quando si usa

Il canale di ricerca **di default è Claude**: WebSearch/WebFetch, context7 per librerie e crate, la skill `anthropic-skills:deep-research` per le sintesi multi-fonte, con ogni fonte citata per URL e data di accesso. Questa skill entra in gioco **solo** quando:
- l'utente chiede esplicitamente che una ricerca la faccia GPT (ChatGPT Deep Research);
- un `implementation-risk-audit` di GPT apre da solo una `external_research` (fasi critiche, skill `plan` passo 6-bis);
- serve un contratto vicino — `double-check`, `status-tracker`, `plan-gate` (sezione sotto).

## Il canale

Controller **AgentBridge** su `http://127.0.0.1:8765` (solo loopback), contratto **`ask-deepsearch`**, progetto `CorporateKnowledgeAssistant`. Ogni comando passa da `POST /api/projects/CorporateKnowledgeAssistant/commands` oppure dall'MCP `execute_contract`. Gli schemi completi stanno in `C:\Agent Bridge\docs\contracts.md`, l'uso operativo e la lettura della dashboard in `C:\Agent Bridge\docs\HANDOFF.md`.

**Il controller è una macchina a stati, non una cartella.** Non ispezionare né modificare a mano i file sotto `C:\Agent Bridge\CorporateKnowledgeAssistant\`: solo un comando valido sposta un file, e solo un `ACK-RESPONSE` valido archivia una coppia in `Solved/`.

Se il controller non risponde, è spento: si avvia con `.venv\Scripts\python.exe -m bridge.main` dalla radice `C:\Agent Bridge`. Non inventare un canale di ripiego e non chiedere all'utente di eseguire il prompt a mano; se l'utente preferisce, la ricerca torna al canale di default di Claude.

## Pubblicare una richiesta

1. L'agente che scopre una lacuna riferisce all'orchestratore solo quesito, fase, decisioni già fissate, fonti necessarie e impatto. **Un solo orchestratore scrive**: evita comandi concorrenti sullo stesso `task_id`.
2. Assegna un `content_id` mai usato nella forma `deepsearch_N` (`^deepsearch_[1-9][0-9]*$`, numerazione progressiva). Un nuovo ID equivale a una nuova ricerca; puoi mandare più ID in un `WRITE-REQUEST`, uno per quesito autonomo. Non riusare un ID archiviato per un'integrazione: creane uno nuovo che cita il precedente.
3. Il markdown della richiesta **inizia con un titolo `#` breve e specifico** — è il titolo della riga in dashboard. Scrivi un prompt autosufficiente: domanda ristretta, contesto e decisioni ratificate, data, fonti da privilegiare (RFC, OWASP, documentazione ufficiale di crate, PostgreSQL/pgvector, JEV), formato del rapporto. Il destinatario non vede il repository: inserisci nel prompt i fatti indispensabili e dichiara quali ricerche sono già acquisite — anche quelle fatte da Claude, con le loro fonti — e **non vanno rifatte**. Firma la richiesta `Claude (Anthropic)`.
4. Invia `WRITE-REQUEST` con `agent`, `phase` (intero positivo), `task_id: "ask-deepsearch"` e `contents`. Verifica nella risposta che ogni `content_id` risulti `PENDING`. Conserva gli ID per il controllo del ritorno.

## Attendere e riprendere

1. Sospendi i passi che dipendono dalla ricerca. Puoi svolgere lavoro indipendente; non duplicare in parallelo la stessa ricerca nel canale di Claude.
2. La riga passa a **`READY`** quando GPT ha scritto la risposta. Leggila con `READ-RESPONSE`: la lettura è ripetibile e **non archivia nulla**, e restituisce un **`delivery_token`**.
3. Conferma con **`ACK-RESPONSE`** passando quel `delivery_token`. Solo allora la coppia si sposta in `Solved/` e la riga diventa `SOLVED`. Il token copre i byte esatti della risposta: se la risposta cambia, il token non vale più. **Una risposta letta e mai confermata resta appesa in dashboard e blocca la lettura del prossimo giro** — l'ACK non è facoltativo.
4. Accetta il risultato solo quando rapporto e fonti ci sono davvero. Se manca o è incongruente, **non confermare**: lascia la riga `READY`, segnala il problema e non inventare una risposta.
5. **Nessuna riverifica delle fonti di GPT**: la ricerca è supervisionata dall'utente, il rapporto si usa come evidenza così com'è — niente WebFetch/WebSearch sugli URL che cita. L'orchestratore confronta il risultato con le decisioni e il codice locali e redige l'artefatto di fase firmato **Claude (Anthropic)**, attribuendo separatamente **ChatGPT Deep Research (OpenAI)** con il `content_id`. Deep Research fornisce evidenza; scelte di prodotto e oracoli di evaluation seguono le ratifiche previste dal loop.

## I contratti vicini

- **`double-check`** — il canale del **verificatore esterno** del triplo guardrail sugli oracoli di evaluation, **sempre** usato quando un piano introduce o cambia un valore atteso (domanda, risposta, citazione attesa, soglia). Non è un secondo canale di ricerca. `content_id` nella forma `doublecheck_N`, `recipient` diverso dal mittente, consegna in sessione pulita (`context_policy: CLEAN`): GPT riceve solo il documento sorgente e la derivazione di Claude, **mai il codice**. Risposta con `verdict` fra `CONFIRMED`, `PARTIALLY_CONFIRMED`, `REJECTED`, `UNDETERMINED`. Stesso ciclo `READ-RESPONSE` → `ACK-RESPONSE`. Dettaglio del guardrail: skill `eval-oracle-check`.
- **`status-tracker`** — registra l'avanzamento del verbo in corso con `APPEND-STATUS` (`stage`: `discuss | plan | exec | verify`; `status`: `STARTED | PROGRESS | COMPLETED | BLOCKED | FAILED`; `phase` intero). Il `summary` è il titolo in dashboard: breve e specifico. Le voci sono **immutabili** — una correzione è una nuova voce con `supersedes`, e la scrive solo l'autore originario. Si aggiorna a ogni tappa reale. **Non sostituisce `.planning/ORCHESTRATOR-STATUS.md`**, che resta lo stato del loop e lo scrive solo `update-orchestrator-state`.
- **Revisione dei piani, solo fasi critiche** (identity/autenticazione, RBAC con retrieval autorizzato, ingestion): `implementation-risk-audit` (`risk_audit_N`) e `test-design` (`test_design_N`), Claude → GPT; `plan-gate` con `REGISTER-PLAN` (solo Claude) e `READ-GATE`. Stesse operazioni e stesso ciclo `READ-RESPONSE` → `ACK-RESPONSE`, ma la parte controllata è il **primo blocco ```yaml**. Uso nel loop: skill `plan` passo 6-bis ed `execute`. **Le `external_research` di un audit aprono da sole una `ask-deepsearch`**, a nome di chi ha chiesto l'audit: si gestiscono con questa skill, lettura e ACK compresi. Sulle fasi non critiche il test design lo scrive un agente Claude separato e questi contratti non si usano.
- **`test-write`** verso Codex: lo slot esiste nel controller ma in questo progetto è **spento per decisione utente**; i test rossi li scrive un agente Claude fresco in `execute`.

## Aggancio GSD

- `plan`: dopo la lettura del rapporto e la redazione di `0X-RESEARCH.md`, usa `gsd-plan-phase X --skip-research`. Mai `--research`, che rispawna `gsd-phase-researcher`.
- `discuss`: se l'utente affida a GPT più aree, usa ID distinti quando i quesiti sono autonomi. Se GSD entra in advisor mode, non eseguire `advisor_research` sulle aree già coperte dal rapporto.
- `verify`: il triage UAT resta locale (`ricerca-punti-aperti`). Un punto ricercabile affidato a GPT ottiene un ID; il risultato è una proposta con fonte e non chiude la UAT.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
