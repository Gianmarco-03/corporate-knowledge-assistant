<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_1, piano PLAN-010-02, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 2
  sha256: 0cb3f8a78f325730ec3418f7788641ccf4964323688a98c4df0c984dbbb71e2b
reviewed_risk_audit:
  id: risk_audit_8
tests:
  - id: TEST-1
    title: La configurazione Compose risolve i servizi dichiarati e non avvia profili impliciti
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Due sessioni concorrenti non acquisiscono lo stesso lock e reclaim non rimuove container estranei
    type: CONCURRENCY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Ogni sottocomando rifiuta run_id vuoto o malformato prima di invocare Docker
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Runner uid 10001 scrive sui volumi nuovi e dopo ricreazione
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Immagine Docling con inventario incompleto dei modelli non supera il gate offline
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Conversione Docling CPU/GPU funziona con egress negato e cache read-only verificata
    type: E2E
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Scansione di segreti, bind localhost e porte pubblicate
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Gate GPU conserva output e produce verdetto esplicito prima di ogni benchmark GPU
    type: E2E
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: Docker Compose e Docker Desktop/WSL2; GPU NVIDIA per il percorso GPU. Per i test offline servono le immagini Docling fissate, una cache modelli popolata e la possibilità di bloccare egress. Il test dell’inventario deve partire da volume vuoto e includere file mancanti/alterati oltre a una conversione reale.