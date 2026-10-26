# Fase 1 — banda ECC di /verify 1

**Modifica:** Claude (Anthropic) · 2026-10-26 · registro della verifica ingegneristica della fase 1 (catene, review per linguaggio, sicurezza, verifica sui dati) e instradamento deciso dall'utente

## Ambito

Diff di fase `ea09d68..5e31a7e` (dal commit che precede il primo del piano 01-01 alla registrazione di 01-17): 147 file di codice e strumenti sotto `spikes/`, più `skills/security-scanner/security_scan.py`, `.githooks/controlli-comuni.sh`, `README.md`, `DEPENDENCIES.md` e i documenti. I risultati in `spikes/**/results/` sono stati letti come prova, non rivisti come codice.

## Catene

| Controllo | Esito |
|---|---|
| Gate di prodotto Rust (`Cargo.toml` alla radice) | non applicabile: il workspace di prodotto non esiste ancora |
| Gate web (`apps/web/package.json`) | non applicabile |
| e2e Playwright | non applicabile: nessuna superficie UI |
| Regressione sull'evaluation corpus | non applicabile: `evaluation/datasets/` vuoto, nessuna baseline; gli oracoli degli spike sono misure di spike, non il corpus di evaluation |
| `node scripts/lint-stato.cjs` | verde |
| `cargo fmt --all -- --check` (spikes) | verde |
| `cargo clippy --workspace --all-targets --locked --offline -- -D warnings` (spikes) | verde |
| `cargo test --workspace --locked --offline` sull'host (spikes) | verde su tutti i crate tranne `s02-parser`, che ha 20 test che richiedono qpdf, PDFium Linux e bash del runner (falliscono con «serve il runner») |
| `cargo test -p s02-parser` nel runner (`tools/session.sh with verify1 … oneshot`) | verde: tutti i binari e le librerie, 37 test d'isolamento compresi, nessun falso rosso in questa esecuzione |

## Review

Quattro revisori in sola lettura, in parallelo: `ecc:rust-reviewer` su common, s01 e s02; `ecc:rust-reviewer` su s03-s07; `ecc:python-reviewer` su script, Dockerfile, compose, scanner e hook; `ecc:security-reviewer` sull'intero diff, con la verifica delle righe «misurata nello spike» del threat model. Nessun segreto reale trovato: storia git, CSV, `hw.json`, argv, immagini e `.env*` tracciati sono puliti.

Conteggio dopo la deduplica fra revisori: 0 critici, 14 alti, una quarantina di medi, una trentina di bassi.

### Verifica sui dati dei finding che toccano le misure

Un agente separato ha controllato sui CSV e sui raw rimasti se sei difetti di misura hanno alterato numeri ratificati.

| Difetto | Verdetto | Prova |
|---|---|---|
| `run_ttq.sh:211-217` considera riuscito l'OCR senza contare i rapporti `ok` | non determinabile, esito ratificato non alterabile | `ocr_pages` sempre uguale all'atteso in `ttq_parse.csv`, ma è il valore pianificato; raw persi; `tesseract_ms` 497-943 ms per pagina mai 0; il p95 di TTQ-parse è dominato da Docling, quindi un OCR mancato accorcerebbe solo la parete |
| `budget_real.py:259-269` M10 senza confronto dei `job_id` | misura intatta | `budget-real-jobs.csv`: 178 righe, 178 `job_id` univoci, indici contigui, 0 input vuoti, `job_errors` 0 |
| `vram.sh:213-219` carico con rc ≠ 0 solo loggato | misura intatta | log della sessione: «carichi: tei-embed=0 tei-rerank=0 docling=0; oom=false» |
| `dhat_25m.rs:128-146` legge i JSON di altre run | misura intatta (indizi forti) | i tre JSON sono entrati nello stesso commit `15ea345` di una sola sessione |
| `ocr/main.rs:437-441` SIGPIPE oltre 4 KiB di stderr | misura intatta | 0 esiti `Corrupt`/`tesseract_exit` nei CSV OCR |
| `fixtures.sh:31-36` pool ridotto | misura intatta | pool pieno (876 GiB liberi), 0 codici 000, 6.000 latenze tutte valorizzate |

**Nessuna misura ratificata negli ADR 0003-0012 risulta alterata.**

## Instradamento (decisione dell'utente, 2026-10-26)

### A. Corretto in questa verifica

