#!/usr/bin/env python3
"""Pagine PDF di prova per lo stadio geometrico del locator (piano 01-09, Task 1).

Legge la fixture a mano `rect_cases.expected.json` e, per ogni caso, scrive un PDF di una
pagina con la MediaBox, la CropBox e il /Rotate del caso, un rettangolo pieno tracciato su
`user_rect` (operatori `re f`, coordinate esatte, nessuna metrica di font) e una stringa
marcatore in Helvetica. Accanto scrive `cases.json`: le voci della fixture copiate senza
modifiche (compreso `expected_viewport_rect`) più `pdf_file`.

Lo script non contiene alcuna formula per il rettangolo atteso: non lo ricalcola, lo copia.
Solo libreria standard; gira nel runner senza rete (`session.sh oneshot`, working_dir
`/src` = `spikes/`).

Uso: python3 s02-parser/locator-gen/gen_pages.py [--fixture F] [--out-dir D]

**Modifica:** Claude (gsd-executor 01-09) · 2026-10-22 · generatore delle pagine ruotate e ritagliate
"""

import argparse
import json
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_FIXTURE = os.path.join(HERE, "rect_cases.expected.json")
DEFAULT_OUT = os.path.join(HERE, "..", "results", "raw", "locator")


def num(v):
    """Numero PDF senza notazione esponenziale."""
    if float(v).is_integer():
        return str(int(v))
    return ("%.4f" % v).rstrip("0").rstrip(".")


def arr(values):
    return "[" + " ".join(num(v) for v in values) + "]"


def build_pdf(case):
    x0, y0, x1, y1 = case["user_rect"]
    marker = "MARK " + case["case_id"]
    # Marcatore sopra il rettangolo, dentro la CropBox di tutti i casi della fixture.
    content = (
        "0 0 0 rg\n"
        "%s %s %s %s re f\n"
        "BT /F1 12 Tf %s %s Td (%s) Tj ET\n"
        % (num(x0), num(y0), num(x1 - x0), num(y1 - y0), num(x0), num(y1 + 40), marker)
    ).encode("ascii")
    objects = [
        b"<< /Type /Catalog /Pages 2 0 R >>",
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        (
            "<< /Type /Page /Parent 2 0 R /MediaBox %s /CropBox %s /Rotate %d "
            "/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
            % (arr(case["mediabox"]), arr(case["cropbox"]), int(case["rotate"]))
        ).encode("ascii"),
        b"<< /Length %d >>\nstream\n" % len(content) + content + b"endstream",
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>",
    ]
    out = bytearray(b"%PDF-1.7\n")
    offsets = []
    for i, body in enumerate(objects, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n0000000000 65535 f \n" % (len(objects) + 1)
    for off in offsets:
        out += b"%010d 00000 n \n" % off
    out += b"trailer\n<< /Size %d /Root 1 0 R >>\nstartxref\n%d\n%%%%EOF\n" % (
        len(objects) + 1,
        xref,
    )
    return bytes(out)


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--fixture", default=DEFAULT_FIXTURE)
    ap.add_argument("--out-dir", default=DEFAULT_OUT)
    a = ap.parse_args()
    with open(a.fixture, encoding="utf-8") as f:
        fixture = json.load(f)
    os.makedirs(a.out_dir, exist_ok=True)
    cases = []
    for case in fixture["cases"]:
        name = case["case_id"] + ".pdf"
        with open(os.path.join(a.out_dir, name), "wb") as f:
            f.write(build_pdf(case))
        entry = dict(case)
        entry["pdf_file"] = name
        cases.append(entry)
    with open(os.path.join(a.out_dir, "cases.json"), "w", encoding="utf-8") as f:
        json.dump({"source": "locator-gen/rect_cases.expected.json", "cases": cases}, f, indent=2)
        f.write("\n")
    print("gen_pages: %d PDF in %s" % (len(cases), a.out_dir), file=sys.stderr)


if __name__ == "__main__":
    main()
