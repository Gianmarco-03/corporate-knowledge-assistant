# Racconto del piano 01-17: consolidamento della fase 1

**Modifica:** Claude (bridge 01-17) · 2026-10-26 · creazione del racconto didattico del piano 01-17

## Cosa

Fino a questo piano gli spike della fase 1 avevano prodotto misure e proposte. Ogni spike aveva un RISULTATI con le misure e un ADR in stato *Proposed*. Nessuna di queste proposte vincolava ancora le fasi successive, e il threat model era uno stub di due righe. Il piano 01-17 trasforma le proposte in decisioni e mette per iscritto i rischi.

Concretamente:

- **Il threat model è compilato** (`docs/security/threat-model.md`). Elenca 39 minacce. Per ognuna dice quale requisito la copre e dove si trova la prova: un file di misura in `spikes/` oppure il test previsto nel prodotto, con la fase. Chiude il requisito PLAT-07.
- **Hai ratificato tutti gli ADR 0003-0012** il 2026-10-26 («ratifica-tutti»). Sono passati da *Proposed* ad *Accepted*.
- **ADR-0013 rinvia antivirus e CDR** alla fase 14, con un rischio residuo dichiarato.
- **`DEPENDENCIES.md` registra le scelte come Decise**, ciascuna col link al proprio ADR: Garage, pgvector con ricerca esatta, reqwest 0.13, bge-m3 su TEI, Sonnet 5.5, PDFium con Docling e Tesseract, no-rerank, `jev-1.13.0`.
- **Una sezione «Limiti noti»** (D-25) in ADR-0004 e nel README dichiara i PDF che il sistema rifiuta per scelta.

Il piano chiude anche il criterio 5 della fase. Ogni esito degli spike sta in un ADR con misure e hardware, e il codice degli spike resta fuori dal prodotto.

## Come

Il piano non contiene codice: solo documenti. Il percorso di lettura consigliato quindi segue i documenti nell'ordine in cui dipendono l'uno dall'altro.

### 1. Come si legge il threat model

Apri `docs/security/threat-model.md` e leggilo in quest'ordine.

- **La riga delle priorità** (`:5`) è quella dello stub, conservata parola per parola. La riga successiva (`:7`) la traduce in righe della tabella. Per esempio, «broken access control» corrisponde a T-04, T-13, T-14, T-15 e T-36. È l'indice più rapido per arrivare a una minaccia partendo da una preoccupazione.
- **Gli asset** (`:11-21`) sono ciò che va protetto. Ogni riga della tabella nomina, nella colonna «Asset», uno di questi.
- **Attori e confini** (`:25-43`). La tabella degli attori dice per ciascuno quanto è fidato, cosa può fare e cosa potrebbe volere. Le righe di JEV e Anthropic (`:33-34`) dicono dove finiscono i dati che inviamo e per quanto tempo restano: JEV negli Stati Uniti senza un termine di cancellazione, Anthropic fino a 30 giorni. I cinque confini (`:39-43`) sono i punti in cui un dato passa da una zona fidata a una meno fidata. Il confine 2 (`:40`) è il più sottile: anche il DTO che arriva da un altro servizio interno è trattato come dato non fidato, e diventa `AuthorizedEvidence` solo passando da `authorize_response` (ADR-0012).
- **La tabella «Threat analysis»** (`:49-89`) è il cuore del documento. Ogni riga si legge da sinistra a destra come una frase. Prendiamo T-13 (`:63`): «il retrieval restituisce chunk non autorizzati» (minaccia), categoria STRIDE *I* (information disclosure), probabilità e impatto alti. La mitigazione è il predicato ACL dentro la stessa query SQL. I requisiti sono SEC-01, SEC-02 e SEC-05. La prova è `spikes/s03-pgvector/results/index/leak.csv`, dove la colonna `denied_in_topk` vale 0 su 2.067 righe. Lo stato è «misurata nello spike».
- **Le tre parole della colonna Stato** sono definite nella legenda (`:47`) e hanno significati diversi:
  - «misurata nello spike»: c'è un file di misura che puoi aprire e ricontrollare;
  - «da implementare»: la mitigazione si costruisce nel prodotto, e la colonna Evidenza dice quale test lo proverà e in quale fase. Per esempio T-02 (`:52`) si prova nella fase 5, con token `alg none` e `kid` sconosciuto rifiutati con 401;
  - «rinviata (ADR-0013)»: c'è un solo caso, T-39 (`:89`).

  Lo stato «da implementare» non è un rischio accettato. È una promessa con un test, e il verificatore di quella fase lo controllerà (`:118`).
