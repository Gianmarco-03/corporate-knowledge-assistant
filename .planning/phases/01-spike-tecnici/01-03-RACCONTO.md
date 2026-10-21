# Racconto del piano 01-03: il corpus comune degli spike

## Cosa

Prima di questo piano gli spike avevano un ambiente (01-02) ma niente su cui misurare. Adesso hanno un corpus condiviso, descritto da un manifest immutabile. Contiene nove relazioni annuali pubbliche, cinque italiane della Banca d'Italia e quattro inglesi (Berkshire Hathaway, Federal Reserve, Bank of England, BIS), per circa 1.360 pagine. A queste si aggiungono un estratto di 25 pagine che è il documento di riferimento del target TTQ, nove documenti sintetici di una società inventata con quattro livelli di classificazione e i DOCX derivati da quattro di quei sintetici. Una matrice dice quale ruolo vede quale classificazione: con questa matrice e con il manifest, i leak test degli spike hanno un oracolo deterministico. Due sintetici contengono casi di prompt injection, un terzo è il documento trappola che solo chi ha accesso `restricted` deve poter vedere.

I PDF pubblici non entrano nel repository. Si versionano l'URL, la licenza e lo sha256, e chiunque ricostruisce i file con un comando, ottenendo gli stessi byte. Il piano porta PLAT-07 per la parte degli spike: provenienza registrata, solo HTTPS, nessun dato reale riservato che possa uscire verso JEV o l'LLM (D-16).

## Come

### Percorso di lettura consigliato

1. **`spikes/data/corpus/CORPUS.md`**: è la spiegazione in prosa di tutto il resto. La tabella dei documenti pubblici (`CORPUS.md:17-28`) dice per quale misura è stato scelto ciascuno: lunghezza, tabelle dense, due colonne, grafici vettoriali, TTQ. Poi vengono le licenze (`:32-34`), la rigenerazione (`:36-46`), i sintetici (`:48-64`), i casi di injection (`:66-77`), la trappola (`:79-81`), i DOCX (`:83-89`) e la matrice (`:91-105`).

2. **`spikes/data/corpus/manifest.json`**: un array di 23 oggetti con le stesse chiavi. I pubblici occupano `manifest.json:3-136`, il derivato TTQ inizia a `:138`, i sintetici a `:153`, i DOCX a `:288`. Due campi meritano attenzione. `derived_from` collega l'estratto TTQ alla relazione da cui nasce e ogni DOCX al suo Markdown. `oracle_author` è sempre `null`: gli oracoli delle domande li scrive il piano 01-06, e il campo esiste già perché lo schema W11 sia stabile.

