<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_12, piano PLAN-010-18, plan_version 5, su risk_audit_25. Sostituisce test_design_10 (versione precedente nella storia git di questo file). Trascrizione: Claude (Anthropic), 2026-10-22. -->

# Test design PLAN-010-18 v5

```yaml
reviewed_plan:
  version: 5
  sha256: c373101a0cff3e4f2dbce3e909b66fb2c4cdc927105898c09fbe07275e316a00
reviewed_risk_audit:
  id: risk_audit_25
tests:
  - {id: TEST-1, status: invariato, title: Soglie M8-M11 e attesi manifest versionati prima delle misure, type: REGRESSION, priority: HIGH, catches: []}
  - {id: TEST-2, status: invariato, title: Generatore ostile senza rete e hash validi delle fixture, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-3, status: invariato, title: Fixture ostili ripetute terminano entro 5 secondi con esito atteso e parent vivo, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-4, status: modificato, title: Matrice prescan allineata agli esiti 01-07 v6, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-5, status: invariato, title: Budget selftest verifica limiter ma non concorrenza produzione, type: PROPERTY, priority: HIGH, catches: []}
  - {id: TEST-6, status: invariato, title: Budget reale copre classi benign hostile e concorrenze 1 4 8 16, type: PERFORMANCE, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-7, status: modificato, title: M10 riporta margine senza usarlo come soglia e distingue eventi OOM, type: BOUNDARY, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-8, status: invariato, title: Calcoli prenotazione RSS p95 memory.peak ed eventi dai dati sessione, type: INTEGRATION, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-9, status: invariato, title: Orchestratore gestisce restart worker exit e snapshot cgroup senza segreti, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-10, status: invariato, title: Test OOM verifica figlio vittima padre vivo reaping e cleanup, type: CONCURRENCY, priority: HIGH, catches: []}
  - {id: TEST-11, status: invariato, title: OOM non esercitato o fallito distinto da verificato e artefatto stantio rifiutato, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-12, status: invariato, title: Container OOM con memoria nota rete off filesystem read-only uid 10001, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-13, status: invariato, title: Validazione UnknownOp out_dir_exists e manifest invalido con exit code attesi, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-14, status: modificato, title: Validazione 01-07 confronta CLI CSV job_id Kind cleanup e codici col contratto v6, type: COMPATIBILITY, priority: HIGH, catches: []}
  - {id: TEST-15, status: invariato, title: Overhead misura 20 run per modalità rispetto a M9, type: PERFORMANCE, priority: MEDIUM, catches: []}
  - {id: TEST-16, status: modificato, title: RISULTATI limita conclusioni ai workload misurati e riporta pressione OOM e deviazioni, type: REGRESSION, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-17, status: nuovo, title: Preflight richiede PLAN-010-07 v6 su main con SHA256 esatto, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-18, status: nuovo, title: Preflight objstm-many verifica esito ok e sei campi prima delle misure, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-19, status: nuovo, title: Preflight difettoso interrompe misure e registra deviazione, type: FAILURE, priority: HIGH, catches: []}
```

TEST-17 fallisce chiuso se commit/PLAN su main non corrisponde al pin v6. TEST-18 controlla encryption, embedded_files, objstm_count, objstm_decompressed_bytes, xref_streams e decompress_work_bytes nel job parse objstm-many; se un campo manca nessuna misura hostile o budget parte. TEST-4 verifica i detail della matrice v6 per cifratura, xref, allegati e decompressione. TEST-7/8/16 verificano il warning RISK-001: margine numerico per classe e concorrenza, memory.peak ed events, conclusioni limitate ai workload misurati e ripetizione di M10 quando cambiano workload, memoria o limiti. TEST-10/11 distinguono OOM verificato, fallito e non esercitato; un file risultato precedente non passa.

TEST-1..3, TEST-5/6, TEST-8..13 e TEST-15 invariati rispetto a test_design_10; TEST-4, TEST-7, TEST-14/16 modificati; TEST-17..19 nuovi. Docker Linux e cgroup servono per le misure; pin e schema si controllano nella sessione prima di ogni misura.
