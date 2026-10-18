#!/usr/bin/env python3
"""
solid_scan.py - analisi euristica SOLID per CKA: Rust (servizi e crate del
workspace) e TypeScript/React (apps/web). Zero dipendenze (stdlib).
Segnala CANDIDATI a violazione, non verdetti: il verdetto lo scrive l'agente
con la review per principio del SKILL.md.

Uso:
    python solid_scan.py <file_o_cartella> [<file_o_cartella> ...] [--json]
        [--max-impl-lines N] [--max-fn-lines N] [--max-trait-methods N]
        [--max-struct-fields N] [--max-component-lines N] [--max-hooks N]
        [--max-ts-members N] [--min-enum-match-sites N]

Esito:
    0  nessun candidato
    1  almeno un candidato
    2  non eseguito: percorso inesistente oppure nessun file .rs/.ts/.tsx
       nello scope (uno scope vuoto non e' mai verde)
"""
import argparse
import json
import os
import re
import sys

EXIT_OK, EXIT_FINDINGS, EXIT_NOT_RUN = 0, 1, 2

DEFAULTS = dict(max_impl_lines=300, max_fn_lines=60, max_trait_methods=6,
                max_struct_fields=12, max_component_lines=300, max_hooks=6,
                max_ts_members=10, min_enum_match_sites=3)

PRUNED = {"target", "node_modules", "dist", "build", ".git", "coverage",
          "generated", "__generated__", "playwright-report", "test-results"}
FIXTURE_MARKER = ".solid-check-fixture"

# --------------------------------------------------------------------------
# Rimozione di commenti e stringhe (preserva i numeri di riga)
# --------------------------------------------------------------------------

_RUST_NOISE = re.compile(
    r'/\*.*?\*/'
    r'|//[^\n]*'
    r'|(?<![\w])r(#*)".*?"\1'
    r'|b?"(?:\\.|[^"\\])*"'
    r"|b?'(?:\\.|[^'\\\n])'",
    re.S)

_TS_NOISE = re.compile(
    r'/\*.*?\*/'
    r'|//[^\n]*'
    r'|"(?:\\.|[^"\\\n])*"'
    r"|'(?:\\.|[^'\\\n])*'"
    r'|`(?:\\.|[^`\\])*`',
    re.S)


def _blank(m):
    return re.sub(r"[^\n]", " ", m.group(0))


def strip_code(text, lang):
    return (_RUST_NOISE if lang == "rust" else _TS_NOISE).sub(_blank, text)


def block_end(code, open_idx):
    """Indice della graffa che chiude quella in `open_idx` (o len)."""
    depth = 0
    for k in range(open_idx, len(code)):
        c = code[k]
        if c == "{":
            depth += 1
        elif c == "}":
            depth -= 1
            if depth == 0:
                return k
    return len(code)


def line_of(code, idx):
    return code.count("\n", 0, idx) + 1


# --------------------------------------------------------------------------
# Selezione dei file
# --------------------------------------------------------------------------

def lang_of(path):
    p = path.lower()
    if p.endswith(".rs"):
        return "rust"
    if p.endswith((".ts", ".tsx")) and not p.endswith(".d.ts"):
        return "ts"
    return None


def is_test_path(path):
    p = path.replace("\\", "/").lower()
    base = p.rsplit("/", 1)[-1]
    return ("/tests/" in p or "/__tests__/" in p or "/e2e/" in p
            or base.endswith(("_test.rs", "_tests.rs"))
            or re.search(r"\.(test|spec)\.tsx?$", base) is not None)


def is_generated(path, text):
    p = path.replace("\\", "/").lower()
    head = text[:500].lower()
    return (p.endswith((".gen.ts", ".generated.ts"))
            or "@generated" in head or "auto-generated" in head
            or "do not edit" in head)


def find_files(paths, excluded):
    out, seen = [], set()
    for path in paths:
        if os.path.isfile(path):
            cands = [path]
        else:
            cands = []
            for r, dirs, fs in os.walk(path):
                if FIXTURE_MARKER in fs and os.path.abspath(r) != os.path.abspath(path):
                    excluded.append(r)
                    dirs[:] = []
                    continue
                dirs[:] = [d for d in dirs if d not in PRUNED]
                cands.extend(os.path.join(r, f) for f in fs)
        for f in cands:
            if lang_of(f) is None:
                continue
            k = os.path.normcase(os.path.abspath(f))
            if k not in seen:
                seen.add(k)
                out.append(f)
    return out


# --------------------------------------------------------------------------
# Rust
# --------------------------------------------------------------------------

