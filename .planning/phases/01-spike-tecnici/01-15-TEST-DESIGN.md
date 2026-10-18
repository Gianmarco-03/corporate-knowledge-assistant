# Test design 01-15 — S-04/A-06 baseline di reranking e budget VRAM

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-15-PLAN.md
  sha256: 5bd630b56e34942c91db9d69985435544d730f1048723bad9faa82dc82feb927
risks:
  - id: RISK-1
    severity: CRITICAL
    title: Candidato non visibile al ruolo inviato al reranker TEI o a JEV
    source: threat_model T-01-15-01
  - id: RISK-2
    severity: HIGH
    title: TYPESAFE_API_KEY esposta dal bin rerank o da vram.sh
    source: threat_model T-01-15-04
  - id: RISK-3
    severity: HIGH
    title: Soglie scritte o ritoccate dopo la misura
    source: Task 1 acceptance "Soglie committate prima di ogni file in results/rerank/"
  - id: RISK-4
    severity: HIGH
    title: Mapping index → candidato della risposta /rerank sbagliato, quindi nDCG del reranker TEI falsato e baseline scelta male
    source: must_have "misurati ... con nDCG@5, recall@8 e latenza p95"
  - id: RISK-5
    severity: MEDIUM
    title: File dei candidati manomesso o diverso da quello del manifest; la regola di divergenza del piano contraddice la verifica obbligatoria del bin
    source: threat_model T-01-15-02
  - id: RISK-6
    severity: MEDIUM
    title: Misura VRAM non rappresentativa (servizi non tutti caricati, carico non concorrente, OOM non registrato) o vram.sh che ferma servizi non suoi
    source: threat_model T-01-15-03
  - id: RISK-7
    severity: MEDIUM
    title: Pesi del reranker non quelli registrati (revisione HF ≠ model_sha di /info)
    source: Task 1 action pin dei pesi
  - id: RISK-8
    severity: MEDIUM
    title: Il piano modifica file del piano 01-11 (libreria s04-jev)
    source: key_link "riuso della libreria del piano 01-11 senza modificarla"
  - id: RISK-9
    severity: MEDIUM
    title: Evidenze incomplete (metodi, ripetizioni, scenari VRAM, hw.json, link di RISULTATI e ADR)
    source: must_have artifacts rerank.csv, vram.csv, RISULTATI.md, ADR-0009
  - id: RISK-10
    severity: LOW
    title: URL letterali o http:// nel bin o in vram.sh
    source: Task 1 acceptance
tests:
  - {id: TEST-1, title: rerank_blocks_invisible_candidate_before_tei, type: cargo-integration-locale, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: rerank_blocks_invisible_candidate_before_jev, type: cargo-integration-locale, priority: P0, catches: [RISK-1]}
  - {id: TEST-3, title: Hard gate unauthorized_sent = 0 in rerank.csv, type: check-evidenze, priority: P0, catches: [RISK-1]}
  - {id: TEST-4, title: La chiave non compare in output e artefatti, type: check-statico+runtime, priority: P0, catches: [RISK-2]}
  - {id: TEST-5, title: Soglie committate prima dei risultati e invariate, type: check-git, priority: P0, catches: [RISK-3]}
  - {id: TEST-6, title: tei_rerank_maps_index_to_candidate, type: cargo-unit, priority: P0, catches: [RISK-4]}
  - {id: TEST-7, title: Ricalcolo di nDCG@5 e recall@8 per metodo da rerank.csv, type: check-evidenze, priority: P1, catches: [RISK-4, RISK-9]}
  - {id: TEST-8, title: candidates_sha_mismatch_refused_without_explicit_override, type: cargo-unit, priority: P1, catches: [RISK-5]}
  - {id: TEST-9, title: vram.sh non arresta servizi già attivi, type: check-comportamento, priority: P1, catches: [RISK-6]}
  - {id: TEST-10, title: vram.csv con scenario a tre servizi sotto carico e oom valorizzato, type: check-evidenze, priority: P1, catches: [RISK-6]}
  - {id: TEST-11, title: Revisione del reranker registrata e coincidente con model_sha, type: check-evidenze, priority: P2, catches: [RISK-7]}
  - {id: TEST-12, title: git diff non tocca file del piano 01-11, type: check-git, priority: P1, catches: [RISK-8]}
  - {id: TEST-13, title: Validità delle evidenze e link, type: check-evidenze, priority: P1, catches: [RISK-9]}
  - {id: TEST-14, title: Nessun URL letterale, scanner verde, type: check-statico, priority: P2, catches: [RISK-10]}
