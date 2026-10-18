#!/usr/bin/env node
// scripts/gate-perimetro.cjs — gate `perimetro-check` del loop: niente ponti per le fasi successive.
// (Nato in Gestionale Light dopo la terza ricaduta dello stesso errore: un errore che si ripete
// si rende meccanico, non si affida a un'altra nota.)
//
// Regola: cio' che copre una fase successiva del ROADMAP compare nella fase corrente SOLO come
// segnaposto nudo — una riga, senza test dedicati, senza copy curata, senza accessibilita',
// senza colonne o migrazioni, senza `human_judgment`. In verify, un punto UAT o un finding su
// quell'area si scarta d'ufficio come «differito alla fase N».
// I seam INTER-FASE registrati in .planning/WIRING-MAP.md (trait-porta, registry, punto di
// montaggio del router Axum, segnaposto nudo) sono ammessi SOLO vuoti: il contenuto della fase
// futura resta vietato. Lo script li segnala comunque; e' l'agente-gate a riconoscerli come seam
// legittimi confrontandoli con la WIRING-MAP.
//
// ECCEZIONE PROVIDER-FIRST (decisione utente 2026-10-18, strategia «provider-first, insiemi di file
// chiusi per fase»): in una fase PROVIDER (adapter dietro i trait-porta: DocumentStore,
// DocumentParser, Chunker, Embedder, VectorIndex, DecisionEngine/JEV, AnswerGenerator, AuditSink,
// Cache) un'operazione del provider che serve a una fase consumatrice FUTURA e' ammessa completa,
// purche' citi il piano consumatore che la usa: `consumatori: [0X-YY, …]`. Un'operazione provider
// senza consumatore citato e' speculativa → da correggere. Nella Fondazione i seam inter-fase
// restano innesti vuoti. Lo script etichetta ogni riga `consumatori: [...]` con l'esistenza del
// piano citato (`pianificato` / `non pianificato`): al passaggio di riconciliazione, quando tutte
// le fasi consumatrici sono pianificate, un consumatore `non pianificato` e' da correggere.
//
// Lo script e' la parte meccanica: trova i rimandi. Il giudizio (rimando legittimo, per esempio
// «l'ingestion da fonti online NON e' qui, va alla fase 5», oppure ponte da togliere) resta
// all'agente-gate, che lo scrive nel GATE.md. Uno script che giudicasse da solo sbaglierebbe
// sulle negazioni.
//
// Layout GSD 1.42.x: cartelle di fase .planning/phases/NN-slug/ (intere) e NN.N-slug/ (decimali
// inserite); il ROADMAP intitola «### Phase N: Nome» o «### Phase N.M: Nome (INSERTED)».
// «02», «2», «02.1» e «2.1» sono normalizzati allo stesso id.
//
//   node scripts/gate-perimetro.cjs piani <dir-fase>          # ogni *-PLAN.md della fase,
//                                                             # piu' le righe `consumatori: [...]`
//   node scripts/gate-perimetro.cjs voci  <dir-fase>          # human_verification del VERIFICATION
//                                                             # e finding di GAPS/REVIEW della fase
//
// Esce 0 se non c'e' alcun rimando, 1 se c'e' almeno un rimando da giudicare, 2 su errore d'uso.
// Un rimando non giudicato non deve passare in silenzio: per questo l'uscita e' 1 e non 0.
const fs = require("fs");
const path = require("path");

const RADICE = path.resolve(__dirname, "..");
const ROADMAP = path.join(RADICE, ".planning", "ROADMAP.md");