3. **`spikes/data/corpus/fetch.sh`**: è lo script che ogni worktree lancia prima di misurare.
   - `set -euo pipefail` (`fetch.sh:28`) fa uscire lo script al primo comando fallito, a ogni variabile non definita e a ogni errore dentro una pipeline.
   - La radice del repository si ricava dalla posizione del file (`:30-34`) e non da `git -C`. Il motivo è nel commento. Dentro `session.sh` la conversione automatica dei percorsi di Git Bash è spenta (`MSYS_NO_PATHCONV=1`), quindi `git.exe` riceverebbe un percorso `/c/...` che non sa risolvere. Per lo stesso motivo `native()` (`:56-58`) converte i percorsi con `cygpath -m` prima di passarli a `node.exe`, e su Linux li lascia com'erano.
   - La validazione del manifest (`:65-85`) è uno script `node -e` che controlla ogni voce prima di qualsiasi accesso alla rete o al disco. Il doc_id deve avere una forma sicura per un nome di file (`:72`): un doc_id come `../x` non può scrivere fuori da `raw/`. Lo sha256 deve essere di 64 caratteri esadecimali, i pubblici devono avere un URL `https://` (`:78`) e un derivato deve puntare a una voce già vista (`:79`). Le pagine sono un intero per i PDF e `null` per gli altri formati (`:82`). L'output è una riga per voce con i campi separati da TAB, e `tr -d '\r'` toglie il ritorno a capo che `node.exe` aggiunge su Windows.
   - `download()` (`:90-102`) chiama `curl` con `--proto '=https'`, che rifiuta ogni altro protocollo anche dopo un redirect, con al massimo 3 redirect, 300 secondi e 100 MiB. Fa tre tentativi, con una pausa crescente fra l'uno e l'altro.
   - Il ciclo dei pubblici (`:104-130`) legge le righe con `while IFS=$'\t' read -r …`. `IFS` è il carattere con cui `read` divide la riga in campi; qui è il TAB, come nell'output di node. `done <<< "$ENTRIES"` è una *here-string*, cioè passa la variabile come input del ciclo, che così resta nella shell principale e può aggiornare `failed`. Per ogni voce lo script fa una di tre cose. Se il file c'è e lo sha coincide, lo salta. Se c'è ma lo sha non coincide, lo cancella e lo riscarica (`:107-114`). Se non c'è, lo scarica in un file `.part`, ne calcola lo sha e lo rinomina solo se coincide (`:115-129`). Un file a metà o sbagliato non prende mai il nome definitivo. Un fallimento non interrompe subito il ciclo: segna `failed=1`, e alla fine lo script esce con 1 (`:132`).
   - Il ramo dei derivati (`:134-174`) raccoglie gli estratti mancanti. Se non ne manca nessuno, non parte nessun container. Se ne manca qualcuno e il chiamante non ha una sessione, lo script si rilancia da sé dentro `session.sh with 01-03` (`:150-157`). `exec` sostituisce il processo corrente invece di crearne uno figlio. La variabile `CKA_FETCH_IN_SESSION` impedisce un ciclo infinito di rilanci, nel caso la sessione non esportasse `CKA_RUN_ID`. Dentro la sessione `qpdf` gira nel runner con `oneshot` (`:163-165`) e i percorsi sono quelli del container (`/src/data/corpus/raw/...`). `--deterministic-id` fa sì che qpdf calcoli l'identificativo del PDF dal contenuto e non da un valore casuale, quindi lo sha resta lo stesso a ogni esecuzione.
   - L'ultimo ciclo (`:177-189`) controlla gli sha dei file versionati in `synthetic/`. Il `case $kind/$fmt` (`:180-184`) distingue un sorgente Markdown (`<doc_id>.md`) da un DOCX, il cui file porta il nome del sorgente (`<derived_from>.docx`).

4. **`spikes/data/corpus/synthetic/*.md`**: ogni file inizia con un front matter YAML, cioè il blocco fra due righe `---` in testa (`syn-09-trappola-restricted.md:1-6`), con `doc_id`, `classification`, `lang` e `author`. La classificazione è ripetuta nella prima riga del testo (`syn-09-trappola-restricted.md:10`). I casi di injection stanno fra commenti HTML con un id, per esempio `syn-07-injection-public.md:24-26` per il caso `inj-07-a` e `:42-44` per il link di esfiltrazione verso `https://exfil.example.com`. Un commento HTML non compare quando il Markdown viene visualizzato, ma resta nel testo che il parser estrae: per questo gli spike sanno dove cercare. La trappola descrive il «Progetto Cormorano» (`syn-09-trappola-restricted.md:14-20`): la cessione di uno stabilimento inventato per 12,7 milioni di euro, un'informazione che nel corpus compare solo lì.

5. **`spikes/data/corpus/synthetic/build.sh`**: trasforma quattro Markdown in DOCX.
   - L'immagine è fissata per tag e per digest (`build.sh:28`). Il tag da solo può essere ripubblicato con un contenuto diverso, il digest no.
   - `SOURCE_DATE_EPOCH` (`:29`, passato al container a `:98`) è una convenzione rispettata da pandoc e da molti altri strumenti di build: al posto dell'ora corrente scrivono quella data nei metadati. Senza, ogni DOCX conterrebbe un orario diverso e quindi uno sha diverso.
   - Prima di avviare il container, lo script controlla gli sha dei sorgenti contro il manifest (`:62-70`), così un sorgente modificato non produce un DOCX che poi verrebbe scartato.
   - Il filtro Lua (`:85-92`) è un piccolo programma che pandoc applica al documento già letto. Trova i blocchi TeX grezzi che contengono solo `\newpage` e li sostituisce con un frammento OpenXML, il formato interno di Word, che rappresenta un'interruzione di pagina. Il filtro viene scritto con un *heredoc* quotato (`<<'LUA'`): le virgolette impediscono a bash di espandere `$` o backslash dentro il testo.
   - Il container (`:96-102`) gira senza rete, senza capability, come uid 10001 e con l'etichetta `cka.run_id`, così il `down` della sessione lo pulisce anche in caso di interruzione.
   - La regola d'installazione (`:105-124`) è asimmetrica. Un DOCX che coincide si copia. Uno che non coincide fa uscire con 1 senza toccare il file versionato. Uno non ancora registrato si copia, ma lo script esce comunque con 1. Il manifest non si aggiorna mai da solo.

