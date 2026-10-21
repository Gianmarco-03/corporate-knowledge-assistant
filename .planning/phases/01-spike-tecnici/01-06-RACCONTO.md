# Racconto del piano 01-06: gli oracoli di misura degli spike

## Cosa

Prima di questo piano gli spike avevano un corpus (01-03) ma nessun metro: non c'era modo di dire se un parser aveva letto bene una pagina o se il retrieval aveva trovato il passaggio giusto. Adesso il corpus ha i suoi valori attesi. Sono 80 domande etichettate, in italiano, in inglese e incrociate fra le due lingue, ciascuna con i passaggi che la sostengono, il ruolo di chi la pone, la complessità e l'indicazione se il sistema deve rifiutarsi di rispondere. A queste si aggiungono 11 oracoli del parser italiano: il testo atteso di sei pagine, le celle di quattro tabelle e l'elenco delle pagine con tabelle dell'estratto da 25 pagine.

Ogni oracolo è passato da tre controlli: chi lo ha derivato, il double-check di GPT e la ratifica dell'utente. Entrano nelle misure di S-02, S-03, S-04 e A-04 solo quelli con tutti e tre i controlli verdi: 74 query su 80 e tutti gli 11 oracoli del parser. Il piano porta PLAT-07 per la parte che riguarda la provenienza dei dati di misura e l'uscita controllata di contenuti verso un altro provider.

## Come

### Percorso di lettura consigliato

Qui non c'è codice: ci sono file di dati e documenti di derivazione. Conviene leggerli in quest'ordine.

1. **`spikes/data/corpus/oracles/ORACOLI.md`** spiega in prosa tutto il resto.
   - La regola d'ingresso nelle misure, cioè i tre assi verdi, sta a `ORACOLI.md:5`.
   - Da dove vengono i valori è spiegato a `:17-21`: i Markdown sintetici letti come file, i PDF rasterizzati con `pdftoppm` nel runner e letti a vista come immagini, senza alcun estrattore di testo.
   - L'identità del derivatore e la deviazione accettata dall'utente stanno a `:23-25`.
   - Le tre classi di provenienza ammesse sono a `:27-35`, le convenzioni (pagine, normalizzazione, tabelle, duplicati, visibilità) a `:37-49`.
   - La regola di complessità, scritta prima di etichettare, è a `:51-58`.
   - I conteggi sono a `:60-75`, lo stato del guardrail con la tabella dei sette lotti a `:77-101`.
   - I due TO-DO di dominio, con le decisioni dell'utente, sono a `:103-113`.

2. **`spikes/data/corpus/oracles/queries.json`** è un array di 80 oggetti con le stesse chiavi. Conviene guardarne tre.
   - **Q-001** (`queries.json:2-38`) è il caso tipico. `provenance_class` è `estratto-sorgente` (`:6`) e `source_ref` dice dove sta il valore: documento e `section_path`, l'elenco delle intestazioni del Markdown dalla più esterna alla più interna (`:7-11`). `expected_value` è la stringa `"182,4 M€"` (`:12`), non un numero: così resta la virgola decimale del documento, e il controllo automatico rifiuta qualunque `number` JSON. Il blocco `guardrail` (`:17-21`) porta i tre assi. Poi vengono le chiavi di misura: il ruolo `guest` (`:25`), la complessità con il rimando alla motivazione (`:26-27`), i passaggi attesi con l'estratto verbatim (`:28-34`), `no_answer` (`:35`). `gpt_ref` e `ratified_at` (`:36-37`) dicono quale lotto di GPT ha visto la voce e quando l'utente l'ha ratificata.
   - **Q-008** (da `queries.json:272`) è una delle sei escluse. `guardrail.gpt` vale `PARTIALLY_CONFIRMED` (`:288`), `guardrail.user` è `null` (`:289`) e `todo_ref` punta a `TODO-02` (`:291`). Un consumatore che filtra sui tre assi la scarta da sola.
   - **Q-015** (da `queries.json:538`) è una trappola. La classe è `derivazione-documentata`, perché un rifiuto non ha un estratto da citare: si documenta invece quali documenti sono stati controllati, cioè tutti quelli visibili al ruolo `manager` (`source_ref.documents_checked`).

