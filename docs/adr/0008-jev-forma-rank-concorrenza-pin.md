# ADR-0008 — JEV: forma della chiamata `rank`, concorrenza, retry e pin di versione

- Stato: Accepted
- Data: 2026-10-21
- Decisori: l'utente (ratifica nel consolidamento, piano 01-17); la proposta è di Claude (gsd-executor 01-11), dallo spike S-04/A-09
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-11) · 2026-10-21 · ADR proposto con le misure dello spike S-04/A-09
**Modifica:** Claude (Anthropic) · 2026-10-26 · ratificato dall'utente nel piano 01-17 (ratifica-tutti)

## Contesto

ADR-0002 fissa il ruolo di JEV (typesafe.ai, `POST /v1/systemone`): dà solo segnali semantici (rerank, sufficienza e contraddizione delle evidenze, validazione claim–citazione), dietro una porta Rust e senza SDK. RBAC, provenance, mapping delle citazioni e deduplicazione restano deterministici, e un guasto di JEV non bypassa mai un controllo di sicurezza.

Restavano aperti quattro parametri dell'adapter:

- **la forma della chiamata `rank`** su 20 candidati (D-22). Lo `state` di una richiesta è uno solo, quindi le strade sono tre: una richiesta per candidato (A), uno `state` con tutti i passaggi e una domanda Noul per passaggio (B), una sola domanda Choice (C);
- **la concorrenza e il comportamento sui 429/529.** I limiti dell'organizzazione non sono pubblicati (RICERCA Area D §1.2);
- **alias o pin.** `jev-latest` è un alias mobile, e la documentazione raccomanda l'ID versionato quando si calibrano soglie (D-17, RICERCA Area D §1.4);
- **la varianza dei segnali**, che decide quanto margine serve attorno alle soglie.

Vincoli di privacy (RICERCA Area D §1.6): JEV è ospitato negli Stati Uniti; la retention è «as long as reasonably necessary», senza un termine in giorni; non c'è training sui prompt; la ZDR esiste solo per i contratti enterprise. Ogni passaggio inviato esce quindi dalla macchina con retention indefinita. Lo spike invia solo testo di documenti pubblici o sintetici del corpus (D-16), e solo passaggi visibili al ruolo della query.

## Opzioni considerate

Le misure vengono dallo spike S-04 su 30 query ratificate (10 IT, 10 EN, 10 cross-lingua), con 20 candidati ciascuna, 3 ripetizioni e soglie scritte prima della misura (`spikes/s04-jev/RISULTATI-jev.md`).

1. **Forma A: una richiesta per candidato**, `state = {query, passage}`, con le domande Noul `relevant`, `sufficient` e `instructs_model`.
   - Vantaggi:
     - nDCG@5 1,0000 e recall@8 1,0000: in ogni rank tutti i rilevanti superano tutti i distrattori;
     - è la forma più stabile (std per candidato: p95 0,0058);
     - lo stesso invio dà anche i segnali per l'evidence gate (`sufficient`) e per l'injection (`instructs_model`);
     - un passaggio per richiesta, quindi cancellazione e limiti per candidato.
   - Svantaggi:
     - 20 round-trip per query: p95 606 ms a concorrenza 16;
     - 8.449 token di input per query, circa 0,00035 USD.
2. **Forma B: uno `state` con 20 passaggi e 20 domande Noul.**
   - Vantaggi: una sola richiesta (p95 277 ms) e circa un quarto dei token.
   - Svantaggi:
     - nDCG@5 0,6790, la qualità peggiore delle tre;
     - varianza oltre soglia (p95 0,0503, massimo 0,3323);
     - ogni domanda vede tutti i passaggi: è il failure mode «Large state full of irrelevant detail» documentato da typesafe.
3. **Forma C: una Choice sui 20 passaggi.**
   - Vantaggi: una richiesta (p95 288 ms) e i token più bassi.
   - Svantaggi:
     - nDCG@5 0,7286;
     - le probabilità sommano a 1, quindi danno un ordine relativo e non una rilevanza assoluta: non servono all'evidence gate.
4. **Concorrenza 4, 8 o 16 richieste in volo.** 0 risposte 429 e 0 risposte 529 a tutti i livelli su 3 run da 200 richieste. Latenza per richiesta stabile (p95 fra 266 e 323 ms), throughput da circa 14 a circa 29 rps.
5. **Alias `jev-latest` o pin `jev-1.13.0`.**
   - Oggi `jev-latest` risolve a `jev-1.13.0`, ma è mobile: `jev-preview` risolve già a `jev-1.14.0`.
   - Il pin risolve a sé stesso in tutte le 3.803 risposte della sessione.

## Decisione

