# Racconto del piano 01-15: reranker baseline, convivenza in VRAM e ADR-0009 (S-04)

## Cosa

Il piano 01-11 aveva fissato come chiamare JEV per riordinare i passaggi (forma A, 16 richieste in volo, pin `jev-1.13.0`). Restavano due domande. La prima: serve una **baseline**, cioè un riordino senza JEV, per due scopi. Uno è misurare nell'evaluation quanto JEV aggiunge, l'altro è avere un ripiego quando JEV non risponde. La seconda: la RTX 3050 da 6 GB regge insieme il servizio di embedding, un eventuale reranker locale e Docling su GPU? Questo piano risponde con misure, chiude lo spike S-04 con l'indice dei due RISULTATI e propone ADR-0009.

Le risposte, in breve:

- **Baseline: no-rerank.** Il reranker `bge-reranker-v2-m3` su GPU in TEI migliora molto l'ordine: nDCG@5 da 0,5777 a 0,7338, cioè +0,1561 contro un margine richiesto di +0,02. Però riordinare 20 candidati costa 360 ms al p95, contro un tetto di 300 ms. La regola scritta prima chiedeva entrambe le cose insieme, quindi la baseline è l'ordine denso di S-03 così com'è. Su CPU lo stesso modello dà la stessa qualità, ma con un p95 di 32 secondi.
- **JEV resta il rank di prodotto.** Sugli stessi candidati: nDCG@5 0,7649 e recall@8 0,7973, cioè +0,1872 e +0,0541 sulla baseline, con un p95 di 483 ms. Le 4.437 richieste sono costate 0,130760 USD.
- **VRAM.** Con tei-embed, tei-rerank e Docling caricati e sotto carico insieme per 3 minuti, la GPU arriva a 5.562 MiB su un tetto di 5.632. Non ci sono OOM e i parametri di batch restano quelli di default. Il margine però è di soli 70 MiB, e dipende da quanto occupa il desktop di Windows.
- **No-leak.** Su 887 righe di misura, 0 passaggi non visibili al ruolo sono stati inviati a TEI o a JEV.

ADR-0009 propone quindi tre cose. Il no-rerank come adapter «identità» della porta `Reranker`, usato per confronto e come ripiego. JEV forma A come rank. Un budget VRAM di 5,5 GiB, desktop compreso, con solo tei-embed e Docling sulla GPU nell'MVP. Resta *Proposed* fino alla tua ratifica nel consolidamento (01-17, Task 2). Il piano porta PLAT-07 per la parte reranker del criterio 4.

## Come

### Tre concetti prima del codice

- **Reranking con un cross-encoder.** Il retrieval denso di S-03 confronta due vettori calcolati separatamente, uno per la domanda e uno per il chunk. È veloce, ma «grossolano». Un *cross-encoder* come bge-reranker-v2-m3 legge invece domanda e chunk **insieme**, nello stesso passaggio del modello, e restituisce un punteggio di pertinenza della coppia. È molto più preciso, ma costa un passaggio del modello per ogni candidato. Per questo lo si usa solo per riordinare i pochi candidati già trovati (qui 20), non per cercare in tutto il corpus.
- **TEI `/rerank`.** Text Embeddings Inference (TEI) è il server di Hugging Face che già usiamo per gli embedding; con un modello reranker espone `POST /rerank`. Si manda `{"query": "...", "texts": [t0, t1, …], "raw_scores": false}` e si riceve `[{"index": 2, "score": 0.91}, {"index": 0, "score": 0.40}, …]`. La lista è ordinata per punteggio, e `index` è la posizione del testo **nella richiesta**. Con `raw_scores: false` il punteggio passa da una sigmoide e sta fra 0 e 1. Il punto delicato è ricollegare ogni `index` al candidato giusto: un errore qui produce un nDCG plausibile ma falso, che deciderebbe la baseline (RISK-4 del design).
- **nDCG@5 e recall@8.** Per ogni query c'è un insieme di chunk rilevanti, deciso dagli oracoli ratificati. *recall@8* è la frazione dei rilevanti che compare nei primi 8 posti: misura se il rilevante c'è. *nDCG@5* guarda i primi 5 posti e premia di più un rilevante in alto. Ogni rilevante vale 1/log2(posizione + 1), e la somma si divide per il valore dell'ordine ideale: 1,0 vuol dire «tutti i rilevanti in cima», 0 «nessuno nei primi 5». Qui il riordino lavora solo sui 20 candidati portati dal retrieval: se un rilevante non è fra i 20, nessun metodo può recuperarlo. Per questo le 15 query senza rilevanti fra i candidati valgono 0 per tutti (RISULTATI-rerank.md:25).

