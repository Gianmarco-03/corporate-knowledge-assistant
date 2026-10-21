---
phase: 01-spike-tecnici
plan: 03
subsystem: testing
tags: [corpus, manifest, sha256, curl, qpdf, pandoc, docx, acl, prompt-injection, leak-test, spike-data]

# Dependency graph
requires:
  - phase: 01-spike-tecnici (piano 01-02)
    provides: "spikes/tools/session.sh (with, oneshot, down, release), runner con qpdf, file segreti esterno $HOME/.cka-spikes/spikes.env, spikes/.gitignore con data/**/raw/"
provides:
  - "spikes/data/corpus/manifest.json (commit 58930ec e a63908e): 23 voci (9 pubblici, 1 derivato TTQ, 9 sintetici md, 4 DOCX derivati) con doc_id, lang, kind, format, classification, source_url, license, sha256, pages, bytes, derived_from, oracle_author"
  - "Corpus pubblico IT/EN: 5 documenti Banca d'Italia (36-228 pagine) e 4 relazioni annuali EN (Berkshire con 10-K, Federal Reserve, Bank of England, BIS), circa 1.360 pagine; binari solo in raw/ ignorato"
  - "Estratto TTQ der-it-01-relazione-annuale-2025-p1-25: 25 pagine, 1.994.134 byte, riproducibile byte per byte con qpdf --deterministic-id"
  - "spikes/data/corpus/fetch.sh: download solo https con limiti curl, verifica sha256, idempotente, derivato prodotto nel runner dentro una sessione, controllo degli sha dei file versionati"
  - "Nove documenti sintetici classificati di Alfa Componenti S.p.A. (public, internal, confidential, restricted; IT ed EN) con front matter e tabelle"
  - "Due documenti con 4 casi di prompt injection ciascuno (syn-07 public, syn-08 internal) delimitati da <!-- injection-case: id --> e <!-- end-injection-case: id -->"
  - "Documento trappola syn-09 (restricted): unica fonte del corpus sul Progetto Cormorano"
  - "Quattro DOCX derivati con Heading1-3 e interruzione di pagina, ricostruibili byte per byte con synthetic/build.sh (pandoc/core:3.11.0.0 per digest, SOURCE_DATE_EPOCH fisso)"
  - "spikes/data/corpus/acl.json: matrice di misura ruolo × classificazione (admin, manager, employee, guest) e profilo restricted-only con obiettivo di selettività ≤ 5%"
affects: [01-05, 01-06, 01-07, 01-08, 01-09, 01-10, 01-11, 01-12, 01-13, 01-14, 01-15]

# Tech tracking
tech-stack:
  added: [pandoc/core 3.11.0.0 (immagine per digest, solo build dei DOCX), qpdf 12.2.0 (già nel runner di 01-02)]
  patterns:
    - "Manifest immutabile: gli hash si fissano al primo download; una sostituzione è una voce nuova con motivo in CORPUS.md"
    - "Scaricamento in file .part, verifica sha256, rename solo se coincide"
    - "Script host che si rilanciano dentro session.sh with 01-03 se il chiamante non ha una sessione (CKA_RUN_ID)"
    - "Derivati riproducibili: qpdf --deterministic-id, pandoc con SOURCE_DATE_EPOCH fisso"
    - "Fixture di injection delimitate da commenti HTML con id, URL solo verso example.com"

key-files:
  created:
    - spikes/data/corpus/CORPUS.md
    - spikes/data/corpus/manifest.json
    - spikes/data/corpus/acl.json
    - spikes/data/corpus/fetch.sh
    - spikes/data/corpus/synthetic/build.sh
    - spikes/data/corpus/synthetic/syn-01-budget-interno.md
    - spikes/data/corpus/synthetic/syn-02-piano-industriale-riservato.md
    - spikes/data/corpus/synthetic/syn-03-verbale-cda-restricted.md
    - spikes/data/corpus/synthetic/syn-04-policy-spese-internal.md
    - spikes/data/corpus/synthetic/syn-05-risk-report-confidential-en.md
    - spikes/data/corpus/synthetic/syn-06-faq-public.md
    - spikes/data/corpus/synthetic/syn-07-injection-public.md
    - spikes/data/corpus/synthetic/syn-08-injection-internal-en.md
    - spikes/data/corpus/synthetic/syn-09-trappola-restricted.md
    - spikes/data/corpus/synthetic/syn-01-budget-interno.docx
    - spikes/data/corpus/synthetic/syn-02-piano-industriale-riservato.docx
    - spikes/data/corpus/synthetic/syn-05-risk-report-confidential-en.docx
    - spikes/data/corpus/synthetic/syn-09-trappola-restricted.docx
  modified: []

