# Racconto del piano 01-18: misure di isolamento, casi ostili, budget e OOM (S-02)

## Cosa

Il piano 01-07 ha costruito il lanciatore isolato: un processo figlio per documento, con limiti del kernel, un budget di memoria condiviso e un prescan che guarda il PDF prima di PDFium. Questo piano non scrive codice del lanciatore. Lo mette alla prova dall'esterno e risponde a quattro domande, con soglie fissate prima di misurare:

- **i file ostili restano contenuti?** (M8) Sì per il contenimento: 26 casi su 26, padre sempre vivo, nessun processo residuo, fallimento entro 238 ms. No per l'esattezza degli esiti: 24 casi su 26 danno l'esito previsto, mentre `deep-nesting` e `jbig2-malformed` danno `ok`. La soglia chiedeva il 100%, quindi M8 è dichiarata non raggiunta, e così hai deciso di lasciarla (D-34);
- **quanti job `parse` reali possono girare insieme senza rischi?** (M10) Due, nel worker da 2 GiB. Gli 8 batch, benigni e ostili a concorrenza richiesta 1, 4, 8 e 16, sono tutti conformi;
- **se un figlio supera la memoria prenotata e il cgroup va in OOM, muore il figlio o il padre?** (M11) Il figlio. È stato provato in un container da 256 MiB;
- **quanto costa avere un processo per documento?** (M9) Il 2,7% del tempo di parsing del documento da 25 pagine, con una riserva su cosa misura la colonna.

In più il piano fa da **collaudo di 01-07**: confronta 33 voci del suo contratto con ciò che si è misurato. Tutte le voci esercitate sono conformi; due difetti minori dei driver di misura vanno alla gap-closure 1.4 (D-35).

Il piano porta PLAT-07 per la parte «file ostili e isolamento» del criterio 2. L'esito complessivo di S-02 e l'ADR-0004 li scriverà 01-13.

## Come

### Percorso di lettura consigliato

L'ordine segue la vita di una misura: prima le regole, poi i casi, poi le misure, infine il resoconto.

1. **`spikes/s02-parser/RISULTATI-isolamento.md`, sezione Soglie** (`RISULTATI-isolamento.md:9-40`). È il primo file scritto, nel commit `4a28697`, prima di qualsiasi numero. Contiene M8 (`:15`), M9 (`:16`), la definizione completa di M10 (`:22`) e di M11 (`:24`), e la frase che limita il selftest alla «correttezza aritmetica» (`:20`). Le soglie del lanciatore sono copiate da 01-07 (`:30-40`) e non si ridefiniscono qui. Che non siano state toccate dopo lo puoi controllare tu stesso: `git diff 4a28697 3fb03bc -- spikes/s02-parser/RISULTATI-isolamento.md` aggiunge solo sezioni dopo le Soglie.

2. **`spikes/data/hostile/manifest.json`**: 26 casi, ciascuno con `expected_status` e `expected_kind_or_reasons` scritti come array di stringhe. Per esempio `deep-nesting` accetta `["Corrupt","Crashed"]`, cioè l'una o l'altra. Lo `sha256` di ogni file fissa la generazione che è stata misurata. Accanto c'è `.security-scan-fixture`, il marcatore che dice allo scanner che quella cartella contiene solo dati ostili e nessun codice.

