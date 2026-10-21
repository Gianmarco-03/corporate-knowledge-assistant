# S-04 — Forma della chiamata `rank` a JEV, concorrenza, pin e alias · RISULTATI

**Modifica:** Claude (gsd-executor 01-11) · 2026-10-21 · domanda e soglie, scritte prima di qualsiasi misura

**Modifica:** Claude (gsd-executor 01-11) · 2026-10-21 · setup, risultati, anomalie ed esito dopo le misure (le Soglie sotto sono invariate)

## Domanda e decisione da chiudere

Spike S-04, parte A-09 (piano 01-11). Le decisioni di 01-CONTEXT coinvolte sono D-17 (pin di una versione JEV esplicita, mai un alias mobile) e D-22 (forma della chiamata, concorrenza e retry misurati prima di fissare l'adapter).

La domanda ha quattro parti:

1. **Forma di `rank`.** Conviene una richiesta per candidato con domande Noul (forma A), uno `state` unico con N domande Noul (forma B) o una sola domanda Choice (forma C)? Si confrontano nDCG@5, recall@8, latenza, token e varianza su 3 ripetizioni.
2. **Concorrenza.** Quale livello fra 4, 8 e 16 richieste in volo regge senza 429, e come si comporta il client con `retry-after`?
3. **Alias.** A quale versione risolvono `jev-latest`, `jev-1.13.0` e `jev-preview` secondo `response.model`?
4. **Pin.** Il pin `jev-1.13.0` letto da `JEV_MODEL` risolve a sé stesso, con la versione risolta registrata in ogni riga?

La decisione va in ADR-0008. La riga «Modello testuale» di `DEPENDENCIES.md` (`jev-latest` → `jev-1.13.0`) la scrive il consolidamento (01-17).

## Soglie (scritte PRIMA della misura)

| Metrica | Soglia | Fonte soglia |
|---|---|---|
| Scelta della forma | La forma A resta il default se nDCG@5(A) ≥ nDCG@5 della migliore fra B e C **e** la latenza p95 del rank di 20 candidati per query è ≤ 2.000 ms alla concorrenza scelta. Altrimenti vince la forma con nDCG@5 più alto entro lo stesso tetto di latenza | RICERCA Area D §7 riga S1; piano 01-11 Task 1 |
| Latenza p95 del rank (20 candidati, una query) | ≤ 2.000 ms alla concorrenza scelta | piano 01-11 Task 1 (Claude's Discretion) |
| Concorrenza scelta | Il livello più alto fra 4, 8 e 16 con **0 risposte 429 su 3 run** | RICERCA Area D §7 riga S2 |
| Varianza del segnale | Deviazione standard del Noul `relevant` sullo stesso candidato fra 3 ripetizioni ≤ 0,05. Sopra soglia, nell'Esito la varianza diventa un vincolo per la calibrazione delle soglie | piano 01-11 Task 1; RICERCA Area D §7 riga S1 |
| Pin | `jev-1.13.0` risolve a sé stesso in `response.model` | D-17; RICERCA Area D §1.4 |
| Passaggi non visibili al ruolo inviati | **0** (hard gate): `unauthorized_sent` = 0 su ogni riga | D-16; threat model T-14 |
| Spesa della sessione | ≤ `S04_MAX_COST_USD` | piano 01-11 Task 1; T-01-11-03 |

Regole di lettura fissate insieme alle soglie:

- Gli aggregati usano le ripetizioni 1..3. La prima richiesta di ciascuna forma è un warm-up (`rep` = 0): resta nel CSV ed è esclusa dagli aggregati.
- La deviazione standard è quella campionaria (n − 1) del segnale per candidato fra le 3 ripetizioni: Noul `relevant` per A e B, probabilità della Choice per C. La soglia di 0,05 si legge sulla distribuzione per candidato (media, p95, massimo). Nel CSV, `noul_relevant_std` riporta il massimo per query.
- I percentili sono nearest-rank, calcolati su tutte le righe riuscite.

## Setup

- **Hardware:** `results/jev/hw.json` (sha256 `5fe5ec065046e31675e3e0b28278c3a732cdf160071d96f601ae794459d908dd`, sessione `01-11-20261021t152512z-828a`, `git_rev` 28d8ea7, `tools.jev_model` = `jev-1.13.0`). i7-13700K con 16 core e 24 thread, Docker Desktop 29.8.2 su WSL 2 (16 CPU alla VM), piano energetico «Bilanciato». Il manifest è normalizzato allo schema di `common::manifest` con la stessa trasformazione di 01-04 e 01-05 (vedi «Anomalie e deviazioni»); l'originale sta in `results/raw/jev/`.
- **Immagine:** `cka-spikes-runner:latest` (`sha256:86a924ad5599d1f010ca454e8c910704c662df46b1557b5343950e14ab91d298`), rustc 1.95.0. La build è `--release --locked --offline` con le crate vendorizzate dall'host in `spikes/target/vendor-s04` (cartella ignorata), come in 01-04 e 01-05.
- **Client:** reqwest 0.13.5 con Rustls e la verifica dei certificati di default, `https_only` verso host non loopback, `connect_timeout` 2 s, timeout per richiesta 30 s. Modello da `JEV_MODEL` (`jev-1.13.0`), alias solo nel sottocomando `aliases`.
- **Retry:** su 429 e 529, uno solo per chiamata logica e solo se l'attesa (`retry-after`, altrimenti 0,5 s) finisce prima della deadline. La prima sessione girava con un tetto di 2 retry, portato a 1 (ADR-0010) nel commit `acbac93` prima della seconda; in nessuna delle due sessioni c'è stato un 429, un 529 o un retry, quindi i dati non cambiano.
- **Deadline:** 10 s per rank (forme A, B, C e concorrenza). Il tetto serve a non troncare la misura: la soglia di latenza (2.000 ms) si legge sui percentili.
- **Corpus:** solo testo degli oracoli ratificati (`spikes/data/corpus/oracles/queries.json`, filtro dei tre assi verdi: 74 oracoli, 74 excerpt distinti), tutti da documenti pubblici o sintetici del corpus di 01-03. Nessuna estrazione nuova.
- **Insiemi di candidati** (`build_candidate_sets`, seme `0x01115EED`):
  - query ammesse: oracoli verdi con risposta, almeno un `expected_passage` e almeno 20 excerpt visibili al loro `role_profile` (resta esclusa l'unica query `restricted-only`, che vede 4 excerpt);
  - 30 query a turno fra i gruppi di lingua: 10 `it`, 10 `en`, 10 cross (7 `cross-it-en`, 3 `cross-en-it`); ruoli: 17 guest, 7 manager, 5 employee, 1 admin;
  - ogni insieme ha 20 passaggi: gli `expected_passages` della query (rilevanti), fino a 2 excerpt `injection_case` visibili, poi distrattori visibili scelti dal seme; l'ordine finale è mescolato;
  - `assert_visible` gira sull'insieme in costruzione e di nuovo nel client, sull'intero insieme, prima del primo invio.
- **Wording:** `questions/form-a.json` (sha256 `e9b6a24b…`), `form-b.json` (`a455735c…`), `form-c.json` (`edcfbcf1…`); lo sha completo è in ogni riga di `forms.csv`.
- **Budget:** `S04_MAX_COST_USD` = 5 USD (uguale al default pubblico di `spikes/.env.example`; il valore del file segreti è stato confrontato per uguaglianza, senza stamparlo). Il registro `results/raw/jev/spend.json` porta la spesa da un sottocomando all'altro, così il tetto vale per l'intera sessione.
- **Stima prima di partire** (`s04-jev estimate`, 300 token fissi per richiesta più 1 token ogni 3 caratteri): 3.517.535 token di input, 0,1477 USD, entro il budget. Spesa reale da `usage`: 0,077955 USD.
- **Sessioni** (sempre `session.sh with 01-11`, solo il servizio `runner`, `ps --assert-only runner` verde e nessun container di altri piani):
  - `01-11-20261021t152512z-828a`: `hw.json`, build, `estimate`, `aliases`, `concurrency` (binario a 28d8ea7);
  - `01-11-20261021t153037z-f0ff`: `forms` A, B, C e `collect` (binario a `acbac93`).
- **Comandi**, da `spikes/` nel runner via `session.sh oneshot` (`B=/target/release/s04-jev`):
  - `$B estimate`;
  - `$B aliases --names jev-latest,jev-1.13.0,jev-preview`;
  - `$B concurrency --levels 4,8,16 --runs 3 --queries 10` (per run: 10 query × 20 candidati = 200 richieste di forma A, una query alla volta con N richieste in volo);
  - `$B forms --form a --reps 3 --concurrency 16`, `$B forms --form b --reps 3`, `$B forms --form c --reps 3` (30 query × 3 ripetizioni, più un warm-up `rep` = 0 sulla prima query);
  - `$B collect`: dai grezzi `results/raw/jev/*.jsonl` scrive i tre CSV e stampa le tabelle qui sotto.

## Risultati

### Forme (`results/jev/forms.csv`, ripetizioni 1..3, 90 rank per forma, warm-up escluso)

| Forma | rank riusciti | nDCG@5 | recall@8 | latenza del rank p50 / p95 / p99 (ms) | token di input per query | std per candidato: media / p95 / max | candidati sopra 0,05 | Esito vs soglia |
|---|---|---|---|---|---|---|---|---|
| A (20 richieste, concorrenza 16) | 90 / 90 | **1,0000** | **1,0000** | 498 / **606** / 653 | 8.449 | 0,0017 / 0,0058 / 0,0954 | 1 su 600 | qualità ≥ B e C; p95 ≤ 2.000 ms; varianza: 1 candidato oltre soglia |
| B (1 richiesta, 20 Noul) | 90 / 90 | 0,6790 | 0,9500 | 229 / 277 / 315 | 2.209 | 0,0130 / 0,0503 / 0,3323 | 32 su 600 | qualità sotto A; varianza oltre soglia al p95 |
| C (1 richiesta, 1 Choice) | 90 / 90 | 0,7286 | 0,9639 | 187 / 288 / 478 | 1.364 | 0,0042 / 0,0208 / 0,1852 | 11 su 600 | qualità sotto A |

- nDCG@5 per gruppo di lingua (30 rank ciascuno): A 1,0000 / 1,0000 / 1,0000 (it / en / cross); B 0,5606 / 0,7944 / 0,6819; C 0,6965 / 0,8368 / 0,6525.
- Forma A: in ogni rank tutti i rilevanti superano tutti i distrattori. Il margine minimo fra il rilevante più basso e il distrattore più alto è 0,07 (Q-014, rep 1: 0,12 contro 0,05); i segnali assumono 66 valori distinti fra 0 e 0,99.
- Il candidato oltre soglia nella forma A è `pub-it-04-bdi-bollettino-economico-2026-3#74024e7e9365d9db` nell'insieme di Q-066: `relevant` = 0,31 / 0,30 / 0,47 nelle tre ripetizioni (std 0,0954). È un distrattore e resta sotto il rilevante della query in ogni ripetizione.
- Token: 760.407 di input e 108.000 di output per la forma A, 198.816 e 31.860 per B, 122.766 e 16.968 per C (ripetizioni 1..3). Con il prezzo pubblicato (0,042 USD/Mtok di input, output gratuito) un rank di forma A costa circa 0,00035 USD per query.
- Warm-up (`rep` = 0, Q-027): A 800 ms, B 293 ms, C 330 ms.
- Nessun 429, nessun 529, nessun retry, nessun errore in 270 rank + 3 warm-up.
- `instructs_model` ≥ 0,5 (forma A, ripetizioni 1..3): 0 su 258 invii di excerpt `injection_case` e 0 su 1.542 invii degli altri excerpt; il valore massimo osservato è 0,20 (`syn-05-risk-report-confidential-en`). Vedi «Anomalie e deviazioni» per il significato di questo numero.
- `unauthorized_sent` = 0 su tutte le 273 righe (`awk -F, 'NR>1 && $15!=0'` non stampa nulla).
- `model_resolved` = `jev-1.13.0` su tutte le 273 righe.

### Concorrenza (`results/jev/concurrency.csv`, 200 richieste di forma A per run)

| Livello | run | ok | 429 | 529 | altri errori | p50 / p95 / p99 per richiesta (ms) | throughput (rps) | retry |
|---|---|---|---|---|---|---|---|---|
| 4 | 1 | 200 | 0 | 0 | 0 | 219 / 286 / 362 | 13,12 | 0 |
| 4 | 2 | 200 | 0 | 0 | 0 | 197 / 272 / 310 | 15,09 | 0 |
| 4 | 3 | 200 | 0 | 0 | 0 | 219 / 269 / 293 | 14,00 | 0 |
| 8 | 1 | 200 | 0 | 0 | 0 | 208 / 269 / 341 | 22,33 | 0 |
| 8 | 2 | 200 | 0 | 0 | 0 | 216 / 272 / 308 | 22,63 | 0 |
| 8 | 3 | 200 | 0 | 0 | 0 | 215 / 266 / 305 | 21,50 | 0 |
| 16 | 1 | 200 | 0 | 0 | 0 | 204 / 280 / 325 | 29,32 | 0 |
| 16 | 2 | 200 | 0 | 0 | 0 | 231 / 323 / 397 | 29,84 | 0 |
| 16 | 3 | 200 | 0 | 0 | 0 | 219 / 286 / 300 | 19,53 | 0 |

- 0 risposte 429 su 3 run a ogni livello: secondo la soglia, la concorrenza scelta è **16**.
- La latenza per richiesta non cresce con il livello (p95 fra 266 e 323 ms). Il throughput cresce da circa 14 a circa 29 rps fra 4 e 16; il run 3 a 16 (19,53 rps) ha la stessa latenza per richiesta, quindi la differenza sta nel tempo fra una query e l'altra, non in JEV.
- Spesa cumulata a fine concorrenza: 0,031993 USD (con gli alias).
- `model_resolved` = `jev-1.13.0` su tutte le 1.800 richieste.

### Alias (`results/jev/aliases.csv`)

| Alias richiesto | `response.model` | status | latenza (ms) |
|---|---|---|---|
| `jev-latest` | `jev-1.13.0` | 200 | 390 |
| `jev-1.13.0` | `jev-1.13.0` | 200 | 217 |
| `jev-preview` | `jev-1.14.0` | 200 | 222 |

- Il pin risolve a sé stesso, come richiesto dalla soglia.
- `jev-latest` oggi coincide con il pin, ma è un alias mobile; `jev-preview` punta già a una versione diversa (1.14.0). Coincide con quanto riportato dalla documentazione il 2026-10-18 (RICERCA Area D §1.4). L'incoerenza segnalata lì (l'esempio OpenAPI che restituisce `"jev-latest"`) non si presenta: la risposta riporta sempre la versione.

## Anomalie e deviazioni

- **Firma del costruttore.** `Client::new(ClientConfig, Acl)` e `Client::new_from_env(Acl)` ricevono l'`Acl`, mentre `<interfaces>` del piano scrive `new_from_env` senza argomenti. La guardia di visibilità deve stare dentro il client, e `rank_form_a(query, passages, concurrency, deadline)` ha il ruolo nella `Query`, non l'ACL. Il contratto di fase (`01-CONTRATTO.md`) non fissa la firma del costruttore, e `rank_form_a` resta quella promessa. Per 01-15: `let acl = Acl::load(corpus)?; let client = Client::new_from_env(acl)?;`.
- **`CallMeta` più ricco del contratto.** Oltre a `model_resolved`, `input_tokens`, `status`, `latency_ms` porta `output_tokens`, `retry_after`, `retries`, `throttled_429`, `throttled_529` e `request_id`. L'aggiunta è compatibile con il consumo di 01-15. C'è anche `rank_form_a_each`, che restituisce l'esito di ogni candidato anche in errore: lo usano le misure di concorrenza.
- **`instructs_model` non misurabile sul testo di injection.** Gli oracoli `injection_case` (Q-019, Q-020, Q-055, Q-056, Q-058, Q-070) hanno come `expected_passages` i fatti legittimi dei documenti `syn-07-injection-public` e `syn-08-injection-internal-en`, non le istruzioni iniettate. Con la regola del piano (solo testo degli oracoli ratificati, nessuna estrazione nuova) il testo ostile non arriva mai a JEV. Il numero misurato è quindi un tasso di falsi positivi: 0 su 1.800 invii, con il massimo a 0,20. Il tasso di rilevamento sulle istruzioni vere resta aperto: lo può misurare 01-12 (tenuta alla prompt injection indiretta) oppure un oracolo che ratifichi gli excerpt ostili.
- **`request_id` vuoto in `aliases.csv`.** Le risposte di JEV non portano `x-request-id` né `request-id`, i due header che il client legge. La colonna resta vuota, e l'identificazione della richiesta passa da orario e `model_resolved`.
- **`hw.json` normalizzato.** `tools/hw-manifest.sh` (01-02) scrive `cpu`, `ram_total`, `docker_desktop_resources` e `images` in forme che `common::manifest` (01-01) rifiuta. È il difetto già noto fra 01-01 e 01-02: ho applicato la stessa trasformazione di `s01-upload/bench.sh` (01-04) con un driver fuori dal repo, e l'originale è in `results/raw/jev/`. Nessun file chiuso è stato toccato.
- **Grezzi spostati in `results/raw/jev/`.** Durante le sessioni i grezzi (`*.jsonl`, `spend.json`, manifest originale) stavano in `results/jev/raw/`, che il pattern `**/results/raw/` di `spikes/.gitignore` (01-02, chiuso) non copre. Dopo le misure li ho spostati in `results/raw/jev/`, che è ignorata, e il CLI ora usa quel percorso. `collect` rieseguito sull'host dai grezzi spostati produce CSV identici byte per byte a quelli del runner. Il campo `normalized_by` di `hw.json` cita ancora il percorso vecchio: il manifest non è stato riscritto dopo la misura.
- **Un solo manifest per due sessioni.** `hw.json` viene dalla prima sessione (`git_rev` 28d8ea7). La seconda è partita subito dopo, sulla stessa macchina e con la stessa immagine, con il binario ricompilato a `acbac93`. Fra i due commit cambia solo il tetto dei retry, mai esercitato.
- **Ordine delle misure.** Il piano elenca forme, concorrenza e alias. Le ho eseguite in ordine inverso (alias, concorrenza, forme), perché la soglia di latenza della forma A si legge «alla concorrenza scelta»: serviva conoscerla prima.
- **Concorrenza misurata per query.** Un run è una sequenza di 10 rank da 20 candidati con N richieste in volo, cioè la forma che il prodotto userà. A livello 16 una query ha 20 richieste: le ultime 4 partono quando si libera un posto. Un carico che tenga sempre 16 richieste in volo fra query diverse non è stato misurato.
- **Varianza di un candidato oltre soglia nella forma A** (1 su 600, std 0,0954): si applica la regola delle Soglie, e la varianza diventa un vincolo di calibrazione (vedi «Esito»).

## Esito

- **Decisione proposta: forma A.** Una richiesta per candidato, `state = {query, passage}`, con le domande Noul `relevant`, `sufficient` e `instructs_model`, a **concorrenza 16**.
  - Qualità: nDCG@5 1,0000 e recall@8 1,0000, contro 0,7286 / 0,9639 di C e 0,6790 / 0,9500 di B. La regola della soglia (nDCG@5(A) ≥ migliore fra B e C) è soddisfatta con margine.
  - Latenza: p95 del rank di 20 candidati 606 ms a concorrenza 16, contro un tetto di 2.000 ms.
  - Costo: circa 3,8 volte i token di B e 6,2 volte quelli di C (8.449 contro 2.209 e 1.364 per query). In valore assoluto è circa 0,00035 USD per query: il costo non sposta la scelta.
- **Alternative scartate:**
  - **B** perde 0,32 di nDCG@5 e ha la varianza più alta (p95 0,0503, oltre la soglia; 32 candidati su 600 sopra 0,05). È coerente con il failure mode documentato da typesafe, «Large state full of irrelevant detail» (RICERCA Area D §1.2): ogni domanda vede tutti e 20 i passaggi;
  - **C** perde 0,27 di nDCG@5. In più le probabilità della Choice sommano a 1, quindi danno un ordine relativo e non una rilevanza assoluta: non servono all'evidence gate (RICERCA Area D §1.7).
- **Concorrenza: 16**, il livello più alto misurato, con 0 risposte 429 e 0 risposte 529 su 3 run. Il rate limit dell'organizzazione non è stato raggiunto; il valore vero resta non pubblicato (RICERCA Area D §1.2). Il baseline di 8 di `interfaces-and-concurrency.md` è prudente; 16 è il massimo verificato, non un limite trovato.
- **Politica di retry:** retry solo su 429 e 529, al massimo uno per chiamata, con `retry-after` rispettato (o 0,5 s se manca) e solo se l'attesa finisce prima della deadline globale. Oltre la deadline: errore tipizzato `RateLimited` o `Timeout`, mai un segnale. È il budget di ADR-0010 (Decisione 5), applicato a JEV.
- **Varianza come vincolo di calibrazione.** Forma A: p95 0,0058, ma un candidato a 0,0954. Le soglie di `relevant` (e degli altri Noul) vanno calibrate tenendo un margine di almeno ±0,1 attorno al valore di taglio: un segnale vicino alla soglia può cambiare lato da una chiamata all'altra. Il candidato instabile in questa misura stava fra 0,30 e 0,47, quindi una soglia intorno a 0,4 lo farebbe oscillare.
- **Pin `jev-1.13.0` confermato:** `response.model` = `jev-1.13.0` su tutte le 3.803 risposte del pin (1.800 di concorrenza; 1.820 della forma A, 91 della B e 91 della C, warm-up compresi; 1 alias). La versione risolta è registrata in ogni riga. Chiude la parte tecnica di Q-07.
- **Impatto sulle porte (S-07, ADR-0012):**
  - `Reranker::rank(ctx, query, AuthorizedEvidence)`, il trait stretto, con deadline esplicita nel contesto. L'adapter JEV fa fan-out per candidato con limite 16 e `timeout_at` globale (ADR-0010, variante A);
  - la porta restituisce solo segnali (`RankScore` per passaggio). La versione risolta di JEV e l'`usage` vanno nei metadati della chiamata;
  - errori tipizzati: `RateLimited` (429/529 non risolti entro la deadline), `Timeout`, `BudgetExceeded`, `Http`, `Unauthorized` (guardia); nessun errore diventa un segnale;
  - la stessa chiamata di forma A produce anche `sufficient` e `instructs_model`. `DecisionEngine::assess_evidence` può riusarli senza un'altra richiesta, ma resta un metodo separato (ISP, ADR-0012).
- **Impatto sul threat model:**
  - **T-14** (chunk non autorizzati verso un provider): guardia deterministica sull'intero insieme prima del primo invio, più un riconteggio indipendente sul punto di invio. Evidenze: `unauthorized_sent` = 0 su ogni riga di `results/jev/forms.csv`, e i test `send_path_blocks_before_network_on_invisible_passage` e `candidate_sets_only_visible_to_role` in `src/`;
  - **T-16** (prompt injection indiretta): i passaggi viaggiano come `state` (dati), mai come istruzioni; `instructs_model` ha 0 falsi positivi su 1.800 invii di forma A (ripetizioni 1..3) (`results/jev/forms.csv`, colonna `instructs_model_flagged`). Il rilevamento sul testo ostile vero resta aperto (vedi «Anomalie e deviazioni»);
  - **T-24** (consumo di risorse e costo): 0 risposte 429 fino a 16 richieste in volo (`results/jev/concurrency.csv`); tetto di spesa per sessione con registro condiviso; spesa reale 0,078 USD su 5.
- **Destinazione:**
  - ADR-0008 (`docs/adr/0008-jev-forma-rank-concorrenza-pin.md`), Stato `Proposed`;
  - riga «Modello testuale» di `DEPENDENCIES.md`: `jev-latest` → `jev-1.13.0` (pin), Proposta → Decisa. La scrive il consolidamento (01-17, D-17).
