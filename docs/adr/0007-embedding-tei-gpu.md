# ADR-0007 — Modello di embedding bge-m3 servito da TEI su GPU

- Stato: Accepted
- Data: 2026-10-22
- Decisori: utente (ratifica nel consolidamento, piano 01-17); proposta di Claude (gsd-executor 01-10) dallo spike S-03/A-03, parte embedding
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-10) · 2026-10-22 · ADR proposto con le misure dello spike S-03/A-03, parte embedding
**Modifica:** Claude (Anthropic) · 2026-10-26 · ratificato dall'utente nel piano 01-17 (ratifica-tutti)

## Contesto

Il Document & Indexing Service calcola gli embedding dei chunk per la ricerca semantica e quello della domanda a ogni query. La riga `DEPENDENCIES.md` «Embedding model» è **Aperta**. Le decisioni della fase 1 (01-CONTEXT) da confermare con i numeri sono:

- **D-13:** serving locale con Text Embeddings Inference (TEI) su GPU (RTX 3050, 6 GB, condivisa con il reranker e con Docling), con un dato CPU di ripiego; il ripiego su CPU è una decisione dell'utente.
- **D-14:** tre modelli candidati da 1024 dimensioni, Qwen3-Embedding-0.6B (default), BAAI/bge-m3 e multilingual-e5-large-instruct, confrontati sulle stesse query etichettate in italiano, inglese e cross-lingual.

Non esiste un benchmark pubblico su testi finanziari e societari italiani: le classifiche generali (MTEB) non dicono quale modello regga relazioni annuali di banche centrali, bilanci e documenti interni in due lingue. Il corpus di misura ha 19 documenti indicizzabili (10 PDF pubblici in italiano e inglese, 9 Markdown sintetici con quattro classificazioni) e 74 query con oracoli ratificati.

Vincoli: il testo dei documenti non lascia la macchina (T-20); il filtro ACL sta nella stessa query della ricerca (D-12, D-16); il Time To Queryable di un documento da 25 pagine è di 15 s p95, di cui circa 5 s per il parse.

## Opzioni considerate

Numeri dallo spike, ricerca esatta con ACL nella stessa query, 60 query valutabili; serving sulla configurazione migliore di ogni modello entro il budget di VRAM, ripetizioni 2 e 3.

1. **Qwen3-Embedding-0.6B** (Apache-2.0, istruzione di query).
   - Vantaggi: default di D-14, modello recente; recall@10 0,825.
   - Svantaggi: nDCG@10 complessivo 0,6408 con istruzione e 0,6595 senza; cross-lingual 0,5837; 30,9 chunk/s, 2,2 volte più lento degli altri due; 25 pagine in 2,25 s.
   - Rischi: l'istruzione di query non migliora i risultati sul corpus (−0,019), cosa che lascia aperta la forma corretta del prompt.
2. **BAAI/bge-m3** (MIT, nessuna istruzione).
   - Vantaggi: nDCG@10 complessivo 0,7226, il più alto; cross-lingual 0,7968; recall@10 0,90; 69,3 chunk/s, 25 pagine in 1,02 s, query p95 12 ms, VRAM 1,42 GiB.
   - Svantaggi: sull'italiano è dietro a e5 (0,6884 contro 0,7157).
   - Rischi: modello del 2024; bge-m3 offre anche vettori sparsi e multi-vettore, che qui non si usano.
3. **multilingual-e5-large-instruct** (MIT, istruzione di query).
   - Vantaggi: migliore sull'italiano (0,7157), il più veloce (71,3 chunk/s).
   - Svantaggi: nDCG@10 complessivo 0,6180 e cross-lingual 0,4181, sotto la soglia di calo massimo di 0,10 rispetto alla media; senza istruzione crolla a 0,4752.
   - Rischi: dipendenza forte dal prefisso di query.
4. **Serving CPU** (TEI `cpu-1.9`, bge-m3): 3,0 chunk/s, 24 s per il documento da 25 pagine, query p95 50 ms: fuori dal budget TTQ.

## Decisione