- **Le righe da T-29 in poi** (`:79-89`) sono minacce che RICERCA non aveva previsto e che gli spike hanno fatto emergere. Due esempi. T-30 (`:80`): la GPU da 6 GiB condivisa fra embedding, Docling ed eventuale reranker, con un margine misurato di soli 70 MiB. T-32 (`:82`): la variabilità dei segnali JEV fra una ripetizione e l'altra, che rende pericolosa una soglia troppo stretta.
- **La checklist LLM/RAG** (`:91-102`) è un controllo incrociato. Una voce è spuntata solo se esiste una riga con la mitigazione corrispondente. La nota a `:102` avverte che una spunta non significa «già nel prodotto»: per saperlo si guarda lo Stato delle righe citate.
- **Il rischio residuo** (`:104-118`) elenca ciò che resta scoperto anche dopo le mitigazioni. Ogni voce ha un responsabile e una condizione di riapertura. Per esempio la voce AV/CDR (`:108`) si riapre «prima di caricare documenti reali o di aprire l'upload a utenti esterni».

### 2. La ratifica negli ADR

Ogni ADR 0003-0012 è cambiato in due righe sole: `- Stato: Accepted` alla riga 3 e una riga `**Modifica:**` di ratifica. Ad esempio `docs/adr/0003-storage-originali-garage.md:3` e `:9`. Gli ADR sono stati scritti da altre slice, quindi questo piano ne tocca il meno possibile, e il diff lo dimostra (TEST-7). L'unica eccezione voluta è ADR-0004, che riceve anche la sezione «Limiti noti» (`docs/adr/0004-parser-ocr-isolamento.md:76-83`).

Su ADR-0006 c'è un dettaglio che conviene ricordare. La riga 31 dice che la porta «conserva `query_text`». ADR-0012 invece l'ha tolto da `VectorIndex::search`, perché la ricerca ibrida non è stata adottata. Hai scelto di non correggere la frase e di lasciare il rimando nella riga di ratifica (`:67`). Chi legge ADR-0006 deve quindi arrivare fino in fondo per sapere che quella frase non vale più.

### 3. Il legame ADR → DEPENDENCIES → Storia

È una catena in tre anelli, e conviene seguirla su un esempio. Prendiamo lo storage degli originali.

1. **L'ADR porta la decisione e le misure.** `docs/adr/0003-storage-originali-garage.md` dice perché Garage e non `bytea`: il canale laterale della deduplica, T-11. Dice anche con quali parametri: parti da 2 MiB, una sola in volo. Ora è *Accepted* (`:3`).
2. **`DEPENDENCIES.md` porta lo stato della dipendenza.** La riga «Originali PDF/DOCX» (`DEPENDENCIES.md:54`) passa da *Aperta* a *Decisa*. Riassume la scelta e finisce col link `[ADR-0003](docs/adr/0003-storage-originali-garage.md)` e con le decisioni del CONTEXT (D-01, D-02). La regola che segue il piano è che una riga diventa Decisa solo se l'ADR che cita è stato ratificato (TEST-2).
3. **La Storia conserva la regola superata.** Sotto la tabella, il paragrafo *Storia* (`DEPENDENCIES.md:57`) riporta com'era la riga prima: *Aperta*, fra «Object storage S3-compatible oppure PostgreSQL `bytea`», con la nota su MinIO. Dice anche quale spike l'ha chiusa. Niente viene cancellato in silenzio: dalla Storia si ricostruisce sempre l'evoluzione della regola (CLAUDE.md, §Firma).

Le altre tre Storia seguono lo stesso schema:

