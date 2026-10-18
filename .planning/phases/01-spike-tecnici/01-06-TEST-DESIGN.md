# Test design 01-06 — oracoli degli spike

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-06-PLAN.md
  sha256: e1b92958ddc3b02307cddffaf437bdbe9e93a012be6db36f35ad022e2d57db6e
risks:
  - id: RISK-1
    severity: CRITICAL
    title: Valore atteso derivato dall'output di una pipeline (parser, retrieval, JEV, LLM) invece che dal documento sorgente
    source: threat_model T-01-06-01
  - id: RISK-2
    severity: HIGH
    title: Excerpt atteso non verbatim (parafrasato, numero alterato, pagina sbagliata), quindi metriche di retrieval e CER falsate
    source: must_have "derivati dal documento sorgente"
  - id: RISK-3
    severity: HIGH
    title: Classe di provenienza non ammessa o refusa accettata dal controllo (il verify esclude solo due classi vietate)
    source: must_have "provenienza qualificante"; skills/eval-oracle-check
  - id: RISK-4
    severity: HIGH
    title: Oracolo usato nelle misure senza triplo guardrail verde (GPT non CONFIRMED o utente non ratificato senza todo_ref)
    source: must_have "triplo guardrail completo ... un oracolo con dissenso resta pendente"
  - id: RISK-5
    severity: HIGH
    title: Etichetta no_answer incoerente con la matrice di visibilità (query trappola che dichiara una risposta visibile al ruolo)
    source: must_have "Ogni query porta il profilo di ruolo"
  - id: RISK-6
    severity: MEDIUM
    title: Riferimenti rotti (document_id non nel manifest, role_profile non in acl.json)
    source: key_link queries.json → manifest.json
  - id: RISK-7
    severity: MEDIUM
    title: Distribuzione delle query fuori specifica (lingue, numeriche, no-answer, trappola, injection) o parser-it incompleto
    source: must_have "60-100 query IT/EN/cross-lingual, numeriche, senza risposta"
  - id: RISK-8
    severity: MEDIUM
    title: Complessità etichettata senza regola scritta prima o senza motivazione
    source: must_have "etichetta simple/complex con regola scritta"
  - id: RISK-9
    severity: MEDIUM
    title: Agente derivatore non distinto dall'esecutore o dagli esecutori degli spike
    source: must_have "agente Claude separato dall'esecutore"
  - id: RISK-10
    severity: MEDIUM
    title: Risposta GPT non salvata verbatim prima dell'ACK, o oracolo allineato al verificatore
    source: threat_model T-01-06-02
  - id: RISK-11
    severity: MEDIUM
    title: Markdown del double-check contiene codice, prompt o output di pipeline, o estratti di documenti non pubblici/sintetici
    source: threat_model T-01-06-03
  - id: RISK-12
    severity: LOW
    title: Numeri salvati come number JSON invece che stringa (perdita di formattazione, 1.000 vs 1000)
    source: Task 1 acceptance
tests:
  - {id: TEST-1, title: Whitelist delle classi di provenienza e campi richiesti per classe, type: check-dati, priority: P0, catches: [RISK-3, RISK-1]}
  - {id: TEST-2, title: Excerpt verbatim presenti nei sorgenti sintetici (match esatto dopo normalizzazione dichiarata), type: check-dati, priority: P0, catches: [RISK-2, RISK-1]}
  - {id: TEST-3, title: Campione di excerpt da PDF controllato a mano contro la pagina indicata, type: manuale, priority: P0, catches: [RISK-2, RISK-1]}
  - {id: TEST-4, title: Nessun file di derivazione cita output di pipeline o artefatti degli spike, type: check-statico, priority: P0, catches: [RISK-1]}
  - {id: TEST-5, title: Guardrail completo per ogni oracolo usabile, pendenti con todo_ref e voce TO-DO, type: check-dati, priority: P0, catches: [RISK-4]}
  - {id: TEST-6, title: Coerenza no_answer con acl.json e classificazione del manifest, type: check-dati, priority: P0, catches: [RISK-5]}
  - {id: TEST-7, title: Integrità referenziale document_id e role_profile, type: check-dati, priority: P1, catches: [RISK-6]}
  - {id: TEST-8, title: Distribuzione delle query e contenuto di parser-it, type: check-dati, priority: P1, catches: [RISK-7]}
  - {id: TEST-9, title: Regola di complessità in ORACOLI.md prima delle etichette e motivazione per ogni query, type: check-git+dati, priority: P2, catches: [RISK-8]}
  - {id: TEST-10, title: Identità del derivatore distinta nel SUMMARY e nei file di derivazione, type: check-documento, priority: P1, catches: [RISK-9]}
  - {id: TEST-11, title: Un doublecheck_N.md per lotto, attribuito, ACK dopo il salvataggio, valori non riallineati, type: check-documento+git, priority: P1, catches: [RISK-10]}
  - {id: TEST-12, title: Lotti inviati contengono solo estratti e derivazioni da documenti pubblici o sintetici, type: check-documento, priority: P1, catches: [RISK-11]}
  - {id: TEST-13, title: Nessun number dentro expected_value ed excerpt, type: check-dati, priority: P2, catches: [RISK-12]}
