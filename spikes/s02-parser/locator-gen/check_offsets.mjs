#!/usr/bin/env node
// Verifica dei vettori di offset scalar -> UTF-16 (piano 01-09, Task 1). Node, nessuna
// dipendenza.
//
// Uso: node check_offsets.mjs --fixture <offset_vectors.expected.json> --got <offset_vectors.json>
//
// Il confronto è contro la FIXTURE scritta a mano, mai contro l'output del bin:
// 1. per ogni vettore della fixture, Array.from(text).slice(s, e).join('') e
//    text.slice(u16s, u16e) danno entrambi `expected` (la fixture è coerente con la
//    semantica JavaScript);
// 2. la riga del bin con lo stesso case_id (uno a uno, nessun mancante o duplicato) ha
//    got_utf16_range = utf16_range e got_text = expected, e nessun campo atteso.
// Exit 0 se tutto torna, 1 altrimenti, 2 per argomenti o file non validi.
//
// **Modifica:** Claude (gsd-executor 01-09) · 2026-10-22 · verificatore JavaScript dei vettori di offset

import { readFileSync } from 'node:fs';

function arg(name) {
  const i = process.argv.indexOf(name);
  return i > 0 ? process.argv[i + 1] : undefined;
}

const fixturePath = arg('--fixture');
const gotPath = arg('--got');
if (!fixturePath || !gotPath) {
  console.error('uso: check_offsets.mjs --fixture <file> --got <file>');
  process.exit(2);
}

let fixture, got;
try {
  fixture = JSON.parse(readFileSync(fixturePath, 'utf8'));
  const raw = JSON.parse(readFileSync(gotPath, 'utf8'));
  got = Array.isArray(raw) ? raw : raw.vectors;
} catch (e) {
  console.error(`file non valido: ${e.message}`);
  process.exit(2);
}

const failures = [];
const vectors = fixture.vectors ?? [];
const sameRange = (a, b) => Array.isArray(a) && Array.isArray(b) && a.length === 2 &&
  b.length === 2 && a[0] === b[0] && a[1] === b[1];

const byId = new Map();
for (const row of got ?? []) {
  if (byId.has(row.case_id)) failures.push(`${row.case_id}: riga duplicata nel bin`);
  byId.set(row.case_id, row);
  for (const k of ['utf16_range', 'expected']) {
    if (k in row) failures.push(`${row.case_id}: il bin emette il campo atteso ${k}`);
  }
}

for (const v of vectors) {
  const [s, e] = v.scalar_range;
  const [us, ue] = v.utf16_range;
  const byScalar = Array.from(v.text).slice(s, e).join('');
  const byUnits = v.text.slice(us, ue);
  if (byScalar !== v.expected) failures.push(`${v.case_id}: Array.from(...).slice != expected`);
  if (byUnits !== v.expected) failures.push(`${v.case_id}: String.prototype.slice != expected`);
  const row = byId.get(v.case_id);
  if (!row) {
    failures.push(`${v.case_id}: riga del bin mancante`);
    continue;
  }
  if (!sameRange(row.got_utf16_range, v.utf16_range)) {
    failures.push(`${v.case_id}: got_utf16_range ${JSON.stringify(row.got_utf16_range)} != ${JSON.stringify(v.utf16_range)}`);
  }
  if (row.got_text !== v.expected) failures.push(`${v.case_id}: got_text diverso da expected`);
}
for (const id of byId.keys()) {
  if (!vectors.some((v) => v.case_id === id)) failures.push(`${id}: riga del bin senza vettore`);
}

if (failures.length) {
  for (const f of failures) console.error(f);
  console.error(`check_offsets: FAIL (${failures.length} errori su ${vectors.length} vettori)`);
  process.exit(1);
}
console.log(`check_offsets: OK ${vectors.length}/${vectors.length} vettori (fixture coerente con Array.from e String.prototype.slice; bin uguale alla fixture)`);
