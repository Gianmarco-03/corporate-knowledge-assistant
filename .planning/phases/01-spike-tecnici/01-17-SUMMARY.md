---
phase: 01-spike-tecnici
plan: 17
subsystem: consolidamento-threat-model-adr-dependencies
tags: [threat-model, plat-07, stride, adr, ratifica, dependencies, storia, adr-0013, antivirus, cdr, asvs, limiti-noti, d-25, criterio-5, isolamento-spike]

# Dependency graph
requires:
  - phase: 01-04
    provides: "S-01 e ADR-0003 (Garage, deduplica, canale laterale): evidenze di T-05, T-06, T-07, T-08, T-11, T-28"
  - phase: 01-05
    provides: "S-05 e ADR-0010 (deadline, cancellazione, reqwest con Rustls): evidenze di T-24 e T-33"
  - phase: 01-09
    provides: "ADR-0005 (locator di citazione a due varianti)"
  - phase: 01-10
    provides: "ADR-0007 (bge-m3 su TEI GPU, D-38): evidenze di T-28, T-29"
  - phase: 01-11
    provides: "ADR-0008 (JEV forma A, pin jev-1.13.0): evidenze di T-14, T-16, T-24, T-32"
  - phase: 01-12
    provides: "ADR-0011 (Sonnet 5.5 per tutte le query, D-33): evidenze di T-14, T-16, T-17, T-34"
  - phase: 01-13
    provides: "ADR-0004 (PDFium, Docling, Tesseract, processo isolato)"
  - phase: 01-14
    provides: "ADR-0006 (ricerca esatta con predicato ACL, ibrida non adottata): evidenze di T-13, T-36"
  - phase: 01-15
    provides: "ADR-0009 (no-rerank, budget VRAM): evidenze di T-14, T-30"
  - phase: 01-16
    provides: "ADR-0012 (firme delle porte, AuthorizedEvidence, Vincoli verso la Fondazione): evidenze di T-19, T-35"
  - phase: 01-18
    provides: "hostile.csv e RISULTATI-isolamento.md: evidenze di T-06, T-07, T-09, T-10, T-39; calibrazione dei limiti D-25"
provides:
  - "docs/security/threat-model.md compilato (PLAT-07): asset, 9 attori, 5 confini di fiducia, 39 minacce T-01..T-39 in tabella a 10 colonne (21 misurate nello spike con 31 file di evidenza esistenti, 17 da implementare con test e fase, 1 rinviata), checklist LLM/RAG, rischio residuo con responsabile in 9 voci, Storia"
  - "ADR 0003-0012 passati da Proposed ad Accepted dopo la ratifica dell'utente del 2026-10-26 (ratifica-tutti), con una sola riga Modifica ciascuno"
  - "docs/adr/0013-rinvio-antivirus-cdr.md (Accepted): ClamAV e CDR rinviati alla fase 14 con condizione di riapertura"
  - "DEPENDENCIES.md: Vector search, HTTP client, Embedding, Originali PDF/DOCX, Modello testuale JEV e le quattro righe dei Modelli AI aggiornate con link agli ADR e quattro paragrafi Storia"
  - "Sezione «Limiti noti» (D-25) in ADR-0004 e in README.md"
affects: [fase 2 Fondazione, fase 3 ingestion, fase 4 DecisionEngine, fase 5 identity, fase 8, fase 9 viewer, fase 10 RBAC e retrieval, fase 11 audit, fase 12 fonti online, fase 13 evaluation, fase 14 hardening, DEPENDENCIES.md, README.md, verify della fase 1]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Threat model a tabella unica: ogni riga lega minaccia, requisito di REQUIREMENTS.md, file di evidenza dello spike o test previsto con fase, e uno stato chiuso fra tre valori"
    - "Catena di ratifica: ADR Proposed → decisione esplicita dell'utente → Stato Accepted + riga Modifica → riga Decisa in DEPENDENCIES.md col link all'ADR → paragrafo Storia che conserva la regola superata"
    - "Diff minimo sugli ADR di altre slice (solo Stato e Modifica), controllato con git diff --numstat"

