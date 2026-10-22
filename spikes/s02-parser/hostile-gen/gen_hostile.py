#!/usr/bin/env python3
"""Generatore dei casi ostili di S-02 (piano 01-18, Task 1).

Scrive i file ostili in `--out` (nel repo `spikes/data/hostile/generated/`, ignorata da git).
Gira nel runner senza rete, dentro una sessione di `session.sh`:

    docker run --rm --network none --label cka.run_id="$CKA_RUN_ID" -v <spikes>:/src \
        cka-spikes-runner python3 /src/s02-parser/hostile-gen/gen_hostile.py \
        --out /src/data/hostile/generated --manifest /src/data/hostile/manifest.json

Solo standard library (`zlib`, `zipfile`, `struct`, `hashlib`, `random`, `secrets`,
`subprocess`, ...) più la CLI `qpdf` dell'immagine runner per la cifratura. Le password
dei casi cifrati si generano a runtime con `secrets` e non si scrivono da nessuna parte.
I PDF sono scritti a mano con gli offset della xref calcolati; i casi `objstm-*` hanno
object stream veri e una cross-reference stream.

Il generatore controlla le proprietà della propria costruzione (rapporti di compressione,
dimensioni, assenza di `JavaScript` in chiaro), mai l'esito atteso del lanciatore: gli esiti
attesi stanno nel manifest e si fissano prima della misura.

Con `--manifest` scrive lo sha256 dei casi che non ce l'hanno ancora; con `--verify-only`
non genera nulla e confronta i file già presenti con gli sha256 del manifest (exit 1 se uno
manca o differisce). I PDF cifrati cambiano a ogni generazione (sale e chiavi casuali di
qpdf): il manifest registra la generazione usata per la misura.

**Modifica:** Claude (Anthropic) · 2026-10-22 · generatore dei casi ostili di 01-18
"""

from __future__ import annotations

import argparse
import hashlib
import json
import random
import re
import secrets
import subprocess
import sys
import tempfile
import zipfile
import zlib
from pathlib import Path
from typing import Callable, Iterable, Iterator

KIB = 1024
MIB = 1024 * KIB
GIB = 1024 * MIB

# Tetti del lanciatore (Soglie del Task 1 di 01-07): servono solo a verificare che la
# costruzione dei casi stia dal lato giusto di ogni tetto.
PER_STREAM_CAP = 64 * MIB
TOTAL_CAP = 512 * MIB
RATIO_CAP = 100
RATIO_MIN_BYTES = 16 * MIB
DOCX_TOTAL_CAP = 200 * MIB

# Allegato dei casi `pdf-embedded-*`: stringa fissa e non segreta, cercata dal controllo (d)
# della «Validazione di 01-07» nell'out_dir del job.
EMBEDDED_MARKER = b"CKA-EMBEDDED-MARKER"

CONTENT = b"BT /F1 12 Tf 72 712 Td (Ciao dal caso ostile) Tj ET"


class BuildError(Exception):
    """La costruzione di un caso non rispetta la propria specifica."""


# ---- PDF: oggetti e xref -----------------------------------------------------------------


def stream(dict_extra: str, data: bytes) -> bytes:
    head = f"<< /Length {len(data)} {dict_extra} >>\nstream\n".encode()
    return head + data + b"\nendstream"


def push_obj(out: bytearray, num: int, body: bytes) -> int:
    off = len(out)
    out += f"{num} 0 obj\n".encode() + body + b"\nendobj\n"
    return off


