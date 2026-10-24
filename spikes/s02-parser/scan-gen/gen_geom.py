#!/usr/bin/env python3
"""Generatore delle pagine del controllo analitico delle bbox (piano 01-13, `reference=analytic`).

Scrive in `--out` 4 PDF di una pagina A4 (595 × 842 pt) con parole maiuscole e cifre in
Helvetica standard non incorporata (`/BaseFont /Helvetica`, `/WinAnsiEncoding`), a corpi 10,
14 e 24 pt e a posizioni note (inclusi i quattro angoli della pagina), disegnate con un solo
operatore `Tj` per parola e nessun kerning; accanto `geom-cases.json` con, per ogni parola, il
rettangolo d'inchiostro atteso in punti PDF (origine in basso a sinistra), calcolato
analiticamente dalle metriche AFM di Helvetica:

    x0 = x + s·llx(primo)/1000
    x1 = x + s·(Σ avanzamenti fino al penultimo + urx(ultimo))/1000
    y0 = y_base + s·min lly/1000
    y1 = y_base + s·max ury/1000

con `s` il corpo in punti, `(x, y_base)` l'origine del testo (`Tm`), `llx lly urx ury` il box
del glifo e `WX` l'avanzamento, in unità di 1/1000 del corpo. Lo svolgimento è copiato in
`RISULTATI-ocr.md` §Setup. Classe di provenienza `derivazione-documentata`.

Metriche: tabella copiata da `Helvetica.afm` di Adobe (Core14 AFM, «Copyright (c) 1985, 1987,
1989, 1990, 1997 Adobe Systems Incorporated», UniqueID 43054), nella copia distribuita con
Apache PDFBox (`pdfbox/src/main/resources/org/apache/pdfbox/resources/afm/Helvetica.afm`,
https://github.com/apache/pdfbox, consultata il 2026-10-24). Licenza delle AFM Core14: libera
copia e distribuzione per qualunque scopo, senza oneri, a condizione di mantenere le note di
copyright. Si copiano solo le righe `C` di cifre e maiuscole.

Solo standard library (`argparse`, `json`, `pathlib`). Deterministico. Gira nel runner:

    python3 /src/s02-parser/scan-gen/gen_geom.py --out /src/s02-parser/results/raw/geom/<run_id>

**Modifica:** Claude (gsd-executor 01-13) · 2026-10-24 · generatore delle pagine del controllo analitico (Task 2)
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

# Helvetica.afm (Adobe Core14): carattere → (WX, llx, lly, urx, ury).
AFM = {
    "0": (556, 37, -19, 519, 703),
    "1": (556, 101, 0, 359, 703),
    "2": (556, 26, 0, 507, 703),
    "3": (556, 34, -19, 522, 703),
    "4": (556, 25, 0, 523, 703),
    "5": (556, 32, -19, 514, 688),
    "6": (556, 38, -19, 518, 703),
    "7": (556, 37, 0, 523, 688),
    "8": (556, 38, -19, 517, 703),
    "9": (556, 42, -19, 514, 703),
    "A": (667, 14, 0, 654, 718),
    "B": (667, 74, 0, 627, 718),
    "C": (722, 44, -19, 681, 737),
    "D": (722, 81, 0, 674, 718),
    "E": (667, 86, 0, 616, 718),
    "F": (611, 86, 0, 583, 718),
    "G": (778, 48, -19, 704, 737),
    "H": (722, 77, 0, 646, 718),
    "I": (278, 91, 0, 188, 718),
    "J": (500, 17, -19, 428, 718),
    "K": (667, 76, 0, 663, 718),
    "L": (556, 76, 0, 537, 718),
    "M": (833, 73, 0, 761, 718),
    "N": (722, 76, 0, 646, 718),
    "O": (778, 39, -19, 739, 737),
    "P": (667, 86, 0, 622, 718),
    "Q": (778, 39, -56, 739, 737),
    "R": (722, 88, 0, 684, 718),
    "S": (667, 49, -19, 620, 737),
    "T": (611, 14, 0, 597, 718),
    "U": (722, 79, -19, 644, 718),
    "V": (667, 20, 0, 647, 718),
    "W": (944, 16, 0, 928, 718),
    "X": (667, 19, 0, 648, 718),
    "Y": (667, 14, 0, 653, 718),
    "Z": (611, 23, 0, 588, 718),
}

PAGE_W, PAGE_H = 595.0, 842.0
MARGIN = 36.0
# Parole senza discendenti (niente Q): maiuscole e cifre.
WORDS = ["ATTIVO", "TOTALE", "2026", "PASSIVO", "BILANCIO", "RICAVI", "NETTO", "1847",
         "UTILE", "CREDITI", "FONDI", "IMPOSTE"]


def advance(text: str, size: float) -> float:
    return size * sum(AFM[c][0] for c in text) / 1000.0


def ink_rect(text: str, x: float, y_base: float, size: float) -> list[float]:
    """Rettangolo d'inchiostro atteso (formule del docstring)."""
    first, last = AFM[text[0]], AFM[text[-1]]
    x0 = x + size * first[1] / 1000.0
    x1 = x + size * (sum(AFM[c][0] for c in text[:-1]) + last[3]) / 1000.0
    y0 = y_base + size * min(AFM[c][2] for c in text) / 1000.0
    y1 = y_base + size * max(AFM[c][4] for c in text) / 1000.0
    return [round(v, 4) for v in (x0, y0, x1, y1)]


