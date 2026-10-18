# Strategia del corpus

## Perché serve

Un sistema RAG non possiede conoscenza utile senza documenti. Il corpus serve a fornire fonti, esercitare la pipeline, verificare RBAC/citazioni/rifiuti e misurare regressioni.

Non servono migliaia di documenti all'inizio. Per il primo vertical slice bastano 10–30 documenti selezionati che coprano testo, tabelle, grafici, versioni e livelli di accesso.

## Knowledge corpus ed evaluation corpus

### Knowledge corpus

- Visibile secondo ACL.
- Cambia tramite upload e fonti online.
- Ogni documento è versionato.
- La nuova versione sostituisce quella pubblicata solo dopo l'indicizzazione completa.

### Evaluation corpus

- Versionato e immutabile per release.
- Contiene domande, risposte attese, evidenze, pagine e profilo utente.
- Non viene ampliato automaticamente dal crawler.
- Ogni modifica produce una nuova versione del benchmark.

Senza evaluation corpus non possiamo distinguere un miglioramento da una regressione dovuta a parser, chunker, embedding, prompt o versione JEV.

## Aggiornamento online

```text
source connector
  -> discovered candidate
  -> quarantine
  -> checksum/deduplication
  -> provenance and license policy
  -> deterministic file validation
  -> JEV semantic triage
  -> approval policy
  -> ingestion
  -> atomic publication
```

### Cosa può fare JEV

- classificare tipo e pertinenza del documento;
- segnalare nuova versione o candidato irrilevante;
- rilevare segnali di contenuto sospetto o prompt injection;
- valutare pagine/immagini in modalità sperimentale;
- assegnare priorità alla revisione.

### Cosa non deve decidere da solo

- autenticità e licenza della fonte;
- autorizzazioni e classificazione finale;
- assenza di malware;
- deduplicazione esatta;
- pubblicazione di contenuti riservati.

## Fonti e provenance

Ogni `DocumentSource` espone discovery incrementale tramite cursor e fetch dell'originale. Il primo connettore deve puntare a una fonte ufficiale e stabile; API, RSS o manifest sono preferibili allo scraping HTML.

Metadati minimi:

```json
{
  "source_id": "official-source",
  "external_id": "stable-provider-id",
  "canonical_url": "https://example.org/document.pdf",
  "discovered_at": "RFC-3339",
  "published_at": "RFC-3339",
  "content_sha256": "...",
  "retrieval_etag": "...",
  "license": "...",
  "provenance_status": "verified"
}
```

## Dataset iniziale

TAT-DQA è candidato per evaluation e documenti visualmente ricchi. Il runtime corpus può partire da un sottoinsieme piccolo e poi includere documenti online. FinanceBench è un benchmark complementare dopo verifica delle condizioni d'uso.

- TAT-DQA: <https://nextplusplus.github.io/TAT-DQA/>
- FinanceBench: <https://github.com/patronus-ai/financebench>
- DocLayNet, utile per test di parsing e layout: <https://github.com/DS4SD/DocLayNet>

## Politica MVP

- Upload manuale: pubblicazione dopo validazione e ACL esplicita.
- Scoperta online: quarantena obbligatoria.
- Auto-approvazione: disabilitata inizialmente.
- Fonte fidata: aggiornamento solo dopo provenance, checksum, tipo file e policy.
- Documento fallito: diagnostica conservata, mai queryable.
- Cancellazione: tombstone e rimozione asincrona da indice e storage.

## Dataset di sicurezza sintetico

Preparare documenti con ACL diverse, informazioni presenti solo in contenuti vietati, versioni in conflitto, domande senza risposta, prompt injection, metadati falsi, PDF corrotti/cifrati e scansioni di bassa qualità.