def build_classic(objs: list[tuple[int, bytes]], trailer_extra: str = "") -> bytes:
    """PDF 1.7 con tabella xref classica; l'oggetto 1 è il Catalog."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets: dict[int, int] = {}
    for num, body in sorted(objs, key=lambda o: o[0]):
        offsets[num] = push_obj(out, num, body)
    size = max(offsets) + 1
    xref = len(out)
    out += f"xref\n0 {size}\n0000000000 65535 f \n".encode()
    for i in range(1, size):
        if i in offsets:
            out += f"{offsets[i]:010d} 00000 n \n".encode()
        else:
            out += b"0000000000 65535 f \n"
    out += (
        f"trailer\n<< /Size {size} /Root 1 0 R {trailer_extra} >>\n"
        f"startxref\n{xref}\n%%EOF\n"
    ).encode()
    return bytes(out)


class ObjStm:
    """Object stream: membri `(numero, corpo)` e un riempimento opzionale dopo gli oggetti."""

    def __init__(
        self,
        num: int,
        members: list[tuple[int, bytes]],
        pad: Callable[[], Iterable[bytes]] | None = None,
    ) -> None:
        self.num = num
        self.members = members
        self.pad = pad
        self.plain_len = 0
        self.packed_len = 0

    def body(self) -> bytes:
        header = ""
        bodies = bytearray()
        for num, b in self.members:
            header += f"{num} {len(bodies)} "
            bodies += b + b" "
        first = len(header)
        comp = zlib.compressobj(6)
        parts = [comp.compress(header.encode()), comp.compress(bytes(bodies))]
        plain = first + len(bodies)
        if self.pad is not None:
            for chunk in self.pad():
                plain += len(chunk)
                parts.append(comp.compress(chunk))
        parts.append(comp.flush())
        data = b"".join(parts)
        self.plain_len = plain
        self.packed_len = len(data)
        return stream(
            f"/Type /ObjStm /N {len(self.members)} /First {first} /Filter /FlateDecode",
            data,
        )


def xref_row(kind: int, field2: int, field3: int) -> bytes:
    return bytes([kind]) + field2.to_bytes(4, "big") + field3.to_bytes(2, "big")


def build_modern(normal: list[tuple[int, bytes]], objstms: list[ObjStm]) -> bytes:
    """PDF 1.7 con object stream veri (voci di tipo 2) e una cross-reference stream."""
    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    compressed: dict[int, tuple[int, int]] = {}
    allobjs = list(normal)
    for o in objstms:
        allobjs.append((o.num, o.body()))
        for idx, (m, _) in enumerate(o.members):
            compressed[m] = (o.num, idx)
    offsets: dict[int, int] = {}
    for num, body in sorted(allobjs, key=lambda x: x[0]):
        offsets[num] = push_obj(out, num, body)
    xnum = max(list(offsets) + list(compressed)) + 1
    size = xnum + 1
    xoff = len(out)
    rows = bytearray()
    for i in range(size):
        if i == xnum:
            rows += xref_row(1, xoff, 0)
        elif i in offsets:
            rows += xref_row(1, offsets[i], 0)
        elif i in compressed:
            rows += xref_row(2, compressed[i][0], compressed[i][1])
        else:
            rows += xref_row(0, 0, 65535 if i == 0 else 0)
    xbody = stream(
        f"/Type /XRef /W [1 4 2] /Size {size} /Root 1 0 R /Filter /FlateDecode",
        zlib.compress(bytes(rows)),
    )
    push_obj(out, xnum, xbody)
    out += f"startxref\n{xoff}\n%%EOF\n".encode()
    return bytes(out)


def base_objects(
    catalog_extra: str = "", page_extra: str = "", content: bytes = CONTENT
) -> list[tuple[int, bytes]]:
    """Catalog (1), Pages (2), una pagina (3) con contenuto (4) e font (5)."""
    return [
        (1, f"<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>".encode()),
        (2, b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>"),
        (
            3,
            (
                "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
                f"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R {page_extra} >>"
            ).encode(),
        ),
        (4, stream("", content)),
        (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ]


def refs(nums: Iterable[int]) -> str:
    return " ".join(f"{n} 0 R" for n in nums)


# ---- contenuti compressi -----------------------------------------------------------------


def zlib_repeat(byte: int, n: int, level: int = 9) -> bytes:
    """`n` copie di `byte` compresse in streaming (nessun buffer da `n` byte)."""
    comp = zlib.compressobj(level)
    chunk = bytes([byte]) * MIB
    parts = []
    left = n
    while left > 0:
        k = min(left, len(chunk))
        parts.append(comp.compress(chunk[:k]))
        left -= k
    parts.append(comp.flush())
    return b"".join(parts)


def low_entropy_lines(total: int, seed: int) -> Iterator[bytes]:
    """Righe di commento PDF da 1 KiB con 40 caratteri esadecimali pseudo-casuali ciascuna:
    il rapporto di Flate resta fra 10 e 100 (sotto il tetto di rapporto, sopra i casi banali)."""
    rng = random.Random(seed)
    block_lines = 1024
    left = total
    while left > 0:
        lines = []
        for _ in range(block_lines):
            noise = rng.getrandbits(160).to_bytes(20, "big").hex().encode()
            lines.append(b"% " + noise + b" " * (1024 - 2 - len(noise) - 1) + b"\n")
        block = b"".join(lines)
        if len(block) > left:
            block = block[:left]
        left -= len(block)
        yield block


def low_entropy_string(total: int, seed: int) -> bytes:
    """Contenuto di una stringa letterale: blocchi da 1 KiB di `A` con 40 lettere a-p
    pseudo-casuali (nessuna parentesi né barra rovescia)."""
    rng = random.Random(seed)
    alphabet = b"abcdefghijklmnop"
    out = bytearray()
    while len(out) < total:
        noise = bytes(alphabet[rng.getrandbits(4)] for _ in range(40))
        out += b"A" * (1024 - 40) + noise
    return bytes(out[:total])


def ratio_of(plain: int, packed: int) -> float:
    return plain / max(packed, 1)


# ---- qpdf --------------------------------------------------------------------------------


def qpdf(src: bytes, opts: list[str]) -> bytes:
    """`qpdf <opts> <in> <out>` su file temporanei; exit 3 = solo avvisi."""
    with tempfile.TemporaryDirectory(prefix="gen-hostile-") as tmp:
        src_path = Path(tmp) / "in.pdf"
        dst_path = Path(tmp) / "out.pdf"
        src_path.write_bytes(src)
        argv = ["qpdf", *opts, str(src_path), str(dst_path)]
        res = subprocess.run(argv, capture_output=True, check=False)
        if res.returncode not in (0, 3):
            # Le password sono negli argomenti: si riporta solo lo stderr di qpdf.
            raise BuildError(f"qpdf exit {res.returncode}: {res.stderr.decode(errors='replace')}")
        return dst_path.read_bytes()


def runtime_secret() -> str:
    """Password di prova generata a runtime, mai scritta su disco né nel manifest."""
    return secrets.token_hex(12)


def encrypt(src: bytes, user_secret: str, objstm_mode: str) -> bytes:
    """AES-256 con `qpdf --encrypt <utente> <proprietario> 256 --`."""
    return qpdf(src, [objstm_mode, "--encrypt", user_secret, runtime_secret(), "256", "--"])


# ---- casi PDF ----------------------------------------------------------------------------


def case_encrypted_user() -> bytes:
    return encrypt(build_classic(base_objects()), runtime_secret(), "--object-streams=disable")


def case_encrypted_owner_only() -> bytes:
    return encrypt(build_classic(base_objects()), "", "--object-streams=disable")


def case_encrypted_pubsec() -> bytes:
    pubsec = (
        b"<< /Filter /Adobe.PubSec /SubFilter /adbe.pkcs7.s4 /V 4 /R 4 "
        b"/Recipients [<3082000A>] >>"
    )
    return build_classic(base_objects() + [(10, pubsec)], "/Encrypt 10 0 R /ID [<01> <01>]")


def case_encrypted_object_dropped() -> bytes:
    """Cifrato con password utente vuota; poi l'intestazione dell'oggetto citato solo dal
    Catalog (`/Extra`, mai dalle pagine) diventa `obx`, così lopdf lo scarta in silenzio."""
    plain = build_classic(
        base_objects("/Extra 10 0 R") + [(10, b"<< /CKA (oggetto fuori dalle pagine) >>")]
    )
    pdf = bytearray(encrypt(plain, "", "--object-streams=disable"))
    roots = re.findall(rb"/Root (\d+) 0 R", pdf)
    if not roots:
        raise BuildError("encrypted-object-dropped: /Root assente")
    root = int(roots[-1])
    # Fino a `endobj`: il Catalog di qpdf ha dizionari annidati (`/Extensions`).
    cat = re.search(rb"(?:^|[\r\n])" + str(root).encode() + rb" 0 obj(.*?)endobj", pdf, re.S)
    if cat is None:
        raise BuildError("encrypted-object-dropped: Catalog non trovato")
    extra = re.search(rb"/Extra (\d+) 0 R", cat.group(1))
    if extra is None:
        raise BuildError("encrypted-object-dropped: /Extra non trovato nel Catalog")
    target = extra.group(1)
    hits = [m for m in re.finditer(rb"(?:^|[\r\n])" + target + rb" 0 obj", pdf)]
    if len(hits) != 1:
        raise BuildError(f"encrypted-object-dropped: {len(hits)} intestazioni per {target!r}")
    kw = hits[0].end() - 3
    pdf[kw : kw + 3] = b"obx"
    return bytes(pdf)


def small_objstms(first_container: int, count: int) -> list[ObjStm]:
    return [
        ObjStm(first_container + i, [(first_container + count + i, f"<< /K {i} >>".encode())])
        for i in range(count)
    ]


def member_refs(objstms: list[ObjStm]) -> str:
    return "/Extra [" + refs(m for o in objstms for m, _ in o.members) + "]"


def case_encrypted_objstm_quota() -> bytes:
    """16 object stream, uno da 40 MiB decompressi (rapporto ≤ 100×), cifrato con password
    utente vuota e `--object-streams=preserve`: 40 MiB < 64 MiB ma oltre ⌊512 MiB / 16⌋."""
    osm = small_objstms(10, 16)
    big = b"<< /Pad (" + low_entropy_string(40 * MIB, seed=1816) + b") >>"
    osm[15].members[0] = (osm[15].members[0][0], big)
    plain = build_modern(base_objects(member_refs(osm)), osm)
    o = osm[15]
    if not (40 * MIB <= o.plain_len < PER_STREAM_CAP and o.plain_len > TOTAL_CAP // 16):
        raise BuildError(f"encrypted-objstm-quota: {o.plain_len} byte fuori specifica")
    if ratio_of(o.plain_len, o.packed_len) > RATIO_CAP:
        raise BuildError("encrypted-objstm-quota: rapporto oltre 100")
    return encrypt(plain, "", "--object-streams=preserve")


def case_truncated() -> bytes:
    pdf = build_classic(base_objects())
    return pdf[: len(pdf) // 2]


def case_flate_bomb() -> bytes:
    data = zlib_repeat(0, GIB)
    if len(data) > 2 * MIB:
        raise BuildError(f"flate-bomb: {len(data)} byte compressi")
    objs = base_objects()
    objs[3] = (4, stream("/Filter /FlateDecode", data))
    return build_classic(objs)


def case_objstm_bomb() -> bytes:
    """Un object stream che si espande a 128 MiB (oltre il tetto per stream) al caricamento."""

    def pad() -> Iterator[bytes]:
        chunk = b" " * MIB
        for _ in range(128):
            yield chunk

    osm = [ObjStm(10, [(11, b"<< /K 1 >>")], pad)]
    pdf = build_modern(base_objects("/Extra [11 0 R]"), osm)
    if osm[0].plain_len <= PER_STREAM_CAP:
        raise BuildError("objstm-bomb: sotto il tetto per stream")
    return pdf


def case_objstm_many() -> bytes:
    """80 object stream piccoli (più di 64), somma reale ben sotto 512 MiB."""
    osm = small_objstms(10, 80)
    pdf = build_modern(base_objects(member_refs(osm)), osm)
    if sum(o.plain_len for o in osm) >= TOTAL_CAP:
        raise BuildError("objstm-many: somma oltre il totale")
    return pdf


def case_objstm_sum_bomb() -> bytes:
    """9 object stream da 60 MiB ciascuno (sotto 64 MiB, rapporto ≤ 100×): 540 MiB in tutto."""
    osm = []
    for i in range(9):
        size = 60 * MIB
        seed = 1800 + i
        osm.append(
            ObjStm(
                10 + i,
                [(30 + i, f"<< /K {i} >>".encode())],
                lambda size=size, seed=seed: low_entropy_lines(size, seed),
            )
        )
    pdf = build_modern(base_objects("/Extra [" + refs(30 + i for i in range(9)) + "]"), osm)
    for o in osm:
        r = ratio_of(o.plain_len, o.packed_len)
        if o.plain_len >= PER_STREAM_CAP or r > RATIO_CAP:
            raise BuildError(f"objstm-sum-bomb: stream {o.num} {o.plain_len} byte, rapporto {r:.1f}")
    if sum(o.plain_len for o in osm) <= TOTAL_CAP:
        raise BuildError("objstm-sum-bomb: somma sotto il totale")
    return pdf


def case_filter_chain_bomb() -> bytes:
    inner = zlib_repeat(0, GIB)
    data = zlib.compress(inner, 9)
    objs = base_objects()
    objs[3] = (4, stream("/Filter [/FlateDecode /FlateDecode]", data))
    return build_classic(objs)


def case_lzw_stream() -> bytes:
    objs = base_objects()
    objs[3] = (4, stream("/Filter /LZWDecode", b"\x80\x0b\x60\x50\x22\x0c\x0c\x85\x01"))
    return build_classic(objs)


def case_pages_10k() -> bytes:
    pages = 10_000
    first = 10
    kids = refs(first + i for i in range(pages))
    objs = [
        (1, b"<< /Type /Catalog /Pages 2 0 R >>"),
        (2, f"<< /Type /Pages /Kids [{kids}] /Count {pages} >>".encode()),
        (4, stream("", CONTENT)),
        (5, b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>"),
    ]
    page = (
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
        b"/Resources << /Font << /F1 5 0 R >> >> /Contents 4 0 R >>"
    )
    objs += [(first + i, page) for i in range(pages)]
    return build_classic(objs)


def case_deep_nesting() -> bytes:
    depth = 100_000
    nested = b"[" * depth + b"]" * depth
    return build_classic(base_objects("/Extra 10 0 R") + [(10, nested)])


def case_jbig2_malformed() -> bytes:
    """Immagine `/JBIG2Decode` con dimensioni valide e un flusso JBIG2 rotto (segmento
    troncato dopo un'intestazione plausibile)."""
    junk = b"\x97JB2\r\n\x1a\n\x01\x00\x00\x00\x01" + b"\x00\x00\x00\x00\x30\x00\x01\xff\xff\xff"
    image = stream(
        "/Type /XObject /Subtype /Image /Width 100 /Height 100 /ColorSpace /DeviceGray "
        "/BitsPerComponent 1 /Filter /JBIG2Decode",
        junk,
    )
    objs = base_objects(content=b"q 100 0 0 100 72 600 cm /Im1 Do Q " + CONTENT)
    objs[2] = (
        3,
        (
            b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] "
            b"/Resources << /Font << /F1 5 0 R >> /XObject << /Im1 10 0 R >> >> /Contents 4 0 R >>"
        ),
    )
    return build_classic(objs + [(10, image)])


