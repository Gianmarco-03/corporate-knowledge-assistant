# Onde di esecuzione — Fase 01 (Spike tecnici)

Fonte: `gsd-tools phase-plan-index 1` (18 piani, `depends_on` + `files_modified`), `01-WIRING.md` §3-§5, W20 (classi di risorsa, finestra di misura serializzata da `session.sh acquire`). Tetto 2 esecutori/onda. `gate-proprieta-file.cjs --fase 1` exit 0 (18 piani, 286 voci). Overlap `files_modified` dentro ogni onda: zero. Nessun piano con superficie API o migrazioni (fase spike, codice fuori prodotto). Membri workspace `spikes/Cargo.toml`: solo 01-01. `spikes/Cargo.lock`: 01-01 (w1) e 01-16 (w10), onde diverse.

Wave 0 = `wiring-build` (seam di §5 di `01-WIRING.md`) + gate GPU di 01-02 Task 3 come prerequisito misure GPU.

## Onda 1
- **01-01** (workspace Cargo spike + crate `common`) — nessuna dipendenza; unico piano che tocca `spikes/Cargo.toml` members e `Cargo.lock` in w1
- **01-02** (harness compose, `session.sh`, gate GPU, checkpoint chiavi API) — nessuna dipendenza; file disgiunti da 01-01
- Attenzione integratore: merge 01-01 poi 01-02. 01-02 ha checkpoint umano (Task 4 chiavi API + decisione gate GPU): onda chiusa solo dopo risposta utente.

## Onda 2
- **01-03** (corpus e fetch) — dipende 01-02
- **01-04** (upload streaming S-01, admission, GC) — dipende 01-01, 01-02
- Attenzione integratore: file disgiunti; misure serializzate da `session.sh acquire` (exit 3 = attendi). Merge 01-03 poi 01-04.

## Onda 3
- **01-05** (JEV client base) — dipende 01-01, 01-02, 01-04
- **01-06** (oracoli corpus IT) — dipende 01-03
- Attenzione integratore: 01-06 tocca evaluation/oracoli → triplo guardrail (double-check GPT) prima di chiudere. Merge 01-06 poi 01-05.

## Onda 4
- **01-07** (contratto job JSON, lanciatore isolato, prescan, figli, calibrazione) — dipende 01-01, 01-02, 01-03, 01-05
- **01-11** (JEV rank/forme) — dipende 01-01, 01-02, 01-05, 01-06
- Attenzione integratore: 01-07 crea seam W17 (`s02-parser` lib + bin): file chiusi per 08/09/13/18. Classi risorsa CPU (07) vs JEV (11). Merge 01-07 poi 01-11.

## Onda 5
- **01-08** (estrazione PDF + Docling tabelle) — dipende 01-06, 01-07
- **01-18** (casi ostili, budget reale, OOM, validazione 01-07) — dipende 01-02, 01-03, 01-07
- Attenzione integratore: entrambi in `s02-parser` ma file disgiunti (W18: `docling-bench/` 08, `hostile-gen/` + `measure/` + `tests/oom.rs` 18); nessuno edita file di 07. Classi GPU/CPU (08) vs CPU (18): finestra di misura serializzata. Merge 01-18 poi 01-08. Difformità trovate da 18 in «Validazione di 01-07» → gap-closure, non fix in onda.

## Onda 6
- **01-09** (locator DOCX/MD/PDF) — dipende 01-03, 01-07, 01-08
- **01-10** (embedding TEI + pgvector) — dipende 01-01, 01-02, 01-06, 01-08
- Attenzione integratore: 01-09 CPU worker, 01-10 GPU TEI. Fixture a mano di 09 (`offset_vectors.expected.json`, `rect_cases.expected.json`) committate da test-write prima dei risultati. Merge 01-09 poi 01-10.

## Onda 7
- **01-13** (OCR Tesseract vs PP-OCRv6, TTQ-parse, ADR-0004) — dipende 01-06, 01-08, 01-09, 01-10, 01-18
- **01-12** (JEV evidenze/citazioni) — dipende 01-01, 01-02, 01-06, 01-11; **spostato da onda 5** (`wave: 5` nel frontmatter, livello di dipendenza 5) per tetto 2 esecutori: onda 5 aveva 08, 12, 18. Unico consumatore 01-16 (onda 10): nessun ritardo sul percorso critico.
- Attenzione integratore: 13 ha checkpoint umano (Task 1 legittimità pip). Classi CPU/OCR (13) vs JEV (12). Merge 01-12 poi 01-13.

## Onda 8
- **01-14** (pgvector predicato autorizzazione/RRF) — dipende 01-10, 01-13

## Onda 9
- **01-15** (reranker GPU, VRAM) — dipende 01-08, 01-10, 01-11, 01-13, 01-14
- Attenzione integratore: VRAM via `session.sh up tei-embed tei-rerank docling` solo per nome.

## Onda 10
- **01-16** (firme porte S-07, ADR-0012) — dipende 01-04, 01-05, 01-12, 01-13, 01-14, 01-15
- Attenzione integratore: tocca `spikes/s07-ports/Cargo.toml` + `spikes/Cargo.lock` (unico in onda).

## Onda 11
- **01-17** (consolidamento ADR, DEPENDENCIES, threat model) — dipende 01-16, 01-18
- Attenzione integratore: limiti noti da documentare (D-25: PDF con filtri LZW/RunLength/ignoti o ObjStm oltre limite rifiutati → ADR-0004 + documentazione utente).

## Percorso critico
01-01/02 → 03/04 → 05 → 07 → 08 → 09/10 → 13 → 14 → 15 → 16 → 17 (11 onde). Divisione 07/18 (D-30): 08 parte in onda 5 senza attendere misure ostili.

**Modifica:** Claude (Anthropic) · 2026-10-19 · onde della fase 1 (onde-slice): 01-12 spostato in onda 7 per tetto 2 esecutori
