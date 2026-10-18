<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_11, piano PLAN-010-08, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 4399af15ba84f6ab9df02a5ead1a96807531ee2a8de87cd3a79863da63f1e85c
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: service_isolation
    title: Il riavvio per documento non dimostra da solo l’assenza di stato esterno
    scenario: La v2 ricrea il container Docling dopo timeout/disconnessione/fallimento e misura CPU e memoria a 0 e 10 secondi.
    trigger: Cache o volume condiviso, job esterno o servizio GPU conserva stato oltre il container ricreato.
    failure: Il documento successivo eredita risorse o dati e la prova di cancellazione viene sovrastimata.
    impact: Misure di isolamento e tempi falsate; possibile consumo cumulativo di memoria.
    affected: [spikes/s02-parser/docling-bench/docling-bench.sh, spikes/compose.yaml, spikes/s02-parser/results/extract/docling_isolation.csv]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-08-PLAN.md], details: 'La v2 richiede stop+up per documento, page_range, timeout/disconnect/outside_range e campioni post-job, ma non richiede verifica di cache/volumi/processi esterni oltre al container.'}
    mitigation: Enumerare volumi, processi e connessioni per run; usare cache in sola lettura e verificare che il nuovo job abbia un identificatore isolato.
    test_required: true
    test_hint: Inserire un marker nel job precedente e verificare che non sia osservabile dal successivo dopo stop/up.
external_research: []
```

La v2 risolve i rischi precedenti: job JSON e prescan, timeout trattato come segnale, riavvio esplicito, soglie e matching pre-registrati, holdout separato e nessun `down` per documento. Resta una verifica operativa sullo stato persistente del servizio.