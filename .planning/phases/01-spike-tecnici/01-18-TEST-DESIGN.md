<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_10, piano PLAN-010-18, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

Disegno dei test per PLAN-010-18 v2, allineato al piano registrato e all'audit `risk_audit_18`.

```yaml
reviewed_plan:
  version: 2
  sha256: 80b8b639ec2b8f8c2dff6743b0680cf1a82faa13203c8823e598f5fba3fe672c
reviewed_risk_audit:
  id: risk_audit_18
tests:
  - id: TEST-1
    title: Soglie M8-M11 e attesi del manifest sono versionati prima dei risultati e non cambiano dopo la misura
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Generatore ostile usa solo standard library, non rete e scrive sotto generated con hash e attesi validi
    type: SECURITY
    priority: HIGH
    catches: []
  - id: TEST-3
    title: Ogni fixture ostile termina tre volte con esito atteso tipizzato entro 5 secondi, parent vivo e zero processi residui
    type: FAILURE
    priority: HIGH
    catches: []
  - id: TEST-4
    title: Prescan dei casi cifrati, object stream, filter chain e contenuti attivi produce i Kind e motivi attesi
    type: SECURITY
    priority: HIGH
    catches: []
  - id: TEST-5
    title: Budget selftest verifica solo la formula del limiter e non alimenta la conclusione sulla concorrenza di produzione
    type: PROPERTY
    priority: HIGH
    catches: []
  - id: TEST-6
    title: Budget reale include tutte le coppie benign/hostile × concorrenza 1/4/8/16 e prova il completamento dei job ammessi
    type: PERFORMANCE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: M10 marca separatamente i batch con memory.events max o margine ridotto e non li descrive come sicuri senza soglia
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Prenotazioni, RSS, p95, memory.peak, memory.events e max_admitted sono calcolati dai dati della sessione e dal profilo limits corretto
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-9
    title: Orchestratore riavvia il worker via cid, cattura exit non zero e snapshot cgroup, valida input e non stampa segreti
    type: FAILURE
    priority: HIGH
    catches: []
  - id: TEST-10
    title: Test OOM dedicato verifica victim figlio, oom_kill_delta, classificazione, parent vivo, reaping e cleanup
    type: CONCURRENCY
    priority: HIGH
    catches: []
  - id: TEST-11
    title: Esito OOM non esercitato o fallito resta distinto da verificato e un file oom.json stantio non passa
    type: FAILURE
    priority: HIGH
    catches: []
  - id: TEST-12
    title: Container OOM usa memoria nota, rete disabilitata, filesystem read-only, uid 10001 e sole capability necessarie
    type: SECURITY
    priority: HIGH
    catches: []
  - id: TEST-13
    title: Controlli di validazione riproducono UnknownOp, out_dir_exists e manifest invalido con gli exit code dichiarati
    type: INTEGRATION
    priority: HIGH
    catches: []
  - id: TEST-14
    title: Validazione di 01-07 confronta CLI, CSV, job_id, Kind, cleanup e codici di uscita con il contratto
    type: COMPATIBILITY
    priority: HIGH
    catches: []
  - id: TEST-15
    title: Overhead misura 20 run nelle modalità child-per-doc e in-process-mutex rispetto alla soglia M9
    type: PERFORMANCE
    priority: MEDIUM
    catches: []
  - id: TEST-16
    title: RISULTATI limita le conclusioni a parse e ai corpus misurati, riporta pressione, OOM e difformità senza correzioni retroattive
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: test del generatore, manifest, soglie, calcoli e controlli contrattuali sul runner locale; Docker Linux per hostile/budget/overhead e per il container OOM. Le misure di CPU devono usare la sessione prenotata del piano. L'esito OOM «non esercitato» è un risultato distinto, non un test positivo della mitigazione.