# Phase 3: Provider di ingestion e indice - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-10-21
**Phase:** 03-provider-di-ingestion-e-indice
**Areas discussed:** Buchi nelle firme, ACL dentro l'indice, Test/fake/CI, Parser/chunk/fonte web

Ogni domanda è arrivata armata del finding del brief `03-RICERCA-DISCUSSIONE.md` e di un esempio concreto (preferenza dell'utente).

---

## Buchi nelle firme

**Ricerca ibrida senza testo** (D-15 di fase 1 vs firma S-07 candidata)

| Option | Description | Selected |
|--------|-------------|----------|
| Testo + vettore | `search(ctx, SearchQuery { text, embedding }, &predicato, k)`, una query SQL | ✓ |
| Due porte separate | `LexicalIndex` + RRF nel chiamante, predicato ripetuto | |
| Solo vettoriale nell'MVP | D-15 rivista, sigle e numeri trovati peggio | |

**`delete` per il GC** (ADR-0003, regola provider-first)

| Option | Description | Selected |
|--------|-------------|----------|
| Sì, consumatore fase 6 | `delete` in porta; job GC e `pending` alla fase 6 | ✓ |
| GC tutto in fase 3 | accoppia i metadati della fase 6 | |
| Niente GC nell'MVP | ADR-0003 emendato | |

**Input del parser** (D-08, spike 01-07)

| Option | Description | Selected |
|--------|-------------|----------|
| File locale | `parse(ctx, &LocalFile, MediaType)` | ✓ |
| Resta ObjectRef | il parser legge da `DocumentStore` | |

**Destinazione delle variazioni** (01-16 non eseguito)

| Option | Description | Selected |
|--------|-------------|----------|
| Nel brief di 01-16 | ADR-0012 nasce allineato, senza re-gate | ✓ |
| Solo nel plan della fase 3 | divergenze motivate da ADR-0012 | |
| Riapro il piano 01-16 | `plan_version` +1 e re-gate | |

---

## ACL dentro l'indice

**Forma del predicato** (Azure AI Search, security filter pattern)

| Option | Description | Selected |
|--------|-------------|----------|
| Insieme di etichette | intersezione; vuoto = zero risultati; indice agnostico da Q-02 | ✓ |
| Ruoli + livello di classificazione | l'indice conosce la scala, dipende da Q-02 | |

**Dove sta l'ACL** (revoca immediata, fase 7 criterio 4)

| Option | Description | Selected |
|--------|-------------|----------|
| Proiezione dell'indice | `document_acl` fase 3 + `set_acl` (consumatori 6, 7, 12) | ✓ |
| Copiata su ogni chunk | revoca = riscrittura di tutti i chunk | |
| Join sulle tabelle della fase 6 | insiemi chiusi violati di fatto | |

**Tombstone senza fase proprietaria**

| Option | Description | Selected |
|--------|-------------|----------|
| Lacuna ai discuss 6/12 | `withdraw` aggiunto in riconciliazione | ✓ |
| Assegno ora alla fase 6 | decisione di scope della fase 6 presa qui | |

**Versione ACL del documento**

| Option | Description | Selected |
|--------|-------------|----------|
| Sì, versione per documento | `set_acl` incrementa, restituita con i candidati | ✓ |
| Lascio decidere alla fase 7 | solo etichette in fase 3 | |

---

## Test, fake e CI

**Suite sugli adapter reali** (CI senza GPU)

| Option | Description | Selected |
|--------|-------------|----------|
| CI + gate locale GPU | PG, Garage, PDFium+Tesseract, TEI CPU in CI; TEI GPU e Docling in locale (verify) | ✓ |
| Reali solo in locale | CI solo fake | |
| Runner self-hosted | PC personale come runner | |

**Esposizione dei fake** (Cargo features)

| Option | Description | Selected |
|--------|-------------|----------|
| Feature `testkit` | mai nel binario di release, controllo CI | ✓ |
| Crate separato | `crates/document-testkit` | |

**Capacità dei fake**

| Option | Description | Selected |
|--------|-------------|----------|
| Deterministici con guasti | copione di guasti, clock in pausa, conteggio chiamate | ✓ |
| Solo percorso felice | ogni fase consumer scrive i propri fake | |

**Fixture ostili**

| Option | Description | Selected |
|--------|-------------|----------|
| Copia con manifest | da `spikes/data/hostile/` a `tests/fixtures/hostile/` | ✓ |
| Generati dai test | nessun binario versionato | |

---

## Parser, chunk e fonte web

**Docling nel parser della fase 3**

| Option | Description | Selected |
|--------|-------------|----------|
| Sì, condizionato agli spike | forma da ADR-0004 e D-27 | ✓ |
| Rinviato alla fase 14 | tabelle come testo | |

**Confini dei chunk** (locator D-10 a una pagina)

| Option | Description | Selected |
|--------|-------------|----------|
| Mai a cavallo (raccomandata) | come lo spike 01-10; dato NVIDIA sul chunking per pagina | |
| Locator multi-pagina | locator a lista di span; ADR-0005 e viewer cambiano | ✓ |

**Notes:** scelta contro la raccomandazione. Domande di seguito:
- Ambito: **PDF e testo** (✓) vs solo PDF.
- Tetto: **al massimo un confine** (✓) vs nessun tetto.
- Propagazione: **brief di 01-09 e 01-16** (✓) vs riapertura di 01-09.

**Misura dei chunk** (TEI `--auto-truncate`)

| Option | Description | Selected |
|--------|-------------|----------|
| Caratteri + TEI che rifiuta | come lo spike, errore tipizzato invece del troncamento | ✓ |
| Token del modello | crate `tokenizers` | |

**SSRF** (OWASP SSRF Prevention Cheat Sheet)

| Option | Description | Selected |
|--------|-------------|----------|
| Nel processo + rete | resolver custom, IP validati, redirect controllati, https, tetti; rete Compose dedicata | ✓ |
| Proxy egress Smokescreen | servizio Go in più | |

---

## Criticità

Ratificata **critica** (parser su input non fidato, SSRF, predicato del retrieval autorizzato).

## Claude's Discretion

- Immagine del worker (distroless o debian-slim) secondo ADR-0004.
- Lingua del `tsvector` e parametri di ricerca secondo ADR-0006 e discuss 7.
- Modello piccolo per TEI CPU in CI, nomi delle tabelle, struttura dei moduli, forma del copione di guasti.

## Deferred Ideas

- Tombstone `DELETE /api/v1/documents/{id}` senza fase proprietaria → discuss 6 o 12.
- Re-embedding al cambio di modello: fuori dall'MVP.
- HNSW + `iterative_scan`: fase 14.

**Modifica:** Claude (Anthropic) · 2026-10-21 · log della discussione della fase 03