key-decisions:
  - "Fonte IT unica Banca d'Italia: permesso esplicito di riproduzione per uso didattico non commerciale e URL stabili; nessuna società quotata italiana"
  - "Niente SEC EDGAR (richiede un User-Agent con recapito personale): il 10-K è coperto dal PDF Berkshire sul sito dell'emittente"
  - "pages: null per md e DOCX (nessuna paginazione intrinseca); fetch.sh lo valida"
  - "doc_id dei DOCX = doc_id del sorgente + suffisso -docx, file synthetic/<derived_from>.docx, per non avere doc_id duplicati"
  - "Nome della trappola «Progetto Cormorano» invece di «Aurora»: il BIS AER 2025 cita un Project Aurora reale"
  - "fetch.sh controlla solo la presenza di CKA_SPIKES_ENV e non lo legge (lo legge session.sh): minimo privilegio rispetto al «set -a; . env» del piano"

patterns-established:
  - "Corpus come fonte di verità di sola lettura per gli altri piani (WIRING W11): consumatori calcolano la visibilità attesa da acl.json + manifest, mai dalla tabella che implementa il filtro"
  - "Precondizione di worktree: bash spikes/data/corpus/fetch.sh ricostruisce raw/ da zero o verifica senza rete se raw/ è già completo"

requirements-completed: [PLAT-07]