key-files:
  created:
    - docs/adr/0013-rinvio-antivirus-cdr.md
  modified:
    - docs/security/threat-model.md
    - DEPENDENCIES.md
    - README.md
    - docs/adr/0003-storage-originali-garage.md
    - docs/adr/0004-parser-ocr-isolamento.md
    - docs/adr/0005-locator-citazione.md
    - docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md
    - docs/adr/0007-embedding-tei-gpu.md
    - docs/adr/0008-jev-forma-rank-concorrenza-pin.md
    - docs/adr/0009-reranker-baseline.md
    - docs/adr/0010-deadline-cancellazione-http.md
    - docs/adr/0011-llm-generativo-routing-structured-output.md
    - docs/adr/0012-firme-porte-candidate.md

key-decisions:
  - "Task 2, decisione dell'utente del 2026-10-26: «ratifica-tutti» su ADR 0003-0012, threat model (4a3b14e) e ADR-0013; nessuna eccezione, nessuna rimisura"
  - "ADR-0006: dopo una spiegazione l'utente ha scelto la «riga Modifica col rimando» (conservazione di query_text in VectorIndex::search superata da ADR-0012) invece di correggere la frase dell'ADR"
  - "DEPENDENCIES.md riporta i valori reali degli ADR e non quelli scritti nel piano prima delle misure: Sonnet 5.5 per tutte le query (D-33), bge-m3 su TEI 1.9.4 (D-38), ibrida non adottata, no-rerank, Tesseract 5.5.0, reqwest 0.13"
  - "AV e CDR rinviati alla fase 14 (ADR-0013), rischio accettato e dichiarato in T-39 e nel Rischio residuo; si riapre prima di caricare documenti reali o di aprire l'upload a utenti esterni"

