# ADR-0013 — Rinvio di antivirus e CDR sui file caricati alla fase di hardening

- Stato: Accepted
- Data: 2026-10-26
- Decisori: utente (D-05 di `01-CONTEXT.md`; ratifica nel consolidamento, piano 01-17, «ratifica-tutti» del 2026-10-26); proposta di Claude (gsd-executor 01-17)
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-17) · 2026-10-26 · ADR scritto nel consolidamento della fase 1 sul numero riservato W13; stato `Accepted` perché ratificato dall'utente insieme agli ADR 0003-0012 e al threat model

## Contesto

Il Document & Indexing Service riceve PDF, DOCX e Markdown non fidati, caricati dagli utenti o scoperti online e approvati dopo la quarantena. Con D-05 l'utente ha fissato le difese sui file a **ASVS L1 più alcune regole economiche**: limiti sugli archivi DOCX contro le zip bomb (ASVS 5.2.3, che è L2 ma costa poco), quarantena dei PDF con `/JavaScript` o `/Launch`, PDF cifrati ammessi solo con la password utente vuota, allegati incorporati contati e mai estratti, tetti di decompressione nel prescan, parsing in un processo figlio isolato (ADR-0004).

Il livello L2 di ASVS 5.0 chiede in più la **scansione antivirus** dei file caricati (V5.4.3) e l'OWASP File Upload Cheat Sheet raccomanda il **Content Disarm & Reconstruction (CDR)** per PDF e DOCX, cioè la ricostruzione del file con il solo contenuto ritenuto sicuro (fonti in `01-RICERCA-DISCUSSIONE.md`, Area A §3: F3.1 ASVS 5.0 V5 File Handling e F3.2 <https://cheatsheetseries.owasp.org/cheatsheets/File_Upload_Cheat_Sheet.html>, Q4). La minaccia che queste difese coprono è T-39 del threat model: un file malevolo che passa i controlli strutturali e viene servito a chi lo apre.

Il progetto è dimostrativo e lavora su un corpus pubblico o sintetico. La domanda è se aggiungere antivirus e CDR già nell'MVP o rinviarli con un rischio residuo dichiarato.

## Opzioni considerate

1. **ClamAV nell'MVP.** Copre il malware noto per firma, come chiede ASVS V5.4.3. Costa un container in più (circa 1 GB di RAM per le firme, secondo RICERCA Q4), l'aggiornamento continuo delle firme e una nuova superficie d'attacco (il motore di scansione analizza file ostili), senza un corpus reale da proteggere. Non copre il malware nuovo, che è proprio il caso in cui un parser isolato serve di più.
2. **CDR per PDF e DOCX nell'MVP.** Elimina per costruzione il contenuto attivo invece di cercarlo. Richiede un motore di ricostruzione (prodotto commerciale o componente da scrivere) che deve preservare testo, pagine e coordinate su cui si basano chunking e locator delle citazioni (ADR-0005): un CDR che riscrive il PDF cambia l'hash del contenuto e i rettangoli, quindi tocca deduplicazione, versionamento e viewer. È un lavoro di progettazione a sé, non una dipendenza da aggiungere.
3. **Rinvio di antivirus e CDR alla fase di hardening.** Restano le difese L1 misurate negli spike; il rischio residuo si registra nel threat model con responsabile e condizione di riapertura. Nessun servizio in più nell'MVP; il debito è esplicito.

## Decisione

Si sceglie l'**opzione 3**: antivirus e CDR sono **rinviati alla fase 14 (Osservabilità, prestazioni e hardening)**, senza alcuna implementazione nella fase 1 né nelle fasi dell'MVP prima della 14.

Motivazione verificabile:

- le difese L1 che restano sono misurate: 26 casi ostili su 26 contenuti dal lanciatore isolato, padre vivo e 0 processi residui su 78 righe su 78 di `hostile.csv` (piano 01-18, ADR-0004);
- il corpus dell'MVP è pubblico o sintetico, quindi un file malevolo non trovato colpisce un ambiente dimostrativo e non dati reali;
- antivirus e CDR aggiungono servizi e superficie che, senza un corpus reale, non hanno un obiettivo da proteggere (D-05).

Il rinvio decade, e la decisione va riaperta con un nuovo ADR, **prima di caricare documenti reali o di aprire l'upload a utenti esterni** (condizione scritta nel «Rischio residuo» del threat model).

## Conseguenze

- **Positive:** nessun container né motore di scansione in più nell'MVP; la pipeline di ingestion resta quella misurata negli spike; le porte non cambiano.
- **Negative:** il livello L2 di ASVS sui file non è raggiunto; un malware incorporato che supera i controlli strutturali può arrivare a chi scarica l'originale (T-39, stato «rinviata (ADR-0013)» nel threat model).
- **Rischio residuo:** registrato nella sezione «Rischio residuo» di `docs/security/threat-model.md`, con l'utente come responsabile, la fase 14 come destinazione e la condizione di riapertura sopra.
- **Prerequisiti già presenti per aggiungerli dopo:** parsing e OCR in processo separato con limiti (ADR-0004), quarantena dei PDF attivi e delle fonti online, limiti d'archivio DOCX, allegati mai estratti, tetti di decompressione nel prescan. Un passo di scansione o di ricostruzione si inserisce in questo stesso percorso, prima della pubblicazione della versione.
- **Follow-up:** nella fase 14 valutare ClamAV (o un'alternativa) e un CDR per PDF e DOCX, misurando l'effetto del CDR su hash di contenuto, locator e viewer.

## Evidenze

- Threat model: [`docs/security/threat-model.md`](../security/threat-model.md), riga T-39 e sezione «Rischio residuo».
- Difese L1 misurate sui casi ostili: [`spikes/s02-parser/results/isolation/hostile.csv`](../../spikes/s02-parser/results/isolation/hostile.csv) e [`spikes/s02-parser/RISULTATI-isolamento.md`](../../spikes/s02-parser/RISULTATI-isolamento.md) (piano 01-18).
- Parser, isolamento e tetti di decompressione: [ADR-0004](0004-parser-ocr-isolamento.md).
- Fonti normative: `01-RICERCA-DISCUSSIONE.md`, Area A §3 (ASVS 5.0 V5, OWASP File Upload Cheat Sheet, Q4).
