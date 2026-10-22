# Racconto del piano 01-11: forma del rank JEV, concorrenza, pin e alias (S-04)

## Cosa

Il RAG & Answer Service userà JEV per riordinare i passaggi trovati dal retrieval. Prima di questo piano non sapevamo quale forma di chiamata convenisse, quante richieste si potessero tenere in volo senza essere frenati da JEV, né a quale versione puntassero davvero gli alias. Adesso c'è uno spike, `s04-jev`, che è insieme una libreria (client JEV, insiemi di candidati, metriche) e una CLI di misura. Il piano 01-15 riuserà la libreria.

I risultati, con le soglie scritte prima delle misure:

- **la forma A vince**: una richiesta per candidato, con domande Noul, ordina perfettamente i 30 insiemi di prova (nDCG@5 1,0 e recall@8 1,0). La latenza del rank di 20 candidati ha un p95 di 606 ms a concorrenza 16. La forma B (uno `state` con 20 passaggi e 20 domande) si ferma a 0,679, la C (una sola domanda Choice) a 0,729;
- **concorrenza**: a 4, 8 e 16 richieste in volo, 3 run da 200 richieste ciascuno, non c'è stato nessun 429 né 529. Per la regola scelta prima, la concorrenza è 16;
- **alias**: `jev-latest` risolve oggi a `jev-1.13.0`, `jev-preview` già a `jev-1.14.0`. Il pin `jev-1.13.0` ha risposto con sé stesso in tutte le 3.803 risposte;
- **sicurezza e costo**: nessun passaggio non visibile al ruolo della query è partito (`unauthorized_sent` 0 su 273 righe), e la spesa è stata 0,078 USD su un tetto di 5;
- **varianza**: un solo candidato su 600 ha superato la soglia di 0,05 (std 0,0954). Per la regola pre-registrata questo non boccia la forma, ma diventa un vincolo per la calibrazione: le soglie sui Noul vanno tenute ad almeno ±0,1 dal valore di taglio.

Da qui ADR-0008, `Proposed`. Il piano porta PLAT-07 per la parte «la forma della chiamata JEV è misurata» del criterio 4 della fase.

Due precisazioni sul valore delle misure. La prima: JEV è reale, e il testo inviato è quello del corpus ratificato (74 oracoli verdi, documenti pubblici o sintetici), non un testo inventato. La seconda: il rilevamento di `instructs_model` sul testo di injection **non è stato misurato**, per un motivo spiegato in fondo. Si è misurato solo che non dà falsi allarmi.

## Come

### Percorso di lettura consigliato

1. **`spikes/s04-jev/RISULTATI-jev.md`**, prima del codice. Le soglie (`RISULTATI-jev.md:20-36`) sono nel commit `28d8ea7` delle 17:22; la prima sessione di misura parte alle 15:25:12Z, cioè le 17:25 ora locale. Il bridge ha controllato che la tabella e le regole di lettura siano identiche fra `28d8ea7` e la versione finale. Poi Setup (`:38`), Risultati (`:64`), Anomalie (`:114`), Esito (`:127`).

