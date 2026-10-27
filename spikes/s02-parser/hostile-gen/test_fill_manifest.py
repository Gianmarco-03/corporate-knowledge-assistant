"""Test di `fill_manifest` dei due generatori e del determinismo di `write_docx` (gap-closure 1.6).

`fill_manifest` calcola lo sha256 di tutti i casi del manifest, quindi ogni classe genera
una volta l'intero insieme dei casi in `setUpClass`, dentro `tempfile.TemporaryDirectory`
sotto `/work/tmp` (volume del runner) o, fuori dal runner, nella cartella temporanea di
sistema. I test lavorano su copie dei manifest nella stessa cartella: i manifest veri
(`spikes/data/hostile/manifest.json`, `spikes/data/hostile/sof/manifest.json`) e
`spikes/data/hostile/generated/` non si toccano (D-12).

Gira nel runner, da `spikes/`:
`python3 -m unittest discover -s s02-parser/hostile-gen -p "test_*.py"`.

**Modifica:** Claude (test-write 01.6-05) · 2026-10-27 · test di fill_manifest con esito contato e di write_docx a data fissa (D-10, D-11)
"""

from __future__ import annotations

import contextlib
import hashlib
import importlib.util
import io
import json
import os
import tempfile
import time
import unittest
import zipfile
import zlib
from pathlib import Path

HERE = Path(__file__).resolve().parent
SPIKES = HERE.parents[1]
HOSTILE_MANIFEST = SPIKES / "data/hostile/manifest.json"
SOF_MANIFEST = SPIKES / "data/hostile/sof/manifest.json"
DOCX_CASES = ("docx-zip-bomb", "docx-xml-bomb", "docm-macro", "docx-external-link")


def load(name: str):
    spec = importlib.util.spec_from_file_location(name, HERE / f"{name}.py")
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


gen_hostile = load("gen_hostile")
gen_sof = load("gen_sof")


def tmp_root() -> str | None:
    root = Path("/work/tmp")
    if root.is_dir() and os.access(root, os.W_OK):
        return str(root)
    return None


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def call_quiet(fn, *args):
    """Chiama `fn` e ritorna (esito, stderr); lo stdout del generatore si scarta."""
    err = io.StringIO()
    with contextlib.redirect_stdout(io.StringIO()), contextlib.redirect_stderr(err):
        result = fn(*args)
    return result, err.getvalue()


def manifest_copy(src: Path, dest: Path, overrides: dict[str, str]) -> Path:
    """Copia del manifest con lo sha256 dei casi in `overrides` sostituito."""
    cases = json.loads(src.read_text(encoding="utf-8"))
    for c in cases:
        if c["case_id"] in overrides:
            c["sha256"] = overrides[c["case_id"]]
    dest.write_bytes((json.dumps(cases, ensure_ascii=False, indent=2) + "\n").encode("utf-8"))
    return dest


def deterministic_case(cases: list[dict]) -> str:
    for c in cases:
        cid = c["case_id"]
        if not cid.startswith("encrypted-") and cid not in DOCX_CASES:
            return cid
    raise AssertionError("nessun caso deterministico nel manifest")


