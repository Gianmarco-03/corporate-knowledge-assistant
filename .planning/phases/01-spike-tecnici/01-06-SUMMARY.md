---
phase: 01-spike-tecnici
plan: 06
subsystem: testing
tags: [oracoli, evaluation, triplo-guardrail, agentbridge, double-check, gpt, ratifica, query-etichettate, parser-it, cer, acl, spike-data]

# Dependency graph
requires:
  - phase: 01-spike-tecnici (piano 01-03)
    provides: "spikes/data/corpus/manifest.json (doc_id, classificazione, pagine), acl.json (matrice ruolo × classificazione, profilo restricted-only), synthetic/*.md, fetch.sh per ricostruire raw/, CORPUS.md (società inventata, nomi della trappola controllati sui PDF)"
provides:
  - "spikes/data/corpus/oracles/queries.json: 80 query etichettate (32 it, 32 en, 8 cross-it-en, 8 cross-en-it) con expected_passages verbatim, role_profile, complexity con complexity_ref, no_answer; 12 rifiuti attesi (8 assenti dal corpus, 4 trappole su syn-09), 7 su documenti con injection-case"
  - "spikes/data/corpus/oracles/parser-it.json: 11 oracoli del parser italiano (6 page-text, 4 table con celle {row, col, text}, 1 table-pages: der-it-01 = [12])"
  - "spikes/data/corpus/oracles/derivations/*.md: per ogni oracolo documento, punto esatto, estratto verbatim, svolgimento, motivazione della complessità, guardrail.claude"
  - "spikes/data/corpus/oracles/doublecheck/doublecheck_1..7.md: richieste inviate e risposte di GPT trascritte verbatim con delivery_token"
  - "spikes/data/corpus/oracles/ORACOLI.md: regole, classi, convenzioni (pagina fisica 1-based, normalizzazione), regola di complessità, conteggi, stato del guardrail, TO-DO di dominio"
  - "Insieme utilizzabile nelle misure (tre assi verdi): 74 query su 80 e 11 oracoli del parser su 11"
affects: [01-07, 01-08, 01-09, 01-10, 01-11, 01-12, 01-14, S-02, S-03, S-04, A-04]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Triplo guardrail per oracolo: guardrail.claude (derivatore), guardrail.gpt (double-check su AgentBridge, solo CONFIRMED è verde), guardrail.user (ratified con ratified_at)"
    - "Oracolo pendente = todo_ref valorizzato che punta a una voce della sezione «TO-DO di dominio» di ORACOLI.md; resta fuori dalle misure"
    - "PDF letti come immagini rasterizzate (pdftoppm nel runner, 110-150 dpi), mai con un estrattore di testo"
    - "Numeri sempre stringhe con i separatori del documento; pagina = indice fisico del PDF, printed_page a parte"
    - "Anti-allineamento: dopo un dissenso del verificatore valore, passaggi, complessità, ruolo e classe restano identici al commit di derivazione"

key-files:
  created:
    - spikes/data/corpus/oracles/ORACOLI.md
    - spikes/data/corpus/oracles/queries.json
    - spikes/data/corpus/oracles/parser-it.json
    - spikes/data/corpus/oracles/derivations/queries-it.md
    - spikes/data/corpus/oracles/derivations/queries-en.md
    - spikes/data/corpus/oracles/derivations/queries-cross.md
    - spikes/data/corpus/oracles/derivations/parser-it-text.md
    - spikes/data/corpus/oracles/derivations/parser-it-tables.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_1.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_2.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_3.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_4.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_5.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_6.md
    - spikes/data/corpus/oracles/doublecheck/doublecheck_7.md
  modified: []

key-decisions:
  - "Utente 2026-10-21, checkpoint 01-06: «ratifica-tutti», cioè ratifica di tutti gli 85 oracoli CONFIRMED (74 query, 11 parser)"
  - "Utente 2026-10-21: deviazione del derivatore accettata (derivatore = esecutore del piano, distinto da orchestratore ed esecutori degli spike); TEST-10 resta deviazione accettata, non verde"
  - "Utente 2026-10-21: TODO-01 chiuso con «teneteli», P-006 conserva riquadro WEB e didascalia"
  - "Utente 2026-10-21: TODO-02 chiuso con «resta la regola attuale»; Q-008, Q-010, Q-012, Q-040, Q-048, Q-056 restano simple ma fuori dalle misure (guardrail.gpt PARTIALLY_CONFIRMED); reinviabili a GPT con la regola esplicita"
  - "Pagina = indice fisico 1-based del PDF, printed_page separato"
  - "Rifiuti per assenza nei PDF fondati su CORPUS.md (Alfa Componenti inventata; nomi della trappola controllati da 01-03 su tutti i PDF)"
  - "Lotti GPT con il testo integrale dei sintetici citati o controllati, non solo gli estratti"