### Percorso di lettura consigliato

1. **Le soglie e il metodo** (`spikes/s04-jev/RISULTATI-rerank.md:12-46`), nel commit `4407e74`, prima di qualunque numero. R-1 (`:16`) è la regola che decide la baseline. Le due condizioni sono legate da un **e**: non basta guadagnare in qualità se si sfora la latenza. R-3 (`:18`) dice che la VRAM si misura come totale della GPU, desktop compreso. Il Metodo spiega come si attribuisce la VRAM ai singoli servizi (`:45`) e quando si dichiara un OOM (`:46`). Che non siano cambiate lo verifichi confrontando le sezioni Soglie e Metodo fra `4407e74` e `a7341dc`: sono identiche, il file cresce solo in fondo.

2. **La funzione centrale, `rank_query`** (`spikes/s04-jev/src/bin/rerank/main.rs:95-170`).
   - **Guardia di visibilità prima dell'invio** (`:103-104`). `assert_visible` (libreria di 01-11) controlla che **ogni** passaggio dell'insieme sia visibile al `role_profile` della query, secondo `acl.json`. Se anche uno solo non lo è, la funzione restituisce un errore e non parte nessuna richiesta. La chiamata sta prima del `match` sul metodo, quindi vale per tutti e tre, anche per `none`, che non manda niente a nessuno. Così nessun ramo futuro può «dimenticarla». Il `?` dopo `.with_context(...)` è l'operatore di Rust che, su un errore, esce subito dalla funzione restituendolo, con il contesto aggiunto al messaggio.
   - **Riconteggio sul punto di invio a TEI** (`:113-119`). Subito prima della chiamata HTTP il bin riconta in modo indipendente i passaggi non visibili, e annulla l'invio se ne trova. È ridondante con la guardia, ed è voluto: è difesa in profondità, la stessa che il client JEV di 01-11 fa dal suo lato (`:128`, `:133`). Il valore finisce nella colonna `unauthorized_sent` del CSV.
   - **JEV** (`:123-162`). `rank_form_a` manda 20 richieste, una per candidato, con al massimo 16 in volo e una deadline globale di 2 s. Poi il bin ordina per il segnale `relevant`. `total_cmp` confronta due `f64` dando un ordine totale anche ai casi strani come NaN, e `.then(a.0.cmp(&b.0))` scioglie i pareggi con la posizione d'ingresso, cioè l'ordine denso. `model_resolved` raccoglie la versione davvero risolta da JEV, `jev-1.13.0` su tutte le righe.

3. **Il client TEI** (`spikes/s04-jev/src/bin/rerank/tei_rerank.rs`).
   - `TeiReranker::new` (`:42-65`) accetta l'URL solo da `TEI_RERANK_URL` (`from_env`, `:68-71`). Rifiuta schemi diversi da http/https, URL senza host e URL con credenziali o query. Non c'è nessun endpoint scritto nel codice.
   - `request_body` (`:122-129`) mette i testi nell'ordine dei candidati: è quell'ordine che dà significato a `index`.
   - `ranked_ids` (`:135-163`) è il cuore di RISK-4. Pretende che la risposta sia una **permutazione completa** dei candidati inviati: tante voci quanti candidati, nessun indice fuori intervallo, nessun indice ripetuto, nessun punteggio infinito o NaN. Qualunque altra cosa è un errore, mai un ordine «completato a caso». È la scelta P-5 del test-write. Poi ordina per punteggio decrescente e, a parità, per indice (`:158`). Serve perché TEI 1.9.4 non conserva l'ordine d'ingresso fra punteggi uguali (RISULTATI-rerank.md:52).
   - `e.without_url()` (`:80`, `:105`) toglie l'URL dai messaggi d'errore di reqwest, così un errore non stampa l'indirizzo del servizio.