patterns-established:
  - "Ogni decisione di stack diventa Decisa solo con un ADR Accepted collegato nella stessa riga"
  - "Una regola superata in DEPENDENCIES.md resta scritta in un paragrafo Storia sotto la tabella"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Threat model compilato: asset, attori e confini, minacce T-01..T-28 più quelle emerse dagli spike, mitigazioni mappate ai requisiti e, dove misurate, al file di evidenza (PLAT-07; criterio 5)"
    verification:
      - {test: "verify del Task 1", risks: [RISK-4, RISK-8], ref: ".planning/phases/01-spike-tecnici/01-17-PLAN.md:93", status: pass, evidence: "rieseguito dal bridge dalla radice sul main 26bb554: exit 0; T-01..T-28 presenti, sezioni Asset e Rischio residuo presenti, grep -c spikes/ = 28 righe, security_scan.py OK su 1 file"}
      - {test: TEST-3, risks: [RISK-3], ref: "docs/security/threat-model.md:51-89", status: pass, evidence: "script del bridge (scratchpad, fuori repo): 31 percorsi spikes/ distinti, 0 glob, 0 mancanti; le colonne citate esistono nell'header del CSV della stessa riga: denied_in_topk in index/leak.csv (T-13), unauthorized_sent in jev/forms.csv e rerank/rerank.csv (T-14), unauthorized_in_prompt in s06 llm.csv (T-14), instructs_model_flagged in jev/forms.csv (T-16), claims_unknown_id in s06 llm.csv (T-17)"}
      - {test: TEST-4, risks: [RISK-3], ref: "docs/security/threat-model.md:47 ; :51-89", status: pass, evidence: "stati solo nei tre ammessi: 21 «misurata nello spike» (ognuna con almeno un percorso spikes/ esistente), 17 «da implementare» (colonna Evidenza / Test non vuota, con il test previsto e la fase), 1 «rinviata (ADR-0013)» (T-39, cita ADR-0013)"}
      - {test: TEST-5, risks: [RISK-4], ref: "docs/security/threat-model.md:49-89 ; :5 ; :120-122", status: pass, evidence: "39 righe che iniziano con «| T-NN |», tutte a 10 colonne, STRIDE valido, ogni ID di requisito esiste in REQUIREMENTS.md; T-29 supply chain di pesi e binari, T-30 VRAM, T-31 residenza e retention presso JEV e Anthropic, T-32 varianza JEV; riga delle priorità identica allo stub di 5cfb581; Storia e Modifica presenti"}
      - {test: TEST-11, risks: [RISK-10], ref: "docs/security/threat-model.md:91-102", status: pass, evidence: "lettura manuale del bridge, voce → righe: injection → T-16; retrieval non autorizzato → T-13, T-14, T-15, T-36; esfiltrazione → T-17, T-19, T-22, T-25; hallucination → T-17, T-34; file malevoli → T-06..T-10, T-38, T-39; poisoning → T-18, T-26; denial of wallet → T-05, T-24, T-30, T-33; segreti e PII → T-22, T-31. Ogni voce ha almeno una riga con la mitigazione. Due annotazioni senza effetto sull'esito: il rimando a T-18 per il «rifiuto su evidenza insufficiente» è impreciso (T-18 rifiuta sulla contraddizione); per i segreti nei prompt la copertura è indiretta (T-22 è sui log, T-31 sulla retention)"}
  - must_have: "Ogni esito degli spike è registrato in un ADR o in DEPENDENCIES.md con misure e hardware; ogni ADR nuovo collega RISULTATI e hw.json esistenti (criterio 5)"
    verification:
      - {test: TEST-8, risks: [RISK-7], ref: "docs/adr/0003-storage-originali-garage.md ; docs/adr/0012-firme-porte-candidate.md:76 ; :92", status: pass, evidence: "ciclo esteso a md|json|csv sugli ADR 0003-0011: 57 link, 0 mancanti, almeno un hw.json per ADR; ADR-0012 collega spikes/s07-ports/RISULTATI.md e src/lib.rs, dichiara l'assenza di hw.json (riga 92) e contiene una volta «Vincoli verso la Fondazione» (riga 76)"}
      - {test: "contabilità dei numeri ADR 0003-0013", risks: [RISK-7], ref: "docs/adr/", status: pass, evidence: "0003, 0004, 0005, 0006, 0007, 0008, 0009, 0010, 0011, 0012, 0013: tutti emessi, ciascuno con il proprio file in docs/adr/; nessun numero non emesso"}
  - must_have: "DEPENDENCIES.md riflette le decisioni ratificate (D-01, D-12, D-13, D-14, D-07, D-09, D-18, D-21, D-17, reqwest con Rustls); ogni regola superata resta con un paragrafo Storia"
    verification:
      - {test: TEST-2, risks: [RISK-2], ref: "docs/adr/0003-storage-originali-garage.md:3 ; DEPENDENCIES.md:25-93", status: pass, evidence: "ratifica-tutti del 2026-10-26 (Task 2); ADR 0003-0013 hanno «- Stato: Accepted» e una riga Modifica di ratifica del 2026-10-26; le righe aggiunte con Decisa in git diff 5cfb581..26bb554 -- DEPENDENCIES.md citano solo ADR-0003, 0004, 0005, 0006 (due volte), 0007, 0009, 0010, 0011, tutti ratificati; Modello testuale cita ADR-0008 (tabella JEV senza colonna Stato)"}
      - {test: TEST-6, risks: [RISK-5], ref: "DEPENDENCIES.md:3 ; :33 ; :57 ; :73 ; :93 ; :123", status: pass, evidence: "11 righe rimosse dal diff, tutte nominate in uno dei 4 paragrafi Storia aggiunti (Vector search e HTTP client; Embedding e Originali; Modello testuale; le quattro righe dei Modelli AI e la colonna Scelta); la sezione «Dipendenze da evitare nell'MVP» (riga 123) sta fuori da tutti gli hunk (l'ultimo finisce intorno alla riga 97); firma Modifica alla riga 3; jev-1.13.0 presente"}
  - must_have: "Antivirus e CDR (ASVS L2) rinviati con un ADR dedicato, senza implementarli"
    verification:
      - {test: TEST-10, risks: [RISK-9], ref: "docs/adr/0013-rinvio-antivirus-cdr.md:3 ; :10-49 ; docs/security/threat-model.md:89 ; :108", status: pass, evidence: "cinque sezioni Contesto, Opzioni considerate, Decisione, Conseguenze, Evidenze (una ciascuna); Stato Accepted coerente con ratifica-tutti; link a docs/security/threat-model.md e a spikes/s02-parser/results/isolation/hostile.csv esistenti; nel threat model 2 occorrenze di «rinviata (ADR-0013)» (legenda e T-39) e la voce T-39 nel Rischio residuo con responsabile, motivazione e condizione di riapertura"}
  - must_have: "Il codice degli spike sta in spikes/, fuori dal workspace di prodotto e dalla CI, e nessun file di prodotto lo importa (criterio 5)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "git grep -l \"spikes/\" -- ':!spikes' ':!.planning' ':!docs' ':!*.md' ':!.gitignore'", status: pass, evidence: "output vuoto (exit 1); nessun Cargo.toml alla radice; nessun spikes nei Cargo.toml di services, crates, apps né in .github/workflows; .githooks non cita spikes"}
      - {test: "verify del Task 3", risks: [RISK-1, RISK-7, RISK-8], ref: ".planning/phases/01-spike-tecnici/01-17-PLAN.md:161", status: pass, evidence: "rieseguito dal bridge tale e quale dalla radice: exit 0 (ciclo dei link ADR 0003-0011, ADR-0013 presente, jev-1.13.0, test ! -f Cargo.toml, grep vuoti, security_scan.py OK su 16 file)"}
  - must_have: "Diff degli ADR 0003-0012 limitato a Stato e Modifica, salvo ADR-0004 (Limiti noti D-25); README con Limiti noti"
    verification:
      - {test: TEST-7, risks: [RISK-6], ref: "git diff --numstat 5cfb581 26bb554 -- docs/adr", status: pass, evidence: "0003, 0005, 0006, 0007, 0008, 0009, 0010, 0011, 0012: 2 aggiunte e 1 rimozione ciascuno, e le righe +/- iniziano solo con «- Stato:» o «**Modifica:**». Le 4 righe toccate in ADR-0012 dal coordinatore in 5cfb581 (deadline std::time::Instant) stanno nel commit base, fuori dall'intervallo: il bridge l'ha verificato con git show 5cfb581. Lo stesso numstat dal working tree contro 5cfb581 è identico"}
      - {test: TEST-12, risks: [RISK-6], ref: "docs/adr/0004-parser-ocr-isolamento.md:76-83 ; README.md:111-120", status: pass, evidence: "«## Limiti noti» presente una volta in ADR-0004 e in README; nella sezione di ADR-0004 compaiono LZW ed estrapolat; la sezione del README linka docs/adr/0004-parser-ocr-isolamento.md; diff di ADR-0004: hunk 3 (Stato), 11 (Modifica), 75-83 (riga vuota e sezione Limiti noti, nient'altro)"}
  - must_have: "Nessun segreto né endpoint interno letterale nei documenti"
    verification:
      - {test: TEST-9, risks: [RISK-8], ref: "skills/security-scanner/security_scan.py", status: pass, evidence: "security_scan.py DEPENDENCIES.md docs/adr docs/security: OK su 16 file, exit 0; grep di sk-/GK con 16+ caratteri e postgres:// con credenziali su DEPENDENCIES.md, docs/adr/*.md, docs/security/*.md: vuoto"}
  - must_have: "Ratifica dell'utente (Task 2, checkpoint bloccante)"
    verification:
      - {test: "checkpoint Task 2", risks: [RISK-2], ref: ".planning/tmp/ratifica-01-17.md", status: pass, evidence: "tabella di sintesi presentata all'utente (decisione in una riga, numero principale contro la soglia, soglie mancate dichiarate, punti aperti, link a RISULTATI per ogni ADR, più threat model e ADR-0013); risposta dell'utente del 2026-10-26: ratifica-tutti, con la scelta «riga Modifica col rimando» su ADR-0006"}
      - {test: "accettazione del contenuto ratificato", risks: [RISK-2], ref: "docs/security/threat-model.md:104-118", status: human_judgment, human_judgment: true, rationale: "il bridge prova che gli stati e i link corrispondono alla ratifica, non che le soglie mancate accettate (CER OCR, TTQ-parse, throughput OCR, estrazione PDFium in ADR-0004; latenza del reranker in ADR-0009) e i rischi residui siano accettabili: è il giudizio che l'utente ha espresso con ratifica-tutti e che la UAT di fase può confermare"}

