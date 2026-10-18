# Test design 01-03 — corpus degli spike

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-03-PLAN.md
  sha256: a8743b44a17ee5c70e9e577bc8110eb21832a2aafd2a8eb8b1639a1762d69dbb
risks:
  - id: RISK-1
    severity: HIGH
    title: Matrice di visibilità sbagliata (ruolo che vede una classificazione non prevista) falsa l'oracolo di tutti i leak test a valle
    source: must_have "matrice sintetica ruolo × classificazione ... oracolo di visibilità deterministico"
  - id: RISK-2
    severity: HIGH
    title: Classificazione del front matter diversa da quella del manifest (o DOCX derivato che perde la classificazione del sorgente)
    source: must_have "documenti sintetici classificati"
  - id: RISK-3
    severity: MEDIUM
    title: Download non verificato o via canale non HTTPS; file manomesso accettato
    source: threat_model T-01-03-01
  - id: RISK-4
    severity: MEDIUM
    title: Documento reale riservato o dato personale nel corpus inviato a provider cloud
    source: threat_model T-01-03-02
  - id: RISK-5
    severity: MEDIUM
    title: Binari protetti da copyright versionati nel repo
    source: threat_model T-01-03-04
  - id: RISK-6
    severity: MEDIUM
    title: Manifest incompleto (chiavi mancanti, sha256 non coincidente) o documento TTQ di riferimento fuori specifica (25 pagine, ≤ 10 MB)
    source: must_have "provenienza registrata ... sha256"
  - id: RISK-7
    severity: MEDIUM
    title: fetch.sh non idempotente; worktree successivi non riescono a ricostruire raw/
    source: must_have "download riproducibile con verifica sha256"
  - id: RISK-8
    severity: LOW
    title: Casi di injection non marcati o contenenti URL http:// o segreti; documento trappola assente
    source: threat_model T-01-03-03
  - id: RISK-9
    severity: LOW
    title: DOCX senza stili di intestazione o interruzione di pagina, inutili per misurare il locator DOCX
    source: must_have "documenti sintetici ... DOCX derivati" (Task 2 action)
  - id: RISK-10
    severity: LOW
    title: Matrice di misura scambiata per decisione di prodotto sui permessi (Q-01/Q-02 aperte)
    source: Task 2 action
  - id: RISK-11
    severity: MEDIUM
    title: Il profilo restricted-only non raggiunge la selettività ≤ 5% sui chunk, quindi non stressa il filtro ACL
    source: must_have "almeno un profilo a selettività ≤ 5%"
tests:
  - {id: TEST-1, title: acl.json coincide con la matrice dichiarata e ha il profilo restricted-only, type: check-dati, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: Classificazione front matter = manifest per ogni sintetico, e DOCX = sorgente, type: check-dati, priority: P0, catches: [RISK-2]}
  - {id: TEST-3, title: fetch.sh solo HTTPS con i limiti curl dichiarati, type: check-statico, priority: P1, catches: [RISK-3]}
  - {id: TEST-4, title: File manomesso in raw/ mai accettato, type: check-comportamento, priority: P1, catches: [RISK-3, RISK-7]}
  - {id: TEST-5, title: fetch.sh idempotente (seconda esecuzione senza download, exit 0), type: check-comportamento, priority: P1, catches: [RISK-7]}
  - {id: TEST-6, title: Schema e conteggi del manifest, derivato TTQ 25 pagine e ≤ 10 MiB, type: check-dati, priority: P1, catches: [RISK-6]}
  - {id: TEST-7, title: sha256 di ogni sintetico e DOCX coincide col manifest, type: check-dati, priority: P1, catches: [RISK-6, RISK-2]}
  - {id: TEST-8, title: raw/ non tracciato, nessun PDF versionato nel corpus, type: check-git, priority: P1, catches: [RISK-5]}
  - {id: TEST-9, title: Provenienza dei pubblici (source_url HTTPS) e autore sintetico dichiarato, type: check-dati, priority: P1, catches: [RISK-4]}
  - {id: TEST-10, title: Revisione umana dei sintetici per dati personali reali, type: manuale, priority: P2, catches: [RISK-4]}
  - {id: TEST-11, title: Marcatori injection-case in syn-07 e syn-08, syn-09 restricted, scanner verde, type: check-dati, priority: P2, catches: [RISK-8]}
  - {id: TEST-12, title: DOCX con stili Heading e interruzione di pagina, type: check-dati, priority: P2, catches: [RISK-9]}
  - {id: TEST-13, title: Disclaimer «matrice di misura, non decisione di prodotto» in acl.json e CORPUS.md, type: check-statico, priority: P3, catches: [RISK-10]}
