<!-- Risposta verbatim di ChatGPT Deep Research (GPT, OpenAI), content_id deepsearch_6, ricerca aperta da risk_audit_19 (RESEARCH-001, RISK-003) sul piano PLAN-010-07 v4; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-21. -->

# Risultato

Ricerca verificata il 21 ottobre 2026. Per lopdf ho controllato il tag ufficiale **v0.45.0**, rilasciato l’8 settembre 2026 secondo il changelog. Per PDFium ho consultato l’header pubblico upstream sul branch main il 21 ottobre 2026; essendo un branch mobile, la build effettiva del progetto va ricontrollata.

## Ciò che ho letto

- In lopdf v0.45.0, `LoadOptions.max_decompressed_size` è un tetto per **singolo stream**, applicato durante il caricamento a object stream e cross-reference stream. Il changelog indica che il limite di decompressione in caricamento è stato introdotto in v0.44.0. [LoadOptions v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/load_options.rs), [changelog v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/CHANGELOG.md)

- Nel ramo non cifrato, il callback `filter` viene applicato all’oggetto prima della decodifica dell’ObjStm. Se scarta il contenitore ObjStm, il loader non raggiunge il codice che lo decodifica. Il callback è applicato anche ai membri estratti. [reader.rs v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/reader.rs)

- Il loader cifrato riceve il parametro come `_filter_func` e non lo usa. Elabora gli ObjStm con `ObjectStream::new_with_limit`, quindi il limite nativo rimane per stream. In questo ramo, fallimenti nel parsing degli oggetti raw e nella decrittazione sono ignorati; anche l’errore di decodifica dell’ObjStm non viene propagato. Se il loader non riesce ad autenticare e non è configurato per richiedere un errore, può restituire `Ok` senza popolare gli oggetti. [reader.rs v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/reader.rs)

- L’autenticazione lopdf prova prima la password vuota. Se funziona, crea lo stato di decrittazione. `was_encrypted()` controlla la presenza dello stato, mentre `is_encrypted()` controlla la presenza di `/Encrypt` nel trailer. [reader.rs v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/reader.rs), [document.rs v0.45.0](https://github.com/J-F-Liu/lopdf/blob/v0.45.0/src/document.rs)

- L’API pubblica PDFium dice che la password può essere vuota o NULL quando non serve; `FPDF_ERR_PASSWORD` significa password richiesta o errata. Il security handler upstream verifica anche la password user quando la stringa è vuota. Ciò supporta il tentativo con password vuota; la prova conclusiva va eseguita con la build PDFium effettivamente fissata e una fixture con user password vuota. [header pubblico PDFium](https://pdfium.googlesource.com/pdfium/%2B/refs/heads/main/public/fpdfview.h), [security handler PDFium](https://pdfium.googlesource.com/pdfium/%2B/refs/heads/main/core/fpdfapi/parser/cpdf_security_handler.cpp)

## Cosa ne deduco per PLAN-010-07

Le assunzioni del piano sono confermate solo in parte. Il filtro ObjStm può rinviare la decodifica nel percorso non cifrato, ma non protegge il percorso cifrato. lopdf fornisce un tetto per singolo stream, non il contatore aggregato da 512 MiB tra loader e prescan. L’aggregato richiede quindi contabilizzazione applicativa e test che coprano entrambi i percorsi.

Il rischio di perdita silenziosa nel loader cifrato è confermato dal codice: controllare soltanto il risultato di `load_mem_with_options` non basta. Il piano deve confrontare gli oggetti risultanti con xref e fallire in caso di omissioni o errori di decrittazione. Servono test con ObjStm cifrati, errori di decodifica e oggetti mancanti.

La documentazione PDFium è coerente con il tentativo a password vuota e con il rilevamento di errori password, ma da sola non dimostra che la specifica versione distribuita accetti tutte le fixture “permissions-only”. Questa parte richiede un test con la build PDFium bloccata dal repository.

Non ho trovato fonti primarie contraddittorie. Non ho eseguito i test del repository né il runtime PDFium del progetto.

## Esito

La ricerca conferma la necessità del fail-closed e della riconciliazione xref per il loader cifrato. Conferma inoltre che il limite nativo di lopdf è per stream, non aggregato. Restano necessari i test d’integrazione per dimostrare il tetto totale e il comportamento della build PDFium fissata.
