<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_21, piano PLAN-010-07, plan_version 5. Trascrizione: Claude (Anthropic), 2026-10-22. -->

```yaml
reviewed:
  task_id: PLAN-010-07
  repository: Gianmarco-03/corporate-knowledge-assistant
  branch: task/PLAN-010-07
  base_sha: 743ff2c4fec0cf85ed8312ec69b57752f8e5390a
  plan_path: .planning/phases/01-spike-tecnici/01-07-PLAN.md
  plan_version: 5
  plan_sha256: bd03d436d460a50597eb702f25202f20430956e2489ba5cc0d612c0b9e88c0f8
  git_head_sha: d0d1113415f65fc26e86ec96d899a23795932cfb
verdict: REVISE
summary: {critical: 0, high: 0, medium: 1, low: 0, info: 0}
risks:
  - id: RISK-001
    severity: MEDIUM
    likelihood: HIGH
    category: interface_contract
    title: I campi del rapporto di prescan non hanno un contratto coerente fra interfaccia, tabella e figlio
    scenario: Task 3 implementa il rapporto JSON del figlio attenendosi alla descrizione locale di parse-child, mentre Task 4 e 01-18 validano i campi nuovi.
    trigger: Il figlio serializza soltanto encryption, embedded_files e objstm_decompressed_bytes come elencato nel punto parse-child; oppure la tabella per objstm-many richiede objstm_count senza dichiararlo nell'interfaccia pubblica.
    failure: Gli esiti ok non contengono xref_streams e decompress_work_bytes benché 01-07 li dichiari additivi in data e 01-18 v4 li richieda per validare I2; inoltre objstm_count è atteso nella tabella dei casi e nel CSV ma non compare fra i campi nominati per PrescanReport al punto 5.
    impact: La verifica di conformità 01-18 non può leggere i dati richiesti, oppure i due piani implementano schemi JSON diversi. Il caso objstm-many può risultare non verificabile e la misura del limite I2 perde il dato necessario.
    affected: [.planning/phases/01-spike-tecnici/01-07-PLAN.md, .planning/phases/01-spike-tecnici/01-18-PLAN.md, spikes/s02-parser/src/prescan.rs, spikes/s02-parser/src/bin/parse-child.rs]
    evidence: {type: plan, files: [.planning/phases/01-spike-tecnici/01-07-PLAN.md], details: 'Al punto 5 di interfaces PrescanReport nomina encryption, embedded_files, objstm_decompressed_bytes, xref_streams e decompress_work_bytes; la tabella objstm-many richiede objstm_count e objstm_decompressed_bytes; la descrizione di parse-child elenca soltanto i primi tre campi in data. Le acceptance criteria verificano i campi in prescan.rs ma non richiedono esplicitamente l emissione di tutti i campi nel JSON del figlio.'}
    mitigation: 'Dichiarare un unico schema PrescanReport/data; aggiungere esplicitamente objstm_count se va esposto; elencare tutti i campi serializzati nella descrizione parse-child e nelle acceptance criteria; aggiungere un test JSON end-to-end per i campi e per la lettura in 01-18.'
    test_required: true
    test_hint: Eseguire parse-child su objstm-many con xref stream e verificare che data contenga encryption, embedded_files, objstm_count, objstm_decompressed_bytes, xref_streams e decompress_work_bytes; confrontare decompress_work_bytes con B in 01-18.
external_research: []
```

L’audit è sul ref esatto `d0d1113415f65fc26e86ec96d899a23795932cfb`; GitHub ha restituito il piano v5 a quel ref. Il checksum dichiarato nella richiesta è riportato sopra. I rilievi precedenti su decompressione, lopdf, password vuota e memoria sono stati affrontati nel testo v5 e nei test proposti; resta la discrepanza interna sullo schema dei dati. La tabella «objstm-many» espone un campo non dichiarato e la descrizione di parse-child omette i due campi additivi richiesti dall’interfaccia e da 01-18.

Verdict: REVISE
