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

*Versioni, digest, revisioni dei modelli e comandi esatti: dopo le misure (Task 2).*