- `:33` per Vector search e HTTP client;
- `:73` per il modello JEV, da `jev-latest` a `jev-1.13.0`, con il motivo: un alias mobile cambierebbe i segnali senza che il codice cambi;
- `:93` per le quattro righe dei Modelli AI. Questa Storia registra anche che il piano prevedeva Haiku per le query semplici, e che la riga dice invece Sonnet per la tua decisione D-33.

### 4. ADR-0013

`docs/adr/0013-rinvio-antivirus-cdr.md` segue il template:

- **Contesto** (`:10`): ASVS L2 chiede antivirus e CDR;
- **tre Opzioni** (`:18`);
- **Decisione** (`:24`): rinvio alla fase 14, con la motivazione e la condizione che fa decadere il rinvio;
- **Conseguenze** (`:36`), compresi i prerequisiti già pronti per aggiungere un passo di scansione;
- **Evidenze** (`:44`).

Il CDR, *Content Disarm & Reconstruction*, ricostruisce il file tenendo solo il contenuto ritenuto sicuro. L'ADR spiega perché non è una dipendenza da aggiungere e basta: riscrivendo il PDF cambierebbero l'hash e i rettangoli su cui poggiano deduplica, locator e viewer (`:21`).

### 5. Limiti noti

La sezione di ADR-0004 (`:76-83`) è per chi sviluppa. Nomina i filtri (`LZWDecode`, `RunLengthDecode`) e gli esiti tipizzati (`Unsupported`, `DecompressionLimit`), e dà il conteggio della calibrazione: 0 PDF benigni rifiutati su 10. La sezione del README (`README.md:111-120`) dice le stesse cose in linguaggio per l'utente e rimanda all'ADR.

## Perché

- **Ratifica in un solo punto, dopo tutte le slice.** Ogni ADR poteva essere ratificato alla fine del proprio spike. Si è preferito farlo qui perché è l'unico momento in cui vedi tutte le decisioni insieme, con le soglie mancate affiancate (`.planning/tmp/ratifica-01-17.md`, «In sintesi»). Per esempio la scelta del no-rerank (ADR-0009) e il margine VRAM (T-30) si capiscono solo insieme.
- **Solo Stato e Modifica sugli ADR altrui.** L'alternativa era correggere gli ADR durante la ratifica, ad esempio la frase su `query_text`. È stata scartata perché il piano 01-17 non possiede quei file: la regola dei file chiusi e WIRING §4 vietano di riscriverli. Il compromesso, una riga di stato e una di firma, è dichiarato come deviazione nel piano ed è controllato da TEST-7.
- **Valori reali degli ADR in DEPENDENCIES, non quelli del piano.** Il piano era stato scritto prima delle misure e diceva «Haiku per le semplici» e «ibrida secondo l'esito». Copiarlo alla lettera avrebbe registrato come Decisa una scelta che le misure e la tua D-33 avevano già superato. L'esecutore ha scritto ciò che dicono gli ADR ratificati e l'ha spiegato nella Storia (`DEPENDENCIES.md:93`).
- **Rinviare AV/CDR invece di installarli.** ClamAV costa un container, l'aggiornamento continuo delle firme e una nuova superficie d'attacco. Il CDR tocca hash, locator e viewer. Su un corpus pubblico o sintetico non c'è nulla di reale da proteggere (D-05). Il rischio è accettato apertamente, con una condizione di riapertura scritta (T-01-17-05 del piano).
- **Stato a tre valori nel threat model.** Un semplice «mitigata sì/no» avrebbe mescolato ciò che è misurato con ciò che è solo progettato. Separare «misurata nello spike» da «da implementare» impedisce il falso verde che il test design chiama RISK-3: una mitigazione dichiarata misurata senza un file che lo provi.
- **T-12, T-15, T-22 e T-23 «da implementare» anche con prove parziali.** Lo spike prova il contratto candidato (`spikes/s07-ports/src/lib.rs`), non la mitigazione nel servizio. Dichiararle misurate avrebbe anticipato una garanzia che esisterà solo nel prodotto.

## Test

Il test design prevede solo controlli shell da lanciare dalla radice del repository, nessun file di test. Il bridge li ha rilanciati tutti sul main fuso `26bb554`:

- **Isolamento degli spike (TEST-1):** `git grep -l "spikes/" -- ':!spikes' ':!.planning' ':!docs' ':!*.md' ':!.gitignore'` deve restare vuoto.
- **Ratifiche coerenti (TEST-2):** `grep -m1 'Stato:' docs/adr/00*.md` deve dare Accepted su 0003-0013. In `git diff 5cfb581..26bb554 -- DEPENDENCIES.md`, le righe aggiunte con «Decisa» devono citare solo quegli ADR.
- **Evidenze e stati del threat model (TEST-3, TEST-4, TEST-5):** si estraggono i percorsi con `grep -oE 'spikes/[A-Za-z0-9_./*-]+' docs/security/threat-model.md | sort -u`, poi `test -f` su ciascuno. Per le colonne e gli stati il bridge ha usato un piccolo script Python che spezza ogni riga `| T-NN |` nelle sue 10 celle.
- **Storia (TEST-6) e diff limitato (TEST-7):** `git diff 5cfb581 26bb554 -- DEPENDENCIES.md` e `git diff --numstat 5cfb581 26bb554 -- docs/adr`. Attenzione al commit base: le righe di ADR-0012 cambiate in `5cfb581` stanno prima dell'intervallo e non contano.
- **Link degli ADR (TEST-8) e verify del Task 3:** il ciclo `for f in docs/adr/00{03..11}-*.md` del piano (`01-17-PLAN.md:161`).
- **Scanner (TEST-9):** `python skills/security-scanner/security_scan.py DEPENDENCIES.md docs/adr docs/security`.
- **ADR-0013 (TEST-10) e Limiti noti (TEST-12):** si controllano con `grep` sulle intestazioni di sezione.
- **Checklist (TEST-11):** è manuale. Il bridge ha fatto la lettura voce per voce, trovando due annotazioni minori, descritte qui sotto.

Tutti verdi.

## Deviazioni e punti aperti

- **Evidenze aggiustate rispetto al piano.** Il piano suggeriva di accoppiare `instructs_model_flagged` a `injection.csv`, ma la colonna sta in `spikes/s04-jev/results/jev/forms.csv`, ed è lì che T-16 la cita. T-24 cita anche le misure di concorrenza e di spesa. T-28 cita i `hw.json` che contengono davvero un digest.
- **README «Stack deciso».** Dopo il merge il paragrafo diceva ancora che storage, LLM ed embedding erano decisioni aperte. Il coordinatore l'ha corretto nel working tree (`README.md:52`, `:58`, Storia a `:60`, firma a `:122`). La correzione non è ancora committata ed entra nella registrazione del piano.
- **Incoerenze rimaste**, da sistemare con un ritocco documentale (i Markdown si possono modificare da qualsiasi fase):
  - il threat model cita ADR-0013 come testo senza link (`threat-model.md:89`, `:108`);
  - in ADR-0006 le righe Modifica attaccate alla tabella (`:66-67`) vengono rese come righe della tabella;
  - il titolo di ADR-0011 parla ancora di «routing per complessità»;
  - la checklist (`threat-model.md:96`) attribuisce a T-18 un «rifiuto su evidenza insufficiente», mentre T-18 rifiuta sulla contraddizione;
  - per «segreti nei prompt» la copertura è solo indiretta (T-22, T-31).
- **Cosa hai accettato ratificando.** Le soglie mancate dichiarate di ADR-0004 (CER OCR, TTQ-parse, throughput OCR, estrazione PDFium) e la latenza del reranker in ADR-0009. In più le misure rinviate al prodotto: PUT idempotente, `instructs_model` su testo ostile, VRAM con due servizi, worker oltre 2 GiB. Il bridge può provare che i documenti riflettono la ratifica, non che queste accettazioni siano giuste. Per questo nel SUMMARY la voce resta come giudizio umano.
- **Ancora da decidere:** come rientrare nel TTQ-parse (D-27, ADR-0004). Inoltre PLAT-07 risulta ancora *Pending* in `REQUIREMENTS.md:109`, finché la registrazione di stato o la verifica di fase non lo aggiornano.