uncovered: []
```

## Come leggere questo design

Gli oracoli sono il metro di tutte le misure di qualità della fase (retrieval, rerank, LLM, CER del parser). Un oracolo derivato dalla pipeline rende circolare ogni misura, ed è l'unico rischio che classifico CRITICAL. Il piano mette tre guardie (agente separato, GPT, utente) ma il controllo automatico del Task 1 ha due punti deboli che i test chiudono: rifiuta le classi vietate invece di ammettere solo quelle valide (RISK-3), e non verifica che l'excerpt «verbatim» esista davvero nel documento (RISK-2). Il terzo punto debole è la coerenza fra `no_answer` e la matrice ACL (RISK-5): una query trappola etichettata male fa sembrare un leak un comportamento corretto, o viceversa.

Nessun codice Rust: i test sono comandi `node -e`, `grep` e `git` lanciati dalla radice, più due controlli manuali. Leggono i file del piano (`spikes/data/corpus/oracles/**`) e gli input condivisi del piano 01-03 (`manifest.json`, `acl.json`, `synthetic/*.md`), senza scrivere nulla.

## Test

**TEST-1 — Whitelist delle classi.** Per ogni voce di `queries.json` e `parser-it.json`, `provenance_class ∈ {estratto-sorgente, derivazione-documentata, ratifica-utente}` (le classi ammesse di `skills/eval-oracle-check/SKILL.md`; se la skill ne elenca altre qualificanti, l'insieme si copia da lì, mai si allarga a mano). Per classe: `estratto-sorgente` ⇒ `source_ref` non vuoto e, nelle query con risposta, `expected_passages` non vuoto con ogni `excerpt` non vuoto; `derivazione-documentata` ⇒ `derivation_ref` non vuoto e che punta a un file esistente in `derivations/`; `ratifica-utente` ⇒ `ratification_ref` non vuoto. Ogni voce ha `dataset_version = "spike-01"` e `test_id = null`. Il verify del piano esclude solo `hand-crafted` e `generato-dalla-pipeline`: un refuso come `estratto_sorgente` oggi passerebbe.

**TEST-2 — Excerpt verbatim nei sintetici.** Per ogni `expected_passages[]` il cui `document_id` è un sintetico Markdown, l'excerpt, normalizzato con la regola dichiarata in ORACOLI.md (NFC, spazi collassati), è sottostringa del file sorgente normalizzato allo stesso modo, nella sezione indicata da `section_path`. Per i DOCX derivati si usa il Markdown sorgente (`derived_from`). Fallimento = elenco degli `oracle_id` con excerpt non trovato. Questo controllo legge il sorgente, non esegue alcuna pipeline, quindi non viola la regola di indipendenza.

**TEST-3 — Campione dai PDF (manuale).** Per i documenti PDF il confronto automatico richiederebbe un'estrazione di testo che è a sua volta un parser; resta umano. Il verificatore estrae a sorte (seme scritto) 10 query con passaggi su PDF e 2 pagine di `parser-it.json`, apre il PDF in `raw/` alla pagina indicata e conferma l'excerpt carattere per carattere, numeri compresi. Esito e seme nel SUMMARY. Una sola discrepanza riapre il lotto (opzione «rifai» del Task 3).

**TEST-4 — Nessuna traccia di pipeline.** `grep -rniE "pdftotext|pdfium|docling|tesseract|tei|embed|rerank|systemone|anthropic|results/" spikes/data/corpus/oracles/derivations/ spikes/data/corpus/oracles/ORACOLI.md` letto dal verificatore: ogni occorrenza deve essere una regola che lo vieta, mai una fonte di valore. Inoltre il commit degli oracoli non tocca né legge file sotto `spikes/s0*/results/` (il piano 01-06 dipende solo da 01-03, e quei risultati non esistono ancora in wave 3: il controllo serve contro ri-derivazioni future).

**TEST-5 — Guardrail.** È il `node -e` del verify del Task 3, esteso: per ogni oracolo, se `guardrail.claude` è valorizzato contiene l'identità dell'agente; `guardrail.gpt ∈ {CONFIRMED, PARTIALLY_CONFIRMED, REJECTED, UNDETERMINED}`; `guardrail.user ∈ {ratified, null}`; ogni voce che non ha i tre assi verdi ha `todo_ref` e la stringa di `todo_ref` compare nella sezione «TO-DO di dominio» di ORACOLI.md. Le query con tre assi verdi sono ≥ 60. Il conteggio dei pendenti in ORACOLI.md coincide con quello calcolato.

**TEST-6 — Coerenza `no_answer` / visibilità.** Con `acl.json` e le classificazioni del manifest si calcola, per ogni query, l'insieme dei documenti visibili al `role_profile`. Regole: una query con `no_answer: false` ha almeno un `expected_passages[].document_id` visibile al suo ruolo, e nessun passaggio atteso su un documento non visibile; una query le cui risposte stanno solo in documenti non visibili al ruolo ha `no_answer: true`. Si contano le query trappola su `syn-09` con ruolo che non lo vede: devono essere ≥ 4. Il test usa solo i file JSON, quindi è deterministico.

**TEST-7 — Integrità referenziale.** Ogni `source_ref.document_id` e ogni `expected_passages[].document_id` esiste come `doc_id` nel manifest; ogni `role_profile` è una chiave di `acl.json.roles` o di `acl.json.profiles`; `oracle_id` univoci, `Q-NNN` per le query e `P-NNN` per il parser.

**TEST-8 — Distribuzione.** Il `node -e` del verify del Task 1 (60-100 voci, IT ≥ 24, EN ≥ 24, cross ≥ 12, no-answer ≥ 6) è più largo del target dichiarato nell'azione (80 voci, 32/32/16, 8 no-answer). Il test controlla i minimi del verify come soglia bloccante e riporta lo scarto dal target come avviso. In più: ≥ 12 query numeriche o con sigle (EBITDA, PFN, CET1: `grep` sul `text`), ≥ 4 query su documenti con `injection-case`. Per `parser-it.json`: 6 voci `page-text`, 4 `table` con celle `{row, col, text}`, almeno 1 `table-pages` con l'elenco delle pagine del documento derivato da 25 pagine.

**TEST-9 — Regola di complessità scritta prima.** La regola simple/complex compare in ORACOLI.md in un commit che precede (o coincide con) il primo commit di `queries.json` con il campo `complexity`. Ogni query ha nel file di derivazione una riga di motivazione della complessità (cerca `oracle_id` + `complex`/`simple` nei `derivations/queries-*.md`).

**TEST-10 — Derivatore distinto.** Il SUMMARY del piano nomina l'agente derivatore; la stessa identità compare in `guardrail.claude` e nei file di derivazione; non coincide con l'esecutore del piano dichiarato nel SUMMARY. È un controllo documentale: l'indipendenza reale la garantisce l'orchestratore di `execute`.

**TEST-11 — Double-check tracciato.** Un `doublecheck_N.md` per ogni lotto, con «GPT (OpenAI)», il `content_id` e la firma di trascrizione di Claude; il SUMMARY elenca gli stessi `content_id` con ACK confermato. Anti-allineamento: per ogni oracolo con `guardrail.gpt ≠ CONFIRMED`, `expected_value` ed `expected_passages` sono identici fra il commit del Task 1 e HEAD (`git show <commit-task1>:… | node -e` contro il file attuale). Un valore cambiato dopo un dissenso senza una nuova derivazione documentata è un fallimento.

**TEST-12 — Contenuto dei lotti.** Il verificatore apre i markdown inviati (la parte di richiesta riportata nei `doublecheck_N.md` o nel log di AgentBridge) e controlla che contengano solo documento, pagina o sezione, estratto e derivazione; `grep -nE '```(rust|bash|json)|system prompt|cargo ' spikes/data/corpus/oracles/doublecheck/*.md` non trova blocchi di codice o prompt nella parte inviata. Tutti i documenti citati sono `public`, `synthetic` o `derived` nel manifest.

**TEST-13 — Numeri come stringhe.** `node -e` che percorre ricorsivamente `expected_value` ed `expected_passages` di ogni voce e fallisce su qualunque `typeof === 'number'`. È l'acceptance del Task 1 che il verify automatico non contiene.

## Copertura

Il rischio CRITICAL (RISK-1) è coperto da tre direzioni: classe ammessa (TEST-1), verifica di verbatim contro il sorgente (TEST-2, TEST-3) e assenza di tracce di pipeline (TEST-4). I HIGH sono tutti coperti. TEST-3 resta manuale per scelta: un controllo automatico sui PDF userebbe un estrattore di testo, cioè la stessa classe di strumenti che gli spike misurano.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-06

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da a29d3d41eec3… a e1b92958ddc3… per la revisione del piano: whitelist chiusa delle classi di provenienza, controllo dei numeri come stringhe e controllo meccanico degli excerpt verbatim sui sorgenti Markdown/testo nel verify del Task 1. Il design non è stato rigenerato: va riletto contro la revisione.
