#!/usr/bin/env python3
"""
boundary_check.py - confini fra i crate del workspace Cargo di CKA.
Zero dipendenze esterne (stdlib Python 3).

Due modalita':

  scan (default)   python boundary_check.py [ROOT | --root <dir>] [--config <json>] [--json]
      Legge il workspace con `cargo metadata --format-version 1 --no-deps` e
      verifica: (1) ogni membro del workspace e' dichiarato in boundaries.json;
      (2) le dipendenze interne (path) puntano solo ai crate ammessi, e mai da
      un servizio a un altro servizio; (3) i crate con `forbidden_cargo_deps`
      non dichiarano quelle dipendenze; (4) i file Rust sotto i `pure_paths`
      (moduli `domain` dei servizi, l'intero `contracts`) non usano crate
      esterni vietati (sqlx, reqwest, axum, tower, hyper, SDK provider...) ne'
      moduli di adapter dello stesso crate.

  plan             python boundary_check.py --plan <PLAN.md> [<PLAN.md> ...] [--json]
      Non serve cargo. Dal PLAN legge `files_modified` (frontmatter), la mappa
      opzionale `crate_deps` (frontmatter), i comandi `cargo add` e i blocchi
      ```toml preceduti da un `<crate>/Cargo.toml`, e i blocchi ```rust
      preceduti da un file sotto un pure_path. Segnala: file in un crate non
      dichiarato, dipendenze interne non ammesse, dipendenze vietate nei
      moduli puri. Le righe trovate sono CANDIDATI: il verdetto lo scrive
      l'agente-gate.

Esito (regola anti-falso-verde: solo 0 e' verde):
    0  nessuna violazione
    1  almeno una violazione
    2  non eseguito: configurazione assente/illeggibile/incompleta, nessun
       Cargo.toml nella radice, cargo non risolvibile, cargo metadata fallito,
       workspace senza membri, PLAN assente o senza files_modified.
       Uno scope vuoto non e' mai verde.
"""
import argparse
import fnmatch
import json
import os
import re
import shutil
import subprocess
import sys

EXIT_OK, EXIT_FAIL, EXIT_NOT_RUN = 0, 1, 2

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_CONFIG_PATH = os.path.join(HERE, "boundaries.json")
# Radice del repo ancorata alla posizione dello script (skills/<nome>/), mai
# alla directory corrente: l'esito non cambia a seconda di dove si invoca.
DEFAULT_ROOT = os.path.normpath(os.path.join(HERE, "..", ".."))

FIXTURE_MARKER = ".boundary-check-fixture"
CARGO_TIMEOUT_S = 180


class NotRun(Exception):
    """Controllo non eseguibile. Distinto da una violazione: si corregge
    nell'ambiente o nella configurazione, non nel codice sorvegliato."""

    def __init__(self, code, message):
        super().__init__(message)
        self.code = code
        self.message = message


# --------------------------------------------------------------------------
# Configurazione
# --------------------------------------------------------------------------

def norm_crate(name):
    return name.replace("-", "_").lower()


def load_config(path):
    if not os.path.exists(path):
        raise NotRun("boundaries-config-missing",
                     f"configurazione assente: '{path}' non esiste. Il gate non "
                     "valida contro regole implicite: senza la mappa dei crate un "
                     "verde sarebbe un verde su zero soggetti valutati.")
    try:
        with open(path, encoding="utf-8") as fh:
            cfg = json.load(fh)
    except (OSError, ValueError, UnicodeDecodeError) as exc:
        raise NotRun("boundaries-config-unreadable",
                     f"configurazione illeggibile: '{path}' ({exc}).")
    crates = cfg.get("crates") if isinstance(cfg, dict) else None
    if not isinstance(crates, dict) or not crates:
        raise NotRun("boundaries-config-incomplete",
                     f"configurazione incompleta: '{path}' non dichiara una chiave "
                     "'crates' non vuota.")
    for name, spec in crates.items():
        if not isinstance(spec, dict) or not spec.get("path") or not spec.get("kind"):
            raise NotRun("boundaries-config-incomplete",
                         f"crate '{name}' senza 'path' o 'kind' in '{path}'.")
        spec["path"] = spec["path"].replace("\\", "/").strip("/")
        spec.setdefault("allowed_internal", [])
        spec.setdefault("pure_paths", [])
        spec.setdefault("forbidden_cargo_deps", [])
    cfg.setdefault("pure_forbidden_crates", [])
    cfg.setdefault("pure_forbidden_modules", [])
    cfg.setdefault("composition_roots", ["src/main.rs", "src/app.rs", "src/app/"])
    cfg.setdefault("non_crate_areas", [])
    return cfg