3. **`spikes/s02-parser/hostile-gen/gen_hostile.py`**: come nascono i casi. Il generatore scrive i PDF byte per byte, senza librerie PDF, perché un caso ostile deve essere esattamente quello dichiarato e non quello che una libreria ritiene sensato scrivere.
   - `build_modern` (`gen_hostile.py:146-177`) costruisce un PDF 1.7 con object stream veri e una cross-reference stream compressa. Le voci di tipo 2 della xref (`:167-168`) dicono «l'oggetto n sta dentro l'object stream m, alla posizione k»: è il meccanismo con cui un PDF nasconde oggetti dentro uno stream compresso.
   - La cifratura passa da `qpdf` (`:257-268`) con password generate a runtime da `secrets.token_hex` (`:271-273`). `secrets` è il modulo della standard library pensato per valori segreti; `random` non lo è. Le password non finiscono su disco né nel manifest.
   - **I controlli di costruzione** sono la parte da notare. `case_objstm_sum_bomb` (`:391-411`) vuole nove object stream da 60 MiB, ciascuno sotto il tetto per stream di 64 MiB e con rapporto di compressione sotto 100, ma con somma oltre 512 MiB. Se la costruzione sbaglia anche solo una di queste condizioni, il generatore si ferma con `BuildError` (`:405-410`) invece di produrre un caso diverso. Lo stesso fa `case_pdf_objstm_hidden_js` (`:519-527`): se la parola `JavaScript` compare in chiaro nel file, il caso non prova più che il prescan veda dentro gli object stream, quindi il generatore si rifiuta.
   - Un costrutto Python poco ovvio sta in `:401`: `lambda size=size, seed=seed: low_entropy_lines(size, seed)`. Una lambda in un ciclo, in Python, legge le variabili al momento in cui viene chiamata, non quando viene creata: senza i valori di default tutte e nove userebbero `size` e `seed` dell'ultimo giro. Con `size=size` il valore si fissa alla creazione. La lambda serve a produrre i 60 MiB solo quando servono, come generatore.
   - `case_encrypted_object_dropped` (`:300-324`) prende un PDF già cifrato da qpdf e cambia la parola `obj` in `obx` nell'intestazione di un oggetto citato solo dal Catalog. lopdf lo scarta senza dirlo; il prescan di 01-07 deve accorgersene con la riconciliazione della xref. La regex cerca il Catalog fino a `endobj` (`:312`), perché quello scritto da qpdf contiene dizionari annidati e una regex più corta si fermava a metà.
   - `generate` (`:682-697`) scrive ogni file prima come `.part` e poi lo rinomina (`part.replace(dest)`, `:694`): un file a metà non prende mai il nome definitivo. `verify_manifest` (`:729-740`) confronta gli sha256 con il manifest prima delle misure.

4. **`spikes/s02-parser/measure/budget_real.py`**: i job reali e il calcolo di M10.
   - `jobs` (`budget_real.py:95-119`) scrive un job `parse` per riga, in JSON Lines, con J = max(2 × N, documenti della classe) (`:101`), così a concorrenza 16 ci sono almeno 32 job e il limiter ha davvero da scegliere. I benigni sono i PDF e DOCX del manifest di 01-03 presenti in `raw/` (`:77-87`): in pratica i 10 PDF, perché i DOCX stanno altrove.
   - `reservation_bytes` (`:125-133`) rifà la formula di 01-07: `memory_reservation_bytes` × (1 + discendenti) + quota d'uscita. Se il profilo non ha `memory_reservation_bytes` esce con errore, senza ripiegare sul limite di memoria virtuale.
   - `summarize_dir` (`:186-290`) legge rapporti, cgroup prima e dopo, exit di `batch` e stato del padre. Il cuore è l'espressione di M10 (`:259-269`): tutte le condizioni in `and`, e il rapporto prenotazione/RSS ≥ 1 conta solo se la classe è `benign` (`:268`). Il margine del cgroup e l'evento `max` si calcolano (`:249-258`) ma non entrano in `m10`: è la scelta scritta nella soglia, perché `memory.peak` comprende anche page cache e tmpfs.
   - `p95` (`:160-164`) usa il metodo «nearest rank»: ordina, prende l'elemento in posizione ⌈0,95 × n⌉. Fino a 19 valori il p95 coincide col massimo, ed è per questo che nelle righe benigne con 10 e 16 job di `budget-real.csv` `peak_rss_kb_max` e `peak_rss_kb_p95` sono uguali; con 32 job il p95 è il trentunesimo valore.

