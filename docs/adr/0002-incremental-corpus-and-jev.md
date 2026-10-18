# ADR-0002 — Corpus incrementale e ruolo di JEV

- Stato: Accepted
- Data: 2026-10-18
- Decisore: progetto portfolio

## Contesto

Il sistema deve partire da pochi documenti e crescere scoprendo fonti online senza trasformare automaticamente contenuti non verificati in conoscenza attendibile.

## Decisione

- Separare knowledge corpus dinamico ed evaluation corpus versionato.
- Tutti i documenti scoperti online entrano in quarantena.
- Usare JEV per query validation, reranking, evidence sufficiency, contradiction detection, citation validation e triage opzionale immagini.
- Non usare JEV per autorizzazione, provenance, licenze, malware scan, deduplicazione esatta o mapping citation.
- Integrare JEV dal backend Rust tramite il contratto HTTP ufficiale, dietro `DecisionEngine`.

## Conseguenze

- Serve un workflow di approvazione e pubblicazione atomica.
- Le soglie JEV devono essere calibrate su dati etichettati.
- JEV non è prerequisito per autenticazione, viewer o RBAC.
- Le immagini restano sperimentali e non bloccanti per l'happy path.