def case_pdf_javascript() -> bytes:
    js = b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>"
    return build_classic(
        base_objects("/Names << /JavaScript 10 0 R >>")
        + [(10, b"<< /Names [(a) 11 0 R] >>"), (11, js)]
    )


def case_pdf_launch() -> bytes:
    annot = (
        b"<< /Type /Annot /Subtype /Link /Rect [72 700 300 720] "
        b"/A << /S /Launch /F (cmd.exe) >> >>"
    )
    return build_classic(base_objects(page_extra="/Annots [10 0 R]") + [(10, annot)])


def attachment(data_filter: str, data: bytes) -> list[tuple[int, bytes]]:
    return [
        (10, b"<< /Type /Filespec /F (allegato.bin) /UF (allegato.bin) /EF << /F 11 0 R >> >>"),
        (11, stream(f"/Type /EmbeddedFile {data_filter}", data)),
        (12, b"<< /Names [(allegato.bin) 10 0 R] >>"),
    ]


def case_pdf_embedded_file() -> bytes:
    return build_classic(
        base_objects("/Names << /EmbeddedFiles 12 0 R >>")
        + attachment("", EMBEDDED_MARKER + b"\n")
    )


def case_pdf_embedded_launch() -> bytes:
    return build_classic(
        base_objects("/Names << /EmbeddedFiles 12 0 R >> /OpenAction 13 0 R")
        + attachment("", EMBEDDED_MARKER + b"\n")
        + [(13, b"<< /S /Launch /F 10 0 R >>")]
    )


