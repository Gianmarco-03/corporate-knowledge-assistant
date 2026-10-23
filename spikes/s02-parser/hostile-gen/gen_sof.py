#!/usr/bin/env python3
"""Generatore dei casi ostili della gap-closure 1.5 (piano 01.5-01, D-04, D-09).

Scrive 7 PDF di una pagina in `--out` (nel repo `spikes/data/hostile/generated/sof/`,
ignorata da git): 5 con un'immagine `DCTDecode` il cui marker SOF non coincide con
`/Width` e `/Height` dichiarati (o manca), 2 con un'immagine `JPXDecode` il cui marker SIZ
è enorme, nel codestream nudo e nel box `jp2c` di un file JP2. Gira nel runner, dentro una
sessione di `session.sh`:

    bash spikes/tools/session.sh oneshot "$CKA_RUN_ID" -- \
        python3 /src/s02-parser/hostile-gen/gen_sof.py \
        --out /src/data/hostile/generated/sof --manifest /src/data/hostile/sof/manifest.json

Solo standard library (`argparse`, `base64`, `hashlib`, `json`, `struct`, `zlib`).
Deterministico: niente orari, niente casualità, niente `qpdf`.

Il generatore controlla soltanto la propria costruzione: rilegge il SOF o il SIZ dai byte
che ha appena scritto, le lunghezze dei segmenti JPEG e dei box JP2, i prefiltri
(ASCII85, Flate) e gli offset della xref. Non conosce l'esito del lanciatore: quello sta
nel manifest della 1.5, scritto prima della misura.

Con `--manifest` scrive lo sha256 dei soli casi che hanno `"sha256": null` e riscrive il
file con `json.dumps(…, indent=2, ensure_ascii=False) + "\\n"`; con `--verify-only` non
genera nulla e confronta i file già presenti con gli sha256 del manifest (exit 1 se uno
manca o differisce); con `--only CASE` genera un solo caso.

**Modifica:** Claude (agente test-write 01.5-01) · 2026-10-23 · generatore dei 7 casi ostili SOF e SIZ della gap-closure 1.5
"""

from __future__ import annotations

import argparse
import base64
import hashlib
import json
import struct
import sys
import zlib
from pathlib import Path
from typing import Callable

MIB = 1024 * 1024


class BuildError(Exception):
    """La costruzione di un caso non rispetta la propria specifica."""


# ---- JPEG (ITU-T T.81, Annex K) ----------------------------------------------------------

SOI = b"\xff\xd8"
EOI = b"\xff\xd9"

# Tabella di quantizzazione della luminanza (Annex K.1), in ordine naturale.
LUMA_Q = [
    16, 11, 10, 16, 24, 40, 51, 61,
    12, 12, 14, 19, 26, 58, 60, 55,
    14, 13, 16, 24, 40, 57, 69, 56,
    14, 17, 22, 29, 51, 87, 80, 62,
    18, 22, 37, 56, 68, 109, 103, 77,
    24, 35, 55, 64, 81, 104, 113, 92,
    49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
]

