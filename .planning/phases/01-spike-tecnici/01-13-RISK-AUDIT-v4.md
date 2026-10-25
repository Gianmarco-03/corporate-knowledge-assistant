<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_26, piano PLAN-010-13, plan_version 4. Trascrizione: Claude (Anthropic), 2026-10-23. -->

# Audit di rischio PLAN-010-13 v4

```yaml
reviewed:
  task_id: PLAN-010-13
  repository: Gianmarco-03/corporate-knowledge-assistant
  branch: task/PLAN-010-13
  base_sha: b6f36e9
  plan_path: .planning/phases/01-spike-tecnici/01-13-PLAN.md
  plan_version: 4
  plan_sha256: 5b2fadccc400450f4e1f11bdd92562e998d4111cb657646126d35aae436ec257
  git_head_sha: 7e64913c24f78f5ba052d2d1a9eaa59c1125fcd5
verdict: APPROVED_WITH_WARNINGS
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: MEDIUM
    category: resource-limits
    title: La prenotazione basata su input benigni non è un tetto rigido per OCR
    scenario: Un render PDF o una sequenza OCR consuma molta più memoria del VmHWM benigno di calibrazione mentre il job e i suoi discendenti sono attivi.
    trigger: Fixture ostile o documento reale provoca un picco RSS superiore alla stima, oppure coesistono job e output temporanei nel cgroup da 2 GiB.
    failure: Il cgroup può terminare un figlio; il job fallisce e la disponibilità del worker dipende dal contenimento e dal cleanup.
    impact: Perdita del job OCR o pressione sui job concorrenti; una misura su corpus e configurazione specifici non garantisce altri workload.
    affected: [spikes/s02-parser/ocr-run/run_hostile_render.sh, spikes/s02-parser/ocr-run/run_ttq.sh, spikes/s02-parser/results/ocr/hostile-render.csv, spikes/s02-parser/results/ocr/ocr_throughput.csv, docs/adr/0004-parser-ocr-isolamento.md]
    evidence:
      type: plan
      files: [.planning/phases/01-spike-tecnici/01-13-PLAN.md]
      details: La v4 applica la prenotazione 3x VmHWM benigno del contratto 01-07 v6, esplicitamente una stima e non un limite rigido. Le prove hostile verificano padre vivo e zero processi residui; i batch OCR richiedono il metodo M10, inclusi rapporto prenotazione/peak RSS e oom_kill_delta zero.
    mitigation: Conservare per ogni batch i valori per job e cgroup, non estrapolare oltre i workload misurati e dichiarare fallita la conformità se un batch non soddisfa M10; ripetere la misura se cambiano workload, memoria o limiti.
    test_required: true
    test_hint: Eseguire tre ripetizioni per ciascuna fixture hostile senza scartare run; controllare status e kind attesi, RunReport, worker vivo e leftover_procs zero. Per throughput OCR controllare max_admitted, peak_rss_kb, rapporto prenotazione/RSS, oom_kill_delta zero, exit del batch e assenza di job_error.
external_research: []
```

Il commit richiesto è disponibile e il diff corrisponde alla revisione v4. La nuova prova raggiunge il decoder PDFium tramite ocr_render, usa gli hash e gli esiti attesi del manifest 01-18, ripete ogni caso tre volte e rende esplicito quando T-09 è chiuso, aperto o quando il contenimento fallisce. L’allineamento al contratto di budget 01-07 v6 e l’uso dei risultati M10 di 01-18 sono coerenti con i puntatori indicati.

Resta il warning MEDIUM sulla memoria: la prenotazione limita l’ammissione ma non è un limite rigido al consumo residente. La v4 misura il percorso e registra segnali di conformità appropriati, senza trasformare la misura in garanzia per workload differenti.

Verdict: APPROVED_WITH_WARNINGS.

**Modifica:** Claude (Anthropic) · 2026-10-23 · trascrizione verbatim di risk_audit_26 (GPT, OpenAI)
