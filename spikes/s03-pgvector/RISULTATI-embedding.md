# S-03 / A-03 — Modello di embedding e serving TEI su GPU · RISULTATI

**Modifica:** Claude (gsd-executor 01-10) · 2026-10-22 · domanda, soglie, regola di rilevanza e metodo scritti prima di ogni misura (Task 1 del piano 01-10)

## Domanda e decisione da chiudere

Spike S-03, parte embedding (assunzione A-03; decisioni D-13 e D-14 di `01-CONTEXT.md`). Domande:

1. Quale dei tre modelli candidati (Qwen3-Embedding-0.6B, default; BAAI/bge-m3; intfloat/multilingual-e5-large-instruct) recupera meglio i passaggi attesi delle query etichettate ratificate, in italiano, in inglese e cross-lingual?
2. Il serving con TEI CUDA sulla RTX 3050 da 6 GB regge throughput, latenza della query e budget di VRAM?
3. Quanto pesa l'istruzione asimmetrica sulla query? La risposta fissa la forma della porta `Embedder` (embedding dei documenti e della query separati, S-07).

La ricerca usata per tutte le metriche è esatta (nessun indice ANN) con il predicato ACL nella stessa query SQL del ranking (D-12, D-16). Il modello e il serving si decidono in ADR-0007; l'indice e la soglia HNSW sono del piano 01-14.

## Soglie (scritte PRIMA della misura)

Queste soglie, la regola di rilevanza e il metodo qui sotto sono fissati in questo commit, prima di qualunque file in `results/embedding/`, e non si ritoccano sui risultati: una correzione va in «Anomalie e deviazioni», con motivo.