- **Threat model:** sette righe dichiarate «misurata nello spike» erano provate solo in parte: T-13, T-14, T-16, T-17, T-24, T-28 e T-29. Il caso più netto è T-14: `unauthorized_sent` e `unauthorized_in_prompt` valgono 0 per costruzione, perché il riconteggio riusa lo stesso predicato sugli stessi passaggi già filtrati, e la guardia è provata solo dai test unitari. Decisione dell'utente: «Riscrivi l'evidenza». La colonna dice cosa è provato e cosa resta al prodotto, e lo stato passa a «da implementare» dove manca la prova a runtime. Si aggiungono il limite del contenimento di T-09 (un parser già compromesso può uscire dal gruppo di processi con `setsid`) e una sezione «Vincoli per le fasi di prodotto». La nuova versione va riratificata in UAT.
- **`spikes/s06-llm/RISULTATI.md`:** la frase «riconteggio indipendente sul punto di invio» viene corretta in coerenza con T-14.

### B. Gap-closure 1.6 (piani di codice, prima di chiudere la fase)

Decisione dell'utente: «Strumenti + TODO noti».

1. **Scanner** `skills/security-scanner/security_scan.py`:
   - correzione D-41 (regola `upload-in-memory` sensibile alla posizione di `.take(`, nuova regola su `io::copy` verso `Vec`, test sulle forme), con il prescan riportato a `take` + `read_to_end`;
   - regole dei segreti estese a `*_SECRET_KEY`, `*_ACCESS_KEY`, `*_TOKEN` e ai prefissi noti (`sk-ant-`, `GK…`);
   - file testuali senza estensione di codice (`.txt`, `.csv`, `.pem`, `.key`) scansionati per le sole regole dei segreti;
   - allowlist versionata per il marcatore `.security-scan-fixture` e gate sulle directory escluse;
   - potatura limitata a `target`, `node_modules` e `.git`;
   - file illeggibile come finding bloccante;
   - macro di log su più righe normalizzate prima delle regole no-leak;
   - handler Axum senza estrattore d'identità segnalati anche senza argomenti;
   - exit code senza percorsi chiarito.
2. **Hook** `.githooks/controlli-comuni.sh`:
   - controlli Rust anche sul manifest `spikes/Cargo.toml`, oppure dichiarazione esplicita che gli spike non sono coperti;
   - stesse regole dei segreti dello scanner;
   - commit di soli dati non più bloccati da «security-scope-empty».
3. **Runner e compose** `spikes/compose.yaml`:
   - `cap_drop: [ALL]`, `no-new-privileges` e `pids_limit` sul runner;
   - segreti passati solo ai job che li usano;
   - `read_only` e `pids_limit` su paddle;
   - `docling --all` (riga ~175) corretto;
   - revisioni dei pesi obbligatorie (`${EMBED_REVISION:?}`).
4. **Misure:**
   - `run_ttq.sh` e `ttq-merge` richiedono N rapporti `ok` pari alle pagine OCR attese;
   - `vram.sh` esce 1 se un carico fallisce;
   - `budget_real.py` confronta l'insieme dei `job_id`;
   - `fill_manifest` esce 1 sul mismatch.
5. **Test e TODO noti:**
   - test d'isolamento di s02 flaky sotto carico;
   - pulizia delle directory tmp dei test di s02;
   - `results/docling-models.txt` allineato ai modelli rimossi;
   - calibrazione di `BudgetExceeded` (`parse-parent.rs:599-610`, `isolate.rs:120-125`);
   - piccole incoerenze di testo già annotate: nota di `isolate.rs:534`, timestamp di `gen_hostile.py:577`, anomalia di `ORACOLI.md:39`, maiuscole miste negli stati di overhead.

### C. Vincoli scritti per le fasi di prodotto (non si corregge il codice degli spike)

Sono elencati nel threat model, sezione «Vincoli per le fasi di prodotto emersi dalla verifica della fase 1»:

- isolamento con `cgroup.kill` o namespace PID per job, più `RLIMIT_NPROC`;
- client verso i provider con URL parsato, host verificato e redirect disattivati;
- `Debug` redatto anche per `CacheValue`, `Embedding` e i tipi con locator;
- ricontrollo per passaggio in `authorize_response`;
- testo validato costruito solo dai claim ammessi;
- tetto di spesa con prenotazione del costo;
- pesi verificati all'avvio e immagini per digest;
- una sola regola per il magic PDF;
- marcatori XML cercati anche in UTF-16.

### D. Residui accettati con motivo

- Pattern dei contatori tautologici in s04 e s06: non si rimisura, perché la guardia esiste ed è provata dai test unitari. Il prodotto conta sul corpo serializzato (vincolo C).
- Debito delle immagini inline, L-4, JBIG2 (T-09) e dipendenze apt indirette non pinnate: restano dichiarati negli ADR ratificati e nel threat model.
- Finding medi e bassi di stile, o limitati a script usa-e-getta (per esempio `format!` in DDL con valori interni, `set_var` nei test di common): elencati nei rapporti dei revisori, nessuna azione.
