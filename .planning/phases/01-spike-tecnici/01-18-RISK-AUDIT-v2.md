<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_18, piano PLAN-010-18, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 80b8b639ec2b8f8c2dff6743b0680cf1a82faa13203c8823e598f5fba3fe672c
  git_head_sha: 4e41406ba027f490b1a9d15179786cd4e093856e
verdict: APPROVED_WITH_WARNINGS
summary:
  critical: 0
  high: 0
  medium: 1
  low: 0
  info: 0
external_research: []
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: resource_pressure
    title: M10 può approvare concorrenza con pressione sostenuta sul limite cgroup
    scenario: I batch reali possono non causare OOM e completare tutti i job, ma raggiungere spesso memory.max, attivare reclaim e degradare latenza o throughput.
    trigger: memory.events:max cresce o il margine tra memory.max e memory.peak si riduce molto durante un batch che altrimenti soddisfa M10.
    failure: Il batch è marcato conforme e la massima concorrenza parse viene pubblicata anche se le allocazioni restano sotto pressione e il kernel deve recuperare memoria per proseguire.
    impact: Le prestazioni possono collassare o diventare instabili con carichi più lunghi o variabili, pur senza oom_kill; il risultato può essere interpretato come un dimensionamento operativo sicuro.
    affected:
      - .planning/phases/01-spike-tecnici/01-18-PLAN.md
      - spikes/s02-parser/results/isolation/budget-real.csv
      - M10
      - concorrenza parse pubblicata per PLAN-010-13
    evidence:
      type: plan
      files:
        - .planning/phases/01-spike-tecnici/01-18-PLAN.md
      details: M10 richiede oom_kill_delta=0, exit 0, nessun job_error, cleanup e rapporto prenotazione/RSS; memory_max_events_delta e margin_kb sono registrati ma dichiarati non soglia. Quindi un batch può risultare conforme anche con eventi max e margine minimo, se il reclaim evita l'OOM.
    mitigation: Definire prima delle misure una soglia di pressione/margine per interpretare memory.events:max e margin_kb, oppure dichiarare esplicitamente che M10 dimostra solo l'assenza di OOM sul campione e non l'idoneità prestazionale della concorrenza; non pubblicare una singola massima concorrenza come sicura se la pressione è elevata.
    test_required: true
    test_hint: Per ciascuna classe e concorrenza, confrontare memory_max_events_delta, margin_kb, latenza e throughput; includere una prova di stress ripetuta e verificare che la regola M10 distingua batch senza pressione da batch che sopravvivono solo grazie al reclaim.
```

Il piano v2 risolve i due problemi principali dell'audit v1: misura il budget anche con job parse reali su classi benign e ostili, limitando esplicitamente le conclusioni ai dati osservati, e aggiunge un test OOM isolato con esito tristato e controlli di victim/reaping/cleanup. La verifica del selftest è correttamente ridotta alla sola aritmetica del limiter. Il residuo riguarda l'interpretazione operativa dei dati di pressione; memory.events e margine vengono raccolti, ma M10 non li usa per distinguere un batch sano da uno sostenuto dal reclaim.