1. **Forma A** per `rank`: una richiesta per candidato con le domande Noul `relevant`, `sufficient` e `instructs_model`. Il wording è materiale di calibrazione, versionato, con il suo sha256 nei metadati.
2. **Concorrenza 16** richieste in volo per query, dentro la deadline globale (ADR-0010, variante A: `timeout_at` + `buffered`/`buffer_unordered`). È il massimo misurato senza 429, non un limite trovato. Il baseline di 8 di `interfaces-and-concurrency.md` resta un valore prudente ammesso.
3. **Retry** solo su 429 e 529, al massimo uno per chiamata, con `retry-after` rispettato (0,5 s se manca) e solo se l'attesa finisce prima della deadline. Oltre la deadline l'adapter restituisce un errore tipizzato (`RateLimited` o `Timeout`), mai un segnale di default.
4. **Pin `jev-1.13.0`** da configurazione (`JEV_MODEL`), senza default implicito. La versione risolta (`response.model`) e l'`usage` vanno nei metadati di ogni chiamata.
5. **Guardia di visibilità nell'adapter:** l'insieme dei candidati si controlla contro l'ACL prima del primo invio (tutto o niente), e un riconteggio indipendente sul punto di invio deve restare a 0. È difesa in profondità sul filtro di retrieval, non un sostituto.
6. **Varianza come vincolo di calibrazione:** le soglie sui Noul si calibrano con un margine di almeno ±0,1 attorno al taglio. Nella forma A un candidato su 600 ha std 0,0954 (0,30-0,47).

## Conseguenze

- Positive:
  - **qualità:** il reranker JEV ordina perfettamente i 30 insiemi di prova (nDCG@5 1,0000);
  - **latenza:** 606 ms di p95 per 20 candidati, ben sotto il tetto di 2.000 ms;
  - **costo:** circa 0,00035 USD per query;
  - **T-14:** nessun passaggio non visibile al ruolo è mai partito (`unauthorized_sent` = 0 su ogni riga di `forms.csv`), e la guardia sta nel percorso di invio (test con server finto: 0 richieste ricevute);
  - **T-24:** con 16 richieste in volo nessun 429. Il tetto di spesa per sessione e il retry dentro la deadline limitano costo e durata;
  - **D-17:** il pin rende riproducibile la calibrazione. Ogni cambio di versione è deliberato e rilancia la calibrazione delle soglie.
- Negative:
  - 20 richieste per query rendono l'adapter sensibile al rate limit dell'organizzazione, che non è pubblicato: con più utenti contemporanei 16 × N richieste possono superarlo;
  - la forma A costa circa 6 volte i token della C. Oggi è trascurabile, ma va rivisto se il prezzo cambia;
  - la misura di concorrenza tiene in volo al massimo 16 richieste di una sola query; un carico con query concorrenti non è stato misurato;
  - `instructs_model` è misurato solo come tasso di falsi positivi (0 su 1.800 invii, massimo 0,20). Gli oracoli ratificati contengono i fatti dei documenti di injection, non le istruzioni ostili, quindi il rilevamento sul testo ostile vero resta aperto.
- Invariante: nessun segnale JEV bypassa un controllo di sicurezza. Un errore JEV non diventa mai `relevant = true` né un punteggio di default.
- Follow-up:
  - 01-15 usa la forma A a concorrenza 16, con deadline 2 s, nel confronto con il reranker TEI e il no-rerank. Costruisce il client con `Client::new_from_env(Acl::load(corpus)?)`;
  - 01-12, oppure un oracolo che ratifichi gli excerpt ostili, misura il rilevamento di `instructs_model` sul testo di injection;
  - 01-16 trascrive in S-07 il trait `Reranker::rank` con deadline esplicita, metadati di versione ed errori tipizzati;
  - 01-17 porta la riga «Modello testuale» di `DEPENDENCIES.md` da `jev-latest` a `jev-1.13.0` (Decisa) e ratifica questo ADR;
  - prima del passaggio a `jev-1.14.0`: nuova misura di qualità e varianza, e ricalibrazione.

## Evidenze

- `spikes/s04-jev/RISULTATI-jev.md`: soglie, setup, risultati, anomalie, esito.
- `spikes/s04-jev/results/jev/hw.json`: manifest hardware.
- `spikes/s04-jev/results/jev/forms.csv`, `spikes/s04-jev/results/jev/concurrency.csv`, `spikes/s04-jev/results/jev/aliases.csv`.

| Misura | Valore | Soglia |
|---|---|---|
| nDCG@5 / recall@8, forma A | 1,0000 / 1,0000 | ≥ migliore fra B e C |
| nDCG@5 / recall@8, forma B | 0,6790 / 0,9500 | — |
| nDCG@5 / recall@8, forma C | 0,7286 / 0,9639 | — |
| p95 del rank di 20 candidati, forma A, concorrenza 16 | 606 ms | ≤ 2.000 ms |
| 429 su 3 run a 4 / 8 / 16 | 0 / 0 / 0 | 0 per il livello scelto |
| std del Noul `relevant` per candidato, forma A (media / p95 / max) | 0,0017 / 0,0058 / 0,0954 | ≤ 0,05 (1 candidato su 600 sopra: vincolo di calibrazione) |
| `jev-1.13.0` → `response.model` | `jev-1.13.0` (3.803 risposte su 3.803) | sé stesso |
| `jev-latest` / `jev-preview` → `response.model` | `jev-1.13.0` / `jev-1.14.0` | registrato |
| `unauthorized_sent` | 0 su 273 righe | 0 (hard gate) |
| Spesa della sessione | 0,078 USD | ≤ `S04_MAX_COST_USD` (5 USD) |