_CFG_TEST = re.compile(r"#\[cfg\(test\)\]\s*(?:pub\s+)?mod\s+\w+\s*\{")
_IMPL = re.compile(r"\bimpl\b(?:\s*<[^{]*?>)?\s+([^{;]+?)\s*\{")
_FN = re.compile(r"\bfn\s+(\w+)\s*(?:<[^{;]*?>)?\s*\([^;{]*?\)[^;{]*\{", re.S)
_TRAIT = re.compile(r"\btrait\s+(\w+)[^{;]*\{")
_STRUCT = re.compile(r"\bstruct\s+(\w+)(?:\s*<[^{;]*?>)?\s*\{")
_MATCH_ARM_ENUM = re.compile(r"\b([A-Z]\w*)::([A-Z]\w*)\b[^=\n]*=>")
_MATCH_STR = re.compile(r"\bmatch\s+[^{]*\.as_str\(\)\s*\{|\bmatch\s+[^{]*\.as_ref\(\)\s*\{")
_DOWNCAST = re.compile(r"\b(downcast_ref|downcast_mut|downcast|type_id|TypeId::of)\b")
_UNIMPL = re.compile(r"\b(unimplemented|todo)!\s*\(")
_STATIC_MUT = re.compile(r"\bstatic\s+mut\b|\blazy_static!\s*\{|"
                         r"\bstatic\s+\w+\s*:\s*[^=;]*(Mutex|RwLock|RefCell|Cell)<")
_BLOCKING = re.compile(r"\bstd::thread::sleep\b|\bthread::sleep\s*\(|\bblock_on\s*\(|"
                       r"\bstd::fs::(read|write|read_to_string)\b")
_CONCRETE_INFRA = re.compile(r"\b(PgPool|sqlx::Pool|reqwest::Client|redis::Client|"
                             r"PgPoolOptions)\b")
_ADAPTER_PATH = re.compile(r"/(infrastructure|adapters|app|db|persistence|http)/|"
                           r"/src/(main|app)\.rs$")


def strip_cfg_test(code):
    out = list(code)
    for m in _CFG_TEST.finditer(code):
        end = block_end(code, m.end() - 1)
        for k in range(m.start(), min(end + 1, len(out))):
            if out[k] != "\n":
                out[k] = " "
    return "".join(out)


def analyze_rust(path, text, cfg, add, enum_sites):
    code = strip_cfg_test(strip_code(text, "rust"))
    norm = path.replace("\\", "/")

    for m in _IMPL.finditer(code):
        brace = m.end() - 1
        n = line_of(code, block_end(code, brace)) - line_of(code, brace)
        if n > cfg["max_impl_lines"]:
            add(line_of(code, m.start()), "SRP",
                f"impl '{m.group(1).strip()[:60]}' ~{n} righe (> {cfg['max_impl_lines']}): "
                "probabile tipo con troppe responsabilita', valuta lo split.")

    for m in _FN.finditer(code):
        brace = m.end() - 1
        n = line_of(code, block_end(code, brace)) - line_of(code, brace)
        if n > cfg["max_fn_lines"]:
            add(line_of(code, m.start()), "SRP",
                f"fn '{m.group(1)}' ~{n} righe (> {cfg['max_fn_lines']}): valuta l'estrazione.")

    for m in _TRAIT.finditer(code):
        brace = m.end() - 1
        body = code[brace:block_end(code, brace)]
        n = len(re.findall(r"\bfn\s+\w+", body))
        if n > cfg["max_trait_methods"]:
            add(line_of(code, m.start()), "ISP",
                f"trait '{m.group(1)}' con {n} metodi (> {cfg['max_trait_methods']}): "
                "valuta trait-porta piu' piccoli per client.")

    for m in _STRUCT.finditer(code):
        brace = m.end() - 1
        body = code[brace + 1:block_end(code, brace)]
        n = len(re.findall(r"^\s*(?:pub(?:\([^)]*\))?\s+)?\w+\s*:", body, re.M))
        if n > cfg["max_struct_fields"]:
            add(line_of(code, m.start()), "SRP",
                f"struct '{m.group(1)}' con {n} campi (> {cfg['max_struct_fields']}): "
                "valuta il raggruppamento in tipi coesi.")

    # OCP: match sullo stesso enum in piu' siti (accumulato su tutto lo scope)
    for m in re.finditer(r"\bmatch\b[^{]*\{", code):
        brace = m.end() - 1
        body = code[brace:block_end(code, brace)]
        enums = {a.group(1) for a in _MATCH_ARM_ENUM.finditer(body)}
        for e in enums:
            if e in ("Some", "None", "Ok", "Err", "Self", "Ordering", "Option", "Result"):
                continue
            enum_sites.setdefault(e, []).append((path, line_of(code, m.start())))

    for i, line in enumerate(code.splitlines(), 1):
        if _MATCH_STR.search(line):
            add(i, "OCP", "match su stringa (`.as_str()`): dispatch per tag testuale, valuta "
                          "un enum chiuso o un registry di handler.")
        if _DOWNCAST.search(line):
            add(i, "OCP/LSP", "downcast/TypeId: dispatch sul tipo concreto, valuta un metodo "
                              "del trait.")
        if _UNIMPL.search(line):
            add(i, "LSP", "unimplemented!/todo! in codice di produzione: l'implementazione "
                          "non e' sostituibile (Liskov) o il trait e' troppo largo (ISP).")
        if _STATIC_MUT.search(line):
            add(i, "DIP/Arch", "stato globale mutabile (static mut / lazy_static / static con "
                               "Mutex): passa lo stato via composition root.")
        if _BLOCKING.search(line):
            add(i, "Arch", "chiamata bloccante (sleep/block_on/std::fs) in codice async: "
                           "tokio::time, tokio::fs o spawn_blocking.")
        if _CONCRETE_INFRA.search(line) and not _ADAPTER_PATH.search(norm):
            add(i, "DIP", "dipendenza concreta da infrastruttura (pool/client) fuori da "
                          "adapter e composition root: dipendi dal trait-porta.")