2. **`spikes/s04-jev/src/candidates.rs`**: chi può vedere cosa, e con quali passaggi si misura.
   - `Acl` (`candidates.rs:44-49`) tiene due mappe: documento → classificazione, presa dal manifest, e ruolo o profilo → classificazioni visibili, prese da `acl.json`. `from_json` (`:53`) le costruisce e rifiuta un `doc_id` duplicato o un nome che sia insieme ruolo e profilo.
   - `is_visible` (`:116-124`) è il cuore del **deny by default**. Il `match` su una coppia di `Option` ha un solo ramo che concede, `(Some(class), Some(visible))`, quando sia il documento sia il ruolo sono noti. Tutto il resto, `_ => false`, nega. Un documento sconosciuto non è mai «visibile perché non trovato».
   - `assert_visible` (`:153-168`) raccoglie **tutti** i passaggi negati e restituisce un errore che li nomina per id e documento, mai per testo, così l'errore stesso non può far trapelare contenuto. Conta la classificazione del manifest, non il campo `classification` del `Passage`: un passaggio che si dichiara `public` non diventa visibile per questo (test dedicato a `:486`).
   - `SplitMix64` (`:171-189`) è un generatore pseudo-casuale di poche righe, con un mescolamento di Fisher-Yates. Con lo stesso seme dà sempre la stessa sequenza, quindi gli insiemi di candidati sono riproducibili.
   - `is_green` (`:193`) accetta un oracolo solo se ha i tre assi verdi (Claude, GPT `CONFIRMED`, utente `ratified`) e nessun `todo_ref` aperto.
   - `build_candidate_sets` (`:290-414`) sceglie 30 query a turno fra `it`, `en` e cross. Scarta quelle che non riempiono 20 passaggi visibili: è il caso dell'unica query `restricted-only`, che ne vede 4. Per ogni query mette prima gli `expected_passages` (i rilevanti), poi al più 2 excerpt `injection_case` visibili, poi distrattori visibili scelti dal seme, e mescola. In fondo (`:400`) chiama già `assert_visible`: l'insieme nasce autorizzato.

3. **`spikes/s04-jev/src/jev.rs`**: il client. Conviene leggerlo in quest'ordine.
   - **Costanti** (`jev.rs:18-42`): il prezzo con la fonte e la data, `MAX_RETRIES = 1`, il backoff di 0,5 s, il timeout di connessione. Il wording delle tre forme entra nel binario con `include_str!`, una macro che legge un file a tempo di compilazione e lo incorpora come stringa. Lo sha256 del file (`:62`) va in ogni riga del CSV: se qualcuno cambia il wording, il cambio si vede.
   - **Costruttore** (`:306-340`): rifiuta un URL non `https://` (salvo `127.0.0.1` per i test), una chiave vuota e soprattutto un modello vuoto, quindi non esiste un default implicito a `jev-latest`. `https_only(!loopback)` impedisce a reqwest di seguire un redirect verso `http`.
   - **La guardia di visibilità prima del fan-out.** Ogni metodo pubblico di rank chiama `self.guard` sull'intero insieme **prima** di costruire la prima richiesta: `rank_form_a_each` a `:528`, `rank_form_b` a `:582`, `rank_form_c` a `:616`. `guard` (`:352`) traduce l'errore di `assert_visible` in `JevError::Unauthorized`. È una scelta tutto-o-niente: se un solo passaggio su 20 non è visibile, non parte nulla. Il test TEST-2 mette l'invisibile al decimo posto proprio per distinguere questa guardia da una guardia per singolo invio, che avrebbe già spedito i primi nove.
   - **Il doppio conteggio indipendente.** `count_unauthorized` (`:361-369`) ricontrolla i passaggi che stanno per partire, uno per uno, sul punto di invio (`:423`), e incrementa un contatore atomico (`AtomicU64`, un intero che più task possono aggiornare senza lock). Se la guardia funziona, questo contatore resta a 0 per costruzione. Il punto è che i due controlli stanno in punti diversi del codice e non si fidano l'uno dell'altro: se un giorno qualcuno aggiungesse un percorso di invio che salta la guardia, il riconteggio lo vedrebbe. Il valore finisce in ogni riga di `forms.csv` (colonna 15), ed è l'hard gate delle soglie.
   - **`send_inner`** (`:413-486`) è una chiamata logica. Prima controlla il budget (`:422`), poi riconta, poi invia con `bearer_auth`, che reqwest marca come header sensibile. `tokio::time::timeout_at(deadline, request)` (`:430`) avvolge l'invio: se scade la deadline, il future della richiesta viene distrutto e la richiesta cancellata, come misurato in 01-05.
   - **Retry-after entro la deadline** (`:443-466`). Su 429 o 529 il client legge `retry-after` (in secondi; se manca usa il backoff), poi si chiede: l'attesa finisce prima della deadline? (`:454-456`, `Instant::now().checked_add(wait).is_some_and(|t| t < deadline)`; `checked_add` restituisce `None` invece di andare in overflow, e `is_some_and` è vero solo se c'è un valore e la condizione vale.) Solo se sì, e se non ha già fatto il suo unico retry, dorme e riprova. Altrimenti restituisce `RateLimited`. Non c'è nessun ramo che, per mancanza di tempo, restituisca un segnale di default: un `relevant = true` inventato, nel prodotto, equivarrebbe a far passare un'evidenza mai valutata.
   - **Il cost guard.** `cost_usd` (`:224`) converte i token di `usage` in dollari: 0,042 USD per milione di token di input, output gratuito. Dopo ogni risposta il client somma la spesa (`:480`) in un `Mutex<f64>`; prima di ogni invio `check_budget` (`:371`) rifiuta con `BudgetExceeded` se la spesa ha raggiunto il tetto. Il `Mutex` qui è il modo più semplice per sommare un `f64` da più task, perché non esiste un `AtomicF64` nella libreria standard. Il tetto vale per la sessione, non per il processo: in `main.rs` il budget passato al client è quello residuo, letto da un registro su file (`main.rs:244-279`).
   - **Le tre forme.** `rank_form_a_each` (`:521-538`) usa `futures::stream::iter(...).buffered(concurrency)`: tiene in volo al più `concurrency` richieste nello stesso task e restituisce i risultati nell'ordine dei candidati (a differenza di `buffer_unordered`, che li restituisce man mano). Ogni richiesta ha già il suo `timeout_at`; quello esterno, con 50 ms di margine, è una rete di sicurezza. `rank_form_a` (`:541`) trasforma il primo errore di un candidato in errore del rank. `body_multi` (`:561`) costruisce lo `state` con tutti i passaggi per B e C; B crea una domanda Noul per indice (`:584-589`), C una sola Choice con un criterio per passaggio (`:622-632`).
   - **`Debug` scritto a mano** (`:684-695` e `:103-113`): mostra `<redatta>` al posto della chiave.

