<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_8, piano PLAN-010-02, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 0cb3f8a78f325730ec3418f7788641ccf4964323688a98c4df0c984dbbb71e2b
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: supply_chain
    title: Manifest dei pesi Docling non vincola un inventario atteso
    scenario: Il piano calcola digest delle immagini e SHA-256 di tutti i file presenti nel volume, ma non definisce un elenco atteso di artefatti obbligatori per layout e TableFormer accurate.
    trigger: Un job di preload termina parzialmente o produce una directory con file extra/mancanti e il manifest viene comunque accettato.
    failure: La prova hash può essere ripetibile senza dimostrare che la conversione usi tutti i pesi necessari.
    impact: Misura offline falsa o fallimento ritardato al primo documento.
    affected: [spikes/results/docling-models.txt, spikes/compose.yaml]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-02-PLAN.md], details: 'La v2 prescrive download, elenco SHA-256 e prova positiva/negativa, ma non un inventory atteso con versione e file obbligatori.'}
    mitigation: Registrare repository, revisione e lista attesa dei modelli; validare presenza, digest e caricamento prima di chiudere il gate.
    test_required: true
    test_hint: Cache vuota, preload interrotto e directory con un artefatto mancante devono fallire prima della misura.
external_research: []
```

La v2 risolve i quattro rischi precedenti: profili sostituiti da set di servizi espliciti, volumi inizializzati, preload offline con prova negativa e lock atomico con `run_id` validato. Il residuo è una condizione di completezza del manifest.