def case_pdf_embedded_bomb() -> bytes:
    data = zlib_repeat(0, 128 * MIB)
    return build_classic(
        base_objects("/Names << /EmbeddedFiles 12 0 R >>")
        + attachment("/Filter /FlateDecode", data)
    )


def case_pdf_objstm_hidden_js() -> bytes:
    """Azione JavaScript (azione aggiuntiva di apertura della pagina) dentro un object stream
    compresso: la scansione a byte non la vede."""
    js = b"<< /S /JavaScript /JS (app.alert\\(1\\)) >>"
    osm = [ObjStm(10, [(11, js)])]
    pdf = build_modern(base_objects(page_extra="/AA << /O 11 0 R >>"), osm)
    if b"JavaScript" in pdf or b"/JS" in pdf:
        raise BuildError("pdf-objstm-hidden-js: JavaScript visibile in chiaro")
    return pdf


def case_magic_mismatch() -> bytes:
    """Estensione `.pdf`, contenuto PNG: nessun `%PDF-` nel primo KiB."""
    return b"\x89PNG\r\n\x1a\n" + b"\x00\x00\x00\rIHDR" + bytes(range(256)) * 8


# ---- casi DOCX ---------------------------------------------------------------------------

CT_DOCX = "application/vnd.openxmlformats-officedocument.wordprocessingml.document.main+xml"
CT_MACRO = "application/vnd.ms-word.document.macroEnabled.main+xml"
XML_DECL = '<?xml version="1.0" encoding="UTF-8"?>'
NS_W = "urn:test:ooxml/wordprocessingml/2006/main"
NS_REL = "urn:test:ooxml/package/2006/relationships"
DOC_BODY = (
    f'<w:document xmlns:w="{NS_W}"><w:body><w:p><w:r><w:t>Ciao</w:t></w:r></w:p>'
    "</w:body></w:document>"
)