4. **`spikes/s04-jev/src/metrics.rs`**: nDCG@k e recall@k con rilevanza binaria. Lo sconto di posizione è `1 / log2(i + 2)` (`metrics.rs:25`); senza rilevanti il risultato è 0, mai NaN (`:8`, `:32`).

5. **`spikes/s04-jev/src/main.rs`**: la CLI.
   - **La risoluzione degli alias** (`cmd_aliases`, `main.rs:392-443`) costruisce un client nuovo per ogni nome, sostituendo `JEV_MODEL` con l'alias richiesto (`:274-276`), e manda una richiesta di forma A con un solo passaggio pubblico di una query `guest`, lo stato più piccolo possibile. Poi registra `response.model`. È l'unico punto in cui il modello non è il pin.
   - `cmd_concurrency` (`:447`): per ogni livello e run, 10 query da 20 candidati, una query alla volta con N richieste in volo.
   - `cmd_forms` (`:522`): un warm-up `rep` 0 sulla prima query, poi 30 query × 3 ripetizioni. Dopo ogni rank scrive il registro di spesa e si ferma se il contatore di passaggi non autorizzati è diverso da 0 (`:623`).
   - `cmd_collect` (`:728`) legge i grezzi jsonl e scrive i tre CSV. `ranking` (`:678`) ordina per punteggio e, a parità, per posizione nell'insieme mescolato. `sample_std` (`:694`) è la deviazione standard campionaria, con `n − 1` al denominatore.

### Flusso dei dati

`build_candidate_sets` legge `queries.json`, `acl.json` e `manifest.json` di `spikes/data/corpus/` e produce 30 insiemi autorizzati. Nel runner, `forms` e `concurrency` passano ogni insieme al client, che lo controlla, lo manda a `{JEV_API_URL}/v1/systemone` e restituisce segnali e metadati (`model_resolved`, `usage`, status, latenza). La CLI scrive una riga per rank nei grezzi di `results/raw/jev/` (ignorata da git) e aggiorna `spend.json`. Alla fine `collect` produce `forms.csv`, `concurrency.csv` e `aliases.csv`, gli unici versionati.

## Perché