# Tabelle di Huffman tipiche (Annex K.3): (classe, id, BITS, HUFFVAL).
DC_LUMA_BITS = [0, 1, 5, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0, 0, 0]
DC_CHROMA_BITS = [0, 3, 1, 1, 1, 1, 1, 1, 1, 1, 1, 0, 0, 0, 0, 0]
DC_VALS = list(range(12))
AC_LUMA_BITS = [0, 2, 1, 3, 3, 2, 4, 3, 5, 5, 4, 4, 0, 0, 1, 0x7D]
AC_LUMA_VALS = bytes.fromhex(
    """
    01 02 03 00 04 11 05 12 21 31 41 06 13 51 61 07
    22 71 14 32 81 91 a1 08 23 42 b1 c1 15 52 d1 f0
    24 33 62 72 82 09 0a 16 17 18 19 1a 25 26 27 28
    29 2a 34 35 36 37 38 39 3a 43 44 45 46 47 48 49
    4a 53 54 55 56 57 58 59 5a 63 64 65 66 67 68 69
    6a 73 74 75 76 77 78 79 7a 83 84 85 86 87 88 89
    8a 92 93 94 95 96 97 98 99 9a a2 a3 a4 a5 a6 a7
    a8 a9 aa b2 b3 b4 b5 b6 b7 b8 b9 ba c2 c3 c4 c5
    c6 c7 c8 c9 ca d2 d3 d4 d5 d6 d7 d8 d9 da e1 e2
    e3 e4 e5 e6 e7 e8 e9 ea f1 f2 f3 f4 f5 f6 f7 f8
    f9 fa
    """
)
AC_CHROMA_BITS = [0, 2, 1, 2, 4, 4, 3, 4, 7, 5, 4, 4, 0, 1, 2, 0x77]
AC_CHROMA_VALS = bytes.fromhex(
    """
    00 01 02 03 11 04 05 21 31 06 12 41 51 07 61 71
    13 22 32 81 08 14 42 91 a1 b1 c1 09 23 33 52 f0
    15 62 72 d1 0a 16 24 34 e1 25 f1 17 18 19 1a 26
    27 28 29 2a 35 36 37 38 39 3a 43 44 45 46 47 48
    49 4a 53 54 55 56 57 58 59 5a 63 64 65 66 67 68
    69 6a 73 74 75 76 77 78 79 7a 82 83 84 85 86 87
    88 89 8a 92 93 94 95 96 97 98 99 9a a2 a3 a4 a5
    a6 a7 a8 a9 aa b2 b3 b4 b5 b6 b7 b8 b9 ba c2 c3
    c4 c5 c6 c7 c8 c9 ca d2 d3 d4 d5 d6 d7 d8 d9 da
    e2 e3 e4 e5 e6 e7 e8 e9 ea f2 f3 f4 f5 f6 f7 f8
    f9 fa
    """
)
HUFFMAN_TABLES = [
    (0, 0, DC_LUMA_BITS, bytes(DC_VALS)),
    (1, 0, AC_LUMA_BITS, AC_LUMA_VALS),
    (0, 1, DC_CHROMA_BITS, bytes(DC_VALS)),
    (1, 1, AC_CHROMA_BITS, AC_CHROMA_VALS),
]

# Marker SOFn (C4 = DHT, C8 = JPG e CC = DAC non lo sono).
SOF_MARKERS = {0xC0, 0xC1, 0xC2, 0xC3, 0xC5, 0xC6, 0xC7, 0xC9, 0xCA, 0xCB, 0xCD, 0xCE, 0xCF}


def zigzag_order() -> list[int]:
    """Indici in ordine naturale delle 64 posizioni lette a zig-zag."""
    order = []
    for s in range(15):
        cells = [(i, s - i) for i in range(8) if 0 <= s - i < 8]
        if s % 2 == 0:
            cells.reverse()
        order.extend(r * 8 + c for r, c in cells)
    return order


def segment(marker: int, payload: bytes) -> bytes:
    if len(payload) + 2 > 0xFFFF:
        raise BuildError(f"segmento {marker:02X} troppo lungo")
    return bytes([0xFF, marker]) + struct.pack(">H", len(payload) + 2) + payload


def check_huffman_tables() -> None:
    ac_symbols = {0x00, 0xF0} | {(r << 4) | s for r in range(16) for s in range(1, 11)}
    for cls, ident, bits, vals in HUFFMAN_TABLES:
        if len(bits) != 16 or sum(bits) != len(vals) or len(set(vals)) != len(vals):
            raise BuildError(f"tabella di Huffman {cls}/{ident} incoerente")
        if cls == 1 and set(vals) != ac_symbols:
            raise BuildError(f"tabella AC {ident}: simboli diversi da Annex K")


def jpeg(width: int, height: int, comps: int, with_sof: bool = True) -> bytes:
    """SOI, DQT, SOF0, DHT con le 4 tabelle dell'Annex K, SOS, 64 byte a zero, EOI."""
    check_huffman_tables()
    zz = zigzag_order()
    out = bytearray(SOI)
    out += segment(0xDB, bytes([0x00]) + bytes(LUMA_Q[i] for i in zz))
    if with_sof:
        sof = struct.pack(">BHHB", 8, height, width, comps)
        for c in range(comps):
            sof += bytes([c + 1, 0x11, 0])
        out += segment(0xC0, sof)
    dht = bytearray()
    for cls, ident, bits, vals in HUFFMAN_TABLES:
        dht += bytes([(cls << 4) | ident]) + bytes(bits) + vals
    out += segment(0xC4, bytes(dht))
    sos = bytes([comps])
    for c in range(comps):
        sos += bytes([c + 1, 0x00 if c == 0 else 0x11])
    out += segment(0xDA, sos + bytes([0, 63, 0]))
    out += bytes(64)
    out += EOI
    return bytes(out)


