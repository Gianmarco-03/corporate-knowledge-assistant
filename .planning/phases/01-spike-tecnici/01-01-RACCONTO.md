# Racconto del piano 01-01 — l'harness Rust degli spike

## Cosa

Prima di questo piano il repository non conteneva una sola riga di Rust. Adesso esiste un workspace Cargo isolato nella cartella `spikes/`, con otto crate già compilabili: `common`, che è la libreria di misura condivisa, e i sette crate vuoti `s01-upload` … `s07-ports`, che le slice successive riempiranno. Tutte le dipendenze che gli spike useranno sono già dichiarate, approvate da te una per una, pinnate a una versione esatta e risolte in un unico `Cargo.lock`. Questo permette alle slice di lavorare in parallelo senza mai toccare il manifest del workspace o il lockfile.

Il crate `common` offre cinque strumenti: la lettura di variabili d'ambiente obbligatorie, che non stampa mai il valore; la lettura del manifest hardware `hw.json`; uno scrittore CSV che si rifiuta di creare un file di misura senza quel manifest accanto; un istogramma per i percentili di latenza; un cronometro. Il piano dichiara il requisito PLAT-07. In concreto ne realizza le mitigazioni del proprio threat model (supply chain, isolamento dal prodotto, nessuna fuga di segreti, nessuna misura senza hardware dichiarato), non il threat model di progetto: se il requisito sia chiuso lo decide il verifier di fase.

## Come

Ti consiglio di leggere il codice in questo ordine.

1. **`spikes/Cargo.toml:4-6`**: il blocco `[workspace]`. `resolver = "3"` è il risolutore di dipendenze dell'edizione 2024 e tiene conto della versione minima di Rust dichiarata dai crate. `members` elenca gli otto crate a mano, in ordine fisso, senza glob: un crate nuovo non può entrare nel workspace senza che qualcuno modifichi questa riga.
2. **`spikes/Cargo.toml:8-10`**: `[workspace.package]` fissa `edition` e `publish` una volta sola. I crate li ereditano con `edition.workspace = true` (vedi `spikes/common/Cargo.toml:4-5`).
3. **`spikes/Cargo.toml:12-49`**: `[workspace.dependencies]`. Il commento alle righe 13-15 stabilisce la regola della coda append-only. Ogni voce ha la forma `{ version = "=x.y.z", … }`: il segno `=` impone a Cargo quella versione esatta, mentre senza `=` Cargo accetterebbe qualsiasi versione compatibile secondo semver. Alla riga 21 sqlx ha `default-features = false`: si spengono le feature di default (fra cui il TLS) e si accendono solo quelle elencate.
4. **`spikes/common/Cargo.toml:7-13`**: il crate usa le dipendenze del workspace con `<dep>.workspace = true`, senza ripetere la versione. Gli altri manifest seguono lo stesso schema; `spikes/s07-ports/Cargo.toml` non ha dipendenze.
5. **`spikes/common/src/lib.rs:3-7`**: dichiara i cinque moduli pubblici. In Rust ogni `pub mod x;` corrisponde al file `src/x.rs`.
6. **`spikes/common/src/env.rs:12-31`**: `required`. Alla riga 16 si usa `std::env::var_os`, che restituisce un `Option<OsString>`, invece di `std::env::var`. Il motivo è che l'errore di `var`, `VarError::NotUnicode`, contiene il valore grezzo della variabile, e incatenarlo come causa lo farebbe comparire nella stampa `{err:?}`. Alla riga 17 `with_context` (dal trait `anyhow::Context`) trasforma il `None` in un errore che nomina la variabile; la closure `|| format!(…)` viene eseguita solo in caso d'errore. Alla riga 18 `let Ok(value) = … else { bail!(…) };` è un *let-else*: se il pattern non corrisponde si esegue il ramo `else`, che deve uscire dalla funzione (qui con `bail!`, che restituisce subito un `Err`). Le righe 21-29 rifiutano un valore vuoto, uno con spazi in testa o in coda e uno con a-capo interni, e ogni messaggio riporta il nome della variabile, mai il valore.
7. **`spikes/common/src/manifest.rs:7-22`**: `HwManifest`. `#[derive(Deserialize)]` fa generare a serde il codice che legge il JSON. `timestamp_utc` è l'unico campo obbligatorio; gli altri sono `Option`, quindi possono mancare. Le mappe usano `BTreeMap` perché ha un ordine deterministico.
8. **`spikes/common/src/manifest.rs:25-39`**: `MANIFEST_NAME` è una costante privata (vedi le deviazioni). `ensure_present` legge il file e lo deserializza, e ogni passaggio con `with_context` nomina il percorso; alla riga 35 rifiuta un `timestamp_utc` vuoto.
9. **`spikes/common/src/csv.rs:11-43`**: `Writer` contiene lo scrittore del crate esterno `csv`, che dentro un modulo che si chiama anch'esso `csv` va scritto `::csv::Writer` (riga 12). Il `::` iniziale indica il crate esterno e non il modulo corrente. In `create` le righe 24-27 ricavano la cartella del file (con `.` per un percorso senza cartella), e alla riga 28 il manifest viene validato **prima** di `from_path` alla riga 34. È questo ordine a garantire che non resti un CSV parziale. L'header viene scritto subito come prima riga.
10. **`spikes/common/src/csv.rs:46-67`**: `write_row` controlla l'arità prima di scrivere; `write_record` fa `flush` dopo ogni riga.
11. **`spikes/common/src/hist.rs:35-64`**: `Recorder` incapsula un `hdrhistogram::Histogram<u64>` a 3 cifre significative. Alle righe 41-44 il `match` con `unreachable!` sostituisce un `unwrap`: è lo stesso panico, ma con un messaggio che documenta perché quel ramo non può essere raggiunto. Alla riga 52 `saturating_record` registra un valore senza poter fallire. Le righe 66-70 implementano `Default`: clippy lo pretende per un tipo che ha `new()` senza argomenti.
12. **`spikes/common/src/timer.rs:6-25`**: `Stopwatch` usa `Instant`, che è monotono e non risente dei cambi dell'orologio di sistema. Alla riga 19 `u64::try_from(…).unwrap_or(u64::MAX)` converte da `u128` a `u64`; in caso di overflow restituisce il massimo invece di andare in panico.

