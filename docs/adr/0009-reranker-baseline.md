# ADR-0009 — Reranker baseline e budget VRAM della GPU locale

- Stato: Proposed
- Data: 2026-10-25
- Decisori: l'utente (ratifica nel consolidamento, piano 01-17); la proposta è di Claude (gsd-executor 01-15), dallo spike S-04/A-06
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-15) · 2026-10-25 · ADR proposto con le misure dello spike S-04/A-06
**Modifica:** Claude (gsd-executor 01-15, ripresa) · 2026-10-26 · budget VRAM dalla serie rimisurata in sessione tracciata

## Contesto

ADR-0002 assegna il reranking a JEV, che resta il rank di prodotto (LOCKED). La forma della chiamata, la concorrenza e il pin sono in ADR-0008: forma A, 16 richieste in volo, `jev-1.13.0`. Serve comunque una baseline, per due motivi:

- **confronto nell'evaluation:** quanto aggiunge JEV rispetto a un ordine senza JEV;
- **ripiego:** cosa usare quando JEV non risponde entro la deadline o il circuito è aperto. La baseline non sostituisce mai un controllo di sicurezza.

D-21 fissa i candidati: nessun riordino (l'ordine denso di S-03) e `BAAI/bge-reranker-v2-m3` (Apache-2.0, multilingue) su GPU in un container TEI. `jina-reranker-v2-base-multilingual` è escluso perché la sua licenza (CC-BY-NC-4.0) vieta l'uso commerciale.

La GPU locale è una RTX 3050 da 6 GB, condivisa con il desktop di Windows. Deve ospitare TEI embedding (bge-m3, ADR-0007), l'eventuale TEI reranker e Docling GPU per le pagine con tabelle (ADR-0004). Il budget VRAM vincola quindi anche la scelta del reranker.

Regola no-leak (D-16): nessun candidato non visibile al ruolo della query raggiunge il reranker, locale o remoto.

## Opzioni considerate

Misure dello spike sulle 74 query ratificate di S-03, con i loro top-20 autorizzati (bge-m3, ricerca esatta), 3 ripetizioni di cui la prima scartata e soglie scritte prima della misura (`spikes/s04-jev/RISULTATI-rerank.md`).

1. **Nessun riordino (no-rerank):** l'ordine denso di S-03, adapter identità dietro la porta `Reranker`.
   - Vantaggi:
     - latenza nulla, nessuna VRAM, nessun costo, deterministico;
     - nessun passaggio esce dal processo: la superficie T-14 si riduce alla guardia della porta.
   - Svantaggi: è la qualità più bassa, con nDCG@5 0,5777 e recall@8 0,7432 su 74 query. Sulle 59 query con almeno un rilevante fra i candidati i valori sono 0,7246 e 0,9322.
2. **bge-reranker-v2-m3 su GPU in TEI** (`86-1.9.4`, revisione dei pesi fissata):
   - Vantaggi:
     - nDCG@5 0,7338 (+0,1561 sul no-rerank) e recall@8 0,7838;
     - migliora 26 query, ne peggiora 3, ne lascia uguali 45;
     - deterministico fra le ripetizioni;
     - licenza Apache-2.0, nessun dato fuori dalla macchina.
   - Svantaggi:
     - il p95 del rerank di 20 candidati è 360 ms (p99 432, massimo 435), sopra il tetto di 300 ms della soglia R-1 scritta prima della misura;
     - occupa VRAM accanto a TEI embedding e a Docling: circa 1.479 MiB attribuiti con `TEI_RERANK_MAX_BATCH_TOKENS` 8192. Con i tre servizi sotto carico il picco totale è di 5.562 MiB su un tetto di 5.632, con un margine di 70 MiB e il desktop a 1,6 GiB;
     - sotto contesa con embedding e Docling il suo p95 sale a 1.045 ms.
3. **bge-reranker-v2-m3 su CPU in TEI** (`cpu-1.9`, ripiego senza GPU):
   - Vantaggi: stessa qualità della GPU (nDCG@5 0,7319), nessuna VRAM.
   - Svantaggi: mediana di 20,3 s e p95 di 32,0 s per query, con un timeout a 60 s. È fuori da qualunque budget interattivo.
4. **JEV forma A** (ADR-0008, per confronto; non è un'opzione di baseline):
   - nDCG@5 0,7649 e recall@8 0,7973, p95 483 ms a deadline 2 s;
   - circa 0,00059 USD per query (0,130760 USD per 222 rank);
   - sulle 59 query con rilevanti recall@8 è 1,0000.

   Resta il rank di prodotto (ADR-0002).
5. **jina-reranker-v2-base-multilingual:** escluso senza misura per la licenza CC-BY-NC-4.0 (D-21).

## Decisione

- **La baseline di reranking dell'MVP è no-rerank:** un adapter identità della porta `Reranker`, che restituisce l'ordine denso dell'`AuthorizedEvidence` ricevuta. Si applica la regola R-1 scritta prima della misura. bge-reranker-v2-m3 su GPU supera il margine di qualità (+0,1561 contro +0,02 richiesti), ma non il tetto di latenza (p95 360 ms contro 300 ms). Le due condizioni erano congiunte.
- **JEV forma A resta il rank di prodotto** (ADR-0002, ADR-0008). Il suo valore rispetto alla baseline, da usare nell'evaluation, è +0,1872 di nDCG@5 e +0,0541 di recall@8.
- **Ripiego:** quando JEV non risponde entro la deadline o il circuito è aperto, la composizione usa l'adapter identità. La guardia di visibilità e i controlli deterministici valgono anche sul ripiego.
- **bge-reranker-v2-m3 su TEI non si installa nell'MVP.** Resta un'implementazione possibile della stessa porta, se una misura futura rientra nei 300 ms di p95, per esempio con meno candidati o con un'altra GPU. Non è un'opzione su CPU.
- **Budget VRAM:** 5,5 GiB totali (5.632 MiB), desktop di Windows compreso (soglia R-3).
  - Con i default di compose (`TEI_MAX_BATCH_TOKENS` 16384, `TEI_RERANK_MAX_BATCH_TOKENS` 8192), tei-embed, tei-rerank e Docling GPU sotto carico concorrente arrivano a 5.562 MiB senza OOM, circa 3,9 GiB sopra il desktop. La soglia è rispettata e la riduzione dei batch non serve.
  - Nell'MVP la GPU ospita tei-embed (`TEI_MAX_BATCH_TOKENS` 16384) e Docling GPU, senza TEI reranker. La loro quota, stimata per attribuzione, è di circa 2,8 GiB.

## Conseguenze

- **Positive:**
  - nessun servizio TEI reranker da gestire, nessuna VRAM riservata al reranker e nessuna dipendenza di latenza in più nel flusso di query;
  - il ripiego è deterministico e senza costi;
  - la superficie T-14 del reranker locale scompare.
- **Negative:**
  - quando JEV non risponde, la qualità del rank scende al livello del no-rerank (−0,1872 di nDCG@5), invece che a quello del reranker locale (−0,0311);
  - l'evidence gate e il rifiuto devono tenerne conto: un ripiego si registra nell'audit come rank degradato.
- **Porta `Reranker` (S-07):**
  - trait stretto del solo `rank`, separato da `DecisionEngine` per ISP;
  - lo implementano l'adapter JEV e l'adapter identità;
  - restituisce solo segnali, con errori tipizzati (`Unauthorized`, `Timeout`, `RateLimited`, `Http`, `InvalidResponse`);
  - la guardia di visibilità è nella porta, per tutte le implementazioni.
- **VRAM e parametri di batch:**
  - senza TEI reranker restano liberi circa 1,5 GiB: con tei-embed e Docling il margine stimato è di circa 1,2 GiB con il desktop a 1,6 GiB;
  - il budget dipende dalla quota del desktop di Windows, che sulla stessa macchina va da 1,5 a 2,9 GiB. Con il desktop a 2,8 GiB e i tre servizi, una serie scartata ha superato il tetto (5.851 MiB). L'ambiente locale tiene quindi il desktop leggero, oppure misura con `vram.sh` prima di aggiungere un servizio GPU;
  - se si riattiva il TEI reranker accanto agli altri due, prima si riducono i max-batch-tokens e si rimisura con `spikes/s04-jev/vram.sh`. Le configurazioni ridotte non sono state misurate.
- **Da rivedere se:**
  - cambia la GPU, o il numero di candidati da riordinare scende sotto 20;
  - JEV diventa indisponibile per periodi lunghi;
  - una versione TEI o del modello abbassa il p95 sotto 300 ms;
  - la quota del desktop supera stabilmente 1,65 GiB, oppure si aggiunge un servizio GPU.

  In questi casi si rimisura con `spikes/s04-jev/src/bin/rerank/` e con le stesse soglie.

## Evidenze

- `spikes/s04-jev/RISULTATI.md`: l'indice di S-04 con l'esito complessivo.
- `spikes/s04-jev/RISULTATI-rerank.md`: soglie scritte prima della misura, metodo, setup (pin dei pesi, sha dei candidati, controllo incrociato della rilevanza), risultati, anomalie ed esito parziale.
- `spikes/s04-jev/results/rerank/rerank.csv`: 887 righe per query, metodo e ripetizione.
- `spikes/s04-jev/results/rerank/vram.csv`: campioni di VRAM per scenario e fase.
- `spikes/s04-jev/results/rerank/hw.json`: macchina, immagini, revisioni dei pesi e `git_rev` del codice misurato.
- `spikes/s04-jev/results/jev/hw.json` e `spikes/s04-jev/RISULTATI-jev.md`: la parte JEV dello spike (ADR-0008).