Il modello di embedding è **BAAI/bge-m3**, revisione `5617a9f61b028005a4858fdac845db406aefb181`, servito da **TEI 1.9.4 CUDA** (immagine `86-1.9.4`, `float16`) sulla GPU locale, con `--max-batch-tokens 16384` e lotti client da 64. La revisione dei pesi e il digest dell'immagine sono fissati e verificati all'avvio contro `model_sha` di `/info`.

La scelta segue la regola scritta prima della misura: nDCG@10 complessivo più alto (0,7226), con un distacco di 0,063 dal secondo, sopra la soglia di pari di 0,02. Il modello scelto rispetta tutte le soglie: calo dell'italiano sull'inglese 0,031 (≤ 0,10), cross-lingual sopra la media, query p95 12 ms (≤ 50), 25 pagine in 1,02 s (≤ 4), VRAM 1,42 GiB (≤ 2,5), nessun leak.

## Conseguenze

- Positive:
  - qualità migliore sul corpus bilingue, in particolare sul cross-lingual, che per un assistente con documenti in italiano e inglese conta;
  - l'embedding del documento da 25 pagine occupa circa 1 s del budget TTQ di 15 s;
  - 1,42 GiB di VRAM lasciano spazio al reranker e a Docling nei 6 GB della RTX 3050;
  - licenza MIT; nessun testo inviato fuori dalla macchina.
- Negative:
  - serve una GPU: su CPU il documento da 25 pagine richiede 24 s e sfora il budget TTQ;
  - la GPU è condivisa: le misure valgono con la GPU non occupata da altri carichi;
  - il campione è piccolo (60 query valutabili, 12 cross-lingual) e senza intervallo di confidenza.
- Follow-up:
  - S-07 (piano 01-16) trascrive in `Embedder` `embed_documents` ed `embed_query` separati, con dimensione e identità del modello (id + revisione) come dati della porta; l'istruzione di query resta un dettaglio dell'adapter;
  - ogni cambio di modello o di revisione impone la reindicizzazione completa: l'indice registra con quale modello è stato costruito;
  - il piano 01-14 decide indice e soglia HNSW con le tabelle e la libreria `s03-pgvector`; il piano 01-15 usa i candidati top-20 di bge-m3;
  - il consolidamento (01-17) porta la riga `DEPENDENCIES.md` «Embedding model» da Aperta a Decisa e questo ADR ad `Accepted` dopo la ratifica dell'utente.

## Evidenze

- Spike: [`spikes/s03-pgvector/RISULTATI-embedding.md`](../../spikes/s03-pgvector/RISULTATI-embedding.md) (soglie scritte prima della misura, setup, risultati, anomalie, esito).
- Hardware: [`spikes/s03-pgvector/results/embedding/hw.json`](../../spikes/s03-pgvector/results/embedding/hw.json) (sessione `01-10-20261022t195602z-b9ee`).
- CSV: `spikes/s03-pgvector/results/embedding/` (`models.csv`, `query_instruction.csv`, `throughput.csv`, `chunks_manifest.json`, `candidates_manifest.json`).

| Misura | Qwen3-0.6B | **bge-m3** | e5-large-instruct | Soglia |
|---|---|---|---|---|
| nDCG@10 complessivo | 0,6408 (0,6595 senza istruzione) | **0,7226** | 0,6180 | il più alto |
| nDCG@10 it / en / cross | 0,6306 / 0,6796 / 0,5837 | 0,6884 / 0,7198 / 0,7968 | 0,7157 / 0,6202 / 0,4181 | calo ≤ 0,10 |
| recall@10 complessivo | 0,8250 | 0,9000 | 0,8167 | — |
| chunk/s (GPU) | 30,93 | 69,28 | 71,31 | — |
| 25 pagine p95 (s) | 2,248 | 1,017 | 0,992 | ≤ 4 |
| query p95 (ms) | 17,10 | 12,00 | 11,89 | ≤ 50 |
| VRAM picco (MiB) | 1.662 | 1.453 | 1.389 | ≤ 2.560 |
| Leak (chunk non visibili nei top-10) | 0 | 0 | 0 | = 0 |
