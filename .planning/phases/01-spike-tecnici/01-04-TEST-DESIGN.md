<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_2, piano PLAN-010-04, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 2
  sha256: 114c5667a117527de1fe38d0a6419fa3d2045be15435933d8b1fe0646ad21293
reviewed_risk_audit:
  id: risk_audit_9
tests:
  - id: TEST-1
    title: Upload streaming mantiene il picco di memoria entro soglia al crescere del corpo
    type: PERFORMANCE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Confini 25 MiB accettati e 25 MiB più un byte rifiutato senza lettura completa
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: File cifrati, XML/ZIP ostili e polyglot sono rifiutati prima dello storage
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Admission control limita upload, validazioni, byte temporanei e descriptor sotto carico
    type: CONCURRENCY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Race fra GC e adozione conserva l’oggetto adottato e rende ripetibile il cleanup
    type: CONCURRENCY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Garage, PG e NullSink producono risultati di correttezza sullo stesso corpus e carichi
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Il protocollo benchmark randomizza l’ordine e valuta la soglia dedup su sessioni indipendenti
    type: PERFORMANCE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Risposte upload deduplicate non rivelano esistenza tramite differenza temporale oltre soglia preregistrata
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-9
    title: Interruzione e crash fra PUT e transazione non lasciano metadati o oggetti incoerenti
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-10
    title: Upload e benchmark non espongono segreti o dipendono da porte non loopback
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: Rust test suite per i test locali; Garage e PostgreSQL/pgvector in Docker per integrazione e benchmark; disco temporaneo monitorabile. Le sessioni del test 7 devono essere indipendenti, con ordine new/dedup alternato e intervallo di confidenza/criterio di esclusione fissato prima di raccogliere i tempi.