def clean_rel(rel):
    """Separatori uniformi e via il solo prefisso `./` (mai `.planning`)."""
    rel = rel.replace("\\", "/").strip()
    while rel.startswith("./"):
        rel = rel[2:]
    return rel


def is_composition_root(cfg, rel_in_crate):
    """Composition root = `src/main.rs`, `src/app.rs` e l'intero sottoalbero
    `src/app/` (un `compose()` per feature). Una voce che finisce con `/`
    vale come sottoalbero."""
    rel_in_crate = clean_rel(rel_in_crate)
    for r in cfg["composition_roots"]:
        if rel_in_crate == r or (r.endswith("/") and rel_in_crate.startswith(r)):
            return True
    return False


def crate_by_relpath(cfg, rel):
    """Il crate dichiarato che contiene il percorso relativo `rel`, o None."""
    rel = clean_rel(rel)
    best = None
    for name, spec in cfg["crates"].items():
        p = spec["path"]
        if rel == p or rel.startswith(p + "/"):
            if best is None or len(p) > len(cfg["crates"][best]["path"]):
                best = name
    return best


def in_pure_path(cfg, crate, rel_in_crate):
    rel_in_crate = rel_in_crate.replace("\\", "/")
    for pp in cfg["crates"][crate]["pure_paths"]:
        pp = pp.strip("/")
        if rel_in_crate == pp or rel_in_crate.startswith(pp + "/") \
                or rel_in_crate == pp + ".rs":
            return True
    return False


# --------------------------------------------------------------------------
# Scansione dei sorgenti Rust (moduli puri)
# --------------------------------------------------------------------------

# Una sola passata da sinistra a destra: commenti, stringhe (anche raw) e
# letterali char. Cosi' un `//` dentro una stringa non apre un commento e un
# `"` dentro un char non apre una stringa.
_RUST_NOISE = re.compile(
    r'/\*.*?\*/'                      # commento a blocco
    r'|//[^\n]*'                      # commento di riga
    r'|r(#*)".*?"\1'                  # stringa raw r"..." / r#"..."#
    r'|b?"(?:\\.|[^"\\])*"'           # stringa (anche byte)
    r"|b?'(?:\\.|[^'\\\n])'",         # char
    re.S)


def strip_rust(text):
    """Rimuove commenti, stringhe e char preservando i numeri di riga."""
    return _RUST_NOISE.sub(lambda m: re.sub(r"[^\n]", " ", m.group(0)), text)


def forbidden_uses(code, cfg):
    """[(riga, simbolo, motivo)] per uso di crate o moduli vietati."""
    out = []
    crates = sorted({norm_crate(c) for c in cfg["pure_forbidden_crates"]}, key=len, reverse=True)
    if crates:
        alt = "|".join(map(re.escape, crates))
        rx_path = re.compile(r"(?<![\w:])(?:::)?(" + alt + r")\s*::")
        rx_use = re.compile(r"\buse\s+(?:::)?(" + alt + r")\b")
        rx_ext = re.compile(r"\bextern\s+crate\s+(" + alt + r")\b")
    mods = cfg["pure_forbidden_modules"]
    rx_mod = (re.compile(r"\b(" + "|".join(re.escape(m) for m in mods) + r")\b")
              if mods else None)
    for i, line in enumerate(code.splitlines(), 1):
        seen = set()
        if crates:
            for rx in (rx_use, rx_ext, rx_path):
                for m in rx.finditer(line):
                    if m.group(1) not in seen:
                        seen.add(m.group(1))
                        out.append((i, m.group(1), "crate esterno vietato in un modulo puro"))
        if rx_mod:
            for m in rx_mod.finditer(line):
                out.append((i, m.group(1), "modulo di adapter usato da un modulo puro"))
    return out