coverage:
  - must_have: "Il corpus degli spike è pubblico o sintetico: relazioni annuali pubbliche IT/EN più documenti sintetici classificati che simulano la riservatezza (D-16)"
    verification:
      - {test: TEST-6, risks: [RISK-6], ref: "spikes/data/corpus/manifest.json ; verify del Task 1 (node -e) ; controllo di schema del bridge", status: pass, evidence: "rieseguito dal bridge: 23 voci, doc_id univoci, 5 pubblici it e 4 en, 9 sintetici, 1 derivato PDF da 25 pagine e 1.994.134 byte con derived_from su pub-it-01, enumerazioni lang/kind/format valide, oracle_author null ovunque, chiavi title/pages/bytes/derived_from presenti"}
      - {test: TEST-2, risks: [RISK-2], ref: "front matter di synthetic/syn-0N-*.md ; manifest.json:153-345 ; docProps/custom.xml dei 4 DOCX", status: pass, evidence: "rieseguito dal bridge: classificazione del front matter = manifest per i 9 sintetici e = classificazione attesa per nome (syn-01 internal … syn-09 restricted); ogni DOCX ha la classificazione del sorgente anche in docProps/custom.xml; i 9 pubblici e il derivato TTQ sono public"}
  - must_have: "Ogni documento ha provenienza registrata (source_url o autore sintetico, licenza, sha256) e i file pubblici non sono ridistribuiti: si versionano URL e hash, i binari stanno in raw/ ignorato (D-11)"
    verification:
      - {test: TEST-9, risks: [RISK-4], ref: "manifest.json:3-150 (pubblici) ; front matter author dei sintetici", status: pass, evidence: "rieseguito dal bridge: ogni pubblico ha source_url https e licenza con «non ridistribuito»; ogni sintetico ha source_url null, licenza CC0 e author: synthetic (Claude)"}
      - {test: TEST-8, risks: [RISK-5], ref: "git status --porcelain spikes/data/corpus/raw ; git ls-files spikes/data/corpus ; git check-ignore", status: pass, evidence: "rieseguito dal bridge dopo il fetch sul main: raw/ non compare in git status (spikes/.gitignore:5 data/**/raw/), nessun PDF tracciato, gli unici binari tracciati sono i 4 DOCX sintetici"}
      - {test: TEST-3, risks: [RISK-3], ref: "spikes/data/corpus/fetch.sh:78,93", status: pass, evidence: "rieseguito dal bridge: grep -c 'http://' = 0 su fetch.sh e manifest.json; --proto '=https', --max-redirs 3, --max-time 300, --max-filesize 104857600 e sha256sum presenti una volta ciascuno; la validazione del manifest rifiuta source_url non https prima di ogni accesso"}
      - {test: TEST-4, risks: [RISK-3, RISK-7], ref: "spikes/data/corpus/fetch.sh:107-114 (file presente con sha errato) ; :121-127 (download con sha errato)", status: pass, evidence: "rieseguito dal bridge sul main: 1 byte alterato a offset 100 in raw/pub-en-02-...pdf (sha diventato 646b5a52…), fetch.sh exit 0 con «sha256 errato su file presente, scartato e riscaricato» poi «scaricato, sha256 ok», sha finale fc5e7145… = manifest. Il ramo «download con sha errato → file scartato, exit 1» (:122-132) non è provato end-to-end: vedi la voce human_judgment sotto"}
      - {test: TEST-4 (ramo exit 1 su download con sha errato), risks: [RISK-3], ref: "spikes/data/corpus/fetch.sh:121-132", status: human_judgment, human_judgment: true, rationale: "provarlo richiede un manifest alterato o un emittente che cambia il file; né l'esecutore né il bridge lo hanno eseguito. La lettura del codice mostra rm del .part, failed=1 e die 1 alla riga 132, ma la prova resta solo statica"}
      - {test: TEST-5, risks: [RISK-7], ref: "touch stamp ; bash spikes/data/corpus/fetch.sh ; find raw -newer stamp", status: pass, evidence: "rieseguito dal bridge: primo fetch sul main da raw/ assente exit 0 (9 download, derivato prodotto nel runner con sha coincidente, quindi qpdf riproducibile anche a un giorno di distanza); secondo fetch exit 0, nessun file nuovo in raw/, 10 righe «già presente, sha256 ok», nessun download né sessione"}
  - must_have: "Esiste una matrice sintetica ruolo × classificazione con almeno un profilo a selettività ≤ 5% e un documento trappola, così il leak test degli spike ha un oracolo di visibilità deterministico (D-12, D-15)"
    verification:
      - {test: TEST-1, risks: [RISK-1], ref: "spikes/data/corpus/acl.json:4-17", status: pass, evidence: "rieseguito dal bridge contro la matrice scritta nel piano (non contro lo stesso file): ruoli esattamente admin, manager, employee, guest con insiemi {tutte}, {public, internal, confidential}, {public, internal}, {public}; profilo restricted-only con visible esattamente [restricted]"}
      - {test: TEST-13, risks: [RISK-10], ref: "spikes/data/corpus/acl.json:2 ; CORPUS.md:93", status: pass, evidence: "rieseguito dal bridge: _note con «Matrice di misura» e rimandi a Q-01/Q-02; CORPUS.md ripete il disclaimer in grassetto e ha le righe **Modifica:**"}
      - {test: TEST-11 (trappola), risks: [RISK-8], ref: "synthetic/syn-09-trappola-restricted.md:1-8,14-20", status: pass, evidence: "rieseguito dal bridge: front matter restricted; Cormorano, Valfonda, Gamma Immobiliare e Alfa Componenti assenti dal testo di tutti i 10 PDF in raw/ (pdftotext) e presenti solo in syn-09 fra i md (oltre a CORPUS.md e manifest.json, che lo descrivono). «Project Aurora» compare 2 volte nel BIS AER 2025: la scelta del nome è confermata"}
      - {test: RISK-11 (selettività ≤ 5%), risks: [RISK-11], ref: "acl.json:13 (max_chunk_selectivity 0.05) ; 01-03-TEST-DESIGN.md uncovered", status: human_judgment, human_judgment: true, rationale: "non misurabile in questo piano: la selettività si calcola sui chunk, che nascono in S-03. Il design la mette in uncovered e la consegna a 01-14 (comando leak). Stima grezza dell'esecutore, non verificata: 2 documenti restricted brevi contro circa 1.360 pagine pubbliche"}
  - must_have: "Il corpus contiene documenti con prompt injection indiretta per misurare la tenuta di LLM e JEV (D-20)"
    verification:
      - {test: TEST-11, risks: [RISK-8], ref: "synthetic/syn-07-injection-public.md:24-52 ; synthetic/syn-08-injection-internal-en.md:27-55 ; python skills/security-scanner/security_scan.py spikes/data/corpus", status: pass, evidence: "rieseguito dal bridge: grep -l 'injection-case:' elenca solo syn-07 e syn-08; 4 aperture e 4 chiusure in ciascuno; tutti gli URL dei sintetici sono https verso www.example.com o exfil.example.com; scanner «OK - nessun problema di sicurezza rilevato su 14 file», exit 0"}
  - must_have: "DOCX derivati utili al locator e hash di tutti i file versionati coincidenti col manifest (artifact fetch.sh e Task 2)"
    verification:
      - {test: TEST-7, risks: [RISK-6, RISK-2], ref: "bash spikes/data/corpus/synthetic/build.sh ; sha256 dei 9 md e 4 DOCX", status: pass, evidence: "rieseguito dal bridge: build.sh dentro session.sh with 01-03 (run_id 01-03-20261021t094148z-4ba6) exit 0, «DOCX ok, sha256 coincide» per i 4 file, down con 0 residui, prenotazione rilasciata, working tree pulito; sha dei 9 md e dei 4 DOCX = manifest anche al controllo indipendente"}
      - {test: TEST-12, risks: [RISK-9], ref: "unzip -p synthetic/<doc>.docx word/document.xml ; synthetic/build.sh:28,85-92", status: pass, evidence: "rieseguito dal bridge: Heading1/Heading2 e page break = syn-01 6/1, syn-02 6/1, syn-05 7/1, syn-09 5/1; CORPUS.md:85 riporta il tag pandoc/core:3.11.0.0 e il digest sha256:62518bfb…553a"}
  - must_have: "Nessun dato personale reale nei sintetici (T-01-03-02)"
    verification:
      - {test: TEST-10 (parte automatica), risks: [RISK-4], ref: "grep codice fiscale / IBAN / email su synthetic/*.md", status: pass, evidence: "rieseguito dal bridge: nessuna corrispondenza per codice fiscale, IBAN ed email"}
      - {test: TEST-10 (parte manuale, nomi), risks: [RISK-4], ref: "lettura dei nove synthetic/syn-0N-*.md", status: human_judgment, human_judgment: true, rationale: "il design la assegna a una lettura umana; né l'esecutore né il bridge l'hanno fatta. L'esecutore dichiara persone citate solo per carica e società fittizie"}

