# Oracoli di misura degli spike

Questa cartella contiene i valori attesi con cui S-02 (parser), S-03 (recall e nDCG per modello e lingua), S-04 (rerank) e A-04 (fedeltà delle citazioni, routing per complessità) confrontano le loro misure. La possiede il piano 01-06; per tutti gli altri piani è in sola lettura. Non è l'evaluation corpus di prodotto in `evaluation/`: queste voci non generano test `eval_oracle_` e valgono solo per la fase 1 (`dataset_version: spike-01`).

Un oracolo entra nelle misure solo con i tre assi del guardrail verdi: `guardrail.claude` compilato, `guardrail.gpt = CONFIRMED`, `guardrail.user = ratified`. Un oracolo con `todo_ref` valorizzato resta fuori, e i RISULTATI dello spike ne riportano il numero.

## File

| File | Contenuto |
|---|---|
| `queries.json` | 80 query etichettate (Q-001…Q-080) con passaggi attesi, ruolo, complessità, rifiuto atteso |
| `parser-it.json` | 11 oracoli del parser italiano: 6 pagine di testo (P-001…P-006), 4 tabelle (P-007…P-010), le pagine con tabelle dell'estratto TTQ (P-011) |
| `derivations/queries-it.md`, `queries-en.md`, `queries-cross.md` | per ogni query: documento, punto esatto, estratto verbatim, svolgimento, motivazione della complessità |
| `derivations/parser-it-text.md`, `parser-it-tables.md` | testo atteso delle pagine, celle delle tabelle, controllo pagina per pagina dell'estratto TTQ |
| `doublecheck/doublecheck_N.md` | richieste inviate a GPT e risposte salvate verbatim (Task 2) |

## Da dove vengono i valori

Ogni valore atteso viene dal documento sorgente e da nient'altro. I Markdown sintetici di `synthetic/` sono stati letti come file. I PDF sono stati letti come immagini: le pagine sono state rasterizzate con `pdftoppm` nel servizio `runner` (110-150 dpi, sola immagine, nessuna estrazione di testo) e lette a vista, una per una. Nessun parser, retrieval, embedding, reranker, JEV o LLM degli spike ha prodotto o controllato un valore; nessun risultato sotto `spikes/s0*/results/` è stato letto, e in wave 3 quei risultati non esistono ancora. Un estrattore di testo come strumento di controllo è escluso per scelta: è la stessa classe di strumenti che S-02 misura, e il suo errore finirebbe dentro l'oracolo.

Per i Markdown il controllo che l'estratto sia verbatim è meccanico (verify del Task 1: sottostringa del file dopo NFC e collasso degli spazi). Per i PDF resta manuale: lo fanno il double-check GPT e la ratifica, e il TEST-3 del test design (campione di 10 query e 2 pagine confrontate a mano con il PDF).

**Agente derivatore.** Ha derivato gli oracoli Claude Opus 5.5 (Anthropic), nella sessione `gsd-executor` del piano 01-06, in un worktree isolato (`exec/01-06`). È un agente diverso da quelli che eseguiranno S-02, S-03, S-04 e A-04 e dall'orchestratore di `execute`, ma è anche l'esecutore di questo piano: il piano chiedeva un agente fresco distinto anche dall'esecutore, e in questa sessione non era possibile avviare un sotto-agente. La deviazione è dichiarata nel rapporto del piano e va giudicata alla ratifica (Task 3).

## Classi di provenienza

Sono ammesse solo le tre classi qualificanti di `skills/eval-oracle-check`:

- `estratto-sorgente`: il valore è scritto letteralmente nel documento, alla pagina o sezione citata (65 query, 10 oracoli del parser).
- `derivazione-documentata`: il valore richiede un passaggio scritto, come un confronto fra righe, un calcolo, un rifiuto per assenza di evidenza o la regola di P-011 (15 query, 1 oracolo del parser). Ha sempre `derivation_ref`.
- `ratifica-utente`: nessuna voce la usa per ora. Servirà se un TO-DO si chiude con una decisione dell'utente.

`hand-crafted` e `generato-dalla-pipeline` non compaiono, e il verify del Task 1 rifiuta qualunque classe fuori da queste tre.

## Convenzioni

**Pagine.** `page` è l'indice fisico del PDF a partire da 1; `printed_page` è il numero stampato sulla pagina, che di solito è diverso (nella Relazione annuale la pagina fisica 11 è la «3»). Per i Markdown, che non hanno pagine, il punto è `section_path`, l'elenco delle intestazioni dalla più esterna alla più interna come compaiono nel file.

**Normalizzazione.** Il confronto avviene dopo: Unicode NFC; spazi multipli e a capo collassati in uno spazio; trattini di sillabazione a fine riga uniti (anche fra due colonne); legature tipografiche scritte come lettere separate; segno meno scritto come `-` (U+002D) nelle tabelle. Apostrofi e virgolette restano come nel documento: tipografici (’ “ ”) nei PDF, ASCII nei Markdown sintetici. I numeri sono sempre stringhe, con i separatori del documento.

