# ADR-0003 — Storage degli originali PDF/DOCX su Garage

- Stato: Accepted
- Data: 2026-10-21
- Decisori: utente (ratifica nel consolidamento, piano 01-17); proposta di Claude (gsd-executor 01-04) dallo spike S-01 + A-02
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-04) · 2026-10-21 · ADR proposto con le misure dello spike S-01 + A-02
**Modifica:** Claude (Anthropic) · 2026-10-26 · ratificato dall'utente nel piano 01-17 (ratifica-tutti)

## Contesto

Il Document & Indexing Service deve conservare gli originali caricati (PDF, DOCX, Markdown fino a 25 MiB) per il parsing, il re-indexing e il viewer in-app. La riga `DEPENDENCIES.md` «Originali PDF/DOCX» è **Aperta**. Le decisioni della fase 1 (01-CONTEXT) da confermare con i numeri sono:

- **D-01:** object storage S3-compatibile con Garage (AGPLv3, senza versioning: le versioni stanno nel dominio), client `object_store`, dietro la porta `DocumentStore`. MinIO CE è archiviato dal 2026-04-25 e senza immagini da ottobre 2025; RustFS è GA solo dal 2026-09-16.
- **D-02:** chiave = sha256 del contenuto, PUT prima della transazione breve metadati + outbox, GC degli orfani dopo N ore, risposta identica con o senza deduplica.
- **D-03:** upload in streaming su tempfile in un volume Linux, validazione completa prima di storage e DB.
- **D-04:** limite rigido di 25 MiB su tre livelli.
- **D-05:** difese ASVS L1 (limiti DOCX, PDF cifrati rifiutati, PDF attivi in quarantena).
- **D-06:** tre adapter misurati con le stesse misure: Garage, PostgreSQL `bytea` a chunk da 1 MiB, adapter nullo.

Minacce coinvolte (01-RICERCA-DISCUSSIONE §3): **T-05** upload enormi, lenti o concorrenti; **T-06** tipo di file falso e polyglot; **T-07** zip bomb e XML bomb; **T-08** path traversal; **T-11** canale laterale della deduplica; **T-23** accesso diretto allo storage.

## Opzioni considerate

Tutti i numeri vengono dallo spike S-01 + A-02 sullo stesso hardware (i7-13700K, 32 GiB, Docker Desktop su WSL 2), confrontati a parità di dimensione, carico e concorrenza; p95 in ms, run 2 / run 3 della matrice.

1. **Garage + `object_store`.**
   - Vantaggi: ack p95 a 10 MiB `new` di 318,6 / 49,5 ms a c = 1 e 101,6 / 78,9 ms a c = 4 (soglia TTQ ≤ 500 ms), 281,6 / 278,0 ms a c = 16; canale laterale della deduplica entro la soglia pre-registrata in 3 sessioni su 3; range da 64 KiB p95 3,267 ms; nessuna scrittura nel WAL del database.
   - Svantaggi: con la configurazione di default di `object_store` (parti da 10 MiB, fino a 8 in volo) `VmHWM` cresce di 42,6 / 48,1 MiB fra 1 e 25 MiB (soglia 8 MiB); con parti da 2 MiB e una in volo cresce di 1,2 / 1,9 MiB. Nella deduplica concorrente (stesso corpo, c ≥ 4, file ≥ 10 MiB) una parte delle richieste riceve 500: fino a 78 su 200 con il default, 2-7 su 200 a 10 MiB e 0 a 25 MiB con le parti da 2 MiB, per l'abort di multipart concorrenti sulla stessa chiave (`NoSuchUpload`).
   - Rischi: un nuovo servizio da operare; il comportamento dei multipart concorrenti sulla stessa chiave è un'inferenza dai log, non verificata sul codice di Garage.
2. **PostgreSQL `bytea` a chunk da 1 MiB.**
   - Vantaggi: memoria costante (delta `VmHWM` −0,4 / 1,6 MiB), nessun 500 nella deduplica concorrente, range da 64 KiB p95 0,742 ms, nessun servizio in più.
   - Svantaggi: ack p95 a 10 MiB `new` di 1_170,8 / 1_169,3 ms a c = 4 e 4_734,3 / 6_043,7 ms a c = 16; a 25 MiB e c = 16, 12_175,0 / 14_444,0 ms; WAL di 1_123_080 byte per MiB scritto.
   - Rischi: **canale laterale fuori soglia** in 3 sessioni su 3 (la deduplica è più veloce di 32-194 ms perché `ON CONFLICT DO NOTHING` salta la scrittura dei chunk), e ancora in 3 su 3 con la riscrittura dei chunk (`ON CONFLICT DO UPDATE`, intervallo fino a [−191; −38] ms con T = 39,8 ms).
3. **Adapter nullo (baseline).** Solo streaming e hash, nessuna persistenza: p95 a 10 MiB `new` 15,0 / 17,1 ms a c = 1. Non è un'opzione di prodotto; serve a leggere il costo dello storage (Garage aggiunge 42,8 / 32,8 ms di p50 a 10 MiB e c = 1).

## Decisione

Gli originali stanno su **Garage, dietro la porta `DocumentStore`, con il client `object_store`** (D-01 confermata), alle seguenti condizioni:

- **buffer del multipart limitato:** parti da 2 MiB e una parte in volo, l'unica configurazione misurata che supera sia la soglia di memoria sia quella del canale laterale;
- **PUT idempotente sulla chiave di contenuto:** se il PUT di un hash fallisce, l'adapter verifica con un HEAD che `sha256/<hex>` esista con la dimensione attesa e allora lo considera riuscito; questa correzione dei 500 della deduplica concorrente non è stata misurata nello spike e va provata quando si implementa l'adapter, con lo stesso carico `dedup` a c = 16 e con il protocollo del canale laterale;
- **N = 24 h** per il GC degli orfani.

