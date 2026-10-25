# S-03 — Embedding, indice vettoriale e retrieval autorizzato · RISULTATI (indice)

**Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · indice di S-03 con l'esito complessivo delle due parti (piani 01-10 e 01-14)

Lo spike S-03 (assunzione A-03, criterio 3 della fase) ha due parti, ciascuna con soglie scritte prima della misura, setup, risultati e anomalie:

| Parte | Piano | Documento | Hardware |
|---|---|---|---|
| Modello di embedding | 01-10 | [`RISULTATI-embedding.md`](RISULTATI-embedding.md) | [`results/embedding/hw.json`](results/embedding/hw.json) |
| Indice, metrica, ibrida, leak, ingestion concorrente | 01-14 | [`RISULTATI-indice.md`](RISULTATI-indice.md) | [`results/index/hw.json`](results/index/hw.json) |

## Esito

- **Modello di embedding: `BAAI/bge-m3`** (1.024 dimensioni, TEI CUDA), deciso nella parte embedding e proposto in ADR-0007 (`docs/adr/0007-embedding-tei-gpu.md`): nDCG@10 0,7226 e recall@10 0,9000 sulle 60 query valutabili.
- **Ricerca esatta con il predicato di autorizzazione nella stessa query SQL, nell'MVP.** Sul corpus attuale (3.950 chunk) e fino a 10k chunk: p95 SQL 23,7 ms con 4 worker, end-to-end (embedding della query + SQL) 46,0 ms contro 200 ms.
- **Soglia misurata per HNSW: 50.000 chunk.** È il primo punto della griglia 10k/50k/100k/300k in cui la ricerca esatta supera p95 50 ms sul profilo meno selettivo (100,9 ms). Oltre la soglia HNSW con `hnsw.iterative_scan = relaxed_order` risponde in p95 ≤ 16 ms a ogni scala e selettività, ma con `m = 16`, `ef_construction = 64` e `ef_search` ≤ 200 la recall filtrata contro l'esatto resta sotto 0,95 a 50k (0,91), 100k (0,84) e 300k (0,70-0,73), su un corpus sintetico che è pessimistico per l'ANN. Superata la soglia, quindi, HNSW va rimisurato su dati reali e con parametri più alti prima di sostituire l'esatta; `iterative_scan = off` con filtro è escluso (meno di k risultati per tutte le query a selettività 1%).
- **Metrica: coseno** su vettori normalizzati (prodotto interno con top-10 identici su 74 query su 74).
- **`vector` per la ricerca esatta; `halfvec` se si attiva HNSW** (indice di un terzo, build più rapida, recall entro 0,03 nella combinazione migliore).
- **Ricerca ibrida `tsvector` + RRF: non adottata.** recall@10 0,9000 contro 0,9000 della sola densa (soglia +0,03), nDCG@10 +0,0145, p95 +2,7 ms.
- **Leak: 0** chunk negati in top-k su 2.067 righe (4 scale × 19 configurazioni × 9 ruoli × parti `sem`, `lex`, `hybrid`, più il corpus reale), contati contro la matrice attesa e non contro `doc_acl`.
- **Inserimento concorrente:** 0 errori in 5 minuti a 20 chunk/s su 100k chunk; p95 delle query in carico 142,5 ms contro 110,0 ms a riposo (1,29 ×, soglia 2 ×).

## Impatto su porte (S-07)

- `VectorIndex::search(query_embedding, query_text, predicate: AuthorizationPredicate, k, deadline)`. Il predicato lo costruisce il chiamante tramite `AccessPolicy` dai ruoli del soggetto, e l'adapter lo applica **dentro** la query del ranking (`EXISTS` su `doc_acl` con i ruoli in bind). Nessun post-filtro applicativo: un post-filtro restituisce meno di k risultati senza segnalarlo, come mostra la configurazione `iterative_scan = off`.
- `query_text` resta nella firma anche se l'ibrida non è adottata, così una rivalutazione della parte lessicale non cambia la porta; l'adapter MVP lo ignora.
- La forma SQL fa parte dell'adapter: con HNSW il predicato va tenuto come filtro della scansione dell'indice (`EXISTS (… OFFSET 0)`), altrimenti il planner sceglie il seq scan o un hash join e non usa l'indice (RISULTATI-indice, «Anomalie» 2).
- `publish_version` atomica (versione di documento visibile solo a commit completo) è fuori da questa misura: l'ingestion concorrente ha verificato solo assenza di errori e latenza con un commit per documento.

## Impatto sul threat model

- **T-13** (il retrieval restituisce chunk non autorizzati, per un filtro ACL dopo il top-k o assente): il predicato nella stessa query, in entrambe le parti della ricerca ibrida e con ricerca esatta o HNSW, non ha fatto passare nessun chunk negato in nessuna scala e configurazione ([`results/index/leak.csv`](results/index/leak.csv)). La misura non è circolare: il filtro legge `doc_acl`, il contatore la matrice ricalcolata da `acl.json`, dal manifest e dal seme. Il rischio residuo con un indice ANN non è un leak ma una perdita silenziosa di recall (T-01-14-02 del piano): con `iterative_scan = off` `returns_k_rate` è 0 a selettività 1%, con `relaxed_order` è 1 ma la recall filtrata scende sotto 0,95 oltre 10k sul corpus sintetico ([`results/index/recall.csv`](results/index/recall.csv)). È il motivo per cui l'MVP usa la ricerca esatta.
- **T-20** (inversione degli embedding): gli embedding stanno solo nelle tabelle dei chunk, con le stesse ACL dei chunk, e la query restituisce id, chiave del documento e distanza, mai il vettore. Lo spike non ha aggiunto superfici che li espongano; la mitigazione «mai via API» resta da verificare sulle rotte del Document service.

## Destinazione

- [ADR-0006](../../docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md) (indice, metrica, ibrida, soglia), stato *Proposed*, ratifica nel consolidamento (piano 01-17).
- Righe di `DEPENDENCIES.md` «Vector search» ed «Embedding» (`pgvector`) da Proposta a Decisa: le scrive il consolidamento (piano 01-17), non questo piano.
- L'attivazione di HNSW resta un'idea differita (CONTEXT): questo spike misura solo la soglia e le condizioni della rivalutazione.
