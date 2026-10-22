<!-- Risposta verbatim di GPT (OpenAI), content_id test_design_11, piano PLAN-010-07, plan_version 6, su risk_audit_24. Sostituisce test_design_8 (versione precedente nella storia git di questo file). Trascrizione: Claude (Anthropic), 2026-10-22. -->

# Test design PLAN-010-07 v6

```yaml
reviewed_plan:
  version: 6
  sha256: 5c9f942d1ca2ae16ed21792af7ee00868ba27b325561c04c911520b9df9fd5b9
reviewed_risk_audit:
  id: risk_audit_24
tests:
  - {id: TEST-1, status: invariato, title: Job JSON chiuso e allowlist, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-2, status: invariato, title: Percorsi fuori radice symlink TOCTOU e out_dir esistente rifiutati prima dello spawn, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-3, status: invariato, title: Limiti memoria budget aggregato e parametri fuori dominio falliscono chiusi, type: BOUNDARY, priority: HIGH, catches: []}
  - {id: TEST-4, status: invariato, title: Errori setrlimit e setup reaping non classificati come isolamento riuscito, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-5, status: invariato, title: Timeout cancellazione output flood crash e discendenti terminano e vengono raccolti, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-6, status: invariato, title: Esiti tipizzati mantengono job_id e distinguono errore job da failure figlio, type: REGRESSION, priority: HIGH, catches: []}
  - {id: TEST-7, status: modificato, title: Prescan applica le regole PDF aggiornate prima di PDFium, type: SECURITY, priority: HIGH, catches: []}
  - {id: TEST-8, status: modificato, title: Object stream numerosi non rifiutano per conteggio e le quote reali sono rispettate, type: BOUNDARY, priority: HIGH, catches: []}
  - {id: TEST-9, status: invariato, title: Quota output aggregata e cleanup failure non rimuovono risultati validi, type: FAILURE, priority: HIGH, catches: []}
  - {id: TEST-10, status: invariato, title: Limiti calibrati hash e versioni binari e tessdata verificati e consumati, type: INTEGRATION, priority: HIGH, catches: []}
  - {id: TEST-11, status: modificato, title: OOM cgroup lascia vivo il padre e raccoglie il figlio, type: CONCURRENCY, priority: HIGH, catches: [RISK-001]}
  - {id: TEST-12, status: nuovo, title: I1 conta stream distinti una sola volta e si arresta a 512 MiB più un blocco, type: BOUNDARY, priority: HIGH, catches: []}
  - {id: TEST-13, status: nuovo, title: I2 limita il lavoro cumulativo delle passate e riespansioni a B, type: PROPERTY, priority: HIGH, catches: []}
  - {id: TEST-14, status: nuovo, title: Caratterizzazione lopdf 0.45.0 c2-c7 verifica rami cifrati xref revisioni incrementali filtri e corruzione, type: COMPATIBILITY, priority: HIGH, catches: []}
  - {id: TEST-15, status: nuovo, title: parse_child_data_schema verifica i sei campi su PDF objstm-many e DOCX, type: INTEGRATION, priority: HIGH, catches: []}
```

Copertura: TEST-7 prova password vuota e richiesta, voci xref in uso perse, JavaScript/Launch/OpenAction nascosti e allegati contati ma mai estratti. TEST-8 ammette oltre 64 object stream piccoli e rifiuta la somma oltre soglia; nel ramo cifrato verifica quota per stream. TEST-12 copre I1 su entrambi i rami; TEST-13 il limite I2, B = 2 × 512 MiB + 2 × xref_streams × 64 MiB. TEST-14 verifica lopdf 0.45.0 e caratterizza i casi c2-c7, segnalando divergenze senza ritoccare le regole. TEST-15 richiede i sei campi su PDF objstm-many e DOCX, con objstm_count > 64, xref_streams ≥ 1 e DOCX none/0. Calibrazione: input benigni, VmHWM, profilo default e budget aggregato.

TEST-1..6, TEST-9 e TEST-10 invariati rispetto a test_design_8; TEST-7, TEST-8 e TEST-11 modificati; TEST-12..15 nuovi. Docker Linux serve per prescan, calibrazione e OOM; OOM non esercitato va registrato come tale.
