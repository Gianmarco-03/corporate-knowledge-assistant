<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_13, piano PLAN-010-13, plan_version 4. Trascrizione: Claude (Anthropic), 2026-10-23. Il test design precedente (test_design_9, v3) resta in 01-13-TEST-DESIGN-v3.md. -->

# Test design PLAN-010-13 v4

```yaml
reviewed_plan:
  version: 4
  sha256: 5b2fadccc400450f4e1f11bdd92562e998d4111cb657646126d35aae436ec257
reviewed_risk_audit:
  id: risk_audit_26
tests:
  - {id: TEST-1, status: invariato, title: Gate pacchetti blocca installazione senza approvazione, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-2, status: invariato, title: Build seleziona PP-OCRv6 con versione tier e lingua espliciti senza fallback, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-3, status: invariato, title: Family assertion identifica modelli det e rec risolti, type: COMPATIBILITY, priority: HIGH, catches: []}
  - {id: TEST-4, status: invariato, title: Hash pesi caricati coincidono col manifest, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-5, status: invariato, title: Sessione confronta versione tier lingua e configurazione col build, type: REGRESSION, priority: HIGH, catches: []}
  - {id: TEST-6, status: invariato, title: Selezione v6 o copertura italiana non dimostrata ferma il piano, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-7, status: invariato, title: Immagine OCR precarica pesi e benchmark senza rete, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-8, status: invariato, title: Tesseract produce TSV parola per parola bbox ita eng e OMP_THREAD_LIMIT, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-9, status: invariato, title: CER tempo e fedeltà bbox su scansioni buone e degradate, type: REGRESSION, priority: HIGH, catches: []}
  - {id: TEST-10, status: invariato, title: TTQ compone extract_pdf Docling render e Tesseract con hash e run_id comuni, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-11, status: modificato, title: Batch OCR misura max_admitted con cgroup 2 GiB e metodo M10, type: PERFORMANCE, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-12, status: modificato, title: ADR-0004 riporta estrapolazione ambiente e risultati aggiornati, type: REGRESSION, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-13, status: nuovo, title: Render ostile JBIG2 e deep-nesting ripetuto tre volte con contenimento, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-14, status: nuovo, title: Limite immagine verificato quando dimensioni dichiarate e SOF JPEG divergono, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-15, status: nuovo, title: Freschezza M10 e configurazione budget verificate prima di estrapolare concorrenza, type: INTEGRATION, priority: HIGH, catches: [RISK-001]}
uncovered: []
```

TEST-13: rigenerare le fixture offline e confrontare SHA-256 col manifest in sola lettura; eseguire ocr_render tre volte senza scarti. Ogni riga richiede status failed, kind Corrupt o Crashed, oom_adj_set true, RunReport presente, parent terminato normalmente, worker vivo e leftover_procs zero. Un ok lascia aperto T-09; violare il contenimento è bloccante. T-09 JBIG2 si dichiara chiuso solo con 3/3 passaggi di jbig2-malformed.

TEST-11 controlla exit 0, parent vivo, oom_kill_delta zero, nessun job_error, leftover_procs zero e rapporto prenotazione/peak_rss_kb almeno 1 per batch. Registra concorrenza richiesta e max_admitted, senza inferire capacità reale da budget.csv selftest. La prenotazione v6 usa VmHWM per budget residente e VmPeak solo per RLIMIT_AS. Il margine è una stima: conclusioni limitate ai workload misurati.

Continuità: TEST-1..10 invariati rispetto a test_design_9; TEST-11 e TEST-12 aggiornati; TEST-13..15 nuovi. I rischi legacy di identità dei pesi restano coperti da TEST-2..7. Il prompt segnala inoltre due controlli di allineamento prima dell’esecuzione: la chiusura del RISK-16 sulle miniature JPEG (dimensioni dichiarate vs SOF) e la freschezza di M10 dopo la ricalibrazione di main. TEST-14/15 li rendono gate espliciti; finché RISK-16 non è chiuso o la prova non è valida, non dichiarare sicuro il percorso OCR. Docker/Linux serve per render ostile, batch e containment; il gate pacchetti precede ogni installazione.

**Modifica:** Claude (Anthropic) · 2026-10-23 · trascrizione verbatim di test_design_13 (GPT, OpenAI); il precedente test_design_9 rinominato in 01-13-TEST-DESIGN-v3.md
