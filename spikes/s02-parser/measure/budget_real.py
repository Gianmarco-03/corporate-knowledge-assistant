#!/usr/bin/env python3
"""Budget aggregato sui job `parse` reali (piano 01-18, Task 2; soglia M10).

Gira sull'host con `python`, solo standard library. Due sottocomandi:

- `jobs --class <benign|hostile> --concurrency <N> --run-tag <tag> --out <file.jsonl>`:
  un job `parse` per riga per `parse-parent batch` di 01-07. J = max(2 × N, documenti della
  classe) job, documenti ciclati in ordine fisso: `benign` = i PDF e DOCX del manifest di 01-03
  presenti in `spikes/data/corpus/raw/` (nel worker `/data/corpus/raw/<file>`), `hostile` =
  tutti i casi del manifest ostile (nel worker `/data/hostile/generated/<file>`). `job_id`
  `brl-<b|h>-c<N>-j<k>-<tag>`, `out_dir` `/results/raw/isolation/<job_id>`. Esce 2 se un
  `job_id` supera 64 caratteri o non rispetta `^[a-z0-9-]{1,64}$`, o se un argomento non è
  valido.

- `summarize --raw <dir> --limits <limits.json> --out <budget-real.csv> --jobs-out
  <budget-real-jobs.csv>`: legge le cartelle `<classe>-c<N>/` scritte da
  `run_budget_real.sh` (`jobs.jsonl`, `reports.jsonl`, `exit_code`, `parent_alive`,
  `cg-before.txt`, `cg-after.txt`) e scrive una riga per job ammesso e una riga per classe e
  concorrenza, con `m10_pass` calcolato qui secondo M10 e mai corretto a mano.

Regole di `summarize` (M10 di `RISULTATI-isolamento.md`, scritte prima della misura):
- prenotazione per job = `memory_reservation_bytes` × (1 + discendenti di `parse` = 0) +
  `out_quota_bytes`, dal profilo `parse` di `limits.json` se c'è, altrimenti `default`;
  `address_space_bytes` è solo il `RLIMIT_AS` e non entra; un profilo senza
  `memory_reservation_bytes` fa uscire 2 senza ripieghi;
- `job_errors` = righe `job_error` più `launch_errors` della riga `batch_summary`;
  `completed` = rapporti `RunReport` scritti (job arrivati in fondo, qualunque esito);
- `oom_kill_delta` e `memory_max_events_delta` = differenza fra `cg-after` e `cg-before`;
  `margin_kb` = `memory.max` − `memory.peak` (registrato, non soglia);
- `parent_alive` = `session.sh ps --assert-only worker` uscito 0 e `batch` non terminato da un
  segnale (exit < 128);
- `m10_pass` = padre vivo, exit di `batch` 0, `memory.max` limitato, riga `batch_summary`
  presente, `oom_kill_delta` = 0, nessun `job_error`, `leftover_procs` = 0 su ogni job e, solo
  per `benign`, rapporto prenotazione/`peak_rss_kb` ≥ 1 su ogni job ammesso (un picco assente
  conta come rapporto non verificato).

**Modifica:** Claude (Anthropic) · 2026-10-22 · driver dei job reali e riepilogo di M10 (01-18)
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
import sys
from pathlib import Path

CLASSES = ("benign", "hostile")
CONCURRENCIES = (1, 4, 8, 16)
JOB_ID_RE = re.compile(r"^[a-z0-9-]{1,64}$")
RUN_TAG_RE = re.compile(r"^[a-z0-9]{8}$")
PARSE_DESCENDANTS = 0

JOBS_HEADER = (
    "class,requested_concurrency,job_id,input,outcome,kind,peak_rss_kb,reservation_kb,"
    "reserve_peak_ratio,wall_ms,oom_kill_delta,leftover_procs,oom_adj_set"
).split(",")
BATCH_HEADER = (
    "class,requested_concurrency,jobs,max_admitted,completed,job_errors,reservation_kb,"
    "peak_rss_kb_max,peak_rss_kb_p95,reserve_peak_ratio_min,cgroup_memory_max_kb,"
    "cgroup_memory_peak_kb,margin_kb,oom_kill_delta,memory_max_events_delta,parent_exit,"
    "parent_alive,m10_pass"
).split(",")

SPIKES = Path(__file__).resolve().parents[2]


class Usage(Exception):
    """Argomento o dato non valido: exit 2."""


# ---- jobs --------------------------------------------------------------------------------


def benign_inputs(spikes: Path) -> list[str]:
    manifest = json.loads((spikes / "data/corpus/manifest.json").read_text(encoding="utf-8"))
    raw = spikes / "data/corpus/raw"
    out = []
    for d in manifest:
        if d.get("format") not in ("pdf", "docx"):
            continue
        name = f"{d['doc_id']}.{d['format']}"
        if (raw / name).is_file():
            out.append(f"/data/corpus/raw/{name}")
    return out


def hostile_inputs(spikes: Path) -> list[str]:
    manifest = json.loads((spikes / "data/hostile/manifest.json").read_text(encoding="utf-8"))
    return [f"/data/hostile/generated/{c['file']}" for c in manifest]


def cmd_jobs(cls: str, n: int, tag: str, out: Path, spikes: Path) -> int:
    if cls not in CLASSES or n not in CONCURRENCIES or not RUN_TAG_RE.match(tag):
        raise Usage("classe benign|hostile, concorrenza 1|4|8|16, run-tag [a-z0-9]{8}")
    inputs = benign_inputs(spikes) if cls == "benign" else hostile_inputs(spikes)
    if not inputs:
        raise Usage(f"nessun documento per la classe {cls}")
    count = max(2 * n, len(inputs))
    letter = cls[0]
    lines = []
    for k in range(count):
        job_id = f"brl-{letter}-c{n}-j{k}-{tag}"
        if len(job_id) > 64 or not JOB_ID_RE.match(job_id):
            raise Usage(f"job_id non valido: {job_id}")
        job = {
            "job_id": job_id,
            "op": "parse",
            "input": inputs[k % len(inputs)],
            "out_dir": f"/results/raw/isolation/{job_id}",
            "params": {},
        }
        lines.append(json.dumps(job, separators=(",", ":")))
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_bytes(("\n".join(lines) + "\n").encode("utf-8"))
    print(f"jobs: {count} job {cls} c{n} in {out}")
    return 0


# ---- summarize ---------------------------------------------------------------------------


def reservation_bytes(limits_path: Path) -> tuple[int, str]:
    limits = json.loads(limits_path.read_text(encoding="utf-8"))
    profile = "parse" if "parse" in limits else "default"
    p = limits.get(profile)
    if not isinstance(p, dict) or "memory_reservation_bytes" not in p:
        raise Usage(f"profilo {profile} di limits.json senza memory_reservation_bytes")
    mrb = int(p["memory_reservation_bytes"])
    quota = int(p.get("out_quota_bytes", 0))
    return mrb * (1 + PARSE_DESCENDANTS) + quota, profile


def read_text(path: Path) -> str | None:
    try:
        return path.read_text(encoding="utf-8").replace("\r", "")
    except OSError:
        return None


def parse_cgroup(text: str | None) -> dict:
    """`cat memory.peak memory.max memory.events`: picco, massimo (int o `max`), eventi."""
    if text is None:
        return {}
    lines = [l.strip() for l in text.splitlines() if l.strip()]
    if len(lines) < 2:
        return {}
    events = {}
    for l in lines[2:]:
        k, _, v = l.partition(" ")
        if v.strip().isdigit():
            events[k] = int(v)
    peak = int(lines[0]) if lines[0].isdigit() else None
    mmax: int | str | None = int(lines[1]) if lines[1].isdigit() else lines[1]
    return {"peak": peak, "max": mmax, "events": events}


def p95(values: list[int]) -> int | None:
    if not values:
        return None
    s = sorted(values)
    return s[max(0, math.ceil(0.95 * len(s)) - 1)]


def fmt(v) -> str:
    if v is None:
        return ""
    if isinstance(v, bool):
        return "true" if v else "false"
    if isinstance(v, float):
        return f"{v:.3f}"
    return str(v)


def outcome_cols(outcome: dict) -> tuple[str, str]:
    status = outcome.get("status", "")
    if status == "failed":
        return status, outcome.get("kind", "")
    if status == "quarantined":
        return status, ";".join(outcome.get("reasons", []))
    return status, ""


def summarize_dir(cls: str, n: int, d: Path, res_bytes: int):
    jobs_lines = [l for l in (read_text(d / "jobs.jsonl") or "").splitlines() if l.strip()]
    inputs = {}
    for l in jobs_lines:
        j = json.loads(l)
        inputs[j["job_id"]] = j["input"]
    reports, job_errors, summary = [], 0, None
    for l in (read_text(d / "reports.jsonl") or "").splitlines():
        l = l.strip()
        if not l.startswith("{"):
            continue
        r = json.loads(l)
        if r.get("status") == "job_error":
            job_errors += 1
        elif r.get("status") == "batch_summary":
            summary = r
        elif "outcome" in r:
            reports.append(r)
    if summary is not None:
        job_errors += int(summary.get("launch_errors", 0))
    exit_text = (read_text(d / "exit_code") or "").strip()
    parent_exit = int(exit_text) if exit_text.lstrip("-").isdigit() else None
    ps_text = (read_text(d / "parent_alive") or "").strip()
    parent_alive = ps_text == "0" and parent_exit is not None and 0 <= parent_exit < 128
    before = parse_cgroup(read_text(d / "cg-before.txt"))
    after = parse_cgroup(read_text(d / "cg-after.txt"))

    res_kb = res_bytes // 1024
    job_rows, peaks, ratios = [], [], []
    ratio_ok = True
    leftovers_ok = True
    for r in reports:
        status, kind = outcome_cols(r["outcome"])
        peak = r.get("peak_rss_kb")
        ratio = None
        if isinstance(peak, int) and peak > 0:
            peaks.append(peak)
            ratio = res_bytes / (peak * 1024)
            ratios.append(ratio)
            if ratio < 1:
                ratio_ok = False
        else:
            ratio_ok = False
        if r.get("leftover_procs", 1) != 0:
            leftovers_ok = False
        job_rows.append(
            [
                cls,
                n,
                r["job_id"],
                inputs.get(r["job_id"], ""),
                status,
                kind,
                fmt(peak),
                res_kb,
                fmt(ratio),
                fmt(r.get("wall_ms")),
                fmt(r.get("oom_kill_delta")),
                fmt(r.get("leftover_procs")),
                fmt(r.get("oom_adj_set")),
            ]
        )

    ev_b, ev_a = before.get("events", {}), after.get("events", {})
    oom_delta = (
        ev_a["oom_kill"] - ev_b["oom_kill"] if "oom_kill" in ev_a and "oom_kill" in ev_b else None
    )
    max_delta = ev_a["max"] - ev_b["max"] if "max" in ev_a and "max" in ev_b else None
    mmax, mpeak = after.get("max"), after.get("peak")
    limited = isinstance(mmax, int)
    max_kb = mmax // 1024 if limited else None
    peak_kb = mpeak // 1024 if isinstance(mpeak, int) else None
    margin_kb = max_kb - peak_kb if max_kb is not None and peak_kb is not None else None
    m10 = (
        parent_alive
        and parent_exit == 0
        and limited
        and summary is not None
        and oom_delta == 0
        and job_errors == 0
        and leftovers_ok
        and len(reports) > 0
        and (cls != "benign" or ratio_ok)
    )
    batch_row = [
        cls,
        n,
        len(jobs_lines),
        fmt(summary.get("max_admitted") if summary else None),
        len(reports),
        job_errors,
        res_kb,
        fmt(max(peaks) if peaks else None),
        fmt(p95(peaks)),
        fmt(min(ratios) if ratios else None),
        fmt(max_kb if limited else mmax),
        fmt(peak_kb),
        fmt(margin_kb),
        fmt(oom_delta),
        fmt(max_delta),
        fmt(parent_exit),
        fmt(parent_alive),
        fmt(m10),
    ]
    return batch_row, job_rows


def write_csv(path: Path, header: list[str], rows: list[list]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("w", encoding="utf-8", newline="") as f:
        w = csv.writer(f, lineterminator="\n")
        w.writerow(header)
        for r in rows:
            w.writerow([fmt(x) if not isinstance(x, str) else x for x in r])


def cmd_summarize(raw: Path, limits: Path, out: Path, jobs_out: Path) -> int:
    res_bytes, profile = reservation_bytes(limits)
    batch_rows, job_rows = [], []
    for cls in CLASSES:
        for n in CONCURRENCIES:
            d = raw / f"{cls}-c{n}"
            if not d.is_dir():
                print(f"summarize: manca {d}: riga con m10_pass=false", file=sys.stderr)
            b, j = summarize_dir(cls, n, d, res_bytes)
            batch_rows.append(b)
            job_rows.extend(j)
    write_csv(out, BATCH_HEADER, batch_rows)
    write_csv(jobs_out, JOBS_HEADER, job_rows)
    print(
        f"summarize: profilo {profile}, prenotazione {res_bytes} byte, "
        f"{len(batch_rows)} batch, {len(job_rows)} job"
    )
    return 0


def main(argv: list[str]) -> int:
    ap = argparse.ArgumentParser(description="Budget sui job parse reali (01-18, M10)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    j = sub.add_parser("jobs")
    j.add_argument("--class", dest="cls", required=True)
    j.add_argument("--concurrency", required=True, type=int)
    j.add_argument("--run-tag", required=True)
    j.add_argument("--out", required=True, type=Path)
    j.add_argument("--spikes-dir", type=Path, default=SPIKES)
    s = sub.add_parser("summarize")
    s.add_argument("--raw", required=True, type=Path)
    s.add_argument("--limits", required=True, type=Path)
    s.add_argument("--out", required=True, type=Path)
    s.add_argument("--jobs-out", required=True, type=Path)
    try:
        args = ap.parse_args(argv)
    except SystemExit as e:
        return 2 if e.code else 0
    try:
        if args.cmd == "jobs":
            return cmd_jobs(args.cls, args.concurrency, args.run_tag, args.out, args.spikes_dir)
        return cmd_summarize(args.raw, args.limits, args.out, args.jobs_out)
    except Usage as e:
        print(f"budget_real: {e}", file=sys.stderr)
        return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