def layout(sizes: list[float]) -> list[dict]:
    """Parole ai quattro angoli più una griglia di 3 colonne, corpi a rotazione."""
    out: list[dict] = []
    k = 0

    def put(text: str, x: float, y_base: float, size: float) -> None:
        out.append({"text": text, "x": round(x, 4), "y_base": round(y_base, 4), "size": size,
                    "rect": ink_rect(text, x, y_base, size)})

    s0 = sizes[0]
    top = PAGE_H - MARGIN - s0 * 0.718
    bottom = MARGIN + s0 * 0.019
    # Quattro angoli: sinistra a MARGIN, destra allineata a PAGE_W - MARGIN.
    for text, right, y in ((WORDS[0], False, top), (WORDS[1], True, top),
                           (WORDS[2], False, bottom), (WORDS[3], True, bottom)):
        x = PAGE_W - MARGIN - advance(text, s0) if right else MARGIN
        put(text, x, y, s0)
    # Griglia interna: righe ogni 3 corpi massimi, 3 colonne larghe 170 pt.
    step = 3.0 * max(sizes)
    y = top - step
    while y > bottom + step:
        for col in range(3):
            size = sizes[k % len(sizes)]
            text = WORDS[4 + k % (len(WORDS) - 4)]
            k += 1
            put(text, MARGIN + 30.0 + col * 175.0, y, size)
        y -= step
    return out


def pdf_bytes(words: list[dict]) -> bytes:
    """PDF 1.4 di una pagina, content stream non compresso, Helvetica non incorporata."""
    ops = []
    for w in words:
        ops.append("BT /F1 %s Tf 1 0 0 1 %s %s Tm (%s) Tj ET" % (
            fmt(w["size"]), fmt(w["x"]), fmt(w["y_base"]), w["text"]))
    content = ("\n".join(ops) + "\n").encode("ascii")
    objs = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        ("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 %s %s] /Resources << /Font << /F1 4 0 R >> >> "
         "/Contents 5 0 R >>" % (fmt(PAGE_W), fmt(PAGE_H))).encode("ascii"),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
        b"<< /Length %d >>\nstream\n" % len(content) + content + b"endstream",
    ]
    out = bytearray(b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n")
    offsets = []
    for i, body in enumerate(objs, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objs) + 1)
    for off in offsets:
        out += b"%010d 00000 n \n" % off
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (len(objs) + 1, xref)
    return bytes(out)


def fmt(v: float) -> str:
    s = ("%.4f" % v).rstrip("0").rstrip(".")
    return s if s else "0"


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--out", required=True, type=Path)
    args = ap.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    plans = {"geom-1": [24.0], "geom-2": [14.0], "geom-3": [10.0], "geom-4": [10.0, 14.0, 24.0]}
    pages = []
    for doc, sizes in plans.items():
        words = layout(sizes)
        (args.out / f"{doc}.pdf").write_bytes(pdf_bytes(words))
        pages.append({"doc": doc, "file": f"{doc}.pdf", "page_w_pt": PAGE_W, "page_h_pt": PAGE_H,
                      "sizes": sizes, "words": words})
    cases = {
        "provenance_class": "derivazione-documentata",
        "derivation_ref": "spikes/s02-parser/scan-gen/gen_geom.py + RISULTATI-ocr.md §Setup",
        "afm_source": "Helvetica.afm (Adobe Core14, UniqueID 43054) nella copia di Apache PDFBox",
        "formula": "x0 = x + s*llx(primo)/1000; x1 = x + s*(somma WX fino al penultimo + urx(ultimo))/1000; "
                   "y0 = y_base + s*min lly/1000; y1 = y_base + s*max ury/1000",
        "units": "punti PDF, origine in basso a sinistra",
        "pages": pages,
    }
    (args.out / "geom-cases.json").write_text(json.dumps(cases, indent=2, ensure_ascii=False) + "\n",
                                              encoding="utf-8")
    print("gen_geom: %d pagine, %d parole in %s" % (len(pages), sum(len(p["words"]) for p in pages),
                                                   args.out))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
