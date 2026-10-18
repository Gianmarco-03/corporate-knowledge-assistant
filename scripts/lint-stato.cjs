#!/usr/bin/env node
// scripts/lint-stato.cjs — il guardiano del peso dello stato GSD.
//
// STATE.md e' letto INTERO da ~20 workflow GSD: ogni suo KB si paga a ogni verbo. La regola e'
// «indice, non archivio»: una decisione e' UNA riga (sommario + puntatore), il rationale vive
// nell'artefatto di fase. Questo script rende la regola meccanica. Esce 1 col motivo; 0 se tutto
// e' entro i limiti. Invocato da `verify` (banda ECC), dal bridge di `execute` e dagli hook
// .githooks/pre-commit e pre-push (solo se .planning/ esiste); eseguibile a mano.
// Layout GSD 1.42.x: .planning/STATE.md (sezione «### Decisions»), .planning/phases/NN-slug/.
const fs = require("fs");
const path = require("path");

const RADICE = path.resolve(__dirname, "..");
const LIMITI = { stateKb: 20, rigaDecisione: 300, indexRiga: 320 };
const errori = [];

function righeDecisioni(testo, intestazione) {
  const m = testo.match(new RegExp("^" + intestazione + "\\n([\\s\\S]*?)(?=^#{1,3} |(?![\\s\\S]))", "m"));
  if (!m) return [];
  return m[1].split("\n").filter((l) => l.startsWith("- "));
}

const statePath = path.join(RADICE, ".planning", "STATE.md");
if (fs.existsSync(statePath)) {
  const grezzo = fs.readFileSync(statePath, "utf8");
  const kb = Buffer.byteLength(grezzo, "utf8") / 1024;
  const testo = grezzo.replace(/\r\n/g, "\n");
  if (kb > LIMITI.stateKb) {
    errori.push(`STATE.md pesa ${kb.toFixed(1)} KB (limite ${LIMITI.stateKb}): sposta il rationale nell'artefatto di fase, lascia il sommario`);
  }
  for (const r of righeDecisioni(testo, "### Decisions")) {
    if (r.length > LIMITI.rigaDecisione) {
      errori.push(`STATE.md ### Decisions: voce di ${r.length} caratteri (limite ${LIMITI.rigaDecisione}): "${r.slice(0, 80)}..."`);
    }
  }
}

const indexPath = path.join(RADICE, ".planning", "DECISIONS-INDEX.md");
if (fs.existsSync(indexPath)) {
  const righe = fs.readFileSync(indexPath, "utf8").replace(/\r\n/g, "\n").split("\n").filter((l) => l.startsWith("- "));
  for (const r of righe) {
    if (r.length > LIMITI.indexRiga) {
      errori.push(`DECISIONS-INDEX.md: riga di ${r.length} caratteri (limite ${LIMITI.indexRiga}): "${r.slice(0, 80)}..."`);
    }
    if (!/→ `[^`]+`\s*$/.test(r)) {
      errori.push(`DECISIONS-INDEX.md: riga senza puntatore finale (-> \`file\`): "${r.slice(0, 80)}..."`);
    }
  }
}

// --- Artefatti di fase: AVVISI, non blocchi --------------------------------------------------
// Oltre un certo peso un PLAN o un SUMMARY e' solo prosa che i checker pagano a ogni lettura.
// Si MISURA: il tetto diventera' un rosso solo per decisione utente, quando i numeri lo
// confermano. Fino ad allora l'avviso non cambia l'uscita. `--fase <dir>` sceglie la fase
// (nome della cartella in .planning/phases/ o percorso assoluto); default: l'ultima.
const AVVISI = { planKb: 20, summaryKb: 15 };
const avvisi = [];
const argFase = process.argv.indexOf("--fase");
const dirFasi = path.join(RADICE, ".planning", "phases");
let dirFase = null;
if (argFase > -1 && process.argv[argFase + 1]) {
  dirFase = path.isAbsolute(process.argv[argFase + 1]) ? process.argv[argFase + 1] : path.join(dirFasi, process.argv[argFase + 1]);
} else if (fs.existsSync(dirFasi)) {
  const fasi = fs.readdirSync(dirFasi).filter((d) => fs.statSync(path.join(dirFasi, d)).isDirectory()).sort();
  if (fasi.length) dirFase = path.join(dirFasi, fasi[fasi.length - 1]);
}
if (dirFase && fs.existsSync(dirFase)) {
  const misure = [];
  for (const f of fs.readdirSync(dirFase).sort()) {
    if (!f.endsWith(".md")) continue;
    const kb = fs.statSync(path.join(dirFase, f)).size / 1024;
    if (f.endsWith("-PLAN.md") && kb > AVVISI.planKb) avvisi.push(`${f}: ${kb.toFixed(0)} KB (avviso oltre ${AVVISI.planKb})`);
    if (f.endsWith("-SUMMARY.md") && kb > AVVISI.summaryKb) avvisi.push(`${f}: ${kb.toFixed(0)} KB (avviso oltre ${AVVISI.summaryKb})`);
    if (/-(RESEARCH|PATTERNS|WIRING|UI-SPEC|CONTEXT|ONDE|CONTRATTO)\.md$/.test(f)) misure.push(`${f} ${kb.toFixed(0)} KB`);
  }
  if (misure.length) console.log(`lint-stato: intermedi di ${path.basename(dirFase)} — ${misure.join(" · ")}`);
}

if (errori.length) {
  console.error("lint-stato: ROSSO\n- " + errori.join("\n- "));
  process.exit(1);
}
if (avvisi.length) console.log("lint-stato: AVVISI (non bloccanti)\n- " + avvisi.join("\n- "));
console.log("lint-stato: verde (STATE.md e DECISIONS-INDEX.md entro i limiti)");

// **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
