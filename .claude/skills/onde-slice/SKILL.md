---
name: onde-slice
description: Il pianificatore della concorrenza del loop GSD+ECC di Corporate Knowledge Assistant — trasforma i piani di una fase in onde eseguibili in parallelo in sicurezza, usando dipendenze, sovrapposizioni di file e i vincoli meccanici di progetto (superficie API, migrazioni per servizio, dipendenze del workspace, tetto di 2 esecutori). USARE come ultimo passo di plan X, oppure standalone per riraggruppare le slice dopo un ritocco ai piani ("rifai le onde della fase X"). Produce 0X-ONDE.md, consumato da execute e dall'agente integratore-onda.
---

# onde-slice

Skill di servizio del loop GSD+ECC (spec: `docs/superpowers/specs/loop-gsd-ecc-design.md` §9). Input: i PLAN della fase (frontmatter `files_modified` e `depends_on`, letti via `node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" phase-plan-index X`), `0X-WIRING.md` (§3 convenzioni, §5 seam di wave 0), `0X-CONTRATTO.md`.

## Algoritmo

1. **Grafo delle dipendenze** — da `depends_on` si derivano i livelli: un piano sta in un'onda successiva a tutte le sue dipendenze.
2. **Sovrapposizione file** — dentro ogni livello, controllo `files_modified` a coppie, glob-aware e bidirezionale (`services/document-service/src/**` copre `services/document-service/src/retrieval/mod.rs` e viceversa). Anche UN solo file in comune → i due piani NON girano in parallelo: onde separate, o stessa onda con serializzazione dichiarata nel documento. `Cargo.lock`, `Cargo.toml` alla radice e `apps/web/package-lock.json` contano come file comuni: due piani che cambiano dipendenze non stanno nella stessa onda.
3. **Vincoli meccanici di progetto**:
   - **un solo piano con superficie API per onda** — la specifica OpenAPI e il client generato per `apps/web` si rigenerano per intero: due rigenerazioni nella stessa onda collidono;
   - **un solo piano per servizio e per onda che aggiunge migrazioni SQL** — la numerazione delle migrazioni di un servizio segue la convenzione della WIRING-MAP e due piani paralleli prenderebbero lo stesso numero;
   - **un solo piano per onda che aggiunge membri al workspace Cargo** — la lista `members` è un seam condiviso.
3-bis. **Proprietà dei file fra fasi** (strategia «provider-first, insiemi di file chiusi per fase», decisione utente 2026-10-18) — prima di comporre le onde lancia `node scripts/gate-proprieta-file.cjs --fase X`: uscita 1 (un file di X reclamato anche da un'altra fase, fuori da whitelist, innesti della WIRING-MAP e riaperture `riapre_perimetro` di una fase decimale) o 2 (non eseguito) → **rifiuta** e rimbalza a `plan`: un'onda non serializza ciò che due fasi si contendono. Nelle **onde di dipendenza cross-fase** (esecuzione per onde dopo la riconciliazione, con fasi indipendenti in parallelo come Provider di ingestion e Provider di decisione) due piani di fasi diverse stanno nella stessa onda solo se l'intersezione dei loro `files_modified` è fatta di soli file in whitelist; anche allora si applicano i vincoli del punto 3 (lockfile, un solo rigeneratore del client TS per onda, sezioni append-only di `Cargo.toml`/`package.json` → onde separate o serializzazione dichiarata).
4. **Tetto di concorrenza** — massimo esecutori per onda da config (`workflow.max_esecutori_onda`, default **2**); onde più larghe si spezzano.

## Output: `0X-ONDE.md`

Committato nella directory di fase. Formato, con esempio compilato:

```markdown
# Onde di esecuzione — Fase 0X

## Onda 1
- **0X-02** (porta `Chunker` + adapter) — indipendente; nessun file condiviso con 0X-03
- **0X-03** (componente viewer citazioni) — indipendente
- Attenzione per l'integratore: crate e app distinti — conflitti attesi: zero; ordine di merge: 0X-02 poi 0X-03

## Onda 2
- **0X-05** (endpoint `/internal/v1/retrieval/search`) — dipende da 0X-02; UNICO piano API dell'onda; UNICO piano con migrazioni di document-service
- Attenzione per l'integratore: rigenera il client OpenAPI per intero — mai fondere a metà rigenerazione
```

Ogni onda dichiara: slice, motivo del raggruppamento, ordine di merge e punti d'attenzione per `integratore-onda`. **Stile caveman full** (Regola caveman, spec §2): motivi e attenzioni tersi, ID e percorsi verbatim.

## Rifiuti (mai indovinare)

- **Ciclo nelle dipendenze** → rifiuta e rimbalza a `plan` con il ciclo nominato.
- **Piano senza `files_modified`** → rifiuta e rimbalza a `plan`: senza la lista dei file la sicurezza del parallelo non è dimostrabile.
- **`gate-proprieta-file.cjs --fase X` non verde** (1 o 2) → rifiuta e rimbalza a `plan` con le violazioni elencate dallo script.

## Cosa NON fa

Non esegue nulla, non modifica i piani, non decide l'ordine DENTRO una slice (è del piano), non scrive lo stato (lo fa `plan` via `update-orchestrator-state`).

**Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
**Modifica:** Claude (Anthropic) · 2026-10-18 · strategia provider-first e insiemi di file chiusi (decisione utente)
