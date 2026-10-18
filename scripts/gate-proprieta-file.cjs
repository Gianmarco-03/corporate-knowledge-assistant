#!/usr/bin/env node
// scripts/gate-proprieta-file.cjs — gate `proprieta-file` del loop: insiemi di file chiusi per fase.
// (Decisione utente 2026-10-18, strategia «provider-first, insiemi di file chiusi per fase».)
//
// Regola: ogni fase possiede un insieme CHIUSO di file. Un file che compare nei `files_modified`
// di piani di PIU' FASI e' una violazione, salvo:
//   1. il file corrisponde a una voce della whitelist `scripts/proprieta-file.whitelist.json`
//      (registri append-only, artefatti rigenerati come Cargo.lock / package-lock.json / client TS
//      generato / .sqlx, documenti), con `tipo` = append | regenerate | shared-doc e un `motivo`;
//   2. il file e' un innesto pre-registrato della WIRING-MAP: riga `WM-NNN` non `retired` il cui
//      file e' creato vuoto dalla fase `created-in` (Fondazione) e riempito SOLO dalla
//      `fase-proprietaria`. Il passaggio fra esattamente queste due fasi e' ammesso; una terza
//      fase sullo stesso file e' violazione.
//   3. riapertura di gap-closure: un piano di una fase DECIMALE (es. 7.1) che dichiara nel frontmatter
//      `riapre_perimetro: [3]` puo' toccare i file della fase 3. E' cosi' che si corregge un difetto
//      in un file chiuso (ROADMAP «Strategia di consegna»), con la deviazione registrata e motivata.
//      Una fase intera che dichiara `riapre_perimetro` non ne gode: resta violazione.
// Dentro la STESSA fase piu' piani possono toccare lo stesso file: lo serializza `onde-slice`.
//
// Confronto glob-aware e bidirezionale, come `onde-slice`: `services/x/src/**` copre
// `services/x/src/a.rs` e viceversa; una voce che finisce con `/` vale come `<dir>/**`.
// La whitelist si valuta sulla voce PIU' SPECIFICA della coppia: `apps/web/**` contro
// `apps/web/package-lock.json` e' ammessa (il lockfile e' whitelisted), `apps/web/**` contro
// `apps/web/src/x.tsx` no.
//
// Lo script trova; il giudizio sul rimedio (chi tiene il file, come spezzarlo in un innesto per
// fase, deviazione registrata) resta all'agente-gate, che lo scrive nel GATE.md.
//
//   node scripts/gate-proprieta-file.cjs                       # tutti i piani del ROADMAP
//   node scripts/gate-proprieta-file.cjs --fase 05             # solo i conflitti che coinvolgono la fase 05
//   node scripts/gate-proprieta-file.cjs --root <dir>          # radice alternativa (alberi sintetici di test)
//   node scripts/gate-proprieta-file.cjs --whitelist <file>    # whitelist alternativa
//
// Uscita: 0 nessuna violazione · 1 almeno una violazione · 2 non eseguito (niente .planning, niente
// piani, fase senza piani, piano senza `files_modified`, whitelist illeggibile, errore d'uso).
// Uno scope vuoto non e' mai verde.
const fs = require("fs");
const path = require("path");

function esci(msg) {
  console.error(`proprieta-file: NON ESEGUITO — ${msg}`);
  process.exit(2);
}

// ---------- argomenti ----------
const argv = process.argv.slice(2);
let radice = path.resolve(__dirname, "..");
let whitelistPath = path.join(__dirname, "proprieta-file.whitelist.json");
let faseFiltro = null;
for (let i = 0; i < argv.length; i++) {
  const a = argv[i];
  const valore = () => {
    if (i + 1 >= argv.length) esci(`manca il valore di ${a}`);
    return argv[++i];
  };
  if (a === "--root") radice = path.resolve(valore());
  else if (a === "--whitelist") whitelistPath = path.resolve(valore());
  else if (a === "--fase") faseFiltro = valore();
  else esci(`argomento sconosciuto: ${a} (uso: [--fase NN] [--root <dir>] [--whitelist <file>])`);
}

/** Lettura con fine riga normalizzati (CRLF → LF). */
function leggi(p) {
  return fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
}

