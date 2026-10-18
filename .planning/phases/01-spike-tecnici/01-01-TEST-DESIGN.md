# Test design 01-01 — workspace degli spike e crate common

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-01-PLAN.md
  sha256: 1139356fff086314476375baf2d96a92cdd7286f8de6e55a79af742eb8bf330a
risks:
  - id: RISK-1
    severity: HIGH
    title: Crate non approvato o typosquat entra nel workspace e nel lockfile (build.rs eseguito sulla postazione)
    source: threat_model T-01-01-SC
  - id: RISK-2
    severity: HIGH
    title: Dipendenza non pinnata con =x.y.z o dichiarata con versione locale in un crate, deriva del lockfile fra slice
    source: must_have "dipendenze pinnate =x.y.z in [workspace.dependencies] e attive nei crate"
  - id: RISK-3
    severity: MEDIUM
    title: Codice usa-e-getta agganciato al prodotto (Cargo.toml in radice o riferimento a spikes nei manifest di prodotto)
    source: threat_model T-01-01-01
  - id: RISK-4
    severity: MEDIUM
    title: common::env espone il valore di una variabile (segreto) in un messaggio d'errore
    source: threat_model T-01-01-02
  - id: RISK-5
    severity: MEDIUM
    title: Un CSV di misura viene creato senza hw.json gemello o con hw.json malformato
    source: threat_model T-01-01-03
  - id: RISK-6
    severity: MEDIUM
    title: Writer CSV accetta righe con un numero di campi diverso dall'header (evidenze corrotte)
    source: must_have "API fissa ... CSV a header fisso"
  - id: RISK-7
    severity: MEDIUM
    title: Percentili dell'istogramma sbagliati falsano tutte le soglie p95/p99 degli spike
    source: must_have "API fissa ... istogrammi"
  - id: RISK-8
    severity: LOW
    title: members non esplicito o in ordine diverso; lockfile incoerente con i manifest
    source: must_have "workspace Cargo isolato con membri espliciti"
  - id: RISK-9
    severity: LOW
    title: unwrap() fuori dai test in common, panico al posto di errore contestualizzato
    source: must_have "API fissa" (Task 3 action)
