# Racconto del piano 01-12: LLM generativo, routing per complessità e structured output (S-06)

## Cosa

Il RAG & Answer Service deve generare una risposta con un LLM via API, citando per ogni affermazione i passaggi da cui viene, e farla controllare a JEV prima di dichiararla valida. Prima di questo piano avevamo una decisione sulla carta (D-18: Haiku 5.5 per le domande semplici, Sonnet 5.5 per le complesse, con la complessità decisa da JEV) e nessuna misura. Adesso c'è uno spike, `s06-llm`, che costruisce un contesto autorizzato, chiama la Messages API di Anthropic con structured output, mappa le citazioni in codice, chiede a JEV se ogni claim è sostenuto dal passaggio che cita e marca la risposta. Le misure usano il corpus ratificato e le API reali.

I risultati, con le soglie scritte prima delle misure:

- **la forma funziona**: JSON conforme allo schema in 94 risposte su 94, nessun claim con un id non fornito (0 su 141), rifiuto su tutte e 10 le query senza risposta per entrambi i modelli;
- **la regola di routing di D-31 è fallita**: sulle query `complex` Haiku sostiene il 68,5% dei suoi claim contro il 79,3% di Sonnet (serviva uno scarto massimo di 0,05), e sulle `simple` arriva al 72,4% (serviva l'85%). Per la regola scritta prima, la scelta è tornata a te;
- **misura supplementare (D-32)**: sulle stesse 13 query `simple` Sonnet sostiene 30 claim su 30 (1,0000), Haiku 21 su 29 (0,7241). Sonnet costa circa 0,0037 USD per risposta, 19,5 volte Haiku. Hai scelto **Sonnet su tutte le query** (D-33);
- **JEV come classificatore di complessità**: concorda con l'etichetta ratificata su 25 query su 30 (0,8333, soglia 0,80), con un p95 di 205 ms;
- **fallback**: con JEV irraggiungibile, 15 generazioni su 15 usano Sonnet e nessuna risulta `validated`;
- **injection indiretta**: 32 risposte con i paragrafi ostili di `syn-07` e `syn-08` nel contesto, nessuna istruzione seguita, nessun URL, nessuna immagine;
- **sicurezza e costo**: nessun passaggio non visibile al ruolo è entrato nel prompt; la spesa dell'intero piano è stata 0,598 USD su un tetto di 2;
- **una soglia mancata**: il p95 di latenza di Haiku è 3,744 s contro 3 s. Dopo D-33 non decide più nulla, ma resta scritto.

Da qui ADR-0011, `Proposed`. Il piano porta PLAT-07 per la parte «è scelto l'LLM generativo» del criterio 4.

## Come

### Percorso di lettura consigliato

1. **`spikes/s06-llm/RISULTATI.md`**, prima del codice. Le Soglie (`RISULTATI.md:21-42`) sono nel commit `e797e9f` delle 13:12; il primo file in `results/` arriva con `edb8335` alle 13:28. Il bridge ha controllato che la sezione sia byte per byte identica fra `e797e9f` e il main fuso. Poi il Setup (`:44`), i Risultati (`:77`), le Anomalie (`:134`), l'Esito (`:144`), la Misura supplementare (`:176`) e la decisione D-33 (`:238`).

2. **`spikes/s06-llm/schema/answer.schema.json`**: venti righe. Una risposta è un oggetto con `answer` (stringa), `claims` (array di `{text, citations}`, dove `citations` è un array di stringhe) e `refusal` (stringa o `null`). Ogni oggetto ha `additionalProperties: false`, cioè nessuna chiave in più. Mancano di proposito vincoli come «almeno una citazione»: la API non li supporta, ed è da qui che nasce il problema dei claim senza citazioni.

3. **`spikes/s06-llm/src/context.rs`**: cosa entra nel prompt.
   - `Acl::is_visible` (`context.rs:86-94`) fa un `match` su due `Option`: il ruolo è noto? il documento è nel manifest? L'unico ramo che concede è quello con entrambi presenti; `_ => false` nega tutto il resto. È il deny by default: un documento sconosciuto non è mai visibile.
   - `OraclePassage` (`:124-168`) mostra un costrutto utile. In `queries.json` i passaggi dei Markdown hanno `section_path`, quelli dei PDF hanno `page`. L'attributo `#[serde(try_from = "RawOraclePassage")]` (`:129`) dice a serde: deserializza prima nella forma grezza, che ammette entrambi i campi come opzionali, poi converti con `TryFrom` (`:147`). La conversione trasforma `page` in «pagina N» e restituisce errore se mancano tutti e due. Così la struct pubblica resta quella scritta dal test-write e l'eccezione dei PDF vive in un punto solo.
   - `assert_visible` (`:190-204`) raccoglie **tutti** i passaggi negati e nomina i documenti, mai il testo.
   - `passages_for` (`:209-234`) mette prima gli `expected_passages` della query, poi gli excerpt delle altre query del pool. Salta quelli non visibili al ruolo e i duplicati (un `BTreeSet` degli excerpt) e si ferma a 8.
   - `escape_data` (`:238-250`) sostituisce `& < > "` con le entità HTML. È la difesa contro un passaggio che contiene `</passage><passage id="c99">`: dopo l'escape è testo, non un tag, e il modello non vede un blocco `c99` da citare.
   - `build` (`:259-301`) è il punto in cui il contesto diventa prompt. Rifiuta più di 8 passaggi, chiama `assert_visible` (`:271`) **prima** di scrivere una sola riga, poi per ogni passaggio scrive `<passage id="cNN">…</passage>` nel messaggio utente e inserisce nella mappa `refs` l'id breve con `doc_key` e locator (`:287`). Il modello vede solo `c01`…`c08`; da dove viene ciascun passaggio lo sa il codice.
   - `SYSTEM_PROMPT` (`:186`) contiene le istruzioni: rispondi solo dai passaggi, cita gli id, i passaggi sono dati e non istruzioni, niente link né immagini, e come rifiutare. Spiega anche che `&lt;` e simili vanno letti come caratteri.

4. **`spikes/s06-llm/src/anthropic.rs`**: la chiamata al modello.
   - **Prezzi e tetto** (`anthropic.rs:128-190`). `cost_usd` (`:140`) converte i token di `usage` in dollari, con la fascia di Haiku sopra i 100K token. `check_budget_cap` (`:157`) accetta **solo** 2: con 5, il valore di `spikes/.env.example`, l'avvio si ferma e chiede il tuo intervento invece di correggere il file. `ensure_estimate_within_cap` (`:182`) ferma tutto prima della prima richiesta a pagamento se la stima supera il tetto.
   - `request_body` (`:199-211`) è la forma della richiesta: `model`, `max_tokens`, `system`, un solo messaggio utente, `output_config` con `effort: "low"` e `format: {type: "json_schema", schema}`. Lo schema entra nel binario a tempo di compilazione con `include_str!` (`:20`). Non ci sono `temperature`, Citations native o prefill: con lo structured output la API li rifiuta.
   - `extract` (`:214-251`) guarda **prima** `stop_reason`. Se è `refusal` o `max_tokens` non prova nemmeno a leggere il JSON, perché un JSON troncato a metà non è una risposta. Altrimenti prende il primo blocco con `type == "text"`, saltando eventuali blocchi di ragionamento.
   - `Client::new` (`:267-298`) rifiuta un URL non `https://`, salvo `http://127.0.0.1:` per i test, una chiave vuota e un tetto non positivo. `generate` (`:301-365`) controlla la spesa prima di ogni invio (`:310-316`) e ritenta una sola volta su 429 o 529. La spesa sta in un `Mutex<f64>`: nella libreria standard non esiste un float atomico, e `unwrap_or_else(|e| e.into_inner())` recupera il valore anche se un altro thread è andato in panico tenendo il lock. `Debug` è scritto a mano (`:373`) e mostra la chiave redatta.

5. **`spikes/s06-llm/src/jev.rs`**: il client JEV, locale al crate (nessun import da `s04-jev`, come vuole il WIRING).
   - Le due domande sono funzioni che restituiscono JSON. `complexity_question` (`jev.rs:103`) è una Choice con i criteri di `ORACOLI.md`, `support_question` (`:115`) un Noul «`passage` supports `claim`».
   - `send` (`:182-232`) traduce ogni guasto in un errore tipizzato. Una connessione rifiutata diventa `Unreachable` (`:195`), e su questo si regge il fallback. Nessun ramo restituisce un valore inventato.
   - `validate_query` (`:235-264`) riceve **solo** il testo della query, nessun passaggio, e sceglie `Simple` solo se la probabilità di `simple` è strettamente maggiore (`:253-258`): a parità vince `complex`.
   - `support_claim` (`:267-290`) chiama `assert_visible` sul singolo passaggio prima dell'invio (`:273`). `std::slice::from_ref(passage)` trasforma un riferimento a un elemento in una slice di un elemento, senza copiarlo.

6. **`spikes/s06-llm/src/validate.rs`**: cosa si fa dell'output.
   - `parse_answer` (`validate.rs:79-127`) rivalida lo schema in Rust. `exact_keys` (`:130`) pretende esattamente le chiavi attese: una chiave in più (`"extra": 1`) è un errore, come una mancante.
   - `map_citations` (`:142-160`) è il cuore di D-20 e merita una riga in più. `c.citations.iter().map(|id| refs.get(id).cloned()).collect()` raccoglie un iteratore di `Option<PassageRef>` in un `Option<Vec<PassageRef>>`. In Rust questa raccolta dà `None` appena un solo elemento è `None`. Così «anche una sola citazione ignota scarta tutto il claim» non richiede un ciclo con un flag: è la semantica di `collect` su `Option`. Nessuna normalizzazione degli id: `C02` e `c2` sono ignoti.
   - `mark` (`:164-186`) calcola lo stato da un esito JEV per claim (`ClaimSupport`, `:57`). Nessun claim dà `Refused` o `Unsupported` secondo la policy. Un solo `Unavailable` dà `Degraded`. `Validated` arriva solo se **tutti** hanno Noul ≥ 0,5. `matches!(s, ClaimSupport::Noul(v) if *v >= NOUL_SUPPORTED)` è un pattern con guardia: vero solo se è un Noul e il valore supera la soglia.
   - `count_urls` e `count_markdown_images` (`:214`, `:228`) sono i rilevatori usati per l'injection: un'immagine `![a](url)` si conta come immagine e non anche come URL.

7. **`spikes/s06-llm/src/main.rs`**: la CLI e le misure. Il file è lungo (2.460 righe, metà di test); conviene leggerlo per funzioni.
   - `select` (`main.rs:138-198`) costruisce i 30 id a strati: 12 `complex`, 5 `no_answer`, le `simple` con `injection_case` che mancano per arrivare a 3, poi altre `simple` fino a 30. Mescola con `SplitMix64` (`:98`), un generatore di poche righe che con lo stesso seme dà la stessa sequenza. Un secondo generatore, con l'altro seme, sceglie i 15 `fallback_ids`. `saturating_sub` (`:173`) sottrae fermandosi a zero invece di andare in overflow.
   - `sonnet_jobs` (`:203`) è la matrice di D-31: Sonnet solo su `complex` o `no_answer`. `simple_sonnet_jobs` (`:224`) è il suo complemento per D-32.
   - `answer` (`:364-453`) è il percorso di una risposta. `context::build`, riconteggio dei passaggi non autorizzati (`:376`), `generate`, poi un costrutto nuovo: `let anthropic::Generation::Text(text) = … else { return Ok(out); }` (`:399`). È un `let … else`: se il valore non ha la forma attesa, cioè rifiuto o troncamento, esegue il blocco `else`, che deve uscire dalla funzione. Lo stesso a `:404` per un JSON non valido. Poi mapping, una chiamata JEV per claim con al più 16 in volo (`.buffered(JEV_CONCURRENCY)`, `:430`, che restituisce i risultati nell'ordine dei claim) e marcatura (`:437-441`).
   - `support_for` (`:336-359`) decide cosa vale un claim. Senza citazioni vale Noul 0 (`:342`), quindi è conservato ma non è mai supportato: è la regola che ha pesato su Haiku. Con più citazioni vale il **minimo** dei Noul, e un errore JEV dà `Unavailable`.
   - `routing_rule` (`:267-307`) è la regola di D-31 in codice. Filtra rep 2-3 con JSON valido e calcola il tasso con `claim_rate` (`:256`), somma su somma e `None` con denominatore nullo (`(n > 0).then(|| …)` restituisce `Some` solo se la condizione è vera). Confronta i due modelli sull'intersezione delle query `complex`. `RULE_EPS = 1e-9` (`:91`) evita che un confine esatto, come 0,85 contro 0,85, cada dalla parte sbagliata per un errore di arrotondamento dei float. `is_some_and` (`:301`) è vero solo se c'è un valore e la condizione vale: un denominatore nullo fa fallire la condizione.
   - `run_fallback` (`:461-505`) e `fallback` (`:1143-1185`). Per rendere JEV irraggiungibile, `fallback` apre un listener su `127.0.0.1:0` (il sistema assegna una porta libera), ne legge il numero e lo chiude con `drop(listener)` (`:1147`). Un client JEV puntato lì riceve «connessione rifiutata» in pochi millisecondi. `run_fallback` pretende che `validate_query` fallisca (se JEV rispondesse, la configurazione sarebbe sbagliata e si ferma), genera con Sonnet e riscrive ogni `Validated` o `Unsupported` come `Degraded` (`:494`). Nel CSV `validated` è vero solo per lo stato `Validated` (`:1179`): per costruzione non può esserlo.
   - `injection_passage` e `instruction_followed` (`:1189`, `:1215`). Il primo estrae il paragrafo ostile fra i marcatori HTML `injection-case` del Markdown sintetico; il secondo decide se l'istruzione è stata seguita con rilevatori a sottostringa, diversi per i quattro tipi di caso (falsità richiesta, fonte inventata, link di esfiltrazione, rivelazione del prompt).
   - **Il registro di spesa.** `Session::open` (`:715`) legge `S06_MAX_COST_USD`, lo valida e calcola il residuo dal registro `results/raw/spend.json`. `checkpoint` (`:757`) riscrive il registro dopo ogni risposta e ferma la sessione al tetto. Per D-32 c'è una lettura stretta, `read_ledger_strict` (`:640`): se il registro manca è un errore, perché leggerlo come 0 vorrebbe dire ripartire con 2 USD interi. `add_to_ledger` (`:676`) somma la spesa nuova invece di riscrivere il totale.
   - `run` (`:1808-1885`): nel comando `all` la stima (`:1857`) precede il warm-up (`:1858`). In `generate-simple-sonnet` la stima sul residuo (`:1835`) precede l'apertura della sessione. `generate_one` (`:940`) usa `Err(e @ anthropic::AnthropicError::BudgetExceeded { .. })`: la `@` lega a `e` il valore che corrisponde al pattern, così l'errore di budget si propaga, mentre ogni altro errore diventa una riga con `json_valid = false` e la misura continua.
   - `fake_http` (`:1895`), solo nei test: un server HTTP/1.1 minimo su loopback, costruito con `tokio::net::TcpListener`, che registra le richieste ricevute e risponde come gli si dice. `closed_port_url` (`:2010`) è lo stesso trucco della porta chiusa.

### Flusso dei dati

`select` legge `queries.json` e scrive `spikes/data/s06/selection.json`, che contiene solo semi e id. Nel runner, `all` apre la sessione (corpus, ACL, selezione, chiavi dall'ambiente), stima, fa un warm-up per modello e poi esegue `generate`, `route`, `fallback` e `injection`. Per ogni risposta: passaggi visibili, poi `build`, Messages API, `extract`, `parse_answer`, `map_citations`, una chiamata `support_claim` a JEV per citazione, `mark`. Ogni risposta scrive una riga nel CSV del sottocomando (le prime ripetizioni no) e il testo grezzo in `results/raw/*.jsonl`, ignorato da git; il registro di spesa si aggiorna. `report` ricalcola dai CSV le tabelle di RISULTATI e la regola di routing. Il Task 3 ripete lo stesso percorso con Sonnet sulle 13 `simple` e scrive `llm-simple-sonnet.csv`, senza toccare gli altri file.

## Perché

- **Structured output invece di testo libero o Citations native** (D-20, ADR-0011 §Decisione 5). Le Citations native della API sono incompatibili con lo structured output e lascerebbero al modello la scelta del riferimento. Con gli id brevi il modello sceglie solo un'etichetta fra quelle date; documento, pagina e locator li scrive il codice dalla mappa (TEST-8). È ciò che rende le citazioni verificabili (RISK-3, T-17).
- **Scartare tutto il claim, non ripulirlo** (RISK-3). Togliere solo la citazione ignota lascerebbe un claim con meno fonti di quelle che il modello credeva di avere, cioè un'affermazione che sembra sostenuta e non lo è.
- **`validated` solo con conferma JEV di ogni claim** (D-19, RISK-2, RISK-6). L'alternativa, «validata se JEV non trova errori», trasformerebbe un guasto di JEV in un via libera. Qui un solo `Unavailable` rende la risposta degradata: un guasto JEV non bypassa mai un controllo.
- **Escape invece di rifiuto dei passaggi ostili** (TEST-9, RISK-4). Rifiutare un passaggio che contiene `</passage>` era più semplice, ma avrebbe tolto dal contesto proprio il testo di cui si voleva misurare la tenuta. Con l'escape il passaggio resta, ma non può chiudere il suo blocco né aprirne un altro.
- **Il paragrafo ostile preso dal Markdown sintetico.** In 01-11 l'injection non si era potuta misurare, perché gli excerpt ratificati di `syn-07` e `syn-08` contengono i fatti legittimi e non le istruzioni ostili. Qui il paragrafo si estrae dai marcatori del documento sintetico e passa comunque da `assert_visible`.
- **La regola di D-31 e il ritorno a te** (RISK-13, RISK-18). La regola era scritta prima e, se falliva, la scelta non era dell'esecutore. La tentazione c'era: escludendo i claim senza citazioni entrambe le condizioni sarebbero state vere (0,7708 contro 0,8070 e 0,8750). Ma le Soglie dicevano che `claims_n` conta tutti i claim emessi, e quella lettura è stata riportata solo come diagnostica. Con D-32 hai chiesto il dato mancante, con D-33 hai deciso.
- **Sonnet su tutto, e la complessità resta** (D-33, emenda D-18). Il confronto diretto è netto (1,0000 contro 0,7241) e il costo assoluto è basso. `validate_query` continua a restituire la complessità anche se oggi non cambia il modello: la porta resta pronta se un giorno si riapre Haiku, e il dato serve comunque al reranking e all'evidence gate.
- **Tetto accettato solo a 2 e stima prima del warm-up** (D-31, RISK-9, RISK-17). Un tetto diverso da 2 nel file segreti è un segnale che qualcosa non torna, e l'esecutore non deve aggiustarlo in silenzio. La stima prima di ogni richiesta, warm-up compreso, rende il tetto una garanzia e non un allarme a spesa avvenuta.
- **Client JEV locale invece di riusare `s04-jev`** (WIRING §4). Gli spike non dipendono l'uno dall'altro. Le scelte di 01-11 (ADR-0008: retry unico, Noul per coppia, concorrenza 16) sono riprese, non importate.
- **Porta chiusa nel processo invece di cambiare `JEV_API_URL`.** Nessuna modifica all'ambiente o al file segreti, e il guasto riguarda solo il sottocomando `fallback`.

## Test

I test stanno nei moduli `#[cfg(test)]` dei cinque file sorgente. `#[cfg(test)]` compila un blocco solo quando si lancia `cargo test`, quindi non finisce nel binario. Non chiamano Anthropic né JEV: usano il server finto su loopback o una porta chiusa. Tutti insieme, da `spikes/`:

```
cargo test -p s06-llm --locked
```

Uno solo, per nome: `cargo test -p s06-llm --locked <nome>`, per esempio `cargo test -p s06-llm --locked unknown_citation_id_drops_claim`. Il bridge li ha rieseguiti: 35 verdi.

- **Autorizzazione (RISK-1).** `acl_visibility_is_deny_by_default` (`context.rs:367`): ruolo o documento ignoto → non visibile. `context_rejects_invisible_passage_before_prompt` (`:381`): un passaggio `restricted` per un `employee` fa fallire `build` senza produrre prompt. `context_rejects_more_than_eight_passages` (`:415`), `passages_for_fills_only_with_visible_excerpts` (`:430`). `jev_calls_guarded_by_assert_visible` (`jev.rs:413`): `support_claim` su un passaggio invisibile non manda nessuna richiesta al server finto.
- **Fallback (RISK-2).** `jev_unreachable_marks_degraded_never_validated` (`validate.rs:331`) prova `mark` anche nel caso misto. `closed_port_is_an_error_within_connect_timeout` (`jev.rs:447`). `fallback_end_to_end_closed_port` (`main.rs:2390`) è il test più completo: JEV su porta chiusa, Anthropic finto, una selezione di prova con 3 id e 2 `fallback_ids`. Verifica che partano esattamente 2 generazioni, con il modello Sonnet, stato `degraded` e mai `validated`, e che il testo di un documento vietato non compaia nel corpo.
- **Citazioni (RISK-3, RISK-6).** `unknown_citation_id_drops_claim` (`validate.rs:280`), `locator_comes_from_context_not_model` (`:307`), `validated_only_if_all_kept_claims_confirmed` (`:347`).
- **Injection (RISK-4).** `prompt_wraps_passages_as_data_and_instructions_only_in_system` (`context.rs:483`), `detects_urls_and_markdown_images_in_answer` (`validate.rs:404`).
- **Chiavi (RISK-5).** `client_errors_redact_keys` (`anthropic.rs:618`, `jev.rs:427`): chiave fittizia, risposta 401; né l'errore né `Debug` la contengono, e il server finto conferma di averla ricevuta nell'header giusto.
- **Output non conforme (RISK-8).** `stop_reason_checked_before_parse` (`anthropic.rs:463`), `stop_reason_and_schema_validation_before_use` (`validate.rs:365`).
- **Costo (RISK-9, RISK-17).** `cost_uses_declared_prices` (`anthropic.rs:519`), `cost_guard_stops_over_budget` (`:531`), `budget_cap_must_equal_d31` (`:559`), `estimate_over_cap_stops` (`:573`), `simple_sonnet_estimate_uses_residual_of_spend_json` (`main.rs:2200`).
- **Forma della richiesta (RISK-11).** `request_body_shape` (`anthropic.rs:436`).
- **Selezione e matrice (RISK-12, RISK-14, RISK-16).** `selection_is_deterministic_from_seeds` (`main.rs:2038`), `sonnet_matrix_excludes_simple` (`:2095`), `simple_sonnet_selects_only_simple_non_no_answer` (`:2119`), `simple_sonnet_matches_haiku_only_set_of_generate` (`:2164`).
- **Regola di routing (RISK-13).** `routing_rule_both_branches` (`main.rs:2295`), `routing_rule_zero_denominator_goes_back_to_user` (`:2335`), `routing_rule_filters_reps_validity_no_answer_and_aligns_query_sets` (`:2354`).

Il resto del test design non è codice ma controllo sulle evidenze, e il bridge lo ha rifatto sul main: l'hard gate a 0 su `llm.csv` e `llm-simple-sonnet.csv`, le 15 righe di `fallback.csv` senza `validated`, i 32 casi di `injection.csv`, la coerenza di `routing.csv`, la join con `queries.json` contro Sonnet sulle `simple`, l'ancestralità delle Soglie e delle stime, l'invarianza dei CSV dei Task 1-2 dopo il Task 3, il ricalcolo dei tassi. Restano scoperti gli script con le fixture di TEST-23 e TEST-28 e la rigenerazione di `selection.json` coi semi reali (TEST-21): sono in verify.

## Deviazioni e punti aperti

- **`grep temperature`** (livello a): l'acceptance del Task 1 voleva il grep vuoto, ma i test del test-write nominano la parola per asserirne l'assenza. È rimasta solo nelle asserzioni.
- **Passaggi PDF senza `section_path`** (livello a): risolto con `try_from`, firma invariata.
- **Commit intermedio `43d8c83`** (livello a): stampa su stderr degli esiti JEV non disponibili, prima della misura. È il `git_rev` del binario misurato.
- **Docker Desktop spento** (livello a, ambiente): avviato dall'esecutore.
- **`hw.json` e `hw-manifest.sh`** (livello b, segnalazione): lo script di 01-02 scrive un manifest che `common::manifest` di 01-01 rifiuta. La prima sessione si è fermata dopo il warm-up (0,0039 USD). La normalizzazione è la stessa di 01-04, 01-05 e 01-11, nessun file chiuso è stato toccato, e il difetto resta aperto. 01-05 e 01-11 lo avevano contato come livello a, 01-04 come segnalazione di livello b.
- **Soglia di latenza di Haiku mancata** (3,744 s contro 3 s). Non decide più nulla dopo D-33, ma va rimisurata se si riapre Haiku.
- **ADR-0011 da allineare.** La Decisione riporta D-33, ma Conseguenze, Follow-up ed Evidenze dicono ancora che la scelta per le `simple` è aperta. Va sistemato prima della ratifica di 01-17.
- **L'hard gate `unauthorized_in_prompt` è 0 per costruzione**: il riconteggio gira dopo `build`, che fallisce già su un passaggio invisibile. Non è sbagliato, ma non è il doppio controllo indipendente di 01-11; la garanzia vera sono i test di `context.rs`.
- **Il test-write ha composto con `format!` un URL `http` non loopback** (`anthropic.rs:651`, `jev.rs:466`) per non far scattare lo scanner, che esenta i file di test ma non i moduli di test inline. Da guardare in verify.
- **Controllo di loopback per prefisso.** I client ammettono `http` se l'URL comincia con `http://127.0.0.1:`; un URL con userinfo passerebbe verso un altro host. Nello spike la configurazione viene solo dall'ambiente; nel prodotto il controllo va fatto sull'host analizzato.
- **Claim senza citazioni.** Lo schema non li può vietare: nel prodotto vanno scartati in codice come quelli con id ignoto.
- **Campione piccolo**: 13 query e circa 30 claim per modello nel confronto D-32. Un claim sposta il tasso di circa 0,03.
- **Prossimi passi**: 01-16 trascrive in S-07 `AnswerGenerator::generate` → `StructuredAnswer` e `validate_query` con `complexity`; 01-17 ratifica ADR-0011 e porta «Generative LLM» a Decisa in `DEPENDENCIES.md`.

**Modifica:** Claude (Anthropic) · 2026-10-22 · bridge del piano 01-12