uncovered: []
```

## Come leggere questo design

Lo spike confronta tre modi di riordinare i candidati (nessuno, bge-reranker su TEI, JEV) e misura se tre servizi GPU convivono in 6 GB. Il rischio CRITICAL è lo stesso del piano 01-11, applicato a un destinatario in più: il reranker TEI è locale, ma la regola del progetto vale per qualunque reranker, quindi la guardia di visibilità va provata per entrambi gli invii. Il rischio di misura più insidioso è RISK-4: TEI restituisce `[{index, score}]` ordinati per punteggio, e un errore nel ricollegare `index` al candidato originale produce un nDCG plausibile ma sbagliato, che decide la baseline.

C'è un punto del piano da chiarire (RISK-5): la precondizione dice che, se lo sha256 dei candidati rigenerati diverge dal manifest, si registra l'anomalia e si usa il file rigenerato; il bin invece «verifica lo sha256 contro il manifest». Il manifest versionato appartiene al piano 01-10 e non si può aggiornare qui. Senza un'opzione esplicita del bin (per esempio `--accept-sha <hash>` registrato in RISULTATI §Anomalie) l'esecutore si troverebbe con un bin che rifiuta il file o con una verifica che si aggira in silenzio. TEST-8 fissa il comportamento atteso.

Dove vivono i test: moduli `#[cfg(test)]` di `spikes/s04-jev/src/bin/rerank/main.rs` e `tei_rerank.rs`; comando `cd spikes && cargo test -p s04-jev --bin rerank <nome>`. I test «locali» usano server su `127.0.0.1:0` costruiti con `tokio::net::TcpListener` che contano le richieste; per JEV si usa il client della libreria del piano 01-11 con URL base locale (senza modificarla).

## Test

**TEST-1 — `rerank_blocks_invisible_candidate_before_tei`** (in `main.rs`). Insieme di 20 candidati per una query con `role_profile = employee`, uno dei quali di un documento restricted; metodo `tei` con `TEI_RERANK_URL` puntato al server locale. La funzione per-query restituisce errore e il server riceve 0 richieste. È la prova che `assert_visible` sta prima dell'invio a TEI, non solo prima di JEV.

**TEST-2 — `rerank_blocks_invisible_candidate_before_jev`** (in `main.rs`). Stesso insieme, metodo `jev`, URL JEV locale: 0 richieste ricevute, errore restituito.

**TEST-3 — Hard gate.** `awk -F, 'NR>1 && $9!=0' spikes/s04-jev/results/rerank/rerank.csv` non stampa nulla. Le righe `none` hanno anch'esse `unauthorized_sent = 0`: il controllo vale per tutti i metodi.

**TEST-4 — Chiave.** `grep -rn "TYPESAFE_API_KEY" spikes/s04-jev/src/bin spikes/s04-jev/vram.sh` vuoto (la chiave la legge solo la libreria del piano 01-11); `vram.sh` non esegue `set -x` dopo aver caricato `CKA_SPIKES_ENV` e non fa `echo`/`env`/`printenv` delle variabili; a runtime `grep -rqF "$TYPESAFE_API_KEY" spikes/s04-jev docs/adr/0009-*.md` non trova nulla.

**TEST-5 — Soglie prima delle misure.** Il commit che introduce «## Soglie (scritte PRIMA della misura)» in `RISULTATI-rerank.md` è antenato del primo commit che aggiunge file in `results/rerank/`; la sezione è identica a HEAD e contiene: regola della baseline (+0,02 nDCG@5 e p95 ≤ 300 ms), JEV riportato come delta senza soglia di esclusione, VRAM ≤ 5,5 GiB senza OOM con procedura di riduzione del batch, hard gate a 0.

**TEST-6 — `tei_rerank_maps_index_to_candidate`** (in `tei_rerank.rs`). Risposta finta `[{"index": 2, "score": 0.9}, {"index": 0, "score": 0.5}, {"index": 1, "score": 0.1}]` per candidati `[A, B, C]`: l'ordine restituito è `[C, A, B]`. Caso con un `index` fuori intervallo → errore, non panico e non candidato inventato. Caso con meno elementi dei candidati inviati → errore o completamento documentato. Il corpo della richiesta contiene `query`, `texts` nell'ordine dei candidati e `"raw_scores": false`.