# --------------------------------------------------------------------------
# TypeScript / React
# --------------------------------------------------------------------------

_TS_SWITCH = re.compile(r"\bswitch\s*\(\s*[\w.?]*\b(type|kind|role|variant|status|mode)\b")
_TS_TYPE_IF = re.compile(r"\.(type|kind)\s*===?\s*$|\.(type|kind)\s*===?")
_TS_ROLE = re.compile(r"\brole\s*===?|===?\s*[\w.]*\brole\b|\broles\.includes\s*\(")
_TS_INSTANCEOF = re.compile(r"\binstanceof\s+[A-Z]\w*")
_TS_FETCH = re.compile(r"\bfetch\s*\(|\baxios(\.\w+)?\s*\(|\bnew\s+XMLHttpRequest\b")
_TS_HOOK = re.compile(r"\buse(State|Reducer|Effect|LayoutEffect)\s*[(<]")
_TS_IFACE = re.compile(r"\b(?:interface\s+(\w+)[^{]*|type\s+(\w+)\s*=\s*)\{")
_TS_COMPONENT = re.compile(
    r"(?:export\s+(?:default\s+)?)?(?:function\s+([A-Z]\w*)\s*\(|"
    r"const\s+([A-Z]\w*)\s*(?::[^=]+)?=\s*(?:\([^)]*\)|\w+)\s*=>)")
_TS_API_LAYER = re.compile(r"/(api|services|lib/http|client)/|api-client", re.I)


def analyze_ts(path, text, cfg, add):
    code = strip_code(text, "ts")
    norm = path.replace("\\", "/")
    lines = code.splitlines()

    if path.lower().endswith(".tsx") and len(lines) > cfg["max_component_lines"]:
        add(1, "SRP", f"file componente ~{len(lines)} righe (> {cfg['max_component_lines']}): "
                      "valuta sotto-componenti o hook dedicati.")

    for m in _TS_COMPONENT.finditer(code):
        name = m.group(1) or m.group(2)
        brace = code.find("{", m.end())
        if brace == -1 or code[m.end():brace].count("\n") > 3:
            continue
        body = code[brace:block_end(code, brace)]
        hooks = len(_TS_HOOK.findall(body))
        if hooks > cfg["max_hooks"]:
            add(line_of(code, m.start()), "SRP",
                f"componente '{name}' con {hooks} useState/useEffect/useReducer "
                f"(> {cfg['max_hooks']}): stato e side effect di piu' responsabilita', "
                "estrai hook dedicati.")

    for m in _TS_IFACE.finditer(code):
        name = m.group(1) or m.group(2)
        brace = m.end() - 1
        body = code[brace + 1:block_end(code, brace)]
        n = len(re.findall(r"^\s*(?:readonly\s+)?[\w$]+\??\s*[:(]", body, re.M))
        if n > cfg["max_ts_members"]:
            add(line_of(code, m.start()), "ISP",
                f"interface/type '{name}' con ~{n} membri (> {cfg['max_ts_members']}): "
                "props o contratto troppo larghi, valuta la segregazione.")

    for i, line in enumerate(lines, 1):
        if _TS_SWITCH.search(line):
            add(i, "OCP", "switch sul tipo/ruolo/stato: valuta una mappa (registry) "
                          "tipo -> componente/handler.")
        elif _TS_TYPE_IF.search(line):
            add(i, "OCP", "ramificazione su `.type`/`.kind`: se si ripete, valuta un registry.")
        if _TS_ROLE.search(line):
            add(i, "OCP", "controllo di ruolo inline: le capacita' per ruolo vanno in una "
                          "mappa di permessi unica (e l'autorizzazione vera resta lato server).")
        if _TS_INSTANCEOF.search(line):
            add(i, "OCP/LSP", "instanceof su classe concreta: possibile dispatch sul tipo.")
        if _TS_FETCH.search(line) and not _TS_API_LAYER.search(norm):
            add(i, "DIP", "chiamata HTTP diretta fuori dal client API: passa dal client "
                          "generato/hook di query (TanStack Query).")


