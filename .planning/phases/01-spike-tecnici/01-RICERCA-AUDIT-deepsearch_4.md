<!-- Risposta verbatim di GPT (OpenAI), content_id deepsearch_4, ricerca aperta dagli audit della fase 1; fonti non riverificate. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Risultato

## Letto

Docker Compose abilita un servizio profilato quando almeno uno dei suoi profili coincide con quelli attivi: `profiles: [s02, gpu]` usa quindi l’unione dei profili attivi, non l’intersezione. I servizi senza `profiles` restano sempre abilitati. Più profili si attivano con più `--profile` o `COMPOSE_PROFILES=s02,gpu`. Un servizio mirato esplicitamente può partire senza attivare manualmente il profilo. Fonti: [Using profiles](https://docs.docker.com/compose/how-tos/profiles/), [Compose profiles reference](https://docs.docker.com/reference/compose-file/profiles/).

## Verifica applicabile

Allo SHA indicato il repository non contiene ancora il compose generato, quindi non è possibile eseguire onestamente `docker compose config --services` o `up` sulla matrice reale. La matrice da rendere esplicita deve avere run separate per Docling CPU, Docling GPU, OCR e worker, con servizi core più il solo servizio della run; TEI non deve essere nel set atteso della run Docling. I Dockerfile `worker` e `paddle` non esistono ancora allo SHA analizzato: includerli in `up` prima della consegna causa un errore di build.

## Dedotto

Usare servizi esplicitamente nominati e controllare `config --services` prima dell’avvio. Aggiungere test per ogni combinazione e un assert che TEI non sia avviato. La prova finale deve usare la versione Compose presente sul runner Windows 11/Docker Desktop.