def content_types(main: str) -> bytes:
    return (
        f'{XML_DECL}<Types xmlns="urn:test:ooxml/package/2006/content-types">'
        '<Default Extension="rels" ContentType="application/vnd.openxmlformats-package.relationships+xml"/>'
        '<Default Extension="xml" ContentType="application/xml"/>'
        f'<Override PartName="/word/document.xml" ContentType="{main}"/></Types>'
    ).encode()


def package_rels() -> bytes:
    return (
        f'{XML_DECL}<Relationships xmlns="{NS_REL}"><Relationship Id="rId1" '
        'Type="urn:test:ooxml/officeDocument/2006/relationships/officeDocument" '
        'Target="word/document.xml"/></Relationships>'
    ).encode()


def base_docx(main_ct: str = CT_DOCX) -> dict[str, bytes]:
    return {
        "[Content_Types].xml": content_types(main_ct),
        "_rels/.rels": package_rels(),
        "word/document.xml": (XML_DECL + DOC_BODY).encode(),
    }


def write_docx(path: Path, parts: dict[str, bytes], big: tuple[str, int] | None = None) -> None:
    """Scrive lo zip; `big` = (nome, byte) di una parte XML riempita di spazi in streaming."""
    with zipfile.ZipFile(path, "w", compression=zipfile.ZIP_DEFLATED) as zf:
        for name, data in parts.items():
            zf.writestr(name, data)
        if big is not None:
            name, size = big
            with zf.open(name, "w", force_zip64=True) as f:
                head = (XML_DECL + f'<w:document xmlns:w="{NS_W}"><w:body>').encode()
                tail = b"</w:body></w:document>"
                f.write(head)
                left = size - len(head) - len(tail)
                chunk = b" " * MIB
                while left > 0:
                    k = min(left, len(chunk))
                    f.write(chunk[:k])
                    left -= k
                f.write(tail)


