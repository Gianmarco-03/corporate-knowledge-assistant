# Test design 01-17 — threat model, DEPENDENCIES.md e chiusura degli ADR

```yaml
reviewed_plan:
  path: .planning/phases/01-spike-tecnici/01-17-PLAN.md
  sha256: e99ab09adf51a8efbd0b2ddf6acbbdc8872808c249fdf8b40a13118d7eaf2c07
risks:
  - id: RISK-1
    severity: HIGH
    title: Codice degli spike agganciato al prodotto (Cargo.toml in radice, riferimenti a spikes in manifest, Dockerfile, compose o workflow di prodotto)
    source: threat_model T-01-17-04
  - id: RISK-2
    severity: HIGH
    title: Una decisione diventa Decisa/Accepted senza ratifica dell'utente, o un ADR in eccezione viene marcato Accepted
    source: must_have "DEPENDENCIES.md riflette le decisioni ratificate"; threat_model T-01-17-01
  - id: RISK-3
    severity: HIGH
    title: Threat model con mitigazione dichiarata «misurata» che cita un file di evidenza inesistente o un glob non verificabile
    source: must_have "mitigazioni mappate ... al file di evidenza dello spike"
  - id: RISK-4
    severity: MEDIUM
    title: Minaccia T-01..T-28 mancante o senza requisito e stato; minacce emerse dagli spike non aggiunte
    source: must_have "minacce prioritarie T-01..T-28 (più quelle emerse dagli spike)"
  - id: RISK-5
    severity: MEDIUM
    title: Regola superata in DEPENDENCIES.md cancellata senza paragrafo Storia; voce nuova nell'elenco «da evitare»
    source: threat_model T-01-17-02
  - id: RISK-6
    severity: MEDIUM
    title: Modifica degli ADR 0003-0012 oltre la riga Stato e la riga Modifica (deviazione dichiarata superata)
    source: Task 3 action deviazione da WIRING §4
  - id: RISK-7
    severity: MEDIUM
    title: ADR 0003-0011 senza link a RISULTATI e hw.json esistenti; ADR-0012 senza «Vincoli verso la Fondazione»; numeri ADR non contabilizzati
    source: must_have "ogni ADR nuovo collega RISULTATI e hw.json esistenti"
  - id: RISK-8
    severity: MEDIUM
    title: Segreti o endpoint interni letterali nei documenti
    source: threat_model T-01-17-03
  - id: RISK-9
    severity: MEDIUM
    title: Rinvio AV/CDR senza ADR completo o senza rischio residuo nel threat model
    source: threat_model T-01-17-05 (accept); must_have "AV e CDR rinviati con un ADR dedicato"
  - id: RISK-10
    severity: LOW
    title: Voci della checklist LLM/RAG spuntate senza riga di mitigazione corrispondente
    source: Task 1 action
tests:
  - {id: TEST-1, title: Isolamento degli spike dal prodotto su tutti i file tracciati, type: check-git, priority: P0, catches: [RISK-1]}
  - {id: TEST-2, title: Corrispondenza fra ratifiche del SUMMARY, Stato degli ADR e righe Decisa di DEPENDENCIES.md, type: check-documento, priority: P0, catches: [RISK-2]}
  - {id: TEST-3, title: Ogni percorso di evidenza citato nel threat model esiste, nessun glob, type: check-documento, priority: P0, catches: [RISK-3]}
  - {id: TEST-4, title: Stato «misurata nello spike» solo con evidenza, type: check-documento, priority: P0, catches: [RISK-3]}
  - {id: TEST-5, title: Tabella delle minacce completa e ben formata (T-01..T-28, T-29+), type: check-documento, priority: P1, catches: [RISK-4]}
  - {id: TEST-6, title: Storia per ogni regola cambiata e elenco «da evitare» invariato, type: check-git, priority: P1, catches: [RISK-5]}
  - {id: TEST-7, title: Diff degli ADR 0003-0012 limitato a Stato e Modifica (salvo ADR-0004, eccezione D-25), type: check-git, priority: P1, catches: [RISK-6]}
  - {id: TEST-12, title: Limiti noti D-25 in ADR-0004 e README, type: check-static, priority: P1, catches: [RISK-6]}
  - {id: TEST-8, title: Link degli ADR e contabilità dei numeri 0003-0013, type: check-documento, priority: P1, catches: [RISK-7]}
  - {id: TEST-9, title: Scanner di sicurezza verde sui documenti, type: check-statico, priority: P1, catches: [RISK-8]}
  - {id: TEST-10, title: ADR-0013 completo e rischio residuo AV/CDR nel threat model, type: check-documento, priority: P2, catches: [RISK-9]}
  - {id: TEST-11, title: Checklist LLM/RAG coerente con le righe, type: manuale, priority: P2, catches: [RISK-10]}
uncovered: []
```

