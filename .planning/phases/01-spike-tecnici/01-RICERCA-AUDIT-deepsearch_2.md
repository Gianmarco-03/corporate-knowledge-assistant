<!-- Risposta verbatim di GPT (OpenAI), content_id deepsearch_2, ricerca aperta dagli audit della fase 1; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Risultato

## Letto

La documentazione di docling-serve descrive `document_timeout` come timeout di elaborazione per documento e `page_range` come intervallo di pagine da convertire, numerato da 1. Non presenta questi parametri come limite di memoria o garanzia di terminazione del processo figlio. [Docling-serve usage](https://github.com/docling-project/docling-serve/blob/main/docs/usage.md).

La documentazione non dimostra che una disconnessione HTTP uccida il lavoro già in esecuzione né che un timeout HTTP interrompa il parser. Non ho trovato una garanzia che `page_range` impedisca la lettura di tutte le strutture PDF necessarie per aprire e indicizzare il documento.

## Dedotto

Trattare `document_timeout` come scadenza applicativa. Per un confine forte per documento, eseguire ogni conversione in worker/processo isolato con timeout esterno, kill del gruppo di processo e cleanup verificato; un timeout HTTP o la chiusura del client non va interpretata come kill. Usare `page_range` per limitare l’output e misurare comunque memoria/tempo sull’intero job. Verificare con un test che il job venga cancellato e che non restino worker o file temporanei.

## Stato

La semantica del parametro è verificata; isolamento e cancellazione effettiva richiedono una prova del servizio v1.37.0 e non sono garantiti dalla documentazione pubblica consultata.