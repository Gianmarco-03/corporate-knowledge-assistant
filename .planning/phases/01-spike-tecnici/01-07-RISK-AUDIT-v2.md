<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_10, piano PLAN-010-07, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 136aeff3ba3c1fbf0387a52ba0ce6dd2d7bcee87f69b5e7035989c965c9fcd65
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: resource_limits
    title: L’uso di oom_score_adj e subreaper può degradare il confine non-root
    scenario: Il lanciatore imposta budget aggregati, oom_score_adj, killpg e reaping asincrono dentro un worker containerizzato non-root.
    trigger: Il kernel o il runtime rifiuta una delle operazioni oppure il subreaper non è disponibile nel profilo effettivo.
    failure: Il test misura un limite parziale o lascia figli zombie senza distinguere errore di configurazione da OOM del parser.
    impact: Isolamento e telemetria non dimostrati sul runner reale.
    affected: [spikes/s02-parser/src, spikes/docker/worker.Dockerfile, spikes/s02-parser/results/isolation]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-07-PLAN.md], details: 'La v2 prescrive rlimit, budget aggregato, killpg, subreaper, oom_kill_delta e test dedicati, ma non mostra una verifica esplicita dei permessi del container per ciascuna chiamata.'}
    mitigation: Registrare esito e errno di ogni impostazione; se non applicabile, fallire il job e non classificare la misura come isolata.
    test_required: true
    test_hint: Eseguire il caso ostile come uid 10001 e verificare rlimit, cgroup events, reaping e cleanup dopo timeout e OOM.
external_research: []
```

La v2 risolve i rischi precedenti: limite al load lopdf, filtri/object stream, allowlist JSON, ambiente figlio pulito, quote aggregate, cleanup asincrono e TOCTOU residuo dichiarato. L’avvertenza riguarda la prova sul kernel/container effettivo.