**TEST-7 — Ricalcolo delle metriche.** Da `rerank.csv`, media di `ndcg_at_5` e `recall_at_8` per metodo (run 2..3) e p95 di `latency_ms`: coincidono con la tabella di RISULTATI-rerank e la baseline dichiarata rispetta la regola delle Soglie applicata a quei numeri. Il calcolo per-query delle metriche usa la libreria `s04_jev::metrics`, già testata nel piano 01-11.

**TEST-8 — `candidates_sha_mismatch_refused_without_explicit_override`** (in `main.rs`). File di candidati temporaneo con sha diverso da quello del manifest: senza opzione il bin rifiuta con errore che mostra i due hash; con l'opzione esplicita che accetta quell'hash (nome a scelta dell'esecutore) procede e scrive l'hash accettato in un punto che finisce in RISULTATI §Anomalie (log o file di setup). Un'opzione che accetta qualunque hash non è ammessa.

**TEST-9 — `vram.sh` rispetta i servizi altrui.** Prova manuale nel Task 1, prima delle misure: si avvia `tei-embed` a mano, si lancia `vram.sh` in una modalità breve (durata ridotta per parametro o variabile), e alla fine `tei-embed` è ancora `running` mentre `tei-rerank` e `docling` (avviati dallo script) sono fermi. Seconda prova: interruzione con Ctrl-C a metà, la `trap` ferma solo i servizi avviati dallo script. Esito nel SUMMARY.

**TEST-10 — Evidenza VRAM.** `vram.csv` contiene almeno uno scenario in cui `used_mib_tei_embed`, `used_mib_tei_rerank` e `used_mib_docling` sono tutti > 0 nella stessa riga (tre servizi caricati insieme); la colonna `oom` è valorizzata su ogni riga; lo scenario copre ≥ 3 minuti (`t_ms` finale − iniziale ≥ 180000); il massimo di `used_mib_total` riportato in RISULTATI coincide con il CSV; se ci sono righe con `oom = true`, esiste uno scenario successivo con `max_batch_tokens_*` ridotti.

**TEST-11 — Revisione dei pesi.** RISULTATI-rerank §Setup ha lo sha di 40 caratteri esadecimali di `bge-reranker-v2-m3` e lo stesso valore come `model_sha` di `/info`; `hw.json` contiene i digest TEI `86-1.9.4` e `docling-serve-cu128` e `tools.reranker`.

**TEST-12 — Perimetro.** `git diff --name-only <base>..HEAD` del piano non contiene `spikes/s04-jev/src/{lib,jev,candidates,metrics,main}.rs`, `questions/*.json` né `RISULTATI-jev.md`; nessun file di `spikes/s03-pgvector/` è modificato (il piano lo esegue soltanto).

**TEST-13 — Evidenze e link.** Il verify del Task 2 più: `rerank.csv` ha i metodi `none`, `tei`, `jev` per ogni query del set, run 2..3, `candidates = 20`; `model_resolved` valorizzato per `jev`; il gate GPU `VERDETTO: VERDE` era presente (lettura di `spikes/results/gpu-check.txt`); `RISULTATI.md` di S-04 collega `RISULTATI-jev.md`, `RISULTATI-rerank.md` e i due `hw.json`, tutti esistenti; ogni percorso `spikes/...` citato in ADR-0009 esiste.

**TEST-14 — Nessun URL letterale.** `grep -n "http://\|https://" spikes/s04-jev/vram.sh spikes/s04-jev/src/bin/rerank/*.rs` non trova nulla fuori dai test (che usano `127.0.0.1`) e dall'URL di Hugging Face per il pin, che sta in un comando documentato in RISULTATI e non nel codice; scanner exit 0.

## Copertura

Il CRITICAL è coperto per entrambi i destinatari (TEST-1, TEST-2) e nell'evidenza (TEST-3). I HIGH sono coperti. Suggerimento al planner o all'esecutore: rendere esplicita nel piano l'opzione di accettazione dell'hash divergente, oggi implicita nella precondizione.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-15

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 6eb2775af330… a 5bd630b56e34… per la revisione del piano: divergenza dello sha dei candidati accettata solo con --accept-sha <hex> esplicito e registrata in §Anomalie; il manifest del piano 01-10 non si tocca. Il design non è stato rigenerato: va riletto contro la revisione.
