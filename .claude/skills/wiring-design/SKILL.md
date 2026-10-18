---
name: wiring-design
description: >
  Ingegnerizza, per ogni wiring point trovato da wiring-search, la convenzione
  additiva che permette alle slice di una fase di Corporate Knowledge Assistant, e
  alle fasi successive, di contribuire allo stesso seam senza editare le stesse
  righe. Default: registri espliciti con append e innesti per-slice pre-registrati
  in wave 0; mai auto-discovery né registrazione magica; mai match sul tipo, ma
  trait + registro. Regola di serializzazione per la rigenerazione del client TS
  da OpenAPI, numerazione delle migrazioni sqlx per servizio, decisione su quali
  seam inter-fase promettere. Aggiorna `.planning/WIRING-MAP.md` e scrive §3-§5 di
  `0X-WIRING.md` (con §4, i vincoli per gli esecutori). USARE come secondo passo
  del wiring dentro `plan`, dopo wiring-search e prima dei piani; o quando
  l'utente chiede "progetta i wiring point", "come rendo additivo questo seam",
  "convenzione di wiring", "aggiorna la WIRING-MAP".
---

# wiring-design — ingegnerizzare i wiring point

Secondo passo della triade di wiring, dentro `plan` al passo 3, prima dei piani. Input: §1 e §2 di `0X-WIRING.md` scritti da `wiring-search`, la WIRING-MAP, CONTEXT e CONTRATTO X-1. Scopo: per ogni punto scegliere il meccanismo che trasforma «edita le stesse righe» in «aggiungi il tuo pezzo isolato», decidere quali seam inter-fase diventano promesse di progetto, e scrivere i vincoli che piani ed esecutori dovranno rispettare. Non scrive codice: la creazione è di `wiring-build`, in wave 0 di `execute`.

## Principio guida

Un wiring point è sicuro per il parallelo quando il contributo di ogni slice è **isolato**: un file nuovo di sua proprietà, oppure una voce in un registro esplicito che non tocca le righe delle altre.

**Default di progetto: registro esplicito.** I registri stanno in file noti e il wiring si legge in un unico posto. Ordine di preferenza:
1. **Innesto pre-registrato in wave 0**: `wiring-build` scrive una volta la riga di registrazione e crea il file per-slice vuoto (es. `pub fn router() -> Router { Router::new() }`); la slice riempie solo il proprio file. È l'unica forma che lascia le slice davvero parallele, perché `onde-slice` serializza qualunque coppia di piani con un file in comune.
2. **Append in coda a un registro esplicito**, fatto dalla slice: rischio di merge MEDIO accettato, ma i piani che appendono allo stesso file finiscono in onde diverse. Si usa quando le slice sono già in serie per dipendenza.
3. **Serializzazione esplicita** dove l'append non basta: artefatti rigenerati.