PostgreSQL `bytea` è scartato perché non supera la soglia del canale laterale (T-11), che è un requisito di sicurezza, nemmeno con la mitigazione, e perché sotto concorrenza è 10-20 volte più lento di Garage.

## Conseguenze

- Positive:
  - la memoria del percorso completo è costante rispetto alla dimensione (con il buffer limitato), e il puro streaming lo è comunque (heap dhat +1,03 MiB fra 1 e 25 MiB);
  - la chiave sha256 deduplica fisicamente gli originali e non contiene mai il nome file del client (T-08); lo storage non ha versioning, le versioni stanno nel dominio;
  - il PUT avviene prima della transazione breve metadati + outbox, senza chiamate esterne dentro la transazione; un crash fra PUT e transazione lascia solo un orfano, che il GC raccoglie;
  - il GC con N = 24 h usa il protocollo di adozione `live|deleting` + `pending` con lock di riga e I/O fuori transazione, provato sulle 8 interleaving enumerate (solo quelle, non l'assenza di ogni race);
  - limite di 25 MiB su tre livelli (`RequestBodyLimitLayer`, `DefaultBodyLimit` sulla route, contatore manuale) e admission control (tetto di upload in corso con 503 + `Retry-After`, semaforo di validazione, budget di byte temporanei, deadline di ricezione e di validazione) contro T-05;
  - la validazione (magic bytes, struttura e limiti DOCX, rifiuto di DTD/entità, prefiltro PDF) precede storage e DB (T-06, T-07); il prefiltro PDF è byte-level e il controllo autorevole resta il processo isolato di parsing (S-02).
- Negative:
  - un servizio in più da operare (Garage), con le sue credenziali (T-23: solo rete interna, credenziali da env non versionato, `garage.toml` senza segreti);
  - `with_allow_http(true)` è accettato solo sulla rete Compose locale; il TLS verso lo storage nel prodotto resta alla Fondazione;
  - i 500 della deduplica concorrente sono anche un segnale osservabile della presenza di un upload concorrente dello stesso contenuto (T-11), finché il PUT idempotente non è in opera e misurato;
  - con le parti da 2 MiB la differenza fra deduplica e upload nuovo è negativa in tutte e tre le sessioni (−1,3 … −9,6 ms): entro la tolleranza pre-registrata, ma da rimisurare nel prodotto.
- Follow-up:
  - S-07 (piano 01-16) trascrive in `DocumentStore` le firme dichiarate in RISULTATI §Esito «Impatto su porte»: `put(ContentHash, percorso validato)` idempotente, `get` con range, `delete` solo dal GC, nessun tipo `object_store` in firma;
  - il consolidamento (01-17) porta la riga `DEPENDENCIES.md` «Originali PDF/DOCX» da Aperta a Decisa, aggiorna `docs/security/threat-model.md` per T-05, T-06, T-07, T-08, T-11, T-23 e porta questo ADR ad `Accepted` dopo la ratifica dell'utente;
  - all'avvio il servizio ripulisce i tempfile rimasti da un crash;
  - l'adapter del prodotto misura di nuovo il carico `dedup` a c = 16 e il canale laterale con il PUT idempotente.

## Evidenze

- Spike: [`spikes/s01-upload/RISULTATI.md`](../../spikes/s01-upload/RISULTATI.md) (soglie scritte prima della misura, setup, risultati, anomalie, esito).
- Hardware: [`spikes/s01-upload/results/hw.json`](../../spikes/s01-upload/results/hw.json) (sessione `01-04-20261020t160904z-c0f9`); i manifest di ogni altra sessione stanno in `spikes/s01-upload/results/raw/` (non versionata).
- CSV: `spikes/s01-upload/results/` (`adapters.csv`, `mem_by_size.csv`, `dedup_timing.csv`, `correctness.csv`, `admission.csv`, `storage.csv`, `gc.csv`, `gc_race.csv`, `runs.csv`).

| Misura | Garage (default) | Garage, parti da 2 MiB | PG `bytea` | Nullo | Soglia |
|--------|------------------|------------------------|------------|-------|--------|
| Delta `VmHWM` 25 − 1 MiB (MiB) | 42,6 / 48,1 | 1,2 / 1,9 | −0,4 / 1,6 | 0,8 / 0,4 | < 8 |
| Ack p95 10 MiB `new` c = 1 (ms) | 318,6 / 49,5 | 58,0 / 59,7 | 322,9 / 448,1 | 15,0 / 17,1 | ≤ 500 (Garage) |
| Ack p95 10 MiB `new` c = 4 (ms) | 101,6 / 78,9 | 97,0 / 99,6 | 1_170,8 / 1_169,3 | 17,7 / 17,4 | ≤ 500 (Garage) |
| Canale laterale, sessioni dentro [−T, +T] | 3 su 3 | 3 su 3 | 0 su 3 (0 su 3 anche con riscrittura) | 3 su 3 (controllo) | 3 su 3 |
| 500 nella deduplica, 25 MiB c = 16 (su 200) | 41-78 | 0 | 0 | 0 | — |
| Range 64 KiB p95 (ms) | 3,267 | non misurato | 0,742 | — | ≤ 50 |
| Correttezza | 20 su 20 | — | — | — | 100 % |