tests:
  - {id: TEST-1, title: Elenco dei crate risolti contro l'elenco ratificato, type: check-supply-chain, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: Ogni voce di workspace.dependencies è pinnata con =, type: check-statico, priority: P0, catches: [RISK-2]}
  - {id: TEST-3, title: I crate dichiarano solo .workspace = true o path ../common, type: check-statico, priority: P0, catches: [RISK-2]}
  - {id: TEST-4, title: Lockfile coerente, cargo check --workspace --locked, type: build, priority: P0, catches: [RISK-1, RISK-2, RISK-8]}
  - {id: TEST-5, title: Nessun Cargo.toml in radice e nessun riferimento a spikes nei manifest di prodotto, type: check-statico, priority: P1, catches: [RISK-3]}
  - {id: TEST-6, title: env_required_unset_names_variable, type: cargo-unit, priority: P1, catches: [RISK-4]}
  - {id: TEST-7, title: env_required_error_never_contains_value, type: cargo-unit, priority: P1, catches: [RISK-4]}
  - {id: TEST-8, title: csv_create_without_hw_json_fails, type: cargo-unit, priority: P0, catches: [RISK-5]}
  - {id: TEST-9, title: manifest_ensure_present_rejects_malformed_json, type: cargo-unit, priority: P1, catches: [RISK-5]}
  - {id: TEST-10, title: csv_create_with_hw_json_writes_exact_header, type: cargo-unit, priority: P1, catches: [RISK-5, RISK-6]}
  - {id: TEST-11, title: csv_write_row_wrong_arity_fails, type: cargo-unit, priority: P1, catches: [RISK-6]}
  - {id: TEST-12, title: hist_summary_1_to_100_percentiles, type: cargo-unit, priority: P1, catches: [RISK-7]}
  - {id: TEST-13, title: members esatti e in ordine, type: check-statico, priority: P2, catches: [RISK-8]}
  - {id: TEST-14, title: Nessun unwrap fuori dai moduli di test di common, type: check-statico, priority: P2, catches: [RISK-9]}
uncovered: []
```

## Come leggere questo design

Il piano 01-01 crea l'harness Rust condiviso: il workspace isolato `spikes/` e il crate `common`, che tutti gli spike usano per scrivere le evidenze. Due cose contano più delle altre: che nel workspace entrino solo crate approvati dall'utente con versione esatta (RISK-1, RISK-2), e che `common` renda impossibile scrivere un CSV di misura senza il manifest hardware accanto (RISK-5). Gli altri rischi riguardano la correttezza dell'API di `common`, da cui dipende la validità di ogni numero degli spike successivi.

I test unitari stanno nei moduli `#[cfg(test)]` dei file di `common` elencati in `files_modified`. I controlli sul workspace sono comandi che si lanciano da `spikes/` e non richiedono file nuovi.

## Test

**TEST-1 — Elenco dei crate risolti contro l'elenco ratificato.** Dopo il Task 2, i nomi dei pacchetti diretti del workspace (`cargo metadata --format-version 1 --no-deps` letto con `node -e`, unione delle `dependencies` di ogni membro esclusi `common` e i membri stessi) devono essere un sottoinsieme dell'elenco approvato dall'utente nel Task 1 e riportato nel SUMMARY. Un nome in più fa fallire il controllo. Il controllo dei crate transitivi resta affidato al pin e al lockfile (TEST-4): l'utente ha ratificato i crate diretti, non l'albero intero. Comando singolo: `cd spikes && cargo metadata --format-version 1 --no-deps | node -e "<confronto con l'elenco del SUMMARY>"`.

**TEST-2 — Pin esatto.** Ogni riga di `[workspace.dependencies]` contiene `"=`. È il comando già scritto nell'acceptance del Task 2 (`sed … | grep -vc '"='` deve dare 0); va esteso alle voci in forma tabella (`{ version = "=…", features = … }`), che il `sed` attuale copre solo se stanno su una riga. Se l'esecutore usa tabelle multilinea `[workspace.dependencies.axum]`, il controllo deve leggerle con `cargo metadata` invece che con `grep`.

**TEST-3 — Nessuna versione locale nei crate.** `grep -rhE 'version *= *"[0-9]' spikes/*/Cargo.toml | grep -vc '0.0.0'` dà 0, come nell'acceptance; in più ogni riga sotto `[dependencies]` e `[dev-dependencies]` dei crate termina con `workspace = true` oppure è `common = { path = "../common" }`.

**TEST-4 — Lockfile coerente.** `cd spikes && cargo check --workspace --locked` esce 0. `--locked` fa fallire la build se `Cargo.lock` non corrisponde ai manifest, e quindi rivela un lockfile scritto a mano o rigenerato dopo un cambio non dichiarato. È anche la garanzia che le slice successive non abbiano bisogno di toccare il lockfile.

**TEST-5 — Isolamento dal prodotto.** `test ! -f Cargo.toml` alla radice, `git ls-files | grep -c '^Cargo.toml$'` = 0 e `grep -r "spikes" --include=Cargo.toml services crates apps` vuoto. Sono i controlli dell'acceptance; vanno rieseguiti alla fine del piano, non solo dopo il Task 2.

**TEST-6 — `env_required_unset_names_variable`** (in `spikes/common/src/env.rs`). Chiama `required("CKA_SPIKE_TEST_UNSET_VAR")` e verifica che sia `Err` e che il messaggio (formattato con `{:#}` per includere la catena anyhow) contenga il nome della variabile. Singolo: `cd spikes && cargo test -p common env_required_unset_names_variable`.

**TEST-7 — `env_required_error_never_contains_value`** (in `env.rs`). Il threat model T-01-01-02 promette «un test dedicato» che il valore non compaia mai, ma il blocco `<behavior>` del Task 3 prova solo la presenza del nome. Questo test copre la parte mancante: se `required` rifiuta anche le variabili vuote o solo spazi (comportamento ragionevole per una chiave API), il test imposta una variabile con nome univoco a un valore riconoscibile composto di soli spazi più un marcatore impossibile da confondere (per esempio un valore che `required` rifiuta per un'altra regola documentata) e verifica che il messaggio d'errore non contenga il valore. Se `required` accetta qualunque valore non vuoto, il test si riduce a verificare che il tipo d'errore non abbia un campo che trasporta il valore e che `format!("{:?}", err)` non contenga il contenuto di una variabile impostata in precedenza nello stesso test. In edition 2024 `std::env::set_var` è `unsafe`: il test usa un nome di variabile univoco per evitare interferenze con i test paralleli e lo dichiara in un commento `// SAFETY:`.

**TEST-8 — `csv_create_without_hw_json_fails`** (in `spikes/common/src/csv.rs`). Cartella temporanea univoca sotto `std::env::temp_dir()` senza `hw.json`; `Writer::create(dir/"x.csv", &["a","b"])` è `Err` e il file `x.csv` non esiste dopo la chiamata (nessun file parziale). È il test che rende vera la mitigazione di T-01-01-03.

**TEST-9 — `manifest_ensure_present_rejects_malformed_json`** (in `manifest.rs`). `hw.json` con contenuto `{not json`; `ensure_present(dir)` è `Err` e il messaggio nomina il percorso del file. Una seconda asserzione con un `hw.json` valido ma privo di un campo obbligatorio (`timestamp_utc`) verifica che anche questo sia `Err`, così il manifest non può essere un `{}` vuoto.

**TEST-10 — `csv_create_with_hw_json_writes_exact_header`** (in `csv.rs`). Con un `hw.json` minimo valido nella cartella, `create` riesce, una `write_row` con due campi riesce, e il file letto da disco ha come prima riga esattamente `a,b`.

**TEST-11 — `csv_write_row_wrong_arity_fails`** (in `csv.rs`). Header di due campi; `write_row(&["1"])` e `write_row(&["1","2","3"])` sono entrambi `Err` e il file non contiene la riga.

**TEST-12 — `hist_summary_1_to_100_percentiles`** (in `hist.rs`). Registra i valori 1..=100 µs; `summary()` dà n = 100, p50 in [50, 51], p95 in [95, 96], p99 in [99, 100], max = 100. Con 3 cifre significative di `hdrhistogram` questi intervalli sono esatti; un errore di unità (ns contro µs) li fa uscire di tre ordini di grandezza.

**TEST-13 — Membri espliciti.** Il `grep -c '^members = \["common", … "s07-ports"\]'` del verify del Task 2 dà 1. Nessun glob nei membri.

**TEST-14 — Niente `unwrap()` fuori dai test.** Per ogni file `spikes/common/src/*.rs`, nessuna occorrenza di `unwrap()` o `expect(` prima della riga `#[cfg(test)]`: `awk '/#\[cfg\(test\)\]/{exit} /unwrap\(\)|expect\(/{print FILENAME": "FNR}' spikes/common/src/*.rs` (un file alla volta) non stampa nulla. È la versione meccanica della «verifica a vista» dell'acceptance.

## Copertura

I due rischi HIGH sono coperti: RISK-1 da TEST-1 e TEST-4 (più il checkpoint umano del Task 1, che è la mitigazione primaria e resta fuori dai test automatici), RISK-2 da TEST-2, TEST-3 e TEST-4. `cargo test -p common` deve contare almeno 7 test (TEST-6..TEST-12), sopra il minimo di 5 fissato dal piano.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-01

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 51572d6fbb5a… a 1139356fff08… per la revisione del piano: test di T-01-01-02 con valore sentinella che non deve comparire nell'errore; nomi di variabile univoci per test perché in edizione 2024 set_var è unsafe. Il design non è stato rigenerato: va riletto contro la revisione.
