<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_2, piano PLAN-010-04. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-04 — Upload streaming e storage originali

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_2.

**Esito: REVISE.** L'esecuzione del benchmark e la comparabilità fra adapter vanno corrette; la soglia per-file non dimostra un budget aggregato e il GC richiede un protocollo contro l'adozione concorrente.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-04.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-04-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `e0ba8c86b304d7ac94550ad812231e33f073a0c79c44bf6fc237429b436e25b4`; coincide esattamente con la richiesta.
- Il connettore GitHub è stato usato con lo SHA completo, senza sostituirlo con il default branch.

L'albero Git completo restituito non era troncato: allo SHA indicato **non esistono spikes/, Cargo.toml radice, codice o test di prodotto implementati**. Esistono documenti e scheletro con .gitkeep. L'assenza degli spike futuri è prevista: non è un finding. I rischi sotto riguardano l'esecuzione futura del PLAN e i contratti che promette, non difetti runtime di codice già scritto.

## Evidenza e limiti

Letti CONTEXT, RESEARCH, WIRING e GATE della fase, il PLAN in oggetto e i piani confinanti rilevanti. Ulteriori riscontri: regole correnti in CLAUDE.md e docs/architecture/interfaces-and-concurrency.md; stub di data-model/api-contract/threat-model; configurazione .gitignore e regole dei confini dei crate. Non ho avviato container, installato pacchetti, eseguito parser/file ostili o chiamato provider a pagamento. I test indicati sono verifiche richieste, **non test già passati**.

Provenienza:
- **repository**: presenza/assenza dei file, configurazione e regole attuali allo SHA sopra;
- **plan**: istruzioni e interfacce prescritte dai documenti citati in evidence, con righe del PLAN dove applicabile;
- **reasoning**: scenario condizionale e conseguenze riportati per ciascun rischio; non sono osservazioni di un incidente reale;
- **external_required**: questioni puntuali in external_research. Le etichette [V]/[NV] della ricerca di fase sono attribuzioni del documento: qui non diventano verifiche web nuove.

Il gate 01-GATE.md è stato letto, inclusi i finding già corretti. I suoi esiti non sostituiscono questa verifica indipendente dei failure mode. Le decisioni D-01..D-22 restano vincolanti; nessun rischio viene accettato per conto dell'utente.

## Copertura