class HostileFillManifest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls._tmp = tempfile.TemporaryDirectory(prefix="01-6-05-test-", dir=tmp_root())
        cls.root = Path(cls._tmp.name)
        cls.out = cls.root / "generated"
        call_quiet(gen_hostile.generate, cls.out, None)
        cls.real_sha = sha256(HOSTILE_MANIFEST)

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    def tearDown(self):
        self.assertEqual(sha256(HOSTILE_MANIFEST), self.real_sha, "manifest vero modificato")

    def test_hostile_fill_returns_1_on_deterministic_mismatch(self):
        cases = json.loads(HOSTILE_MANIFEST.read_text(encoding="utf-8"))
        cid = deterministic_case(cases)
        copy = manifest_copy(HOSTILE_MANIFEST, self.root / "m-mismatch.json", {cid: "0" * 64})
        before = copy.read_bytes()
        result, err = call_quiet(gen_hostile.fill_manifest, copy, self.out)
        self.assertEqual(result, 1, f"fill_manifest su sha256 diverso di {cid}")
        self.assertEqual(copy.read_bytes(), before, "manifest riscritto nonostante il DIVERSO")
        self.assertIn(f"DIVERSO {cid}", err)

    def test_hostile_fill_tolerates_declared_cases(self):
        overrides = {"encrypted-user": "1" * 64, "docx-zip-bomb": "2" * 64}
        copy = manifest_copy(HOSTILE_MANIFEST, self.root / "m-declared.json", overrides)
        result, err = call_quiet(gen_hostile.fill_manifest, copy, self.out)
        self.assertEqual(result, 0, f"casi dichiarati non tollerati: {err}")
        for cid in overrides:
            self.assertIn(f"ATTESO DIVERSO {cid}", err)
        after = {c["case_id"]: c["sha256"] for c in json.loads(copy.read_text(encoding="utf-8"))}
        for cid, value in overrides.items():
            self.assertEqual(after[cid], value, f"sha256 del caso dichiarato {cid} sostituito")


class SofFillManifest(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls._tmp = tempfile.TemporaryDirectory(prefix="01-6-05-test-", dir=tmp_root())
        cls.root = Path(cls._tmp.name)
        cls.out = cls.root / "generated"
        call_quiet(gen_sof.generate, cls.out, None)
        cls.real_sha = sha256(SOF_MANIFEST)

    @classmethod
    def tearDownClass(cls):
        cls._tmp.cleanup()

    def tearDown(self):
        self.assertEqual(sha256(SOF_MANIFEST), self.real_sha, "manifest vero modificato")

    def test_sof_fill_returns_1_on_mismatch(self):
        cases = json.loads(SOF_MANIFEST.read_text(encoding="utf-8"))
        cid = cases[0]["case_id"]
        copy = manifest_copy(SOF_MANIFEST, self.root / "m-mismatch.json", {cid: "0" * 64})
        before = copy.read_bytes()
        result, err = call_quiet(gen_sof.fill_manifest, copy, self.out)
        self.assertEqual(result, 1, f"fill_manifest su sha256 diverso di {cid}")
        self.assertEqual(copy.read_bytes(), before, "manifest riscritto nonostante il DIVERSO")
        self.assertIn(f"DIVERSO {cid}", err)


class WriteDocx(unittest.TestCase):
    def test_write_docx_is_deterministic(self):
        parts = gen_hostile.base_docx()
        big = ("word/big.xml", gen_hostile.MIB)
        with tempfile.TemporaryDirectory(prefix="01-6-05-test-", dir=tmp_root()) as tmp:
            d = Path(tmp)
            gen_hostile.write_docx(d / "a-small.docx", parts)
            gen_hostile.write_docx(d / "a-big.docx", parts, big=big)
            # Il timestamp zip ha risoluzione di 2 secondi.
            time.sleep(2.2)
            gen_hostile.write_docx(d / "b-small.docx", parts)
            gen_hostile.write_docx(d / "b-big.docx", parts, big=big)
            for kind in ("small", "big"):
                self.assertEqual(
                    sha256(d / f"a-{kind}.docx"),
                    sha256(d / f"b-{kind}.docx"),
                    f"write_docx ({kind}) non deterministico",
                )
            with zipfile.ZipFile(d / "b-big.docx") as zf:
                infos = {i.filename: i for i in zf.infolist()}
        self.assertEqual(sorted(infos), sorted(list(parts) + [big[0]]))
        for name, info in infos.items():
            self.assertEqual(info.date_time, (1980, 1, 1, 0, 0, 0), name)
            self.assertEqual(info.compress_type, zipfile.ZIP_DEFLATED, name)
            self.assertEqual(info.external_attr, 0o600 << 16, name)
        # Il contenuto delle voci non cambia: solo i metadati.
        for name, data in parts.items():
            self.assertEqual(infos[name].file_size, len(data), name)
            self.assertEqual(infos[name].CRC, zlib.crc32(data), name)
        self.assertEqual(infos[big[0]].file_size, big[1])


if __name__ == "__main__":
    unittest.main()