6. **`spikes/data/corpus/acl.json`**: la nota in testa (`acl.json:2`) dice che cosa è la matrice e che cosa non è. I ruoli sono a `:4-9` e il profilo `restricted-only` a `:11-16`, con l'obiettivo di selettività `0.05` e il rimando a S-03 per la misura.

### Flusso dei dati

`manifest.json` è la fonte di verità. `fetch.sh` lo legge, ricostruisce `raw/` (ignorato da git, `spikes/.gitignore:5`) e verifica i file versionati. `build.sh` lo legge per sapere che cosa deve produrre. I consumatori (S-02 parsing, S-03 pgvector e filtro ACL, S-04 JEV, A-04) leggono il manifest, i file in `raw/` e `synthetic/` e `acl.json`, senza modificarli. La visibilità attesa di un documento per un ruolo si calcola da `acl.json` più la `classification` del manifest, mai dalla tabella che implementa il filtro (`CORPUS.md:105`). Così un errore del filtro non viene nascosto da un oracolo che ripete lo stesso errore.

## Perché

- **Banca d'Italia come unica fonte italiana.** Dichiara il permesso di riproduzione per uso didattico non commerciale e pubblica a URL stabili. Una società quotata avrebbe dato bilanci più tipici, ma con condizioni d'uso meno chiare. Il piano chiedeva «fonti istituzionali dell'emittente», e questa lo è.
- **Niente SEC EDGAR.** L'accesso automatico richiede un User-Agent con un recapito personale, cioè un dato dell'utente in ogni richiesta. Il 10-K arriva dal PDF della relazione Berkshire pubblicato dall'emittente.
- **`qpdf --deterministic-id` invece di `qpdf` senza opzioni.** Senza l'opzione, l'identificativo nel trailer del PDF cambia a ogni esecuzione e lo sha del manifest non si potrebbe riprodurre. La prova è che il bridge, un giorno dopo e da zero, ha ottenuto lo stesso sha.
- **Manifest immutabile, anche per `build.sh`.** L'alternativa era lasciare che gli script aggiornassero gli hash. Ma allora un file cambiato verrebbe accettato senza che nessuno se ne accorga, e i risultati degli spike su date diverse non sarebbero più confrontabili. Una sostituzione è una voce nuova con il motivo scritto in `CORPUS.md`.
- **Auto-rilancio in sessione invece di fallire senza `CKA_RUN_ID`.** Ogni worktree deve poter lanciare `fetch.sh` come precondizione, senza sapere nulla di sessioni. `with` prenota la finestra di misura, aspetta fino a 900 secondi se un altro piano (per esempio 01-04) la tiene, e in uscita fa `down` e `release`. È la regola di 01-02: niente `--profile` e niente `docker compose run` diretti.
- **Radice del repository dalla posizione del file invece di `git -C`.** Il primo tentativo con `git -C` è fallito con exit 128 dentro `session.sh`, per la conversione dei percorsi spenta. Il fix è arrivato prima del commit.
- **«Cormorano» invece di «Aurora».** La trappola deve essere l'unica fonte della sua risposta. Il rapporto annuale del BIS cita un Project Aurora reale, e un leak test su «Aurora» avrebbe trovato risposte legittime nei documenti pubblici. Il bridge ha controllato: «Project Aurora» compare due volte nel BIS, mentre Cormorano, Valfonda e Gamma Immobiliare non compaiono in nessun PDF.
- **`fetch.sh` non legge il file dei segreti.** Il piano suggeriva `set -a; . "$CKA_SPIKES_ENV"`, ma i segreti servono solo a `session.sh`, che li passa con `--env-file`. Uno script che non li carica non può nemmeno stamparli per errore.
- **doc_id dei DOCX con il suffisso `-docx`.** I nomi dei file dati dal piano erano gli stessi dei sorgenti. Con doc_id uguali il manifest avrebbe avuto due voci con lo stesso identificativo, e fetch.sh le rifiuta (`fetch.sh:73`).
- **La matrice è una fixture di misura.** I permessi di prodotto per ruolo restano aperti (Q-01/Q-02). Lo dicono `acl.json` e `CORPUS.md`, così che nessuno la scambi per una decisione (RISK-10).
- **Legami.** Il corpus è il seam W11 della WIRING-MAP, con proprietario 01-03 e accesso in sola lettura per gli altri piani, tranne `oracles/` che appartiene a 01-06. Le decisioni del CONTEXT sono D-11 (licenze, nessun dataset di terzi qui), D-12 e D-15 (filtro ACL nella stessa query, selettività ≤ 5%), D-16 (solo dati pubblici o sintetici verso i provider) e D-20 (citazioni solo dai chunk_id forniti). Le minacce sono T-13 (leak), T-16 e T-17 (injection).

