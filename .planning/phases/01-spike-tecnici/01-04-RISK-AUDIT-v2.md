<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_9, piano PLAN-010-04, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 114c5667a117527de1fe38d0a6419fa3d2045be15435933d8b1fe0646ad21293
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: side_channel
    title: La soglia di deduplica può essere applicata dopo il raccolto dei tempi
    scenario: Il benchmark confronta 200 upload nuovi e 200 deduplicati e richiede una soglia laterale predefinita, ma la decisione pratica dipende da rumore di rete, concorrenza e latenza Garage/PG.
    trigger: La differenza supera la soglia per cause di carico o viene normalizzata dopo aver visto i risultati.
    failure: La conclusione sulla deduplica può essere falsamente positiva o negativa.
    impact: Scelta di storage errata e ripetibilità debole della misura.
    affected: [spikes/s01-upload/bench.sh, spikes/s01-upload/RISULTATI.md]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-04-PLAN.md], details: 'La v2 aggiunge pool distinti, warm-up escluso, CSV e soglia scritta prima, ma non fissa un numero di run indipendenti o un criterio statistico per il rumore.'}
    mitigation: Fissare prima numero di run, randomizzazione, criterio di esclusione e intervallo di confidenza; ripetere su sessioni pulite con lo stesso hardware.
    test_required: true
    test_hint: Eseguire più sessioni con ordine alternato new/dedup e verificare che la decisione resti entro la soglia pre-registrata.
external_research: []
```

La v2 chiude i rischi precedenti: limiti e admission control, validazione autorevole, segreti fuori repo, GC con protocollo di adozione, avvio per nome e lettura tempfile fuori dall’handler async. Resta da rendere statisticamente stabile il criterio del canale laterale.