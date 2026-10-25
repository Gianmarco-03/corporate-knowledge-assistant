<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_9, piano PLAN-010-13, plan_version 3. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 3
  sha256: f3f7bf8c47415617a97eace5e12926d55476916a1ecfab34d266449ff74b7f23
reviewed_risk_audit:
  id: risk_audit_16
tests:
  - id: TEST-1
    title: Gate di legittimità blocca installazione pip finché l’approvazione utente non è presente
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Build seleziona PP-OCRv6 con versione, tier e lingua italiani espliciti e fallisce senza fallback a v5
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Family assertion e configurazione runtime identificano i modelli det/rec effettivamente risolti
    type: COMPATIBILITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Hash dei file caricati corrispondono al manifest e una modifica, omissione o sostituzione viene rifiutata
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Ogni sessione confronta versione, tier, lingua e configurazione normalizzata con l’evidenza di build
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Selezione v6 non disponibile o copertura italiana non dimostrata ferma il piano senza produrre righe v6
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Immagine OCR precarica i pesi, verifica manifest e completa il benchmark senza accesso di rete
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Tesseract CLI produce TSV parola-per-parola con bbox, lingua ita+eng e OMP_THREAD_LIMIT=1
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-9
    title: CER, secondi per pagina e fedeltà bbox coprono scansioni buone e variante degradata con oracolo fissato
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-10
    title: TTQ compone extract_pdf, Docling, render e Tesseract con input hash e run_id comuni
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-11
    title: Batch OCR misura max_admitted sul cgroup da 2 GiB a concorrenza 1, 4, 8 e 16 senza dichiarare osservazioni non misurate
    type: PERFORMANCE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-12
    title: ADR-0004 riporta esplicitamente estrapolazione, condizioni ambientali e risultati dei quattro spike
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: il test 1 richiede approvazione umana prima dell’installazione dei pacchetti; i test d’integrazione/benchmark richiedono Docker, worker senza rete, pesi precaricati e hardware GPU dove previsto. Il benchmark deve fermarsi con anomalia se PP-OCRv6 o l’italiano non risultano selezionabili in modo esplicito.