## Test

Il piano non contiene codice di test: produce dati, e i test del design (`01-03-TEST-DESIGN.md`) sono controlli eseguiti dalla radice del repository. Il bridge li ha rilanciati sul main:

- **Ricostruzione e verify del Task 1:** `bash spikes/data/corpus/fetch.sh`. Su `raw/` assente ha scaricato i 9 PDF e prodotto il derivato nel runner, uscendo con 0. Rilanciato subito dopo, non scarica nulla: dieci righe «già presente, sha256 ok» e nessun file nuovo (TEST-5).
- **File manomesso (TEST-4):** si altera un byte con `printf 'X' | dd of=spikes/data/corpus/raw/pub-en-02-federal-reserve-annual-report-2024.pdf bs=1 seek=100 conv=notrunc` e si rilancia `fetch.sh`. Il file viene scartato, riscaricato e alla fine coincide con il manifest.
- **DOCX (TEST-7, TEST-12):** `bash spikes/data/corpus/synthetic/build.sh` ricostruisce i quattro DOCX identici al manifest. `unzip -p spikes/data/corpus/synthetic/syn-01-budget-interno.docx word/document.xml` contiene gli stili `Heading1`/`Heading2` e un `w:br w:type="page"`.
- **Matrice, classificazioni, schema, provenienza, disclaimer (TEST-1, 2, 6, 9, 13):** controlli `node -e` su `acl.json` e `manifest.json` e lettura dei front matter. TEST-1 confronta la matrice con quella scritta nel piano, non con un'altra lettura dello stesso file.
- **Solo HTTPS (TEST-3):** `grep -c 'http://' spikes/data/corpus/fetch.sh spikes/data/corpus/manifest.json` dà 0 e 0.
- **Binari non versionati (TEST-8):** `git ls-files spikes/data/corpus | grep -Ei '\.pdf$'` è vuoto.
- **Injection e scanner (TEST-11):** `grep -l 'injection-case:' spikes/data/corpus/synthetic/*.md` elenca solo syn-07 e syn-08. `python skills/security-scanner/security_scan.py spikes/data/corpus` esce con 0.
- **Dati personali (TEST-10, parte automatica):** grep di codici fiscali, IBAN ed email sui sintetici, senza risultati.

## Deviazioni e punti aperti

- **Il primo fetch dentro la sessione falliva** per la radice calcolata con `git -C`. È stato corretto prima del commit, come descritto sopra.
- **Le affermazioni sul layout sono più caute del piano.** Le tabelle multipagina e le due colonne sono state viste su pagine campione, non su ogni pagina. `CORPUS.md:15` lo dichiara e rimanda la conferma a S-02.
- **Un container è partito fuori sessione.** Per contare le pagine dei PDF candidati l'esecutore ha lanciato un `docker run --rm --network none` del runner prima di usare `session.sh`, e senza l'etichetta `cka.run_id`. Non ha lasciato residui, ma è un'eccezione alla regola di 01-02.
- **Il ramo «download con sha errato → exit 1» non è provato.** Il codice c'è (`fetch.sh:121-132`), ma provarlo richiede un manifest alterato. Nel SUMMARY resta un giudizio umano.
- **La lettura umana dei nomi nei sintetici è da fare.** Il controllo automatico non trova codici fiscali, IBAN o email. Per escludere nomi di persone reali, però, i nove file vanno letti.
- **La selettività ≤ 5% del profilo `restricted-only` (RISK-11) non è misurabile qui**, perché i chunk non esistono ancora. La misura spetta a 01-14 (comando leak).
- **Contenuti duplicati, da sapere per chi misura:** l'estratto TTQ è identico alle prime 25 pagine di `pub-it-01`, e ogni DOCX ripete il testo del suo Markdown. Chi conta chunk o deduplica deve tenerne conto. I Markdown di syn-01, 02, 05 e 09 contengono una riga `\newpage` letterale.
- **Dipendenza dagli strumenti:** gli sha dei DOCX valgono per pandoc 3.11 con quella data fissa, e quello del TTQ per qpdf 12.2.0 nel runner. Se cambia l'immagine, servono voci nuove nel manifest.

**Modifica:** Claude (Anthropic) · 2026-10-21 · racconto del piano 01-03 dal bridge di execute