def walk_pure_files(cfg, root, crate, excluded):
    spec = cfg["crates"][crate]
    base = os.path.join(root, spec["path"])
    files = []
    for pp in spec["pure_paths"]:
        target = os.path.join(base, pp)
        if os.path.isfile(target + ".rs"):
            files.append(target + ".rs")
        if os.path.isfile(target) and target.endswith(".rs"):
            files.append(target)
        if os.path.isdir(target):
            for r, dirs, fs in os.walk(target):
                if FIXTURE_MARKER in fs:
                    excluded.append(r)
                    dirs[:] = []
                    continue
                dirs[:] = [d for d in dirs if d not in ("target", ".git")]
                files.extend(os.path.join(r, f) for f in fs if f.endswith(".rs"))
    return sorted(set(files))


# --------------------------------------------------------------------------
# Modalita' scan
# --------------------------------------------------------------------------

def cargo_metadata(root):
    manifest = os.path.join(root, "Cargo.toml")
    if not os.path.isfile(manifest):
        raise NotRun("cargo-workspace-missing",
                     f"nessun Cargo.toml in '{root}': il workspace Rust non esiste "
                     "ancora. Controllo NON eseguito (non e' un verde).")
    cargo = shutil.which("cargo")
    if not cargo:
        raise NotRun("cargo-missing",
                     "cargo non risolvibile nel PATH: controllo NON eseguito.")
    try:
        proc = subprocess.run(
            [cargo, "metadata", "--format-version", "1", "--no-deps",
             "--manifest-path", manifest],
            capture_output=True, text=True, encoding="utf-8", errors="replace",
            timeout=CARGO_TIMEOUT_S)
    except subprocess.TimeoutExpired:
        raise NotRun("cargo-metadata-timeout",
                     f"cargo metadata oltre {CARGO_TIMEOUT_S}s: controllo NON eseguito.")
    if proc.returncode != 0:
        raise NotRun("cargo-metadata-failed",
                     "cargo metadata e' fallito (rc={}): {}".format(
                         proc.returncode, proc.stderr.strip()[:800]))
    try:
        return json.loads(proc.stdout)
    except ValueError as exc:
        raise NotRun("cargo-metadata-unreadable", f"output di cargo metadata non JSON ({exc}).")


def relpath(path, root):
    return os.path.relpath(os.path.normpath(path), os.path.normpath(root)).replace("\\", "/")