// Parole che annunciano una soluzione ponte o un segnaposto rifinito.
// «fino alla» conta solo davanti a un numero di fase: «fino alla chip» non e' un ponte.
// Taratura ereditata da GL: «temporaneo», «nel frattempo», «per ora» davano solo rumore.
// Le forme inglesi coprono i piani GSD scritti in inglese, con lo stesso criterio.
const PAROLE_PONTE = [
  /\bfino all[ae']\s*(?:fase\s+)?[0-9]/i, /\bfinch[eé]\b/i, /\bponte\b/i, /\bprovvisori[oae]?\b/i,
  /\bsegnaposto\b/i, /\bplaceholder\b/i, /\bin attesa della fase\b/i,
  /\buntil phase\s+[0-9]/i, /\bstopgap\b/i,
];

/** Lettura con fine riga normalizzati: un file CRLF non deve rompere le ancore `^…$`. */
function leggi(p) {
  return fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
}

function esci(msg) {
  console.error(msg);
  process.exit(2);
}

/** Le fasi del ROADMAP nell'ordine in cui il documento le elenca (2.10 viene prima di 3). */
function fasi() {
  if (!fs.existsSync(ROADMAP)) esci(`ROADMAP assente: ${ROADMAP} (bootstrap GSD non ancora eseguito)`);
  const testo = leggi(ROADMAP);
  const out = [];
  const re = /^#{2,3} Phase ([0-9]+(?:\.[0-9]+)?): (.+)$/gm;
  let m;
  while ((m = re.exec(testo)) !== null) out.push({ id: m[1], nome: m[2].trim() });
  return out;
}

/** «02.1» e «2.1» sono la stessa fase; «3» e «03» anche. */
function normalizza(id) {
  return id.split(".").map((p) => String(Number(p))).join(".");
}

function idDellaCartella(dir) {
  const m = path.basename(dir).match(/^([0-9]+(?:\.[0-9]+)?)-/);
  if (!m) esci(`cartella di fase non riconosciuta: ${dir}`);
  return m[1];
}

/** Espressione che riconosce un rimando a una fase: «02.1», «2.1», «Fase 3», «Phase 3». */
function reDellaFase(id) {
  const [maggiore, minore] = normalizza(id).split(".");
  const m = `0?${maggiore}`;
  if (minore !== undefined) return new RegExp(`(?<![0-9.])${m}\\.${minore}(?![0-9])`);
  return new RegExp(`\\b(?:Fase|Phase|fase)\\s+${m}(?![0-9.])`);
}

function fasiSuccessive(idCorrente) {
  const elenco = fasi();
  const i = elenco.findIndex((f) => normalizza(f.id) === normalizza(idCorrente));
  if (i < 0) esci(`fase ${idCorrente} assente dal ROADMAP`);
  return elenco.slice(i + 1).map((f) => ({ ...f, re: reDellaFase(f.id) }));
}

/** Id dei piani gia' scritti in .planning/phases, normalizzati («05-02» → «5-2»). */
function pianiEsistenti() {
  const dirFasi = path.join(RADICE, ".planning", "phases");
  const out = new Set();
  if (!fs.existsSync(dirFasi)) return out;
  for (const d of fs.readdirSync(dirFasi)) {
    const pd = path.join(dirFasi, d);
    if (!fs.statSync(pd).isDirectory()) continue;
    for (const f of fs.readdirSync(pd)) {
      const m = f.match(/^([0-9]+(?:\.[0-9]+)?)-([0-9]+)-PLAN\.md$/);
      if (m) out.add(`${normalizza(m[1])}-${Number(m[2])}`);
    }
  }
  return out;
}

/** Etichette per una riga `consumatori: [0X-YY, …]` di un'operazione provider. */
function consumatori(riga, esistenti) {
  const m = riga.match(/\bconsumatori\s*:\s*\[([^\]]*)\]/i);
  if (!m) return [];
  const ids = m[1].split(",").map((x) => x.trim().replace(/^["'`]|["'`]$/g, "")).filter(Boolean);
  if (!ids.length) return ["consumatori vuoto"];
  return ids.map((id) => {
    const k = id.match(/^([0-9]+(?:\.[0-9]+)?)-([0-9]+)$/);
    const noto = k && esistenti.has(`${normalizza(k[1])}-${Number(k[2])}`);
    return `consumatore ${id} ${noto ? "pianificato" : "non pianificato"}`;
  });
}

/** Righe con un rimando a una fase successiva, una parola-ponte o una lista `consumatori:`. */
function rimandi(testo, successive, esistenti = new Set()) {
  const out = [];
  testo.split("\n").forEach((riga, n) => {
    const fase = successive.filter((f) => f.re.test(riga)).map((f) => f.id);
    const ponte = PAROLE_PONTE.filter((re) => re.test(riga)).map((re) => riga.match(re)[0].toLowerCase())
      .concat(consumatori(riga, esistenti));
    if (fase.length || ponte.length) out.push({ riga: n + 1, fase, ponte, testo: riga.trim().slice(0, 220) });
  });
  return out;
}

function stampa(file, trovati) {
  const rel = path.relative(RADICE, file).replace(/\\/g, "/");
  if (!trovati.length) {
    console.log(`${rel}: pulito`);
    return 0;
  }
  const conFase = trovati.filter((t) => t.fase.length).length;
  console.log(`${rel}: DA GIUDICARE — ${trovati.length} righe (${conFase} con rimando a fase successiva)`);
  for (const t of trovati) {
    const etichette = [...t.fase.map((f) => `fase ${f}`), ...t.ponte].join(", ");
    console.log(`  :${t.riga} [${etichette}] ${t.testo}`);
  }
  return 1;
}

function piani(dir) {
  const successive = fasiSuccessive(idDellaCartella(dir));
  const file = fs.readdirSync(dir).filter((f) => /-PLAN\.md$/.test(f)).sort();
  if (!file.length) esci(`nessun PLAN in ${dir}`);
  const esistenti = pianiEsistenti();
  let esito = 0;
  for (const f of file) {
    const p = path.join(dir, f);
    esito |= stampa(p, rimandi(leggi(p), successive, esistenti));
  }
  return esito;
}

/** Il blocco YAML `human_verification:` del frontmatter, fino alla chiave successiva. */
function humanVerification(testo) {
  const m = testo.match(/^human_verification:\n([\s\S]*?)(?=^[A-Za-z_]+:|^---$)/m);
  return m ? m[0] : "";
}

function voci(dir) {
  const successive = fasiSuccessive(idDellaCartella(dir));
  let esito = 0;
  for (const f of fs.readdirSync(dir).sort()) {
    const p = path.join(dir, f);
    if (/-VERIFICATION\.md$/.test(f)) {
      // Le righe del blocco mantengono la numerazione del file: si conta l'offset.
      const testo = leggi(p);
      const blocco = humanVerification(testo);
      const offset = testo.slice(0, testo.indexOf(blocco)).split("\n").length - 1;
      const trovati = rimandi(blocco, successive).map((t) => ({ ...t, riga: t.riga + offset }));
      esito |= stampa(p, trovati);
    } else if (/-(GAPS[^.]*|REVIEW[^.]*)\.md$/.test(f)) {
      esito |= stampa(p, rimandi(leggi(p), successive));
    }
  }
  return esito;
}

const [modo, dir] = process.argv.slice(2);
if (!modo || !dir) esci("uso: gate-perimetro.cjs piani|voci <dir-fase>");
if (!fs.existsSync(dir)) esci(`cartella inesistente: ${dir}`);
if (modo === "piani") process.exit(piani(dir));
if (modo === "voci") process.exit(voci(dir));
esci(`modo sconosciuto: ${modo}`);

// **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
// **Modifica:** Claude (Anthropic) · 2026-10-18 · strategia provider-first e insiemi di file chiusi (decisione utente)
