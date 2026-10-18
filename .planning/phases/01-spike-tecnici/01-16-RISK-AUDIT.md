<!-- Risposta verbatim di GPT (OpenAI), content_id risk_audit_7, piano PLAN-010-16. Trascrizione: Claude (Anthropic), 2026-10-18. -->

# Audit dei rischi — PLAN-010-16 — Firme Rust candidate delle porte

**Modifica:** gpt · 2026-10-18 · risposta indipendente a risk_audit_7.

**Esito: REVISE.** La compilazione delle dodici porte è necessaria ma non verifica da sola le garanzie semantiche dichiarate: autorizzazione, scope cache, idempotenza e compatibilità degli embedding devono diventare contratti verificabili.

## Identità e revisione effettivamente letta

- Repository: Gianmarco-03/corporate-knowledge-assistant.
- Branch dichiarato: task/PLAN-010-16.
- base_sha = head_sha = 180d63713a6216f905ee3f51845bd0feeb9a23ce.
- [PLAN v1 esaminato](https://github.com/Gianmarco-03/corporate-knowledge-assistant/blob/180d63713a6216f905ee3f51845bd0feeb9a23ce/.planning/phases/01-spike-tecnici/01-16-PLAN.md).
- SHA-256 del PLAN ricalcolato sul contenuto UTF-8 recuperato: `5270a1aa953de4b3b8082bbb75de005499f6b1bfff03339e0ca35fca8e537fe9`; coincide esattamente con la richiesta.
- Il connettore GitHub è stato usato con lo SHA completo, senza sostituirlo con il default branch.

L'albero Git completo restituito non era troncato: allo SHA indicato **non esistono spikes/, Cargo.toml radice, codice o test di prodotto implementati**. Esistono documenti e scheletro con .gitkeep. L'assenza degli spike futuri è prevista: non è un finding. I rischi sotto riguardano l'esecuzione futura del PLAN e i contratti che promette, non difetti runtime di codice già scritto.

## Evidenza e limiti

Letti CONTEXT, RESEARCH, WIRING e GATE della fase, il PLAN in oggetto e i piani confinanti rilevanti. Ulteriori riscontri: regole correnti in CLAUDE.md e docs/architecture/interfaces-and-concurrency.md; stub di data-model/api-contract/threat-model; configurazione .gitignore e regole dei confini dei crate. Non ho avviato container, installato pacchetti, eseguito parser/file ostili o chiamato provider a pagamento. I test indicati sono verifiche richieste, **non test già passati**.

Provenienza:
- **repository**: presenza/assenza dei file, configurazione e regole attuali allo SHA sopra;
- **plan**: istruzioni e interfacce prescritte dai documenti citati in evidence, con righe del PLAN dove applicabile;
- **reasoning**: scenario condizionale e conseguenze riportati per ciascun rischio; non sono osservazioni di un incidente reale;
- **external_required**: questioni puntuali in external_research. Le etichette [V]/[NV] della ricerca di fase sono attribuzioni del documento: qui non diventano verifiche web nuove.

Il gate 01-GATE.md è stato letto, inclusi i finding già corretti. I suoi esiti non sostituiscono questa verifica indipendente dei failure mode. Le decisioni D-01..D-22 restano vincolanti; nessun rischio viene accettato per conto dell'utente.

## Copertura

Dyn-compatibilità e Send/Sync, confini di crate/tipi provider, split Reranker/DecisionEngine già corretto, mutazioni e retry, deadline e contesto, audit, cache/revoca, identità modello e locator. Non si chiede di implementare adapter reali o prodotto in S-07: servono specifiche e prove di contratto con fake, oltre ai test che ritornano sempre errore.

Database, migrazioni e dati legacy: nello stato corrente non c'è uno schema di prodotto eseguito né dati legacy documentati da preservare; dove il PLAN introduce persistenza, sono stati esaminati i contratti futuri e le finestre di failure. Non invento una migrazione o un rollback di produzione. Precisione: rilevanti byte, offset/coordinate, unità temporali e percentili secondo il piano; non c'è logica monetaria/decimal da auditare. Compatibilità e UX: valutate nei seam con i consumatori, nei codici/esiti e nelle citazioni, non in schermate di prodotto ancora assenti.

## Rischi e ricerche nel formato AgentBridge

Gli ID RISK e RESEARCH sono locali a risk_audit_7; non sommare finding correlati di audit diversi come incidenti distinti. evidence.files può includere percorsi futuri prescritti dal PLAN: non afferma che tali file esistano già. I numeri di riga in evidence.details appartengono al documento citato come primo file.

```yaml
reviewed:
  plan_version: 1
  plan_sha256: "5270a1aa953de4b3b8082bbb75de005499f6b1bfff03339e0ca35fca8e537fe9"
  git_head_sha: "180d63713a6216f905ee3f51845bd0feeb9a23ce"
verdict: REVISE
summary: {"critical":0,"high":1,"medium":3,"low":0,"info":0}
risks:
  - id: "RISK-001"
    severity: "HIGH"
    likelihood: "MEDIUM"
    category: "authorization_contract"
    title: "La compilazione non dimostra che solo passaggi autorizzati attraversino le porte"
    scenario: "Il PLAN dichiara che le firme non consentono di passare contenuto non autorizzato a JEV/LLM; richiede un AuthorizationPredicate a search, ma non prescrive chi possa costruire candidati/PassageRef o un contenitore di evidenze autorizzate legato a soggetto, versione e decisione."
    trigger: "Un adapter o un chiamante costruisce direttamente un CandidateChunk/PassageRef e lo passa a Reranker o AnswerGenerator senza passare dalla ricerca autorizzata."
    failure: "Tutti gli Arc<dyn Trait> possono compilare e i fake che restituiscono errori passare i test, pur senza rendere verificabile l'invariante dichiarata. Un predicato arbitrariamente costruibile non è prova di autorizzazione."
    impact: "Le firme candidate trasferiscono alla Fondazione un confine che può consentire invio di contenuti non autorizzati al cloud, in contrasto con D-16."
    affected: ["spikes/s07-ports/src/lib.rs","docs/adr/0012-firme-porte-candidate.md","docs/architecture/interfaces-and-concurrency.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-16-PLAN.md","docs/architecture/interfaces-and-concurrency.md","CLAUDE.md"],"details":"Riga 105: le firme non consentono di passare a JEV/LLM/reranker un passaggio senza il predicato di autorizzazione già applicato a monte, T-14).  `docs/adr/0012-firme-porte-candidate.md` (Stato `Proposed`, template condiviso): Cont"}
    mitigation: "Definire nel disegno chi produce e valida il contesto autorizzato e quali identità/versioni copre; usare tipi/costruttori controllati oppure un contratto esplicito verificato all'adapter. Separare la prova di forma/dyn-compatibilità dalla prova di autorizzazione e includere i percorsi diretti nei test di contratto."
    test_hint: "Tentare di creare/chiamare una richiesta generativa con passaggio senza decisione valida o di altro soggetto/versione. Deve essere impossibile costruirla o deve essere rifiutata prima di qualunque invio esterno; includere fake con esito positivo."
    test_required: true
  - id: "RISK-002"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "cache_authorization"
    title: "CacheKey e invalidazione non hanno uno scope autorizzativo obbligatorio"
    scenario: "Cache viene ripresa dalle firme attuali con aggiustamenti di forma. Il piano non impone che chiave e scope comprendano soggetto/ruoli o versione ACL, documenti/versioni e modello rilevanti."
    trigger: "Due soggetti fanno la stessa query oppure un ruolo viene revocato mentre esiste una risposta in cache."
    failure: "Una futura implementazione può usare una chiave globalmente valida per query e restituire contenuto di uno scope precedente; la compilazione con fake di errore non lo scopre."
    impact: "Contratto candidato insufficiente a sostenere deny-by-default su cache e revoche, anche se il primo adapter è NoOpCache."
    affected: ["spikes/s07-ports/src/lib.rs","docs/adr/0012-firme-porte-candidate.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-16-PLAN.md","docs/architecture/interfaces-and-concurrency.md","CLAUDE.md"],"details":"Riga 78: `AccessPolicy`, `AuditSink`, `Cache`, `DocumentSource`, `Chunker` dalle firme attuali con i soli aggiustamenti di forma comuni (errori tipizzati, deadline). Sezione «Tipi destinati ai contratti condivisi»: `Locator`, `Co"}
    mitigation: "Dichiarare lo scope della cache e la regola di ricontrollo prima del consumo; rendere esplicite le dipendenze di invalidazione da ACL/versione/modello senza implementare Redis nello spike."
    test_hint: "Fake cache positivo: stessa query per manager e guest non deve restituire la stessa evidenza riservata; revoca ACL e cambio versione devono rendere inutilizzabile la voce precedente."
    test_required: true
  - id: "RISK-003"
    severity: "MEDIUM"
    likelihood: "HIGH"
    category: "idempotency_observability"
    title: "Deadline da sola non copre il contesto delle operazioni mutanti"
    scenario: "Le regole correnti impongono identificatore stabile/idempotency key, request_id e trace context; il PLAN elenca nuove firme e deadline ma non obbliga esplicitamente tali dati per publish_version, AuditSink.append e operazioni mutanti/cache."
    trigger: "Retry dopo timeout senza sapere se la prima chiamata sia stata applicata."
    failure: "Una firma può compilare con errore tipizzato e deadline, ma non consentire al chiamante di riconoscere lo stesso tentativo o correlare l'esito. Un errore Timeout non implica rollback."
    impact: "Duplicazioni di audit/pubblicazione o retry ambiguo nei provider futuri; diagnosi degli spike non trasferibile al prodotto."
    affected: ["spikes/s07-ports/src/lib.rs","docs/adr/0012-firme-porte-candidate.md","docs/architecture/interfaces-and-concurrency.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-16-PLAN.md","docs/architecture/interfaces-and-concurrency.md"],"details":"Riga 20: deadline esplicita (D-22)\"     - \"Nessun tipo di provider, SDK o database attraversa una firma (object_store, sqlx, reqwest, pgvector, pdfium-render, axum), e ogni trait è usabile come Arc<dyn Trait>\"     - \"La scelta fr"}
    mitigation: "Fissare per ogni mutazione la chiave stabile e la semantica dei retry/esito ignoto, oltre a propagazione request/trace tramite contesto puro. Documentare le precondizioni di atomicità e non introdurre tipi di provider."
    test_hint: "Fake che applica l'operazione e poi simula timeout: il retry con lo stesso ID non deve creare un secondo evento/versione. Verificare correlazione e distinzione fra non applicato e esito sconosciuto."
    test_required: true
  - id: "RISK-004"
    severity: "MEDIUM"
    likelihood: "MEDIUM"
    category: "embedding_compatibility"
    title: "Versione del modello non esaurisce l'identità dello spazio di embedding"
    scenario: "La firma separa embed_documents/embed_query e porta una versione, ma la search può ancora ricevere un vettore compatibile per dimensione e incompatibile per modello/revisione/normalizzazione o istruzione."
    trigger: "Query generata dallo sfidante o dopo un cambio modello contro un indice pubblicato col precedente."
    failure: "Con i modelli previsti a 1024 dimensioni un semplice controllo di lunghezza non rileva il mismatch; il compilatore accetta la chiamata e la ricerca restituisce candidati semanticamente sbagliati."
    impact: "Recall degradata silenziosamente e risultati di S-03 non preservati nei contratti di provider."
    affected: ["spikes/s07-ports/src/lib.rs","docs/adr/0012-firme-porte-candidate.md"]
    evidence: {"type":"plan","files":[".planning/phases/01-spike-tecnici/01-16-PLAN.md",".planning/phases/01-spike-tecnici/01-10-PLAN.md",".planning/phases/01-spike-tecnici/01-CONTEXT.md"],"details":"Riga 78: con versione del modello (S-03, D-14); `VectorIndex::search(query, predicate: AuthorizationPredicate, k, deadline)` e `publish_version` (S-03, D-12, D-15); `DecisionEngine::{validate_query → QueryDecision { complexity: S"}
    mitigation: "Definire un'identità di spazio che leghi modello/revisione/preprocessing e indice; farla accompagnare vettori query/documento e pubblicazione, con rifiuto del mismatch prima della search. La porta può restare indipendente da TEI."
    test_hint: "Due embedding da 1024 dimensioni ma space_id diversi: pubblicazione/ricerca deve fallire con errore di compatibilità; stesso spazio su query e documenti deve essere accettato."
    test_required: true
external_research: []
```

## Condizioni per una nuova valutazione

Correggere o dimostrare la mitigazione dei rischi HIGH con le verifiche indicate, riconciliare i contratti con i piani proprietari senza modifiche silenziose ai file chiusi e aggiornare versione/impronta del PLAN quando cambia. Le ricerche blocking devono essere chiuse con fonti e versione prima di attestare la rispettiva garanzia. I MEDIUM richiedono mitigazione o decisione esplicita nella sede prevista dal progetto; questo audit non concede accettazioni.

Verdetto finale: **REVISE**