def run_scan(cfg, root):
    meta = cargo_metadata(root)
    ws_root = meta.get("workspace_root") or root
    members = set(meta.get("workspace_members") or [])
    pkgs = [p for p in meta.get("packages", []) if not members or p.get("id") in members]
    if not pkgs:
        raise NotRun("cargo-workspace-empty",
                     "il workspace non ha membri: nessun crate da giudicare. "
                     "Controllo NON eseguito (non e' un verde).")

    violations, notes, excluded = [], [], []
    present = {}  # crate dichiarato -> nome package
    pkg_dir_to_crate = {}
    for p in pkgs:
        rel_dir = relpath(os.path.dirname(p["manifest_path"]), ws_root)
        crate = crate_by_relpath(cfg, rel_dir)
        if crate is None or cfg["crates"][crate]["path"] != rel_dir:
            violations.append(dict(rule="crate-undeclared", source=p["name"], target=rel_dir,
                                   reason="membro del workspace non dichiarato in "
                                          "boundaries.json (nessun quarto bounded context "
                                          "senza ADR e aggiornamento della mappa)"))
            continue
        present[crate] = p["name"]
        pkg_dir_to_crate[os.path.normcase(os.path.normpath(os.path.dirname(p["manifest_path"])))] = crate

    for crate in cfg["crates"]:
        if crate not in present:
            notes.append(f"[NON PRESENTE] {crate} ({cfg['crates'][crate]['path']}): "
                         "dichiarato ma non ancora nel workspace")

    # Dipendenze dichiarate in Cargo.toml
    for p in pkgs:
        src_dir = os.path.normcase(os.path.normpath(os.path.dirname(p["manifest_path"])))
        crate = pkg_dir_to_crate.get(src_dir)
        if crate is None:
            continue
        spec = cfg["crates"][crate]
        forbidden_cargo = {norm_crate(d) for d in spec["forbidden_cargo_deps"]}
        for d in p.get("dependencies", []):
            kind = d.get("kind") or "normal"
            dep_name = d.get("name", "")
            if d.get("path"):
                tgt = pkg_dir_to_crate.get(os.path.normcase(os.path.normpath(d["path"])))
                if tgt is None:
                    tgt_rel = relpath(d["path"], ws_root)
                    tgt = crate_by_relpath(cfg, tgt_rel)
                    if tgt is None:
                        violations.append(dict(rule="internal-dep-undeclared", source=crate,
                                               target=tgt_rel,
                                               reason=f"dipendenza path ({kind}) verso un crate "
                                                      "non dichiarato"))
                        continue
                if cfg["crates"][tgt]["kind"] == "service":
                    violations.append(dict(rule="service-to-service", source=crate, target=tgt,
                                           reason=f"dipendenza ({kind}) da un servizio a un "
                                                  "altro servizio: i dati attraversano i "
                                                  "confini solo via contratti HTTP o outbox"))
                elif tgt not in spec["allowed_internal"]:
                    violations.append(dict(rule="internal-dep-not-allowed", source=crate,
                                           target=tgt,
                                           reason=f"dipendenza interna ({kind}) non ammessa; "
                                                  f"ammesse: {spec['allowed_internal'] or 'nessuna'}"))
            elif norm_crate(dep_name) in forbidden_cargo:
                violations.append(dict(rule="forbidden-cargo-dep", source=crate, target=dep_name,
                                       reason=f"dipendenza ({kind}) vietata per questo crate"))

    # Moduli puri
    scanned = 0
    for crate in present:
        for f in walk_pure_files(cfg, ws_root, crate, excluded):
            scanned += 1
            try:
                with open(f, encoding="utf-8", errors="replace") as fh:
                    code = strip_rust(fh.read())
            except OSError:
                continue
            for line, sym, why in forbidden_uses(code, cfg):
                violations.append(dict(rule="pure-module-forbidden-use", source=crate,
                                       target=sym, file=relpath(f, ws_root), line=line,
                                       reason=why))
    notes.append(f"File Rust analizzati nei moduli puri: {scanned}")

    # Wiring fuori dal composition root (solo servizi): pool, listener e
    # server si costruiscono in src/main.rs, src/app.rs o src/app/**.
    pats = cfg.get("wiring_patterns") or []
    rx_wiring = re.compile("|".join(re.escape(p) for p in pats)) if pats else None
    wired = 0
    for crate in present:
        spec = cfg["crates"][crate]
        if spec["kind"] != "service" or rx_wiring is None:
            continue
        src = os.path.join(ws_root, spec["path"], "src")
        for r, dirs, fs in os.walk(src):
            if FIXTURE_MARKER in fs:
                excluded.append(r)
                dirs[:] = []
                continue
            for f in fs:
                if not f.endswith(".rs"):
                    continue
                full = os.path.join(r, f)
                in_crate = relpath(full, os.path.join(ws_root, spec["path"]))
                if is_composition_root(cfg, in_crate):
                    continue
                wired += 1
                try:
                    with open(full, encoding="utf-8", errors="replace") as fh:
                        code = strip_rust(fh.read())
                except OSError:
                    continue
                for i, line in enumerate(code.splitlines(), 1):
                    m = rx_wiring.search(line)
                    if m:
                        violations.append(dict(
                            rule="wiring-outside-composition-root", source=crate,
                            target=m.group(0), file=relpath(full, ws_root), line=i,
                            reason="wiring (pool/listener/server) fuori dal composition "
                                   "root: va in src/main.rs, src/app.rs o src/app/**"))
    notes.append(f"File Rust analizzati per il wiring fuori dal composition root: {wired}")
    notes.extend(f"[ESCLUSA] {relpath(d, ws_root)} (marcatore {FIXTURE_MARKER})" for d in excluded)
    return violations, notes, len(pkgs)


# --------------------------------------------------------------------------
# Modalita' plan
# --------------------------------------------------------------------------

_FRONT = re.compile(r"\A﻿?---\s*\n(.*?)\n---\s*(?:\n|\Z)", re.S)


def _scalar(v):
    v = v.strip()
    if len(v) >= 2 and v[0] == v[-1] and v[0] in "'\"":
        v = v[1:-1]
    return v


