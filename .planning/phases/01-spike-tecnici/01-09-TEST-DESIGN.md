<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_5, piano PLAN-010-09, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 2
  sha256: f712533d4bcf246dcdbef02d2bb5bbe89691a50526a1174f7f0ed4ee6f278fb3
reviewed_risk_audit:
  id: risk_audit_12
tests:
  - id: TEST-1
    title: Round-trip DOCX/MD conserva testo canonico, hash sezione e versione projection
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Titoli omonimi e nesting producono section_path stabile con ordinali
    type: PROPERTY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Offset Unicode scalar convertono in UTF-16 secondo String.slice JavaScript
    type: COMPATIBILITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Locator con projection o hash non corrispondente è rifiutato
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Trasformazione rettangoli PDF copre rotazioni e CropBox spostati
    type: PROPERTY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Fixture attese sono indipendenti e committate prima dei risultati
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Relazioni DOCX esterne e contenuti ostili non vengono risolti dal locator
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Job locate usa out_dir nullo e non può scrivere fuori dalle radici ammesse
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: Rust e test del worker Linux in Docker; Node.js per confronto UTF-16; LibreOffice/PDFium solo se richiesti dai fixture generator del piano. I vettori devono includere caratteri astrali, combinazioni Unicode e offset ai limiti di surrogate pair.