# Metrics
duration: "circa 1 h 51 min: Task 1 4a3b14e 12:14:32 → checkpoint del Task 2 → 9525372 13:59:35 → 843ba0a 13:59:55 → 380ab60 14:00:49 → 668a4f4 14:02:19 → merge 26bb554 14:05:09"
completed: 2026-10-26
---

# Fase 1 Piano 17: consolidamento, threat model, ratifica degli ADR e DEPENDENCIES · Summary

**Il threat model di CKA è compilato (PLAT-07): 39 minacce, ciascuna con requisito, evidenza o test previsto e stato, 21 delle quali misurate su 31 file di evidenza degli spike che esistono. L'utente ha ratificato tutti gli ADR 0003-0012 il 2026-10-26, che sono passati ad Accepted. ADR-0013 rinvia antivirus e CDR alla fase 14. `DEPENDENCIES.md` registra come Decise le scelte misurate (Garage, pgvector con ricerca esatta, reqwest 0.13, bge-m3 su TEI, Sonnet 5.5, PDFium + Docling + Tesseract, no-rerank, `jev-1.13.0`), ciascuna col proprio ADR e con un paragrafo Storia. I controlli del criterio 5, rieseguiti dal bridge, sono verdi.**

## Performance

- **Duration:** circa 1 h 51 min, da `4a3b14e` (12:14:32) al merge `26bb554` (14:05:09) del 2026-10-26. La maggior parte del tempo è l'attesa del checkpoint del Task 2.
- **Started:** 2026-10-26T12:14:32+01:00
- **Completed:** 2026-10-26T14:05:09+01:00
- **Tasks:** 3 di 3 (Task 2 checkpoint di decisione)
- **Files modified:** 14 su 14 dei `files_modified` in `git diff --stat 5cfb581..26bb554`, e nessun altro file. In più c'è una correzione del coordinatore a `README.md`, fatta dopo il merge e non committata (vedi sotto).

