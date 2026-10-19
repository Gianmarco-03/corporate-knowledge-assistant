<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_16, piano PLAN-010-13, plan_version 3. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 3
  plan_sha256: f3f7bf8c47415617a97eace5e12926d55476916a1ecfab34d266449ff74b7f23
  git_head_sha: 743ff2c4fec0cf85ed8312ec69b57752f8e5390a
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
    category: dependency_identity
    title: Il controllo di famiglia PP-OCRv6 potrebbe non identificare da solo i pesi effettivamente caricati
    scenario: La build seleziona esplicitamente PP-OCRv6, verifica i file e confronta la configurazione risolta a ogni sessione, ma i nomi di famiglia o i campi esposti dalla configurazione possono differire tra PaddleOCR/PaddleX e i modelli realmente caricati.
    trigger: L'API risolve un modello con alias o campi non canonici, oppure il confronto di configurazione omette un path/revision che identifica i pesi in uso.
    failure: La validazione rifiuta una selezione valida oppure, nel caso più rischioso, accetta la famiglia attesa pur senza provare che ciascun file di modello caricato coincida con il manifest approvato.
    impact: Il comportamento OCR e la riproducibilità possono divergere dalla versione PP-OCRv6 prevista; un errore di rilevamento dovrebbe comunque fermarsi in modo sicuro se il gate è implementato come descritto.
    affected:
      - .planning/phases/01-spike-tecnici/01-13-PLAN.md
      - pipeline OCR PaddleOCR/PaddleX
      - manifest dei pesi
    evidence:
      type: plan
      files:
        - .planning/phases/01-spike-tecnici/01-13-PLAN.md
      details: Il piano richiede selezione PP-OCRv6 esplicita, family assertion, configurazione risolta e manifest.sha256, con stop se il modello non è selezionabile; non specifica però in dettaglio la normalizzazione dei campi runtime né l'asserzione di hash sui file effettivamente aperti dal loader.
    mitigation: Normalizzare la configurazione risolta in un formato deterministico e verificare gli hash dei file dal percorso effettivo del loader prima di dichiarare la sessione valida; includere lingua italiana, versione, tier e famiglie det/rec nell'asserzione.
    test_required: true
    test_hint: In un test d'integrazione, confrontare manifest, configurazione risolta e file caricati; alterare un peso, rimuovere un campo di configurazione, fornire un alias inatteso e verificare che ogni caso non possa passare silenziosamente.
```