def case_docx_zip_bomb(path: Path) -> None:
    parts = base_docx()
    del parts["word/document.xml"]
    write_docx(path, parts, big=("word/document.xml", 210 * MIB))
    with zipfile.ZipFile(path) as zf:
        declared = sum(i.file_size for i in zf.infolist())
    if declared <= DOCX_TOTAL_CAP:
        raise BuildError("docx-zip-bomb: somma dichiarata sotto 200 MiB")


def case_docx_xml_bomb(path: Path) -> None:
    lol = ['<!ENTITY lol0 "lol">']
    for i in range(1, 10):
        prev = f"&lol{i - 1};" * 10
        lol.append(f'<!ENTITY lol{i} "{prev}">')
    doctype = "<!DOCTYPE w:document [" + "".join(lol) + "]>"
    body = (
        f'<w:document xmlns:w="{NS_W}"><w:body><w:p><w:r><w:t>&lol9;</w:t></w:r></w:p>'
        "</w:body></w:document>"
    )
    parts = base_docx()
    parts["word/document.xml"] = (XML_DECL + doctype + body).encode()
    write_docx(path, parts)


def case_docm_macro(path: Path) -> None:
    parts = base_docx(CT_MACRO)
    parts["word/vbaProject.bin"] = b"\xd0\xcf\x11\xe0\xa1\xb1\x1a\xe1" + b"\x00" * 504
    write_docx(path, parts)


def case_docx_external_link(path: Path) -> None:
    rels = (
        f'{XML_DECL}<Relationships xmlns="{NS_REL}"><Relationship Id="rId9" '
        'Type="urn:test:ooxml/officeDocument/2006/relationships/attachedTemplate" '
        'Target="https://attacker.invalid/t.dotm" TargetMode="External"/></Relationships>'
    ).encode()
    parts = base_docx()
    parts["word/_rels/document.xml.rels"] = rels
    write_docx(path, parts)