5. **`spikes/s02-parser/measure/run_budget_real.sh`**: il ciclo di un batch, in cinque passi commentati in testa (`run_budget_real.sh:9-17`).
   - Classe, N e tag si validano prima di ogni comando (`:29-32`).
   - Il worker si ferma e si riavvia (`:54-56`). Il motivo: `memory.peak` è il picco dalla nascita del cgroup e l'utente 10001 non lo può azzerare. Un container nuovo ha un cgroup nuovo, quindi un picco che parte da zero. Lo stop passa da `session.sh cid`, perché `stop` vuole l'identificativo di un container e non il nome di un servizio.
   - Il batch gira con `|| code=$?` (`:63-65`). Con `set -e` un comando che fallisce chiude lo script; `|| code=$?` cattura l'exit e lascia proseguire, così le letture del cgroup e del padre dopo il batch (`:69-74`) avvengono anche se il batch fallisce. Senza, un batch fallito lascerebbe una misura senza il suo «dopo».

6. **`spikes/s02-parser/tests/oom.rs`**: la prova più delicata.
   - `#![cfg(target_os = "linux")]` (`oom.rs:12`) compila il file solo su Linux: cgroup, `prctl` e `/proc` non esistono su Windows.
   - La logica è divisa in due. `classify_m11` (`:101-127`) è una **funzione pura**: riceve un'osservazione (`Observed`, `:73-89`) e restituisce l'esito tristato. Se il contatore `oom_kill` non è salito, l'esito è `NonEsercitato`, qualunque cosa dica il resto (`:102-107`). Altrimenti controlla dieci condizioni in ordine (`:108-122`) e, alla prima falsa, dà `Fallito` con il nome del controllo. `result_json` (`:136-166`) costruisce la riga `OOM_RESULT` con le 15 chiavi di `RESULT_KEYS` (`:35-51`). Separare il giudizio dalla misura permette di provare il giudizio con 11 test normali, senza container.
   - Il test vero è `oom_kills_child_not_parent` (`:225-335`). `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]` (`:225`) lo fa girare in un runtime tokio con due thread, come `parse-parent`. `#[ignore = "…"]` (`:226`) lo esclude dal `cargo test` normale: gira solo se lo si chiede con `--ignored`.
   - I passi seguono il piano. Il test si dichiara subreaper (`:229`), come fa `parse-parent`, così i discendenti orfani tornano a lui. Legge `memory.max` (`:234`) con la forma `let … else`: se il valore non c'è, il ramo `else` deve uscire dalla funzione, e qui esce con `non_esercitato`. Poi chiede un'allocazione di 2 × `memory.max` (`:260`, `div_ceil` è la divisione arrotondata per eccesso) con una prenotazione di soli 64 MiB (`:286`) e un budget di 4 × `memory.max` (`:293`). Così il limiter ammette il job e l'unico limite che interviene è quello del cgroup: è proprio lo scenario del rischio residuo di 01-07, un figlio che supera la propria prenotazione.
   - `env!("CARGO_BIN_EXE_fake-child")` (`:250`) è una macro che Cargo riempie a compilazione con il percorso del bin `fake-child` dello stesso crate. Le variabili d'ambiente si impostano in un blocco `unsafe` (`:254-258`): in Rust 2024 `set_var` è `unsafe`, perché modificare l'ambiente mentre un altro thread lo legge è un comportamento indefinito. Il commento `SAFETY` spiega perché qui non succede.
   - Se `run_child` fallisce, `downcast_ref::<JobError>()` (`:299`) guarda dentro l'errore generico di `anyhow` per capire se è un `JobError` e di che tipo: `BudgetExceeded` vuol dire «non ammesso», quindi di nuovo `non_esercitato`.
   - Dopo il ritorno, `unreaped_children` (`:195-205`) chiama `waitpid(-1, WNOHANG)` finché il kernel risponde `ECHILD`, cioè «non hai figli»: ogni altra risposta è un figlio rimasto da raccogliere. `saturating_sub` (`:315`) sottrae senza andare sotto zero.