## Accomplishments

- **Threat model** (`docs/security/threat-model.md`): riga delle priorità conservata (`:5`) e mappata sulle righe (`:7`); asset (`:11-21`); attori con fiducia, capacità e obiettivo, compresi JEV e Anthropic con retention e regione (`:25-35`); cinque confini di fiducia (`:37-43`); tabella «Threat analysis» T-01..T-39 (`:49-89`); checklist LLM/RAG (`:91-102`); rischio residuo in 9 voci con responsabile e condizione di riapertura (`:104-118`); Storia (`:120-122`).
- **Ratifica** degli ADR 0003-0012: `Stato: Accepted` alla riga 3 di ciascuno e una riga `**Modifica:**` di ratifica del 2026-10-26.
- **ADR-0013** (`docs/adr/0013-rinvio-antivirus-cdr.md`): tre opzioni (ClamAV, CDR, rinvio), rinvio alla fase 14 con condizione di riapertura, prerequisiti già presenti, evidenze.
- **DEPENDENCIES.md**: righe Decise con link agli ADR in Backend Rust (`:25`, `:28`), Persistenza (`:52`, `:54`), JEV (riga «Modello testuale»), Modelli AI (`:88-91`, nuova colonna «Scelta»); quattro paragrafi Storia (`:33`, `:57`, `:73`, `:93`); firma alla riga 3.
- **Limiti noti D-25**: sezione in ADR-0004 (`:76-83`) e in `README.md` (`:111-120`).

## Task Commits

1. **Task 1, threat model:** `4a3b14e` (docs).
2. **Task 2, checkpoint di decisione:** nessun commit. La tabella presentata all'utente è `.planning/tmp/ratifica-01-17.md`.
3. **Task 3:** `9525372` (ratifica degli ADR 0003-0012), `843ba0a` (Limiti noti in ADR-0004 e README), `380ab60` (ADR-0013), `668a4f4` (DEPENDENCIES.md).

**Merge:** `26bb554`, merge del branch `exec/01-17` (genitori `5cfb581` e `668a4f4`). **Plan metadata:** questo SUMMARY e `01-17-RACCONTO.md`, scritti dal bridge senza commit.

## Files Created/Modified

- `docs/security/threat-model.md`: da stub di 5 righe a threat model compilato (118 righe aggiunte, 1 tolta).
- `docs/adr/0013-rinvio-antivirus-cdr.md`: nuovo, Accepted.
- `docs/adr/0003` … `0012`: Stato e una riga Modifica; ADR-0004 anche la sezione Limiti noti.
- `DEPENDENCIES.md`: righe Decise, colonna Scelta nei Modelli AI, quattro Storia, firma.
- `README.md`: sezione Limiti noti (`843ba0a`) e, dopo il merge, la correzione del coordinatore a «Stack deciso».

## Decisione del checkpoint (Task 2)