# Metrics
duration: 2h55m
completed: 2026-10-20
---

# Phase 1 Plan 03: Corpus comune degli spike Summary

**Corpus degli spike con 9 relazioni pubbliche IT/EN scaricabili solo in HTTPS e verificate per sha256 (binari mai versionati), estratto TTQ da 25 pagine riproducibile, 9 documenti sintetici classificati con 8 casi di prompt injection e un documento trappola restricted, 4 DOCX ricostruibili byte per byte e una matrice di visibilità per ruolo come oracolo dei leak test**

## Performance

- **Duration:** circa 2 h 55 min dalla base dell'onda (`ecd61ed`, 2026-10-19 21:51) all'ultimo commit del piano (`a63908e`, 2026-10-20 00:46), ora italiana; il merge `0d0d630` è del 2026-10-21 11:37
- **Started:** 2026-10-19T21:51:48+02:00 (base dell'onda 2; l'ora d'inizio reale dell'esecutore non è registrata)
- **Completed:** 2026-10-20T00:46:20+02:00
- **Tasks:** 2 di 2
- **Files modified:** 18 creati, esattamente quelli di files_modified

## Accomplishments

- `manifest.json` con 23 voci e tutte le chiavi dello schema W11: 5 documenti Banca d'Italia (relazione annuale 228 pagine, bilancio 105, sintesi a due colonne 36, bollettino economico 42, relazione sulla gestione 212) e 4 relazioni annuali EN (Berkshire con 10-K 154, Federal Reserve 214, Bank of England 220, BIS 126).
- Estratto TTQ `der-it-01-relazione-annuale-2025-p1-25` (25 pagine, 1,99 MB) prodotto con `qpdf --deterministic-id` nel runner: il bridge lo ha rigenerato da zero un giorno dopo e lo sha coincide.
- `fetch.sh` idempotente: valida il manifest prima di toccare rete o disco, scarica solo in HTTPS con redirect, tempo e dimensione limitati e tre tentativi, scarta i file con sha errato, produce il derivato dentro una sessione di `session.sh`, controlla anche gli sha dei file versionati.
- Nove sintetici di Alfa Componenti S.p.A. con cifre coerenti fra loro, front matter, intestazioni su tre livelli e tabelle; `syn-07` e `syn-08` con quattro casi di injection ciascuno (ignorare le istruzioni, citare un id inesistente, link o immagine di esfiltrazione, rivelare il prompt); `syn-09` come trappola restricted.
- `spikes/data/corpus/synthetic/build.sh` produce i 4 DOCX con pandoc per digest, senza rete, senza capability e con uid 10001, e li installa solo se coincidono col manifest.
- `acl.json` con la matrice dei quattro ruoli di progetto, il profilo `restricted-only` e il disclaimer «matrice di misura, non decisione di prodotto (Q-01/Q-02)».