patterns-established:
  - "Oracoli di misura degli spike in sola lettura per tutti gli altri piani; i consumatori filtrano sui tre assi verdi e dichiarano nei RISULTATI il numero degli esclusi"
  - "Un dissenso del verificatore si registra come TO-DO di dominio, mai come correzione del valore"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Le query etichettate (60-100, IT/EN/cross-lingual, numeriche, senza risposta) e gli oracoli del parser italiano esistono con provenienza qualificante, derivati dal documento sorgente e mai dall'output di una pipeline (D-11, D-14, eval-oracle-check)"
    verification:
      - {test: TEST-1, risks: [RISK-3, RISK-1], ref: "spikes/data/corpus/oracles/queries.json ; parser-it.json ; verify del Task 1 (node -e, exit 3/5/6) ; controllo del bridge", status: pass, evidence: "rieseguito dal bridge sul main: classi solo nella whitelist chiusa (75 estratto-sorgente, 16 derivazione-documentata, 0 ratifica-utente su 91), dataset_version spike-01 e test_id null ovunque, ogni derivation_ref punta a un file esistente in derivations/, ogni query con risposta di classe estratto-sorgente ha expected_passages con excerpt non vuoti"}
      - {test: TEST-2, risks: [RISK-2, RISK-1], ref: "secondo node -e del verify del Task 1 (sottostringa dopo NFC e collasso degli spazi, exit 9/10)", status: pass, evidence: "rieseguito dal bridge sul main: exit 0, ogni excerpt su documento md è sottostringa del file synthetic/ corrispondente"}
      - {test: TEST-3, risks: [RISK-2, RISK-1], ref: "manuale: campione di 10 query su PDF e 2 pagine di parser-it.json confrontati a mano con raw/", status: human_judgment, human_judgment: true, rationale: "il design lo assegna a un controllo umano con seme scritto; né l'esecutore né il bridge l'hanno fatto, e un controllo automatico sui PDF userebbe un estrattore di testo. Gli estratti PDF sono stati letti a vista da immagini rasterizzate e confermati da GPT, ma la verifica carattere per carattere resta da fare"}
      - {test: TEST-4, risks: [RISK-1], ref: "grep -rniE 'pdftotext|pdfium|docling|tesseract|tei|embed|rerank|systemone|anthropic|results/' su derivations/ e ORACOLI.md", status: pass, evidence: "rieseguito dal bridge: a parte le firme «Claude Opus 5.5 (Anthropic)», le occorrenze sono ORACOLI.md:3 (S-04 rerank come consumatore), :19 (regola che vieta parser, embedding, reranker), :111 (rerank non dipende da TODO-02) e parser-it-tables.md:129 (Docling come consumatore di P-011). Nessuna è una fonte di valore; il diff f7e4fad..54f323d non tocca spikes/s0*/results/"}
      - {test: TEST-13, risks: [RISK-12], ref: "verify del Task 1 (exit 8) ; controllo ricorsivo del bridge", status: pass, evidence: "rieseguito dal bridge: nessun typeof number in expected_value né in expected_passages, escluse le sole coordinate row, col, page"}
  - must_have: "Ogni oracolo ha il triplo guardrail completo: agente Claude separato dall'esecutore, double-check GPT su AgentBridge, ratifica dell'utente; un oracolo con dissenso resta pendente e fuori dalle misure"
    verification:
      - {test: TEST-5, risks: [RISK-4], ref: "verify del Task 3 (node -e) ; ORACOLI.md:77-113", status: pass, evidence: "rieseguito dal bridge: verify Task 2 e Task 3 exit 0; 74 query e 11 oracoli del parser con tre assi verdi (minimo 60); guardrail.gpt 85 CONFIRMED e 6 PARTIALLY_CONFIRMED; i 6 pendenti hanno todo_ref TODO-02, presente nella sezione «TO-DO di dominio»; nessun oracolo verde porta todo_ref; il conteggio dei pendenti coincide con ORACOLI.md:85"}
      - {test: TEST-10, risks: [RISK-9], ref: "ORACOLI.md:23-25 (Agente derivatore) ; guardrail.claude di ogni voce ; firme di derivations/*.md", status: human_judgment, human_judgment: true, rationale: "il derivatore (Claude Opus 5.5, sessione gsd-executor 01-06) è anche l'esecutore del piano, contro quanto chiedeva il Task 1; è distinto dall'orchestratore e dagli esecutori degli spike. L'utente ha accettato la deviazione il 2026-10-21: resta una deviazione accettata, non un verde"}
      - {test: TEST-11, risks: [RISK-10], ref: "doublecheck/doublecheck_1..7.md ; git show 7e29c99 contro HEAD su queries.json e parser-it.json ; rapporto-01-06.yaml doublecheck_inviati", status: pass, evidence: "rieseguito dal bridge: 7 file, uno per lotto, ciascuno con «GPT (OpenAI), content_id doublecheck_N», firma di trascrizione di Claude e delivery_token; anti-allineamento: expected_value, expected_passages, source_ref, complexity, no_answer, text e role_profile identici fra 7e29c99 e HEAD su tutte le 91 voci (diff 0). L'ordine salvataggio → ACK è dichiarato nei file e nel rapporto (ack SOLVED per i 7 content_id); il bridge non ha interrogato AgentBridge"}
      - {test: TEST-12, risks: [RISK-11], ref: "grep -nE '```(rust|bash|json)|system prompt|cargo ' doublecheck/*.md ; manifest.json (classificazioni)", status: pass, evidence: "rieseguito dal bridge: nessun blocco di codice né prompt di sistema nella parte inviata; le uniche corrispondenze («system prompt») sono la fixture di injection inj-08-d di syn-08 citata nel testo integrale del sintetico, presente in doublecheck_1, 2, 4 e 5 (il rapporto la conta come una sola occorrenza). Tutti i documenti citati sono pubblici, sintetici o derivati"}
  - must_have: "Ogni query porta il profilo di ruolo, così S-03, S-04 e A-04 possono verificare che nessun passaggio non autorizzato raggiunga reranker, JEV o LLM (D-16)"
    verification:
      - {test: TEST-6, risks: [RISK-5], ref: "queries.json ; acl.json ; manifest.json (classification)", status: pass, evidence: "rieseguito dal bridge con la visibilità calcolata da acl.json + manifest: ogni query con no_answer false ha almeno un passaggio visibile e nessuno invisibile; nessuna query con no_answer true ha passaggi visibili; 4 trappole su syn-09 con ruolo che non lo vede (Q-015, Q-016, Q-017, Q-076)"}
      - {test: TEST-7, risks: [RISK-6], ref: "queries.json ; parser-it.json ; manifest.json ; acl.json", status: pass, evidence: "rieseguito dal bridge: ogni document_id (source_ref, documents_checked, expected_passages) esiste nel manifest; ogni role_profile è chiave di acl.json roles o profiles; oracle_id univoci nella forma Q-NNN e P-NNN"}
  - must_have: "Ogni query porta un'etichetta di complessità simple/complex con regola scritta, usata per misurare il routing per complessità deciso da JEV (D-18)"
    verification:
      - {test: TEST-9, risks: [RISK-8], ref: "ORACOLI.md:51-58 ; commit 7e29c99 ; derivations/queries-*.md (riga «Complessità»)", status: pass, evidence: "rieseguito dal bridge: ORACOLI.md e queries.json nascono entrambi in 7e29c99 (la regola coincide col primo commit delle etichette, ammesso dal design); ogni query ha complexity_ref e una riga di motivazione nel file di derivazione; 68 simple, 12 complex"}
  - must_have: "Artefatti queries.json, parser-it.json, ORACOLI.md con la distribuzione richiesta"
    verification:
      - {test: TEST-8, risks: [RISK-7], ref: "verify del Task 1 ; controllo del bridge", status: pass, evidence: "rieseguito dal bridge: 80 query (target 80), 32/32/16 (8+8), 12 no_answer (≥ 8), 60 con valore numerico o sigle (≥ 12), 7 su documenti con injection-case (≥ 4); parser-it 6 page-text, 4 table con celle {row, col, text}, 1 table-pages"}