# --------------------------------------------------------------------------
# Main
# --------------------------------------------------------------------------

def main():
    ap = argparse.ArgumentParser(prog="solid_scan.py")
    ap.add_argument("path", nargs="+")
    for k, v in DEFAULTS.items():
        ap.add_argument("--" + k.replace("_", "-"), type=int, default=v)
    ap.add_argument("--json", action="store_true")
    args = ap.parse_args()
    cfg = {k: getattr(args, k) for k in DEFAULTS}

    missing = [p for p in args.path if not os.path.exists(p)]
    excluded = []
    files = find_files([p for p in args.path if os.path.exists(p)], excluded)
    skipped = []
    findings = []
    enum_sites = {}
    analyzed = 0

    for f in files:
        try:
            with open(f, encoding="utf-8", errors="replace") as fh:
                text = fh.read()
        except OSError:
            continue
        if is_test_path(f) or is_generated(f, text):
            skipped.append(f)
            continue
        analyzed += 1

        def add(line, principle, msg, _f=f.replace("\\", "/")):
            findings.append(dict(file=_f, line=line, principle=principle, msg=msg))

        if lang_of(f) == "rust":
            analyze_rust(f, text, cfg, add, enum_sites)
        else:
            analyze_ts(f, text, cfg, add)

    for enum, sites in sorted(enum_sites.items()):
        if len(sites) >= cfg["min_enum_match_sites"]:
            where = ", ".join(f"{os.path.basename(p)}:{l}" for p, l in sites[:6])
            for p, l in sites:
                findings.append(dict(
                    file=p.replace("\\", "/"), line=l, principle="OCP",
                    msg=f"match su '{enum}' ripetuto in {len(sites)} siti ({where}): se le "
                        "varianti crescono, un trait (strategy) o un registry evita di "
                        "toccare ogni match. Un match singolo su un enum chiuso e' idiomatico."))

    if analyzed == 0:
        msg = ("NON ESEGUITO [solid-scope-empty] nessun file .rs/.ts/.tsx di produzione "
               "nello scope" + (f"; percorsi inesistenti: {missing}" if missing else "") +
               ". Uno scope vuoto non e' un verde.")
        if args.json:
            print(json.dumps({"error": "solid-scope-empty", "message": msg,
                              "status": "not-run"}, indent=2, ensure_ascii=False))
        else:
            print(msg, file=sys.stderr)
        return EXIT_NOT_RUN

    decl = [f"File analizzati: {analyzed}; esclusi come test/generati: {len(skipped)}"]
    decl += [f"[ESCLUSA] {d} (marcatore {FIXTURE_MARKER})" for d in excluded]
    decl += [f"[INESISTENTE] {p}" for p in missing]

    if args.json:
        for d in decl:
            print(d, file=sys.stderr)
        print(json.dumps(findings, indent=2, ensure_ascii=False))
        return EXIT_FINDINGS if findings else EXIT_OK

    for d in decl:
        print(d)
    if not findings:
        print("OK - nessun candidato rilevato.")
        return EXIT_OK
    by = {}
    for x in findings:
        by.setdefault(x["principle"], []).append(x)
    print(f"\n== SOLID scan: {len(findings)} candidati ==\n")
    for pr in sorted(by):
        print(f"### {pr} ({len(by[pr])})")
        for x in by[pr][:200]:
            print(f"  {x['file']}:{x['line']} - {x['msg']}")
        print()
    print("NB: candidati euristici. Applica la review per principio del SKILL.md prima di concludere.")
    return EXIT_FINDINGS


if __name__ == "__main__":
    sys.exit(main())
