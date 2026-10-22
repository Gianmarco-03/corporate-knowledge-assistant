<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_23, piano PLAN-010-18, plan_version 4. Trascrizione: Claude (Anthropic), 2026-10-22. -->

```yaml
reviewed:
  task_id: PLAN-010-18
  repository: Gianmarco-03/corporate-knowledge-assistant
  branch: task/PLAN-010-18
  base_sha: 3221542
  plan_path: .planning/phases/01-spike-tecnici/01-18-PLAN.md
  plan_version: 4
  plan_sha256: 5f721a4d893903c489639aee6c238e79c1f563194d6889312f6d9c12453ecfb3
  git_head_sha: 0f46860aab7b41e2c28114ae07e9386a4b4d8f7f
verdict: REVISE
summary: {critical: 0, high: 0, medium: 2, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: HIGH
    category: upstream_contract_dependency
    title: La validazione v4 dipende da un’interfaccia di 01-07 v5 che non è descritta in modo uniforme
    scenario: 01-18 esegue Task 4 e valida i campi I2 di ogni risultato ok dopo aver implementato il contratto di 01-07 v5.
    trigger: 01-07 v5 viene implementato secondo la descrizione di parse-child che elenca solo encryption, embedded_files e objstm_decompressed_bytes in data.
    failure: I controlli v4 che richiedono xref_streams e decompress_work_bytes non trovano i campi; in più la tabella upstream per objstm-many richiede objstm_count non definito nel punto PrescanReport.
    impact: La “Validazione di 01-07” fallisce o non può verificare I2 e objstm-many, benché il manifest e la generazione dei casi siano allineati. Un risultato mancante può essere scambiato per regressione del lanciatore downstream.
    affected: [.planning/phases/01-spike-tecnici/01-18-PLAN.md, .planning/phases/01-spike-tecnici/01-07-PLAN.md, spikes/s02-parser/results/isolation/hostile.csv]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-18-PLAN.md, .planning/phases/01-spike-tecnici/01-07-PLAN.md], details: '01-18 v4 richiede data.xref_streams e data.decompress_work_bytes nel controllo (e) e nel confronto della suite; 01-07 v5 dichiara questi campi in <interfaces>, ma il suo paragrafo parse-child ne elenca soltanto tre. risk_audit_21 ha identificato la stessa ambiguità nel piano 01-07 e ha risposto REVISE.'}
    mitigation: Mantenere il gate di esecuzione già dichiarato; dopo la correzione e approvazione della nuova versione 01-07, aggiornare read_first e il pin di versione/hash qui e confermare lo schema JSON condiviso prima della misura.
    test_required: true
    test_hint: Prima di hostile/budget-real, lanciare parse-child su objstm-many e verificare i campi data richiesti da questa versione; fermare l’esecuzione se mancano o se la versione di 01-07 non coincide con il pin approvato.
  - id: RISK-002
    severity: MEDIUM
    likelihood: LOW
    category: resource_budget_measurement
    title: Il margine del cgroup è osservato ma non determina la soglia M10
    scenario: Il batch massimo ammesso termina senza OOM sui carichi misurati, mentre memory.max si avvicina al limite o cresce l’evento max.
    trigger: Un carico futuro differisce dal corpus ostile e benigno o ha un consumo transitorio maggiore durante la concorrenza.
    failure: M10 resta conforme anche con poco margine residuo, perché memory.peak e delta di memory.events:max sono registrati ma non sono soglie.
    impact: La concorrenza pubblicata è valida per i job misurati e non garantisce margine equivalente per input futuri.
    affected: [budget-real.csv, M10, RISULTATI-isolamento.md, concorrenza parse comunicata a 01-13]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-18-PLAN.md], details: 'La v4 registra memory.max, memory.peak, margine e memory.events:max ma dichiara esplicitamente che il margine non è soglia; le conclusioni sono limitate ai job reali e al corpus misurato.'}
    mitigation: Conservare la limitazione delle conclusioni al corpus e alla concorrenza testati; riportare il margine numerico in RISULTATI e ripetere M10 se cambiano workload, memoria del worker o limiti.
    test_required: true
    test_hint: Per ogni classe e concorrenza prevista, confrontare memory.peak, eventi max/oom e rapporto prenotazione/RSS; non pubblicare un livello non misurato come garantito.
external_research: []
```

Audit effettuato sul commit esatto `0f46860aab7b41e2c28114ae07e9386a4b4d8f7f`; il piano recuperato da GitHub è la v4 richiesta e il checksum dichiarato è riportato sopra. Le modifiche v4 alla matrice ostile, ai campi I2, al detail objstm_dropped e al controllo dell’exit 3 risultano allineate al testo v5 di 01-07. Tuttavia il contratto a monte è stato giudicato REVISE in risk_audit_21 per la discrepanza dei campi JSON. Questo piano vincola già l’esecuzione alla presenza su main della versione approvata di 01-07; il pin va aggiornato dopo la revisione. Mantengo anche il warning MEDIUM preesistente: il margine memory.max non è una soglia M10 e la conclusione resta circoscritta ai carichi misurati.

Verdict: REVISE