# Metrics
duration: 57min
completed: 2026-10-21
---

# Phase 1 Plan 06: Oracoli di misura degli spike Summary

**80 query etichettate e 11 oracoli del parser italiano derivati a vista dai documenti sorgente, passati dal double-check di GPT (91/91 verdetti, 85 CONFIRMED) e ratificati dall'utente: 74 query e 11 oracoli del parser entrano nelle misure di S-02, S-03, S-04 e A-04, sei query restano escluse per un dissenso sulla sola complessità**

## Performance

- **Duration:** circa 57 min dalla base dell'onda (`f7e4fad`, 2026-10-21 14:15) all'ultimo commit del piano (`54f323d`, 15:12), ora italiana; il merge `af705f3` è delle 15:51
- **Started:** 2026-10-21T14:15:44+02:00 (base dell'onda; l'ora d'inizio reale dell'esecutore non è registrata)
- **Completed:** 2026-10-21T15:12:30+02:00
- **Tasks:** 3 di 3 (Task 3 è il checkpoint di ratifica dell'utente)
- **Files modified:** 15 creati, tutti dentro `files_modified` (i 7 file di double-check coperti dal glob dichiarato nel PLAN per la cartella doublecheck)

## Accomplishments

- `queries.json`: 80 query, 32 in italiano, 32 in inglese, 16 cross-lingual (8 per direzione). 12 rifiuti attesi: 8 su fatti assenti dal corpus e 4 trappole la cui unica risposta sta in `syn-09`, invisibile al ruolo della query. 7 query su documenti con casi di injection, 60 con valore numerico o sigle finanziarie. Ruoli: guest 40, employee 17, manager 17, admin 5, restricted-only 1.
- `parser-it.json`: testo atteso di 6 pagine (`pub-it-01` pagine 11-13, `pub-it-03` pagine 5, 6 e 12, quest'ultima a due colonne con fotografia), 4 tabelle con celle (Tavola 1.1 della Relazione annuale; Tavole 2, 3 e 7 del Bollettino) e l'elenco delle pagine con tabelle dell'estratto TTQ da 25 pagine: la sola 12.
- Derivazioni per oracolo in cinque file, con estratto verbatim, svolgimento e motivazione della complessità.
- Double-check GPT in sette lotti su AgentBridge: 91/91 verdetti, 85 `CONFIRMED` e 6 `PARTIALLY_CONFIRMED`, nessun `REJECTED` o `UNDETERMINED`.
- Ratifica dell'utente del 2026-10-21 su tutti gli 85 `CONFIRMED`; nessun valore riallineato al verificatore.

## Double-check su AgentBridge

| content_id | Oracoli | Verdetto del lotto | Non `CONFIRMED` | ACK-RESPONSE |
|---|---|---|---|---|
| `doublecheck_1` | Q-001…Q-016 | `PARTIALLY_CONFIRMED` | Q-008, Q-010, Q-012 | confermato (SOLVED) |
| `doublecheck_2` | Q-017…Q-032 | `CONFIRMED` | — | confermato (SOLVED) |
| `doublecheck_3` | Q-033…Q-048 | `PARTIALLY_CONFIRMED` | Q-040, Q-048 | confermato (SOLVED) |
| `doublecheck_4` | Q-049…Q-064 | `PARTIALLY_CONFIRMED` | Q-056 | confermato (SOLVED) |
| `doublecheck_5` | Q-065…Q-080 | `CONFIRMED` | — | confermato (SOLVED) |
| `doublecheck_6` | P-001…P-006 | `CONFIRMED` | — | confermato (SOLVED) |
| `doublecheck_7` | P-007…P-011 | `CONFIRMED` | — | confermato (SOLVED) |

Ogni risposta è trascritta verbatim nella sezione «Risposta» del file, con attribuzione «GPT (OpenAI), content_id doublecheck_N», firma di trascrizione e `delivery_token`, prima dell'ACK. I sei dissensi riguardano solo l'etichetta di complessità: GPT conferma valore ed estratto e considera `complex` una risposta con più elementi in una sola frase. Lo stato degli ACK viene dal rapporto dell'esecutore e dai token nei file; il bridge non ha interrogato AgentBridge. La voce `status-tracker` della tornata è `status_11` (stage `exec`, `PROGRESS`).

## Task Commits

1. **Task 1: derivazione degli oracoli** - `7e29c99` (feat)
2. **Task 2: double-check GPT** - `65f66dc` (docs: richieste inviate) e `707163b` (feat: esito per oracolo, dissensi come TO-DO)
3. **Task 3: ratifica dell'utente** - `54f323d` (feat)

Merge su main: `af705f3`.

## Files Created/Modified

- `spikes/data/corpus/oracles/ORACOLI.md` - regole, classi, convenzioni, regola di complessità, conteggi, stato del guardrail, TO-DO di dominio
- `spikes/data/corpus/oracles/queries.json` - 80 query etichettate
- `spikes/data/corpus/oracles/parser-it.json` - 11 oracoli del parser italiano
- `spikes/data/corpus/oracles/derivations/queries-it.md`, `queries-en.md`, `queries-cross.md` - derivazione per query
- `spikes/data/corpus/oracles/derivations/parser-it-text.md`, `parser-it-tables.md` - testo atteso delle pagine, celle delle tabelle, controllo pagina per pagina dell'estratto TTQ
- `spikes/data/corpus/oracles/doublecheck/doublecheck_1.md` … `doublecheck_7.md` - richieste a GPT e risposte verbatim

## Decisions Made

- **Ratifica (utente, 2026-10-21, checkpoint 01-06):** «ratifica-tutti» su tutti gli oracoli `CONFIRMED`. `guardrail.user = ratified` e `ratified_at = 2026-10-21` su 85 voci. Nessuna voce ha classe `ratifica-utente`, quindi nessun `ratification_ref` è richiesto.
- **TODO-01 (utente): «teneteli».** P-006 conserva il riquadro «WEB» e la didascalia della fotografia; `todo_ref` torna `null` e P-006 è ratificato.
- **TODO-02 (utente): «resta la regola attuale».** La regola di complessità non cambia e le sei query restano `simple`. Restano comunque fuori dalle misure, perché `guardrail.gpt` non è `CONFIRMED`; si possono reinviare a GPT in un nuovo lotto con la regola scritta come criterio esplicito.
- **Derivatore (utente): deviazione accettata.** Il derivatore è l'esecutore del piano e non un agente fresco; l'indipendenza dalla pipeline e dagli esecutori degli spike è rispettata.
- **Lettura dei PDF:** pagine rasterizzate con `pdftoppm` nel runner (`session.sh with 01-06`, 110-150 dpi) e lette a vista; nessuna pipeline e nessuna estrazione testuale.
- **Convenzioni:** pagina = indice fisico 1-based, `printed_page` a parte; estratti PDF brevi (una frase con il numero) per ridurre gli errori di trascrizione; un unico TO-DO per i sei dissensi, perché la domanda è una sola.

## Agente derivatore

Ha derivato gli oracoli **Claude Opus 5.5 (Anthropic), sessione `gsd-executor` del piano 01-06**, nel worktree `exec/01-06`. La stessa identità compare in `guardrail.claude` di tutte le 91 voci e nelle firme di `derivations/*.md`. **Non è distinto dall'esecutore del piano**: è la stessa sessione, contro quanto chiedeva il Task 1 (agente fresco). È distinto dall'orchestratore di `execute` e dagli esecutori di S-02, S-03, S-04 e A-04. L'utente ha accettato la deviazione il 2026-10-21 (`ORACOLI.md:25`); TEST-10 resta `human_judgment`, non verde.

## Deviations from Plan

### Deviazioni accettate dall'utente

**1. Derivatore = esecutore del piano (TEST-10)**
- **Found during:** Task 1 · **Issue:** nella sessione dell'esecutore non era possibile avviare un sotto-agente fresco · **Esito:** dichiarata in ORACOLI.md, accettata dall'utente al checkpoint del Task 3 · **Committed in:** `54f323d`

### Auto-fixed Issues (livello a)

**2. AgentBridge via REST invece del tool MCP**
- **Found during:** Task 2 · **Issue:** `mcp__agentbridge__execute_contract` non caricabile, ToolSearch non disponibile in sessione · **Fix:** stesso contratto via `POST /api/projects/CorporateKnowledgeAssistant/commands` sul loopback · **Committed in:** `65f66dc`

**3. Skill caveman non caricata**
- **Issue:** nessuno strumento Skill nella sessione dell'esecutore · **Fix:** rapporto in YAML sintetico · **Committed in:** nessun commit

---

**Total deviations:** 3 (1 accettata dall'utente, 2 di livello a); nessuna di livello b o c.
**Impact on plan:** nessun file fuori da `files_modified`; `node scripts/gate-proprieta-file.cjs --fase 1` rieseguito dal bridge: conforme.

## Issues Encountered

- `raw/` non serve ai verify automatici (che leggono solo JSON e sintetici md); i render temporanei delle pagine sono stati rimossi da `raw/` dall'esecutore.

## Discrepanze fra rapporto e prove

- TEST-12: il rapporto parla di un'«unica occorrenza» di «system prompt»; il grep ne trova quattro, in `doublecheck_1`, 2, 4 e 5. È sempre la stessa fixture `inj-08-d` di `syn-08`, inclusa nel testo integrale del sintetico inviato in quattro lotti: l'esito non cambia.
- Il rapporto cita un «generatore unico dai dati derivati a mano» che ha prodotto JSON e derivazioni: lo script non è versionato e il bridge non lo ha letto. I controlli del bridge sono stati scritti da zero a partire dal design.
- Nessun'altra discrepanza: commit, file, conteggi, verdetti per lotto, esclusi e anti-allineamento coincidono con le prove.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: info | spikes/data/corpus/oracles/doublecheck/*.md | i lotti contengono il testo integrale dei sintetici, comprese le fixture di injection di syn-07 e syn-08: sono dati, inviati a GPT come contenuto da verificare (D-16, documenti pubblici o sintetici) |
| threat_flag: tampering | spikes/data/corpus/oracles/queries.json, parser-it.json | gli estratti dai PDF non hanno un controllo meccanico (TEST-3 manuale da fare): un errore di trascrizione a vista confermato anche da GPT passerebbe nelle misure |

## Known Stubs

Nessuno.

## User Setup Required

Nessuno.

## Punti aperti

- **TEST-3 da fare:** campione di 10 query su PDF e 2 pagine di `parser-it.json` da confrontare a mano con le pagine di `raw/`, con seme scritto. Una discrepanza riapre il lotto.
- **Sei query escluse** (Q-008, Q-010, Q-012, Q-040, Q-048, Q-056): si recuperano solo con un nuovo double-check che riporti la regola di complessità come criterio esplicito, seguito dalla ratifica. Fino ad allora la misura del routing per complessità di A-04 gira su 74 query.
- **Recall dell'euristica delle pagine-tabella (P-011):** l'estratto TTQ ha una sola pagina con tabelle (la 12), quindi la recall di S-02 poggia su un solo positivo; le pagine 5-8 sono i negativi difficili.
- **Rifiuti sui PDF:** l'assenza di evidenza nei PDF pubblici poggia su CORPUS.md (Alfa inventata, nomi della trappola controllati da 01-03), non su una rilettura dei PDF da parte di questo piano.

## Next Phase Readiness

- S-02, S-03, S-04 e A-04 possono partire: filtrano gli oracoli con i tre assi verdi e dichiarano nei RISULTATI gli esclusi (6 query).
- PLAT-07 è copiato dal frontmatter del piano: qui sono attuate le mitigazioni T-01-06-01..04, non il requisito intero. Se il requisito sia chiuso lo decide il verifier di fase.

## Self-Check: PASSED

- I 15 file del diff `f7e4fad..54f323d` stanno tutti in `files_modified`, e ogni percorso di `files_modified` è toccato.
- I commit `7e29c99`, `65f66dc`, `707163b`, `54f323d` e il merge `af705f3` sono nella storia di main.
- Rieseguiti dal bridge sul main: verify dei Task 1, 2 e 3 (exit 0; 7 file di double-check), controlli TEST-1, 4, 5, 6, 7, 8, 9, 11, 12, 13 riscritti da zero, gate proprietà dei file (conforme).

---
*Phase: 01-spike-tecnici*
*Completed: 2026-10-21*

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-06
