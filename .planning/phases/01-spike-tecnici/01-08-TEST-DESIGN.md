<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_4, piano PLAN-010-08, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 2
  sha256: 4399af15ba84f6ab9df02a5ead1a96807531ee2a8de87cd3a79863da63f1e85c
reviewed_risk_audit:
  id: risk_audit_11
tests:
  - id: TEST-1
    title: Job PDF passa solo da prescan e contratto JSON allowlisted
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Timeout e disconnessione rilevano lavoro residuo e ricreano il servizio prima del job seguente
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Probe outside_range confronta page_range con il documento singolo e registra risorse
    type: PERFORMANCE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Cache, processi e mount sono controllati fra due documenti consecutivi
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Tabella IoU assegna ogni parola a un solo match e gestisce merged/missed/spurious
    type: PROPERTY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Euristica pagine-tabella è tarata sul tune e valutata una sola volta sull’holdout
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Header e valori dei CSV corrispondono agli schemi dichiarati
    type: INTEGRATION
    priority: MEDIUM
    catches: [RISK-001]
  - id: TEST-8
    title: Lo script di benchmark riavvia Docling con stop e up senza down né profili
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: Docker per servizio Docling CPU; GPU NVIDIA e gate verde per il percorso GPU. I probe timeout/disconnect/outside_range richiedono i PDF sintetici descritti, metriche container e riavvio del servizio. Il test di stato persistente deve controllare processi e mount effettivi, non assumere che un nuovo container azzeri risorse esterne.