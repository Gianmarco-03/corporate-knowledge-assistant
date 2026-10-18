<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_12, piano PLAN-010-09, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: f712533d4bcf246dcdbef02d2bb5bbe89691a50526a1174f7f0ed4ee6f278fb3
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: locator_contract
    title: La proiezione cka-text-v1 deve essere immutabile rispetto alle fixture già pubblicate
    scenario: La v2 fissa path con ordinali, hash di sezione, offset scalar→UTF-16 e rettangoli derivati manualmente.
    trigger: Una modifica successiva alle regole di normalizzazione o all’ordine dei titoli riusa fixture e risultati precedenti.
    failure: Round-trip e offset restano verdi pur riferendosi a una proiezione diversa.
    impact: Citazioni non riproducibili tra versioni DOCX/MD e viewer.
    affected: [spikes/s02-parser/locator-gen, spikes/s02-parser/results/locator, docs/adr/0005-locator-citazione.md]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-09-PLAN.md], details: 'La v2 introduce cka-text-v1, section_text_sha256, fixture manuale e commit order, ma il cambio futuro di proiezione richiede ancora un controllo di compatibilità esplicito.'}
    mitigation: Rendere la versione parte obbligatoria di ogni locator e rifiutare fixture con projection/version/hash non coincidenti; aggiungere un test di migrazione negativa.
    test_required: true
    test_hint: 'Modificare una regola di normalizzazione senza cambiare projection: il round-trip deve fallire, non aggiornare silenziosamente i risultati.'
external_research: []
```

La v2 risolve i cinque rischi precedenti: job `locate` allowlisted, path ordinali, hash/versione, UTF-16 verificato con Node, fixture indipendenti e worker avviato per nome. L’avvertenza è sulla compatibilità futura della proiezione.