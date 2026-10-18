<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_14, piano PLAN-010-16, plan_version 2. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 2
  plan_sha256: 7486408a5158c7a1b652af23bda13dd4dcc92525adb2b6d17186113185676669
  git_head_sha: 3ac5650a517360fe9d1f3a3d53969e85c9dc0914
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: authorization_contract
    title: Il confine tra DTO serializzato e tipo autorizzato deve restare non bypassabile
    scenario: La v2 separa document_side, wire e rag_side, con costruttori privati, ricontrollo di subject_id/acl_version/space e Debug redatto.
    trigger: Un adapter futuro deserializza o costruisce direttamente un tipo semantico, oppure usa il DTO prima di authorize_response.
    failure: Un test di compilazione positivo può restare verde mentre il percorso HTTP reale bypassa il ricontrollo.
    impact: Evidenza non autorizzata o di versione/spazio errato passa ai consumatori semantici.
    affected: [spikes/s07-ports/src/lib.rs, docs/adr/0012-firme-porte-candidate.md]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-16-PLAN.md], details: 'La v2 introduce due lati del confine, DTO wire non fidato, costruttori privati, doctest compile_fail e fake positivi; il collegamento all’adapter HTTP futuro resta una condizione della Fondazione.'}
    mitigation: Mantenere authorize_response come unica conversione DTO→tipo autorizzato e aggiungere un test di contratto che attraversi serde e rifiuti bypass/response mismatch.
    test_required: true
    test_hint: Risposta JSON con soggetto, ACL o spazio diversi deve fallire prima di Reranker/AnswerGenerator; il testo non deve apparire in Debug.
external_research: []
```

La v2 risolve i rischi precedenti: scope cache, idempotenza e contesto, EmbeddingSpace completo, confine autorizzativo a due lati, tipi privati e redazione Debug. L’avvertenza riguarda la trascrizione fedele del confine nel futuro adapter HTTP.