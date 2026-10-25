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
- **Hardware e sessioni:** [`results/rerank/hw.json`](results/rerank/hw.json) (sha256 `7794554e4c9a688690d6402c2f8e7cbebcc2a66542efbef21ff4ad8f56f18d0a`, sessione `01-15-20261025t182756z-d090`, `git_rev` `d530189`, `tools.reranker` = `BAAI/bge-reranker-v2-m3`, `tools.jev_model` = `jev-1.13.0`). La macchina è l'i7-13700K con RTX 3050 6 GB (driver 610.88), Docker Desktop 29.8.2 su WSL 2, piano energetico «Bilanciato». Il manifest è normalizzato allo schema di `common::manifest` con la stessa trasformazione dei piani precedenti; l'originale sta in `results/raw/rerank/hw-raw.json`, ignorato da git (vedi «Anomalie e deviazioni»).
- **Immagini:** TEI `86-1.9.4` (`sha256:bee0bdb8…`), TEI `cpu-1.9` (`sha256:2538ea1c…`), `docling-serve-cu128:v1.37.0` (`sha256:2aa6fc68…`), runner `cka-spikes-runner:latest` (`sha256:86a924ad…`); i digest completi sono in `hw.json`. Binari `--release --locked --offline` compilati nel runner all'inizio della sessione dal commit `d530189` («Compiling s04-jev» visto nel log).
- **Comandi della misura di rerank** (da `spikes/`, nel runner via `session.sh oneshot`, sessione `01-15-20261025t182756z-d090`, servizi `tei-rerank` e `runner`, `ps --assert-only` verde, nessun container di altri piani):
  - `/target/release/rerank --info` (pin dei pesi, vedi sopra);
  - `/target/release/rerank --methods none,tei,jev --runs 3 --device gpu --out s04-jev/results/rerank/rerank.csv`;
  - ripiego CPU: `tei-rerank` fermato con `session.sh stop`; avviato un container `text-embeddings-inference:cpu-1.9` con etichetta `cka.run_id` della sessione, sulla rete `cka-spikes_default` con alias `tei-rerank` (così `TEI_RERANK_URL` del runner non cambia), stessi pesi e stessa revisione dal volume `hf-cache`; poi `/target/release/rerank --methods tei --runs 3 --device cpu --out s04-jev/results/rerank/rerank.csv` (accodato) e `session.sh stop` del container.
- **Chiamate a JEV:** 1.479 richieste di forma A per passata × 3 passate = 4.437 richieste (stima prima di partire: circa 3,3 milioni di token di input, 0,14 USD). Spesa reale da `usage`: 0,130760 USD, contro un tetto `S04_MAX_COST_USD` di 5 USD. Nessuna altra chiamata a JEV in questo piano: i carichi di `vram.sh` usano solo il metodo `tei`.

## Risultati

### Rerank (`results/rerank/rerank.csv`, ripetizioni 2 e 3, 74 query)

| Metodo | device | righe | nDCG@5 | recall@8 | latenza p50 / p95 / p99 (ms) | Esito vs soglia |
|---|---|---|---|---|---|---|
| no-rerank (ordine denso di S-03) | na | 148 | 0,5777 | 0,7432 | 0 / 0 / 0 | riferimento di R-1 |
| bge-reranker-v2-m3, TEI GPU | gpu | 148 | 0,7338 | 0,7838 | 287 / **360** / 432 | R-1: nDCG@5 +0,1561 (≥ +0,02, sì); p95 360 ms (≤ 300 ms, **no**) |
| bge-reranker-v2-m3, TEI CPU (ripiego, facoltativo) | cpu | 147 | 0,7319 | 0,7823 | 20.295 / 32.033 / 51.795 | fuori da ogni budget di latenza |
| JEV forma A, concorrenza 16, deadline 2 s | api | 148 | 0,7649 | 0,7973 | 368 / 483 / 603 | R-2: riportato come differenza |

- **Regola R-1 applicata:** il reranker su GPU guadagna 0,1561 di nDCG@5 sul no-rerank, oltre il margine richiesto di 0,02. Ma il p95 del rerank di 20 candidati è 360 ms contro un tetto di 300 ms (massimo 435 ms). Le due condizioni vanno soddisfatte insieme, quindi **la baseline è no-rerank**.
- **Sulle 59 query con almeno un rilevante fra i candidati** (dato informativo, le medie della soglia restano quelle della tabella): nDCG@5 / recall@8 = 0,7246 / 0,9322 per no-rerank, 0,9203 / 0,9831 per TEI GPU, 0,9594 / 1,0000 per JEV. Le altre 15 query valgono 0 per tutti i metodi: 12 sono `no_answer`, 3 (Q-029, Q-047, Q-078) hanno rilevanti attesi che il retrieval denso non porta fra i primi 20.
- **Per query (ripetizione 2):** TEI GPU migliora nDCG@5 rispetto a no-rerank su 26 query, lo peggiora su 3 e lo lascia uguale su 45. JEV migliora TEI su 8 query, lo peggiora su 1 e lo pareggia su 65; è sotto no-rerank su 1 query.
- **Per gruppo di lingua** (59 query con rilevanti; it 24, en 24, cross 11): no-rerank 0,6837 / 0,6992 / 0,8692; TEI GPU 0,8739 / 0,9455 / 0,9664; JEV 0,9018 / 0,9983 / 1,0000. Il guadagno maggiore del reranker sta sulle query in inglese.
- **Stabilità:** no-rerank e TEI danno lo stesso nDCG@5 nelle ripetizioni 2 e 3 su tutte le query (TEI è deterministico). JEV cambia su 3 query su 74: Q-027 (1,0000 / 0,5000), Q-031 (0,8772 / 0,6509) e Q-049 (1,0000 / 0,9197). È la varianza dei Noul già misurata in RISULTATI-jev (vincolo di calibrazione di ±0,1).
- **JEV:** `model_resolved` = `jev-1.13.0` su tutte le 222 righe, nessun errore, nessun timeout entro i 2 s di deadline (latenza massima 630 ms).
- **Prima ripetizione (scartata):** la prima query paga il riscaldamento (TEI GPU 621 ms, JEV 425 ms); il resto della ripetizione 1 sta negli stessi intervalli delle ripetizioni 2 e 3.
- **Candidati per query:** 20 su 73 query; Q-075 ne ha 19, cioè tutti i candidati visibili al suo ruolo (nessun riempimento con candidati non visibili). Il numero è lo stesso in tutti i metodi e in tutte le ripetizioni.
- **Hard gate R-4:** `unauthorized_sent` = 0 su tutte le 887 righe, per tutti i metodi (`awk -F, 'NR>1 && $9!=0' results/rerank/rerank.csv` non stampa nulla). Il client JEV conta 0 passaggi non visibili inviati in tutta la sessione.