7. **`spikes/s02-parser/measure/run_oom.sh`**: il container della prova.
   - Compila il test nel runner e ne estrae il percorso dai messaggi JSON di Cargo (`run_oom.sh:44-58`).
   - Trova il volume `/target` per etichetta (`:61-66`) e mai con `docker compose config`, che stamperebbe i segreti del runner.
   - Lancia il container (`:71-76`): 256 MiB di memoria e altrettanti di swap (così lo swap non allunga il limite), niente rete, filesystem read-only, nessuna capability, utente 10001, al più 256 processi, l'etichetta della sessione per la pulizia.
   - Scrive `oom.json` dalla riga `OOM_RESULT` (`:79-99`), dopo averlo cancellato all'inizio (`:41`), così un file vecchio non può passare per buono.

8. **`spikes/s02-parser/RISULTATI-isolamento.md`, dal Setup in giù** (`:42-233`). Le tabelle sono la trascrizione dei CSV; le sezioni da leggere con attenzione sono la Validazione di 01-07 (`:151-191`), le Anomalie (`:193-211`) e l'Esito parziale (`:213-225`).

### Flusso dei dati

`gen_hostile.py` genera i 26 file nel runner senza rete e scrive gli sha256 nel manifest. Nel worker, `parse-parent hostile` legge il manifest e lancia un job `parse` per caso e ripetizione: ne esce `hostile.csv`. `parse-parent budget` lancia job `selftest` e scrive `budget.csv`. Per il budget reale, `budget_real.py jobs` scrive i job, `run_budget_real.sh` li passa a `parse-parent batch` dentro un worker appena riavviato e salva rapporti e cgroup nei grezzi, poi `budget_real.py summarize` produce `budget-real.csv` e `budget-real-jobs.csv`. `parse-parent overhead` scrive `overhead.csv`. `run_oom.sh` esegue `oom.rs` nel container da 256 MiB e scrive `oom.json`. Infine RISULTATI raccoglie tutto.

## Perché

- **Soglie ed esiti attesi prima della misura** (T-01-18-04). Se gli attesi si scrivessero dopo aver visto i risultati, un caso andato male diventerebbe «atteso» senza che nessuno se ne accorga. Per questo il commit `4a28697` precede quelli delle misure, e i due `ok` inattesi restano `pass=false`. È la stessa regola che ha portato a dichiarare M8 non raggiunta invece di allargare gli attesi.
- **Generatore a mano invece di una libreria PDF.** Una libreria tende a «riparare» il file o a scriverlo in una forma sua. I casi ostili vivono di dettagli precisi: una voce di xref di tipo 2, un'intestazione `obx`, una catena `/FlateDecode /FlateDecode`. Il prezzo sono 769 righe di Python, compensate dai controlli di costruzione.
- **Budget misurato sui job reali, non sul selftest** (RISK-001 HIGH di risk_audit_17). Il selftest alloca una quantità fissa di memoria: prova che il limiter conti bene, non che la prenotazione basti per PDFium su un PDF vero. Per questo la concorrenza pubblicata per 01-13 viene solo da `budget-real.csv`.
- **Il margine del cgroup registrato ma non soglia** (RISK-001 MEDIUM di risk_audit_25). `memory.peak` comprende memoria che il kernel può recuperare, come la page cache. Usarlo come soglia avrebbe dato falsi allarmi. Il rischio che resta, cioè che input futuri diversi consumino di più, si gestisce dichiarando la portata («vale per questi carichi») e chiedendo di ripetere M10 se cambiano workload, memoria o limiti.
- **OOM provocato, non atteso** (RISK-002 MEDIUM di risk_audit_17). Lasciare «non esercitato» il percorso OOM avrebbe lasciato aperta T-01-07-06, il rischio residuo accettato da te nel ripasso di 01-07: la prenotazione non è un tetto rigido. Un container a memoria nota è il modo più semplice per provocarlo senza toccare il worker. La vittima si deduce da tre segnali insieme (contatore `oom_kill`, segnale 9, `oom_score_adj` 1000 contro 0), non dal solo SIGKILL, che potrebbe avere altre cause.
- **Esito tristato invece di vero/falso.** «L'OOM non è avvenuto» e «l'OOM è avvenuto e un controllo è fallito» sono cose diverse. Con un booleano la prima si confonderebbe con un successo. `non_esercitato` non si scrive mai come `verificato`.
- **Nessuna modifica a 01-07.** Il piano è il collaudo di 01-07: se correggesse il codice che collauda, non saprebbe più che cosa ha misurato. Le difformità si registrano e vanno a una gap-closure, come prevede la regola degli insiemi di file chiusi.
- **Test puri accanto al test ignorato.** Il piano voleva un solo test in `oom.rs`. Il test-write ha aggiunto 11 test del classificatore nello stesso file, l'unico che il piano gli permetteva. L'alternativa era non provare `classify_m11` fuori dal container, cioè quasi mai. La deviazione è stata accettata.

