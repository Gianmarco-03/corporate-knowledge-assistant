# S-01 + A-02 — Upload in streaming e storage degli originali · RISULTATI

**Modifica:** Claude (gsd-executor 01-04) · 2026-10-20 · domanda e soglie scritte prima di qualsiasi misura e di qualsiasi codice dello spike

## Domanda e decisione da chiudere

- **S-01 (criterio 1 della fase):** l'upload multipart su Axum 0.8.9 resta in streaming, con memoria di picco costante rispetto alla dimensione del file, limiti rigidi, validazione completa prima di storage e DB, admission control e latenza di acknowledgment entro il target TTQ?
- **A-02 (criterio 4, parte):** dove stanno gli originali PDF/DOCX. Riga `DEPENDENCIES.md` «Originali PDF/DOCX», oggi **Aperta**.
- Decisioni di 01-CONTEXT da confermare o smentire con i numeri:
  - D-01: Garage dietro la porta `DocumentStore`, client `object_store`;
  - D-02: chiave = sha256 del contenuto, PUT prima della transazione breve metadati+outbox, GC degli orfani dopo N ore, risposta identica con o senza deduplica;
  - D-03: streaming su tempfile in un volume Linux, validazione prima di storage e DB;
  - D-04: limite rigido 25 MiB su tre livelli;
  - D-05: difese ASVS L1 (limiti DOCX, PDF cifrati rifiutati, PDF attivi in quarantena);
  - D-06: tre adapter misurati con le stesse misure (Garage, PG `bytea` a chunk da 1 MiB, adapter nullo).

## Soglie (scritte PRIMA della misura)

Tabella immutabile dopo il commit che la introduce: correzioni solo in «Anomalie e deviazioni».