Il flusso previsto per una slice è questo: si legge una variabile con `env::required`, si misura con `Stopwatch`, si accumula nel `Recorder`, e alla fine `Summary::csv_fields()` (`hist.rs:20-28`) produce i cinque campi `n, p50, p95, p99, max` per `Writer::write_row`. Il `Writer` è stato creato in una cartella dove lo script di sessione ha già scritto `hw.json`.

Nei test di `env.rs` compare il blocco `unsafe { std::env::set_var(…) }` (per esempio alla riga 55). Nell'edizione 2024 modificare l'ambiente di processo è `unsafe`, perché un altro thread potrebbe leggerlo nello stesso momento, e `cargo test` esegue i test in parallelo. Il commento `// SAFETY:` spiega perché qui è sicuro: ogni test usa un nome di variabile che nessun altro test tocca.

## Perché

- **Workspace in `spikes/` e non in radice.** Un `Cargo.toml` in radice avrebbe acceso fmt, clippy e test degli hook git anche sul codice usa-e-getta, e avrebbe reso facile far dipendere un servizio di prodotto da uno spike (RISK-3, T-01-01-01). Il prezzo è che gli hook oggi non controllano `spikes/`, quindi fmt e clippy si lanciano a mano o dal bridge.
- **Tutte le dipendenze dichiarate subito, anche quelle che servono solo dopo.** L'alternativa era lasciare che ogni slice aggiungesse le proprie, ma allora più slice parallele avrebbero modificato lo stesso `Cargo.toml` e lo stesso `Cargo.lock`, con conflitti certi al merge (WIRING W2-W4). Pagare una volta sola la risoluzione delle dipendenze li elimina.
- **Pin `=x.y.z` e checkpoint umano.** Il codice di un crate (compreso `build.rs`) viene eseguito sulla tua macchina già in fase di build. Il 2026-10-19 hai approvato i 34 crate, presentati con i metadati e il repository letti dall'API di crates.io (RISK-1), e il pin esatto impedisce che una versione nuova entri in silenzio (RISK-2).
- **async-trait pinnato ma non attivo.** Il piano lascia a S-07 la scelta fra `async-trait` e il boxing esplicito delle future nelle porte; attivarlo ora avrebbe anticipato quella decisione.
- **Build sull'host invece che nel container `rust:1.99.1-trixie`.** `cargo check` è passato su Windows MSVC, quindi non è servito Docker, e la build non ha interferito con il piano 01-02, che usa Docker in parallelo.
- **Voci di dipendenza tutte in forma tabella su una riga.** Le tabelle multilinea `[workspace.dependencies.axum]` sarebbero state equivalenti per Cargo, ma avrebbero reso inaffidabili i controlli a grep.
- **`var_os` e messaggi costruiti a mano in `env`.** L'alternativa naturale, `std::env::var(name).context(…)`, avrebbe incatenato `VarError` e quindi il valore (RISK-4, T-01-01-02).
- **Manifest validato prima di creare il CSV.** Validarlo dopo avrebbe lasciato un file vuoto o parziale, da cui uno script successivo avrebbe potuto leggere una misura senza hardware dichiarato (RISK-5, T-01-01-03).
- **Flush per riga.** Lasciare il flush al `Drop` dello scrittore sarebbe stato più veloce, ma `Drop` ignora gli errori di I/O e una misura interrotta perderebbe le righe ancora nel buffer. Il costo è trascurabile, perché le righe si scrivono fuori dalla finestra cronometrata.
- **`saturating_record` invece di `record()`.** La firma fissata in W8, `record_us(&mut self, v: u64)`, non restituisce `Result`; dato che l'istogramma si ridimensiona da solo, la saturazione scatta solo oltre il massimo rappresentabile.

## Test