- **Forma A invece di B o C** (ADR-0008 §Decisione 1). Con B ogni domanda vede tutti e 20 i passaggi, ed è il failure mode che typesafe stessa documenta («Large state full of irrelevant detail»): qualità peggiore (0,679) e varianza più alta. C ha una qualità migliore di B (0,729) e i token più bassi, ma le sue probabilità sommano a 1: dicono quale passaggio è il migliore, non se un passaggio è rilevante in assoluto. L'evidence gate ha bisogno della seconda informazione. A costa circa 6 volte i token di C, ma in assoluto circa 0,00035 USD per query, e la stessa chiamata dà anche `sufficient` e `instructs_model`.
- **Concorrenza 16.** La regola era «il livello più alto con 0 risposte 429». Va letta per quello che è: 16 è il massimo provato, non un limite di JEV. Il rate limit dell'organizzazione non è pubblicato.
- **Guardia tutto-o-niente invece che per invio** (RISK-1, T-14). Una guardia per invio avrebbe già spedito i passaggi visibili precedenti. La guardia sull'insieme non spedisce nulla. Il riconteggio sul punto di invio è difesa in profondità.
- **Un solo retry** (ADR-0010 Decisione 5). L'SDK Python di typesafe ne fa due, ma lì non c'è una deadline globale. Il tetto è passato da 2 a 1 in `acbac93`, prima della seconda sessione.
- **Registro di spesa su file** invece di un budget per processo: le misure girano in più invocazioni della CLI, e il tetto `S04_MAX_COST_USD` è per sessione.
- **Al più 2 excerpt `injection_case` per insieme** invece di tutti quelli visibili: fino a 8 su 20 avrebbero trasformato l'insieme in un insieme ostile e falsato il ranking.
- **`splitmix64` locale invece del crate `rand`**: `rand` non è fra le dipendenze di `s04-jev`, e il `Cargo.toml` del crate è fuori dai file del piano.
- **Grezzi jsonl più `collect`** invece di CSV scritti direttamente: il `Writer` di `common` tronca il file, le forme girano in tre invocazioni separate, e la std per candidato richiede tutte e tre le ripetizioni.
- **Legami:** D-16, D-17, D-22 di 01-CONTEXT; T-14, T-16, T-24 del threat model; ADR-0002 (JEV dà solo segnali) e ADR-0010 (deadline e retry).

## Test

I 16 test rossi li ha scritti un agente separato in `0c338cb`; l'esecutore ha riempito gli stub senza toccarli (nel diff fino a `a5785c6` le righe tolte sono solo `todo!()` e commenti degli stub). I test di rete usano un server finto su `127.0.0.1:0` costruito con `tokio::net::TcpListener` (`jev.rs:698` e seguenti), perché il crate non ha `wiremock`: il server risponde con lo status e gli header scelti dal test e registra le richieste ricevute.

- **TEST-1** (`candidates.rs:456`, `:477`, `:486`, `:494`): passaggio `restricted` per un `employee` negato, documento ignoto negato, classificazione del manifest prevalente, ruoli e profili.
- **TEST-2** (`jev.rs:904`): le tre forme con l'invisibile a metà danno `Unauthorized`, e il server riceve 0 richieste.
- **TEST-3** (`candidates.rs:532`): sul corpus vero, ogni passaggio di ogni insieme è visibile al ruolo, viene da un oracolo verde, e due costruzioni con lo stesso seme coincidono.
- **TEST-6** (`jev.rs:936`): né `Debug` né l'errore contengono la chiave fittizia, e il server l'ha ricevuta nell'header giusto.
- **TEST-7** (`jev.rs:976`): 429 e poi 529 con `retry-after: 1`, deadline 3 s: due richieste a distanza di almeno 1 s, poi esito positivo.
- **TEST-8** (`jev.rs:1008`, `:1031`): 429 con `retry-after: 5` e deadline 1 s dà `RateLimited` con una sola richiesta; un server muto dà `Timeout`. In entrambi i casi entro la deadline più 100 ms.
- **TEST-10** (`jev.rs:1053`, `:1086`): il corpo porta il modello configurato e la risposta torna come `model_resolved`; senza `JEV_MODEL` il client non si costruisce.
- **TEST-12** (`jev.rs:1112`, `:1123`): un milione di token di input costa 0,042 USD, e oltre il budget la chiamata è rifiutata senza invio.
- **TEST-13** (`metrics.rs:53`, `:74`): valori noti di nDCG e recall, e 0 senza rilevanti.
- **TEST-4, 5, 9, 11, 14, 15, 16, 17** sono controlli su CSV, git e file: il bridge li ha rieseguiti (vedi il `coverage` del SUMMARY), compreso il controllo che la chiave reale non compaia in nessun file, fatto con `grep -q` senza stamparla.