| Id | Metrica | Soglia | Fonte |
|----|---------|--------|-------|
| M-1 | Regola di scelta: nDCG@10 complessivo (riga `all` di `models.csv`) | vince il più alto; una differenza < 0,02 dal primo è un pari, e fra i pari decidono nell'ordine licenza, latenza della query p95 e VRAM | RICERCA pgvector §5, D-14 |
| M-2 | Per il modello scelto: calo dell'italiano rispetto all'inglese, nDCG@10(`en`) − nDCG@10(`it`) | ≤ 0,10 | RICERCA pgvector §5, D-14 |
| M-3 | Per il modello scelto: calo del cross-lingual rispetto alla media, nDCG@10(`all`) − nDCG@10(`cross`) | ≤ 0,10 | RICERCA pgvector §5, D-14 |
| M-4 | Latenza dell'embedding di una query (con istruzione), GPU, p95 | ≤ 50 ms | Claude's Discretion, quota del TTQ |
| M-5 | Embedding di tutti i chunk del documento da 25 pagine (`der-it-01`), GPU, p95 sulle ripetizioni utili | ≤ 4 s | README TTQ 15 s, accanto ai ≤ 5 s del parse |
| M-6 | VRAM di picco di `tei-embed` (picco di `memory.used` meno il valore a riposo prima dell'avvio di TEI) | ≤ 2,5 GiB (2.560 MiB) | budget VRAM: lascia spazio a reranker e Docling nei 6 GB |
| M-7 | Leak: chunk nei top-k (k = 5 e 10) di documenti non visibili al `role_profile` della query, contati contro la matrice attesa da `acl.json` + manifest | = 0 su ogni riga di `models.csv` (hard gate) | D-16, T-13 |
| M-8 | `doc_acl` letta dal DB coincide con la matrice attesa prima di ogni `query-bench` ed `export-candidates` | nessuna differenza, altrimenti il comando si ferma | D-12, T-01-10-01 |
| M-9 | Effetto dell'istruzione di query (Qwen3 ed e5): nDCG@10 e recall@10 con e senza | riportato, nessuna soglia | S-07 |
| M-10 | Throughput (chunk/s) per batch 16/32/64 e `--max-batch-tokens` 4096/8192/16384; dato CPU di ripiego per il modello scelto | riportato, nessuna soglia | D-13 |

**Configurazione su cui si giudicano M-4, M-5 e M-6.** Per il modello scelto, la configurazione di serving proposta è quella con il throughput più alto (mediana dei chunk/s delle ripetizioni utili) fra quelle con VRAM entro M-6; se nessuna rientra in M-6, quella con la VRAM più bassa, e M-6 fallisce. Le tre soglie si giudicano su quella configurazione e si riportano anche per gli altri due modelli.

### Regola di rilevanza (scritta PRIMA della misura)

Un chunk è rilevante per una query se, per almeno uno dei suoi passaggi attesi (`expected_passages` di `queries.json`):

1. appartiene al documento del passaggio (un passaggio su `<doc>` vale anche su `<doc>-docx`, ORACOLI.md «Duplicati del corpus»; l'estratto `der-it-01` è già elencato esplicitamente dagli oracoli che lo riguardano), **e**
2. sta nella stessa pagina fisica (PDF) o nella stessa sezione (Markdown, `section_path` uguale), **e**
3. dopo la normalizzazione di entrambi i lati contiene l'excerpt atteso, **oppure** contiene almeno il 50% dei caratteri dell'excerpt come sottostringa contigua (caso dell'excerpt a cavallo di un confine di chunk).

Normalizzazione (ORACOLI.md, «Normalizzazione»): Unicode NFC; legature U+FB00…U+FB06 in lettere separate; segno meno U+2212 → `-`; trattino morbido rimosso; trattino di sillabazione a fine riga unito (lettera, `-`, a capo, minuscola); ogni sequenza di spazi bianchi, a capo compresi, collassata in uno spazio. Apostrofi e virgolette non si toccano.

### Metodo (scritto PRIMA della misura)

- **Oracoli.** Solo le query di `spikes/data/corpus/oracles/queries.json` con i tre assi verdi (`claude` compilato, `gpt` `CONFIRMED`, `user` `ratified`) e `todo_ref` vuoto; gli esclusi si contano in «Anomalie».
- **Query valutabili.** Una query entra nelle metriche di qualità se non è `no_answer` e se almeno un chunk del corpus è rilevante per lei secondo la regola sopra. Una query non `no_answer` senza chunk rilevanti nel corpus è «irraggiungibile» (il passaggio non è sopravvissuto all'estrazione o al chunking): si esclude dalle medie e si elenca in «Anomalie». Le query `no_answer` entrano solo nel conteggio del leak.
- **Metriche.** Per ogni query: recall@k = chunk rilevanti nei primi k / chunk rilevanti nel corpus; MRR@10 = reciproco del rango del primo rilevante nei primi 10; nDCG@10 con guadagno binario e DCG ideale con min(rilevanti, 10) rilevanti in testa. Valori per lingua (`it`, `en`, `cross` = le due direzioni cross-lingual insieme, `all`) come media semplice sulle query valutabili. Lo stesso k = 10 della ricerca per tutte le metriche; recall@5 dai primi 5 della stessa lista.
- **Ruolo.** Ogni query cerca con il solo `role_profile` dell'oracolo come ruolo (`role = ANY($2)` con un elemento); il profilo `restricted-only` ha righe proprie in `doc_acl`.
- **Istruzione.** Con istruzione è la configurazione di riferimento di `models.csv`; senza istruzione si misura per Qwen3 ed e5 in `query_instruction.csv` (bge-m3 non ne ha).
- **Ripetizioni.** Qualità: una esecuzione (la ricerca esatta è deterministica). Throughput: 3 ripetizioni per configurazione, la prima si scarta; latenza della query = p50 e p95 da istogramma su 200 query per ripetizione, e per M-4 vale il peggiore dei p95 delle ripetizioni utili; M-5 è il p95 nearest-rank dei secondi delle ripetizioni utili (con due ripetizioni, il massimo).
- **Pesi.** Revisione Hugging Face di ogni modello fissata con `EMBED_REVISION=<sha>` prima delle misure e verificata contro `model_sha` di `/info`.

## Setup

- Chunker di misura (`src/chunk.rs`): finestre di al più 1.600 caratteri con sovrapposizione di 200, mai a cavallo di due pagine o due sezioni, offset in caratteri. PDF: `pdftotext -layout -enc UTF-8` nel runner, una pagina per form feed, offset relativi alla pagina. Markdown: una sezione per intestazione ATX, front matter YAML escluso, offset relativi al documento; il `section_path` non comprende l'H1 (titolo del documento), come i dati degli oracoli. DOCX non indicizzati: ripetono il loro Markdown (ORACOLI.md) e la loro estrazione è materia di S-02. Id dei chunk stabili (UUID v8 da FNV-1a di documento, pagina o sezione e offset).
- Istruzione di query (`src/tei.rs`): `Instruct: Given a web search query, retrieve relevant passages that answer the query\nQuery: <testo>` per Qwen3 ed e5-instruct, nessun prefisso per bge-m3; documenti sempre senza istruzione; `normalize: true`, `truncate: true`.
- Ricerca (`src/db.rs`): `ORDER BY c.emb <=> $1, c.id LIMIT $3` con `EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2))` nella stessa query; inserimento a lotti di 500 righe con `INSERT … SELECT … FROM UNNEST(…, $9::vector[])`.

- Hardware: [`results/embedding/hw.json`](results/embedding/hw.json) (sha256 `066e65a8559f22ccf50b7550e801b58801eb4250363ffd2f60e13c61a80534e7`, sessione `01-10-20261022t195602z-b9ee`, `git_rev` bc3e0ab): i7-13700K 16 core / 24 thread, 32 GB, RTX 3050 6 GB (driver 610.88, CUDA 13.3), Docker 29.8.2 su WSL 2 (16 CPU alla VM), rustc 1.95.0, piano energetico «Bilanciato». Il file è normalizzato allo schema di `common::manifest` (vedi «Anomalie»); l'originale sta in `results/raw/embedding-hw-raw.json`. La sessione dei candidati e del dato CPU (`01-10-20261022t202643z-a77f`) ha il suo manifest in `results/raw/hw-session-b.json`.
- Immagini: `pgvector/pgvector:0.8.7-pg18-trixie` (`sha256:9d9c9302…cad2e0`); TEI CUDA `ghcr.io/huggingface/text-embeddings-inference:86-1.9.4` (`sha256:bee0bdb8…fc613`), versione 1.9.4 da `/info`, `float16`, `--max-client-batch-size 64`; TEI CPU `text-embeddings-inference:cpu-1.9` (`sha256:2538ea1c…80c78d`). `pdftotext` 25.03.0 (poppler) nel runner.
- Revisioni dei pesi (sha del commit Hugging Face risolto prima delle misure, passato come `EMBED_REVISION` e uguale al `model_sha` restituito da `/info` a ogni avvio; copie di `/info` in `results/raw/info/`):

| Modello | Revisione (`model_sha` di `/info`) | Licenza |
|---|---|---|
| `Qwen/Qwen3-Embedding-0.6B` | `97b0c614be4d77ee51c0cef4e5f07c00f9eb65b3` | Apache-2.0 |
| `BAAI/bge-m3` | `5617a9f61b028005a4858fdac845db406aefb181` | MIT |
| `intfloat/multilingual-e5-large-instruct` | `274baa43b0e13e37fafa6428dbc7938e62e5c439` | MIT |

- Corpus: `spikes/data/corpus/manifest.json`, 19 documenti indicizzati (10 PDF, 9 Markdown; 4 DOCX esclusi), 3.950 chunk, impronta dell'insieme dei chunk `f7fb97ce…86e1f7a`, uguale per i tre modelli (`results/embedding/chunks_manifest.json`, con lo sha256 di ogni grezzo verificato contro il manifest). `doc_acl` popolata da `acl.json` + manifest per i 4 ruoli e il profilo `restricted-only`.
- Comandi (da `spikes/`, dentro `session.sh with 01-10`, nel runner dopo `cargo build --release --locked -p s03-pgvector`):
  - `s03-pgvector load --model <qwen3|bgem3|e5> --batch 32`
  - `s03-pgvector query-bench --model <m> [--no-instruction]`
  - `s03-pgvector throughput --model <m> --batch <16|32|64> --max-batch-tokens <4096|8192|16384> --run <1|2|3> --vram-log <log di nvidia-smi> --vram-baseline-mib <riposo>`, con `tei-embed` riavviato (`session.sh down` + `up`) per ogni modello e ogni `--max-batch-tokens`
  - `s03-pgvector export-candidates --model bgem3 --k 20` (lanciato due volte: stesso sha256)
  - `s03-pgvector relevance-check` (diagnostica, senza servizi)

## Risultati

### Qualità (`results/embedding/models.csv`, con istruzione dove il modello la prevede)

60 query valutabili su 74 ratificate: 24 italiane, 24 inglesi, 12 cross-lingual. Le 12 `no_answer` contano solo per il leak; 2 sono irraggiungibili (vedi «Anomalie»).

| Modello | nDCG@10 it | nDCG@10 en | nDCG@10 cross | **nDCG@10 all** | recall@5 all | recall@10 all | MRR@10 all | leak (k = 5 e 10) |
|---|---|---|---|---|---|---|---|---|
| Qwen3-Embedding-0.6B | 0,6306 | 0,6796 | 0,5837 | 0,6408 | 0,7208 | 0,8250 | 0,5876 | 0 |
| **bge-m3** | 0,6884 | 0,7198 | 0,7968 | **0,7226** | 0,8500 | 0,9000 | 0,6732 | 0 |
| multilingual-e5-large-instruct | 0,7157 | 0,6202 | 0,4181 | 0,6180 | 0,7167 | 0,8167 | 0,5704 | 0 |

### Istruzione di query (`results/embedding/query_instruction.csv`)

| Modello | nDCG@10 all con / senza | recall@10 all con / senza | nDCG@10 cross con / senza |
|---|---|---|---|
| Qwen3 | 0,6408 / 0,6595 | 0,8250 / 0,8250 | 0,5837 / 0,6072 |
| e5-instruct | 0,6180 / 0,4752 | 0,8167 / 0,6500 | 0,4181 / 0,3333 |
| bge-m3 | 0,7226 (nessuna istruzione prevista) | 0,9000 | 0,7968 |

### Serving (`results/embedding/throughput.csv`, ripetizioni 2 e 3)

Configurazione proposta per modello (regola delle Soglie: chunk/s mediano più alto con VRAM entro M-6). Il documento da 25 pagine produce 69 chunk.

| Modello | batch / max-batch-tokens | chunk/s | 25 pagine p95 (s) | query p50 / p95 (ms) | VRAM picco (MiB) |
|---|---|---|---|---|---|
| Qwen3 | 32 / 8192 | 30,93 | 2,248 | 16,4 / 17,10 | 1.662 |
| **bge-m3** | **64 / 16384** | **69,28** | **1,017** | **10,7 / 12,00** | **1.453** |
| e5-instruct | 64 / 8192 | 71,31 | 0,992 | 10,9 / 11,89 | 1.389 |
| bge-m3, CPU (`cpu-1.9`) | 64 / 16384 | 3,03 | 24,02 | 41,0 / 50,21 | — |

Su tutta la griglia GPU la VRAM di picco sta fra 1.192 e 2.115 MiB (massimo: Qwen3 batch 64 con 16384 token), la query p95 fra 11,1 e 27,6 ms (massimo: bge-m3 batch 16 con 4096 token) e il documento da 25 pagine fra 0,94 e 2,46 s. Per bge-m3 i valori più bassi di `--max-batch-tokens` peggiorano sia il throughput (50,8-65,5 chunk/s con 4096) sia la latenza della query (p95 fino a 27,6 ms).

### Esito vs soglia (modello scelto: bge-m3, configurazione 64 / 16384)

| Id | Valore misurato | Soglia | Esito |
|---|---|---|---|
| M-1 | nDCG@10 all 0,7226; Qwen3 0,6408 (0,6595 senza istruzione), e5 0,6180: distacco 0,063 ≥ 0,02 | il più alto, pari sotto 0,02 | **pass** (nessun pari) |
| M-2 | en − it = 0,7198 − 0,6884 = 0,031 | ≤ 0,10 | **pass** |
| M-3 | all − cross = 0,7226 − 0,7968 = −0,074 (il cross-lingual va meglio della media) | ≤ 0,10 | **pass** |
| M-4 | query p95 12,00 ms (peggiore delle ripetizioni utili) | ≤ 50 ms | **pass** |
| M-5 | 25 pagine p95 1,017 s | ≤ 4 s | **pass** |
| M-6 | VRAM di picco 1.453 MiB (1,42 GiB) | ≤ 2.560 MiB | **pass** |
| M-7 | 0 chunk non visibili su tutte le 24 righe di `models.csv` (74 query, `no_answer` comprese) e sui 1.479 candidati esportati | = 0 | **pass** |
| M-8 | `doc_acl` uguale alla matrice attesa prima di ogni `query-bench` ed `export-candidates` (il comando altrimenti si ferma) | nessuna differenza | **pass** |
| M-9 | e5: +0,143 di nDCG@10 con istruzione; Qwen3: −0,019 | riportato | — |
| M-10 | GPU 69,28 chunk/s contro CPU 3,03 (×23); 25 pagine in 1,0 s su GPU e 24 s su CPU | riportato | — |

Per gli altri due modelli, sulla loro configurazione proposta: M-4, M-5, M-6 e M-7 passano; M-2 passa (Qwen3 0,049; e5 −0,096, l'italiano va meglio dell'inglese); M-3 passa per Qwen3 (0,057) e fallirebbe per e5 (0,6180 − 0,4181 = 0,200).

## Anomalie e deviazioni

- **Oracoli esclusi.** 6 query su 80 non hanno i tre assi verdi (Q-008, Q-010, Q-012, Q-040, Q-048, Q-056: `gpt` `PARTIALLY_CONFIRMED`, `todo_ref` valorizzato) e restano fuori, come previsto da ORACOLI.md.
- **Query irraggiungibili: 2.** Q-029 (`pub-it-03`, pagina 12) e Q-047 (`pub-en-03`, pagina 37, sei passaggi) non hanno alcun chunk rilevante: nel testo di `pdftotext -layout` gli excerpt non compaiono come sottostringa contigua per almeno il 50% (impaginazione a riquadri e colonne affiancate). Sono escluse dalle medie per tutti e tre i modelli allo stesso modo, come stabilito nel Metodo. È un limite del chunker di misura, non dei modelli; il parser di prodotto è materia di S-02.
- **`section_path` degli oracoli Markdown senza H1.** ORACOLI.md (riga 39) dice «le intestazioni … come compaiono nel file», ma i dati di `queries.json` omettono l'H1 (titolo del documento): per esempio `syn-06` ha `# Alfa Componenti S.p.A. — Domande frequenti` e l'oracolo dice `2. Dati principali > 2.1 …`. Ho allineato il chunker ai dati: il `section_path` dei chunk non comprende l'H1 (il testo sotto il solo H1 ha come percorso il titolo). Oracoli e regola di rilevanza non sono stati toccati; l'incoerenza fra ORACOLI.md e `queries.json` resta da segnalare al proprietario (01-06).
- **`hw.json` normalizzato.** `tools/hw-manifest.sh` (01-02) scrive `cpu`, `ram_total`, `docker_desktop_resources` e `images` in forme che `common::manifest` (01-01) rifiuta («invalid type: map, expected a string»): la prima sessione si è fermata al primo CSV. È il difetto già registrato da 01-04, 01-05, 01-07, 01-08, 01-11 e 01-12; ho applicato la stessa trasformazione con un driver fuori dal repo, originale in `results/raw/embedding-hw-raw.json`. Nessun file chiuso toccato.
- **Parametri di `tei-embed` per modello.** Il comando del piano (`EMBED_MODEL=<model> EMBED_REVISION=<sha> bash spikes/tools/session.sh up …`) non ha effetto dalla gap-closure 1.1: `session.sh` toglie a compose le variabili dell'host omonime di una chiave del file segreti, e `EMBED_MODEL`, `EMBED_REVISION` e `TEI_MAX_BATCH_TOKENS` sono chiavi del file. Ho puntato `CKA_SPIKES_ENV` (variabile prevista da 01-02) a una copia del file segreti fuori dal repo, `$HOME/.cka-spikes/spikes-01-10.env`, con le sole righe dei parametri TEI sostituite, rigenerata a ogni avvio e cancellata a fine lavoro. Il controllo `model_sha` = revisione fissata, a ogni avvio, conferma che il modello avviato era quello voluto.
- **TEI CPU.** Il container `cka-tei-cpu` (etichetta `cka.run_id` della sessione) è stato avviato con l'alias di rete `tei-embed`, dopo aver fermato il `tei-embed` GPU, perché il runner raggiunge TEI solo tramite `TEI_EMBED_URL` del file segreti; porta 127.0.0.1:8083 solo per il controllo di prontezza dall'host; fermato con `session.sh stop`. Righe CPU senza VRAM.
- **Binario invece di `cargo run`.** Nel runner il crate è stato compilato una volta per sessione (`cargo build --release --locked`) e il binario `/target/release/s03-pgvector` lanciato per ogni sottocomando: stesso codice, senza il controllo di Cargo a ogni invocazione.
- **VRAM a riposo variabile.** Il valore a riposo prima di ogni avvio di TEI è oscillato fra 2.106 e 2.356 MiB per le applicazioni del desktop che usano la GPU. Il picco si riporta come differenza dal riposo misurato subito prima; l'incertezza (circa ±250 MiB) non cambia nessun esito, perché il massimo misurato (2.115 MiB) resta sotto M-6 anche sommandola.
- **Candidati: 1.479 invece di 1.480.** La query Q-075, con profilo `restricted-only`, ha solo 19 chunk visibili (syn-03 + syn-09), quindi 19 candidati.
- **`pdftotext` dell'host.** Una prova preliminare con `pdftotext` 4.00 (xpdf) sull'host ha prodotto byte non UTF-8 su un PDF: la lettura ora sostituisce le sequenze invalide con U+FFFD invece di fermarsi. Le misure usano `pdftotext` 25.03.0 del runner.
- **[NV] RESEARCH §11.10** risolto: TEI 1.9.4 avvia `BAAI/bge-m3` (XLM-RoBERTa) senza problemi; l'istruzione di Qwen3 è passata come prefisso esplicito, non con `prompt_name`. **[NV] §11.14** risolto: `INSERT … SELECT … FROM UNNEST(…, $9::vector[])` con `Vec<pgvector::Vector>` funziona con sqlx 0.9 (3.950 righe in circa 1 s).
- **Istruzione di Qwen3.** Senza istruzione Qwen3 va leggermente meglio (+0,019 di nDCG@10 complessivo, sotto la soglia di pari). Non ho provato varianti del prompt (per esempio senza lo spazio dopo `Query:`): anche la variante migliore resta 0,063 sotto bge-m3.
- **Ampiezza del campione.** 60 query valutabili, di cui 12 cross-lingual: le differenze fra modelli non hanno un intervallo di confidenza. Il distacco di bge-m3 regge in inglese e cross-lingual; in italiano e5 è avanti di 0,027.
- **Sessioni.** La prima sessione si è fermata sull'`hw.json`; la seconda ha misurato i tre modelli; la terza ha prodotto candidati e dato CPU. Il processo che seguiva la seconda sessione dal lato agente è stato chiuso per pressione di memoria, ma la sessione è arrivata a fine lavoro con `down` + `release` regolari.

## Esito

- **Decisione proposta:** modello di embedding **BAAI/bge-m3** (revisione `5617a9f61b028005a4858fdac845db406aefb181`, licenza MIT, 1024 dimensioni, nessuna istruzione di query), servito da **TEI 1.9.4 CUDA** (`86-1.9.4`, `float16`) sulla RTX 3050 con `--max-batch-tokens 16384` e lotti client da 64. nDCG@10 complessivo 0,7226, recall@10 0,90, nessun calo sopra 0,10 fra le lingue, query p95 12 ms, 25 pagine in 1,0 s, VRAM 1,42 GiB.
- **Alternative scartate:** Qwen3-Embedding-0.6B, il default di D-14, per la qualità (nDCG@10 0,6408 con istruzione e 0,6595 senza, cioè 0,063-0,082 sotto) e per il serving 2,2 volte più lento (30,9 contro 69,3 chunk/s); multilingual-e5-large-instruct per la qualità complessiva (0,6180, −0,105) e soprattutto per il cross-lingual (0,4181: M-3 fallirebbe di 0,20), pur essendo il migliore sull'italiano (0,7157) e il più veloce. Serving CPU: dato di ripiego, 3,0 chunk/s e 24 s per il documento da 25 pagine, fuori dal budget TTQ di 15 s: la GPU serve per M-5.
- **Impatto sulle porte (S-07):** `Embedder` espone `embed_documents(&[&str]) -> Vec<Vec<f32>>` ed `embed_query(&str) -> Vec<f32>` separati, perché l'istruzione di query è una proprietà del modello (pesa +0,14 di nDCG@10 su e5, nulla su bge-m3) e l'adapter la applica solo alla query; la porta dichiara dimensione (1024) e identità del modello (id + revisione) come dati, così l'indice registra con quale modello è stato costruito e un cambio di modello impone la reindicizzazione completa. Nessun tipo TEI o `reqwest` attraversa la porta.
- **Impatto sul threat model:** T-20 (embedding come dato sensibile): gli embedding restano nel database e non escono da alcuna API; i testi dei chunk vanno solo a TEI locale (porta su 127.0.0.1, nessun invio fuori dalla macchina). T-13 / D-16: con il predicato ACL nella stessa query della ricerca esatta il leak è 0 su 74 query e su 1.479 candidati, contato contro la matrice attesa e non contro `doc_acl`. T-01-10-04: revisione dei pesi fissata e verificata contro `/info`.
- **Destinazione:** [ADR-0007](../../docs/adr/0007-embedding-tei-gpu.md) (nuovo, Stato `Proposed`); la riga `DEPENDENCIES.md` «Embedding model» passa da Aperta a Decisa con bge-m3 su TEI CUDA, e la scrive il consolidamento (01-17). Contratto per 01-14 e 01-15: crate `s03-pgvector` (`chunk`, `tei`, `db`, `metrics`), tabelle di `sql/schema.sql`, candidati top-20 di bge-m3 rigenerabili con `export-candidates --model bgem3 --k 20` (sha256 in `results/embedding/candidates_manifest.json`, stabile fra due esecuzioni sullo stesso DB).

**Modifica:** Claude (gsd-executor 01-10) · 2026-10-22 · setup, risultati, anomalie ed esito dopo le misure (Task 2 del piano 01-10)
