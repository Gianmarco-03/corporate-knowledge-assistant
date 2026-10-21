# Derivazioni — tabelle italiane e pagine con tabelle (P-007…P-011)

Quattro tavole italiane con le celle attese e l'elenco delle pagine con tabelle dell'estratto TTQ da 25 pagine.

Ogni voce riporta documento, punto esatto, estratto verbatim e svolgimento. Nessun valore viene da parser, retrieval, JEV, LLM o altri risultati degli spike: i Markdown sintetici sono stati letti come file, i PDF come immagini delle pagine (rasterizzate a 110-150 dpi, nessuna estrazione di testo) lette a vista. Regole, classi e normalizzazione stanno in `../ORACOLI.md`.

**Modifica:** Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06 · 2026-10-21 · derivazione degli oracoli (piano 01-06, Task 1)

<a id="p-007"></a>
### P-007 · `pub-it-01-bdi-relazione-annuale-2025` pagina 12 (numerata 4) · Tavola 1.1 — PIL e inflazione nei principali paesi avanzati ed emergenti (valori percentuali)

- **Perché questa tabella:** tabella densa con intestazione su due livelli (PIL / Inflazione, poi anni) e righe di gruppo in grassetto; identica in der-it-01-relazione-annuale-2025-p1-25 pagina 12.
- **Intestazioni:** «PAESI», «PIL 2022», «PIL 2023», «PIL 2024», «PIL 2025», «Inflazione (1) 2022», «Inflazione (1) 2023», «Inflazione (1) 2024», «Inflazione (1) 2025».
- **Celle (riga per riga, col 1 = etichetta):**

| PAESI | PIL 2022 | PIL 2023 | PIL 2024 | PIL 2025 | Inflazione (1) 2022 | Inflazione (1) 2023 | Inflazione (1) 2024 | Inflazione (1) 2025 |
|---|---|---|---|---|---|---|---|---|
| Mondo | 3,8 | 3,3 | 3,4 | 3,4 | 8,7 | 6,7 | 5,8 | 4,1 |
| Paesi avanzati | 3,1 | 1,7 | 1,8 | 1,9 | 7,3 | 4,6 | 2,6 | 2,5 |
| Giappone | 1,3 | 0,7 | -0,2 | 1,1 | 2,5 | 3,3 | 2,7 | 3,2 |
| Regno Unito | 5,1 | 0,3 | 1,0 | 1,4 | 9,1 | 7,3 | 2,5 | 3,4 |
| Stati Uniti | 2,5 | 2,9 | 2,8 | 2,1 | 6,5 | 3,8 | 2,6 | 2,6 |
| Area dell’euro | 3,6 | 0,4 | 0,9 | 1,4 | 8,4 | 5,4 | 2,4 | 2,1 |
| Paesi emergenti e in via di sviluppo | 4,3 | 4,4 | 4,5 | 4,4 | 9,7 | 8,2 | 8,0 | 5,2 |
| Brasile | 3,0 | 3,2 | 3,4 | 2,3 | 9,3 | 4,6 | 4,4 | 5,0 |
| Cina | 3,1 | 5,4 | 5,0 | 5,0 | 2,0 | 0,2 | 0,2 | 0,0 |
| India (2) | 7,6 | 7,2 | 7,1 | 7,6 | 6,6 | 5,4 | 4,6 | 2,0 |
| Russia | -1,4 | 4,1 | 4,9 | 1,0 | 13,8 | 5,9 | 8,5 | 8,7 |

- **Svolgimento:** celle lette a vista dall'immagine della pagina a 150 dpi, colonna per colonna; nessun calcolo.
- **guardrail.claude:** concordo — Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06.

<a id="p-008"></a>
### P-008 · `pub-it-04-bdi-bollettino-economico-2026-3` pagina 17 (numerata 15) · Tavola 2 — PIL e inflazione nell’area dell’euro (variazioni percentuali)

- **Perché questa tabella:** tabella piccola dentro una pagina a due colonne; una cella vuota (riga «Area dell’euro al netto dell’Irlanda», colonna Inflazione) e un valore fra parentesi; l’etichetta di riga va a capo su due righe.
- **Intestazioni:** «PAESI», «Crescita del PIL 2025», «Crescita del PIL 2025 4° trim. (1)», «Crescita del PIL 2026 1° trim. (1)», «Inflazione 2026 giugno (2)».
- **Celle (riga per riga, col 1 = etichetta):**

| PAESI | Crescita del PIL 2025 | Crescita del PIL 2025 4° trim. (1) | Crescita del PIL 2026 1° trim. (1) | Inflazione 2026 giugno (2) |
|---|---|---|---|---|
| Francia | 0,8 | 0,2 | -0,1 | 2,0 |
| Germania | 0,2 | 0,2 | 0,3 | 2,4 |
| Italia | 0,5 | 0,3 | 0,3 | 3,0 |
| Spagna | 2,8 | 0,8 | 0,6 | 3,6 |
| Area dell’euro al netto dell’Irlanda | 1,0 | 0,4 | 0,3 | (vuota) |
| Area dell’euro | 1,4 | 0,2 | -0,2 | (2,8) |

- **Svolgimento:** celle lette a vista dall'immagine della pagina a 150 dpi, colonna per colonna; nessun calcolo.
- **guardrail.claude:** concordo — Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06.

<a id="p-009"></a>
### P-009 · `pub-it-04-bdi-bollettino-economico-2026-3` pagina 26 (numerata 24) · Tavola 3 — PIL e principali componenti (1) (variazioni percentuali sul periodo precedente e punti percentuali)

