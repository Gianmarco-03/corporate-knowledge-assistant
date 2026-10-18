---
name: eval-oracle-check
description: Verifica che ogni valore atteso dell'evaluation corpus di CKA — risposta o fatto atteso, citazione attesa (documento, versione, pagina, span), rifiuto atteso, numero estratto, decisione JEV attesa, soglia di metrica — provenga da una fonte INDIPENDENTE dalla pipeline RAG (estratto del documento sorgente, derivazione documentata, decisione utente ratificata) e che il triplo guardrail (Claude, GPT double-check, utente) sia completo. USARE come gate sui PLAN in `plan`, quando si aggiunge o modifica una voce dell'evaluation corpus, un golden test `eval_oracle_*` o una soglia di evaluation, quando si dichiara fatta una fase che tocca retrieval, reranking, evidence gate, generazione o citazioni, o quando l'utente chiede "verifica l'oracolo", "da dove viene questa risposta attesa?", "golden test", "il gate eval è rosso, perché?".
---

# Eval Oracle Check — CKA

La domanda che questo gate pone a ogni valore atteso è una sola:

> **Da dove viene la risposta attesa, e chi la garantisce oltre a chi ha
> scritto la pipeline?**

Un golden test verde il cui atteso non ha risposta fa fallire il gate. La
copertura non si misura in test che passano, ma in attesi con provenienza
qualificante. Riferimenti: `docs/quality/llm-evaluation.md` (metriche:
recall@k, faithfulness, citation correctness, answer relevance, refusal
accuracy, leakage rate), `docs/data/corpus-strategy.md` (evaluation corpus
versionato e immutabile, separato dal knowledge corpus).

## Che cosa è un oracolo in CKA

| Tipo | Valore atteso | Tolleranza |
|---|---|---|
| citazione | `document_id` + `version` + `page` + span (offset di carattere sul testo della pagina) | documento, versione e pagina **esatti**; span: **intersezione non vuota** con lo span atteso |
| numero estratto | importo, percentuale, data, conteggio, come stringa decimale con unità/valuta | **esatto** (valore, scala, segno, unità); ammessa solo la normalizzazione di formato dichiarata nel manifest (separatori, locale) |
| fatto atteso | affermazione atomica con la citazione che la sostiene | gli elementi numerici e le entità esatti; il testo libero lo misurano le metriche, non l'oracolo |
| rifiuto atteso | la domanda non ha evidenza sufficiente o autorizzata nel corpus | esatto (rifiuta / risponde) |
| visibilità | per un ruolo, quali documenti non devono mai comparire | esatto: **leakage = 0** |
| decisione JEV | esito atteso di `assess_evidence`/`validate_citations` su un caso costruito | esatto sull'esito |
| soglia | `recall@k`, faithfulness, latenza, … | è una **decisione**, mai un oracolo derivato: classe `ratifica-utente` |

Una soglia più stretta dello «span con intersezione non vuota» (per esempio
una sovrapposizione minima in percentuale) è una decisione di prodotto: si
apre un TO-DO, non la sceglie l'agente.

## Procedura

### 1. Parti dal documento sorgente, mai dalla pipeline

Per ogni oracolo, prima del test, si risponde per iscritto: **quale
documento** (`document_id`, versione, file nel corpus di evaluation),
**quale punto esatto** (pagina, e lo span o la tabella/riga), **che cosa
dice** (estratto verbatim). Se il documento non determina il valore (frase
ambigua, due cifre in conflitto, unità implicita), non si sceglie «quello
ragionevole»: si apre un **TO-DO di dominio** con la domanda, l'assunzione
provvisoria e cosa ne dipende, e l'oracolo nasce di classe `ratifica-utente`.

### 2. Classifica la provenienza — tre classi qualificano, due mai

| Classe | Significato | Qualifica? |
|---|---|---|
| `estratto-sorgente` | il valore è scritto letteralmente nel documento, alla pagina citata | sempre |
| `derivazione-documentata` | il valore è derivato a mano dal documento (somma di voci, confronto fra due pagine, rifiuto per assenza di evidenza) con lo svolgimento scritto | solo con `derivation_ref` |
| `ratifica-utente` | il documento tace o è ambiguo, oppure è una soglia: decisione dell'utente | solo con `ratification_ref` |
| `hand-crafted` | scritto a mano senza fonte | mai |
| `generato-dalla-pipeline` | ottenuto eseguendo retrieval, JEV, LLM o qualunque parte di CKA | **mai**, con un finding dedicato |

