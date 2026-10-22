<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_19, piano PLAN-010-07, plan_version 4. Trascrizione: Claude (Anthropic), 2026-10-21. -->

```yaml
reviewed:
  task_id: PLAN-010-07
  repository: Gianmarco-03/corporate-knowledge-assistant
  branch: task/PLAN-010-07
  base_sha: 743ff2c4fec0cf85ed8312ec69b57752f8e5390a
  plan_path: .planning/phases/01-spike-tecnici/01-07-PLAN.md
  plan_version: 4
  plan_sha256: 5746a7625271093a647128515ae1c8c14eda3f5be555d940b9086c21fe82e1df
  git_head_sha: 66be9b6b4151538d0771c2d62ab67d38c16b5b11
verdict: REVISE
summary: {critical: 0, high: 1, medium: 3, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: HIGH
    likelihood: HIGH
    category: cross_plan_contract
    title: 01-07 v4 contraddice ancora il contratto e i test di 01-18
    scenario: Si implementa 01-07 v4 e si esegue la validazione di fase 01-18 al medesimo head SHA.
    trigger: La matrice degli esiti di 01-18 e D-05 in 01-CONTEXT.md vengono usati senza aggiornamento.
    failure: >-
      encrypted-user/encrypted-owner-only sono attesi come Encrypted, objstm-many come DecompressionLimit e pdf-embedded-file come quarantined: embedded_file; 01-07 v4 invece consente password vuota, accetta ObjStm sotto il limite totale e ignora gli allegati senza estrarli.
    impact: La fase può fallire con implementazione conforme a 01-07 oppure la suite può imporre una politica diversa da quella approvata.
    affected: [.planning/phases/01-spike-tecnici/01-07-PLAN.md, .planning/phases/01-spike-tecnici/01-18-PLAN.md, .planning/phases/01-spike-tecnici/01-CONTEXT.md]
    evidence: {type: repository, files: [.planning/phases/01-spike-tecnici/01-18-PLAN.md, .planning/phases/01-spike-tecnici/01-CONTEXT.md], details: 'Al commit esaminato 01-18 conserva gli esiti indicati e D-05 rifiuta ogni PDF cifrato e mette in quarantena EmbeddedFile, in conflitto con 01-07 v4.'}
    mitigation: Allineare D-05 e gli esiti attesi di 01-18, includere il caso ObjStm sotto soglia aggregata e rieseguire l’audit 01-18.
    test_required: true
    test_hint: Eseguire le fixture encrypted-user, encrypted-owner-only, objstm-many e pdf-embedded-file in entrambi i flussi; confrontare esito, contatori e motivazione rispetto a un contratto unico.
  - id: RISK-002
    severity: MEDIUM
    likelihood: MEDIUM
    category: decompression_accounting
    title: Il limite aggregato di 512 MiB non è dimostrato su tutti i passaggi
    scenario: ObjStm multipli sotto la quota per-stream vengono decodificati nel prescan e nuovamente dal loader.
    trigger: Il ramo non cifrato conta il prescan prima del caricamento reale; il ramo cifrato può espandere gli stream nel loader prima della scansione manuale.
    failure: Il contatore del prescan può non contabilizzare l’output interno del loader o le seconde decodifiche; non è definito se 512 MiB limiti il volume unico o i byte cumulativi prodotti.
    impact: CPU/memoria possono oltrepassare la soglia dichiarata e la garanzia di sicurezza non riflette il consumo effettivo.
    affected: [prescan ObjStm, lopdf loader, scansione post-load degli stream]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-07-PLAN.md], details: 'La v4 descrive prescan e caricamento effettivo separati; nel ramo cifrato il loader espande prima del conteggio manuale senza misurazione esplicita dell’output interno.'}
    mitigation: Definire l’invariante e contare ogni output di decompressione, incluso quello del loader; evitare passaggi non contabilizzati o restringere la garanzia a ciò che si misura.
    test_required: true
    test_hint: Con ObjStm ciascuno sotto 64 MiB e totale oltre 512 MiB, misurare i byte prodotti in ogni fase per entrambi i rami e verificare il limite più un blocco di lettura.
  - id: RISK-003
    severity: MEDIUM
    likelihood: MEDIUM
    category: parser_library_behavior
    title: Le assunzioni su lopdf 0.45.0 non hanno prova sorgente o test mirati
    scenario: Si applica defer_objstm e il limite per-stream, soprattutto nel ramo cifrato.
    trigger: Versione o comportamento del loader differiscono dalle assunzioni su filtri, ObjStm, decrittazione o oggetti scartati.
    failure: Il parser può decomprimere prima del controllo o restituire una struttura parziale non rilevata dalla riconciliazione xref.
    impact: Bypass dei limiti o accettazione/rifiuto incoerente dei PDF.
    affected: [dipendenza lopdf, prescan PDF, riconciliazione xref, apertura PDFium]
    evidence: {type: reasoning, files: [.planning/phases/01-spike-tecnici/01-07-PLAN.md], details: 'La v4 dichiara queste assunzioni ma non include test eseguibili per PDF cifrati con ObjStm, oggetti xref persi o ordine di decompressione della versione risolta.'}
    mitigation: Verificare sorgente e versione bloccata; aggiungere test di caratterizzazione e mantenere fail-closed per discrepanze tra xref e oggetti caricati.
    test_required: true
    test_hint: Con lopdf 0.45.0 risolta, coprire password vuota con ObjStm, xref table/stream, aggiornamento incrementale, oggetto mancante e stream non decodificabile.
  - id: RISK-004
    severity: MEDIUM
    likelihood: LOW
    category: resource_isolation
    title: La prenotazione 3×VmHWM è una stima; l’esito OOM dipende dal sistema
    scenario: Worker concorrenti superano la prenotazione mentre il parent mantiene cache e coda.
    trigger: Un input supera la stima; RLIMIT_AS è per processo e oom_score_adj orienta ma non garantisce la scelta della vittima.
    failure: Il cgroup raggiunge il limite e il kernel termina il parent o un worker diverso da quello previsto.
    impact: Batch interrotto, risultati parziali e cleanup non garantito durante OOM.
    affected: [worker parser, parent orchestrator, cgroup memoria]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-07-PLAN.md, .planning/phases/01-spike-tecnici/01-18-PLAN.md], details: '01-07 dichiara la prenotazione una stima; 01-18 prescrive prove di concorrenza e OOM ma consente di dichiarare il caso non esercitato.'}
    mitigation: Usare prove concorrenti e OOM come gate; controllare sopravvivenza parent, cleanup e il caso in cui oom_score_adj non sia applicabile.
    test_required: true
    test_hint: Aumentare la concorrenza con input realistici e ostili, poi forzare OOM controllato e verificare vittima, parent, gruppo processi e cleanup.
external_research:
  - id: RESEARCH-001
    linked_risk: RISK-003
    question: 'Verificare sul sorgente ufficiale lopdf 0.45.0 ordine di decompressione ObjStm in load_mem_with_options, effetto di defer_objstm, comportamento del loader cifrato e possibili errori/oggetti scartati. Verificare inoltre che PDFium aperto senza password distingua password vuota da password richiesta.'
    blocking: true
```

Audit sul commit esatto richiesto `66be9b6b4151538d0771c2d62ab67d38c16b5b11`; il checksum di 01-07-PLAN.md coincide. Il blocco principale è l’incoerenza con 01-18 e D-05. Inoltre il tetto di 512 MiB non è dimostrato per ogni passaggio di decompressione; le assunzioni su lopdf 0.45.0 richiedono verifica esterna e test.

Verdict: REVISE