## Come leggere questo design

Il piano 01-17 non misura nulla: consolida. I suoi rischi sono di integrità documentale, ma tre sono HIGH perché un errore qui rende ufficiale qualcosa che non lo è. Una decisione non ratificata che diventa «Decisa» (RISK-2) vincola le fasi successive senza il giudizio dell'utente. Una mitigazione dichiarata «misurata» con un'evidenza che non esiste (RISK-3) è un falso verde nel threat model, che è il requisito PLAT-07. E l'isolamento degli spike (RISK-1) è il criterio 5 della fase.

Sul RISK-3 c'è un'incoerenza nel piano da segnalare: l'azione del Task 1 suggerisce per T-14 l'evidenza `spikes/s04-jev/results/**`, ma l'acceptance richiede `test -f` su ogni percorso citato. Un glob non passa `test -f`. L'esecutore deve citare file concreti (`spikes/s04-jev/results/jev/forms.csv`, `spikes/s04-jev/results/rerank/rerank.csv`); TEST-3 lo impone.

Tutti i test sono comandi lanciati dalla radice dopo il Task 3, con letture di `git diff` rispetto al commit di partenza del piano (`<base>`). Nessun file nuovo.

## Test

**TEST-1 — Isolamento.** Oltre ai controlli del verify (`test ! -f Cargo.toml`, nessun `spikes` nei `Cargo.toml` di `services crates apps`, nessun `spikes` in `.github/workflows`), un controllo più largo: `git grep -l "spikes/" -- ':!spikes' ':!.planning' ':!docs' ':!*.md' ':!.gitignore'` restituisce vuoto. Così sono coperti anche `apps/web/package.json`, Dockerfile e compose di prodotto, script di build e configurazioni degli hook. Se `.githooks/` cita `spikes/` solo per escluderlo, la riga è ammessa e si annota.

**TEST-2 — Ratifiche coerenti.** Dal SUMMARY del Task 2 si legge, per ogni ADR 0003-0012, l'esito (ratificato / eccezione / rimisura). Controllo incrociato: ogni ADR ratificato ha `Stato: Accepted` e una riga `**Modifica:**` con la data della ratifica; ogni ADR in eccezione o rimisura resta `Proposed` con il motivo; in `DEPENDENCIES.md` ogni riga passata a «Decisa» in questo piano (`git diff <base> -- DEPENDENCIES.md`, righe aggiunte con `Decisa`) cita un ADR ratificato, e nessuna riga cita come Decisa un ADR non ratificato. Con «rimisura» su un ADR, le righe collegate di DEPENDENCIES.md restano invariate.

**TEST-3 — Evidenze esistenti.** Si estraggono dal threat model tutti i percorsi che iniziano con `spikes/` (`grep -oE 'spikes/[A-Za-z0-9_./*-]+' docs/security/threat-model.md | sort -u`): nessuno contiene `*`; ciascuno esiste (`test -f`); sono almeno 10. I nomi di colonna citati accanto ai file (`unauthorized_sent`, `unauthorized_in_prompt`, `claims_unknown_id`, `instructs_model_flagged`, `denied_in_topk`) compaiono davvero nell'header del CSV citato nella stessa riga.

**TEST-4 — Stato coerente con l'evidenza.** Per ogni riga della tabella «Threat analysis» con `Stato = misurata nello spike`, la colonna `Evidenza / Test` contiene almeno un percorso `spikes/` valido per TEST-3. Per ogni riga `da implementare`, la colonna contiene il test previsto nel prodotto (non vuota). Per `rinviata (ADR-0013)`, la riga cita ADR-0013. Gli stati ammessi sono solo questi tre.

**TEST-5 — Tabella completa.** Il ciclo `for i in $(seq -w 1 28)` del verify, reso più rigido: si cercano righe di tabella che iniziano con `| T-$i |` (non la sola stringa, che potrebbe comparire nel testo); ogni riga ha 10 colonne, `STRIDE` in {S, T, R, I, D, E} o nome esteso, e la colonna `Requisiti` contiene almeno un ID nella forma `(AUTH|DOC|SEC|QRY|VIEW|AUD|ONL|PLAT)-[0-9]+` che esiste in `.planning/REQUIREMENTS.md`. Esistono righe da T-29 in poi che coprono almeno i quattro temi elencati nel piano (supply chain di pesi e binari, VRAM condivisa, residenza e retention presso JEV e Anthropic, varianza dei segnali JEV). La riga delle priorità dello stub è conservata, e c'è il paragrafo *Storia* con la firma.

**TEST-6 — Storia e «da evitare».** `git diff <base> -- DEPENDENCIES.md`: per ogni riga rimossa che conteneva una regola (stato o scelta), esiste nello stesso diff un paragrafo *Storia* aggiunto che la nomina. La sezione «da evitare nell'MVP» non ha righe aggiunte (`git diff` limitato a quella sezione: zero righe `+` di contenuto). `DEPENDENCIES.md` contiene `jev-1.13.0` e la firma `**Modifica:**` in testa.