## Test

- **`tests/oom.rs`, 11 test puri** (girano dovunque ci sia Linux, anche nel runner):
  - `m11_all_checks_with_oom_is_verificato` (`oom.rs:423`), `m11_more_than_one_oom_kill_is_still_verificato` (`:433`), `m11_oom_group_does_not_decide_the_outcome` (`:440`), `m11_reservation_not_below_cgroup_is_fallito` (`:481`): i casi positivi e il confine della prenotazione;
  - `m11_no_oom_kill_is_non_esercitato_with_reason` (`:449`), `m11_without_oom_never_verificato_nor_fallito` (`:458`): senza OOM l'esito non è mai `verificato`;
  - `m11_each_violated_check_with_oom_is_fallito_naming_the_check` (`:469`): per ognuno dei dieci controlli, una mutazione (`violations`, `:359`) deve dare `fallito` con il nome del controllo nel motivo;
  - `result_json_verificato_has_exact_keys_and_types_checked_by_the_plan` (`:508`), `result_json_fallito_carries_reason_and_observed_values` (`:533`), `result_json_precondition_is_non_esercitato_with_nulls` (`:544`), `result_json_is_one_line_without_environment_values` (`:569`): forma della riga, una riga sola, nessun nome di segreto.
- **`oom_kills_child_not_parent`** (`oom.rs:227`), ignorato: lo esegue solo `run_oom.sh` nel container. Esito in `oom.json`.
- **Le misure** non sono test Cargo ma CSV confrontati con le soglie: `hostile.csv` (M8), `budget.csv` (aritmetica del limiter), `budget-real.csv` (M10), `overhead.csv` (M9).

**Come lanciarli.** Dalla cartella `spikes`, i test puri nel runner:

```bash
bash tools/session.sh with 01-18 -- bash -c 'bash tools/session.sh oneshot "$CKA_RUN_ID" -- cargo test -p s02-parser --locked --test oom'
```

Per un test solo si aggiunge il nome dopo `--`, per esempio `-- m11_each_violated_check_with_oom_is_fallito_naming_the_check`. La prova OOM, dalla radice del repo: `bash spikes/tools/session.sh with 01-18 -- bash spikes/s02-parser/measure/run_oom.sh spikes/s02-parser/results/isolation/oom.json`. Le misure si rigenerano con i comandi elencati in `RISULTATI-isolamento.md:53`.

**Esito.** Il gate d'onda dell'integratore nel runner, `cargo test -p s02-parser --locked --offline`, ha dato 180 test verdi, nessun fallimento e un ignorato, quello della prova OOM, per costruzione. Il conteggio comprende anche i test di 01-08, fuso nella stessa onda. Sull'host il resto del workspace dà 96 verdi. `cargo test --workspace` nel runner non si può lanciare: al registry cargo del runner manca `addr2line`, che serve a `dhat` di `s01-upload`. È un limite d'ambiente, non un difetto di questo piano. Lo scanner di sicurezza sui file nuovi esce 0.