**Testo delle pagine (P-001…P-006).** Ordine di lettura dall'alto in basso; nelle parti a due colonne prima la sinistra, poi la destra. Le tabelle si scrivono dopo titolo e sottotitolo, riga per riga (etichetta poi celle, separate da uno spazio), seguite da fonte e note. Le note a piè di pagina vanno in fondo, in ordine, e i richiami in apice diventano cifre attaccate alla parola. Sono esclusi la testatina e il piè di pagina ricorrenti con il numero di pagina, il testo dentro i grafici (assi, legende) e quello dentro le fotografie. S-02 deve applicare le stesse esclusioni all'output del parser prima di calcolare il CER.

**Tabelle (P-007…P-010).** `row` e `col` contano da 1 sul corpo della tabella; la colonna 1 è l'etichetta di riga. Le intestazioni a più livelli sono appiattite in `headers` (livello superiore più anno o periodo). Una cella vuota è la stringa vuota.

**Duplicati del corpus.** L'estratto TTQ `der-it-01` ripete le prime 25 pagine di `pub-it-01`: le query su quelle pagine elencano entrambi i documenti come passaggi attesi. Ogni DOCX derivato ripete il suo Markdown con la stessa classificazione: un passaggio trovato su `<doc>-docx` con lo stesso estratto vale come il passaggio atteso sul Markdown. Le query non elencano i DOCX, così il controllo meccanico degli estratti resta sui Markdown.

**Visibilità.** Il ruolo di ogni query (`role_profile`) è una chiave di `acl.json` (`roles` o `profiles`). I passaggi attesi sono solo su documenti visibili a quel ruolo, calcolati da `acl.json` più la classificazione del manifest. Una query la cui unica risposta sta in un documento invisibile al ruolo ha `no_answer: true`: se il sistema risponde, è un leak.

## Regola di complessità

La regola è stata scritta prima di etichettare le query.

- `simple`: la risposta è un solo fatto, e un solo passaggio basta a sostenerlo (anche se lo stesso fatto è ripetuto altrove nel corpus).
- `complex`: la risposta richiede più fatti o più passaggi, un'aggregazione, un confronto fra periodi, righe o documenti, oppure un calcolo.

Per i rifiuti la regola si applica al fatto chiesto. La motivazione di ogni etichetta, in una riga, sta nel file di derivazione della query. Esito: 68 `simple`, 12 `complex`. La complessità ha classe `derivazione-documentata` (`complexity_ref`).

## Conteggi

| Insieme | Numero |
|---|---|
| Query totali | 80 |
| Italiano / inglese | 32 / 32 |
| Cross-lingual (it→en / en→it) | 16 (8 / 8) |
| Rifiuti attesi (`no_answer: true`) | 12 |
| — senza risposta nel corpus | 8 (Q-032, Q-059, Q-060, Q-061, Q-062, Q-072, Q-079, Q-080) |
| — trappola su `syn-09` con ruolo che non lo vede | 4 (Q-015, Q-016, Q-017, Q-076) |
| Query su documenti con `injection-case` (`syn-07`, `syn-08`) | 7 (Q-019, Q-020, Q-055, Q-056, Q-057, Q-058, Q-070) |
| Query con valore atteso numerico o sigle finanziarie nella domanda (EBITDA, PFN, CET1, capex) | 60 |
| Ruoli | guest 40, employee 17, manager 17, admin 5, restricted-only 1 |
| Oracoli del parser | 11 (6 pagine, 4 tabelle, 1 elenco di pagine) |

Le query senza risposta nel corpus riguardano tutte Alfa Componenti, la società inventata del corpus sintetico. Per i sintetici l'assenza è stata controllata leggendo i nove file; per i PDF pubblici poggia sul fatto che Alfa è inventata dal piano 01-03 (CORPUS.md), e per le trappole sul controllo dei nomi Cormorano, Valfonda e Gamma Immobiliare fatto da 01-03 su tutti i PDF. L'estratto TTQ da 25 pagine ha una sola pagina con tabelle (la 12): la recall dell'euristica di selezione delle pagine si misura su un solo positivo, e le pagine 5-8 (indice e avvertenze) sono i negativi difficili.

## Stato del guardrail

| Asse | Stato |
|---|---|
| Claude (derivazione) | compilato su 91/91 oracoli |
| GPT (double-check) | da eseguire (Task 2) |
| Utente (ratifica) | da eseguire (Task 3) |

## TO-DO di dominio

**TODO-01 — P-006, testo sovrapposto alla fotografia.** A pagina 12 di `pub-it-03` la fotografia ha accanto un riquadro «WEB · L’Economia per tutti · Banca d’Italia per la cultura finanziaria» e una didascalia «Inventiamo una banconota · Centro Convegni Carlo Azeglio Ciampi, Roma». A vista sembrano testo sovrapposto, non pixel della foto, e l'oracolo li include; se nel PDF fossero un'immagine, la regola di esclusione li toglierebbe. Assunzione provvisoria: inclusi. Ne dipende il CER di S-02 su P-006 (circa 120 caratteri su circa 2.400). Domanda per l'utente: tenere l'assunzione, o escludere i due blocchi da P-006? Fino alla risposta P-006 resta pendente e fuori dalle misure.

**Modifica:** Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06 · 2026-10-21 · regole, classi, convenzioni, regola di complessità, conteggi e primo TO-DO (piano 01-06, Task 1)