3. **`spikes/data/corpus/oracles/derivations/queries-it.md`** (e i fratelli `queries-en.md` e `queries-cross.md`) contiene la stessa informazione in forma leggibile, più lo svolgimento.
   - L'intestazione (`queries-it.md:1-8`) ripete la regola di provenienza.
   - La voce di Q-001 (`:10-24`) mostra lo schema: tipo e classe, una riga di motivazione della complessità, la visibilità del ruolo, l'estratto e lo svolgimento. Lo svolgimento spiega anche perché il 182,4 di `syn-01` e `syn-02` non è un passaggio atteso: quei documenti non sono visibili al `guest`.
   - La voce di Q-008 (`:112-123`) mostra il caso discusso con GPT: la motivazione «una sola regola in una frase» è proprio il punto del dissenso.
   - La voce di Q-015 (`:214-222`) è la trappola: la risposta (30 giugno 2026) sta solo in `syn-09`, che è `restricted`. Se il sistema risponde a un `manager`, è un leak.

4. **`spikes/data/corpus/oracles/parser-it.json`** e le sue due derivazioni.
   - Il testo atteso di P-006 inizia a `parser-it.json:128`. La sua `normalization` (`:138`) è la regola completa con cui S-02 dovrà confrontare l'output del parser: NFC, spazi collassati, sillabazioni unite, ordine di lettura a colonne, esclusioni. La derivazione (`derivations/parser-it-text.md:65`) spiega perché il riquadro «WEB» e la didascalia sono inclusi. È la questione di TODO-01, chiusa dall'utente con «teneteli».
   - La tabella P-007 inizia a `parser-it.json:153`: `source_ref` porta sia la pagina fisica (12) sia quella stampata («4»), e `expected_value` ha titolo, `headers` appiattiti e celle `{row, col, text}`. La derivazione corrispondente è a `derivations/parser-it-tables.md:10`.
   - P-011 (`parser-it.json:1492`, derivazione a `parser-it-tables.md:97-130`) elenca le 25 pagine dell'estratto TTQ una per una, con il motivo per cui ciascuna è o non è una pagina con tabella. Il risultato è una sola pagina, la 12. La conseguenza è scritta a `parser-it-tables.md:129`: la recall dell'euristica che sceglie le pagine per Docling poggia su un solo positivo.

5. **`spikes/data/corpus/oracles/doublecheck/doublecheck_1.md`** (e gli altri sei) contiene ciò che è uscito verso GPT e ciò che è tornato.
   - La parte iniziale (`doublecheck_1.md:1-5`) dice a chi è andata la richiesta e con quale `content_id`.
   - Segue la richiesta verbatim, con convenzioni, regola di complessità, visibilità e oracoli (da `:7`), e il testo integrale dei sintetici citati.
   - La risposta di GPT inizia a `:831`, con l'attribuzione (`:833`) e la firma di trascrizione con il `delivery_token` (`:860`).

### Il flusso

Il derivatore legge il documento sorgente, scrive voce JSON e derivazione e compila l'asse Claude (commit `7e29c99`). Poi invia i lotti a GPT (`65f66dc`) e riporta per ogni oracolo il verdetto della risposta, aprendo un TO-DO per ogni dissenso (`707163b`). Infine si ferma: l'utente ratifica, e la ratifica diventa `guardrail.user` (`54f323d`). Fra il primo e l'ultimo commit cambiano solo i campi del guardrail, `todo_ref`, `gpt_ref` e `ratified_at`. Valore, passaggi, complessità, ruolo e testo della domanda restano identici.

## Perché

**Perché tre controlli e non uno.** Un oracolo sbagliato non produce un errore visibile: produce una misura sbagliata, e quindi una decisione sbagliata su quale parser, quale modello di embedding o quale soglia adottare. La regola di `skills/eval-oracle-check` chiede tre controlli indipendenti. Il derivatore legge il documento. GPT è un verificatore di un altro provider, che non condivide gli errori sistematici di Claude. L'utente è l'unico che può ratificare, e la ratifica non si deduce. Solo `CONFIRMED` è verde: un `PARTIALLY_CONFIRMED` non è «quasi verde» ma un dissenso, e resta fuori finché non si chiude. È il motivo per cui Q-008, Q-010, Q-012, Q-040, Q-048 e Q-056 sono escluse anche dopo che l'utente ha deciso di tenere la regola di complessità. La decisione chiude la domanda di dominio, non cancella il verdetto di GPT. Per recuperarle serve un nuovo double-check con la regola scritta come criterio esplicito.