/** «02.1» e «2.1» sono la stessa fase; «3» e «03» anche. */
function normalizzaFase(id) {
  return String(id).split(".").map((p) => String(Number(p))).join(".");
}

function confrontaFasi(a, b) {
  const pa = normalizzaFase(a).split(".").map(Number);
  const pb = normalizzaFase(b).split(".").map(Number);
  for (let i = 0; i < Math.max(pa.length, pb.length); i++) {
    const d = (pa[i] || 0) - (pb[i] || 0);
    if (d) return d;
  }
  return 0;
}

function normalizzaPercorso(p) {
  let s = String(p).trim().replace(/^["'`]|["'`]$/g, "").replace(/\\/g, "/");
  s = s.replace(/^\.\//, "");
  if (s.endsWith("/")) s += "**";
  return s;
}

// ---------- glob ----------
const eGlob = (s) => /[*?[]/.test(s);

function globRe(glob) {
  let re = "";
  for (let i = 0; i < glob.length; i++) {
    const c = glob[i];
    if (c === "*") {
      if (glob[i + 1] === "*") {
        i++;
        if (glob[i + 1] === "/") { i++; re += "(?:.*/)?"; } else re += ".*";
      } else re += "[^/]*";
    } else if (c === "?") re += "[^/]";
    else re += c.replace(/[.+^${}()|[\]\\]/g, "\\$&");
  }
  return new RegExp(`^${re}$`);
}

/** Prefisso statico di un glob, tagliato all'ultima `/`. */
function prefissoStatico(glob) {
  const i = glob.search(/[*?[]/);
  if (i < 0) return glob;
  const pre = glob.slice(0, i);
  return pre.slice(0, pre.lastIndexOf("/") + 1);
}

/** Restituisce la voce piu' specifica se le due voci si sovrappongono, altrimenti null. */
function sovrapposizione(a, b) {
  const ga = eGlob(a), gb = eGlob(b);
  if (!ga && !gb) return a === b ? a : null;
  if (ga && !gb) return globRe(a).test(b) ? b : null;
  if (!ga && gb) return globRe(b).test(a) ? a : null;
  if (a === b) return a;
  if (globRe(a).test(b)) return b;
  if (globRe(b).test(a)) return a;
  const pa = prefissoStatico(a), pb = prefissoStatico(b);
  if (pa.startsWith(pb)) return a;
  if (pb.startsWith(pa)) return b;
  return null;
}

// ---------- whitelist ----------
function caricaWhitelist() {
  if (!fs.existsSync(whitelistPath)) esci(`whitelist assente: ${whitelistPath}`);
  let dati;
  try {
    dati = JSON.parse(leggi(whitelistPath));
  } catch (e) {
    esci(`whitelist non e' JSON valido: ${whitelistPath} (${e.message})`);
  }
  if (!Array.isArray(dati)) esci("la whitelist deve essere un array di {glob, tipo, motivo}");
  return dati.map((v, i) => {
    if (!v || typeof v.glob !== "string" || !v.glob.trim()) esci(`whitelist[${i}]: manca glob`);
    if (!["append", "regenerate", "shared-doc"].includes(v.tipo)) esci(`whitelist[${i}] (${v.glob}): tipo non valido «${v.tipo}»`);
    if (typeof v.motivo !== "string" || !v.motivo.trim()) esci(`whitelist[${i}] (${v.glob}): manca motivo`);
    const glob = normalizzaPercorso(v.glob);
    return { glob, tipo: v.tipo, re: globRe(glob) };
  });
}

// ---------- frontmatter ----------
function frontmatter(testo) {
  const m = testo.match(/^---\n([\s\S]*?)\n---(?:\n|$)/);
  return m ? m[1] : null;
}

/** Lista di frontmatter (`files_modified` per default) in forma inline `[a, b]` o a blocco `- a`. null = chiave assente. */
function filesModified(fm, chiave = /^files[_-]modified\s*:/) {
  const righe = fm.split("\n");
  const i = righe.findIndex((r) => chiave.test(r));
  if (i < 0) return null;
  const resto = righe[i].replace(chiave, "").replace(/\s+#.*$/, "").trim();
  if (resto.startsWith("[")) {
    // inline, eventualmente su piu' righe
    let corpo = resto;
    for (let j = i + 1; !corpo.includes("]") && j < righe.length; j++) corpo += " " + righe[j].trim();
    const interno = corpo.slice(1, corpo.indexOf("]"));
    return interno.split(",").map((s) => s.trim()).filter(Boolean).map(normalizzaPercorso);
  }
  if (resto) return [normalizzaPercorso(resto)];
  const out = [];
  for (let j = i + 1; j < righe.length; j++) {
    const m = righe[j].match(/^\s*-\s+(.+?)\s*(?:#.*)?$/);
    if (m) out.push(normalizzaPercorso(m[1]));
    else if (/^\S/.test(righe[j])) break;
    else if (righe[j].trim() === "") continue;
    else break;
  }
  return out;
}

// ---------- WIRING-MAP: innesti pre-registrati ----------
function innesti() {
  const p = path.join(radice, ".planning", "WIRING-MAP.md");
  if (!fs.existsSync(p)) return [];
  const righe = leggi(p).split("\n");
  const celle = (r) => r.trim().replace(/^\||\|$/g, "").split("|").map((c) => c.trim());
  const hi = righe.findIndex((r) => /^\|/.test(r) && /created-in/.test(r) && /fase-proprietaria/.test(r));
  if (hi < 0) return [];
  const h = celle(righe[hi]);
  const col = (nome) => h.findIndex((c) => c.toLowerCase().startsWith(nome));
  const cId = col("id"), cFile = col("file"), cCreata = col("created-in"), cProp = col("fase-proprietaria"), cStato = col("stato");
  const out = [];
  for (let i = hi + 1; i < righe.length && /^\|/.test(righe[i]); i++) {
    const c = celle(righe[i]);
    if (!/^WM-\d+/.test(c[cId] || "")) continue;
    if (cStato >= 0 && /retired/.test(c[cStato] || "")) continue;
    const prop = (c[cProp] || "").trim();
    if (!/^[0-9]+(?:\.[0-9]+)?$/.test(prop)) continue; // «—»: il file resta chiuso nella fase che lo crea
    const m = (c[cFile] || "").match(/`([^`]+)`/);
    if (!m) continue;
    const file = normalizzaPercorso(m[1].replace(/::.*$/, "").split(/\s+/)[0]);
    out.push({ id: c[cId], file, creata: normalizzaFase(c[cCreata]), proprietaria: normalizzaFase(prop) });
  }
  return out;
}

// ---------- raccolta piani ----------
const dirPlanning = path.join(radice, ".planning");
if (!fs.existsSync(dirPlanning)) esci(`.planning assente in ${radice}`);
const dirFasi = path.join(dirPlanning, "phases");
if (!fs.existsSync(dirFasi)) esci(`nessuna cartella .planning/phases in ${radice}`);

const voci = []; // {file, piano, fase}
const senzaFiles = [];
let nPiani = 0;
for (const d of fs.readdirSync(dirFasi).sort()) {
  const m = d.match(/^([0-9]+(?:\.[0-9]+)?)-/);
  const pd = path.join(dirFasi, d);
  if (!m || !fs.statSync(pd).isDirectory()) continue;
  const fase = normalizzaFase(m[1]);
  for (const f of fs.readdirSync(pd).filter((x) => /-PLAN\.md$/.test(x)).sort()) {
    nPiani++;
    const piano = f.replace(/-PLAN\.md$/, "");
    const fm = frontmatter(leggi(path.join(pd, f)));
    const files = fm === null ? null : filesModified(fm);
    if (!files || !files.length) {
      senzaFiles.push(`${d}/${f}`);
      continue;
    }
    // Fase decimale di gap-closure: `riapre_perimetro: [3]` riapre esplicitamente i file della fase 3.
    const riapre = new Set((filesModified(fm, /^riapre[_-]perimetro\s*:/) || []).map(normalizzaFase));
    for (const file of new Set(files)) voci.push({ file, piano, fase, riapre });
  }
}
if (!nPiani) esci(`nessun *-PLAN.md sotto ${dirFasi}`);
if (faseFiltro !== null) {
  const ff = normalizzaFase(faseFiltro);
  if (!voci.some((v) => v.fase === ff) && !senzaFiles.some((s) => normalizzaFase(s.split("-")[0]) === ff)) {
    esci(`la fase ${faseFiltro} non ha piani: scope vuoto`);
  }
}

const whitelist = caricaWhitelist();
const mappaInnesti = innesti();

// ---------- confronto a coppie fra fasi diverse ----------
const violazioni = new Map(); // specifico → {piani:Set, fasi:Set, via:Set}
const ammesse = new Map(); // specifico → motivo
const ff = faseFiltro === null ? null : normalizzaFase(faseFiltro);
for (let i = 0; i < voci.length; i++) {
  for (let j = i + 1; j < voci.length; j++) {
    const a = voci[i], b = voci[j];
    if (a.fase === b.fase) continue;
    if (ff !== null && a.fase !== ff && b.fase !== ff) continue;
    const spec = sovrapposizione(a.file, b.file);
    if (spec === null) continue;
    const wl = whitelist.find((w) => w.re.test(spec));
    if (wl) {
      ammesse.set(spec, `whitelist ${wl.glob} (${wl.tipo})`);
      continue;
    }
    const fasi = new Set([a.fase, b.fase]);
    const inn = mappaInnesti.find((r) => (r.file === spec || (eGlob(r.file) && globRe(r.file).test(spec)))
      && fasi.has(r.creata) && fasi.has(r.proprietaria) && r.creata !== r.proprietaria);
    if (inn) {
      ammesse.set(spec, `innesto ${inn.id}: creato vuoto in fase ${inn.creata}, riempito dalla fase ${inn.proprietaria}`);
      continue;
    }
    const gap = [[a, b], [b, a]].find(([x, y]) => x.fase.includes(".") && x.riapre.has(y.fase));
    if (gap) {
      ammesse.set(spec, `riapertura: fase decimale ${gap[0].fase} (${gap[0].piano}) riapre il perimetro della fase ${gap[1].fase} — deviazione da registrare con motivo`);
      continue;
    }
    if (!violazioni.has(spec)) violazioni.set(spec, { piani: new Set(), fasi: new Set(), via: new Set() });
    const v = violazioni.get(spec);
    for (const x of [a, b]) {
      v.piani.add(`${x.piano} (fase ${x.fase})`);
      v.fasi.add(x.fase);
      if (x.file !== spec) v.via.add(`${x.piano}: ${x.file}`);
    }
  }
}

// ---------- rapporto ----------
console.log(`proprieta-file: ${nPiani} piani, ${voci.length} voci files_modified${ff !== null ? `, filtro fase ${faseFiltro}` : ""}`);
for (const file of violazioni.keys()) ammesse.delete(file); // un file in violazione non e' «ammesso» a meta'
for (const [file, motivo] of [...ammesse].sort()) console.log(`ammesso   ${file} — ${motivo}`);
for (const [file, v] of [...violazioni].sort()) {
  const fasi = [...v.fasi].sort(confrontaFasi);
  const inn = mappaInnesti.find((r) => r.file === file);
  const suggerito = inn ? `fase ${inn.proprietaria} (fase-proprietaria di ${inn.id})` : `fase ${fasi[0]} (la prima)`;
  console.log(`VIOLAZIONE ${file}`);
  if (inn) console.log(`  innesto ${inn.id}: ammesse solo le fasi ${inn.creata} (crea vuoto) e ${inn.proprietaria} (riempie)`);
  console.log(`  piani: ${[...v.piani].sort().join(", ")}`);
  if (v.via.size) console.log(`  via glob: ${[...v.via].sort().join("; ")}`);
  console.log(`  proprietario suggerito: ${suggerito}`);
}
for (const s of senzaFiles) console.log(`NON VERIFICABILE ${s}: files_modified assente o vuoto`);

if (violazioni.size) {
  console.log(`esito: ${violazioni.size} ${violazioni.size === 1 ? "violazione" : "violazioni"}`);
  process.exit(1);
}
if (senzaFiles.length) {
  console.log(`esito: non eseguito — piani senza files_modified: ${senzaFiles.length}`);
  process.exit(2);
}
console.log("esito: conforme");
process.exit(0);

// **Modifica:** Claude (Anthropic) · 2026-10-18 · strategia provider-first e insiemi di file chiusi (decisione utente)
