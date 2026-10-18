<!-- Risposta verbatim di GPT (OpenAI), content_id deepsearch_3, ricerca aperta dagli audit della fase 1; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Risultato

## Letto

La documentazione di lopdf 0.45.0 dice che il caricamento gestisce automaticamente gli object stream. `ObjectStream::new` decodifica senza limite; esiste `new_with_limit` per input non fidato. Il changelog 0.45.0 documenta anche il limite di decompressione al caricamento tramite `LoadOptions.max_decompressed_size`. Fonti: [ObjectStream 0.45.0](https://docs.rs/lopdf/0.45.0/lopdf/struct.ObjectStream.html), [changelog 0.45.0](https://docs.rs/crate/lopdf/0.45.0/source/CHANGELOG.md).

La pipeline supporta filtri come FlateDecode e LZWDecode; una quota che conta soltanto gli stream Flate dopo il caricamento non è un tetto sull’espansione fatta durante il load. Filtri concatenati e object stream possono precedere il contatore manuale.

Per `pdfium-render 0.9.4` le fonti documentano binding, rendering e configurazione del rendering, ma non un cap unico sull’intero output di decompressione durante `FPDF_LoadDocument`. [pdfium-render 0.9.4](https://docs.rs/pdfium-render/0.9.4/pdfium_render/).

## Dedotto

Impostare `LoadOptions.max_decompressed_size` prima di `load_mem` e usare `ObjectStream::new_with_limit` per object stream espliciti. Applicare limiti di bytes, oggetti, nesting, pagine e tempo a livello di worker. Pdfium va confinato in un processo con limite di memoria e timeout/kill esterno.

## Stato

Il rischio del caricamento anticipato è confermato. La quota lopdf esiste nella versione indicata; un cap globale affidabile per Pdfium non è stato verificato.