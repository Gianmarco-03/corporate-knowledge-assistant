# 01 — Gate sui piani (fase 1, Spike tecnici)

17 piani. Tre giri. Esito finale: **17/17 conformi**. Exit code grezzi: 0 pass, 1 fail, 2 non eseguito.

## Esito per gate (giro finale)

| Gate | Script / exit | Esito |
|---|---|---|
| solid-check | review (nessuno script su piani) | conformi 01,04,05,07–16; non applicabile 02,03,06,17 (niente Rust/TS) |
| crate-boundary-check | `boundary_check.py --plan` exit 0 su 17/17 | verde vuoto su `spikes/**` (FUORI MAPPA) → regole 1–6 non applicabili agli spike (workspace separato, usa-e-getta); confine prodotto↔spike conforme 01-01, 01-17; 01-16 conforme (dipendenze chiuse + grep `pure_forbidden_crates`) |
| security-scanner | `security_scan.py` exit 0, 0 finding, 23 file | 17/17 conformi |
| perimetro-check | `gate-perimetro.cjs piani` exit 0 | 17/17 puliti |
| proprieta-file | `gate-proprieta-file.cjs` exit 0 (roadmap) / `--fase 01` exit 0 (239 voci) | conforme; da rilanciare quando esistono piani di Fondazione/Provider |
| eval-oracle-check | giudizio (nessuno script) | in ambito 03,06,08–15 conformi; non applicabile 01,02,04,05,07,16,17 |

## Giri

- **Giro 1** — da correggere: 01-04 (solid: `validate::check` sincrono in handler async), 01-16 (solid ISP: baseline forzata su `DecisionEngine` a 4 metodi; crate-boundary: dipendenze `s07-ports` aperte, grep incompleto), 01-13 (security high: dipendenze transitive pip senza hash), 01-06 (eval CRITICAL: `parser-it.json` non verificato, schema e ref per classe mancanti), 01-09 (eval: `section_path` senza atteso indipendente), 01-13 (eval: oracoli pendenti non esclusi).
- **Correzioni** — 01-04 `spawn_blocking` + grep; 01-16 trait stretto `Reranker` (JEV + baseline TEI), `DecisionEngine` = validate_query/assess_evidence/validate_citations, divergenza da `interfaces-and-concurrency.md` registrata in ADR-0012, `[dependencies]` chiuse + grep su lista completa; 01-15/01-05/01-11 allineati a `Reranker::rank`; 01-13 `pip install --require-hashes -r paddle-requirements.lock` + ORACOLI.md in read_first + solo voci tre-verdi; 01-06 schema e verifiche su entrambi i file; 01-09 `expected_section_path` a mano prima della misura; non bloccanti: curl con tetti (01-03, 01-08), mapping variabili Garage (01-02), pin revisione HF (01-10, 01-15), T-01-02-07 rischio residuo.
- **Giro 2** — tutti i finding del giro 1 risolti; nuovo da correggere 01-09 (file dei campioni attesi senza nome, fuori `files_modified`).
- **Giro 3** — 01-09: file `spikes/s02-parser/locator-gen/expected_samples.json` nominato, in `files_modified` e `<files>` del Task 2, commit proprio prima dei risultati, verifica `expected_section_path` non vuoto. Note: 01-11 Purpose allineato; 01-04 sezione `[admin]` di Garage omessa (token a runtime come `*_file`, mai nuove chiavi di `.env.example`). Gate meccanici rilanciati: estrai 0, perimetro 0, proprieta-file 0.

## Righe giudicate legittime (il re-gate non le ridiscute)

- crate-boundary: tutte le voci `[FUORI MAPPA] spikes/**` (01-01..01-16) — workspace spike separato, fuori da `boundaries.json`, usa-e-getta, mai importato dal prodotto; `spikes/Cargo.lock` — lockfile del workspace spike; `docs/adr/00NN-*`, `DEPENDENCIES.md`, `docs/security/threat-model.md` — `non_crate_areas`.
- solid: 01-10/01-14 enum chiuso di 3 modelli con `match` (insieme chiuso misurato una volta; nome tabella da enum = difesa anti-SQL injection); 01-11/01-15 enum forme A/B/C di JEV; 01-05/01-11/01-12 client JEV duplicato per crate (WIRING §4 vieta client condiviso); 01-04 `#[global_allocator] static` di dhat (imposto dal profiler); 01-04 handler orchestratore (harness di misura, `BlobSink` con 3 adapter rispetta OCP); 01-07 modo `in-process-mutex` solo per confronto M9.
- eval-oracle: soglie degli spike = delega utente in 01-CONTEXT (Claude's Discretion: soglie di pass/fail di dettaglio); `test_id: null` negli oracoli spike (bin di misura, non golden test di prodotto); ground truth TAT-DQA/DocLayNet pubblicato dagli autori (D-11); regola di rilevanza ≥ 50% di 01-10 (metrica di spike, non tolleranza di prodotto); `acl.json` fixture con leak atteso 0 per definizione (D-16); 01-09 `expected_section_path` = fixture meccaniche su documenti sintetici (titoli verbatim), senza double-check GPT — decisione del coordinatore.
- security: T-01-02-07 accept medium (runner con tutte le chiavi provider, rete `parse` condivisa con docling; corpus pubblico/sintetico, tetti di spesa).

## Note per le fasi successive

- L'aggiornamento di `docs/architecture/interfaces-and-concurrency.md` (split `Reranker` / `DecisionEngine`) non ha un piano proprietario in fase 1: va alla Fondazione (fase 2); ADR-0012 lo registra.
- `proprieta-file` va rilanciato sull'intera roadmap quando esistono i piani delle Fasi 2–4.

**Modifica:** Claude (Anthropic) · 2026-10-18 · gate plan 1 (tre giri)
