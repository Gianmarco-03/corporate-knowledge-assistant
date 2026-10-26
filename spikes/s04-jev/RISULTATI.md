# S-04 — JEV rank, reranker baseline e budget VRAM · RISULTATI (indice)

**Modifica:** Claude (gsd-executor 01-15) · 2026-10-26 · indice di S-04 con l'esito complessivo delle due parti (piani 01-11 e 01-15)
**Modifica:** Claude (gsd-executor 01-15, ripresa) · 2026-10-26 · budget VRAM e T-24 dalla serie VRAM rimisurata (sessione 01-15-20261026t100823z-f6a0)

Lo spike S-04 (assunzioni A-09 e A-06, criterio 4 della fase) ha due parti. Ciascuna ha soglie scritte prima della misura, setup, risultati e anomalie:

| Parte | Piano | Documento | Hardware |
|---|---|---|---|
| Forma della chiamata `rank` a JEV, concorrenza, pin e alias | 01-11 | [`RISULTATI-jev.md`](RISULTATI-jev.md) | [`results/jev/hw.json`](results/jev/hw.json) |
| Reranker baseline (no-rerank, bge-reranker-v2-m3 su TEI, JEV) e convivenza in VRAM | 01-15 | [`RISULTATI-rerank.md`](RISULTATI-rerank.md) | [`results/rerank/hw.json`](results/rerank/hw.json) |

## Esito

- **JEV per il rank (ADR-0008, dalla parte di 01-11):**
  - forma A, una richiesta per candidato con le domande Noul `relevant`, `sufficient` e `instructs_model`;
  - concorrenza 16, retry solo su 429/529 entro la deadline;
  - pin `jev-1.13.0`.

  Sugli insiemi di 01-11 la forma A dà nDCG@5 1,0000 con p95 606 ms. Sui top-20 reali di S-03 (questa parte) dà nDCG@5 0,7649 e recall@8 0,7973 su 74 query, con p95 483 ms a deadline 2 s. Sulle 59 query con almeno un rilevante fra i candidati i valori sono 0,9594 e 1,0000. JEV resta il rank di prodotto (ADR-0002).
- **Baseline di reranking: no-rerank** (soglia R-1 di `RISULTATI-rerank.md`). bge-reranker-v2-m3 su TEI GPU guadagna 0,1561 di nDCG@5 (0,7338 contro 0,5777), ma il p95 del rerank di 20 candidati è 360 ms contro un tetto di 300 ms, e le due condizioni vanno soddisfatte insieme. Il reranker su CPU (TEI `cpu-1.9`) ha la stessa qualità ma una mediana di circa 20 s per query: non è un ripiego praticabile.
- **Differenza di JEV rispetto alla baseline:** +0,1872 di nDCG@5 e +0,0541 di recall@8 sul no-rerank, al costo di circa 0,00059 USD e 483 ms di p95 per query. Rispetto al reranker TEI su GPU (non adottato) la differenza è +0,0311 di nDCG@5.
- **Budget VRAM: rispettato con i default** (soglia R-3, [`results/rerank/vram.csv`](results/rerank/vram.csv)).
  - tei-embed (`TEI_MAX_BATCH_TOKENS` 16384), tei-rerank (`TEI_RERANK_MAX_BATCH_TOKENS` 8192) e docling GPU sono stati caricati insieme, sotto carico concorrente per 3 minuti. Il picco è di 5.562 MiB contro un tetto di 5.632, senza OOM, e sta circa 3,9 GiB sopra il desktop (1,6 GiB). La riduzione dei batch non è servita.
  - Il margine è di 70 MiB e dipende dalla quota del desktop di Windows, che sulla stessa macchina va da circa 1,44 a 2,80 GiB (1.479-2.869 MiB). Con i tre servizi il tetto regge se il desktop resta sotto circa 1,64 GiB (1.677 MiB).
  - Con la baseline no-rerank il reranker TEI non si installa nell'MVP: sulla GPU restano tei-embed e docling. La loro quota, stimata per attribuzione e non misurata in uno scenario a due servizi, è di circa 2,8 GiB.

## Impatto su porte (S-07)

