<!-- Risposta verbatim di GPT (OpenAI), content_id deepsearch_5, ricerca aperta dagli audit della fase 1; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Risultato

## Letto

La documentazione ufficiale di Docling descrive il prefetch con `docling-tools models download`; il percorso predefinito è `$HOME/.cache/docling/models`. La CLI supporta `--output-dir`, modelli selettivi e `--all`. Per conversioni offline si può indicare `--artifacts-path`; docling-serve espone `DOCLING_SERVE_ARTIFACTS_PATH`. Fonti: [offline/model prefetch](https://github.com/docling-project/docling/blob/main/docs/usage/advanced_options.md), [CLI reference](https://github.com/docling-project/docling/blob/main/docs/reference/cli.md), [docling-serve configuration](https://github.com/docling-project/docling-serve/blob/main/docs/configuration.md).

Il catalogo ufficiale distingue TableFormer fast e accurate e indica supporto CPU/CUDA, ma il tag Docker `docling-serve-cu128:v1.37.0` o `docling-serve-cpu:v1.37.0` non dimostra da solo che tutti i pesi siano dentro l’immagine. La release v1.37.0 esiste, mentre non ho trovato un manifest ufficiale della specifica immagine che attesti i pesi. Fonti: [model catalog](https://github.com/docling-project/docling/blob/main/docs/usage/model_catalog.md), [releases](https://github.com/docling-project/docling-serve/releases).

## Dedotto

Preparare una directory `/models` con `docling-tools models download`, impostare `DOCLING_SERVE_ARTIFACTS_PATH=/models`, conservare elenco file e SHA-256 dei pesi, quindi avviare con rete esterna negata e cache sola lettura. Verificare una conversione reale con `table_mode=accurate, do_ocr=false`; un tentativo di egress deve fallire chiaramente. Solo questo dimostra disponibilità offline dal primo job.

## Stato

Non è verificato che le immagini v1.37.0 includano già tutti i pesi. Il percorso e la procedura ufficiali di preload sono verificati; la disponibilità va provata sull’immagine digestata.