| Metrica | Soglia | Fonte soglia |
|---------|--------|--------------|
| Puro streaming (corpi artificiali, `--validate off`, `NullSink`), heap dhat | `max_bytes(25 MiB) − max_bytes(1 MiB)` < 2 MiB; 10 MiB registrato | RESEARCH §3 «Misura memoria» [I] |
| Percorso completo con PDF validi da 1, 10, 24, 25 MiB, `VmHWM` del server (processo nuovo per configurazione) | `VmHWM(25 MiB) − VmHWM(1 MiB)` < 8 MiB per ogni adapter | inferenza dichiarata (piano 01-04) |
| `memory.peak` del cgroup del server | registrato senza soglia (la page cache del tempfile è addebitata al cgroup) | RESEARCH §11.4 |
| Ack p95, file da 10 MiB, adapter Garage, carico `new` | ≤ 500 ms a concorrenza 1 e a concorrenza 4 | README TTQ |
| Ack a concorrenza 8 e 16, carico `dedup`, altri adapter e dimensioni | documentati senza soglia | README TTQ |
| Confini di dimensione (byte dell'intero file) | 26_214_399 e 26_214_400 → 2xx; 26_214_401 e 100 MiB → 413 | D-04 |
| Byte letti dal server prima del 413 | ≤ 25 MiB + 64 KiB + un chunk | D-04 |
| Correttezza | 100 %: sha256 del server = `sha256sum` della fixture; magic mismatch e polyglot → 415; DOCX oltre i limiti d'archivio o con DTD/entità → 422; PDF cifrato → 422; PDF con `/JavaScript`, `/Launch`, `/EmbeddedFile` (anche con nomi `#hh`) → `quarantined`; upload interrotto → nessuna riga, nessun oggetto, nessun tempfile; duplicato → un solo oggetto fisico e risposta con la stessa forma | D-02, D-03, D-05 |
| Carico `new` da 200 richieste | 200 sha distinti e 200 oggetti fisici (Garage, PG) | D-02, D-06 |
| Carico `dedup` da 200 richieste | 1 oggetto fisico | D-02 |
| Admission control | validazioni attive massime ≤ `max_validations`; upload in corso ≤ `max_inflight`; oltre il tetto 503 con `Retry-After`; ricezione lenta → 408; entro 15 s dalla fine del carico ostile FD aperti e byte in `/work/tmp` al baseline (±0 file, FD ±4) | T-05, RISK-003 |
| Canale laterale della deduplica | \|p50(dedup) − p50(new)\| ≤ T, con T = max(10 % di p50(new), 20 ms), su 200 richieste per gruppo; protocollo statistico sotto; oltre soglia l'Esito propone e misura una mitigazione | T-11, D-02 |
| Lettura per range da 64 KiB | p95 ≤ 50 ms per adapter | inferenza dichiarata (viewer) |
| WAL per MiB e `pg_total_relation_size` dopo 30 documenti | registrati senza soglia | D-06 |
| GC: list del prefisso `sha256/` + head su 1.000 oggetti | tempo registrato senza soglia; N = 24 h | D-02 (Claude's Discretion) |
| GC contro adozione concorrente | nelle interleaving enumerate di `gc_race` zero righe `upload` con oggetto illeggibile | RISK-004, T-11 |

### Protocollo della misura del canale laterale (RISK-001 di risk_audit_9, TEST-7)

Fissato qui, prima di raccogliere qualsiasi tempo; la decisione si prende solo con questa regola.

- **Configurazione:** file da 10 MiB, concorrenza 1, adapter Garage e PG `bytea` (l'adapter nullo come controllo, senza decisione).
- **Sessioni indipendenti:** 3 sessioni pulite sullo stesso hardware. Sessione pulita = prenotazione `session.sh` nuova (run_id nuovo), container `postgres` e `garage` ricreati, `probe reset` dello storage, processo server nuovo. Fra due sessioni nessun processo del piano resta vivo.
- **Campioni per sessione e adapter:** 200 richieste `new` (200 corpi distinti del pool) e 200 richieste `dedup` (sempre lo stesso corpo, già caricato da un upload di innesco escluso dalle statistiche). Prima del primo blocco 10 richieste `new` e 10 `dedup` di riscaldamento, escluse.
- **Ordine alternato e randomizzato:** 40 blocchi da 10 richieste (20 `new`, 20 `dedup`). I blocchi si accoppiano (un `new` e un `dedup` per coppia) e l'ordine dentro ogni coppia si sceglie con il bit meno significativo di sha256(`s01-dedup` ‖ sessione ‖ coppia): sequenza deterministica, diversa per sessione, registrata nel `req_id` (`s<sessione>-b<blocco>-<i>`) di `dedup_timing.csv`.
- **Criterio di esclusione:** si escludono solo le richieste con stato diverso da 2xx (contate e riportate). Nessuna esclusione in base alla latenza, nessun trimming. Se gli esclusi superano l'1 % di un gruppo, la sessione è invalida e si ripete una volta; una seconda invalidità si registra come anomalia e la sessione conta come non superata.
- **Statistica:** per sessione e adapter d = p50(dedup) − p50(new) sui tempi `time_total` di `curl`; intervallo di confidenza al 95 % di d con bootstrap percentile (2.000 ricampionamenti indipendenti dei due gruppi, PRNG deterministico con seme = numero di sessione), calcolato da `probe aggregate`.
- **Regola di decisione (pre-registrata):** per ogni adapter **supera** se in tutte e 3 le sessioni l'intervallo al 95 % di d sta dentro [−T, +T], con T calcolata sulla p50(new) della stessa sessione. Altrimenti **non supera** (anche quando l'esito è soltanto incerto: la regola è a sicurezza chiusa) e l'Esito propone e misura una mitigazione con lo stesso protocollo.

## Setup

**Modifica:** Claude (gsd-executor 01-04, ripresa 3) · 2026-10-20 · setup e risultati della matrice, della correttezza e del canale laterale

- **Hardware:** [`results/hw.json`](results/hw.json), sha256 `1f650e01c11e87534b8db2580a7c8e5da843bac8d76113e3900c3512be841edd` (manifest della sessione `01-04-20261020t160904z-c0f9`, `git_rev` `34e73f6`). Intel Core i7-13700K (16 core, 24 thread), 32 GiB di RAM, Docker 29.8.2 su WSL 2.6.3 con 16 CPU e 19,5 GiB assegnati a Docker Desktop. Ogni sessione ha il proprio manifest grezzo in `results/raw/hw-<run_id>.raw.json` (cartella ignorata da git).
- **Immagini:** `pgvector/pgvector:0.8.7-pg18-trixie` (`sha256:9d9c9302…cad2e0`), `dxflrs/garage:v2.4.1` (`sha256:9c96caa2…d020`), immagine del runner `sha256:86a924ad…d298` (colonna `runner_image_digest` di `runs.csv`, identica sulle 288 righe). Client di carico `curl` 8.14.1 nel runner, `rustc` 1.95.0.
- **Codice misurato:** commit `34e73f6` per la matrice (288 righe di `runs.csv`, colonna `git_rev`) e per la correttezza; le misure ereditate (canale laterale, storage, GC, test Linux) vengono da `15ea345`, con l'albero del codice identico (`git diff --stat 15ea345 34e73f6` tocca solo `RISULTATI.md`, `gc.csv` e `storage.csv`).
- **Sessioni** (`session.sh`, una prenotazione per sessione, servizi avviati per nome, `down` + `release` a fine sessione con «residui 0»):

| Misura | run_id | Comando dall'host |
|--------|--------|-------------------|
| Test Linux (dhat, pdfgen, admission, gc_race) | `01-04-20261020t113319z-9379` | `bash spikes/s01-upload/bench.sh host tests` |
| Matrice, run 1 (warm-up, ordine crescente) | `01-04-20261020t144205z-c26d` | `bash spikes/s01-upload/bench.sh host matrix 1` |
| Matrice, run 2 (ordine decrescente) | `01-04-20261020t151201z-9164` | `bash spikes/s01-upload/bench.sh host matrix 2` |
| Matrice, run 3 (ordine crescente) | `01-04-20261020t153938z-68e1` | `bash spikes/s01-upload/bench.sh host matrix 3` |
| Correttezza (normale e `--fault after_put`) | `01-04-20261020t160904z-c0f9` | `bash spikes/s01-upload/bench.sh host correctness` |
| Canale laterale, sessioni 1, 2, 3 | `…t114049z-d302`, `…t114328z-ab51`, `…t114531z-b1d8` | `bash spikes/s01-upload/bench.sh host dedup <1\|2\|3>` |
| Storage e GC | `01-04-20261020t114001z-8b12` | `bash spikes/s01-upload/bench.sh host extras` |
| CSV a header fisso e intervalli del canale laterale | nessuna (solo host) | `bash spikes/s01-upload/bench.sh host finalize` |

- **Fixture:** `fixtures.sh` nel runner, in `/work/tmp/fixtures` (mai versionate): pool di 200 PDF validi distinti per 1, 10, 24 e 25 MiB generati da `probe gen-pdf` (spazio libero 876 GiB, quindi pool pieno), tre PDF di confine da 26_214_399/400/401 byte, corpo artificiale da 100 MiB, casi di formato sintetici (nessun PDF o DOCX reale in `spikes/data/corpus/raw` al momento della misura).
- **Reset e stato cold/warm:** prima di ogni riga della matrice `probe reset` svuota `upload`, `outbox`, `blob`, `blob_chunk` e il prefisso `sha256/` di Garage, e parte un processo server nuovo (`server_container_id` distinto per ognuna delle 288 righe). Postgres, Garage e la page cache dell'host non si riavviano fra le righe di una stessa run: lo stato è «warm» per i servizi, «cold» per i dati.
- **Carico:** 200 richieste per riga, `curl -F file=@…` lanciato da `xargs -P <c>`; `new` = 200 corpi distinti, `dedup` = un upload di innesco escluso e poi 200 volte lo stesso corpo. Le statistiche di latenza usano le sole risposte 2xx; i non-2xx sono contati a parte.

## Risultati

### Memoria (criterio 1)

Puro streaming, heap dhat ([`dhat_1m.json`](results/dhat_1m.json), [`dhat_10m.json`](results/dhat_10m.json), [`dhat_25m.json`](results/dhat_25m.json)):

| Dimensione | `max_bytes` | Esito |
|------------|-------------|-------|
| 1 MiB | 1_486_181 | |
| 10 MiB | 2_334_796 | |
| 25 MiB | 2_564_162 | delta 25 − 1 = 1_077_981 byte (1,03 MiB) < 2 MiB: **supera** |

Percorso completo con PDF validi, `VmHWM` del server dopo 200 upload `new` a concorrenza 1, processo nuovo per riga ([`mem_by_size.csv`](results/mem_by_size.csv); run 2 / run 3, la run 1 è il warm-up):

| Adapter | 1 MiB (KiB) | 10 MiB (KiB) | 24 MiB (KiB) | 25 MiB (KiB) | Delta 25 − 1 (MiB) | Esito vs < 8 MiB |
|---------|-------------|--------------|--------------|--------------|--------------------|------------------|
| Garage | 24_952 / 20_392 | 54_224 / 43_096 | 65_368 / 69_952 | 68_544 / 69_656 | 42,6 / 48,1 | **non supera** |
| PG `bytea` | 20_760 / 19_232 | 21_016 / 19_924 | 28_960 / 20_644 | 20_316 / 20_840 | −0,4 / 1,6 | supera |
| nullo | 16_020 / 16_004 | 15_524 / 16_352 | 16_652 / 18_524 | 16_816 / 16_424 | 0,8 / 0,4 | supera |

`memory.peak` del cgroup (registrato senza soglia) a 25 MiB: Garage circa 104 MB, PG `bytea` circa 49 MB, nullo circa 39 MB; la differenza col nullo comprende la page cache del tempfile.

### Latenza di acknowledgment

Soglia TTQ, file da 10 MiB, Garage, carico `new` ([`adapters.csv`](results/adapters.csv), run 2 / run 3):

| Concorrenza | p50 (ms) | p95 (ms) | p99 (ms) | Esito vs p95 ≤ 500 ms |
|-------------|----------|----------|----------|-----------------------|
| 1 | 56,5 / 46,7 | 318,6 / 49,5 | 328,3 / 50,2 | supera |
| 4 | 82,4 / 68,2 | 101,6 / 78,9 | 118,6 / 82,2 | supera |

Confronto fra adapter a parità di carico, file da 10 MiB, p95 in ms (run 2 / run 3; senza soglia oltre la riga sopra):

| Adapter, carico | c = 1 | c = 4 | c = 8 | c = 16 |
|-----------------|-------|-------|-------|--------|
| Garage `new` | 318,6 / 49,5 | 101,6 / 78,9 | 133,3 / 125,0 | 281,6 / 278,0 |
| PG `bytea` `new` | 322,9 / 448,1 | 1_170,8 / 1_169,3 | 2_772,8 / 3_569,7 | 4_734,3 / 6_043,7 |
| nullo `new` | 15,0 / 17,1 | 17,7 / 17,4 | 26,5 / 27,0 | 60,0 / 57,8 |
| Garage `dedup` | 53,4 / 64,5 | 82,0 / 77,9 | 169,2 / 129,0 | 293,7 / 278,5 |
| PG `bytea` `dedup` | 22,6 / 35,5 | 57,4 / 27,5 | 58,8 / 59,8 | 521,5 / 119,2 |
| nullo `dedup` | 15,5 / 15,1 | 17,4 / 17,2 | 28,0 / 25,7 | 57,9 / 56,7 |

File da 25 MiB, carico `new`, p95 in ms (run 2 / run 3): Garage 96,7 / 379,7 (c = 1), 176,9 / 424,2 (c = 4), 805,9 / 972,6 (c = 16); PG `bytea` 1_009,9 / 865,1 (c = 1), 4_955,5 / 4_094,3 (c = 4), 12_175,0 / 14_444,0 (c = 16).

### Scritture effettive

- Carico `new`: in tutte le 96 righe `new` di Garage e PG `bytea` `distinct_sha` = `objects_after` = `req` = 200; l'adapter nullo ha 200 sha distinti e 0 oggetti, come atteso. **Supera.**
- Carico `dedup`: in tutte le 96 righe `dedup` di Garage e PG `bytea` `objects_after` = 1. **Supera.**
- **Errori 500 di Garage sotto deduplica concorrente:** con lo stesso corpo caricato in parallelo, Garage restituisce 500 su una parte delle richieste, in tutte e tre le run: a 10 MiB 2-4 su 200 a c = 4, 6-14 a c = 8 e c = 16; a 24-25 MiB 5-10 a c = 4, 11-19 a c = 8, 54-78 a c = 16. A 1 MiB, a concorrenza 1, nel carico `new` e con PG `bytea` o con l'adapter nullo gli errori sono zero. Vedi «Anomalie e deviazioni».

### Correttezza e confini (D-02..D-05)

[`correctness.csv`](results/correctness.csv), sessione `01-04-20261020t160904z-c0f9`: **20 casi su 20 con `pass` = `true`.**

| Caso | Atteso | Ottenuto | Byte letti prima del rifiuto |
|------|--------|----------|------------------------------|
| 25MiB-1 (26_214_399 byte) | 200 `accepted` | 200 `accepted` | — |
| 25MiB (26_214_400 byte) | 200 `accepted` | 200 `accepted` | — |
| 25MiB+1 (26_214_401 byte) | 413 | 413 | 26_214_401 (soglia 26_345_472) |
| 100MiB | 413 | 413 | 0 (corpo rifiutato dal limite di corpo prima di leggere il campo) |
| polyglot, magic mismatch | 415 | 415 | — |
| zip bomb, DTD, cifrato, troncato | 422 | 422 | — |
| `/JavaScript`, `/J#61vaScript`, `/Launch`, `/EmbeddedFile` | 200 `quarantined` | 200 `quarantined` | — |
| PDF, DOCX e Markdown sintetici | 200 `accepted` | 200 `accepted` | — |
| duplicato | 2 righe, 1 oggetto, stessi campi | 2 righe, 1 oggetto, campi `sha256,size_bytes,status,upload_id`, `upload_id` diversi | — |
| interrotto | nessuna riga, oggetto, tempfile | 0 / 0 / 0 | — |
| crash fra PUT e transazione | nessuna riga; orfano raccolto da `probe gc` con N = 0 | 0 righe, 1 orfano prima del GC, 0 dopo | — |

Nel caso del crash l'`abort` lascia 1 tempfile in `/work/tmp/srv` (colonna `tmpfiles_after`): è il comportamento atteso di un processo terminato senza distruttori, registrato e poi rimosso; nel prodotto la pulizia dei tempfile all'avvio del servizio resta un requisito (vedi Esito).

### Admission control (T-05)

[`admission.csv`](results/admission.csv): con `max_inflight` = 4 e `max_validations` = 2, 12 client lenti e 6 DOCX costosi, metà chiusi a metà: upload attivi massimi 4, validazioni attive massime 1, 14 risposte 503 (tutte con `Retry-After`), 3 risposte 408; FD 13 → 14 (entro ±4), 0 byte in `/work/tmp` dopo 15 s. **Supera.**

### Canale laterale della deduplica (T-11, TEST-7)

[`dedup_timing.csv`](results/dedup_timing.csv), 3.600 righe (3 adapter × 3 sessioni × 400 richieste), 0 esclusioni (nessun non-2xx). Intervallo al 95 % di d = p50(dedup) − p50(new) da `probe dedup-ci` (bootstrap 2.000 ricampionamenti, seme = sessione), regola pre-registrata:

| Adapter | Sessione | p50 new (ms) | p50 dedup (ms) | d (ms) | IC 95 % (ms) | T (ms) | Dentro [−T, +T] |
|---------|----------|--------------|----------------|--------|--------------|--------|-----------------|
| Garage | 1 | 44,638 | 44,808 | 0,170 | [−0,172; 0,582] | 20,000 | sì |
| Garage | 2 | 43,847 | 43,772 | −0,075 | [−0,766; 0,550] | 20,000 | sì |
| Garage | 3 | 45,969 | 45,996 | 0,027 | [−0,340; 0,369] | 20,000 | sì |
| PG `bytea` | 1 | 221,332 | 27,053 | −194,279 | [−253,990; −95,445] | 22,133 | no |
| PG `bytea` | 2 | 66,053 | 21,003 | −45,050 | [−58,071; −35,235] | 20,000 | no |
| PG `bytea` | 3 | 52,250 | 20,380 | −31,870 | [−35,605; −29,790] | 20,000 | no |
| nullo (controllo) | 1 | 13,540 | 13,488 | −0,053 | [−0,181; 0,064] | 20,000 | sì |
| nullo (controllo) | 2 | 13,747 | 13,528 | −0,219 | [−0,340; −0,093] | 20,000 | sì |
| nullo (controllo) | 3 | 13,717 | 13,707 | −0,010 | [−0,141; 0,107] | 20,000 | sì |

Decisione con la regola pre-registrata: **Garage supera** (3 sessioni su 3); **PG `bytea` non supera** (3 sessioni su 3 fuori, la deduplica è più veloce di 30-190 ms perché `ON CONFLICT DO NOTHING` salta la scrittura dei chunk). L'adapter nullo è il controllo e non decide.

### Storage e GC

[`storage.csv`](results/storage.csv) (30 documenti da 10 MiB): lettura per range da 64 KiB p95 Garage 3,267 ms, PG `bytea` 0,742 ms (soglia ≤ 50 ms: **supera** per entrambi); PUT totale Garage 1.017 ms, PG 2.505 ms; WAL di PG 1_123_080 byte per MiB scritto, `pg_total_relation_size` 326_967_296 byte.
[`gc.csv`](results/gc.csv): list del prefisso `sha256/` + head su 1.000 oggetti in 11,5-12,1 ms (head p95 0,30-0,31 ms); 100 orfani di 48 h raccolti con N = 24 h, 100 orfani di 1 h protetti (0 cancellati).
[`gc_race.csv`](results/gc_race.csv): 8 interleaving enumerate (a, b, c, d × Garage, PG) con `unreadable` = 0 e `pass` = `true`. È la prova delle sole interleaving enumerate, non dell'assenza di ogni race.

### Mitigazioni (misurate dopo la matrice, soglie e protocollo invariati)

**Modifica:** Claude (gsd-executor 01-04, ripresa 4) · 2026-10-21 · misure delle configurazioni mitigate di Garage e PG `bytea` e diagnosi dei 500

Le mitigazioni sono opzioni del server con il default uguale alla configurazione della matrice (commit `2bc8a00`): `--garage-part-mib <n> --garage-part-concurrency <k>` fissa la capacità del `BufWriter` di `object_store` (= dimensione di una parte del multipart) e le parti in volo; `--pg-conflict update` fa riscrivere a PG `bytea` i chunk anche quando la chiave esiste già (`ON CONFLICT DO UPDATE`), così la deduplica scrive quanto un upload nuovo. Le righe stanno negli stessi CSV della matrice con un'etichetta propria nella colonna `adapter` (`garage-diag`, `garage-p5k1`, `garage-p2k1`, `pgbytea-update`): nessuna riga della matrice è cambiata, `finalize` ha solo aggiunto righe.

| Misura | run_id | Codice | Comando dall'host |
|--------|--------|--------|-------------------|
| Diagnosi dei 500 + Garage a parti da 5 MiB e da 2 MiB, una parte in volo, 3 run | `01-04-20261020t161935z-303e` | `2bc8a00` | `bash spikes/s01-upload/bench.sh host mitig` |
| Canale laterale mitigato, sessioni 1, 2, 3 (`pgbytea-update`, `garage-p2k1`) | `01-04-20261021t091524z-da48`, `…t092100z-9c72`, `…t092431z-089d` | `83720bb` (sorgenti del server identici a `2bc8a00`) | `bash spikes/s01-upload/bench.sh host dedup-mitig <1\|2\|3>` |

Ogni sessione ha il proprio manifest hardware grezzo in `results/raw/hw-<run_id>.raw.json`, sullo stesso host della matrice; `results/hw.json` resta quello della sessione `c0f9` citato in «Setup».

**Memoria, `VmHWM` del server** dopo 200 upload `new` a c = 1, processo nuovo per riga ([`mem_by_size.csv`](results/mem_by_size.csv); run 2 / run 3, la run 1 è il warm-up come nella matrice):

| Configurazione | 1 MiB (KiB) | 25 MiB (KiB) | Delta 25 − 1 (MiB) | Esito vs < 8 MiB | `memory.peak` a 25 MiB |
|----------------|-------------|--------------|--------------------|------------------|------------------------|
| Garage default (matrice: parti da 10 MiB, fino a 8 in volo) | 24_952 / 20_392 | 68_544 / 69_656 | 42,6 / 48,1 | non supera | circa 104 MB |
| Garage, parti da 5 MiB, 1 in volo (`garage-p5k1`) | 26_656 / 23_604 | 36_304 / 38_572 | 9,4 / 14,6 | **non supera** | circa 60 MB |
| Garage, parti da 2 MiB, 1 in volo (`garage-p2k1`) | 25_240 / 23_660 | 26_460 / 25_608 | 1,2 / 1,9 | **supera** | circa 48 MB |

Anche la run 1 di `garage-p2k1` sta sotto soglia (4,5 MiB). A 10 e 24 MiB `garage-p2k1` resta fra 25_924 e 27_368 KiB.

**Latenza di `garage-p2k1`**, p95 in ms ([`adapters.csv`](results/adapters.csv), run 2 / run 3): 10 MiB `new` c = 1 58,0 / 59,7 e c = 4 97,0 / 99,6 (soglia TTQ ≤ 500 ms: **supera**); 1 MiB c = 1 11,5 / 11,8; 25 MiB `new` c = 1 133,7 / 138,2; `dedup` c = 16 287,3 / 287,5 a 10 MiB e 678,4 / 676,1 a 25 MiB. `new` a 200 richieste: 200 sha e 200 oggetti in ogni riga; `dedup`: 1 oggetto in ogni riga.

**Errori 500 nella deduplica concorrente** (`status_5xx` su 200 richieste a c = 16, run 1 / 2 / 3):

| Configurazione | 10 MiB | 25 MiB |
|----------------|--------|--------|
| Garage default (riga di diagnosi, sessione 303e) | — | 41 (matrice: 54-78) |
| `garage-p5k1` | 6 / 10 / 13 | 0 / 5 / 1 |
| `garage-p2k1` | 7 / 2 / 2 | 0 / 0 / 0 |

La riduzione delle parti riduce gli errori ma non li elimina: la mitigazione della memoria non è una mitigazione dei 500 (diagnosi in «Anomalie e deviazioni»).

**Canale laterale delle configurazioni mitigate** ([`dedup_timing.csv`](results/dedup_timing.csv), etichette `pgbytea-update` e `garage-p2k1`, 2.400 righe in più; protocollo pre-registrato invariato: 3 sessioni pulite, 200 + 200 richieste, blocchi alternati, 0 esclusioni, nessun non-2xx; intervalli da `probe dedup-ci`, bootstrap 2.000, seme = sessione):

| Configurazione | Sessione | p50 new (ms) | p50 dedup (ms) | d (ms) | IC 95 % (ms) | T (ms) | Dentro [−T, +T] |
|----------------|----------|--------------|----------------|--------|--------------|--------|-----------------|
| PG `bytea`, `ON CONFLICT DO UPDATE` | 1 | 397,885 | 287,979 | −109,906 | [−190,997; −38,397] | 39,788 | no |
| PG `bytea`, `ON CONFLICT DO UPDATE` | 2 | 211,210 | 228,100 | 16,890 | [−5,276; 38,662] | 21,121 | no |
| PG `bytea`, `ON CONFLICT DO UPDATE` | 3 | 219,790 | 227,137 | 7,347 | [−33,419; 51,255] | 21,979 | no |
| Garage, parti da 2 MiB, 1 in volo | 1 | 84,472 | 74,914 | −9,558 | [−13,418; −3,590] | 20,000 | sì |
| Garage, parti da 2 MiB, 1 in volo | 2 | 69,110 | 60,573 | −8,537 | [−15,860; −3,069] | 20,000 | sì |
| Garage, parti da 2 MiB, 1 in volo | 3 | 52,325 | 51,025 | −1,300 | [−1,977; −0,298] | 20,000 | sì |

Decisione con la regola pre-registrata: **PG `bytea` mitigato non supera** (0 sessioni su 3: la riscrittura toglie quasi tutto il vantaggio della deduplica, ma la varianza della scrittura dei chunk lascia intervalli più larghi di T; nella sessione 1 la deduplica resta più veloce di 110 ms). **Garage a parti da 2 MiB supera** (3 sessioni su 3). In tutte e tre le sessioni d è negativo (la deduplica è più veloce di 1-10 ms): resta entro la tolleranza pre-registrata, ma è un segnale piccolo e sistematico da rimisurare nel prodotto.

## Anomalie e deviazioni

**Modifica:** Claude (gsd-executor 01-04, ripresa 2) · 2026-10-20 · provenienza delle misure ereditate da un esecutore interrotto

- **Misure ereditate da un esecutore interrotto (errore di rete verso l'API).** Correttezza (sessione `01-04-20261020t113514z-ecb4`), storage e GC (`01-04-20261020t114001z-8b12`) e le tre sessioni del canale laterale (`01-04-20261020t114049z-d302`, `01-04-20261020t114328z-ab51`, `01-04-20261020t114531z-b1d8`) sono state prodotte da `bench.sh host correctness|extras|dedup <n>` con il codice del commit `15ea345` (campo `git_rev` del manifest hardware grezzo di ogni sessione, in `results/raw/`), con l'albero del codice invariato da quel commit. Sono tracciabili e si tengono: ripetere le tre sessioni del canale laterale dopo che i tempi erano già su disco sarebbe una seconda estrazione, vietata dal protocollo pre-registrato (RISK-001).

**Modifica:** Claude (gsd-executor 01-04, ripresa 3) · 2026-10-20 · anomalie della matrice e della correttezza

- **Correttezza ripetuta al commit `34e73f6`.** La sessione ereditata `…t113514z-ecb4` (codice `15ea345`) aveva già dato 20 casi su 20 verdi; la correttezza non è soggetta al protocollo del canale laterale, quindi è stata ripetuta nella sessione `01-04-20261020t160904z-c0f9` per avere tutta la matrice e la correttezza dallo stesso commit. Il CSV committato è quello della ripetizione, con lo stesso esito (copia della prima in `results/raw/correctness-ecb4.tsv`).
- **Il canale laterale resta quello delle sessioni ereditate** (`15ea345`, albero del codice identico a `34e73f6`): 3 sessioni indipendenti come chiede il protocollo, 0 esclusioni, nessuna ripetizione.
- **Matrice eseguita in tre sessioni separate** (una per run) invece che in una sola: ogni run ha la propria prenotazione, il proprio build e i propri servizi ricreati. L'ordine delle dimensioni è crescente, decrescente, crescente come da piano; la run 1 è il warm-up e non entra nelle tabelle.
- **Errori 500 di Garage nel carico `dedup` concorrente** (righe con `status_5xx` > 0 in `adapters.csv`, solo `garage`, solo `dedup`, solo c ≥ 4 e file ≥ 10 MiB). Il server mappa a 500 un errore di `sink.put`, di `gc::adopt_begin` o della transazione dei metadati; PG `bytea` percorre la stessa adozione e la stessa transazione senza errori, quindi l'indiziato è il PUT concorrente della stessa chiave su Garage (sopra 10 MiB `object_store` passa al multipart). Il CSV non distingue il punto di errore: la diagnosi è nella misura aggiuntiva sotto. Le statistiche di latenza di quelle righe usano solo le risposte 2xx.
- **`VmHWM` di Garage cresce con la dimensione** (42,6 e 48,1 MiB di delta fra 25 e 1 MiB, soglia 8 MiB). Causa probabile: `object_store::buffered::BufWriter::new` usa una capacità di 10 MiB e fino a 8 parti in volo, quindi tiene in memoria fino a un intero file da 25 MiB. Il puro streaming (dhat) e gli altri due adapter sono costanti: il limite è nella configurazione dell'adapter Garage, non nella ricezione. Mitigazione proposta e misurata sotto.
- **Istogrammi per fase di `GET /bench/phases`** (file `*.phases.json` in `results/raw/`, non richiesti dal piano): i percentili per fase escono saturati a valori non plausibili (p50 di 2 µs per `receive_hash`), quindi non si usano. Le latenze riportate vengono tutte da `time_total` di `curl`.

**Modifica:** Claude (gsd-executor 01-04, ripresa 4) · 2026-10-21 · diagnosi dei 500, scelta della configurazione mitigata, manifest hardware

- **Diagnosi dei 500 di Garage.** Nella sessione `303e` ogni richiesta 500 del server ha una riga `upload: put fallito` nel log del server (`results/raw/01-04-20261020t161935z-303e/<config>-<size>-dedup-c16-r<n>.log`, ignorati da git): 41 su 41 nella riga di diagnosi, e lo stesso in tutte le righe mitigate. L'errore è sempre lo stesso: la `DELETE …?uploadId=<id>` (abort del multipart) riceve `404 NoSuchUpload`. Nessun errore viene da `gc::adopt_begin` né dalla transazione dei metadati. Lettura: più PUT concorrenti della stessa chiave aprono multipart distinti; quando uno si completa, il multipart di un altro non esiste più per Garage, il suo completamento fallisce, `object_store` tenta l'abort, anche l'abort fallisce con `NoSuchUpload` e l'errore propagato è quello dell'abort (l'errore originale del completamento non arriva nel log). Che Garage chiuda i multipart concorrenti sulla stessa chiave è un'inferenza coerente con i log, non verificata sul codice di Garage. Il corpo e la chiave sono identici, e in tutte le righe `objects_after` = 1 (la leggibilità dell'oggetto dopo i 500 non è stata misurata a parte); il 500 è un falso negativo dell'upload, più un segnale osservabile della presenza di un upload concorrente dello stesso contenuto (vedi «Impatto threat model»). La correzione proposta per S-07 (non misurata nello spike) sta nell'Esito.
- **Configurazione mitigata del canale laterale scelta dopo la misura di memoria.** Lo script preparato dalla ripresa 3 misurava il canale laterale di Garage a parti da 5 MiB; dato che `garage-p5k1` non supera la soglia di `VmHWM`, il commit `83720bb` lo ha spostato su `garage-p2k1`, l'unica configurazione che l'Esito può proporre, prima di raccogliere qualsiasi tempo del canale laterale mitigato. Le soglie, il protocollo e i tempi del canale laterale della matrice non sono cambiati.
- **`results/hw.json`.** Ogni sessione riscrive `results/hw.json` con il proprio manifest; dopo le sessioni di mitigazione il file è stato riportato a quello della sessione `c0f9` (sha256 `1f650e01…1edd`, lo stesso citato in «Setup»), e i manifest delle sessioni `303e`, `da48`, `9c72`, `089d` restano in `results/raw/`: stesso host, stessa configurazione di Docker Desktop.
- **Righe mitigate negli stessi CSV della matrice.** Le righe con etichetta `garage-diag`, `garage-p5k1`, `garage-p2k1` (in `adapters.csv`, `runs.csv`, `mem_by_size.csv`) e `pgbytea-update`, `garage-p2k1` (in `dedup_timing.csv`) non sono righe della matrice: i conteggi del criterio di accettazione (288 righe, 3 adapter × 4 dimensioni × 2 carichi × 4 concorrenze × 3 run) si leggono filtrando `adapter` ∈ {`garage`, `pgbytea`, `null`}. Nessun CSV nuovo fuori dall'elenco del piano.

## Esito

**Modifica:** Claude (gsd-executor 01-04, ripresa 4) · 2026-10-21 · esito di S-01 e A-02

### Esito delle soglie

| Soglia | Esito | Evidenza |
|--------|-------|----------|
| Puro streaming, delta dhat 25 − 1 MiB < 2 MiB | **supera** (1,03 MiB) | `dhat_*.json` |
| Percorso completo, delta `VmHWM` 25 − 1 MiB < 8 MiB | PG `bytea` e nullo **superano**; Garage con la configurazione di default di `object_store` **non supera** (42,6 / 48,1 MiB); Garage a parti da 2 MiB con una parte in volo **supera** (1,2 / 1,9 MiB) | `mem_by_size.csv` |
| Ack p95 ≤ 500 ms, 10 MiB, Garage, `new`, c = 1 e 4 | **supera** con il default (318,6 / 49,5 e 101,6 / 78,9 ms) e con le parti da 2 MiB (58,0 / 59,7 e 97,0 / 99,6 ms) | `adapters.csv` |
| Confini 25 MiB e byte letti prima del 413 | **supera** | `correctness.csv` |
| Correttezza al 100 % | **supera** (20 su 20) | `correctness.csv` |
| `new` 200 → 200 sha e 200 oggetti; `dedup` → 1 oggetto | **supera** in tutte le righe di Garage e PG | `adapters.csv` |
| Admission control | **supera** | `admission.csv` |
| Canale laterale della deduplica | Garage **supera** (default e parti da 2 MiB, 3 sessioni su 3); PG `bytea` **non supera**, né con `ON CONFLICT DO NOTHING` né con la riscrittura dei chunk (0 su 3 in entrambi i casi) | `dedup_timing.csv` |
| Range da 64 KiB p95 ≤ 50 ms | **supera** (Garage 3,267 ms, PG 0,742 ms) | `storage.csv` |
| GC contro adozione concorrente, interleaving enumerate | **supera** (8 su 8) | `gc_race.csv` |

Il criterio 1 della fase è chiuso con una condizione: la memoria del percorso completo è costante rispetto alla dimensione solo se il client S3 ha un buffer di multipart limitato. La configurazione di default di `object_store` (parti da 10 MiB, fino a 8 in volo) tiene in memoria un file intero fino a 25 MiB, e non va usata.

Fuori soglia ma da portare avanti: nella deduplica concorrente (stesso corpo, c ≥ 4, file ≥ 10 MiB) Garage restituisce 500 su una parte delle richieste, fino a 78 su 200 con il default a 25 MiB e c = 16, ancora 2-13 su 200 a 10 MiB con le parti piccole. La causa sta nel multipart concorrente sulla stessa chiave (diagnosi in «Anomalie e deviazioni»).

### Decisione proposta

**Garage dietro la porta `DocumentStore`, client `object_store` (D-01 confermata)**, con tre condizioni che vengono dalle misure:

1. **buffer del multipart limitato:** parti da 2 MiB e una parte in volo (`BufWriter::with_capacity(…, 2 MiB).with_max_concurrency(1)`), cioè la configurazione `garage-p2k1`, l'unica misurata che supera sia la soglia di memoria sia quella del canale laterale; il budget di memoria per upload è dimensione della parte × parti in volo, e resta un parametro dell'adapter, non della porta;
2. **PUT idempotente sulla chiave di contenuto:** quando il PUT di un hash fallisce, l'adapter verifica con un HEAD che l'oggetto `sha256/<hex>` esista con la dimensione attesa e in quel caso considera il PUT riuscito; solo altrimenti l'errore risale. È la correzione proposta per i 500 della deduplica concorrente: non è stata misurata nello spike e va provata (con lo stesso carico `dedup` a c = 16 e con il canale laterale) dove si implementa l'adapter;
3. **N = 24 h** per il GC degli orfani, con il protocollo di adozione `live|deleting` + `pending` provato sulle interleaving enumerate.

### Alternative scartate

Confronti solo a parità di dimensione, carico e concorrenza (run 2 / run 3).

- **PG `bytea` a chunk da 1 MiB (D-06):** scartato per il canale laterale e per la latenza sotto concorrenza. Canale laterale: non supera in 3 sessioni su 3 con `ON CONFLICT DO NOTHING` (la deduplica è più veloce di 32-194 ms) e in 3 su 3 anche con la riscrittura dei chunk (intervalli fino a [−191; −38] ms con T = 39,8 ms). Latenza, 10 MiB `new`, p95: 1_170,8 / 1_169,3 ms a c = 4 contro 101,6 / 78,9 ms di Garage; 4_734,3 / 6_043,7 ms a c = 16 contro 281,6 / 278,0 ms; a 25 MiB e c = 16, 12_175,0 / 14_444,0 ms contro 805,9 / 972,6 ms. WAL di 1_123_080 byte per MiB scritto: ogni originale passa due volte sul disco del database. A suo favore restano la memoria costante, nessun 500 nella deduplica concorrente e la lettura per range più veloce (0,742 contro 3,267 ms di p95, entrambe molto sotto la soglia di 50 ms): non bastano a compensare il canale laterale, che è un requisito di sicurezza (T-11).
- **Adapter nullo:** è la baseline dello streaming e dell'hash (p95 a 10 MiB `new` 15,0 / 17,1 ms a c = 1), non persiste nulla e non è un'opzione di prodotto. Serve a leggere i costi: a 10 MiB e c = 1 lo storage su Garage aggiunge 42,8 / 32,8 ms di p50 (56,5 / 46,7 contro 13,7 / 13,9 ms).
- **MinIO CE e RustFS** non sono stati misurati: MinIO CE è archiviato (2026-04-25) e RustFS è GA solo dal 2026-09-16 (01-CONTEXT, D-01).

### Impatto su porte (S-07)

Da trascrivere nelle firme candidate di `spikes/s07-ports/src/lib.rs` (proprietario 01-16, WIRING W16); qui solo come testo:

- `DocumentStore::put` riceve un `ContentHash` già calcolato in streaming e il percorso di un file già validato (nessuna validazione dentro l'adapter); la chiave fisica è `sha256/<hex>` e non contiene mai il nome file del client;
- `put` è **idempotente per chiave**, anche sotto PUT concorrenti dello stesso hash (condizione 2 della decisione); il caso d'uso esegue `put` sempre, anche quando l'oggetto esiste già, così la sequenza è la stessa con o senza deduplica;
- `get` con range (`Range<u64>`), per il viewer;
- `delete` usato solo dal GC;
- nessun tipo di `object_store` (`ObjectStore`, `Path`, `PutResult`, errori) in firma: errori di dominio;
- il buffer del multipart (dimensione della parte e parti in volo) è configurazione dell'adapter;
- il protocollo di adozione `live|deleting` + `pending` (adozione A prima del PUT, adozione B dentro la transazione dei metadati e dell'outbox, GC in tre passi con l'I/O fuori transazione) è il contratto fra il caso d'uso di upload e il GC;
- l'admission control (tetto di upload in corso con 503 + `Retry-After`, semaforo di validazione, budget di byte temporanei, deadline di ricezione e di validazione, permesso e tempfile tenuti dal lavoro blocking fino alla fine) è un requisito del caso d'uso di upload, non dell'adapter;
- all'avvio il servizio ripulisce i tempfile rimasti da un crash (nel caso `crash-fra-put-e-tx` l'`abort` ne lascia uno in `/work/tmp/srv`).

### Impatto threat model

Da riportare in `docs/security/threat-model.md` dal consolidamento (WIRING W15):

- **T-05** (upload enormi, lenti, concorrenti): limite di 25 MiB su tre livelli, 413 dopo al più 26_214_401 byte letti, 408 per la ricezione lenta, 503 + `Retry-After` oltre il tetto, risorse al baseline dopo 15 s; evidenza `correctness.csv`, `admission.csv`, `dhat_*.json`.
- **T-06** (tipo falso, polyglot): magic bytes e struttura, 415; evidenza `correctness.csv`.
- **T-07** (zip bomb, DTD/entità): limiti d'archivio e rifiuto di `<!DOCTYPE`/`<!ENTITY`, 422; evidenza `correctness.csv`. Il prefiltro PDF (cifrati rifiutati, `/JavaScript`, `/Launch`, `/EmbeddedFile` anche con nomi `#hh` in quarantena) è byte-level: il controllo autorevole resta il processo isolato di S-02.
- **T-08** (path traversal): chiave `sha256/<hex>`, nome file del client mai nei percorsi; evidenza `correctness.csv` e codice di `sink.rs`.
- **T-11** (canale laterale della deduplica): risposta con la stessa forma e `upload_id` nuovo; tempi entro la soglia pre-registrata per Garage (default e parti da 2 MiB), fuori per PG `bytea`; evidenza `dedup_timing.csv`. Nuovo segnale: i 500 della deduplica concorrente rivelano che qualcuno sta caricando lo stesso contenuto nello stesso momento; la condizione 2 della decisione (PUT idempotente) lo chiude, da verificare quando si implementa l'adapter. Con le parti da 2 MiB d è negativo in tutte e tre le sessioni (−1,3 … −9,6 ms), entro T ma da rimisurare.
- **T-23** (accesso diretto allo storage): Garage solo sulla rete Compose, nessuna porta del server pubblicata, credenziali da env non versionato, `garage.toml` senza segreti, scanner di sicurezza verde; `with_allow_http(true)` accettato solo sulla rete locale (T-01-04-07); evidenza codice di `sink.rs`, `spikes/docker/garage.toml`, rapporto dello scanner.
- **Crash fra PUT e transazione e GC concorrente** (T-01-04-06 del piano): nessuna riga senza oggetto, orfano raccolto; evidenza `correctness.csv`, `gc.csv`, `gc_race.csv` (solo le interleaving enumerate).

### Destinazione

- **ADR-0003** [`docs/adr/0003-storage-originali-garage.md`](../../docs/adr/0003-storage-originali-garage.md), Stato `Proposed`: ad `Accepted` lo porta il consolidamento (01-17) dopo la ratifica dell'utente.
- **Riga `DEPENDENCIES.md` «Originali PDF/DOCX»:** Aperta → Decisa (Garage dietro `DocumentStore`, client `object_store` con buffer di multipart limitato). La scrive il consolidamento (WIRING W14), non questo piano.
- **N = 24 h** per il GC degli orfani (D-02).
- **Canale laterale:** nessuna mitigazione ulteriore richiesta per Garage dalla regola pre-registrata; PG `bytea` resta fuori anche mitigato.