- **Data:** 2026-10-26. **Decisore:** l'utente. **Opzione:** `ratifica-tutti`. **Oggetto:** gli ADR 0003-0012, il threat model del commit `4a3b14e` e ADR-0013.
- **ADR-0006:** la tabella di ratifica segnalava che l'ADR scrive «La firma della porta conserva `query_text`» (`docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md:31`), mentre ADR-0012 l'ha tolto da `VectorIndex::search`. Dopo una spiegazione l'utente ha scelto la «riga Modifica col rimando» invece di correggere la frase. La riga di ratifica (`:67`) dice quindi che la conservazione di `query_text` è superata da ADR-0012. La frase resta, e il diff resta entro le due righe di TEST-7.
- **Nessuna eccezione e nessuna rimisura:** non nasce nessun piano aggiuntivo della fase 1.

## Contabilità dei numeri ADR (WIRING W13)

| Numero | Stato | File |
|---|---|---|
| 0003 | emesso | `docs/adr/0003-storage-originali-garage.md` |
| 0004 | emesso | `docs/adr/0004-parser-ocr-isolamento.md` |
| 0005 | emesso | `docs/adr/0005-locator-citazione.md` |
| 0006 | emesso | `docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md` |
| 0007 | emesso | `docs/adr/0007-embedding-tei-gpu.md` |
| 0008 | emesso | `docs/adr/0008-jev-forma-rank-concorrenza-pin.md` |
| 0009 | emesso | `docs/adr/0009-reranker-baseline.md` |
| 0010 | emesso | `docs/adr/0010-deadline-cancellazione-http.md` |
| 0011 | emesso | `docs/adr/0011-llm-generativo-routing-structured-output.md` |
| 0012 | emesso | `docs/adr/0012-firme-porte-candidate.md` |
| 0013 | emesso | `docs/adr/0013-rinvio-antivirus-cdr.md` |

Nessun numero non emesso. ADR-0012 contiene «Vincoli verso la Fondazione» (`:76`).

## Controlli finali del criterio 5 (rieseguiti dal bridge)

- Per ogni ADR 0003-0011 esistono tutti i file `spikes/` linkati (`md`, `json`, `csv`) e almeno un `hw.json`.
- ADR-0012 collega `spikes/s07-ports/RISULTATI.md` e `spikes/s07-ports/src/lib.rs` e dichiara l'assenza di `hw.json` (`:92`).
- `test ! -f Cargo.toml` alla radice: vero. `grep -r spikes --include=Cargo.toml services crates apps`: vuoto. `.github/workflows`: nessun riferimento a `spikes`.
- Scanner di sicurezza su `DEPENDENCIES.md`, `docs/adr/`, `docs/security/`: OK, 16 file.
- `gate-proprieta-file.cjs --fase 01`: conforme.

## Esiti dei test del design (rieseguiti dal bridge)

Il test design dichiara solo controlli shell da lanciare dalla radice dopo il Task 3 («Nessun file nuovo»), e nessun file di test compare nei `files_modified`. Per questo il passo `test-write` non si applica a questo piano. Il bridge ha rilanciato i TEST-N come prova:

- **TEST-1** verde: `git grep` con le esclusioni del design vuoto; nessun `Cargo.toml` alla radice; `.githooks` non cita `spikes`.
- **TEST-2** verde: Stato Accepted e Modifica di ratifica su 0003-0013; le righe Decise citano solo ADR ratificati.
- **TEST-3** verde: 31 percorsi, 0 glob, 0 mancanti, colonne citate presenti negli header.
- **TEST-4** verde: 21 misurate, 17 da implementare, 1 rinviata, nessuno stato fuori elenco.
- **TEST-5** verde: 39 righe a 10 colonne, STRIDE e requisiti validi, i quattro temi T-29+ presenti.
- **TEST-6** verde: 4 Storia che nominano le 11 righe rimosse, «da evitare» invariato, firma in testa.
- **TEST-7** verde: 2/1 su ogni ADR tranne 0004, solo righe `- Stato:` e `**Modifica:**`.
- **TEST-8** verde: 0 link mancanti, hw.json in ogni ADR 0003-0011, ADR-0012 conforme, contabilità completa.
- **TEST-9** verde: scanner exit 0, grep dei segreti vuoto.
- **TEST-10** verde: ADR-0013 con le cinque sezioni e i link esistenti, T-39 nel rischio residuo.
- **TEST-11** verde alla lettura manuale, con le due annotazioni del blocco `coverage`.
- **TEST-12** verde: Limiti noti in ADR-0004 (LZW, estrapolat) e README (link ad ADR-0004).