- **`Reranker`, trait stretto del solo `rank`** (separato da `DecisionEngine` per ISP, ADR-0012). Lo implementano l'adapter JEV (forma A, fan-out a 16 con `timeout_at` globale) e, come implementazione alternativa, una baseline. Con l'esito di R-1 la baseline dell'MVP è l'adapter **identità**: restituisce l'ordine denso dell'`AuthorizedEvidence` ricevuta, con segnale nullo. Un adapter TEI bge-reranker resta possibile dietro lo stesso trait, senza cambiare la firma, se una rimisura rientra nel budget di latenza.
- Nessun `match` sul tipo di provider nei casi d'uso: il caso d'uso riceve un `dyn Reranker` (o un generico) dalla composizione. Il ripiego da JEV alla baseline è una politica della composizione (timeout o circuito aperto), non un ramo nel dominio.
- La porta restituisce **solo segnali** (`RankScore` per passaggio, ordine stabile a parità di punteggio). La versione risolta (`response.model` per JEV, `model_sha` di `/info` per TEI) e l'`usage` vanno nei metadati della chiamata.
- **Errori tipizzati:** `Unauthorized` (guardia), `Timeout`, `RateLimited`, `Http` e `InvalidResponse`. L'ultimo copre per TEI una risposta incompleta, un indice ripetuto o fuori intervallo. Nessun errore diventa un segnale né un ordine inventato.
- La guardia di visibilità è nella porta per tutte le implementazioni, anche l'identità: insieme intero controllato prima dell'invio e riconteggio sul punto di invio.

## Impatto threat model

- **T-14** (chunk non autorizzati verso un provider, qui reranker TEI locale e JEV remoto): `assert_visible` sull'intero insieme e sul `role_profile` della query prima di qualunque invio, per tutti i metodi. In più c'è un riconteggio indipendente sul punto di invio: nel bin per TEI, nel client di 01-11 per JEV. Evidenze:
  - `unauthorized_sent` = 0 su tutte le 887 righe di [`results/rerank/rerank.csv`](results/rerank/rerank.csv), sui 387 riordini TEI dei carichi VRAM e su tutte le righe di [`results/jev/forms.csv`](results/jev/forms.csv);
  - i test `rerank_blocks_invisible_candidate_before_tei`, `rerank_blocks_invisible_candidate_before_jev` e `rerank_none_also_refuses_invisible_set` (server finti su loopback: 0 richieste ricevute con un passaggio non visibile).
- **T-24** (consumo di risorse e costo):
  - JEV: 0 risposte 429 a 16 in volo (01-11); spesa di questa parte 0,130760 USD per 4.437 richieste, con tetto `S04_MAX_COST_USD`;
  - VRAM: con i tre servizi sotto carico il picco è di 5.562 MiB, senza OOM. Sotto contesa il reranker TEI sale a 1.045 ms di p95. Il rischio residuo è la quota variabile del desktop: con il desktop a 2,8 GiB una serie scartata (vedi RISULTATI-rerank, «Anomalie e deviazioni») ha superato il tetto. Il budget dell'MVP senza reranker TEI lascia un margine stimato di circa 1,2 GiB con il desktop a 1,6 GiB.
- **T-16** (prompt injection indiretta): invariato rispetto a RISULTATI-jev. Il rilevamento di `instructs_model` sul testo ostile resta aperto per 01-12.
- **Integrità dei candidati (T-01-15-02):** il file dei candidati si usa solo con lo sha256 del manifest di 01-10, oppure con `--accept-sha` uguale allo sha calcolato. In questa misura gli sha coincidono.

## Destinazione

- [ADR-0008](../../docs/adr/0008-jev-forma-rank-concorrenza-pin.md) (forma, concorrenza, pin di JEV) e [ADR-0009](../../docs/adr/0009-reranker-baseline.md) (baseline di reranking e budget VRAM), entrambi *Proposed*, ratifica nel consolidamento (piano 01-17).
- Riga di `DEPENDENCIES.md` «Reranker baseline»: da Aperta a Decisa, «no-rerank (adapter identità); bge-reranker-v2-m3 su TEI misurato e non adottato per latenza; JEV forma A resta il rank». La scrive il consolidamento (piano 01-17), non questo piano.
- Riga «Modello testuale» (`jev-1.13.0`, da ADR-0008): consolidamento.

**Modifica:** Claude (Anthropic) · 2026-10-26 · quota del desktop corretta in 1,44-2,80 GiB (1.479-2.869 MiB), segnalata dal bridge di 01-15
