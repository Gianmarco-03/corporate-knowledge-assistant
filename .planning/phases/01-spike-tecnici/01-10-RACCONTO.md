# Racconto del piano 01-10: modello di embedding, serving TEI su GPU e ricerca autorizzata (S-03, parte embedding)

## Cosa

Per rispondere a una domanda, il prodotto deve trovare i passaggi dei documenti che parlano di quell'argomento anche quando le parole non coincidono. Lo fa con gli *embedding*: un modello trasforma ogni pezzo di testo (un *chunk*) in un vettore di 1.024 numeri, e testi di significato simile finiscono in vettori vicini. A ogni domanda si calcola il vettore della domanda e si cercano i chunk più vicini. Questo piano doveva dire quale modello usare, come servirlo sulla macchina, e scrivere la prima versione della ricerca che non restituisce mai un chunk che l'utente non può vedere.

Il piano ha confrontato i tre candidati di D-14 sulle stesse 60 query etichettate (24 in italiano, 24 in inglese, 12 cross-lingual, cioè domanda in una lingua e risposta nell'altra), con il modello servito da TEI (Text Embeddings Inference, il server di Hugging Face) su GPU.

I risultati, in breve:

- **bge-m3 vince** con nDCG@10 complessivo 0,7226, contro 0,6408 di Qwen3-Embedding-0.6B (0,6595 senza istruzione) e 0,6180 di multilingual-e5-large-instruct. Il distacco supera il margine di pari di 0,02 scritto prima della misura.
- **bge-m3 passa tutte le soglie**: l'italiano perde solo 0,031 rispetto all'inglese, il cross-lingual va addirittura meglio della media, una query costa 12 ms al p95, il documento da 25 pagine (69 chunk) 1,02 s, la VRAM 1.453 MiB.
- **Nessun leak**: 0 chunk non visibili al ruolo, su 74 query e su 1.479 candidati esportati.
- **L'istruzione di query** pesa molto su e5 (+0,143) e nulla su Qwen3 (−0,019, leggermente peggio). Basta questo a dire che la porta `Embedder` deve distinguere l'embedding dei documenti da quello della domanda.
- **Su CPU** lo stesso modello impiega 24 s per il documento da 25 pagine, fuori dal Time To Queryable di 15 s: la GPU serve.

Il piano porta PLAT-07 per la parte «con il modello di embedding scelto» del criterio 3. La scelta l'hai già registrata tu oggi come D-38 (`01-CONTEXT.md:77`); ADR-0007 resta `Proposed` fino a verify.

## Come

### Percorso di lettura consigliato

L'ordine segue il viaggio di un documento: diventa chunk, i chunk diventano vettori, i vettori vanno nel database, una domanda li cerca con il filtro dei ruoli, e le metriche dicono se ha trovato quelli giusti.

1. **`spikes/s03-pgvector/src/lib.rs`**: la mappa. Quattro moduli pubblici (`lib.rs:10-13`) e l'enum `Model` con le tre varianti `Qwen3`, `BgeM3`, `E5` (`lib.rs:16-21`). È un **enum chiuso**: il compilatore conosce tutti i valori possibili, e ogni `match` su di esso deve trattarli tutti. Da questo enum derivano sia il nome della tabella sia l'istruzione di query, così nessuno dei due può arrivare da una stringa di input. Il file l'ha scritto l'agente dei test rossi; l'esecutore non l'ha cambiato.

2. **`spikes/s03-pgvector/src/chunk.rs`**: dal documento ai chunk.
   - `windows` (`chunk.rs:85-100`) calcola le finestre `[start, end)` su un testo: al più 1.600 caratteri, e ogni finestra riparte 200 caratteri prima della fine della precedente (`start = end - OVERLAP_CHARS`, `:97`). La sovrapposizione serve perché una frase tagliata a metà al bordo di un chunk compaia intera nel successivo.
   - `chunk_pages` (`:124-140`) spezza il testo di un PDF sul carattere *form feed* (`'\u{c}'`, `:126`), che `pdftotext` mette fra una pagina e l'altra, e applica le finestre a ogni pagina separatamente: un chunk non sta mai su due pagine, quindi ha un solo numero di pagina da citare. Gli offset sono in **caratteri** e non in byte (`page_text.chars().collect()`, `:128`): in UTF-8 «à» o «€» occupano più byte, e un offset in byte cadrebbe a metà di un carattere.
   - `chunk_markdown` (`:178-232`) fa lo stesso per sezione. Riconosce le intestazioni ATX (`heading`, `:143-154`), tiene una pila delle intestazioni aperte e la svuota quando arriva un'intestazione di livello uguale o superiore (`:207-209`), così `section_path` riflette l'annidamento. Il front matter YAML iniziale si salta (`:189-194`): `if … && let Some(close) = …` è una **let-chain**, un `if let` e una condizione booleana nella stessa espressione.
   - `section_path` (`:159-174`) lascia fuori l'H1, il titolo del documento, perché i dati degli oracoli lo omettono. È una delle scelte del piano, spiegata in «Perché».
   - `stable_id` (`:63-80`) ricava l'id del chunk da documento, pagina o sezione e offset con un hash FNV-1a a 128 bit, poi lo impacchetta come UUID versione 8 (`uuid::Builder::from_custom_bytes`, `:79`), la versione che lo standard riserva agli UUID costruiti dall'applicazione. `wrapping_mul` (`:77`) moltiplica lasciando che il risultato «giri» oltre il massimo di `u128` invece di andare in panico: è quello che l'algoritmo FNV prevede.
   - `pdftotext_layout` (`:268-283`) lancia `pdftotext -layout -enc UTF-8` e legge l'uscita con `String::from_utf8_lossy` (`:282`), che sostituisce le sequenze non valide con U+FFFD invece di fallire.

3. **`spikes/s03-pgvector/src/tei.rs`**: dai chunk ai vettori.
   - Qui sta l'**istruzione asimmetrica**. Alcuni modelli sono addestrati a ricevere la domanda con un'istruzione davanti («Instruct: dato una query, trova i passaggi che rispondono…\nQuery: <domanda>») e i documenti senza. L'asimmetria serve al modello per capire che una domanda breve e un paragrafo lungo vanno messi vicini se l'uno risponde all'altro, non se si somigliano. `query_text` (`tei.rs:21-26`) aggiunge il prefisso solo per Qwen3 ed e5 e solo se richiesto: il pattern `(Model::Qwen3 | Model::E5, true)` è un **pattern su tupla con alternativa**, che corrisponde se il modello è uno dei due *e* l'istruzione è attiva.
   - `documents_body` (`:29-31`) non ha nemmeno il parametro del modello: i documenti non possono ricevere istruzioni per costruzione. Entrambi i corpi chiedono `normalize: true` (vettori di lunghezza 1, così la distanza coseno è ben definita) e `truncate: true`.
   - `Client` (`:46-141`) parla con TEI: `embed_documents` (`:114-124`) manda i testi a lotti con `texts.chunks(self.batch)` (qui `chunks` è il metodo delle slice di Rust, che spezza un array in pezzi, non i chunk del corpus); `embed_query` (`:127-140`) manda una sola domanda. `post_embed` (`:93-111`) rifiuta un vettore di dimensione diversa da 1024 (`:107-109`). I metodi sono `async`: restituiscono un *future* che fa la richiesta HTTP solo quando qualcuno lo attende con `.await`, lasciando il thread libero nel frattempo.

4. **`spikes/s03-pgvector/sql/schema.sql`** e **`spikes/s03-pgvector/src/db.rs`**: i vettori nel database e la ricerca autorizzata.
   - Lo schema (`schema.sql:14-67`) ha `doc`, `doc_acl` (quali ruoli vedono quale documento, con indice `(role, doc_id)` a `:25`) e una tabella di chunk per modello, con `emb vector(1024)` di pgvector e un `tsvector` generato per la ricerca testuale dei piani successivi.
   - **`per_model!`** (`db.rs:23-31`) è una macro dichiarativa: a tempo di compilazione si espande in un `match` su `Model` che restituisce una stringa SQL costruita con `concat!`, cioè solo da letterali. Il nome della tabella non passa mai per una stringa a runtime, quindi non c'è modo di iniettarlo.
   - **`exact_search`** (`:278-309`) è il cuore del piano. La query (`:285-292`) è:
     `SELECT … FROM chunk_<modello> c JOIN doc d … WHERE EXISTS (SELECT 1 FROM doc_acl a WHERE a.doc_id = c.doc_id AND a.role = ANY($2)) ORDER BY c.emb <=> $1, c.id LIMIT $3`.
     `<=>` è l'operatore di pgvector per la distanza coseno. È una **ricerca esatta**: senza indice approssimato, il database calcola la distanza da tutti i chunk autorizzati e prende i più vicini, così le metriche misurano il modello e non un indice che potrebbe perdere risultati. Il **predicato ACL nella stessa query** fa sì che il `LIMIT` si applichi *dopo* il filtro: se si prendessero prima i 10 più vicini e poi si togliessero quelli non autorizzati, un utente con pochi documenti visibili riceverebbe meno di 10 risultati, o nessuno, e un errore nel post-filtro farebbe uscire chunk proibiti. L'ordine secondario `c.id` rende il risultato deterministico a parità di distanza. I valori passano solo con `.bind` (`:293-296`): vettore, ruoli e `k` arrivano al database come parametri, mai incollati nel testo SQL.
   - `acl_diff` e `check_doc_acl` (`:166-198`) confrontano `doc_acl` letta dal database con la matrice attesa calcolata dai file del corpus, e fanno fallire il comando con l'elenco delle differenze. `insert_chunks` (`:222-273`) inserisce 500 righe per istruzione con `INSERT … SELECT … FROM UNNEST($1::uuid[], …, $9::vector[])`: ogni colonna arriva come un array e `UNNEST` le ricompone in righe. `export_candidates` (`:389-426`) scrive i top-20 autorizzati di ogni query in JSONL.

5. **`spikes/s03-pgvector/src/metrics.rs`**: le metriche e le loro verità.
   - `load_ratified_queries` (`metrics.rs:67-82`) tiene solo gli oracoli con i tre assi verdi e senza `todo_ref` (`is_ratified`, `:57-64`). `Option::is_none_or` (`:62`) vale vero se il campo manca oppure se la condizione sul valore è vera.
   - `expected_visibility` (`:107-125`) costruisce in memoria, da `acl.json` e dal manifest, la mappa ruolo → documenti visibili. `leaked_chunks` (`:129-135`) conta i chunk restituiti che quella mappa non autorizza: un ruolo sconosciuto o un documento assente valgono come leak (*deny by default*, `:133`).
   - La **regola di rilevanza** (`relevant_to`, `:236-251`) dice quando un chunk restituito è «giusto»: stesso documento e stessa pagina o sezione dell'oracolo (`same_place`, `:217-231`), e dopo la normalizzazione (`normalize`, `:142-195`) contiene l'estratto atteso, oppure almeno metà dei suoi caratteri come sottostringa contigua (`2 * longest_common_substring(…) >= ex.len()`, `:250`), per il caso in cui l'estratto è stato tagliato al bordo di due chunk. `longest_common_substring` (`:198-213`) è la programmazione dinamica classica a due righe di memoria; `std::mem::swap` (`:210`) scambia le due righe senza copiarle.
   - **Le metriche** (`:262-296`). Per ogni query la ricerca restituisce una lista ordinata di 10 chunk, e per ognuno si sa se è rilevante.
     - *recall@k* (`:262-268`): quanti dei chunk rilevanti del corpus stanno nei primi k.
     - *MRR@10* (`:271-277`): 1 diviso la posizione del primo rilevante (1 se è primo, 0,5 se è secondo, 0 se non c'è).
     - *nDCG@10* (`:281-296`) guarda tutta la lista e premia i rilevanti in alto. Ogni rilevante in posizione i vale 1/log2(i+1): 1 in prima posizione, 0,63 in seconda, 0,5 in terza, sempre meno scendendo. La somma (DCG) si divide per la somma della lista ideale, con tutti i rilevanti in testa, così il risultato sta fra 0 e 1. Esempio del test (`:308-329`): due rilevanti, trovati in posizione 1 e 3, danno (1 + 0,5) / (1 + 0,63) ≈ 0,92. È la metrica della regola di scelta perché distingue un modello che mette la risposta prima da uno che la mette ottava, cosa che recall@10 non vede.

6. **`spikes/s03-pgvector/src/main.rs`**: la CLI che mette insieme i pezzi.
   - `Cli` e `Cmd` (`main.rs:60-127`) usano le derive di `clap`: la struttura dei sottocomandi e delle opzioni si dichiara come tipo Rust e `clap` genera il parser.
   - `load` (`:277-329`): schema, documenti, `doc_acl` dalla matrice attesa e controllo immediato (`:288-290`), chunk, embedding, insert, e `chunks_manifest.json` con la revisione del modello letta da `/info` (`:321-322`).
   - `query_bench` (`:431-600`): controllo di `doc_acl` prima di tutto (`:441`), poi per ogni query il totale dei rilevanti sull'intero corpus (`:457-460`), l'embedding della domanda, la ricerca con il solo ruolo dell'oracolo (`std::slice::from_ref`, `:463`, trasforma un riferimento a un valore in una slice di un elemento senza copiarlo), rilevanza e metriche, e il leak a 5 e a 10 (`:478-479`). Una query è valutabile solo se non è `no_answer` e ha almeno un rilevante nel corpus (`:473`). Le medie per lingua vanno in `models.csv` e `query_instruction.csv` con `upsert_csv` (`:230-261`), che sostituisce le righe con la stessa chiave: rilanciare una misura non duplica righe.
   - `throughput` (`:626-693`): embedding dei 69 chunk del documento da 25 pagine cronometrato, poi 200 query una alla volta in un istogramma (`:649-656`), VRAM dal log di `nvidia-smi` meno il valore a riposo (`vram_peak`, `:616-624`).
   - `export` (`:697-772`): candidati top-20, controllo del leak sulle righe prodotte e rifiuto di scrivere se ce n'è uno (`:721-740`), file con il testo solo in `results/raw/`, manifest versionato con conteggi e sha256 (`:747-765`).

### Flusso dei dati

`load` legge il manifest del corpus, chunka PDF e Markdown, manda i testi a TEI e inserisce chunk e vettori nella tabella del modello. `query-bench` rilegge i chunk, verifica `doc_acl`, calcola per ogni query ratificata il vettore della domanda, interroga `exact_search` con il ruolo dell'oracolo e scrive le metriche per lingua. `throughput` misura velocità, latenza e VRAM su una griglia di batch e `--max-batch-tokens`, con `tei-embed` riavviato a ogni configurazione. `export-candidates` produce i top-20 di bge-m3 per il reranker. Il tutto gira nel runner della sessione prenotata con `session.sh`, con Postgres e TEI della stessa sessione.

## Perché

- **Soglie, regola di rilevanza e metodo scritti prima** (RISK-5). La regola che decide se un chunk è «giusto» cambia i numeri, e sceglierla dopo aver visto chi vince sarebbe facile anche senza volerlo. Domanda, Soglie, Regola e Metodo (righe 1-53 di `RISULTATI-embedding.md`) entrano in `b4d3321` e sono identici a HEAD; il primo file di risultati arriva in `1cbf84c`. Anche l'esclusione delle query irraggiungibili è scritta lì (`RISULTATI-embedding.md:47`), prima di sapere quali fossero.
- **Il leak si conta con una verità diversa dal filtro** (RISK-2). Se il contatore leggesse la stessa `doc_acl` che filtra la query, una `doc_acl` popolata male produrrebbe insieme la fuga e un contatore a zero. Per questo `leaked_chunks` usa la matrice calcolata dai file del corpus, e `doc_acl` si confronta con quella matrice prima di ogni misura: due controlli indipendenti che devono dare lo stesso risultato.
- **Predicato ACL nella stessa query invece di un post-filtro** (RISK-1, D-12). Il test TEST-1 (`db.rs:618`) lo rende visibile: cinque chunk riservati a distanza zero e cinque pubblici lontani; un dipendente deve ricevere tre pubblici, e un post-filtro dopo `LIMIT 3` gli restituirebbe zero righe.
- **Ricerca esatta per le metriche.** Un indice approssimato (HNSW) può perdere risultati, e allora un modello sembrerebbe peggiore per colpa dell'indice. L'indice e la sua soglia sono del piano 01-14, che userà questa ricerca come riferimento.
- **Istruzione come prefisso esplicito** invece del `prompt_name` di TEI. `prompt_name` dipende dai nomi definiti nel config di ciascun modello (punto non verificato di RESEARCH §11.10); il prefisso rende la richiesta identica per Qwen3 ed e5 e visibile nel codice. Il prezzo: per Qwen3 non sono state provate varianti del prompt, e il −0,019 può dipendere dalla forma. Anche la sua variante migliore resta 0,063 sotto bge-m3.
- **`section_path` senza H1.** ORACOLI.md riga 39 dice che le intestazioni sono quelle «come compaiono nel file», ma i dati di `queries.json` omettono il titolo del documento (per esempio `syn-06`). L'alternativa era tenere l'H1 e rendere tollerante il confronto in `relevant_chunks`, cioè cambiare la regola di rilevanza dopo averla fissata. L'esecutore ha preferito allineare il chunker ai dati: oracoli e regola restano quelli ratificati, e l'incoerenza è registrata per 01-06.
- **Q-029 e Q-047 escluse per tutti.** Sono domande con una risposta nel corpus, ma su pagine a riquadri e colonne affiancate: `pdftotext -layout` mescola le colonne e l'estratto atteso non sopravvive come sottostringa contigua. Contarle come zero avrebbe abbassato tutti e tre i modelli allo stesso modo, misurando il chunker e non il modello. È lo stesso problema dell'ordine di lettura emerso in S-02 (01-08).
- **Id deterministici e ordine totale nella SQL** (RISK-9). Il piano 01-15 rigenera i candidati nel proprio worktree e deve ottenere lo stesso sha256; con UUID casuali o con un ordinamento instabile a parità di distanza non sarebbe possibile.
- **Tabella per modello da enum** (RISK-3). Tre tabelle con nomi fissi invece di una tabella con colonna «modello» permettono di caricare i tre modelli insieme senza mescolare vettori di spazi diversi; il nome viene da `per_model!`, mai dall'input.
- **sha256 con `sha256sum`.** Il `Cargo.toml` del crate è fuori dai file del piano (insiemi di file chiusi), quindi niente nuova dipendenza `sha2`.
- **bge-m3 invece del default Qwen3.** La regola di M-1 l'ha deciso: qualità complessiva più alta, cross-lingual molto più alto (0,7968 contro 0,5837 e 0,4181), e servizio 2,2 volte più veloce di Qwen3. e5 è il migliore sull'italiano e il più veloce, ma crolla sul cross-lingual e dipende molto dal prefisso. Tu l'hai registrato come D-38.

## Test

Diciassette test unitari e cinque d'integrazione li ha scritti un agente test-write distinto dall'esecutore, prima del codice (`0df057a`); l'esecutore ne ha aggiunti tre. Il bridge ha confrontato i blocchi di test fra `0df057a` e il merge `69af2c9`: identici in `metrics.rs` e `tei.rs`, solo aggiunte in `chunk.rs` e `db.rs`.

- **`metrics.rs#tests`, 12 casi.** TEST-7: valori calcolati a mano di recall, MRR e nDCG (`metrics.rs:308`), zero senza rilevanti (`:332`), nessun NaN per le query senza risposta (`:340`). TEST-8: estratto intero, al 50% e al 49%, pagina o documento sbagliati (`:401`), NFC e spazi (`:448`), sezione Markdown (`:472`). TEST-3: leak contato con la matrice e non con `doc_acl` (`:513`), matrice coincidente con `acl.json` e manifest (`:533`), deny by default su classificazioni sconosciute (`:563`). TEST-17: solo oracoli ratificati (`:596`, `:615`), e sul `queries.json` reale 74 tenuti e 6 esclusi (`:650`).
- **`chunk.rs#tests`, 4 casi.** TEST-10: nessun chunk su due pagine, sovrapposizione di 200, offset in caratteri (`chunk.rs:336`); sezioni Markdown separate e annidate (`:394`); id stabili (`:433`); front matter saltato e H1 come solo titolo (`:448`).
- **`tei.rs#tests`, 2 casi.** TEST-11: istruzione per modello e flag (`tei.rs:171`), documenti mai con istruzione (`:222`).
- **`db.rs`, 2 unitari e 5 d'integrazione.** Unitari: tabella da enum chiuso (`db.rs:609`, TEST-6) e `acl_diff` (`:435`). D'integrazione, marcati `#[ignore]` perché vogliono il Postgres della sessione: TEST-1 (`:618`), TEST-2 con 100 chunk riservati più vicini e 10 pubblici (`:659`), TEST-5 con un ruolo ostile `employee' OR '1'='1` che deve dare zero righe (`:690`), TEST-13 su parità di distanza ordinate per id (`:707`) ed esportazione deterministica (`:736`). Ognuno lavora in uno schema temporaneo `test_<pid>_<uuid>` che crea e poi elimina (`Scratch`, `:477-540`), così non tocca le tabelle delle misure.

**Come lanciarli.** I test unitari sull'host, dalla cartella `spikes`:

```bash
cargo test -p s03-pgvector --locked --offline
```

Un test solo si lancia aggiungendone il nome, per esempio `cargo test -p s03-pgvector metrics_known_values`. Quelli d'integrazione girano nel runner, dentro una sessione con Postgres:

```bash
bash tools/session.sh with 01-10 -- bash -c 'bash tools/session.sh up "$CKA_RUN_ID" postgres && bash tools/session.sh oneshot "$CKA_RUN_ID" -- cargo test -p s03-pgvector -- --ignored --test-threads=1 db::tests'
```

**Esito.** Sull'host il bridge ha rieseguito i test: 20 verdi, 5 ignorati. Il gate d'onda nel runner, su main dopo il merge, ha dato 5 su 5 per i test d'integrazione; fmt e clippy verdi; lo scanner di sicurezza non trova nulla sui 12 file del crate; il gate di proprietà dei file è conforme.

**Controlli sulle evidenze**, rifatti dal bridge: nessuna riga di `models.csv` con leak diverso da zero; le righe `all` coincidono con la media pesata di italiano, inglese e cross-lingual; la regola di configurazione delle Soglie, applicata a `throughput.csv`, ritrova esattamente le configurazioni e i numeri di RISULTATI; la revisione di ogni modello registrata al caricamento coincide con quella fissata; nessun file di `results/raw` è versionato.

**Cosa non provano.** Le evidenze grezze (le copie di `/info` a ogni riavvio, i CSV per query con gli id delle due query irraggiungibili, la seconda esportazione dei candidati) stavano in `results/raw`, ignorata da git, e il worktree del piano è stato rimosso: quelle affermazioni restano dell'esecutore e nel SUMMARY sono marcate `human_judgment`.

## Deviazioni e punti aperti

- **Copia derivata del file segreti.** Dalla gap-closure 1.1, `session.sh` toglie a compose le variabili dell'host che hanno lo stesso nome di una chiave del file segreti; `EMBED_MODEL`, `EMBED_REVISION` e `TEI_MAX_BATCH_TOKENS` lo sono, quindi il comando del piano avviava sempre lo stesso modello. L'esecutore ha puntato `CKA_SPIKES_ENV` a una copia fuori dal repo, `$HOME/.cka-spikes/spikes-01-10.env`, con le sole righe di TEI cambiate, e l'ha cancellata alla fine (`RISULTATI-embedding.md:134`). Ho controllato: il file non c'è più, e nessun valore segreto compare nei file versionati. Il comportamento di `session.sh` è un fatto da tenere presente per i piani che cambiano parametri di servizio fra una misura e l'altra.
- **`hw.json` normalizzato**: lo stesso difetto noto di `hw-manifest.sh` (01-02) contro `common::manifest` (01-01), corretto a valle come negli altri piani; correzione in una fase decimale.
- **`pdftotext` dell'host** produceva byte non UTF-8: ora la lettura è tollerante (`chunk.rs:282`); le misure usano il `pdftotext` del runner.
- **TEI su CPU** lanciato con `docker run`, etichettato con la sessione e fermato con `session.sh stop`: non osservabile a posteriori.
- **Tre piccole discrepanze** che non cambiano alcun esito, scritte nel SUMMARY: la latenza CPU di 50,21 ms include la ripetizione scartata (quella utile è 49,95); il «distacco 0,063» è misurato contro Qwen3 senza istruzione (con istruzione è 0,082); D-38 parla di 72 query, mentre le prove dicono 74 ratificate e 60 valutabili.
- **Da decidere o da fare dopo**: ADR-0007 passa ad `Accepted` e la riga «Embedding model» di `DEPENDENCIES.md` a Decisa nel consolidamento (01-17); ORACOLI.md riga 39 va riallineata a `queries.json` in 01-06; le due query irraggiungibili tornano in gioco solo con un parser che ricostruisca l'ordine di lettura, materia dell'ADR-0004; con 12 query cross-lingual il campione è piccolo, e in italiano e5 resta avanti di 0,027.

**Modifica:** Claude (Anthropic) · 2026-10-23 · creazione (bridge di 01-10)