def reread_sof(data: bytes) -> tuple[int, int, int] | None:
    """Rilegge per segmenti il primo SOF della costruzione: (larghezza, altezza, Nf)."""
    if data[:2] != SOI:
        raise BuildError("JPEG senza SOI")
    i = 2
    while i + 4 <= len(data):
        if data[i] != 0xFF:
            raise BuildError(f"byte {i}: atteso un marker")
        marker = data[i + 1]
        if marker in (0xD9, 0xDA):
            return None
        length = struct.unpack(">H", data[i + 2 : i + 4])[0]
        if length < 2 or i + 2 + length > len(data):
            raise BuildError(f"segmento {marker:02X} con lunghezza {length} fuori dai dati")
        if marker in SOF_MARKERS:
            _p, h, w, nf = struct.unpack(">BHHB", data[i + 4 : i + 10])
            if length != 8 + 3 * nf:
                raise BuildError("SOF con lunghezza incoerente")
            return (w, h, nf)
        i += 2 + length
    raise BuildError("JPEG finito prima di SOS")


# ---- JPEG 2000 (ITU-T T.800) -------------------------------------------------------------

SOC = b"\xff\x4f"
EOC = b"\xff\xd9"
JP2_SIGNATURE = b"\x00\x00\x00\x0c\x6a\x50\x20\x20\x0d\x0a\x87\x0a"


def jpx_codestream(width: int, height: int, comps: int) -> bytes:
    """SOC, SIZ (un solo tile, 8 bit senza segno), COD e QCD minimi, EOC."""
    siz = struct.pack(">H", 0)  # Rsiz
    siz += struct.pack(">IIIIIIII", width, height, 0, 0, width, height, 0, 0)
    siz += struct.pack(">H", comps)
    siz += bytes([7, 1, 1]) * comps
    cod = bytes([0, 0]) + struct.pack(">H", 1) + bytes([0]) + bytes([0, 4, 4, 0, 1])
    qcd = bytes([0x40, 0x40])
    out = bytearray(SOC)
    out += b"\xff\x51" + struct.pack(">H", len(siz) + 2) + siz
    out += b"\xff\x52" + struct.pack(">H", len(cod) + 2) + cod
    out += b"\xff\x5c" + struct.pack(">H", len(qcd) + 2) + qcd
    out += EOC
    return bytes(out)


def box(tbox: bytes, payload: bytes) -> bytes:
    return struct.pack(">I", len(payload) + 8) + tbox + payload


def jp2_file(width: int, height: int, comps: int, codestream: bytes) -> bytes:
    """Firma, `ftyp` (`jp2 `), `jp2h` con `ihdr` e `colr` enumerato sRGB, poi `jp2c`."""
    ftyp = box(b"ftyp", b"jp2 " + struct.pack(">I", 0) + b"jp2 ")
    ihdr = box(b"ihdr", struct.pack(">IIHBBBB", height, width, comps, 7, 7, 0, 0))
    colr = box(b"colr", bytes([1, 0, 0]) + struct.pack(">I", 16))
    jp2h = box(b"jp2h", ihdr + colr)
    return JP2_SIGNATURE + ftyp + jp2h + box(b"jp2c", codestream)


def reread_siz(data: bytes) -> tuple[int, int, int]:
    """Rilegge il SIZ della costruzione (codestream nudo o primo `jp2c`): (W, H, Csiz)."""
    if data.startswith(JP2_SIGNATURE):
        i = len(JP2_SIGNATURE)
        while True:
            if i + 8 > len(data):
                raise BuildError("JP2 senza jp2c")
            lbox = struct.unpack(">I", data[i : i + 4])[0]
            tbox = data[i + 4 : i + 8]
            if lbox < 8 or i + lbox > len(data):
                raise BuildError(f"box {tbox!r} con lunghezza {lbox} fuori dai dati")
            if tbox == b"jp2c":
                data = data[i + 8 : i + lbox]
                break
            i += lbox
    if data[:2] != SOC or data[2:4] != b"\xff\x51":
        raise BuildError("codestream senza SOC seguito da SIZ")
    lsiz = struct.unpack(">H", data[4:6])[0]
    xs, ys, xo, yo = struct.unpack(">IIII", data[8:24])
    csiz = struct.unpack(">H", data[40:42])[0]
    if lsiz != 38 + 3 * csiz:
        raise BuildError("SIZ con Lsiz incoerente")
    return (xs - xo, ys - yo, csiz)


# ---- PDF ---------------------------------------------------------------------------------


def stream(dict_extra: str, data: bytes) -> bytes:
    head = f"<< /Length {len(data)} {dict_extra} >>\nstream\n".encode()
    return head + data + b"\nendstream"


