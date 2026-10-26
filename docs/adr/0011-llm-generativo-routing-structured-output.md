# ADR-0011 — LLM generativo: routing per complessità, fallback e structured output con citazioni per claim

- Stato: Accepted
- Data: 2026-10-22
- Decisori: l'utente (scelta del modello per le query `simple`, D-33 del 2026-10-22, e ratifica nel consolidamento, piano 01-17); la proposta è di Claude (gsd-executor 01-12), dallo spike S-06/A-04
- Sostituisce: nessuno

**Modifica:** Claude (gsd-executor 01-12) · 2026-10-22 · ADR proposto con le misure dello spike S-06/A-04; scelta per le `simple` aperta perché la regola di routing di D-31 è tornata all'utente

## Contesto

Il RAG Service genera la risposta con un LLM via API, dietro la porta `AnswerGenerator`, e la fa validare a JEV (ADR-0002). 01-CONTEXT fissa i vincoli:

- **D-16:** nessun passaggio non visibile al ruolo della query entra nel prompt o va a JEV. Lo spike usa solo testo pubblico o sintetico del corpus.
- **D-18:** niente LLM locale. Claude Haiku 5.5 per le query semplici, Claude Sonnet 5.5 per le complesse; la complessità la decide JEV dentro `validate_query`.
- **D-19:** se JEV non risponde si genera con Sonnet e la risposta è marcata degradata, mai validata. Un guasto JEV non bypassa un controllo di sicurezza.
- **D-20:** l'LLM cita solo i `chunk_id` forniti; versione, pagina e locator li aggiunge il sistema; JEV valida claim–citazione; niente Citations native.
- **D-31:** matrice di misura ridotta (Sonnet solo su `complex` e `no_answer`, tetto di 2 USD) e regola scritta prima della misura. Haiku resta il modello delle `simple` solo se (a) sulle `complex` il suo tasso di claim supportati è entro 0,05 da quello di Sonnet e (b) sulle `simple` è ≥ 0,85. Altrimenti la scelta torna all'utente.

Vincoli del provider (RESEARCH §7 A-04): structured output con `output_config.format` json_schema e `additionalProperties: false` su ogni oggetto; vincoli come `minItems` e `minLength` non supportati; parametri di campionamento non ammessi (400); Citations native e prefill incompatibili con lo structured output (400); `stop_reason` da controllare (`refusal`, `max_tokens`) prima del parse. Retention Anthropic 30 giorni (RICERCA §3.2).

## Opzioni considerate

Le misure vengono da 30 query ratificate (13 `simple`, 12 `complex`, 5 `no_answer`), 3 ripetizioni con la prima scartata, contesto di 8 passaggi autorizzati (`spikes/s06-llm/RISULTATI.md`).

1. **Haiku 5.5 su tutte le query.**
   - Vantaggi: circa 0,0002 USD per risposta; JSON conforme 60/60; rifiuto 10/10 sulle `no_answer`; p95 2,61 s sulle `simple`.
   - Svantaggi: tasso di claim supportati 0,6852 sulle `complex` contro 0,7931 di Sonnet; p95 complessivo 3,74 s, sopra la soglia di 3 s; emette claim senza citazioni (11 su 83, 7 con testo vuoto).
2. **Sonnet 5.5 su tutte le query.**
   - Vantaggi: tasso di claim supportati più alto sulle `complex` (0,7931); 1 solo claim senza citazioni su 58; p95 3,99 s, sotto la soglia di 8 s.
   - Svantaggi: circa 0,0037-0,0044 USD per risposta, circa 20 volte Haiku. *Storia:* la prima stesura aggiungeva «non misurato sulle `simple` (D-31)»; la misura supplementare (D-32) l'ha misurato: 1,0000 (30/30) sulle `simple`.
3. **Routing per complessità con JEV (D-18): `simple` → Haiku, `complex` → Sonnet.**
   - Vantaggi: la Choice `{simple, complex}` di JEV concorda con l'etichetta ratificata su 25/30 query (0,8333, soglia 0,80), con p95 205 ms; il costo segue la difficoltà della query.
   - Svantaggi: la regola di D-31 non conferma Haiku sulle `simple`: (a) 0,6852 contro 0,7931 − 0,05 = 0,7431, falsa; (b) 0,7241 contro 0,85, falsa.
4. **LLM locale.** Escluso da D-18.
5. **Citations native della Messages API.** Escluse: incompatibili con lo structured output (400), e D-20 vuole il mapping deterministico degli id.

## Decisione

1. **Sonnet 5.5 è il modello delle query `complex`** e il modello di fallback quando JEV non risponde.
2. **Sonnet 5.5 è anche il modello delle query `simple`** (decisione utente del 2026-10-22, D-33 di `01-CONTEXT.md`): sulle stesse 13 query `simple` Sonnet ha un tasso di claim supportati di 1,0000 (30/30) contro 0,7241 (21/29) di Haiku, 0,8750 per Haiku anche scartando i claim senza citazioni; il costo è circa 0,0037 USD per risposta contro 0,0002 (≈ 19,5×), basso in assoluto; p95 4,48 s contro 2,61 s, entro la soglia di Sonnet (8 s). Haiku resta un'opzione futura, da riaprire solo con una misura nuova.
   - *Storia:* la prima stesura dell'ADR lasciava aperta la scelta, perché la regola di D-31 era fallita su entrambe le condizioni e Sonnet sulle `simple` non era misurato; la misura supplementare (D-32) ha dato il confronto diretto.
