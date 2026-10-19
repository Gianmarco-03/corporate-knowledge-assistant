<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_7, piano PLAN-010-16, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed_plan:
  version: 2
  sha256: 7486408a5158c7a1b652af23bda13dd4dcc92525adb2b6d17186113185676669
reviewed_risk_audit:
  id: risk_audit_14
tests:
  - id: TEST-1
    title: Ogni trait è dyn-compatible e utilizzabile come Arc dyn Trait
    type: COMPATIBILITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-2
    title: Candidato non autorizzato o con ACL obsoleta non attraversa retrieve_authorized
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-3
    title: Serde DTO con subject, ACL o spazio discordante non produce AuthorizedEvidence
    type: INTEGRATION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-4
    title: Tipi semantici non sono costruibili né serializzabili fuori dai moduli autorizzati
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-5
    title: Debug redige testi sentinella su evidenze, claim e risposta strutturata
    type: SECURITY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-6
    title: Cache key e invalidazione isolano soggetto, ACL, versione corpus e spazio embedding
    type: REGRESSION
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-7
    title: Retry dopo OutcomeUnknown con idempotency key applica una sola mutazione
    type: FAILURE
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-8
    title: Embedding a dimensione uguale ma spazio diverso è rifiutato prima della ricerca
    type: BOUNDARY
    priority: HIGH
    catches: [RISK-001]
  - id: TEST-9
    title: I confini dei crate non espongono dipendenze provider o database nelle firme
    type: COMPATIBILITY
    priority: HIGH
    catches: [RISK-001]
```

Prerequisiti: toolchain Rust del workspace, doctest `compile_fail` e test fake. Il test end-to-end usa JSON serializzato fra i due lati del boundary, quindi il DTO va trattato come input non fidato; i fake positivi devono dimostrare che l’evidenza valida passa e quella non valida viene fermata prima delle porte semantiche.