# Phase 1: Spike tecnici - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-18
**Phase:** 01-spike-tecnici
**Areas discussed:** Upload e storage, Parser OCR e pagine, Indice ed embedding, JEV LLM e citazioni

---

## Upload e storage

| Domanda | Opzioni | Scelta |
|---|---|---|
| Storage originali (A-02) | PG bytea a chunk (racc.) · S3 Garage · S3 RustFS · Misura nello spike | **S3 Garage** |
| Tempfile o streaming diretto | Tempfile (racc.) · Streaming diretto | **Tempfile** |
| Difese file oltre L1 | L1 + regole economiche (racc.) · ClamAV · Solo L1 | **L1 + regole economiche** |
| Coerenza oggetto/transazione | sha256 + GC, dedup invisibile (racc.) · UUID senza dedup · Decidi tu | **sha256 + GC, dedup invisibile** |
| Limite rigido | 25 MB (racc.) · 10 MB · 50 MB | **25 MB** |
| Baseline PG nello spike | Misura entrambi (racc.) · Solo Garage | **Misura entrambi** |

**Note:** l'utente preferisce S3 da subito al default raccomandato (PG bytea).

## Parser, OCR e pagine

| Domanda | Opzioni | Scelta |
|---|---|---|
| Tabelle nei PDF | PDFium + Docling solo tabelle (racc.) · Solo Rust · Docling per tutto | **PDFium + Docling solo tabelle** |
| Isolamento | Processo figlio (racc.) · spawn_blocking | **Processo figlio** |
| Pagina DOCX/MD | Sezione + offset (racc.) · DOCX→PDF LibreOffice | **Sezione + offset** |
| Dataset | Includi FinanceBench (racc.) · Solo permissive | **Includi FinanceBench** |
| OCR | Tesseract + PP-OCRv5 sfidante (racc.) · Solo Tesseract | **Tesseract + PP-OCRv5 sfidante** |

**Note:** la prima formulazione della domanda sulle tabelle è stata fraintesa (tabelle del DB); chiarito che si tratta delle tabelle dentro i PDF (bilanci) e riproposta.

## Indice ed embedding

| Domanda | Opzioni | Scelta |
|---|---|---|
| Indice + filtro ACL | Esatta ora, HNSW a soglia (racc.) · HNSW da subito | **Esatta ora, HNSW a soglia** |
| Embedding locale o API | Solo locale (racc.) · API baseline eval · API anche a runtime | prima «API anche a runtime», poi chiarito: **embedding locale**, ragionamento via API |
| Serving | TEI · fastembed-rs · Ollama | prima «non abbiamo LLM locale»; dopo valutazione dei rischi CPU, **GPU + container TEI** |
| Modello | Qwen3 default con sfidanti (racc.) · bge-m3 default | **Qwen3 default, bge-m3 ed e5 sfidanti** |
| Ricerca ibrida | tsvector + RRF (racc.) · pg_textsearch subito · Solo vettoriale | **tsvector + RRF** |

**Note:** l'utente ha chiesto i rischi prestazionali dell'embedding su CPU (stime: 20–50 s per 25 pagine con modello 0,6 B fp32; contesa con parser/OCR); ha scelto la GPU.

## JEV, LLM e citazioni

| Domanda | Opzioni | Scelta |
|---|---|---|
| Corpus | Pubblico/sintetico (racc.) · Riservato simulato | **Pubblico/sintetico** |
| Versione JEV | Pin jev-1.13.0 (racc.) · jev-latest | **Pin jev-1.13.0** |
| LLM default | Haiku default + Sonnet sfidante (racc.) · Sonnet default · Altro | **Haiku/Sonnet con routing per complessità deciso da JEV** (testo libero dell'utente) |
| Citazioni | Claim strutturati + JEV (racc.) · Citations native | **JSON con citazioni come campo** (testo libero), poi **per affermazione** |
| Fallback senza JEV | Sonnet (racc.) · Haiku · Rifiuto | **Sonnet 5.5** |
| Reranker baseline | No-rerank + bge-reranker su GPU (racc.) · Solo no-rerank | **No-rerank + bge-reranker su GPU** |

**Note:** l'utente ha chiesto se l'LLM può produrre direttamente il JSON: sì, via structured output; il sistema aggiunge solo pagina/coordinate dal `chunk_id` in modo deterministico.

## Claude's Discretion

- Firme Rust candidate delle porte (S-07), GC degli orfani (N ore), soglie di dettaglio degli spike, budget VRAM, scelta del corpus degli spike.

## Deferred Ideas

- Antivirus/CDR (ASVS L2), HNSW oltre soglia, pg_textsearch BM25.