**Perché la pipeline non può produrre l'oracolo.** Un atteso copiato
dall'output della pipeline confronta la pipeline con sé stessa: passa sempre,
anche quando la pipeline sbaglia, e continua a passare dopo ogni regressione.
Vale anche per «l'ho fatto girare e la risposta sembrava giusta». Lo stesso
per l'LLM usato come generatore di attesi: un LLM può proporre domande, mai
le risposte attese.

### 3. Scrivi la voce di manifest

Il manifest è `evaluation/oracles/manifest.json`, ancorato alla radice del
repo; i blocchi di derivazione sono `evaluation/oracles/<oracle_id>.md`.
Una voce per oracolo:

```json
{
  "oracle_id": "Q-0012-cit-1",
  "dataset_version": "eval-v1",
  "kind": "citazione",
  "provenance_class": "estratto-sorgente",
  "source_ref": {
    "document_id": "doc-annual-report-2023",
    "version": "1",
    "page": 47,
    "span": [1180, 1265],
    "excerpt": "Il risultato netto consolidato è pari a 12,4 milioni di euro"
  },
  "expected_value": "12.4 EUR milioni",
  "normalization": "separatore decimale virgola -> punto",
  "derivation_ref": null,
  "ratification_ref": null,
  "test_id": "eval_oracle_q0012_cit_1",
  "guardrail": { "claude": "concordo", "gpt": "CONFIRMED", "user": "ratified" },
  "todo_ref": null
}
```

Regole della voce: numeri sempre come **stringhe decimali**, mai numeri JSON;
`guardrail` ha sempre le tre chiavi, anche a `null`; `derivation_ref` e
`ratification_ref` obbligatori e non vuoti per le rispettive classi;
`excerpt` verbatim dal documento. L'evaluation corpus è **immutabile**: un
oracolo non si modifica per allinearlo alla pipeline, si corregge solo con
una nuova `dataset_version` e il motivo scritto.

### 4. Triplo guardrail — tre verdi, sempre

1. **Claude** deriva il valore dal documento con un **agente separato
   dall'implementatore** (mai l'esecutore della fase, mai chi ha scritto il
   prompt o il retrieval) e lo registra (`guardrail.claude`).
2. **GPT `double-check`, sempre** (AgentBridge, progetto
   `CorporateKnowledgeAssistant`, contratto in
   `C:\Agent Bridge\docs\contracts.md` §double-check): `WRITE-REQUEST` con
   `content_id` `doublecheck_N` e `recipient` diverso dal mittente; il
   markdown contiene **solo** l'estratto del documento sorgente (con
   documento, versione, pagina) e la derivazione, **mai** il codice, l'output
   della pipeline o il prompt di sistema. Il controller la consegna con
   `context_policy: CLEAN`. Solo `CONFIRMED` è verde; `PARTIALLY_CONFIRMED`,
   `REJECTED` e `UNDETERMINED` sono dissenso. La risposta si salva verbatim
   nel blocco di derivazione, attribuita a GPT con il `content_id`, poi
   `ACK-RESPONSE`.
3. **L'utente** ratifica guardando estratto e derivazione, non solo il
   valore. La ratifica non è mai deducibile dall'agente.

Un disaccordo qualsiasi apre un **TO-DO di dominio** e l'oracolo resta
pendente. **Mai allineare l'oracolo al verificatore** né al risultato della
pipeline: il dissenso è la prova che il terzo asse funziona. Un oracolo senza
`gpt: CONFIRMED` non qualifica: a differenza del progetto d'origine, qui il
double-check non è opzionale né rinviabile a fine fase.

### 5. Marca il golden test

Nome con prefisso fisso `eval_oracle_` e riferimento all'oracolo:

```rust
#[tokio::test]
// oracle: Q-0012-cit-1 (evaluation/oracles/manifest.json)
async fn eval_oracle_q0012_cit_1() {
    let answer = harness().ask("Q-0012").await;
    assert_citation(&answer, "doc-annual-report-2023", "1", 47, (1180, 1265));
}

#[tokio::test]
#[ignore = "pending: TODO-7 (oracle Q-0031-num-1, cifra ambigua p. 12)"]
async fn eval_oracle_q0031_num_1() { /* ... */ }
```

- Selezione: `cargo test --workspace eval_oracle_`. Il `test_id` della voce
  è il nome **enumerato dal comando**, da verificare con
  `cargo test --workspace -- --list | grep eval_oracle_`, non dedotto dalla
  firma: un filtro che non seleziona nulla è zero test e un falso verde.
- **Pendente quando c'è un TO-DO aperto**: un test ancorato a un oracolo con
  TO-DO aperto o guardrail incompleto porta
  `#[ignore = "pending: <TODO> (oracle <id>)"]` e risulta *ignored*, mai
  *passed*. Si toglie l'`ignore` solo con i tre verdi.
- I golden test girano con il fake deterministico del `DecisionEngine` o con
  JEV reale solo nelle suite di evaluation dichiarate; i test unitari non
  chiamano API reali.

### 6. Gate sul PLAN (in `plan`)

Si leggono frontmatter e `<tasks>` (`node scripts/estrai.cjs piani
<dir-fase>`). Un piano che aggiunge o cambia voci dell'evaluation corpus,
golden test o soglie è `conforme` solo se, per ogni oracolo: cita la classe
di provenienza e il riferimento (documento, versione, pagina; oppure
derivazione o ratifica); nessun task ricava un atteso eseguendo la pipeline
(«lancia la query e registra la risposta come attesa» è `da correggere`);
prevede i tre passi del guardrail con il double-check GPT; marca come
pendenti i test ancorati a TO-DO aperti; affida la derivazione a un agente
diverso dall'esecutore. Un piano che tocca retrieval, reranking, evidence gate,
generazione o citazioni senza oracoli dichiara perché (sentinella
`Nessun golden test: <motivo>`), e resta comunque soggetto alla regressione
sull'evaluation corpus in `verify` (confronto con la baseline in
`evaluation/results/`).

Se nella fase non c'è alcun oracolo dovuto, il gate scrive in `0X-GATE.md`
«non applicabile» con il motivo: un esito non eseguito non è mai verde e
non si confonde con un `conforme`.

### 7. Finding

Tutti CRITICAL: un oracolo o è indipendente o non lo è.

| `rule_id` | Significato | Come si chiude |
|---|---|---|
| `oracle-provenance-missing` | test senza voce nel manifest, o voce senza `source_ref` | aggiungi la voce; verifica il `test_id` enumerato |
| `oracle-pipeline-generated` | atteso ottenuto eseguendo CKA o un LLM | ri-derivalo dal documento o portalo a ratifica |
| `oracle-class-insufficient` | `hand-crafted`, o classe con riferimento vuoto | fonte reale, `derivation_ref` o `ratification_ref` |
| `oracle-guardrail-incomplete` | manca uno dei tre assi (anche GPT) | completa derivazione, double-check o ratifica; il test resta `ignore` |
| `oracle-guardrail-dissent` | un asse dissente | TO-DO di dominio; mai allineare l'oracolo |
| `oracle-manifest-invalid` | chiavi mancanti, numero non stringa, classe ignota, excerpt assente | correggi la voce |
| `oracle-test-unmarked` | test con atteso di evaluation senza prefisso `eval_oracle_` | rinomina; un test non selezionato è invisibile al gate |
| `oracle-pending-green` | test ancorato a TO-DO aperto che risulta *passed* | rimetti l'`ignore` con il riferimento al TO-DO |

### 8. Report

Prima di dichiarare fatta una fase che tocca la pipeline: oracoli
qualificanti su totale (`N/N`, altrimenti il lavoro non è finito); pendenti
con il loro TO-DO; dissensi con il TO-DO aperto; decisioni `ratifica-utente`
con data e riferimento; esito della regressione sull'evaluation corpus.
Verdetto PASS / FAIL. In UAT si mostra l'estratto del documento sorgente
accanto alla risposta e alla citazione dell'app, non solo il riepilogo del gate.

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