## Decisions Made

### Decisioni dell'utente

- `ratifica-tutti` del 2026-10-26, e la riga Modifica col rimando su ADR-0006 (sopra).
- Già prese in `01-CONTEXT.md` e applicate qui: D-05 (difese L1 più regole economiche, da cui ADR-0013), D-25 (limiti noti dichiarati), D-28 (dimensionamento estrapolato), D-33 (Sonnet anche sulle `simple`), D-38 (bge-m3).

### Scelte degli esecutori

- **Probabilità e impatto** in Alta/Media/Bassa, e ID T-NN di RICERCA invece del T-001 del template (Storia, `threat-model.md:122`).
- **T-12, T-15, T-22, T-23** marcate «da implementare» anche se lo spike ne prova una parte nei contratti candidati: la mitigazione vera è di prodotto. **T-19 e T-35** marcate «misurate» su test unitari di `spikes/s07-ports/src/lib.rs`, non su una misura a runtime.
- **Riga Modifica di ratifica** messa in coda all'ultimo blocco Modifica di ogni ADR, senza riga vuota, per restare a 2 righe aggiunte (TEST-7).
- **Colonna «Scelta»** aggiunta alla tabella Modelli AI, che prima aveva solo stato e criteri (dichiarata nella Storia, `DEPENDENCIES.md:93`).

## Deviations from Plan

### Livello a

1. **Evidenze diverse dal suggerimento del piano** (Task 1). T-24 cita anche `concurrency.csv` e la spesa di S-04 e S-06. T-16 accoppia `instructs_model_flagged` a `jev/forms.csv`, dove la colonna esiste davvero, e non a `injection.csv` come diceva il piano. T-28 cita i `hw.json` concreti che contengono un digest (il bridge ha verificato i digest `sha256:` nei quattro file).
2. **Requisiti non numerati in RICERCA** (T-03, T-22, T-23, T-24, T-28) convertiti in ID che esistono in REQUIREMENTS.md.
3. **Valori reali degli ADR in DEPENDENCIES.md** al posto di quelli del piano, che era stato scritto prima delle misure: Sonnet 5.5 per tutte le query e non «Haiku per le semplici», bge-m3 e non Qwen3, ibrida non adottata, no-rerank, Tesseract 5.5.0, reqwest 0.13. La Storia di `DEPENDENCIES.md:93` lo registra.
4. **Branch `exec/01-17`** fuori dal namespace `worktree-agent-*`: assegnato esplicitamente dall'orchestratore.

### Correzione post-merge del coordinatore (non committata)

- `README.md:58-60`, «Stack deciso»: il paragrafo diceva ancora che object storage, modello generativo ed embedding model erano decisioni aperte e che `pgvector` era solo il candidato predefinito. Il rapporto del Task 3 lo segnalava come stantio e fuori mandato. Il coordinatore l'ha allineato alla ratifica: persistenza con pgvector e ricerca esatta (ADR-0006) e originali su Garage (ADR-0003) alla riga 52; embedding, modello generativo e parser decisi alla riga 58; Storia alla riga 60; firma alla riga 122. `README.md` è nei `files_modified`, quindi resta nel perimetro del piano. La modifica entra nel commit di registrazione.

**Total deviations:** 4 di livello a, più una correzione del coordinatore; nessuna di livello b o c.
**Impact on plan:** nessuna estensione di perimetro (14 file su 14).

## Discrepanze fra rapporto e prove

- **Nessuna sostanziale.** Commit, conteggi (39 minacce, 21/17/1, 31 percorsi, 9 voci di rischio residuo, 4 Storia, numstat 2/1) e posizioni delle righe Modifica coincidono con le riesecuzioni del bridge.
- **«21 righe con evidenza ≥ 10»** (rapporto, TEST-3): sono le 21 righe misurate. Le righe che citano almeno un percorso `spikes/` nella colonna Evidenza sono 28, perché anche alcune «da implementare» citano un'evidenza parziale. La soglia di 10 è rispettata in entrambi i conteggi.
- **TEST-7 e il commit base:** ADR-0012 ha 4 righe modificate dal coordinatore in `5cfb581` stesso. Stanno nel commit base e restano fuori dall'intervallo misurato, quindi il 2/1 del rapporto è corretto.