def _inline_list(v):
    v = v.strip()
    if v.startswith("[") and v.endswith("]"):
        return [_scalar(x) for x in v[1:-1].split(",") if x.strip()]
    return None


def parse_frontmatter(text):
    """Parser minimo per le forme usate dai PLAN di GSD: `k: v`, `k: [a, b]`,
    liste a trattino e una mappa di un livello (`crate_deps:` -> `crate: [..]`)."""
    m = _FRONT.match(text)
    if not m:
        return {}
    data, key, sub = {}, None, None
    for raw in m.group(1).splitlines():
        if not raw.strip() or raw.lstrip().startswith("#"):
            continue
        indent = len(raw) - len(raw.lstrip())
        line = raw.strip()
        if indent == 0:
            sub = None
            if ":" not in line:
                continue
            key, val = line.split(":", 1)
            key = key.strip()
            lst = _inline_list(val)
            data[key] = lst if lst is not None else (_scalar(val) if val.strip() else None)
            continue
        if key is None:
            continue
        if line.startswith("- "):
            item = _scalar(line[2:])
            if sub is not None:
                data[key].setdefault(sub, [])
                data[key][sub].append(item)
            else:
                if not isinstance(data.get(key), list):
                    data[key] = []
                data[key].append(item)
        elif ":" in line:
            k2, v2 = line.split(":", 1)
            if not isinstance(data.get(key), dict):
                data[key] = {}
            lst = _inline_list(v2)
            data[key][k2.strip()] = lst if lst is not None else ([] if not v2.strip() else [_scalar(v2)])
            sub = k2.strip()
    return data


_CARGO_ADD = re.compile(r"cargo\s+add\s+([^\n`]+)")
_FENCE = re.compile(r"^```(\w*)\s*\n(.*?)^```", re.S | re.M)
_PATH_MENTION = re.compile(r"([\w./\\-]+\.(?:rs|toml))")
_TOML_SECTION = re.compile(r"^\s*\[([^\]]+)\]\s*$")
_TOML_DEP = re.compile(r"^\s*([A-Za-z0-9_-]+)\s*=")
_TOML_PATH = re.compile(r"path\s*=\s*\"([^\"]+)\"")


def parse_cargo_add(args):
    toks = args.split()
    deps, pkg, i = [], None, 0
    while i < len(toks):
        t = toks[i]
        if t in ("-p", "--package") and i + 1 < len(toks):
            pkg = toks[i + 1]
            i += 2
            continue
        if t.startswith("--package="):
            pkg = t.split("=", 1)[1]
        elif t.startswith("--path") or t.startswith("--features") or t.startswith("-F"):
            if "=" not in t:
                i += 1
        elif not t.startswith("-"):
            deps.append(re.split(r"[@=]", t)[0])
        i += 1
    return pkg, deps


def resolve_crate_name(cfg, name):
    """Nome di package o di crate dichiarato -> chiave in boundaries.json."""
    if name is None:
        return None
    n = norm_crate(name)
    for k, spec in cfg["crates"].items():
        if norm_crate(k) == n or norm_crate(os.path.basename(spec["path"])) == n:
            return k
    for k in cfg["crates"]:
        # prefisso di progetto (`cka-contracts`), mai un suffisso nudo:
        # `opentelemetry` non e' il crate interno `telemetry`.
        if n.endswith("_" + norm_crate(k)) and not n.startswith("open"):
            return k
    return None


def last_path_before(text, pos, window=400):
    chunk = text[max(0, pos - window):pos]
    found = _PATH_MENTION.findall(chunk)
    return clean_rel(found[-1]) if found else None


def check_dep(cfg, crate, dep, origin, out):
    """Giudica una dipendenza dichiarata dal piano per `crate`."""
    tgt = resolve_crate_name(cfg, dep)
    spec = cfg["crates"][crate]
    if tgt is not None:
        if tgt == crate:
            return
        if cfg["crates"][tgt]["kind"] == "service":
            out.append(dict(rule="service-to-service", source=crate, target=tgt, origin=origin,
                            reason="il piano fa dipendere un servizio da un altro servizio"))
        elif tgt not in spec["allowed_internal"]:
            out.append(dict(rule="internal-dep-not-allowed", source=crate, target=tgt,
                            origin=origin,
                            reason=f"dipendenza interna non ammessa; ammesse: "
                                   f"{spec['allowed_internal'] or 'nessuna'}"))
    elif norm_crate(dep) in {norm_crate(d) for d in spec["forbidden_cargo_deps"]}:
        out.append(dict(rule="forbidden-cargo-dep", source=crate, target=dep, origin=origin,
                        reason="dipendenza vietata per questo crate"))