- **Perché questa tabella:** tabella senza griglia con righe rientrate (sottovoci) e intestazione con anno su due livelli.
- **Intestazioni:** «VOCI», «2025», «2025 2° trim.», «2025 3° trim.», «2025 4° trim.», «2026 1° trim.».
- **Celle (riga per riga, col 1 = etichetta):**

| VOCI | 2025 | 2025 2° trim. | 2025 3° trim. | 2025 4° trim. | 2026 1° trim. |
|---|---|---|---|---|---|
| PIL | 0,5 | 0,0 | 0,2 | 0,3 | 0,3 |
| Importazioni di beni e servizi | 3,6 | 0,2 | 2,0 | 1,3 | -0,7 |
| Domanda nazionale (2) | 1,3 | 0,6 | 0,1 | 1,1 | -0,7 |
| Consumi nazionali | 0,9 | 0,3 | 0,3 | 0,1 | 0,4 |
| spesa delle famiglie (3) | 1,1 | 0,3 | 0,3 | 0,1 | 0,5 |
| spesa delle Amministrazioni pubbliche | 0,6 | 0,4 | 0,1 | 0,2 | 0,0 |
| Investimenti fissi lordi | 3,5 | 1,6 | 1,3 | 1,0 | 0,7 |
| costruzioni | 3,3 | 1,5 | 0,8 | 1,9 | -0,4 |
| beni strumentali (4) | 3,6 | 1,7 | 1,7 | 0,0 | 1,8 |
| Variazioni delle scorte (5) | -0,2 | 0,0 | -0,4 | 0,7 | -1,1 |
| Esportazioni di beni e servizi | 1,2 | -1,7 | 2,2 | -1,0 | 2,2 |
| Esportazioni nette (6) | -0,7 | -0,6 | 0,1 | -0,7 | 0,9 |

- **Svolgimento:** celle lette a vista dall'immagine della pagina a 150 dpi, colonna per colonna; nessun calcolo.
- **guardrail.claude:** concordo — Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06.

<a id="p-010"></a>
### P-010 · `pub-it-04-bdi-bollettino-economico-2026-3` pagina 41 (numerata 39) · Tavola 7 — DFP 2026: quadro tendenziale (aprile 2026) (1) (in percentuale del PIL, se non diversamente specificato)

- **Perché questa tabella:** tabella con righe spaziate; la nota (1) avverte che «eventuali mancate quadrature sono dovute agli arrotondamenti».
- **Intestazioni:** «VOCI», «2025», «2026», «2027», «2028», «2029».
- **Celle (riga per riga, col 1 = etichetta):**

| VOCI | 2025 | 2026 | 2027 | 2028 | 2029 |
|---|---|---|---|---|---|
| Indebitamento netto | 3,1 | 2,9 | 2,8 | 2,5 | 2,1 |
| Avanzo primario | 0,8 | 1,2 | 1,5 | 1,8 | 2,4 |
| Spesa per interessi | 3,9 | 4,1 | 4,2 | 4,4 | 4,5 |
| Debito (2) | 137,1 | 138,6 | 138,5 | 137,9 | 136,3 |
| Crescita spesa netta (3) | 1,9 | 1,6 | 2,2 | 1,7 | 1,5 |
| Crescita spesa netta secondo il PSBMT 2025-29 (3) | 1,3 | 1,6 | 1,9 | 1,7 | 1,5 |

- **Svolgimento:** celle lette a vista dall'immagine della pagina a 150 dpi, colonna per colonna; nessun calcolo.
- **guardrail.claude:** concordo — Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06.

<a id="p-011"></a>
### P-011 · `der-it-01-relazione-annuale-2025-p1-25` · pagine con tabelle (25 pagine)

- **Regola:** una pagina contiene una tabella se contiene almeno una tavola di dati in righe e colonne con intestazioni (in questo documento le tavole sono etichettate «Tavola»). Non sono tabelle i grafici (etichetta «Figura»), l'indice (elenco di titoli con numero di pagina) e l'elenco dei segni convenzionali.
- **Pagine controllate una per una** (immagine a 110 dpi):

  - 1: copertina (fotografia, testata, titolo)
  - 2: pagina bianca
  - 3: frontespizio
  - 4: colophon: indirizzo, ISSN, avvertenze editoriali, testo libero
  - 5: Indice (prima parte): elenco di titoli con numeri di pagina, non una tavola
  - 6: Indice (seconda parte)
  - 7: Indice (terza parte)
  - 8: Avvertenze: elenco dei segni convenzionali, non una tavola
  - 9: occhiello «L’ECONOMIA INTERNAZIONALE»
  - 10: pagina bianca
  - 11: testo (inizio capitolo 1)
  - 12: Tavola 1.1 «PIL e inflazione nei principali paesi avanzati ed emergenti»
  - 13: testo e riquadro
  - 14: Figura del riquadro (grafico a barre) e testo
  - 15: testo del riquadro
  - 16: testo del riquadro e testo
  - 17: Figura 1.1 (grafici) e testo
  - 18: Figura 1.2 (grafici) e testo
  - 19: Figura 1.3 (grafici) e testo
  - 20: Figura 1.4 (grafici) e testo
  - 21: Figure 1.5 e 1.6 (grafici)
  - 22: Figura 1.7 (grafici) e testo
  - 23: testo
  - 24: testo e riquadro
  - 25: Figura del riquadro (grafici a barre) e testo

- **Valore atteso:** pagine con tabelle = 12 (una sola pagina su 25).
- **Conseguenza per la misura:** la recall dell'euristica che sceglie le pagine per Docling (D-07) si misura su una sola pagina positiva; le pagine 5-8 (indice e avvertenze) sono negative difficili, perché hanno un'impaginazione a colonne allineate.
- **guardrail.claude:** concordo — Claude Opus 5.5 (Anthropic), agente derivatore della sessione gsd-executor 01-06.