4. **La verifica dei candidati** (`main.rs:203-253`). I candidati vengono da `export-candidates` del piano 01-10, rigenerati in ogni worktree. `verify_candidates_sha` calcola lo sha256 del file e lo confronta con quello di `candidates_manifest.json`, che appartiene a 01-10 e non si scrive mai. Se divergono, il bin si ferma mostrando i due hash. L'unica deroga è `--accept-sha <hex>`, che passa solo se l'hash dato è **uguale** a quello calcolato (`:227-231`): non esiste un jolly tipo «accetta qualunque cosa». Nella misura gli sha coincidevano e la deroga non è servita (RISULTATI-rerank.md:53).

5. **Rilevanza e caricamento** (`main.rs:293-483`). La regola di rilevanza di S-03 è rifatta qui (`relevant_to`, `:372-399`), perché i crate degli spike non si importano fra loro. Un candidato è rilevante se sta nello stesso documento e nella stessa pagina o sezione, e se contiene l'excerpt normalizzato o almeno metà dei suoi caratteri come sottostringa contigua. `longest_common_substring` (`:352-367`) usa la programmazione dinamica classica con due sole righe di tabella, scambiate a ogni giro con `std::mem::swap`, così la memoria resta proporzionale alla lunghezza di una stringa sola. `load_sets` (`:404-483`) applica la guardia già al caricamento (`:474-475`) e controlla che il ruolo dell'export coincida con quello dell'oracolo (`:434-440`). La regola è stata verificata contro un'implementazione Python indipendente e contro `query-bench` di S-03 (RISULTATI-rerank.md:54-57).

6. **Il ciclo di misura** (`main.rs:612-744`). Per ogni ripetizione, query e metodo il ciclo chiama `rank_query`, calcola nDCG@5 e recall@8 con la libreria di 01-11 (`:686-687`) e scrive una riga di CSV (`:690-699`). In fondo il processo fallisce se anche un solo passaggio non visibile è stato inviato (`:737-739`) o se un riordino non è riuscito (`:740-742`). La modalità `--duration-s` ripete le query a ciclo per un tempo dato e serve da carico per `vram.sh`.