Invarianti hash/originale/metadati/outbox; transazioni e finestre di crash; retry e dedup; GC/range; precisione delle dimensioni e metriche; limiti, tempfile e risorse; upload interrotti, credenziali e informazioni osservabili. D-01 Garage e D-06 tre adapter restano fisse; la 04 già sposta la validazione in spawn_blocking, ma ciò non chiude il problema dei budget.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_2; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "e0ba8c86b304d7ac94550ad812231e33f073a0c79c44bf6fc237429b436e25b4"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":3,"medium":2,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "benchmark_harness"
    title: "bench.sh richiede Docker dentro un runner privo di client e accesso al daemon"
    scenario: "Task 2 esegue bench.sh nel runner e gli fa lanciare docker compose run per un server dedicato. La 02 installa nel runner procps/iproute2/curl/qpdf/Python/git, non il client Docker, e non monta un canale verso il daemon."
    trigger: "Esecuzione letterale del benchmark nel container runner."
    failure: "Il comando di orchestrazione non può creare il container server. Neppure endpoint/risoluzione del server dedicato sono verificati dal piano."
    impact: "Nessuna misura di carico eseguibile come descritta; correzione richiede cambiare l'harness chiuso oppure il confine host/runner."
    affected: ["spikes/s01-upload/bench.sh","spikes/docker/runner.Dockerfile","spikes/compose.yaml"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-04-PLAN.md",".planning/phases/01-spike-tecnici/01-02-PLAN.md"],"details":"Riga 149: `bench.sh` (nel runner, endpoint solo da `S01_TARGET_URL` e dall'env, nessun URL letterale): per ogni adapter (`garage`, `pgbytea`, `null`) avvia il server in un container dedicato (`docker compose ... run -d --name cka-"}
    mitigation: "Tenere l'orchestrazione Compose sull'host; usare il runner per server/client e lettura delle metriche, con endpoint interno verificato e identificativo del container. Non risolvere montando indiscriminatamente il socket Docker nel runner che riceve chiavi provider."
    test_hint: "Da un worktree pulito eseguire lo script documentato fino a una richiesta valida, verificando chi avvia i container, DNS del server e accessibilità del suo PID/cgroup."
    test_required: true
  - id: "RISK-002"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "measurement_validity"
    title: "Il carico ripete lo stesso contenuto e non separa deduplica da persistenza nuova"
    scenario: "Ogni configurazione usa 200 POST della stessa fixture. La chiave è sha256 e PgByteaSink usa ON CONFLICT DO NOTHING; Garage segue invece il proprio percorso PUT."
    trigger: "Dal secondo upload di una fixture e nelle run successive, senza reset o contenuti distinti."
    failure: "Il benchmark dell'adapter PG può misurare soprattutto conflitti/deduplica mentre Garage scrive oggetti. Le statistiche aggregate non descrivono lo stesso workload; VmHWM dello stesso server conserva inoltre il massimo delle configurazioni precedenti."
    impact: "ADR-0003 può attribuire differenze di memoria/latenza/WAL allo storage anziché alla diversa quantità di lavoro realmente eseguita."
    affected: ["spikes/s01-upload/bench.sh","spikes/s01-upload/results/adapters.csv","spikes/s01-upload/results/mem_by_size.csv","docs/adr/0003-storage-originali-garage.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-04-PLAN.md"],"details":"Riga 149: per dimensione 1/10/24/25 MiB lancia `oha -n 200 -c <c> --no-tui --output-format json -m POST -F file=@<fixture> \"$S01_TARGET_URL\"` con c = 1, 4, 8, 16 (file `results/oha_<adapter>_<size>_c<c>.json`); ripete ogni configu"}
    mitigation: "Separare workload nuovi e deduplicati per tutti gli adapter, generare corpi validi distinti e verificarne gli hash; dichiarare reset di DB/oggetti e cold/warm. Misurare VmHWM in un processo nuovo per configurazione, conservando run_id e conteggi effettivi delle scritture."
    test_hint: "Accertare che una run new con 200 richieste produca 200 hash/oggetti, una dedup ne produca uno e ogni adapter lavori sugli stessi insiemi. Ripetere dimensioni in ordine inverso: il confronto memoria deve restare coerente."
    test_required: true
  - id: "RISK-003"
    severity: "HIGH"
    likelihood: "MEDIUM"
    category: "resource_exhaustion"
    title: "spawn_blocking evita il blocco async ma non limita le validazioni ostili"
    scenario: "Dopo il tempfile ogni richiesta lancia una validazione ZIP/PDF sincrona con spawn_blocking. Non sono previsti semaforo di validazione, deadline di upload o budget per tempfile concorrenti."
    trigger: "Molti upload lenti o ZIP onerosi, seguiti da disconnessione dei client."
    failure: "Il contatore limita il singolo file, non il numero dei job né la somma dei file temporanei; il lavoro blocking già partito può continuare anche dopo la cancellazione del futuro HTTP."
    impact: "Esaurimento di disco/pool/memoria e tempi di acknowledgment non rappresentativi; il processo server può diventare indisponibile pur rispettando 25 MiB per file."
    affected: ["spikes/s01-upload/src/upload.rs","spikes/s01-upload/src/validate.rs","spikes/s01-upload/tests"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-04-PLAN.md"],"details":"Riga 113: tokio::task::spawn_blocking` (lettura zip e scansione a byte del PDF fino a 25 MiB sono lavoro CPU-bound e I/O sincrono: mai sul runtime async) → `sink.put(sha, path)` → transazione breve su Postgres con riga `upload` + "}
    mitigation: "Stabilire admission control e budget di job/byte temporanei, deadline di ricezione e cleanup. Per la parte non interrompibile dichiarare e provare il contenimento; se serve isolamento, coordinare il piano proprietario senza aggiungere parser di prodotto."
    test_hint: "Carico di upload lenti e documenti costosi oltre la concorrenza ammessa; scollegare metà dei client. Verificare numero massimo di validazioni attive, rifiuti prevedibili e ritorno di uso disco/FD al baseline."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "transactions_gc"
    title: "GC può eliminare un oggetto che un upload concorrente sta adottando"
    scenario: "Il GC seleziona un vecchio hash senza riga DB e lo cancella dopo list/head. Un retry dello stesso contenuto può fare PUT e poi commit fra controllo e delete del GC."
    trigger: "Upload dello stesso hash durante la scansione di un orfano vecchio, anche con N=24 ore."
    failure: "La soglia temporale non sincronizza controllo DB, adozione e cancellazione; può restare una riga upload con oggetto assente."
    impact: "Invariante originale+metadati rotta e conclusione di robustezza del GC non dimostrata dal solo caso crash con N=0."
    affected: ["spikes/s01-upload/src/bin/probe.rs","spikes/s01-upload/src/sink.rs","spikes/s01-upload/sql/schema.sql"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-04-PLAN.md"],"details":"Riga 117: sottocomandi `gc` (crea oggetti orfani, misura list+head e la cancellazione di quelli più vecchi di N ore con N parametrico, CSV `gc.csv`), `range` (p95 di letture da 64 KiB per adapter, CSV `storage.csv`), `pgsize` (WAL"}
    mitigation: "Definire un protocollo per hash fra adozione e GC (prenotazione/lock e ricontrollo, oppure generazione e delete condizionale se supportato), mantenendo I/O esterno fuori dalle transazioni brevi. Nel report non trattare il test sequenziale come prova di assenza di race."
    test_hint: "Barriere dopo il controllo di orfanità e prima del delete; inserire PUT+commit dello stesso hash in quella finestra e accertare che l'oggetto referenziato resti leggibile."
    test_required: true
  - id: "RISK-005"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "test_fixtures"
    title: "PDF con header e byte casuali non dimostra la correttezza di upload validi"
    scenario: "Le fixture grandi sono definite come printf %PDF-1.7 seguito da urandom. Non hanno una struttura PDF valida e possono contenere casualmente nomi cercati dal prefiltro; l'header va inoltre contato nella dimensione."
    trigger: "Misure nominali 25 MiB o benchmark della validazione con tali corpi."
    failure: "Un falso PDF può risultare accepted solo perché passa il prefiltro; una fixture di S byte più header supera il limite atteso. La matrice può misurare errori/413 o una validazione incompleta."
    impact: "Risultati di memoria e stato HTTP difficili da interpretare; la garanzia dichiarata di validazione completa eccede il controllo effettivo."
    affected: ["spikes/s01-upload/fixtures.sh","spikes/s01-upload/results/correctness.csv","spikes/s01-upload/RISULTATI.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-04-PLAN.md"],"details":"Riga 145: `fixtures.sh` (eseguito nel runner, output in `/work/tmp/fixtures`, mai versionato): PDF sintetici `printf '%%PDF-1.7\\n'` + `head -c` da `/dev/urandom` per 1, 10, 24, 25 MiB, 25 MiB+1 byte, 100 MiB; un PDF e un DOCX real"}
    mitigation: "Separare test di puro streaming con corpi artificiali dai test di formato; produrre PDF strutturalmente validi di dimensione totale esatta e verificarli indipendentemente. Chiamare esplicitamente prefiltro il controllo byte-level: l'oggetto compresso va verificato dallo spike isolato."
    test_hint: "Misurare stat e hash di ogni fixture prima del POST; provare MAX-1, MAX, MAX+1 contando l'intero file e verificare i PDF di successo con un validatore indipendente."
    test_required: true
external_research: []
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**