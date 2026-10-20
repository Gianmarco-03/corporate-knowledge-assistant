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

(da compilare con le misure)

## Risultati

(da compilare con le misure)

## Anomalie e deviazioni

**Modifica:** Claude (gsd-executor 01-04, ripresa 2) · 2026-10-20 · provenienza delle misure ereditate da un esecutore interrotto

- **Misure ereditate da un esecutore interrotto (errore di rete verso l'API).** Correttezza (sessione `01-04-20261020t113514z-ecb4`), storage e GC (`01-04-20261020t114001z-8b12`) e le tre sessioni del canale laterale (`01-04-20261020t114049z-d302`, `01-04-20261020t114328z-ab51`, `01-04-20261020t114531z-b1d8`) sono state prodotte da `bench.sh host correctness|extras|dedup <n>` con il codice del commit `15ea345` (campo `git_rev` del manifest hardware grezzo di ogni sessione, in `results/raw/`), con l'albero del codice invariato da quel commit. Sono tracciabili e si tengono: ripetere le tre sessioni del canale laterale dopo che i tempi erano già su disco sarebbe una seconda estrazione, vietata dal protocollo pre-registrato (RISK-001).

## Esito

(da compilare dopo le misure)
