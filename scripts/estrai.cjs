#!/usr/bin/env node
// scripts/estrai.cjs — estrae da un artefatto di fase SOLO la parte che un agente deve valutare
// (carico minimo per plan-checker, verifier e gate).
//
// Il checker legge tutti i piani della fase e il verifier tutti i piani E tutti i SUMMARY: letti
// interi costano centinaia di KB. Ma cio' che valutano e' il frontmatter, il blocco <tasks>, il
// blocco `coverage`, la sezione di verifica. Questo script rende meccanico «leggi la parte
// strutturata, apri il corpo solo su un rosso».
// Il frontmatter si prende anche da gsd-tools:
//   node "$HOME/.claude/gsd-core/bin/gsd-tools.cjs" frontmatter get <file>
// Layout GSD 1.42.x: <dir-fase> = .planning/phases/NN-slug/ (o NN.N-slug/), file NN-MM-PLAN.md
// e NN-MM-SUMMARY.md. I fine riga CRLF sono normalizzati in lettura.
//
//   node scripts/estrai.cjs sezione <file> "<intestazione>"   # sezione markdown, fino alla prossima
//                                                              # intestazione di pari o superiore livello
//   node scripts/estrai.cjs blocco  <file> <tag>               # blocco <tag>…</tag> (es. tasks, objective)
//   node scripts/estrai.cjs piani   <dir-fase>                 # per ogni *-PLAN.md: frontmatter + <tasks>
//   node scripts/estrai.cjs summary <dir-fase>                 # per ogni *-SUMMARY.md: frontmatter + sezione di verifica
//
// Esce 2 se la sezione/il blocco non esiste: un'assenza deve essere visibile, mai un vuoto silenzioso.
const fs = require("fs");
const path = require("path");

function leggi(p) {
  return fs.readFileSync(p, "utf8").replace(/\r\n/g, "\n");
}

function frontmatter(testo) {
  const m = testo.match(/^---\n([\s\S]*?)\n---\n/);
  return m ? m[0] : "";
}

function sezione(testo, intestazione) {
  const righe = testo.split("\n");
  const inizio = righe.findIndex((r) => /^#{1,6} /.test(r) && r.replace(/^#+ /, "").trim() === intestazione.trim());
  if (inizio < 0) return null;
  const livello = righe[inizio].match(/^#+/)[0].length;
  let fine = righe.length;
  for (let i = inizio + 1; i < righe.length; i++) {
    const m = righe[i].match(/^(#{1,6}) /);
    if (m && m[1].length <= livello) { fine = i; break; }
  }
  return righe.slice(inizio, fine).join("\n");
}

function blocco(testo, tag) {
  const m = testo.match(new RegExp("<" + tag + "(?:\\s[^>]*)?>[\\s\\S]*?</" + tag + ">"));
  return m ? m[0] : null;
}

function perFile(dir, suffisso, estrattore) {
  const file = fs.readdirSync(dir).filter((f) => f.endsWith(suffisso)).sort();
  if (!file.length) { console.error(`estrai: nessun ${suffisso} in ${dir}`); process.exit(2); }
  const out = [];
  for (const f of file) {
    const testo = leggi(path.join(dir, f));
    out.push(`\n<!-- ===== ${f} ===== -->\n` + estrattore(testo, f));
  }
  return out.join("\n");
}

const [, , modo, arg1, arg2] = process.argv;
let esito = null;
switch (modo) {
  case "sezione":
    esito = sezione(leggi(arg1), arg2);
    if (esito === null) { console.error(`estrai: sezione «${arg2}» assente in ${arg1}`); process.exit(2); }
    break;
  case "blocco":
    esito = blocco(leggi(arg1), arg2);
    if (esito === null) { console.error(`estrai: blocco <${arg2}> assente in ${arg1}`); process.exit(2); }
    break;
  case "piani":
    esito = perFile(arg1, "-PLAN.md", (t, f) => {
      const tasks = blocco(t, "tasks");
      return frontmatter(t) + (tasks || `<!-- estrai: blocco <tasks> assente in ${f} -->`);
    });
    break;
  case "summary":
    esito = perFile(arg1, "-SUMMARY.md", (t, f) => {
      const ver = sezione(t, "Verifica del piano, punto per punto") || sezione(t, "Verification") || sezione(t, "Verifica")
        || sezione(t, "Deviations from Plan") || "";
      return frontmatter(t) + (ver || `<!-- estrai: nessuna sezione di verifica in ${f} -->`);
    });
    break;
  default:
    console.error("uso: estrai.cjs sezione <file> \"<intestazione>\" | blocco <file> <tag> | piani <dir> | summary <dir>");
    process.exit(2);
}
process.stdout.write(esito + "\n");

// **Modifica:** Claude (Anthropic) · 2026-10-18 · porting da Gestionale Light a CKA (decisione utente)