7. **La misura della VRAM** (`spikes/s04-jev/vram.sh`).
   - **Si avvia solo in una sessione prenotata**: senza `CKA_RUN_ID` esce con 2 (`:38-41`), e si ferma se gira un container con un'altra etichetta `cka.run_id` (`:106-107`). Lo spegnimento lo fa `session.sh with`, che rimuove solo i container della propria sessione, anche su errore o Ctrl-C.
   - **Segreti fuori dall'ambiente** (`:55-65`). Dal file dei segreti servono solo i due `max-batch-tokens`. `read_env` lo carica dentro una *subshell*, cioè le parentesi `( … )`, che è un processo figlio: le variabili caricate lì spariscono quando la subshell finisce. Ne esce solo il valore stampato con `printf`. `${!1:-$2}` è l'espansione indiretta di bash: legge la variabile il cui **nome** è nel primo argomento, con il secondo come default.
   - **Campionamento con `nvidia-smi`** (`:113-121`). Ogni mezzo secondo un ciclo in background legge `memory.used` dell'intera GPU e lo annota con la fase corrente, letta da un file che le fasi riscrivono (`:111`). Su Windows con Docker Desktop i container girano dentro la VM di WSL2, e `nvidia-smi --query-compute-apps` non vede i loro processi: risponde «No running processes found» o elenca solo programmi di Windows. Lo script ne salva quindi solo un'istantanea per fase, come documentazione (`:124-127`).
   - **Attribuzione per servizio** (`:228-250`). Non potendo leggere la memoria di ogni container, si avviano i servizi **uno alla volta** (fasi `warm-<servizio>`, `:172-179`), ognuno con il suo carico per 30 s. La quota di un servizio è il picco della sua fase meno il minimo della stessa fase. Nelle fasi successive la quota vale costante, mentre il totale delle righe resta la lettura vera della GPU. Nella fase `load` (`:184-192`) i tre carichi partono insieme in background, con `&`, e `wait` raccoglie il codice d'uscita di ciascuno. Il CSV si costruisce con un `awk` che legge due volte lo stesso file (`"$WORK/samples" "$WORK/samples"`, `:250`). La condizione `NR == FNR` è vera solo durante la prima lettura e serve a calcolare minimi e massimi per fase; nella seconda lettura scrive le righe con le quote.
   - **`max_batch_tokens`**. È il numero massimo di token che TEI mette in un solo lotto sulla GPU. La memoria di attivazione cresce con il lotto, quindi abbassarlo riduce il picco di VRAM al prezzo di più passaggi. Il metodo diceva di abbassarlo solo se il tetto veniva superato. Con i default (16384 per l'embedding, 8192 per il reranker) il tetto è stato rispettato, e le configurazioni ridotte non sono state misurate.
   - **OOM** (`:202-216`): il valore è vero se un servizio non è più in esecuzione alla fine o se un log contiene un errore di memoria.

8. **La sintesi.** `spikes/s04-jev/RISULTATI-rerank.md` (Setup `:48-65`, Risultati `:67-109`, Anomalie `:111-123`, Esito parziale `:125-134`), poi `spikes/s04-jev/RISULTATI.md` (Esito `:13-26`, porta `Reranker` `:28-34`, threat model `:36-45`) e `docs/adr/0009-reranker-baseline.md` (Decisione `:54-62`).

### Flusso dei dati

`export-candidates` di 01-10 rigenera i top-20 autorizzati. Il bin `rerank` ne verifica lo sha, li lega agli oracoli e li riordina con i tre metodi, 3 volte, scrivendo `rerank.csv`. Poi, con `tei-rerank` fermato e un container TEI CPU al suo posto, si ripete il solo metodo `tei` come ripiego. Questa prima sessione è `01-15-20261025t182756z-d090`, e il suo `hw.json` porta il commit `d530189`. In un'altra sessione, `01-15-20261026t100823z-f6a0` sul commit `86353bc`, `vram.sh` avvia i tre servizi uno alla volta, li carica insieme e scrive `vram.csv`.

## Perché

- **Soglie prima delle misure** (RISK-3). Con un guadagno di 0,1561 sarebbe stato facile convincersi che 360 ms «vanno bene lo stesso». La regola scritta prima impedisce proprio questo: o si cambia la soglia con una decisione esplicita, oppure vale. Qui vale.
- **No-rerank invece del reranker su CPU come ripiego.** Il CPU ha la qualità del GPU, ma un p95 di 32 s, fuori da qualunque risposta interattiva. Un ripiego che non risponde in tempo non è un ripiego.
- **No-rerank invece di riservare VRAM al reranker.** Con i tre servizi il margine è di 70 MiB: basta un'applicazione in più sul desktop per sforare, come mostra la serie scartata con il desktop a 2,8 GiB (picco 5.851 MiB). Senza reranker locale si liberano circa 1,5 GiB, e la superficie di rischio T-14 del reranker locale sparisce.
- **JEV resta il rank** (ADR-0002, LOCKED). La baseline non lo sostituisce: misura quanto vale e copre i guasti. Rispetto al reranker locale JEV guadagna ancora +0,0311 di nDCG@5.
- **Guardia anche su `none` e riconteggio sul punto di invio** (D-16, RISK-1). La regola di progetto dice che nessun chunk non autorizzato raggiunge un reranker, locale o remoto. Il reranker TEI è locale, ma la regola non fa eccezioni. Tenere la guardia fuori dal `match` la rende impossibile da saltare. Il riconteggio indipendente fa sì che un errore nella guardia non passi inosservato.
- **Risposta di TEI che deve essere una permutazione.** L'alternativa, cioè ignorare gli indici mancanti o completare con l'ordine denso, avrebbe nascosto un problema di contratto dentro una metrica. Un errore esplicito ferma la misura.
- **Sha dei candidati con deroga esplicita** (RISK-5). Il manifest è di 01-10 e non si tocca. Senza `--accept-sha` l'esecutore avrebbe avuto due strade cattive: un bin che rifiuta un file rigenerato legittimo, oppure una verifica aggirata in silenzio.
- **VRAM attribuita per avvio successivo.** Su WSL2 non c'era un'alternativa misurabile per container. Il limite è dichiarato: la somma delle quote (4.292 MiB) supera di 337 MiB l'aumento reale sotto carico (3.955 MiB), perché i picchi di avvio non coincidono. Per questo la quota a due servizi dell'MVP è una stima e non una misura.
- **Serie VRAM non tracciate scartate** (esecutore di ripresa). La serie del 26 alle 00:55 aveva numeri plausibili, con un picco di 5.442 MiB, ma nessun `run_id` né `git_rev` registrato: nessuno poteva dire con quale codice fosse stata prodotta. La rimisura ha riprodotto lo stesso aumento sopra il desktop (3.955 contro 3.963 MiB), e solo quella è entrata in `vram.csv`.

## Test

- **Unitari, sull'host** (13):
  - guardia: `rerank_blocks_invisible_candidate_before_tei` (`main.rs:850`), `rerank_blocks_invisible_candidate_before_jev` (`:882`), `rerank_none_also_refuses_invisible_set` (`:916`). Usano server finti su `127.0.0.1` che contano le richieste ricevute: con un passaggio non visibile devono restare a 0;
  - sha: `candidates_sha_mismatch_refused_without_explicit_override` (`:970`), `candidates_sha_match_needs_no_override` (`:1019`);
  - controlli statici sul sorgente: `test4_key_not_read_by_bin_nor_echoed_by_vram_sh` (`:1090`), `test9_vram_sh_only_session_sh_for_services` (`:1126`), `test14_no_literal_urls` (`:1189`). Guardano solo il codice prima di `#[cfg(test)]` (problemi P-2 e P-3);
  - mapping TEI: `tei_rerank_maps_index_to_candidate` (`tei_rerank.rs:270`), `tei_rerank_rejects_out_of_range_index` (`:290`), `tei_rerank_rejects_incomplete_response` (`:300`), `tei_rerank_request_body_shape` (`:315`), `tei_rerank_posts_rerank_and_maps_reply` (`:329`).
- **Ignorati di evidenza e git** (9): `test3_unauthorized_sent_zero_everywhere` (`main.rs:1340`), `test4_key_value_absent_from_artifacts` (`:1227`), `test5_thresholds_committed_before_results` (`:1357`), `test7_metrics_recomputed_match_results` (`:1413`), `test10_vram_three_services_under_load` (`:1462`), `test11_reranker_revision_and_hw_manifest` (`:1528`), `test12_plan_0111_files_untouched` (`:1561`), `test13_evidence_and_links` (`:1588`), `test14_security_scanner_clean` (`:1214`).
- **Ignorati che non girano qui** (2): `test9_vram_sh_exits_2_without_run_id` (`:1166`) e `test9_vram_sh_respects_foreign_services_manual` (`:1181`).

**Come lanciarli.** Gli unitari, dalla cartella `spikes/`:

```bash
cargo test -p s04-jev --locked --offline --bin rerank
```

Per un test solo, aggiungi il nome dopo `--`, per esempio `-- tei_rerank_maps_index_to_candidate`. Per gli ignorati di evidenza e git, sempre sull'host:

```bash
cargo test -p s04-jev --locked --offline --bin rerank -- --ignored test3_ test5_ test7_ test10_ test11_ test12_ test13_ test14_security
```

`test4_key_value_absent_from_artifacts` vuole la chiave nell'ambiente. La carichi in una subshell, così non resta nella shell:

```bash
( set -a; . "$HOME/.cka-spikes/spikes.env"; set +a; cargo test -p s04-jev --locked --offline --bin rerank -- --ignored test4_key_value )
```

L'equivalente manuale di `test9_vram_sh_exits_2_without_run_id`, da Git Bash nella radice del repo, è `env -u CKA_RUN_ID bash spikes/s04-jev/vram.sh; echo $?`. Deve stampare il messaggio e `2`.

**Esito.** Il gate dell'orchestratore sul main fuso `a7341dc` è verde su tutto: `cargo fmt --check`, `clippy -D warnings`, `cargo test -p s04-jev` (libreria 16 verdi, bin 13 verdi e 11 ignorati) e lo scanner con soglia HIGH (0 rilevamenti su 18 file). L'orchestratore non ha rilanciato gli ignorati. Il bridge ha rilanciato sull'host i 13 unitari e 9 ignorati, tutti verdi, e l'equivalente manuale di `test9_vram_sh_exits_2_without_run_id`, che esce con 2. I due test di TEST-9 restano al giudizio umano nel SUMMARY. Il primo non può girare su Windows, perché `Command::new("bash")` trova il bash di WSL e non Git Bash. Il secondo è una prova manuale con Docker e GPU: l'unica traccia è il messaggio del commit `d7cbb20` (container estranei rifiutati, Ctrl-C che ferma solo la sessione), e nessuno l'ha ripetuta.

**Cosa non provano.** I test controllano le regole e la forma delle evidenze, non le misure: quelle sono i CSV, che il bridge ha ricalcolato a parte. Alcune prove stanno solo nei file grezzi (`results/raw/`), che git ignora e che oggi esistono solo nel worktree dell'esecutore: la spesa JEV, il log della sessione VRAM con `git_rev` e il blob di `vram.sh`, i log dei servizi con revisioni e batch, i 387 riordini TEI dei carichi.

## Deviazioni e punti aperti

- **Interruzione e ripresa.** Il primo esecutore si è fermato dopo `86353bc`, alla chiusura della sessione. Ha lasciato un `vram.csv` non committato e `RISULTATI.md` e ADR-0009 con segnaposto. L'esecutore di ripresa ha scartato le due serie del `vram.csv` come non tracciabili, le ha spostate in una cartella raw ignorata, ha rimisurato e ha compilato i segnaposto. Il manifest della sessione VRAM è stato generato a sessione chiusa ed è rimasto in raw, perché il piano prevede un solo `hw.json` versionato.
- **Legame fra misure e codice.** La misura di rerank è ben legata: `hw.json` porta `d530189`, e dopo quel commit il codice non cambia più, quindi è il codice di HEAD. In 01-14 non era così. La misura VRAM invece è legata al commit `86353bc` solo dal testo di RISULTATI e dai file raw. Il bridge li ha letti e confermano, ma spariranno con il worktree: conviene decidere se conservarli.
- **Ripiego CPU**: 147 righe invece di 148, per un timeout su Q-042. **Q-075** ha 19 candidati, cioè tutti quelli visibili al suo ruolo, e non è mai stata riempita.
- **Piccole imprecisioni di testo.** RISULTATI.md e ADR-0009 dicono che il desktop occupa «da 1,5 a 2,9 GiB», ma i valori osservati sono 1.479-2.869 MiB, cioè 1,44-2,80 GiB. «Circa 1,65 GiB» per 1.677 MiB è in realtà 1,64. RISULTATI.md cita ADR-0012, che non esiste ancora. ADR-0009 porta la data del 25, ma è entrato in git il 26.
- **Da decidere o misurare più avanti:**
  - il margine di 70 MiB con i tre servizi, che dipende dal desktop;
  - la quota tei-embed + Docling dell'MVP (circa 2,8 GiB), che è una stima per attribuzione e va misurata in uno scenario a due servizi;
  - il p95 del reranker TEI sotto contesa, 1.045 ms;
  - le configurazioni di batch ridotte, mai misurate;
  - la ratifica di ADR-0009 e la riga «Reranker baseline» di `DEPENDENCIES.md` nel consolidamento (01-17, Task 2);
  - la stesura di ADR-0012 per la porta `Reranker`.

**Modifica:** Claude (Anthropic) · 2026-10-26 · creazione (bridge di 01-15, passo 3-bis)