def build_pdf(objs: list[tuple[int, bytes]]) -> bytes:
    """PDF 1.7 di una pagina con xref classica e offset calcolati; l'oggetto 1 è il Catalog."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets: dict[int, int] = {}
    for num, body in sorted(objs, key=lambda o: o[0]):
        offsets[num] = len(out)
        out += f"{num} 0 obj\n".encode() + body + b"\nendobj\n"
    size = max(offsets) + 1
    xref = len(out)
    out += f"xref\n0 {size}\n0000000000 65535 f \n".encode()
    for i in range(1, size):
        if i in offsets:
            out += f"{offsets[i]:010d} 00000 n \n".encode()
        else:
            out += b"0000000000 65535 f \n"
    out += f"trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n".encode()
    pdf = bytes(out)
    for num, off in offsets.items():
        if not pdf.startswith(f"{num} 0 obj\n".encode(), off):
            raise BuildError(f"xref: offset dell'oggetto {num} sbagliato")
    if not pdf.startswith(b"xref\n", xref):
        raise BuildError("startxref sbagliato")
    return pdf


def image_pdf(dict_extra: str, data: bytes) -> bytes:
    """Una pagina che disegna l'XObject `/Im1` (oggetto 5, `/Subtype /Image`)."""
    return build_pdf(
        [
            (1, b"<< /Type /Catalog /Pages 2 0 R >>"),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                b"/Resources << /XObject << /Im1 5 0 R >> >> /Contents 4 0 R >>",
            ),
            (4, stream("", b"q 100 0 0 100 0 0 cm /Im1 Do Q")),
            (5, stream(f"/Type /XObject /Subtype /Image {dict_extra}", data)),
        ]
    )