3. **Routing:** oggi il modello è unico, quindi la complessità non cambia il modello; `DecisionEngine::validate_query` restituisce comunque `complexity: Simple | Complex` da una Choice JEV con la regola di `oracles/ORACOLI.md`; a probabilità pari vale `complex`; un errore JEV non produce una complessità e si genera con Sonnet (D-19).
4. **Fallback (D-19):** senza JEV la risposta è `degraded`, o `refused` se il modello rifiuta o non resta alcun claim, e mai `validated`. `validated` richiede la conferma JEV (Noul ≥ 0,5) di ogni claim conservato.
5. **Forma della risposta (D-20):** `StructuredAnswer { answer, claims: [{ text, citations: [PassageRef] }], refusal }` da structured output json_schema con `effort: "low"`, senza parametri di campionamento, Citations native o prefill. Id brevi `c01`…`c08` nel prompt, mappati in codice; un claim con anche un solo id ignoto si scarta per intero; `doc_key` e locator vengono dal contesto, mai dal modello. I passaggi stanno nel messaggio utente come dati delimitati, con i delimitatori neutralizzati; le istruzioni stanno solo nel `system`.

## Conseguenze

- Positive:
  - JSON conforme allo schema in 94 risposte su 94, nessun `refusal` né `max_tokens` del modello;
  - nessun claim con id non fornito (0 su 141) e mapping deterministico a prova di modello (T-17);
  - 0 istruzioni seguite, 0 URL, 0 immagini su 32 risposte con i quattro tipi di injection indiretta di `syn-07` e `syn-08` (T-16);
  - `unauthorized_in_prompt` = 0 su ogni risposta, con guardia anche su ogni invio a JEV (T-14);
  - fallback: 15/15 generati con Sonnet e `degraded`, 0 `validated`;
  - costo: 0,4482 USD per l'intera matrice (T-24).
- Negative:
  - con un solo modello il costo per risposta è circa 20 volte quello di Haiku anche sulle domande semplici (in assoluto ≈ 0,004 USD); la latenza p95 sulle `simple` sale da 2,61 a 4,48 s;
  - lo structured output non impone citazioni non vuote, quindi lo scarto dei claim senza citazioni o con testo vuoto va fatto in codice;
  - la metrica «valore atteso trovato» (Haiku 14/50, Sonnet 2/24) è poco informativa con il confronto stretto, perché i modelli riformulano e traducono i valori;
  - il routing per complessità misurato (3 `simple` su 18 a Sonnet, 2 `complex` su 12 a Haiku) oggi non cambia il modello, quindi i suoi errori non costano fedeltà; conta solo se il routing di modello si riapre.
- Follow-up:
  - 01-17 porta la riga «Generative LLM» di `DEPENDENCIES.md` da Aperta a Decisa (Sonnet 5.5, D-33) e ratifica questo ADR; *Storia:* la prima stesura diceva «l'utente sceglie il modello delle `simple`», scelta fatta con D-33;
  - Haiku sulle `simple` si riapre solo con una misura nuova, che includa lo scarto deterministico dei claim senza citazioni;
  - 01-16 trascrive in S-07 `AnswerGenerator::generate` → `StructuredAnswer`, `DecisionEngine::validate_query` → `QueryDecision { complexity }` e la validazione claim–citazione come operazione separata, con errori tipizzati;
  - ogni risposta registra la versione del modello risolta (`response.model`) e l'`usage`;
  - una richiesta di warm-up per modello all'avvio (compilazione dello schema, cache di 24 ore);
  - la validazione lato Rust dei vincoli che la API non supporta (chiavi esatte, tipi, claim senza citazioni) resta nel servizio.

## Evidenze

- [`spikes/s06-llm/RISULTATI.md`](../../spikes/s06-llm/RISULTATI.md): soglie scritte prima della misura, setup, risultati, anomalie, esito.
- [`spikes/s06-llm/results/hw.json`](../../spikes/s06-llm/results/hw.json): manifest hardware della sessione `01-12-20261022t111515z-5662`.
- [`spikes/s06-llm/results/llm.csv`](../../spikes/s06-llm/results/llm.csv), [`routing.csv`](../../spikes/s06-llm/results/routing.csv), [`fallback.csv`](../../spikes/s06-llm/results/fallback.csv), [`injection.csv`](../../spikes/s06-llm/results/injection.csv).
- [`spikes/s06-llm/schema/answer.schema.json`](../../spikes/s06-llm/schema/answer.schema.json): schema della risposta.
- [`spikes/data/s06/selection.json`](../../spikes/data/s06/selection.json): selezione a seme delle 30 query e dei 15 `fallback_ids`.
- [`spikes/s06-llm/results/llm-simple-sonnet.csv`](../../spikes/s06-llm/results/llm-simple-sonnet.csv) e la sezione «Misura supplementare (D-32)» di [`RISULTATI.md`](../../spikes/s06-llm/RISULTATI.md): Sonnet sulle stesse 13 query `simple` di Haiku, chiesta dall'utente con D-32 prima della scelta per le `simple`. Tasso di claim supportati Haiku 0,7241 (21/29; 0,8750 senza i claim non citati, dato diagnostico) contro Sonnet 1,0000 (30/30); p95 2,61 s contro 4,48 s; 0,000191 contro 0,003720 USD per risposta; spesa della sessione 0,5980 USD su 2. Su questi numeri l'utente ha scelto Sonnet anche sulle `simple` (D-33).

**Modifica:** Claude (gsd-executor 01-12) · 2026-10-22 · Evidenze: misura supplementare di Sonnet sulle `simple` (D-32); Stato e Decisione invariati
**Modifica:** Claude (Anthropic) · 2026-10-22 · Decisione, Opzioni, Conseguenze, Follow-up ed Evidenze allineati a D-33 (Sonnet anche sulle `simple`); Stato invariato, `Proposed` fino alla ratifica di 01-17
**Modifica:** Claude (Anthropic) · 2026-10-26 · ratificato dall'utente nel piano 01-17 (ratifica-tutti)