I test unitari stanno nei moduli `#[cfg(test)]` in fondo a ciascun file e si compilano solo con `cargo test`. Li ha scritti rossi un agente test-write distinto dall'esecutore (commit `d2ea32e`, 9 test, tutti in panico su `todo!()`); l'esecutore li ha portati al verde in `fec041f` e ne ha aggiunti tre.

| Test design | Test | Cosa prova |
|---|---|---|
| TEST-6 | `env.rs:38` `env_required_unset_names_variable` | variabile assente: l'errore la nomina |
| TEST-7 | `env.rs:51` `env_required_error_never_contains_value` | valore sentinella con a-capo finale: assente sia da `{err:#}` sia da `{err:?}` |
| TEST-7b | `env.rs:73` `env_required_blank_value_is_rejected_without_leaking` | valore di soli spazi rifiutato |
| aggiunto | `env.rs:94` `env_required_valid_value_is_returned` | un valore valido viene restituito |
| aggiunto | `env.rs:105` `env_required_inner_newline_is_rejected_without_leaking` | a-capo interno rifiutato, sentinella assente |
| TEST-9 | `manifest.rs:57` `manifest_ensure_present_rejects_malformed_json` | JSON malformato e JSON senza `timestamp_utc` rifiutati |
| TEST-8 | `csv.rs:94` `csv_create_without_hw_json_fails` | senza `hw.json` nessun CSV, nemmeno vuoto |
| TEST-10 | `csv.rs:104` `csv_create_with_hw_json_writes_exact_header` | header `a,b` esatto e riga `1,2` |
| TEST-11 | `csv.rs:119` `csv_write_row_wrong_arity_fails` | righe con 1 e 3 campi rifiutate, nel file resta solo l'header |
| TEST-12 | `hist.rs:77` `hist_summary_1_to_100_percentiles` | percentili di 1..=100 µs negli intervalli attesi |
| aggiunto | `hist.rs:91` `hist_csv_fields_order_and_empty_recorder` | ordine di `csv_fields` e recorder vuoto |
| extra | `timer.rs:32` `timer_elapsed_is_monotonic_and_consistent` | monotonia e almeno 5000 µs dopo 5 ms |

Si lanciano dalla cartella `spikes/`: tutti con `cargo test -p common --locked` (12 passed), uno solo passando il nome come filtro, per esempio `cargo test -p common --locked env_required_error_never_contains_value`.

I controlli sul workspace (TEST-1, 2, 3, 4, 5, 13, 14) sono comandi e non file di test. Il bridge li ha rieseguiti: i crate diretti risultano un sottoinsieme dell'elenco approvato; non ci sono dipendenze senza pin né versioni locali nei crate; la build `--locked` passa; non esiste un `Cargo.toml` in radice; members è esatto; non c'è nessun `unwrap()` fuori dai test. In più `cargo clippy --workspace --all-targets --locked -- -D warnings` e `cargo fmt --all -- --check` passano puliti.

## Deviazioni e punti aperti

- **Ordine dei task.** Il workspace (Task 2, `67fe423`) è stato creato prima dei test rossi, perché senza `Cargo.toml` e lockfile i test di `common` non compilano. Il ciclo RED → GREEN è stato comunque rispettato.
- **cargo fmt sui test rossi.** Per far passare `cargo fmt --check` l'esecutore ha riformattato i test di `d2ea32e` (asserzioni spezzate su più righe, virgole finali). Il bridge ha confrontato i blocchi di test dei due commit ignorando spazi, a-capo e virgole: in manifest, csv e timer sono identici, in env e hist i test originali sono intatti e seguiti dai tre nuovi. Nessuna asserzione è stata tolta o indebolita.
- **Falso positivo dello scanner.** La regola di path traversal scambiava `dir.join(FILE_NAME)` per un join con input dell'utente. La costante è stata rinominata `MANIFEST_NAME` e resa privata (`manifest.rs:25`), senza silenziare lo scanner.
- **async-trait nel lockfile.** Il rapporto dell'esecutore lo dava per assente dal lock. In realtà c'è, alla stessa versione 0.1.92, come dipendenza transitiva di object_store (`cargo tree -i async-trait`). Nessun crate lo dichiara direttamente, quindi la decisione resta intatta.
- **Lockfile.** 401 pacchetti di registro più gli 8 membri del workspace. Due transitive (crypto-common, matchit) non sono all'ultima versione perché vincolate dai crate che le usano.
- **Punti aperti.** Il controllo a grep di TEST-2 non vede le chiavi con una cifra nel nome (sha2, flate2). Oggi sono pinnate (`Cargo.toml:28` e `:44`), ma una slice che aggiungesse in coda un crate con una cifra nel nome sfuggirebbe al controllo: conviene allargare la regex o passare a `cargo metadata`. Resta inoltre da decidere, al verify di fase, se PLAT-07 sia chiuso da questa fase o solo avviato.

**Modifica:** Claude (Anthropic) · 2026-10-19 · racconto didattico del piano 01-01, scritto dal diff reale e dai test rieseguiti