**Cosa non provano.** Il ramo di arresto del controllo d'ingresso (TEST-19) non si è presentato, e lo script che lo implementa non è versionato. I controlli (a)-(e) della Validazione e lo schema dei sei campi sono trascritti in RISULTATI, ma i loro log grezzi non esistono più: li devi prendere sulla parola del resoconto, o rieseguirli. Sei voci del contratto di 01-07 non si sono mai presentate.

## Deviazioni e punti aperti

- **M8 non raggiunta, per tua decisione (D-34).** `deep-nesting` e `jbig2-malformed` danno `ok`. Per JBIG2 la spiegazione è plausibile: l'op `parse` estrae solo il testo e PDFium non decodifica l'immagine, quindi il decoder JBIG2 rotto non viene mai toccato. Lo riprova 01-13 con la rasterizzazione. Per l'annidamento profondo la causa non è misurata (`RISULTATI-isolamento.md:92`): lopdf potrebbe caricarlo o scartarlo. Nessun piano lo riprende per ora, e resta scoperto anche nel threat model (T-09).
- **M9 con riserva.** `spawn_init_ms` è il tempo di parete del job meno `parse_ms` del figlio, e `parse_ms` comprende già il caricamento di PDFium. La colonna misura quindi spawn, exec e lavoro del padre, non «spawn + init PDFium» come dice la soglia. Il confronto di throughput con il modo in-process (184,3 contro 46,0 ms per documento, cioè 4 volte) è un tetto superiore che comprende anche il prescan. La gap-closure 1.4 rimisura `overhead.csv`.
- **PDFium nello stesso processo si rompe sotto concorrenza.** A concorrenza 8 il modo in-process dà `FormatError` su un PDF valido, due volte su due corse (`RISULTATI-isolamento.md:145`). È un argomento in più per il figlio per documento.
- **Due difetti dei driver di 01-07 (D-35):** `budget` lascia un file in `/work` sul percorso d'errore (`parse-parent.rs:1049-1050`, `:1116`); `overhead` si ferma al primo errore in-process (`parse-parent.rs:1255`). Vanno alla gap-closure 1.4.
- **Verifica del Task 2 non eseguibile come scritta:** `--out /dev/null` non funziona, perché lo scrittore CSV di 01-01 vuole un `hw.json` accanto. È stata ripetuta con un percorso vero. Il difetto di fondo, `hw-manifest.sh` contro `common::manifest`, è già noto da cinque piani.
- **Parsing di `OOM_RESULT`:** la prima corsa ha scritto `fallito` per un difetto dello script, non del test. Corretto (`run_oom.sh:83-92`) e rigenerato.
- **Deviazioni accettate:** gli 11 test puri in `oom.rs` e gli 8 letterali `memory_reservation_bytes` nei test del budget di 01-07, con le asserzioni invariate.
- **Incoerenza nominale:** il frontmatter di `01-18-PLAN.md` dice `plan_version: 4`, ma il piano eseguito è la v5. Va allineato dal `plan`.
- **Picco RSS benigno sopra la calibrazione:** `pub-it-01` arriva a 344.588 KiB contro i 267.240 della calibrazione di 01-07 (+29%). Il margine 3× della prenotazione regge, ma a 2,33×: la variabilità fra corse va considerata nell'ADR-0004.
- **Restano da decidere o da dichiarare:** il caso limite PDFium con password del proprietario vuota (dal SUMMARY di 01-07, qui non esercitato); `selftest` e `fake-child` nell'immagine di produzione; la variante Windows con Job Objects, fuori scope e da scrivere nell'ADR-0004.
- **Prossimi passi:** 01-13 usa la concorrenza di 2 job `parse`, misura le op OCR con lo stesso metodo di M10 e scrive l'ADR-0004; la gap-closure 1.4 corregge i due driver.

**Modifica:** Claude (Anthropic) · 2026-10-22 · creazione (bridge di 01-18)