uncovered:
  - risk: RISK-11
    reason: "La selettività si misura solo dopo il chunking, che non esiste in questo piano: il piano stesso delega la verifica numerica a S-03 (piano 01-14, comando leak). Qui TEST-1 controlla solo che il profilo esista e veda solo restricted."
```

## Come leggere questo design

Il piano non scrive codice Rust: produce dati. Quindi i test sono controlli sui file (`node -e` sul JSON, `grep`, `git`) e due prove di comportamento di `fetch.sh`. Il punto più delicato non è la sicurezza del download ma l'oracolo di visibilità: `acl.json` più la classificazione di ogni documento diventano, nei piani 01-10, 01-11, 01-12, 01-14 e 01-15, la verità con cui si conta un leak. Se la matrice è sbagliata, un leak reale risulta «0» a valle. Per questo RISK-1 e RISK-2 sono HIGH anche se il piano li tratta come dati.

Tutti i controlli si lanciano dalla radice del repo dopo `bash spikes/data/corpus/fetch.sh` e `bash spikes/data/corpus/synthetic/build.sh`. Nessun file nuovo: i comandi vivono qui e nel verify di `execute`.

## Test

**TEST-1 — Matrice di visibilità esatta.** `node -e` su `spikes/data/corpus/acl.json`: `roles` ha esattamente le chiavi `admin`, `manager`, `employee`, `guest`; gli insiemi di classificazioni visibili sono, confrontati come insiemi ordinati, admin = {public, internal, confidential, restricted}, manager = {public, internal, confidential}, employee = {public, internal}, guest = {public}; esiste `profiles["restricted-only"]` che vede esattamente {restricted}. Un insieme diverso fa uscire con codice ≠ 0 e stampa il ruolo. Il test confronta con la matrice scritta nel piano, non con un'altra lettura dello stesso file.

**TEST-2 — Classificazione coerente.** Per ogni `syn-0N-*.md`, il campo `classification` del front matter YAML (lettura delle righe fra i due `---`) coincide con la voce del manifest che ha lo stesso `doc_id`; per ogni voce `format: docx` con `derived_from`, la classificazione coincide con quella del sorgente. Inoltre le classificazioni attese per nome sono rispettate: syn-01 internal, syn-02 confidential, syn-03 restricted, syn-04 internal, syn-05 confidential, syn-06 public, syn-07 public, syn-08 internal, syn-09 restricted. Ogni pubblico ha `classification: public`.

**TEST-3 — Solo HTTPS.** `grep -c 'http://' spikes/data/corpus/fetch.sh spikes/data/corpus/manifest.json` dà 0 per entrambi; `fetch.sh` contiene `--proto '=https'`, `--max-redirs 3`, `--max-time 300`, `--max-filesize 104857600` e `sha256sum`. Ogni `source_url` non nullo del manifest inizia con `https://`.

**TEST-4 — File manomesso mai accettato.** Dopo un fetch riuscito, si copia da parte un file di `raw/`, se ne altera un byte (`printf 'X' | dd of=raw/<doc>.pdf bs=1 seek=100 conv=notrunc`) e si rilancia `fetch.sh`. Esiti ammessi: exit 0 con il file riscaricato e sha256 di nuovo coincidente, oppure exit 1 con il file rimosso. Esito vietato: exit 0 con il file alterato ancora presente. Alla fine si ripristina il file. È il test che rende vero «mismatch = uscita 1, file scartato» anche nel ramo «file già presente» dell'idempotenza.