## Task Commits

1. **Task 1: relazioni pubbliche IT/EN, manifest e download riproducibile** - `58930ec` (feat)
2. **Task 2: documenti sintetici classificati, DOCX derivati e matrice di visibilità** - `a63908e` (feat)

Merge su main: `0d0d630` (onda 2).

## Files Created/Modified

- `spikes/data/corpus/CORPUS.md` - criteri di scelta, tabella dei documenti con la misura servita, licenze, rigenerazione, sintetici, injection, trappola, DOCX, matrice
- `spikes/data/corpus/manifest.json` - elenco immutabile dei 23 documenti con provenienza e sha256
- `spikes/data/corpus/acl.json` - matrice ruolo × classificazione e profilo di selettività
- `spikes/data/corpus/fetch.sh` - ricostruzione e verifica di raw/ e dei file versionati
- `spikes/data/corpus/synthetic/build.sh` - conversione md → DOCX riproducibile
- `spikes/data/corpus/synthetic/syn-01 … syn-09 *.md` - sorgenti sintetici classificati
- `spikes/data/corpus/synthetic/syn-01, syn-02, syn-05, syn-09 *.docx` - DOCX derivati per il locator

## Decisions Made

- **Fonte IT unica Banca d'Italia:** dichiara il permesso di riproduzione per uso didattico non commerciale e ha URL stabili. Nessuna società quotata italiana nel corpus.
- **Niente SEC EDGAR:** l'accesso automatico chiede un User-Agent con un recapito personale. Il 10-K arriva dal PDF Berkshire sul sito dell'emittente.
- **`pages: null` per md e DOCX:** l'impaginazione la decide il lettore; `fetch.sh:82` lo impone.
- **doc_id dei DOCX col suffisso `-docx`:** il file resta synthetic/«derived_from».docx, il doc_id è distinto (`fetch.sh:178-182`).
- **Trappola «Progetto Cormorano»:** «Aurora» collideva con un Project Aurora reale citato dal BIS (confermato dal bridge: 2 occorrenze nel PDF).
- **`CKA_SPIKES_ENV` solo controllato, non letto:** lo legge `session.sh`; lo script host non ha bisogno dei segreti (`fetch.sh:24-26,149`).
- **Manifest immutabile anche per build.sh:** un DOCX diverso fa uscire 1 senza toccare il file versionato; un DOCX non registrato si installa ma l'uscita è 1 (`build.sh:105-124`).
- **Auto-rilancio in sessione:** senza `CKA_RUN_ID` gli script si rilanciano in `session.sh with 01-03 --wait ${CKA_FETCH_WAIT_S:-900}` invece di fallire (`fetch.sh:150-157`, `build.sh:73-78`).

## Deviations from Plan

### Auto-fixed Issues (livello a)

**1. [Rule 1 - Bug] REPO calcolato con `git -C` falliva dentro session.sh**
- **Found during:** Task 1 · **Issue:** dentro `session.sh` vale `MSYS_NO_PATHCONV=1`, quindi `git.exe` e `node.exe` ricevevano percorsi `/c/...` che non risolvono (primo run con exit 128) · **Fix:** radice del repo dalla posizione fissa del file (`fetch.sh:30-34`) e `cygpath -m` per node (`fetch.sh:56-58`) · **Committed in:** `58930ec`

**2. [Deviazione a] claim di layout ridotte in CORPUS.md**
- Le tabelle multipagina non sono state verificate su ogni pagina: CORPUS.md:15 dichiara che le caratteristiche di layout sono state viste su pagine campione e che la conferma spetta a S-02 · **Committed in:** `58930ec`

**3. [Deviazione a] container esplorativo fuori sessione**
- Un `docker run --rm --network none` del runner, per contare le pagine dei PDF candidati, è partito prima di `session.sh` e senza l'etichetta `cka.run_id`. Tutte le esecuzioni successive (qpdf, pdftotext, pandoc) sono passate da `session.sh with 01-03`. Il bridge ha controllato: nessuna prenotazione attiva e nessun container con etichetta `cka.run_id` residuo. Non ci sono commit associati.