def thumb_pdf(dict_extra: str, data: bytes) -> bytes:
    """Forma T di 01.4-02: stream 5 senza `/Subtype` riferito da `/Thumb` della pagina,
    `/ColorSpace` indiretto verso l'oggetto 6 (`/DeviceRGB`), come in OTC_NSANY_2004."""
    return build_pdf(
        [
            (1, b"<< /Type /Catalog /Pages 2 0 R >>"),
            (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
            (
                3,
                b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                b"/Contents 4 0 R /Thumb 5 0 R >>",
            ),
            (4, stream("", b"BT /F1 12 Tf 72 712 Td (Ciao) Tj ET")),
            (5, stream(dict_extra, data)),
            (6, b"/DeviceRGB"),
        ]
    )


def a85(data: bytes) -> bytes:
    return base64.a85encode(data) + b"~>"


def check_sof(jpg: bytes, coded: tuple[int, int, int] | None) -> bytes:
    found = reread_sof(jpg)
    if found != coded:
        raise BuildError(f"SOF riletto {found}, costruito {coded}")
    return jpg


def check_siz(data: bytes, coded: tuple[int, int, int]) -> bytes:
    found = reread_siz(data)
    if found != coded:
        raise BuildError(f"SIZ riletto {found}, costruito {coded}")
    return data


# ---- casi --------------------------------------------------------------------------------

HUGE = (30000, 30000, 3)


def case_dct_sof_huge_image() -> bytes:
    jpg = check_sof(jpeg(*HUGE), HUGE)
    return image_pdf(
        "/Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /DCTDecode", jpg
    )


def case_dct_sof_huge_thumb() -> bytes:
    jpg = check_sof(jpeg(*HUGE), HUGE)
    enc = a85(jpg)
    if base64.a85decode(enc[:-2]) != jpg:
        raise BuildError("ASCII85 non reversibile")
    return thumb_pdf(
        "/BitsPerComponent 8 /ColorSpace 6 0 R /Filter [/ASCII85Decode /DCTDecode] "
        "/Width 74 /Height 105",
        enc,
    )


def case_dct_sof_flate_huge() -> bytes:
    jpg = check_sof(jpeg(*HUGE), HUGE)
    packed = zlib.compress(jpg, 6)
    if zlib.decompress(packed) != jpg:
        raise BuildError("Flate non reversibile")
    return image_pdf(
        "/Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 "
        "/Filter [/FlateDecode /DCTDecode]",
        packed,
    )


def case_dct_sof_larger_within_cap() -> bytes:
    coded = (4000, 4000, 3)
    if coded[0] * coded[1] * coded[2] >= 64 * MIB:
        raise BuildError("il SOF deve restare entro il tetto di 64 MiB")
    jpg = check_sof(jpeg(*coded), coded)
    return image_pdf(
        "/Width 16 /Height 16 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode", jpg
    )


def case_dct_sof_missing() -> bytes:
    jpg = check_sof(jpeg(8, 8, 3, with_sof=False), None)
    return image_pdf(
        "/Width 8 /Height 8 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Filter /DCTDecode", jpg
    )


def case_jpx_siz_huge_image() -> bytes:
    cs = check_siz(jpx_codestream(*HUGE), HUGE)
    return image_pdf("/Width 1 /Height 1 /BitsPerComponent 8 /Filter /JPXDecode", cs)


def case_jpx_siz_huge_jp2() -> bytes:
    jp2 = check_siz(jp2_file(*HUGE, jpx_codestream(*HUGE)), HUGE)
    return image_pdf("/Width 1 /Height 1 /BitsPerComponent 8 /Filter /JPXDecode", jp2)


CASES: dict[str, tuple[str, Callable[[], bytes]]] = {
    "dct-sof-huge-image": ("dct-sof-huge-image.pdf", case_dct_sof_huge_image),
    "dct-sof-huge-thumb": ("dct-sof-huge-thumb.pdf", case_dct_sof_huge_thumb),
    "dct-sof-flate-huge": ("dct-sof-flate-huge.pdf", case_dct_sof_flate_huge),
    "dct-sof-larger-within-cap": ("dct-sof-larger-within-cap.pdf", case_dct_sof_larger_within_cap),
    "dct-sof-missing": ("dct-sof-missing.pdf", case_dct_sof_missing),
    "jpx-siz-huge-image": ("jpx-siz-huge-image.pdf", case_jpx_siz_huge_image),
    "jpx-siz-huge-jp2": ("jpx-siz-huge-jp2.pdf", case_jpx_siz_huge_jp2),
}


# ---- CLI e manifest ----------------------------------------------------------------------


def sha256_of(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def generate(out: Path, only: str | None) -> list[str]:
    out.mkdir(parents=True, exist_ok=True)
    done = []
    for case_id, (fname, build) in CASES.items():
        if only is not None and case_id != only:
            continue
        dest = out / fname
        part = out / (fname + ".part")
        part.write_bytes(build())
        part.replace(dest)
        done.append(case_id)
        print(f"generato {case_id}: {fname} {dest.stat().st_size} byte sha256={sha256_of(dest)[:16]}")
    return done


def load_manifest(path: Path) -> list[dict]:
    cases = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(cases, list):
        raise BuildError("manifest: atteso un array")
    ids = [c.get("case_id") for c in cases]
    if sorted(ids) != sorted(CASES):
        raise BuildError(f"manifest e generatore non hanno gli stessi casi: {sorted(set(ids) ^ set(CASES))}")
    for c in cases:
        if c.get("file") != CASES[c["case_id"]][0]:
            raise BuildError(f"manifest: file di {c['case_id']} diverso dal generatore")
    return cases


def fill_manifest(path: Path, out: Path) -> None:
    """Scrive lo sha256 dei soli casi con `"sha256": null`; un hash presente non si tocca."""
    cases = load_manifest(path)
    changed = 0
    for c in cases:
        actual = sha256_of(out / c["file"])
        if c.get("sha256") is None:
            c["sha256"] = actual
            changed += 1
        elif c["sha256"] != actual:
            print(f"ATTENZIONE {c['case_id']}: sha256 nel manifest diverso dal file generato", file=sys.stderr)
    text = json.dumps(cases, indent=2, ensure_ascii=False) + "\n"
    path.write_bytes(text.encode("utf-8"))
    print(f"manifest: {changed} sha256 scritti")


def verify_manifest(path: Path, out: Path) -> int:
    bad = 0
    for c in load_manifest(path):
        f = out / c["file"]
        if not f.is_file():
            print(f"MANCA {c['case_id']}: {c['file']}", file=sys.stderr)
            bad += 1
        elif sha256_of(f) != c.get("sha256"):
            print(f"DIVERSO {c['case_id']}: sha256 del file diverso dal manifest", file=sys.stderr)
            bad += 1
    print(f"verifica manifest: {len(CASES) - bad} casi conformi su {len(CASES)}")
    return 0 if bad == 0 else 1


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Generatore dei casi ostili SOF e SIZ (gap-closure 1.5)")
    ap.add_argument("--out", required=True, type=Path, help="cartella dei file generati")
    ap.add_argument("--manifest", type=Path, help="manifest in cui scrivere gli sha256 mancanti")
    ap.add_argument("--only", choices=sorted(CASES), help="genera un solo caso")
    ap.add_argument(
        "--verify-only",
        action="store_true",
        help="non genera: confronta i file presenti con il manifest",
    )
    args = ap.parse_args(argv)
    try:
        if args.verify_only:
            if args.manifest is None:
                ap.error("--verify-only richiede --manifest")
            return verify_manifest(args.manifest, args.out)
        generate(args.out, args.only)
        if args.manifest is not None:
            fill_manifest(args.manifest, args.out)
    except BuildError as e:
        print(f"gen_sof: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
