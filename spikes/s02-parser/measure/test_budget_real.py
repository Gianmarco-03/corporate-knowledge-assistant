"""Test di `summarize_dir` di `budget_real.py`: m10 conta i `job_id` (gap-closure 1.6, D-10).

Ogni test costruisce una cartella di batch sintetica in `tempfile.TemporaryDirectory`
(`jobs.jsonl`, `reports.jsonl` con `batch_summary`, `exit_code`, `parent_alive`,
`cg-before.txt`, `cg-after.txt`) e legge m10 dall'ultima colonna di `batch_row`. Ogni caso
rosso cambia una sola cosa rispetto al caso verde `test_m10_true_when_ids_match`.

Gira nel runner, da `spikes/`:
`python3 -m unittest discover -s s02-parser/measure -p "test_*.py"`.

**Modifica:** Claude (test-write 01.6-05) · 2026-10-27 · test di m10 con insiemi di job_id (D-10)
"""

from __future__ import annotations

import importlib.util
import json
import tempfile
import unittest
from pathlib import Path

_SPEC = importlib.util.spec_from_file_location(
    "budget_real", Path(__file__).resolve().parent / "budget_real.py"
)
budget_real = importlib.util.module_from_spec(_SPEC)
_SPEC.loader.exec_module(budget_real)

RES_BYTES = 1 << 30
CG_BEFORE = "1048576\n2147483648\noom_kill 0\nmax 0\n"
CG_AFTER = "4194304\n2147483648\noom_kill 0\nmax 0\n"
SUMMARY = {"status": "batch_summary", "launch_errors": 0, "max_admitted": 2}


def job(job_id: str) -> dict:
    return {
        "job_id": job_id,
        "op": "parse",
        "input": f"/data/corpus/raw/{job_id}.pdf",
        "out_dir": f"/results/raw/isolation/{job_id}",
        "params": {},
    }


def report(job_id: str | None) -> dict:
    r = {"outcome": {"status": "ok"}, "peak_rss_kb": 1024, "leftover_procs": 0, "wall_ms": 10}
    if job_id is not None:
        r["job_id"] = job_id
    return r


def write_lines(path: Path, objs: list[dict]) -> None:
    path.write_text("".join(json.dumps(o) + "\n" for o in objs), encoding="utf-8")


class SummarizeDirM10(unittest.TestCase):
    def m10(self, jobs: list[str], reports: list[str | None]) -> str:
        """m10 di una cartella sintetica con i job e i rapporti dati (stringa `true`/`false`)."""
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp)
            write_lines(d / "jobs.jsonl", [job(j) for j in jobs])
            write_lines(d / "reports.jsonl", [report(r) for r in reports] + [SUMMARY])
            (d / "exit_code").write_text("0\n", encoding="utf-8")
            (d / "parent_alive").write_text("0\n", encoding="utf-8")
            (d / "cg-before.txt").write_text(CG_BEFORE, encoding="utf-8")
            (d / "cg-after.txt").write_text(CG_AFTER, encoding="utf-8")
            batch_row, _ = budget_real.summarize_dir("benign", 1, d, RES_BYTES)
        self.assertEqual(budget_real.BATCH_HEADER[-1], "m10_pass")
        return batch_row[-1]

    def test_m10_true_when_ids_match(self):
        self.assertEqual(self.m10(["a", "b"], ["a", "b"]), "true")

    def test_m10_false_when_report_missing(self):
        self.assertEqual(self.m10(["a", "b"], ["a"]), "false")

    def test_m10_false_when_report_duplicated(self):
        self.assertEqual(self.m10(["a", "b"], ["a", "b", "b"]), "false")

    def test_m10_false_when_report_unknown(self):
        self.assertEqual(self.m10(["a", "b"], ["a", "b", "c"]), "false")

    def test_m10_false_when_no_jobs_and_no_reports(self):
        self.assertEqual(self.m10([], []), "false")

    def test_m10_false_when_jobs_duplicated(self):
        self.assertEqual(self.m10(["a", "a"], ["a"]), "false")

    def test_report_without_job_id_never_true(self):
        try:
            m10 = self.m10(["a", "b"], ["a", None])
        except KeyError:
            return
        self.assertEqual(m10, "false")


if __name__ == "__main__":
    unittest.main()