# ---- registro dei casi -------------------------------------------------------------------

BytesCase = Callable[[], bytes]
FileCase = Callable[[Path], None]

CASES: dict[str, tuple[str, BytesCase | None, FileCase | None]] = {
    "encrypted-user": ("encrypted-user.pdf", case_encrypted_user, None),
    "encrypted-owner-only": ("encrypted-owner-only.pdf", case_encrypted_owner_only, None),
    "encrypted-pubsec": ("encrypted-pubsec.pdf", case_encrypted_pubsec, None),
    "encrypted-object-dropped": (
        "encrypted-object-dropped.pdf",
        case_encrypted_object_dropped,
        None,
    ),
    "encrypted-objstm-quota": ("encrypted-objstm-quota.pdf", case_encrypted_objstm_quota, None),
    "truncated": ("truncated.pdf", case_truncated, None),
    "flate-bomb": ("flate-bomb.pdf", case_flate_bomb, None),
    "objstm-bomb": ("objstm-bomb.pdf", case_objstm_bomb, None),
    "objstm-many": ("objstm-many.pdf", case_objstm_many, None),
    "objstm-sum-bomb": ("objstm-sum-bomb.pdf", case_objstm_sum_bomb, None),
    "filter-chain-bomb": ("filter-chain-bomb.pdf", case_filter_chain_bomb, None),
    "lzw-stream": ("lzw-stream.pdf", case_lzw_stream, None),
    "pages-10k": ("pages-10k.pdf", case_pages_10k, None),
    "deep-nesting": ("deep-nesting.pdf", case_deep_nesting, None),
    "jbig2-malformed": ("jbig2-malformed.pdf", case_jbig2_malformed, None),
    "docx-zip-bomb": ("docx-zip-bomb.docx", None, case_docx_zip_bomb),
    "docx-xml-bomb": ("docx-xml-bomb.docx", None, case_docx_xml_bomb),
    "docm-macro": ("docm-macro.docm", None, case_docm_macro),
    "docx-external-link": ("docx-external-link.docx", None, case_docx_external_link),
    "magic-mismatch": ("magic-mismatch.pdf", case_magic_mismatch, None),
    "pdf-javascript": ("pdf-javascript.pdf", case_pdf_javascript, None),
    "pdf-launch": ("pdf-launch.pdf", case_pdf_launch, None),
    "pdf-embedded-file": ("pdf-embedded-file.pdf", case_pdf_embedded_file, None),
    "pdf-embedded-launch": ("pdf-embedded-launch.pdf", case_pdf_embedded_launch, None),
    "pdf-embedded-bomb": ("pdf-embedded-bomb.pdf", case_pdf_embedded_bomb, None),
    "pdf-objstm-hidden-js": ("pdf-objstm-hidden-js.pdf", case_pdf_objstm_hidden_js, None),
}


def sha256_of(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for block in iter(lambda: f.read(MIB), b""):
            h.update(block)
    return h.hexdigest()


def generate(out: Path, only: str | None) -> list[str]:
    out.mkdir(parents=True, exist_ok=True)
    done = []
    for case_id, (fname, as_bytes, as_file) in CASES.items():
        if only is not None and case_id != only:
            continue
        dest = out / fname
        part = out / (fname + ".part")
        if as_bytes is not None:
            part.write_bytes(as_bytes())
        elif as_file is not None:
            as_file(part)
        part.replace(dest)
        done.append(case_id)
        print(f"generato {case_id}: {fname} {dest.stat().st_size} byte {sha256_of(dest)}")
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
    """Scrive lo sha256 dei casi senza hash; un hash già presente non si riscrive."""
    cases = load_manifest(path)
    changed = 0
    for c in cases:
        actual = sha256_of(out / c["file"])
        if not c.get("sha256"):
            c["sha256"] = actual
            changed += 1
        elif c["sha256"] != actual:
            print(f"ATTENZIONE {c['case_id']}: sha256 nel manifest diverso dal file generato", file=sys.stderr)
    text = json.dumps(cases, ensure_ascii=False, indent=2) + "\n"
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
    ap = argparse.ArgumentParser(description="Generatore dei casi ostili di S-02 (01-18)")
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
        print(f"gen_hostile: {e}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
