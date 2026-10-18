# ORCHESTRATOR-STATUS — Corporate Knowledge Assistant

Stato persistente dell'orchestratore del loop GSD+ECC. Lo scrive solo `update-orchestrator-state`.
Regole di sanità: (1) indice, non archivio — una riga + puntatore; (2) mai fatti GSD (avanzamento piani/fasi sta in STATE.md/ROADMAP.md).

## Posizione

fase 01 / onda — / slice — / discuss completato · aggiornato: 2026-10-18T16:20:00Z

## Prossima azione

/discuss 2 (Fondazione) — strategia U-04: discuss di tutte le fasi prima di eseguire gli spike.

## Findings

- fase 01 criticità critica, ratificata → 01-CONTEXT.md
- storage S3 Garage, chiave sha256 + GC orfani, dedup invisibile (D-01/02) · fonte Area A
- tempfile + validazione pre-persistenza; limite 25 MB; L1 + regole economiche (D-03..05)
- PDFium + Docling solo tabelle; processo figlio isolato; Tesseract CLI + PP-OCRv5 sfidante (D-07..09) · fonte Area B
- locator 2 varianti: PDF pagina+rect / testo sezione+offset (D-10)
- ricerca esatta MVP, HNSW a soglia; tsvector + RRF (D-12, D-15) · fonte Area C
- embedding locale GPU, TEI, Qwen3-0.6B + sfidanti bge-m3/e5 (D-13/14)
- corpus pubblico/sintetico; JEV pin jev-1.13.0; LLM API Haiku/Sonnet con routing JEV, fallback Sonnet (D-16..19) · fonte Area D
- risposta JSON structured output, citazioni per claim con chunk_id (D-20); reranker baseline no-rerank + bge-reranker GPU (D-21)

## Limiti trovati

*(vuoto)*

## Gate & verdetti

| Piano | Gate sul PLAN | Review ECC | Verifier GSD |
| --- | --- | --- | --- |

## UAT

*(vuoto)*

## TODO

- hardware spike: i7-13700K 16C/24T, 32 GB RAM, RTX 3050 6 GB VRAM, Windows 11.
- Docker Desktop non installato: prerequisito spike (passthrough GPU WSL2 per TEI).
- remote GitHub non creato: attende `gh auth login` utente.
- S-07 porte: Embedder copre embedding query; validate_query restituisce complessità; locator in contracts.
- DEPENDENCIES.md da aggiornare a esito spike (JEV pin, A-02..A-10).

## Decisioni ratificate in corsa

- strategia provider-first + insiemi file chiusi (U-04) · utente · 2026-10-18 → PROJECT.md
- racconto didattico per slice · utente · 2026-10-18 → skills/execute/SKILL.md §bridge 3-bis

**Modifica:** Claude (Anthropic) · 2026-10-18 · creazione (preflight discuss 1)
**Modifica:** Claude (Anthropic) · 2026-10-18 · --discuss fase 01
