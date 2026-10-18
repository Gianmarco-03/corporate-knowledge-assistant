<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_13, piano PLAN-010-13, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: b292b5c0d6ed1620fd80659e5c8b8fce0494699f8ee6819de1d135a655af6d8d
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: benchmark_integrity
    title: Il pin OCR non prova da solo l’identità dei pesi caricati
    scenario: Il piano fissa versione PaddleOCR/PaddlePaddle e richiede pesi precaricati, TTQ con sessioni correlate e manifest hardware.
    trigger: Cache o immagine contengono artefatti scaricati in precedenza oppure il comando risolve un modello diverso da quello atteso.
    failure: La misura OCR/TTQ è ripetibile come comando ma non come artefatto di modello.
    impact: Confronto v5/v6 o CPU/GPU non attribuibile e conclusioni sul parser non riproducibili.
    affected: [spikes/s02-parser, spikes/results, spikes/docker/paddle.Dockerfile]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-13-PLAN.md], details: 'La v2 aggiunge pesi in build, sessione TTQ correlata, manifest e test di isolamento, ma la validazione deve confrontare digest e configurazione risolta con un inventory atteso.'}
    mitigation: Salvare model_id, revision, file SHA-256 e configurazione OCR effettiva; rifiutare cache extra o mancanti e bloccare download durante la misura.
    test_required: true
    test_hint: Build da cache vuota, rete negata, comando esplicito PP-OCRv5 e verifica del manifest prima del primo job.
external_research: []
```

La v2 risolve i rischi precedenti: comando/versione espliciti, pesi offline, coordinate degradate verificate, budget concorrente e correlazione TTQ end-to-end. Resta da dimostrare l’identità crittografica degli artefatti realmente caricati.