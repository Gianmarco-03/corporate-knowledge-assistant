---
name: ricerca-punti-aperti
description: Ricerca pre-UAT del loop GSD+ECC di Corporate Knowledge Assistant — classifica i punti aperti di una verifica in giudizi puri e punti ricercabili, ricerca i secondi (Claude per default, GPT Deep Research solo su richiesta dell'utente) e li presenta come proposte con fonte, divise per specie: conferme a un tocco → orientati → giudizi puri. USARE come passo 4 di verify, prima di portare all'utente UAT, punti human_verification o decisioni pendenti, oppure standalone su qualsiasi UAT con punti aperti. Non chiude mai una UAT.
---

# ricerca-punti-aperti

Skill di servizio del loop GSD+ECC (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md` §9-bis). Input: l'elenco dei punti aperti di una verifica (voci `human_verification` di `0X-VERIFICATION.md`, voci pendenti di `0X-UAT.md`, decisioni pendenti), già filtrato dal passo 3-bis di `verify` (i punti su aree di fasi successive non arrivano qui). Output: gli stessi punti, classificati e — dove una fonte parla — già armati della risposta proposta con citazione.

## Passo 1 — Triage (nessun agente)

Per ogni punto aperto, decidi a tavolino la specie:

- **Giudizio puro** — estetica, gusto, preferenza d'uso, leggibilità di una risposta o di una citazione nel viewer, verifica visiva dal vivo: NESSUNA fonte può risolverlo. Il punto passa alla presentazione così com'è. (Regola del loop: «UAT: solo giudizio umano» — qui il giudizio è davvero solo dell'utente.)
- **Ricercabile** — il punto tocca una specifica o uno standard (JWT, OWASP ASVS, OWASP Top 10 for LLM Applications, GDPR per i dati personali nei log, accessibilità), una convenzione tecnica o UX consolidata, il comportamento documentato di una libreria, di PostgreSQL/pgvector o di JEV, oppure la coerenza con una decisione già ratificata del progetto. Va in ricerca dedicata.

In dubbio → ricercabile. La ricerca resta mirata al punto, senza ripetere l'intera fase.

## Passo 2 — Ricerca dei punti ricercabili

**Canale di default: Claude.** WebSearch/WebFetch, context7 per librerie e crate, `anthropic-skills:deep-research` quando il punto richiede una sintesi multi-fonte; punti che condividono le stesse fonti si ricercano insieme. Ogni affermazione porta la fonte: URL e data di accesso, oppure file e sezione del repo.

**GPT solo su richiesta dell'utente.** Se l'utente vuole che un punto lo ricerchi ChatGPT Deep Research, prepara con `deep-research-handoff` una richiesta `ask-deepsearch` per quesito distinto (o una per gruppo con le stesse fonti), attendi `READY`, leggi e conferma con `ACK-RESPONSE`. Su quel rapporto vale la regola GPT: non si riverificano le fonti, si attribuisce con il `content_id`.

Per ogni punto valuta TRE lenti:

- **Fonte autorevole**: la specifica, l'RFC, lo standard o la norma che tocca il punto, citata al punto preciso. Se la fonte tace, dirlo esplicitamente.
- **Pratica consolidata**: convenzioni UX e tecniche diffuse per assistenti RAG con citazioni verificabili, pattern dello stack.
- **Decisioni del progetto**: `DEPENDENCIES.md`, `docs/architecture/`, `docs/adr/`, `.planning/DECISIONS-INDEX.md`, CONTEXT e CONTRATTO delle fasi. Prevalgono sull'esterno finché l'utente non le riapre; se il punto contraddice una decisione ratificata, la proposta lo dice.

## Passo 3 — Classificazione dell'esito

Ogni punto ricercabile torna in una di due specie (la terza viene dal triage):

1. **Chiuso dalla fonte** — una fonte o una decisione ratificata impone una risposta univoca → si presenta come conferma a un tocco: «La fonte X dice così → il punto si risolve così. Confermi?»
2. **Orientato** — la fonte autorevole tace ma pratica consolidata o decisioni del progetto indicano una direzione → si presenta con raccomandazione motivata e fonte.
3. **Giudizio puro** (dal triage) — si presenta nudo.

## Passo 4 — Registrazione e presentazione

- Il finding entra nella voce UAT come **proposta con fonte** (campo `decision` precompilato con testo proposto + citazione), MAI come decisione presa: la ratifica è sempre dell'utente.
- La presentazione all'utente arriva **divisa per specie**, nell'ordine: **conferme a un tocco → orientati → giudizi puri**.
- **Stile**: finding interni in caveman full con fonti puntuali; la presentazione all'utente resta in prosa normale. Le proposte sono firmate Claude (Anthropic); un rapporto GPT si attribuisce separatamente a ChatGPT Deep Research (OpenAI) con il `content_id`.
- Stato: il verbo chiamante registra via `update-orchestrator-state` (`--verify` o `--uat`); in standalone usa `--uat`.

## Guardrail

- **Mai auto-risolvere**: nessun punto cambia stato senza risposta dell'utente.
- **Oracoli di evaluation**: un punto che tocca un valore atteso, una citazione attesa o una soglia segue il triplo guardrail di `eval-oracle-check` (derivazione Claude separata, `double-check` GPT, ratifica utente) — la ricerca lo alimenta, non lo sostituisce.
- **Fonte non trovata ≠ fonte contraria**: se la ricerca non trova nulla, il punto resta orientato o nudo — mai inventare una fonte.

## Cosa NON fa

Non giudica l'estetica, non chiude UAT, non scrive VERIFICATION o STATE, non ricerca punti su aree di fasi successive.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
