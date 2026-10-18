<!-- Risposta verbatim di GPT (OpenAI), content_id deepsearch_1, ricerca aperta dagli audit della fase 1; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Risultato

## Letto

La documentazione di PaddleOCR espone `ocr_version` con valori `PP-OCRv5`, `PP-OCRv4` e `PP-OCRv3`; la documentazione corrente indica PP-OCRv5 come default della pipeline OCR. `--lang it` seleziona il modello linguistico italiano compatibile con la versione scelta, ma da solo non fissa una revisione immutabile dei pesi. Fonti: [OCR pipeline](https://paddlepaddle.github.io/PaddleOCR/main/en/version3.x/pipeline_usage/OCR.html), [PP-OCRv5 multilingue](https://paddlepaddle.github.io/PaddleOCR/latest/en/version3.x/algorithm/PP-OCRv5/PP-OCRv5_multi_languages.html).

Con `--ocr_version PP-OCRv5` la selezione di famiglia è esplicita. Senza l’opzione, il default documentato è PP-OCRv5 nella documentazione 3.x corrente; non è una prova che ogni installazione 3.7.0 abbia gli stessi default se il codice o i cataloghi vengono cambiati.

## Dedotto

Per il benchmark usare sempre `--ocr_version PP-OCRv5 --lang it` e registrare versione pacchetti, configurazione risolta, URL/repository dei modelli, file e SHA-256 dei pesi. Precaricare i modelli nel processo di build o in una cache controllata, poi eseguire con rete negata. Le fonti consultate non pubblicano un digest unico dei singoli artefatti per la combinazione esatta 3.7.0/3.3.1: quel digest va ricavato dall’artefatto installato e conservato nel manifest.

## Stato

La scelta v5 è verificabile; la revisione/hash dei pesi in una specifica installazione non è deducibile dal solo comando.