## Incoerenze negli artefatti (non corrette dal bridge)

- **ADR-0013 citato come testo, senza link**, nel threat model: T-39 (`docs/security/threat-model.md:89`) e Rischio residuo (`:108`). Il file è nato al Task 3, dopo il Task 1. In più la frase «Fonti» (`:9`) nomina solo gli ADR 0003-0012.
- **ADR-0006:31** conserva la frase su `query_text`, superata da ADR-0012. È una scelta dell'utente, col rimando a `:67`.
- **Righe Modifica attaccate a una tabella** in ADR-0006 (`:66-67`): senza riga vuota dopo l'ultima riga della tabella, in GFM vengono rese come righe della tabella. La riga 66 era già così prima di questo piano; la 67 l'ha aggiunta la ratifica, per restare nel limite di TEST-7.
- **Titolo di ADR-0011** («routing per complessità») e citazione di D-18/D-19 nella riga «Generative LLM» (`DEPENDENCIES.md:89`), mentre la decisione è Sonnet per tutte le query (D-33). Il testo dell'ADR e la Storia lo spiegano, ma il titolo resta fuorviante.
- **Firma della correzione «Stack deciso»** del README alla riga 122, in coda alla sezione Limiti noti, lontana dal paragrafo modificato (`:58-60`). Il paragrafo *Storia* di `:60` non ha data nel formato degli altri.
- **REQUIREMENTS.md:67 e :109:** PLAT-07 è ancora `[ ]` / `Pending`. Lo aggiornano la registrazione di stato o la verifica di fase, non questo bridge.
- **Checklist LLM/RAG** (`threat-model.md:96`): il «rifiuto su evidenza insufficiente di T-18» non corrisponde alla riga T-18, che rifiuta sulla contraddizione delle evidenze.

## Threat Flags

- **T-01-17-01 (ratifica):** mitigato. Checkpoint bloccante con risposta dell'utente datata; TEST-2 verde.
- **T-01-17-02 (regole superate):** mitigato. Quattro Storia; TEST-6 verde.
- **T-01-17-03 (segreti nei documenti):** mitigato. Scanner e grep verdi (TEST-9).
- **T-01-17-04 (codice spike nel prodotto, high):** mitigato. TEST-1 e verify del Task 3 verdi.
- **T-01-17-05 (file malevoli senza AV/CDR, accept):** documentato in ADR-0013, T-39 e Rischio residuo (TEST-10).

## Known Stubs

Nessuno. Le minacce «da implementare» non sono stub: ciascuna indica il test previsto e la fase.

## User Setup Required

Nessuno.

## Punti aperti

- Link ad ADR-0013 nel threat model (T-39, Rischio residuo) e frase su `query_text` di ADR-0006: si correggono con un ritocco documentale, perché i Markdown si possono modificare da qualsiasi fase.
- Rischi residui con responsabile in fase 3 (JBIG2, immagini inline, PUT idempotente), fase 4 (varianza JEV), fase 13 (`instructs_model` su testo ostile), fase 14 (AV/CDR, recall ANN).
- Scelta su come rientrare nel TTQ-parse (D-27), ancora dell'utente (ADR-0004).

## Next Phase Readiness

- **verify della fase 1:** tutti i piani della fase hanno ora SUMMARY. Criterio 5 e PLAT-07 sono chiusi lato documenti.
- **Fondazione e fasi consumatrici:** pianificano contro gli ADR Accepted, contro `DEPENDENCIES.md` e contro le righe «da implementare» del threat model, ciascuna col suo test previsto.

## Self-Check: PASSED

- 14 file su 14 dei `files_modified` in `5cfb581..26bb554`, e nessun altro; correzione del coordinatore a `README.md` nel working tree, nel perimetro; `gate-proprieta-file` conforme.
- Il bridge ha rieseguito dalla radice di `26bb554` il blocco verify del Task 1 (exit 0), il blocco verify del Task 3 (exit 0), TEST-1..10 e TEST-12 (tutti verdi) e TEST-11 a lettura manuale (verde con due annotazioni).
- Tutti i commit citati esistono nel log; tutti i percorsi `spikes/` del threat model e degli ADR 0003-0011 esistono; ADR 0003-0013 tutti emessi.

**Modifica:** Claude (bridge 01-17) · 2026-10-26 · creazione (bridge di 01-17, passi 1-4)
