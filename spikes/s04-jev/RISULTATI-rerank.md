# S-04/A-06 — Reranker baseline e convivenza in VRAM · RISULTATI

**Modifica:** Claude (gsd-executor 01-15) · 2026-10-25 · Domanda, Soglie e Metodo scritti prima di ogni misura

## Domanda e decisione da chiudere

- **A-06 (D-21):** quale baseline di reranking tenere accanto a JEV `rank`. I candidati sono due: nessun riordino (l'ordine denso di S-03) e `BAAI/bge-reranker-v2-m3` (Apache-2.0) su GPU nel container TEI. Si misurano entrambi contro JEV forma A, sugli stessi top-20 autorizzati di S-03. JEV resta il rank di prodotto (ADR-0002, LOCKED). La baseline serve da confronto nell'evaluation e da ripiego quando JEV non risponde.
- **Budget VRAM (Claude's Discretion; D-13, D-21):** la RTX 3050 ha 6 GB. Bisogna capire se TEI embedding (bge-m3, D-38), TEI reranker e Docling GPU convivono sotto carico concorrente, e con quali `--max-batch-tokens`.
- **Regola no-leak (D-16):** nessun candidato non visibile al `role_profile` della query raggiunge il reranker TEI o JEV.

## Soglie (scritte PRIMA della misura)

| # | Metrica | Soglia | Fonte |
|---|---|---|---|
| R-1 | Baseline di reranking | La baseline è **bge-reranker-v2-m3** se il suo nDCG@5 medio su GPU è ≥ nDCG@5 medio di no-rerank + 0,02 **e** la latenza p95 del rerank di 20 candidati è ≤ 300 ms. Altrimenti la baseline è **no-rerank**. | RICERCA Area D §4 (stima GPU 100-300 ms) e §7 riga S8; D-21 |
| R-2 | JEV rank (forma A, concorrenza 16, deadline 2 s) | Misurato e riportato come differenza di nDCG@5, recall@8 e p95 rispetto alla baseline scelta. Nessuna soglia esclude JEV: resta il rank di prodotto. | ADR-0002; RISULTATI-jev §Esito (ADR-0008) |
| R-3 | VRAM | Con tei-embed, tei-rerank e docling caricati insieme e sotto carico concorrente: VRAM totale di picco ≤ 5,5 GiB (5.632 MiB) e nessun OOM. Se la soglia è superata, prima di dichiarare l'incompatibilità si riducono i max-batch-tokens (`TEI_MAX_BATCH_TOKENS`, `TEI_RERANK_MAX_BATCH_TOKENS`) e si rimisura. Si riporta la configurazione che rientra. | RESEARCH §10; Claude's Discretion |
| R-4 | Candidati non visibili inviati a TEI o JEV | `unauthorized_sent` = 0 su ogni riga di `rerank.csv`, per tutti i metodi (hard gate). | D-16; threat model T-14 |

Definizioni che valgono per le soglie:

- **Candidati:** i top-20 autorizzati per query di `export-candidates --model bgem3 --k 20` (piano 01-10), verificati contro lo sha256 di `candidates_manifest.json`. Una query con meno di 20 candidati autorizzati riceve tutti i suoi candidati: non si riempie mai con candidati non visibili. Il numero di queste query si riporta nei Risultati.
- **Rilevanza:** binaria, dagli oracoli ratificati (tre assi verdi). È la regola di S-03 (`s03_pgvector::metrics`, piano 01-10): stesso documento (anche la copia `-docx`) e stessa pagina o sezione, e il chunk contiene l'excerpt normalizzato o almeno il 50% dei suoi caratteri come sottostringa contigua. Il bin la riapplica sui 20 candidati, perché i crate degli spike non si importano fra loro.
- **Metriche:** per ogni query, nDCG@5 e recall@8 con `s04_jev::metrics` (piano 01-11). L'insieme dei rilevanti è quello dei rilevanti fra i 20 candidati: il riordino non può recuperare un chunk che il retrieval non ha portato. Una query senza rilevanti fra i candidati vale 0 per tutti i metodi e resta nella media. Si contano a parte.
- **Medie e percentili:** 3 ripetizioni per query e metodo; la prima si scarta. Media aritmetica sulle righe delle ripetizioni 2 e 3. La latenza è il tempo di parete del riordino di una query (guardia di visibilità, invio, lettura della risposta). Il p95 è a rango più vicino sulle stesse righe.
- **VRAM totale:** `memory.used` dell'intera GPU letto da `nvidia-smi` sull'host, con la quota del desktop di Windows compresa. La baseline del desktop si misura prima di avviare i servizi e si riporta a parte.

## Metodo (scritto prima della misura)

- **Metodi:**
  - `none`: l'ordine denso di S-03, nessun invio;
  - `tei`: un `POST {TEI_RERANK_URL}/rerank` per query, con `{"query", "texts", "raw_scores": false}`. Ogni `index` della risposta si ricollega al candidato inviato in quella posizione. Una risposta incompleta, con un indice ripetuto o fuori intervallo è un errore, mai un ordine inventato;
  - `jev`: `rank_form_a` della libreria di 01-11, concorrenza 16, deadline 2 s, pin `jev-1.13.0`, ordine per `relevant` decrescente.

  Nei tre metodi i pareggi conservano l'ordine denso.
- **Guardia di visibilità:** `assert_visible` sull'intero insieme e sul `role_profile` della query, prima di qualunque invio e per tutti i metodi, quindi anche per `none`. Il contatore `unauthorized_sent` riconta in modo indipendente i passaggi non visibili al punto di invio: per TEI nel bin, per JEV nel client di 01-11.
- **Chiamate a JEV:** 3 ripetizioni × numero di query × 20 richieste di forma A, una sola passata. Il tetto di spesa è `S04_MAX_COST_USD`, letto dall'ambiente dal client di 01-11.
- **Convivenza in VRAM** (`vram.sh`, una configurazione di batch per invocazione):
  - campionamento di `nvidia-smi --query-gpu=memory.used` ogni 500 ms lungo tutto lo script;
  - fase `desktop`: nessun servizio avviato;
  - fasi `warm-<servizio>`: un servizio alla volta (tei-embed, poi tei-rerank, poi docling), avviato per nome con `session.sh up` e caricato con il proprio carico per 30 s;
  - fase `load`: i tre carichi insieme per 180 s (embedding con `throughput` di s03-pgvector, rerank con il bin `rerank --methods tei`, conversione Docling di una pagina-tabella con `docling-client`);
  - fase `after`: qualche secondo a riposo.
- **VRAM per servizio:** su Docker Desktop/WSL2 `nvidia-smi --query-compute-apps` non vede i processi dei container. La quota di un servizio si attribuisce quindi per avvio successivo: è il picco della sua fase `warm-<servizio>` meno il minimo della stessa fase. Nelle fasi successive la quota di ogni servizio già avviato vale costante. Il totale delle righe resta la lettura vera della GPU. La differenza fra il totale sotto carico e la somma di desktop e quote si riporta come residuo di contesa.
- **OOM:** vale `true` su ogni riga di una configurazione in cui i log dei tre servizi o dei tre carichi riportano un errore di memoria (`out of memory`, `OutOfMemory`, `CUDA_ERROR_OUT_OF_MEMORY`), oppure in cui un servizio non è più in esecuzione alla fine.

## Setup

- **Pesi del reranker:** `BAAI/bge-reranker-v2-m3`, revisione Hugging Face `953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e`, risolta il 2026-10-25 alle 17:57 UTC con `curl -fsSL --proto '=https' https://huggingface.co/api/models/BAAI/bge-reranker-v2-m3/revision/main` (campo `sha`). `tei-rerank` parte con `RERANK_REVISION` uguale a quella revisione. Il suo `/info` riporta `model_sha` = `953dc6f6f85a1b2dbfca4c34a2796e7dde08d41e`, `version` 1.9.4, `max_input_length` 8192, `max_client_batch_size` 32, `auto_truncate` true. I pesi sono stati scaricati in `hf-cache` fuori dalle misure.
- **Embedder (D-38, piano 01-10):** `BAAI/bge-m3`, revisione `5617a9f61b028005a4858fdac845db406aefb181` (`/info`: `model_sha` uguale), su TEI `86-1.9.4`.
- **Forma di `/rerank` ([NV] RESEARCH §11.10, chiuso):** la risposta reale di TEI 1.9.4 è `[{"index", "score"}]`, ordinata per punteggio decrescente. Con `raw_scores: false` il punteggio sta fra 0 e 1, con `true` è il logit grezzo. A parità di punteggio TEI non conserva l'ordine degli indici: il bin ordina i pareggi per indice, cioè per ordine denso.
- **Candidati:** rigenerati con `/target/release/s03-pgvector --out /work/tmp/s04-export export-candidates --model bgem3 --k 20` (`--out` in `/work/tmp`, così il manifest versionato di 01-10 non si riscrive). Il file ha sha256 `8f507d073991677fd081dc7e0df118ec8bb9e866f199339e0ceaae4a1296af92`, uguale al manifest: nessun `--accept-sha`. Contiene 74 query e 1.479 candidati, 0 non visibili. Una sola query ha meno di 20 candidati autorizzati (19); 15 query non hanno rilevanti fra i 20 candidati.
- **Controllo della regola di rilevanza:**
  - un'implementazione indipendente in Python, con il passo NFC, dà su tutte le 74 query lo stesso nDCG@5 e lo stesso recall@8 del bin per il metodo `none`;
  - su `query-bench` di S-03 rieseguito con uscita in `/work/tmp`, i flag di rilevanza per query danno lo stesso recall@5, recall@10 e primo rango;
  - testi delle query, excerpt e corpi dei chunk sono già in NFC (0 eccezioni su 1.479 chunk e su tutti gli excerpt), quindi il passo NFC mancante nel bin non cambia nulla.
- **VRAM del desktop:** 1.909-2.131 MiB occupati prima di avviare i servizi (lettura di `nvidia-smi` all'apertura delle sessioni del 2026-10-25). Sulla RTX 3050 il margine per i servizi è quindi di circa 4 GiB. `nvidia-smi --query-compute-apps` dentro e fuori i container non elenca i processi dei container (WSL2): «No running processes found».
