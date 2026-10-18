<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_3, piano PLAN-010-07. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-07 — Isolamento parser e file ostili

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_3.

**Esito: REVISE.** Il seam del lanciatore non basta ai consumatori e il disegno deve distinguere limite per figlio da limite del worker. La quota di decompressione prima/durante il loader è una premessa ancora non dimostrata.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-07.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-07-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `65bdb3f4fc6913757bdb40f969c5bdc05eae04db12f9122cf39f9c2dcc66c2c7`; coincide esattamente con la richiesta.
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

File ostili, loader prima del prefiltro, formati/filtri, limiti CPU/memoria/disk/PID/FD, stdout/stderr, timeout/cancellazione, process group e reaping, calibrazione e classificazione degli esiti, integrazione con extract/locator/OCR. Non si propone di riaprire D-08 né di introdurre Job Objects: il target resta Linux container come già deciso.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_3; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "65bdb3f4fc6913757bdb40f969c5bdc05eae04db12f9122cf39f9c2dcc66c2c7"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":3,"medium":1,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "interface_contract"
    title: "Il lanciatore chiuso non può esprimere le CLI dei figli successivi"
    scenario: "La CLI pubblicata è parse-parent run --child <bin> --input <file> [--out <dir>]. La 08 aggiunge extract pdf, la 09 locator locate --excerpt-file e la 13 ocr render --dpi oppure tesseract --image."
    trigger: "Un consumatore prova a usare il contratto senza modificare parse-parent.rs, vietato da W17."
    failure: "Mancano forwarding del sottocomando e degli argomenti specifici; il figlio può uscire con errore di parsing CLI anziché elaborare il documento. run_child accetta args a livello libreria ma la CLI obbligatoria non li espone."
    impact: "Tre piani successivi non sono eseguibili esattamente sul seam che la 07 chiude."
    affected: ["spikes/s02-parser/src/bin/parse-parent.rs","spikes/s02-parser/src/isolate.rs","PLAN-010-08","PLAN-010-09","PLAN-010-13"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-08-PLAN.md",".planning/phases/01-spike-tecnici/01-09-PLAN.md",".planning/phases/01-spike-tecnici/01-13-PLAN.md",".planning/phases/01-spike-tecnici/01-WIRING.md"],"details":"Riga 79: parse-parent run --child <nome-bin> --input <file> [--out <dir>]` lancia qualunque bin figlio del crate con i limiti. </interfaces> </context>  <tasks>  <task type=\"auto\">   <name>Task 1: Soglie ed esiti attesi, immagine"}
    mitigation: "Definire prima della chiusura una sola convenzione: argomenti dopo -- oppure job JSON tipizzato con operazione e parametri; conservare isolamento, limiti e allowlist dei bin. Aggiornare le promesse di tutti i consumatori e provarle con figli finti, senza anticiparne il contenuto."
    test_hint: "Lanciare tramite la CLI un figlio di prova che richiede sottocomando, input, out, excerpt-file e dpi; verificare argomenti Unicode/spazi e che nessuno sia reinterpretato dalla shell."
    test_required: true
  - id: "RISK-002"
    severity: "HIGH"
    likelihood: "MEDIUM"
    category: "parser_resource_limits"
    title: "Il tetto di decompressione arriva dopo un loader che può già espandere object stream"
    scenario: "Il primo passo è lopdf::Document::load_mem; solo dopo si enumerano stream FlateDecode applicando take(cap+1). RESEARCH §4 dichiara NV il comportamento del load e il caricamento di ObjStm compressi."
    trigger: "Un object stream grande o una catena di filtri costosa è processata nel loader prima del contatore, oppure un filtro diverso da Flate viene espanso da PDFium."
    failure: "Il limite dichiarato di byte decompressi può non intervenire prima del consumo; il backstop rlimit/timeout contiene alcuni casi ma può produrre ResourceLimit/Crashed invece di DecompressionLimit e non dimostra la quota richiesta."
    impact: "Falsa attestazione anti-bomb e possibile pressione sul cgroup condiviso. Non affermo che la specifica versione di lopdf abbia questo comportamento: è la premessa non verificata esplicita."
    affected: ["spikes/s02-parser/src/prescan.rs","spikes/s02-parser/src/bin/parse-child.rs","spikes/data/hostile/manifest.json"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-RESEARCH.md"],"details":"Riga 132: lopdf::Document::load_mem` dentro il figlio (anche il prescan è input ostile e gira sotto i limiti), rifiuto se il trailer ha `/Encrypt`, conteggio pagine dall'albero con limite 2.000, per ogni stream `/FlateDecode` deco"}
    mitigation: "Chiudere la ricerca collegata prima di attestare un tetto autorevole; delimitare formati/filtri supportati e rifiutare quelli senza quota verificabile, oppure contabilizzare dal primo punto di espansione. Distinguere quota di decompressione da contenimento per memoria/tempo."
    test_hint: "Fixture con ObjStm molto compresso, Filter array e filtri concatenati; misurare risorse già durante load_mem. Assicurarsi che il contatore intervenga prima dell'espansione vietata e registrare correttamente l'esito."
    test_required: true
  - id: "RISK-003"
    severity: "HIGH"
    likelihood: "HIGH"
    category: "concurrency_resource_isolation"
    title: "Il limite del container è condiviso da padre e tutti i figli"
    scenario: "Il worker ha 2 GiB complessivi, mentre RLIMIT_AS è calibrato per singolo figlio a tre volte VmPeak. Gli spike successivi lanciano fino a 16 processi OCR e scrivono nel tmpfs /work da 2 GiB."
    trigger: "Più figli benigni rispettano ciascuno il proprio limite ma superano insieme il cgroup, oppure il rendering riempie tmpfs."
    failure: "Né RLIMIT_AS né FSIZE per file limitano la somma di RSS e file. Il cgroup può terminare anche il padre; un segnale 9 da solo non identifica con certezza la causa o il processo scelto."
    impact: "La proprietà parent_alive non è garantita dal disegno e i risultati di parallelismo possono misurare OOM anziché throughput."
    affected: ["spikes/s02-parser/src/isolate.rs","spikes/s02-parser/src/bin/parse-parent.rs","spikes/compose.yaml","PLAN-010-13"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-07-PLAN.md",".planning/phases/01-spike-tecnici/01-02-PLAN.md",".planning/phases/01-spike-tecnici/01-13-PLAN.md"],"details":"Riga 97: RLIMIT_AS` = 3 × massimo `VmPeak` misurato sul corpus benigno, tetti Flate = 512 MiB totali, 64 MiB per stream, rapporto 100× oltre 16 MiB (RESEARCH §4, da confermare con il massimo rapporto reale del corpus benigno); li"}
    mitigation: "Definire ammissione e budget aggregati coerenti con il cgroup, quota per job anche per disco/tmpfs e cleanup; se si promette padre sempre vivo in caso di OOM usare un confine di memoria per job separato. Non inferire OOM dal solo SIGKILL: registrare evento del container e stato del padre."
    test_hint: "Figli che allocano sotto AS individuale ma oltre il budget aggregato; rendering multi-pagina e 1/4/8/16 OCR. Provare padre vivo, rifiuto controllato e risorse ripristinate dopo ogni job."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "cancellation_lifecycle"
    title: "Cleanup del gruppo definito solo per timeout"
    scenario: "isolate.rs prevede killpg allo scadere e kill_on_drop del Child; OCR può aggiungere tesseract come discendente. Non è definito il cleanup del gruppo su cancellazione del chiamante, errore di lettura o ritorno anticipato."
    trigger: "Il futuro run_child viene droppato o il figlio supera il cap stdout mentre un discendente mantiene pipe o file aperti."
    failure: "La morte del solo processo diretto può lasciare lavoro e file vivi; fermare la lettura dopo 1 MiB può inoltre bloccare lo scrittore fino al timeout, senza segnalare l'output eccedente."
    impact: "Misure successive contaminate, PID/FD/tmpfs trattenuti e classificazione fuorviante come Timeout."
    affected: ["spikes/s02-parser/src/isolate.rs","spikes/s02-parser/src/protocol.rs","spikes/s02-parser/results/isolation/hostile.csv"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-07-PLAN.md"],"details":"Riga 131: stdout/stderr in pipe con lettura limitata a 1 MiB (`take`), `kill_on_drop(true)`, `process_group(0)`, `unsafe pre_exec` che chiama solo `rlimit::setrlimit` per `AS`, `CPU`, `FSIZE`, `NOFILE`, `CORE=0` (nessuna allocazio"}
    mitigation: "Gestire il gruppo su ogni uscita con un supervisore/guardia e reaping esplicito; leggere stdout/stderr concorrentemente con limite e un esito distinto per eccesso, senza accumulo illimitato. Il cleanup va provato anche quando è il padre a essere cancellato."
    test_hint: "Figlio finto che genera un discendente persistente e produce più di 1 MiB su entrambe le pipe; interrompere il futuro e controllare assenza di discendenti, zombie e job directory residue."
    test_required: true
external_research: 
  - id: "RESEARCH-001"
    linked_risk: "RISK-002"
    question: "Su lopdf 0.45.0, Document::load_mem espande automaticamente ObjStm o altre strutture compresse prima che il chiamante possa enumerare gli stream? Esistono quote per tale caricamento e quali filtri/Filter array possono eludere il contatore manuale FlateDecode previsto da PLAN-010-07? Verificare sul sorgente della versione esatta e indicare se PDFium con binding pdfium-render 0.9.4 consente un tetto autorevole sull'intero output di decompressione."
    blocking: true
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**