Per lanciarli singolarmente sull'host: `cd spikes && cargo test -p s04-jev --locked --offline <nome_test>`, per esempio `send_path_blocks_before_network_on_invisible_passage`. Il bridge ha rieseguito i 16 test (verdi in circa 2 s) e lo scanner di sicurezza (exit 0). Le misure vere si rilanciano solo nel runner, dentro `bash spikes/tools/session.sh with 01-11 -- …`, con le crate vendorizzate e la chiave dal file segreti esterno: costano poco (circa 0,08 USD) ma sono chiamate reali a un provider negli Stati Uniti.

## Deviazioni e punti aperti

- **`instructs_model` misurato solo a metà (TEST-16).** Gli oracoli `injection_case` ratificano i fatti legittimi dei documenti `syn-07` e `syn-08`, non le istruzioni ostili che contengono. Il piano vieta di estrarre testo nuovo, quindi il testo ostile non è mai stato inviato. Lo 0 su 1.800 è un tasso di falsi positivi (con un massimo di 0,20), non di rilevamento. **Resta aperto**: lo può misurare 01-12 (tenuta alla prompt injection indiretta) oppure un oracolo nuovo che ratifichi gli excerpt ostili.
- **`Acl` nel costruttore.** `Client::new_from_env(acl)` riceve l'ACL, mentre il piano scrive `new_from_env()` senza argomenti. La firma era già negli stub del test-write ed è compatibile con `01-CONTRATTO.md`, che non fissa il costruttore. Per 01-15: `Client::new_from_env(Acl::load(corpus)?)?`.
- **`CallMeta` più ricco e `rank_form_a_each` in più**: aggiunte compatibili.
- **Retry da 2 a 1** in `acbac93`: il piano non fissava il numero, ADR-0010 sì. La prima sessione, col tetto a 2, non ha avuto retry.
- **Nessun 429 reale**: il comportamento con `retry-after` è provato solo sul server finto.
- **Grezzi spostati** da `results/jev/raw/` a `results/raw/jev/`, perché il `.gitignore` (01-02, chiuso) copriva solo il secondo percorso. `normalized_by` di `hw.json` cita ancora quello vecchio.
- **Grezzi non versionati**: media e p95 della std per candidato, «1 su 600», la ripartizione degli invii injection e il modello delle 1.800 richieste di concorrenza non si possono ricalcolare dai CSV. Il CSV conferma che, nella forma A, solo Q-066 supera 0,05.
- **`request_id` vuoto**: JEV non manda l'header.
- **`hw.json` normalizzato** e **build offline** da vendor, come in 01-04 e 01-05.
- **ADR-0008 scrive «`buffered`/`buffer_unordered`»**: il codice usa `buffered`, che conserva l'ordine dei candidati; per la cancellazione la differenza non conta.
- **Integrazione fuori ordine**: 01-11 è stato fuso prima di 01-07, contro l'ordine di `01-ONDE.md`, per decisione utente del 2026-10-21, perché 01-07 è fermo su tre blocchi esterni al piano.
- **Concorrenza per una query alla volta**: un carico con query di più utenti non è stato misurato, e 16 × N richieste potrebbero superare un rate limit che non conosciamo.
- **ADR-0008 è `Proposed`**: la ratifica e la riga «Modello testuale» di `DEPENDENCIES.md` spettano a 01-17. Prima di passare a `jev-1.14.0` servono una nuova misura e una nuova calibrazione.

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-11