**TEST-7 — Diff degli ADR limitato.** Per ogni file `docs/adr/00{03..12}-*.md` tranne `0004` (eccezione D-25, coperta da TEST-12), `git diff --numstat <base>` dà ≤ 2 righe aggiunte e ≤ 1 rimossa, e le righe toccate sono solo `Stato:` e `**Modifica:**` (`git diff -U0` filtrato: ogni riga `+`/`-` di contenuto inizia con uno dei due marcatori).

**TEST-8 — Link e numeri.** Il ciclo del verify del Task 3 sugli ADR 0003-0011 (ogni link `spikes/…\.(md|json)` esiste e ogni ADR ha un `hw.json`), esteso ai link a `.csv` (`grep -oE 'spikes/[A-Za-z0-9_./-]+\.(md|json|csv)'`). ADR-0012 collega RISULTATI e `lib.rs` di `s07-ports`, dichiara l'assenza di `hw.json` e contiene «Vincoli verso la Fondazione». Il SUMMARY elenca i numeri 0003-0013 con «emesso» o «non emesso» e motivo; ogni numero «emesso» corrisponde a un file esistente, ogni «non emesso» a nessun file.

**TEST-9 — Scanner.** `python skills/security-scanner/security_scan.py DEPENDENCIES.md docs/adr docs/security` esce 0; in più `grep -nE '(sk-|GK)[A-Za-z0-9]{16,}|postgres://[^ ]*:[^ ]*@' DEPENDENCIES.md docs/adr/*.md docs/security/*.md` vuoto.

**TEST-10 — Rinvio AV/CDR.** `docs/adr/0013-rinvio-antivirus-cdr.md` ha le sezioni Contesto, Opzioni considerate, Decisione, Conseguenze, Evidenze; i link a `docs/security/threat-model.md` e `spikes/s02-parser/results/isolation/hostile.csv` esistono; il threat model ha nella sezione «Rischio residuo» la voce AV/CDR con responsabile e motivazione e almeno una riga con stato `rinviata (ADR-0013)`. Lo `Stato` di ADR-0013 è coerente con la ratifica del Task 2.

**TEST-11 — Checklist (manuale).** Il verificatore legge la «Checklist specifica LLM/RAG»: ogni voce spuntata ha una riga della tabella con mitigazione corrispondente (il verificatore annota la coppia voce → T-NN); ogni voce non spuntata è lasciata tale o motivata nel rischio residuo. Non è automatizzabile in modo affidabile perché la corrispondenza è semantica.

## Copertura

I tre HIGH sono coperti da test automatici: RISK-1 da TEST-1, RISK-2 da TEST-2, RISK-3 da TEST-3 e TEST-4. RISK-9 deriva da una minaccia con disposizione `accept` di severità media, coerente con la regola di blocco del piano; TEST-10 verifica che l'accettazione sia documentata.

**Modifica:** Claude (test designer, Anthropic) · 2026-10-18 · test design 01-17

**Modifica:** Claude (Anthropic) · 2026-10-18 · reviewed_plan.sha256 aggiornato da 6c79fd6ec599… a cfe178147e8f… per la revisione del piano: evidenza di T-14 su file concreti (results/jev/forms.csv e results/rerank/rerank.csv) invece del glob. Il design non è stato rigenerato: va riletto contro la revisione.

**Modifica:** Claude (Anthropic) · 2026-10-19 · divisione di 01-07: misure ostili e budget aggregato spostati in 01-18 (decisione utente). reviewed_plan.sha256 aggiornato da cfe178147e8f… a 480b1053c716… (sha256 del piano con fine riga LF): test invariati, cambiano solo i rimandi (`hostile.csv` ora prodotto da 01-18, `depends_on` con 01-18; percorso del file invariato, quindi TEST-3 e TEST-10 restano validi).

**TEST-12 — Limiti noti (D-25).** `grep -q "^## Limiti noti" docs/adr/0004-parser-ocr-isolamento.md` e `grep -q "^## Limiti noti" README.md`; nella sezione di ADR-0004 compaiono `LZW` ed `estrapolat`; la sezione del README contiene il link `docs/adr/0004-parser-ocr-isolamento.md`. Sul diff di ADR-0004, oltre a `Stato` e `**Modifica:**`, le sole righe aggiunte stanno sotto `## Limiti noti`.

**Modifica:** Claude (Anthropic) · 2026-10-19 · limiti noti D-25 in ADR-0004 e README (decisione utente): reviewed_plan.sha256 aggiornato da 480b1053c716… a e99ab09adf51…, TEST-7 con eccezione ADR-0004, nuovo TEST-12.
