<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_17, piano PLAN-010-18, plan_version 1. Trascrizione: Claude (Anthropic), 2026-10-18. -->

```yaml
reviewed:
  plan_version: 1
  plan_sha256: 0fff0d9dab5a4a35a1702065bc31715c3e09022ebd8c9d8481f8e98bfecbf927
  git_head_sha: 743ff2c4fec0cf85ed8312ec69b57752f8e5390a
verdict: REVISE
summary:
  critical: 0
  high: 1
  medium: 1
  low: 0
  info: 0
external_research: []
risks:
  - id: RISK-001
    severity: HIGH
    likelihood: MEDIUM
    category: resource_budget
    title: Il selftest sintetico non dimostra che il budget protegga il parser reale
    scenario: Il piano usa allocazioni selftest e prenotazioni pari a un quarto del budget per derivare la concorrenza ammessa, poi usa il risultato per dimensionare i job OCR/parser.
    trigger: I consumi effettivi di PDFium, OCR, decompressione o strutture temporanee superano l'allocazione sintetica dichiarata per job, specialmente con input validi ma ad alta espansione o concorrenza 4/8/16.
    failure: Il limiter ammette più lavori di quanti la memoria reale possa sostenere, oppure blocca lavori validi in modo non previsto; le misure del selftest non rilevano la discrepanza.
    impact: OOM del worker/host, perdita o retry di job, degradazione delle prestazioni e possibile fallimento dell'isolamento che il piano intende misurare.
    affected:
      - .planning/phases/01-spike-tecnici/01-18-PLAN.md
      - budget.csv
      - parse-parent
      - worker OCR
    evidence:
      type: plan
      files:
        - .planning/phases/01-spike-tecnici/01-18-PLAN.md
      details: La matrice budget usa selftest alloc, riserva 0.25 e concorrenze 1/4/8/16; il piano non richiede di calibrare la prenotazione con RSS/peak memory di parsing e OCR reali o con una miscela rappresentativa di documenti. Dichiarare OOM non esercitato quando oom_kill_delta è zero evita una falsa affermazione, ma non valida la concorrenza di produzione.
    mitigation: Prima di usare max_admitted per la concorrenza di produzione, misurare picco RSS e memory.events su workload parser/OCR rappresentativi e avversi; calibrare la prenotazione per classe di lavoro e margine, o limitare esplicitamente la conclusione del selftest alla sola correttezza aritmetica del limiter.
    test_required: true
    test_hint: Eseguire carichi PDF/DOCX rappresentativi e ad alta espansione ai livelli di concorrenza previsti; confrontare memoria per job dichiarata e osservata, verificare il limite cgroup e dimostrare che il massimo ammesso mantiene margine senza OOM.
  - id: RISK-002
    severity: MEDIUM
    likelihood: MEDIUM
    category: failure_validation
    title: Il percorso di selezione del processo da terminare in caso di OOM può restare non verificato
    scenario: I fixture ostili e le allocazioni controllate non provocano necessariamente un OOM reale; il piano prescrive correttamente di segnalare il percorso come non esercitato quando oom_kill_delta è zero.
    trigger: Nessun caso della matrice attiva il kill del cgroup durante la prova.
    failure: Il report passa i controlli degli altri scenari ma non dimostra che oom_score_adj e la scelta del victim proteggano il processo parent in condizioni reali.
    impact: In caso di OOM di produzione il parent potrebbe essere terminato o il cleanup/reaping potrebbe differire dalle attese.
    affected:
      - .planning/phases/01-spike-tecnici/01-18-PLAN.md
      - .planning/phases/01-spike-tecnici/01-07-PLAN.md
      - parse-parent
    evidence:
      type: plan
      files:
        - .planning/phases/01-spike-tecnici/01-18-PLAN.md
        - .planning/phases/01-spike-tecnici/01-07-PLAN.md
      details: Il piano distingue esplicitamente oom_kill_delta=0 come percorso non esercitato, perciò non dichiara una prova che non è avvenuta; non prescrive però una prova isolata e controllata che causi il kill OOM e verifichi victim, reaping e cleanup.
    mitigation: Aggiungere un test OOM controllato in ambiente isolato, con limiti cgroup noti, e richiedere evidenza del victim atteso, parent_alive, reaping e processi residui; mantenere il caso non esercitato come esito distinto se l'ambiente non lo consente.
    test_required: true
    test_hint: Sotto cgroup dedicato, causare intenzionalmente l'oom_kill in un test isolato, verificare oom_kill_delta, identità del victim, parent_alive, stato oom_adj e cleanup completo.
```