**Divieti** (salvo richiesta esplicita dell'utente per un punto preciso, registrata nella WIRING-MAP):
- **niente auto-discovery né registrazione magica**: niente crate `inventory`/`linkme`, costruttori statici, scansione di cartelle in `build.rs`, macro che raccolgono implementazioni, `import.meta.glob` per montare route o handler. La leggibilità del wiring vale più di un rischio di merge MEDIO;
- **niente match sul tipo**: una nuova variante non aggiunge un braccio a un `match` su enum o stringa (tipo di media, fonte, provider). Si usa un **trait-porta** più un **registro** chiave → `Arc<dyn Trait>` popolato con registrazioni esplicite nel composition root. Una variante nuova = un file nuovo + una riga di registrazione, mai un nuovo braccio. Il `match` resta legittimo su enum chiusi di dominio che non crescono per estensione (stati di una macchina a stati, esiti di una decisione).

## Meccanismo per kind (Rust / Axum / React)

- **workspace-member** (`create-once`) — `members` esplicito nel `Cargo.toml` radice, niente glob `services/*`: un crate nuovo è un evento visibile. Con la pre-registrazione di progetto (sezione sotto) li aggiunge TUTTI la **Fondazione**: ogni crate pianificato è membro fin dall'inizio (anche come crate vuoto), e `[workspace.dependencies]` dichiara le dipendenze di tutti i crate pianificati. `members`, `[package]` e il resto dei `Cargo.toml` restano della fase che li crea (di norma la Fondazione); le sole sezioni `[dependencies]` / `[workspace.dependencies]` sono **append-only** per tutte le fasi (whitelist di `proprieta-file`, ROADMAP «Strategia di consegna»): una dipendenza scoperta più tardi si appende in coda nella fase che la usa, mai riscrivendo righe esistenti. Stessa regola per `dependencies` di `apps/web/package.json`. **`Cargo.lock` e `apps/web/package-lock.json` sono `regenerate`**: in conflitto non si fondono mai a mano, si prende la versione di `main` e si rigenera (`cargo check --workspace`, `npm install` in `apps/web`).
- **Composition root per servizio** (`create-once` + `register`) — `src/main.rs` resta minimo (configurazione, telemetria, `app::build(config)`, bind). Il modulo `app` (`src/app.rs` + `src/app/<feature>.rs`) è l'unico punto che costruisce adapter concreti e li inietta dietro i trait; `domain` non vede `axum`, `sqlx`, `reqwest` né SDK dei provider. Ogni feature ha la propria funzione di composizione in un file proprio (`app/<feature>.rs` → `pub fn compose(infra: &Infra) -> Router`), che costruisce i propri adapter e chiude il proprio stato con `Router::with_state`, così le slice non contendono un unico `AppState`. `app.rs` contiene una riga `.merge(<feature>::compose(&infra))` per feature, pre-registrata dalla **Fondazione** per le feature di tutte le fasi pianificate: `main.rs` e `app.rs` sono file chiusi della Fondazione, `app/<feature>.rs` è file della sola fase che lo riempie.
- **router-mount** (`create-once` + innesto) — un mount point per area di API (`.nest("/api/v1/documents", …)`), scritto una volta. Le rotte di una feature vivono nel suo modulo HTTP; la slice non edita `app.rs`.
- **registry** (`create-once` + `register`) — per le varianti di un comportamento: tipo `ParserRegistry`, `SourceRegistry`, … nel livello applicativo del servizio (puro, senza dipendenze infrastrutturali), con `register(key, Arc<dyn Trait>)` e `get(&key) -> Option<…>`; chiave assente → errore di dominio esplicito (`UnsupportedMediaType`), mai un ramo di default silenzioso. Le registrazioni sono righe esplicite nel composition root. Un test verifica che le chiavi dichiarate supportate abbiano tutte un adapter registrato.
- **trait-port** (`create-once`) — il trait vive nel servizio che lo consuma (o in `crates/contracts` se attraversa servizi), con firme che non espongono tipi di `sqlx`, `reqwest` o SDK. Fake deterministici e adapter reali sono file distinti.
- **migration-sequence** (`append`, serializzato per servizio) — una cartella per servizio, `services/<svc>/migrations/`, e nessun servizio migra lo schema altrui (`DEPENDENCIES.md` §Persistenza). Nome: `<VERSION>_<descrizione>.sql` con `VERSION = <fase su tre cifre><piano su due><progressivo su due>` (fase 06 piano 02 → `0600201_create_documents.sql`; fase 05.1 → `051…`). Il numero è noto già al piano, quindi `files_modified` lo dichiara senza glob: la cartella la crea la Fondazione, ogni file di migrazione è della fase che lo scrive (mai la cartella intera in `files_modified`, che `proprieta-file` leggerebbe come reclamo sull'intera sequenza). **Monotonia**: una migrazione nuova ha sempre versione maggiore di tutte quelle già presenti nel servizio; se la numerazione di una fase decimale inserita in seguito finirebbe sotto il massimo esistente, si usa il primo numero libero sopra il massimo e lo si annota nella WIRING-MAP. Mai rinumerare o modificare una migrazione già fusa: si scrive una migrazione nuova. **Vincolo**: un solo piano per servizio e per onda aggiunge migrazioni. Se il progetto adotta la cache offline `.sqlx/`, è `regenerate` e segue la stessa regola del client TS.
- **openapi-tag** (`shared-artifact`) — una spec per servizio (contract-first o generata da Rust: decisione aperta in `DEPENDENCIES.md`; la convenzione vale in entrambi i casi). Ogni feature possiede un tag e i propri path; il tag si dichiara una volta. Con gli insiemi di file chiusi una spec unica scritta a mano da più fasi viola `proprieta-file`: in contract-first ogni fase possiede il proprio frammento (`openapi/<svc>/<feature>.yaml`) e la Fondazione pre-registra l'elenco dei frammenti da assemblare; con la spec generata da Rust, i path vivono nei moduli HTTP di ciascuna fase e la spec generata è `regenerate` (va in whitelist quando la decisione è presa). Le rotte `/internal/v1/*` stanno in una spec o sezione interna distinta e non entrano nel client del browser.
- **ts-client** (`regenerate`, **punto serializzato**) — il client TypeScript in `apps/web` si genera dalla spec OpenAPI e non si edita mai a mano. **Regola meccanica: un solo piano con superficie API per onda** (`onde-slice` la applica), perché la rigenerazione riscrive l'intero artefatto. Il piano che cambia la superficie HTTP rigenera una volta, a backend verde, e verifica l'assenza di deriva (`git diff --exit-code` sul generato dopo una seconda generazione). Un piano che non cambia la superficie non rigenera.
- **frontend-route** (`create-once` + innesto) — `apps/web/src/app/routes.tsx` compone array esportati da `features/<feature>/routes.tsx`; la navigazione si deriva dai dati delle route (`handle`), non da un secondo elenco a mano. La Fondazione pre-registra lo spread di ogni feature di tutte le fasi pianificate, ciascuna con un array vuoto; la fase proprietaria riempie il proprio file. Se il progetto usa **MSW**, stesso schema: `src/mocks/handlers.ts` fa lo spread di `features/<feature>/mocks/handlers.ts`, e la slice non edita l'indice.
- **audit/outbox-event-registry** (`create-once` + `append`) — un tipo evento è un nome versionato e stabile (`document.uploaded.v1`), dichiarato come costante nel file della feature che lo emette (`src/events/<feature>.rs`, `pub const EVENTS: &[EventType]`); il registro del servizio (`src/events/registry.rs`) concatena gli elenchi delle feature con righe pre-registrate dalla Fondazione, così nessuna fase appende a un file di un'altra. I payload che attraversano servizi stanno in `crates/contracts`. I consumatori registrano l'handler nel composition root (`dispatcher.register(EventType, Arc<dyn OutboxHandler>)`), mai un `match` sul nome. Un nome evento non si riusa mai: si ritira e se ne crea una versione nuova. Test: nomi unici, e ogni evento registrato ha un consumatore o la dichiarazione esplicita «nessun consumatore».
- **compose-service** (`create-once`) — un servizio Compose per container in `infra/`; li aggiunge la Fondazione per i container dell'architettura (Postgres+pgvector, i tre servizi). Un container nuovo deciso più tardi è una deviazione registrata.
- **config-key** (`append`) — `.env.example` diviso in sezioni per servizio con prefisso (`IDENTITY_`, `DOCUMENT_`, `RAG_`, più le chiavi condivise come `DATABASE_URL`); ogni chiave nuova entra nella stessa fase del codice che la legge, in coda alla sezione del servizio (`.env.example` è nella whitelist di `proprieta-file` come `append`). Lato Rust, la configurazione di una feature è una struct nel proprio file, pre-registrata come campo dalla Fondazione. Nessun segreto reale nel file d'esempio.

## Pre-registrazione di progetto (Fondazione)

Decisione utente 2026-10-18, strategia «provider-first, insiemi di file chiusi per fase» (spec §1-bis). Ogni fase possiede un insieme **chiuso** di file, che le altre non modificano (gate `proprieta-file`, `scripts/gate-proprieta-file.cjs`), salvo la whitelist `scripts/proprieta-file.whitelist.json`, allineata alla sezione «Strategia di consegna» del ROADMAP: **append-only** — sezioni di dipendenze dei `Cargo.toml` e di `apps/web/package.json`, `.env.example`, `.planning/WIRING-MAP.md`, una sezione per fase in `docs/architecture/data-model.md`, `docs/architecture/api-contract.md` e nella sezione comandi di `CLAUDE.md`; **rigenerati** — `Cargo.lock`, `apps/web/package-lock.json`, OpenAPI generato, client TS generato, `.sqlx/`. I file di migrazione non servono in whitelist: sono file nuovi il cui nome porta la fascia di numerazione della fase. Un difetto in un file chiuso si corregge in una **fase decimale di gap-closure** che riapre esplicitamente quel perimetro (`riapre_perimetro: [N]` nel frontmatter dei suoi piani), mai dentro un'altra fase. Perché una fase successiva non debba aprire i file di un'altra, la **Fondazione** crea, al proprio `plan`, gli innesti vuoti di **ogni** fase successiva del ROADMAP:
- **file-modulo** di ciascuna fase (`services/<svc>/src/app/<feature>.rs` con `compose` vuota, moduli HTTP, `src/events/<feature>.rs` vuoto, `features/<feature>/routes.tsx` con array vuoto, eventuali frammenti OpenAPI);
- **righe di registrazione** nei composition root (`app.rs`: `.merge(<feature>::compose(&infra))`), nel registro delle route del frontend, nel registro degli eventi, nella struct di configurazione;
- **cartelle delle migrazioni** per servizio (i file di migrazione li possiede, per nome, la fase che li scrive);
- **membri del workspace** per tutti i crate pianificati (anche vuoti) e le dipendenze già note in `[workspace.dependencies]`; le altre si appendono più tardi (sezioni append-only);
- i **trait-porta** con tutte le operazioni previste (la Fase Provider ne possiede gli adapter, le fasi consumatrici li usano).

Regole:
- ogni file d'innesto ha **una sola** `fase-proprietaria` (colonna della WIRING-MAP): la Fondazione lo crea vuoto (`created-in`), solo la fase proprietaria lo riempie. Un registro alimentato da più fasi si spezza in un file d'innesto per fase (es. `app/parsers_pdf.rs`, `app/parsers_docx.rs`), ciascuno pre-registrato con la propria riga;
- il file che contiene le righe di registrazione (composition root, indice delle route, registro eventi) resta della Fondazione: le fasi successive non lo toccano, quindi **non** va in whitelist;
- gli innesti si derivano dai goal delle fasi successive (`wiring-search`, asse inter-fase, in modalità di progetto) e, dopo la pianificazione delle fasi consumatrici, si riconciliano: un innesto mancante scoperto prima di eseguire la Fondazione si aggiunge ai suoi piani (re-gate); scoperto dopo, è una deviazione registrata con motivo e si crea in una fase decimale di gap-closure che riapre il perimetro della Fondazione (`riapre_perimetro: [2]`), mai con una modifica silenziosa dentro la fase che lo scopre; in entrambi i casi riga nuova nella WIRING-MAP;
- l'innesto è vuoto: niente contenuto della fase proprietaria (`perimetro-check`).

## Seam inter-fase: promettere o rimandare

Per ogni candidato di §2 decidi:
- **Promettere** (riga nella WIRING-MAP con `created-in = X`) quando il punto di estensione vive in codice che X costruisce o possiede (composition root, registro, mount point, cartella delle migrazioni del servizio) e, senza il seam, la fase futura dovrebbe modificarne l'interno. Caso preferito: X stesso usa il seam con un contributo proprio (X crea `ParserRegistry` e vi registra il parser PDF; la fase dei DOCX registrerà il suo), così nasce già esercitato. Se X non lo usa, si crea nella sola forma vuota della tabella sotto, senza test.
- **Rimandare** quando il seam vivrebbe in codice che X non tocca: lo crea la fase che lo riempie (`created-in = filled-by = Y`). Una riga si può scrivere comunque, come memoria della forma attesa. Con gli insiemi di file chiusi si rimanda **solo** se il punto d'innesto vive in file che Y stessa possiede: se l'aggancio richiederebbe una riga in un file di X (composition root, indice, registro), X deve prometterlo e pre-registrarlo, altrimenti Y violerebbe `proprieta-file`. Nella Fondazione la scelta di default è promettere (sezione sopra).

Un seam promesso si crea in wave 0 **solo vuoto**, coerentemente con `perimetro-check` (niente ponti per le fasi successive):

| kind | ammesso in X (forma vuota) | vietato in X (contenuto della fase futura) |
|---|---|---|
| trait-port | il trait con le firme promesse | adapter, fake, test della fase futura |
| registry | il tipo registro e le registrazioni di X | registrazioni o adapter delle varianti future |
| router-mount | il mount point e la funzione di composizione vuota | rotte, handler, DTO futuri |
| migration-sequence | la convenzione e la cartella del servizio | tabelle o colonne per la fase futura |
| openapi-tag | — (il tag nasce con il primo path) | path o schemi futuri |
| frontend-route | il registro e gli innesti delle feature di X | route, pagine o voci di menu future, anche segnaposto con copy |
| audit/outbox-event-registry | il registro e gli eventi di X | tipi evento della fase futura |
| workspace-member, compose-service, config-key | solo ciò che X usa; nella Fondazione anche crate vuoti, `[workspace.dependencies]` e container di tutte le fasi pianificate | codice dei crate futuri; chiavi senza consumatore in X |

Il segnaposto nudo ammesso è al più un commento `// fase Y: <seam>` accanto al punto di estensione. Niente test, copy, colonne, migrazioni o letture ponte per l'area futura.

## Procedura

1. **Convenzione per punto** — per ogni riga di §1 e §2 scegli il meccanismo sopra (o uno equivalente motivato), coerente con SOLID/OCP e con le regole di `crate-boundary-check`: un servizio non dipende da un altro servizio, dipendenze interne solo verso `crates/contracts` e `crates/telemetry`, wiring solo nel composition root.
2. **Plug-in contract** — per ogni punto, la frase esatta che una slice deve rispettare per contribuire (es. «aggiungi `features/documents/routes.tsx` che esporta `documentsRoutes: RouteObject[]`; NON editare `app/routes.tsx`»). Va in §3.
3. **Vincoli per gli esecutori (§4)** — è l'**unica** sezione che ogni esecutore riceve integralmente nel brief d'ingresso. Scrivila autosufficiente e imperativa, un punto per riga: file condivisi che nessuna slice tocca; innesti per-slice da riempire; registri a cui si appende in coda; numerazione delle migrazioni del servizio; regola del client TS («solo il piano 0X-YY rigenera»); `Cargo.lock`/`package-lock.json` mai fusi a mano; seam inter-fase da riempire con il riferimento alla riga `WM-NNN`; divieto di contenuti per fasi future. Un esecutore che deve violare un vincolo di §4 è in deviazione (b): toccare un seam della WIRING-MAP fuori da quanto §4 consente rimbalza a `plan`.
4. **Vincoli di pianificazione** — annota in §3 i vincoli meccanici per `onde-slice`: un solo piano con superficie API per onda; un solo piano per servizio e per onda con migrazioni; ogni artefatto `regenerate` ha un solo contributore per onda; dipendenze di serie forzate dai seam.
5. **Seam da creare in wave 0 (§5)** — elenco operativo per `wiring-build`: per ogni seam, intra-fase o inter-fase, il percorso, la forma vuota esatta, le righe di innesto pre-registrate per slice e, per gli inter-fase, la riga `WM-NNN`. Un seam inter-fase che non compare in §5 non si crea.
6. **WIRING-MAP** — se `.planning/WIRING-MAP.md` non esiste, crealo copiando `skills/wiring-design/WIRING-MAP.template.md`. Poi, per ogni seam inter-fase promesso o toccato da X: nuova riga con stato `promised`, oppure aggiornamento della riga esistente (fasi `filled-by`, `fase-proprietaria`, note). Per un file d'innesto la colonna `fase-proprietaria` porta l'unica fase che lo riempie, ed è ciò che `gate-proprieta-file.cjs` legge per ammettere il passaggio `created-in` → `fase-proprietaria`; `—` quando il file resta chiuso nella fase che lo crea. Una riga non si cancella mai: se il disegno cambia, la vecchia passa a `retired` con «sostituita da WM-NNN» e se ne apre una nuova; ogni variazione entra nel registro delle variazioni in coda al file, con data, fase e motivo. Un cambio di forma di un seam già `created` o `filled` è una decisione: va motivata e, se rompe ciò che una fase precedente ha promesso nel proprio CONTRATTO, rimbalza a `discuss`.
7. **Commit** — non qui: WIRING e WIRING-MAP entrano nel commit degli artefatti di fase di `plan` (passo 7).

## Vincoli

- Ogni convenzione rispetta i confini dei crate e SOLID: niente file-dio di registrazione, OCP (nuova slice o nuova fase = nuovo file più una registrazione, mai un nuovo braccio di `match`).
- `0X-WIRING.md` in caveman full; la WIRING-MAP in forma tabellare tersa.
- Nessun codice qui. I piani scritti dopo (passo 5 di `plan`) **devono** rispettare §3-§5: è il vincolo che `gsd-plan-phase` riceve con il manifest.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
**Modifica:** Claude (Anthropic) · 2026-10-18 · strategia provider-first e insiemi di file chiusi (decisione utente)