def run_plan(cfg, plan_path):
    if not os.path.isfile(plan_path):
        raise NotRun("plan-missing", f"PLAN '{plan_path}' non esiste.")
    with open(plan_path, encoding="utf-8", errors="replace") as fh:
        text = fh.read()
    fm = parse_frontmatter(text)
    files = fm.get("files_modified")
    if isinstance(files, str):
        files = [files] if files else []
    if not files:
        raise NotRun("plan-scope-empty",
                     f"'{plan_path}' non dichiara files_modified: scope vuoto, controllo "
                     "NON eseguito. Un piano senza files_modified torna a `plan`.")

    out, notes = [], []
    touched = set()
    for f in files:
        rel = clean_rel(f)
        crate = crate_by_relpath(cfg, rel)
        if crate:
            touched.add(crate)
            in_crate = rel[len(cfg["crates"][crate]["path"]) + 1:]
            if is_composition_root(cfg, in_crate):
                notes.append(f"[COMPOSITION ROOT] {rel}: il wiring DI sta qui, ed e' un "
                             "wiring point (vedi WIRING-MAP)")
            continue
        if re.match(r"^(services|crates)/[^/]+", rel):
            out.append(dict(rule="crate-undeclared", source=rel,
                            target=re.match(r"^(services|crates)/[^/]+", rel).group(0),
                            origin="files_modified",
                            reason="file in un crate non dichiarato in boundaries.json"))
        elif not any(rel == a or rel.startswith(a) for a in cfg["non_crate_areas"]) \
                and not any(fnmatch.fnmatch(rel, a) for a in cfg["non_crate_areas"]):
            notes.append(f"[FUORI MAPPA] {rel}: area non riconosciuta, giudicare a mano")
    notes.append("Crate toccati dal piano: " + (", ".join(sorted(touched)) or "nessuno"))

    # crate_deps nel frontmatter (convenzione opzionale)
    cd = fm.get("crate_deps")
    if isinstance(cd, dict):
        for c, deps in cd.items():
            crate = resolve_crate_name(cfg, c)
            if crate is None:
                out.append(dict(rule="crate-undeclared", source=c, target="-",
                                origin="crate_deps", reason="crate non dichiarato"))
                continue
            for d in deps or []:
                check_dep(cfg, crate, d, "crate_deps", out)

    # cargo add -p <crate> <deps>
    for m in _CARGO_ADD.finditer(text):
        pkg, deps = parse_cargo_add(m.group(1))
        crate = resolve_crate_name(cfg, pkg)
        line = text.count("\n", 0, m.start()) + 1
        if crate is None:
            if len(touched) == 1:
                crate = next(iter(touched))
            else:
                notes.append(f"[NON ATTRIBUITO] riga {line}: `cargo add` senza -p e piano "
                             "multi-crate: giudicare a mano")
                continue
        for d in deps:
            check_dep(cfg, crate, d, f"cargo add (riga {line})", out)

    # blocchi di codice preceduti dal percorso del file
    for m in _FENCE.finditer(text):
        lang, body = m.group(1).lower(), m.group(2)
        path = last_path_before(text, m.start())
        line0 = text.count("\n", 0, m.start()) + 2
        crate = crate_by_relpath(cfg, path) if path else None
        if lang == "toml" and path and path.endswith("Cargo.toml") and crate:
            section = ""
            for k, l in enumerate(body.splitlines()):
                s = _TOML_SECTION.match(l)
                if s:
                    section = s.group(1)
                    continue
                if "dependencies" not in section:
                    continue
                d = _TOML_DEP.match(l)
                if d:
                    dep = d.group(1)
                    pm = _TOML_PATH.search(l)
                    if pm:
                        dep = os.path.basename(pm.group(1).rstrip("/"))
                    check_dep(cfg, crate, dep, f"{path} (riga {line0 + k})", out)
        elif lang in ("rust", "rs") and path and crate:
            in_crate = path[len(cfg["crates"][crate]["path"]) + 1:]
            if in_pure_path(cfg, crate, in_crate):
                for ln, sym, why in forbidden_uses(strip_rust(body), cfg):
                    out.append(dict(rule="pure-module-forbidden-use", source=crate, target=sym,
                                    origin=f"{path} (riga {line0 + ln - 1})", reason=why))
    return out, notes


# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

def emit_not_run(exc, as_json):
    if as_json:
        print(json.dumps({"error": exc.code, "message": exc.message, "status": "not-run"},
                         indent=2, ensure_ascii=False))
    else:
        print(f"NON ESEGUITO [{exc.code}] {exc.message}", file=sys.stderr)
    return EXIT_NOT_RUN


def fmt_violation(v):
    where = v.get("file")
    where = f" ({where}:{v['line']})" if where else (f" ({v['origin']})" if v.get("origin") else "")
    return f"[VIOLAZIONE {v['rule']}] {v['source']} -> {v['target']}{where}\n             {v['reason']}"


def main():
    ap = argparse.ArgumentParser(prog="boundary_check.py",
                                 description="Confini fra i crate del workspace CKA. "
                                             "Esito 0 pass, 1 fail, 2 non eseguito.")
    # Radice come posizionale (forma degli hook: `boundary_check.py .`) o con
    # --root; senza nessuna delle due, la radice del repo ancorata allo script.
    ap.add_argument("root_pos", nargs="?", metavar="ROOT",
                    help="radice del workspace Cargo (equivalente a --root)")
    ap.add_argument("--root", default=None,
                    help=f"radice del workspace Cargo (default: {DEFAULT_ROOT})")
    ap.add_argument("--config", default=DEFAULT_CONFIG_PATH)
    ap.add_argument("--plan", nargs="+", metavar="PLAN.md",
                    help="modalita' plan: giudica uno o piu' PLAN.md invece del workspace")
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()

    try:
        cfg = load_config(args.config)
    except NotRun as exc:
        return emit_not_run(exc, args.json)

    if args.plan:
        results, worst = [], EXIT_OK
        for p in args.plan:
            try:
                v, notes = run_plan(cfg, p)
                rc = EXIT_FAIL if v else EXIT_OK
                results.append(dict(plan=p, status="fail" if v else "pass",
                                    violations=v, notes=notes))
            except NotRun as exc:
                rc = EXIT_NOT_RUN
                results.append(dict(plan=p, status="not-run", error=exc.code,
                                    message=exc.message))
            # 2 domina 1: un piano non giudicabile non si nasconde dietro un rosso.
            worst = max(worst, rc)
        if args.json:
            print(json.dumps(results, indent=2, ensure_ascii=False))
            return worst
        print(f"== Crate boundary check (plan): {len(results)} piani ==")
        print(f"Configurazione: {args.config}")
        for r in results:
            print(f"\n## {r['plan']}: {r['status'].upper()}")
            if r["status"] == "not-run":
                print(f"NON ESEGUITO [{r['error']}] {r['message']}")
                continue
            for n in r["notes"]:
                print(n)
            for v in r["violations"]:
                print(fmt_violation(v))
        print("\nNB: candidati estratti dal testo del piano; il verdetto motivato lo scrive "
              "l'agente-gate.")
        return worst

    try:
        root = args.root or args.root_pos or DEFAULT_ROOT
        violations, notes, n = run_scan(cfg, os.path.abspath(root))
    except NotRun as exc:
        return emit_not_run(exc, args.json)

    if args.json:
        for n_ in notes:
            print(n_, file=sys.stderr)
        print(json.dumps(violations, indent=2, ensure_ascii=False))
        return EXIT_FAIL if violations else EXIT_OK

    print(f"== Crate boundary check: {n} membri del workspace ==")
    print(f"Configurazione: {args.config}")
    for n_ in notes:
        print(n_)
    print("")
    if not violations:
        print("OK - nessuna violazione dei confini.")
        return EXIT_OK
    for v in violations:
        print(fmt_violation(v))
    print(f"\n{len(violations)} violazioni. FAIL.")
    return EXIT_FAIL


if __name__ == "__main__":
    sys.exit(main())