**TEST-5 — Idempotenza.** Seconda esecuzione di `fetch.sh` subito dopo la prima: exit 0, nessuna nuova scrittura in `raw/` (`find raw -newer <timestamp-file>` vuoto) e nessuna chiamata di rete (il log di `fetch.sh` dichiara «già presente, sha256 ok» per ogni voce).

**TEST-6 — Manifest.** Il `node -e` del verify del Task 1 (≥ 5 pubblici IT, ≥ 4 EN, un derivato da 25 pagine, chiavi obbligatorie non nulle) più: ogni voce ha anche `title`, `pages`, `bytes`, `derived_from` (anche null) e `oracle_author` (null); `doc_id` univoci; il derivato TTQ ha `bytes ≤ 10485760` e `derived_from` che punta a un pubblico IT; `lang ∈ {it, en}`, `kind ∈ {public, synthetic, derived}`, `format ∈ {pdf, docx, md}`.

**TEST-7 — Hash dei file versionati.** Per ogni voce `synthetic` e per ogni DOCX, `sha256sum` del file in `synthetic/` coincide con il manifest. Per i pubblici e il derivato PDF il controllo è quello di `fetch.sh` (TEST-4/5).

**TEST-8 — Nessun binario protetto versionato.** `git status --porcelain spikes/data/corpus/raw` vuoto e `git ls-files spikes/data/corpus | grep -Ei '\.pdf$'` vuoto. I soli binari tracciati sono i quattro DOCX sintetici.

**TEST-9 — Provenienza.** Ogni `public` ha `source_url` HTTPS non vuoto e `license` con la dicitura «non ridistribuito»; ogni `synthetic` ha `source_url: null`, licenza CC0 e front matter con `author: synthetic (Claude)`.

**TEST-10 — Revisione dei sintetici (manuale).** Lettura dei nove `.md` da parte del verificatore: società fittizia «Alfa Componenti S.p.A.», nessun nome, codice fiscale, IBAN o indirizzo email reale. Il controllo automatico possibile è parziale (`grep -E '[A-Z]{6}[0-9]{2}[A-Z][0-9]{2}[A-Z][0-9]{3}[A-Z]|IT[0-9]{2}[A-Z][0-9]{10}' synthetic/*.md` vuoto per codici fiscali e IBAN), la parte sui nomi resta umana.

**TEST-11 — Fixture di injection e trappola.** `grep -l 'injection-case:' synthetic/*.md` elenca esattamente syn-07 e syn-08; syn-09 ha front matter `restricted`; `python skills/security-scanner/security_scan.py spikes/data/corpus` esce 0; ogni URL nei sintetici è `https://` verso `example.com` o un altro dominio riservato alla documentazione.

**TEST-12 — DOCX utili al locator.** Per ciascuno dei quattro DOCX, `unzip -p <file> word/document.xml` contiene almeno un `w:pStyle w:val="Heading1"` (o `Heading2`) e almeno un `w:br w:type="page"`. CORPUS.md riporta tag e digest dell'immagine `pandoc/core`.

**TEST-13 — Disclaimer.** `acl.json` ha in testa una chiave descrittiva (per esempio `_note`) che contiene «matrice di misura» e rimanda a Q-01/Q-02; CORPUS.md lo ripete; CORPUS.md ha la riga `**Modifica:**`.

## Copertura

I due HIGH sono coperti: RISK-1 da TEST-1, RISK-2 da TEST-2 e TEST-7. La selettività ≤ 5% (RISK-11, MEDIUM) è in `uncovered` perché il piano stesso la rinvia a S-03.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-03

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da b36284cc0f8e… a a8743b44a17e… per l'allineamento a 01-02 v2 (fetch.sh produce i derivati con session.sh oneshot (o with 01-03) invece di `--profile runner run`). I test del design non cambiano: cambiano solo i comandi di avvio dei servizi e la prenotazione della finestra di misura.
