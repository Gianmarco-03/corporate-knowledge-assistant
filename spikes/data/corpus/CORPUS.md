# Corpus comune degli spike

Questo è il corpus su cui misurano S-02 (parsing), S-03 (pgvector e filtro ACL), S-04 (JEV) e A-04. Lo possiede il piano 01-03 (WIRING W11); per tutti gli altri piani è in sola lettura, tranne `oracles/`, che appartiene al piano 01-06.

Il corpus è fatto di due parti. La prima sono relazioni annuali e bilanci pubblici in italiano e in inglese, scaricati dalle fonti ufficiali degli emittenti: servono a misurare parsing, chunking e tempi su documenti veri, lunghi e con layout difficili. La seconda sono documenti sintetici di una società inventata, classificati da `public` a `restricted`: servono a rendere misurabile la regola no-leak senza mandare a JEV o all'LLM nessun dato riservato reale (D-16).

## Criteri di scelta dei documenti pubblici

Ogni documento pubblico copre almeno una delle difficoltà che gli spike devono misurare (01-RICERCA-DISCUSSIONE, «File di test»): lunghezza fra 100 e 300 pagine, tabelle numeriche dense, impaginazione a due colonne, grafici vettoriali. Le fonti sono soltanto siti istituzionali degli emittenti raggiungibili in HTTPS. Si è evitato SEC EDGAR perché chiede un User-Agent con un recapito personale, che non si vuole spedire da uno script versionato; la relazione di Berkshire Hathaway, scaricata dal sito dell'emittente, contiene comunque il Form 10-K integrale.

Per l'italiano le fonti sono tutte della Banca d'Italia. Non è un'emittente quotata, ma pubblica ogni anno sia una relazione sulla gestione con il proprio bilancio sia documenti di analisi con molte tabelle e grafici, e dichiara in copertina che la riproduzione per fini didattici e non commerciali è consentita citando la fonte. Si è preferita una fonte con un permesso di riproduzione esplicito e URL stabili per anno, anche se meno varia di un campione di società quotate.

## Documenti

La tabella dice per quale misura è stato scelto ogni documento. Le caratteristiche di layout (due colonne, grafici, densità delle tabelle) sono state viste su pagine campione renderizzate nel runner, non su ogni pagina: la presenza di tabelle che proseguono su più pagine non è stata verificata qui e la conferma spetta a S-02.

| doc_id | Pagine | A quale misura serve |
|---|---|---|
| `pub-it-01-bdi-relazione-annuale-2025` | 228 | Documento lungo con tabelle, riquadri, grafici e note a piè di pagina; è anche la sorgente dell'estratto TTQ. |
| `pub-it-02-bdi-bilancio-esercizio-2025` | 105 | Bilancio con stato patrimoniale, conto economico e nota integrativa: tabelle numeriche dense. |
| `pub-it-03-bdi-sintesi-relazione-gestione-2025` | 36 | Brochure impaginata a due colonne con riquadri e immagini: ordine di lettura. |
| `pub-it-04-bdi-bollettino-economico-2026-3` | 42 | Molti grafici vettoriali con didascalie e note: separazione fra testo e figura. |
| `pub-it-05-bdi-relazione-gestione-sostenibilita-2025` | 212 | Relazione annuale dell'emittente sulla gestione, lunga, con il bilancio e indicatori di sostenibilità. |
| `pub-en-01-berkshire-annual-report-2025` | 154 | Relazione annuale societaria con il Form 10-K: tabelle finanziarie in inglese. |
| `pub-en-02-federal-reserve-annual-report-2024` | 214 | Documento lungo in inglese con molte tabelle statistiche. |
| `pub-en-03-bank-of-england-annual-report-2025` | 220 | Annual Report and Accounts: bilancio in inglese con note e tabelle. |
| `pub-en-04-bis-annual-economic-report-2025` | 126 | Rapporto economico con grafici vettoriali e riquadri in inglese. |
| `der-it-01-relazione-annuale-2025-p1-25` | 25 | Estratto delle prime 25 pagine (1,99 MB) della relazione annuale: è il documento di riferimento del target TTQ del README (25 pagine, ≤ 10 MB, p95 ≤ 15 s). |

URL, sha256, dimensione e numero di pagine di ogni documento stanno in `manifest.json`. Il numero di pagine è quello di `qpdf --show-npages` nel runner.

## Licenze

I documenti pubblici restano di proprietà dei rispettivi emittenti. Nel manifest hanno la licenza «documento pubblico, copyright dell'emittente; uso di misura non commerciale, non ridistribuito». Per questo il repository non contiene i PDF: versiona soltanto URL, sha256 e metadati, mentre i file stanno in `raw/`, che `spikes/.gitignore` esclude (`data/**/raw/`). Lo stesso vale per l'estratto TTQ, che è un'opera derivata. Nessun dataset di terzi è in questa cartella: TAT-DQA, DocLayNet e FinanceBench li gestisce S-02 in `spikes/data/s02/` (D-11).

## Rigenerazione

Da una copia pulita del repository basta lanciare dalla radice:

```bash
bash spikes/data/corpus/fetch.sh
```

Lo script legge il manifest, scarica ogni documento pubblico solo in HTTPS (`curl --proto '=https' --max-redirs 3 --max-time 300 --max-filesize 104857600`, tre tentativi), verifica lo sha256 e scarta il file se non coincide, uscendo con codice 1. Un file già presente con lo sha256 giusto non si riscarica; uno presente con lo sha256 sbagliato si scarta e si riscarica. L'estratto TTQ lo produce `qpdf --deterministic-id <relazione> --pages . 1-25 -- <estratto>` nel servizio `runner`, attraverso `spikes/tools/session.sh`: con `oneshot "$CKA_RUN_ID"` se chi chiama è già in una sessione prenotata e ha esportato `CKA_RUN_ID`, altrimenti dentro `session.sh with 01-03`, che prenota la finestra di misura e alla fine fa `down` e `release`. Se l'estratto c'è già con lo sha256 giusto, nessun container parte. `--deterministic-id` serve perché senza di esso qpdf scrive nel trailer un identificativo diverso a ogni esecuzione e lo sha256 cambierebbe; l'hash del manifest è stato ottenuto con qpdf 12.2.0, la versione dell'immagine `runner` di 01-02. Il file segreti `$HOME/.cka-spikes/spikes.env` (variabile `CKA_SPIKES_ENV`) serve solo a `session.sh`: `fetch.sh` ne controlla la presenza quando deve avviare il runner e non lo legge.

Gli hash si sono fissati al primo download riuscito, il 2026-10-20, e da quel momento il manifest non cambia. Se un emittente sostituisce un file all'URL registrato, `fetch.sh` esce con 1: la correzione è una voce nuova, con un `doc_id` nuovo e il motivo scritto in questo file, mai un hash riscritto.

**Modifica:** Claude (gsd-executor, Anthropic) · 2026-10-20 · corpus pubblico IT/EN, estratto TTQ, manifest e fetch.sh (piano 01-03, Task 1)
