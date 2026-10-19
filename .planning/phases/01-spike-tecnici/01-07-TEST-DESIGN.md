<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_8, piano PLAN-010-07, plan_version 3. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 3
  sha256: 2b69ce35fa9855cb842a6d8fb307d7cfbb1aab63b7e90133c6130f2fdae34ce7
reviewed_risk_audit:
  id: risk_audit_15
tests:
  - id: TEST-1
    title: Job JSON chiuso e allowlist impediscono shell, binari e argv controllati dal chiamante
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Percorsi fuori radice, symlink, TOCTOU controllato e out_dir già esistente sono rifiutati prima dello spawn
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Limiti di memoria, budget aggregato e parametri fuori dominio falliscono chiusi
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Errori setrlimit e subreaper non vengono classificati come isolamento riuscito
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Timeout, cancellazione, output flood, crash e spawn-grandchild terminano e raccolgono tutto il gruppo
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Ogni esito tipizzato mantiene job_id e distingue errore del job da failure del figlio
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Prescan rifiuta cifratura, contenuti attivi nascosti, stream oltre soglia e catene di filtri eccessive prima di PDFium
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Conteggio object stream e limite pagine sono applicati prima del caricamento completo
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-9
    title: Quota output aggregata comprende directory job e out_dir e viene pulita al failure senza rimuovere risultati validi
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-10
    title: Limiti calibrati e hash/versioni dei binari e dei tessdata sono verificati e consumati dal profilo default
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-11
    title: Misura runtime con piano 01-18 conferma UID non privilegiato, oom_adj, reaping e assenza di processi residui
    type: CONCURRENCY
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: Docker Compose Linux e immagine worker; i test unitari/fake-child non richiedono GPU o rete. Il test 11 va eseguito nell’ambiente isolato previsto dal piano 01-18 e con il launcher costruito da questo piano; se il percorso OOM non è attivato, registrarlo come non esercitato, senza dichiararlo verificato.