**Perché mai allineare il valore al verificatore.** Se dopo un dissenso si correggesse l'oracolo nel senso di GPT, il secondo controllo diventerebbe un generatore e smetterebbe di essere un verificatore. Per questo il piano vieta l'allineamento, e il bridge lo ha controllato confrontando i due JSON fra `7e29c99` e HEAD: diff zero su tutte le 91 voci (RISK-10).

**Perché le classi di provenienza.** La classe dice che tipo di prova sostiene il valore, e quindi quale controllo serve.
- `estratto-sorgente` vuol dire «è scritto lì»: basta un estratto e un punto preciso.
- `derivazione-documentata` vuol dire «ci si arriva con un ragionamento». Vale per un rifiuto per assenza, per un confronto, per la regola che decide cosa è una tabella. Il ragionamento deve essere scritto, e per questo `derivation_ref` è obbligatorio.
- `ratifica-utente` è per i valori che solo l'utente può decidere; per ora nessuna voce la usa.

Le classi `hand-crafted` e `generato-dalla-pipeline` non sono ammesse. Il controllo usa una whitelist chiusa invece di un elenco di classi vietate, perché un refuso come `estratto_sorgente` passerebbe un elenco di divieti e non passa una whitelist (RISK-3).

**Perché gli oracoli non si generano con la pipeline.** Se il testo atteso di una pagina venisse dall'estrattore di testo, il CER di S-02 misurerebbe quanto il parser somiglia all'estrattore, non quanto è fedele alla pagina. Se i passaggi attesi venissero dal retrieval, recall e nDCG misurerebbero il retrieval contro sé stesso. È il rischio CRITICAL del test design (RISK-1). Per questo i PDF sono stati letti come immagini: l'alternativa più comoda, `pdftotext`, è esattamente la classe di strumenti che S-02 deve misurare, e il suo errore sarebbe finito dentro l'oracolo. Il prezzo è che sui PDF non c'è un controllo meccanico che l'estratto sia verbatim. Lo coprono GPT e la ratifica, e lo coprirà il campione manuale di TEST-3, che resta da fare.

**Altre scelte.**
- *Estratti PDF brevi, una frase con il numero, invece di paragrafi interi.* Una trascrizione a vista sbaglia di più quanto più è lunga.
- *Un generatore unico per JSON e derivazioni, invece di due file scritti separatamente.* Così JSON e Markdown non possono divergere. Lo script non è versionato; i controlli del bridge sono stati riscritti da zero.
- *Un solo TO-DO per i sei dissensi di complessità.* È la stessa domanda, quindi una sola decisione.
- *Lotti con il testo integrale dei sintetici, invece dei soli estratti.* Senza il documento intero GPT non potrebbe confermare un rifiuto per assenza.
- *Pagina fisica 1-based, invece del numero stampato.* Il numero stampato non è univoco: nella Relazione annuale la pagina fisica 11 è la «3».

**Il legame con il resto della fase.** Le query portano il ruolo perché S-03, S-04 e A-04 devono dimostrare che un passaggio invisibile al ruolo non arriva mai a reranker, JEV o LLM (D-16). Le quattro trappole su `syn-09` sono la prova in negativo. La complessità serve al routing deciso da JEV in A-04 (D-18). Gli oracoli stanno nel corpus comune in sola lettura e non in `data/s03/`, perché li usano tre slice (WIRING §1).

## Test

Non ci sono test nel workspace: il `test-write` del piano è `NOT_APPLICABLE` (`.planning/tmp/test-write-01-06.yaml`), perché tutti i TEST-N del design sono controlli sui dati prodotti dal piano o controlli manuali. Si lanciano dalla radice del repository.