---

**Total deviations:** 3 (1 auto-fix, 2 di processo o documentali); nessuna di livello b o c.
**Impact on plan:** nessun file fuori da files_modified; `node scripts/gate-proprieta-file.cjs --fase 1` rieseguito dal bridge: conforme.

## Issues Encountered

- `raw/` non esisteva sul main: il bridge ha dovuto scaricare i 9 PDF (circa 55 MB) per eseguire il verify del Task 1. Tutti i download sono riusciti al primo tentativo.

## Discrepanze fra rapporto e prove

- Il rapporto cita come prova uno script `tests.js` nello scratchpad dell'esecutore, non versionato. Il bridge non lo ha letto: ha riscritto i controlli TEST-1, 2, 6, 7, 9, 11, 12 e 13 da zero a partire dal design e li ha rilanciati, con esito verde.
- Nessun'altra discrepanza: commit, file, conteggi del manifest, flag di curl, comportamento di fetch.sh e build.sh, esito dello scanner e collisione «Project Aurora» coincidono con le prove.

## Threat Flags

| Flag | File | Description |
|------|------|-------------|
| threat_flag: tampering | spikes/data/corpus/fetch.sh | i PDF pubblici sono non fidati e finiranno nei parser degli spike; la verifica sha256 garantisce che siano gli stessi del 2026-10-20, non che siano innocui. L'isolamento del parsing resta a carico di 01-07 e 01-13 |
| threat_flag: info | spikes/data/corpus/synthetic/syn-07, syn-08 | fixture di injection volute, classificate public e internal: si possono inviare a JEV e all'LLM per misura (D-16) |

## Known Stubs

Nessuno. `oracle_author` è null in tutte le voci per scelta del piano: gli oracoli li deriva 01-06.

## User Setup Required

Nessuno.

## Punti aperti

- Il ramo «download con sha errato → file scartato, exit 1» di `fetch.sh` (`:121-132`) non è provato end-to-end: richiede un manifest alterato.
- La lettura umana dei nomi nei nove documenti sintetici, per escludere dati personali reali (TEST-10, parte manuale), è da fare.
- Un container esplorativo è stato lanciato fuori sessione e senza etichetta durante il Task 1 (deviazione 3); nessun residuo, ma va ricordato se si controllano i log di Docker.
- RISK-11 (selettività ≤ 5% del profilo `restricted-only`) è consegnato a 01-14, che la misura dopo il chunking.
- Contenuti duplicati da segnalare ai consumatori: il TTQ è identico alle prime 25 pagine di `pub-it-01`, ogni DOCX ha lo stesso testo del suo md. I md di syn-01, 02, 05 e 09 contengono una riga `\newpage` letterale.
- Gli sha dei DOCX dipendono da pandoc 3.11 e `SOURCE_DATE_EPOCH=1767225600`; quello del TTQ da qpdf 12.2.0 nell'immagine del runner. Un cambio di immagine richiede voci nuove nel manifest.

## Next Phase Readiness

- S-02, S-03, S-04 e A-04 possono partire dallo stesso corpus: ogni worktree lancia `bash spikes/data/corpus/fetch.sh` come precondizione.
- PLAT-07 è copiato dal frontmatter del piano: qui sono attuate le mitigazioni T-01-03-01..04, non il requisito intero. Se il requisito sia chiuso lo decide il verifier di fase.

## Self-Check: PASSED

- I 18 file di files_modified sono tutti nel diff `ecd61ed..a63908e`, e non ce ne sono altri.
- I commit `58930ec`, `a63908e` e il merge `0d0d630` sono nella storia di main.
- Rieseguiti dal bridge sul main: fetch.sh da zero (exit 0), idempotenza, file manomesso, build.sh (exit 0, DOCX identici), controlli TEST-1, 2, 3, 6, 7, 8, 9, 11, 12, 13, grep CF/IBAN/email, scanner (exit 0), gate proprietà dei file (conforme). Dopo le prove, nessuna sessione aperta e working tree pulito.

---
*Phase: 01-spike-tecnici*
*Completed: 2026-10-20*

**Modifica:** Claude (Anthropic) · 2026-10-21 · bridge del piano 01-03