- **Verify del Task 1** (TEST-1, TEST-2, TEST-8, TEST-13): i due `node -e` nel blocco `<automated>` del Task 1 del PLAN. Il primo controlla distribuzione, classi in whitelist, chiavi del guardrail e numeri come stringhe. Il secondo cerca ogni estratto su Markdown dentro il file sorgente, dopo NFC e collasso degli spazi. Non serve `raw/`.
- **Verify del Task 2** (TEST-5, prima parte; TEST-11): il `node -e` del Task 2. Ogni oracolo deve avere `guardrail.gpt`, ogni non-`CONFIRMED` deve avere `todo_ref`; poi conta i file `doublecheck_*.md`, che sono 7.
- **Verify del Task 3** (TEST-5): il `node -e` del Task 3. Servono almeno 60 query con tre assi verdi (sono 74), e ogni voce non ratificata deve avere `todo_ref`.
- **TEST-4**: `grep -rniE "pdftotext|pdfium|docling|tesseract|tei|embed|rerank|systemone|anthropic|results/" spikes/data/corpus/oracles/derivations/ spikes/data/corpus/oracles/ORACOLI.md`. Ogni occorrenza è una regola o un consumatore, mai una fonte.
- **TEST-12**: ``grep -nE '```(rust|bash|json)|system prompt|cargo ' spikes/data/corpus/oracles/doublecheck/*.md``. Trova solo la fixture `inj-08-d` di `syn-08`, ripetuta nei quattro lotti che includono il sintetico.
- **TEST-6, TEST-7, TEST-9** (coerenza fra `no_answer` e visibilità, integrità referenziale, motivazione della complessità): il bridge li ha riscritti in uno script nello scratchpad, non versionato. La logica è quella del test design. La visibilità di ogni ruolo si calcola da `acl.json` più la `classification` del manifest, mai dalla tabella che implementerà il filtro.
- **Anti-allineamento (TEST-11)**: `git show 7e29c99:spikes/data/corpus/oracles/queries.json` confrontato voce per voce con il file attuale, sui campi di valore. Lo stesso confronto si fa per `parser-it.json`.
- **TEST-3 e TEST-10** non hanno un comando. TEST-3 è il campione manuale sui PDF, da fare. TEST-10 è la deviazione sul derivatore, accettata dall'utente.

## Deviazioni e punti aperti

**Il derivatore non è un agente fresco.** Il piano chiedeva che gli oracoli li derivasse un agente distinto anche dall'esecutore del piano. Li ha derivati invece la stessa sessione `gsd-executor`, perché da lì non era possibile avviare un sotto-agente. L'utente ha accettato la deviazione il 2026-10-21. L'indipendenza che conta per gli oracoli è quella dalla pipeline e dagli esecutori degli spike, e quella è rispettata. TEST-10 resta registrato come deviazione accettata, non come verde.

**AgentBridge via REST.** Il tool MCP del bridge non era caricabile, perché nella sessione non c'era ToolSearch. L'esecutore ha usato lo stesso contratto via REST sul loopback. La skill caveman non era caricabile per lo stesso motivo, e il rapporto è in YAML sintetico. Sono deviazioni di livello a e non toccano i dati.

**Cosa resta aperto.**
- **TEST-3.** Il campione manuale di 10 query su PDF e 2 pagine del parser, confrontati carattere per carattere con il PDF, non è stato fatto. Gli estratti dai PDF sono la parte meno protetta: un errore di lettura a vista che anche GPT avesse confermato passerebbe nelle misure.
- **Le sei query di TODO-02.** Restano fuori finché non le si reinvia a GPT con la regola di complessità esplicita. Fino ad allora A-04 misura il routing su 74 query.
- **La recall di P-011.** L'estratto TTQ ha una sola pagina con tabelle, quindi la recall dell'euristica di S-02 vale 0 o 1. Le pagine 5-8, con indice e avvertenze in colonne allineate, sono i negativi difficili. Se serve una misura più fine, va scelta un'altra porzione del documento.
- **I rifiuti sui PDF.** L'assenza di evidenza nei PDF pubblici poggia su CORPUS.md: Alfa Componenti è inventata, e i nomi della trappola li ha controllati 01-03. Questo piano non ha riletto i PDF per confermarla.

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-06
