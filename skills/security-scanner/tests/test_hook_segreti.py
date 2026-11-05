"""Test degli hook versionati (piano 01.6-06, D-08).

Coprono i passi Rust degli spike (`cargo fmt (spikes)`, `cargo clippy (spikes)`),
l'esito `NON COPERTO` per i test degli spike, lo scope di `tocca_rust_spike`, il
commit di sola evidenza, la parita' delle regole dei segreti di D-02 fra
`filtra_righe_segrete` e lo scanner, le regole esistenti dell'hook e l'assenza
di variabili di scavalcamento.

Ogni test d'integrazione lavora su un repository sintetico in una
`tempfile.TemporaryDirectory` (hard rule 3 del piano): `git init`, copia di
`.githooks/`, `scripts/py.sh`, `skills/security-scanner/` (senza `tests/`) e
`skills/crate-boundary-check/`, piu' un workspace minimo `spikes/` con un crate
foglia senza dipendenze e il suo `Cargo.lock` generato con
`cargo generate-lockfile --offline` (TEST-10). Gli hook si invocano con
`sh .githooks/pre-commit` e `sh .githooks/pre-push` dentro il sintetico: il
repository vero e il `core.hooksPath` dell'utente non si toccano. L'ambiente dei
processi ha `GIT_CONFIG_GLOBAL` verso un file vuoto della tempdir,
`GIT_CONFIG_NOSYSTEM=1`, `CARGO_TARGET_DIR` dentro la tempdir e nessuna altra
variabile `GIT_*` ereditata.

I campioni segreti si compongono a runtime per concatenazione e non si stampano
mai: i messaggi d'errore riportano solo l'indice della riga (hard rule 2).

Un rosso e' sempre un FAIL, mai un ERROR (TEST-11): gli esiti dei processi si
confrontano con asserzioni, e uno strumento mancante (git, cargo, sh) e' un
`self.fail`, non un salto.

Lancio, dalla radice del repo (hard rule 5):
    sh scripts/py.sh -m unittest discover -s skills/security-scanner/tests -p 'test_hook_*.py' -v

**Modifica:** Claude (test-write 01.6-06) · 2026-10-29 · test RED degli hook per gli spike, NON COPERTO, commit di sola evidenza e parita' dei segreti (D-08)
"""
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
import unittest

HERE = os.path.dirname(os.path.abspath(__file__))
SKILL_DIR = os.path.normpath(os.path.join(HERE, os.pardir))
REPO = os.path.normpath(os.path.join(SKILL_DIR, os.pardir, os.pardir))
HOOK_FILES = ("controlli-comuni.sh", "pre-commit", "pre-push")

SH = shutil.which("sh")
GIT = shutil.which("git")
CARGO = shutil.which("cargo")

# Regole dei segreti dello scanner (stesso insieme di `_SECRET_RULES`).
SECRET_RULES = frozenset({"hardcoded-password", "hardcoded-apikey", "rust-const-secret",
                          "env-secret-literal", "connstring-inline-pwd", "private-key-block",
                          "sql-role-password", "jwt-literal", "known-key-prefix"})

FMT = "cargo fmt (spikes)"
CLIPPY = "cargo clippy (spikes)"
TEST_SPIKE = "cargo test (spikes)"
MOTIVO_NA = "nessun file Rust degli spike nello scope"

LIB_OK = "pub fn uno() -> u32 {\n    1\n}\n"
LIB_OK_2 = "pub fn uno() -> u32 {\n    1\n}\n\npub fn due() -> u32 {\n    2\n}\n"
LIB_BAD_FMT = "pub fn uno() -> u32 {\n    1\n}\n\npub   fn due()->u32{2}\n"
LIB_LINT = "pub fn uno() -> u32 {\n    return 1;\n}\n"


def has_line(output, prefix):
    """Vero se una riga dell'uscita, tolti gli spazi iniziali, inizia con `prefix`."""
    return any(r.lstrip().startswith(prefix) for r in output.splitlines())


def tools_or_fail(test, *names):
    tools = {"sh": SH, "git": GIT, "cargo": CARGO}
    for n in names:
        if not tools[n]:
            test.fail(f"strumento assente sull'host: {n} (un test dell'hook non salta in silenzio)")


def base_env(tmp):
    env = {k: v for k, v in os.environ.items() if not k.upper().startswith("GIT_")}
    vuoto = os.path.join(tmp, "gitconfig-vuoto")
    if not os.path.exists(vuoto):
        open(vuoto, "w").close()
    env["GIT_CONFIG_GLOBAL"] = vuoto
    env["GIT_CONFIG_NOSYSTEM"] = "1"
    env["CARGO_TARGET_DIR"] = os.path.join(tmp, "cargo-target")
    return env


class RepoSintetico:
    """Repository sintetico della hard rule 3, in una TemporaryDirectory."""

    def __init__(self, test):
        self.test = test
        self._td = tempfile.TemporaryDirectory(prefix="cka-hook-")
        self.tmp = self._td.name
        self.root = os.path.join(self.tmp, "repo")
        self.remote = os.path.join(self.tmp, "remoto.git")
        self.env = base_env(self.tmp)
        os.makedirs(self.root)
        self._build()

    def cleanup(self):
        self._td.cleanup()

    # --- processi -----------------------------------------------------------
    def run(self, args, stdin=None, env=None, cwd=None):
        p = subprocess.run(args, cwd=cwd or self.root, env=env or self.env, input=stdin,
                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT)
        return p.returncode, p.stdout.decode("utf-8", errors="replace")

    def git(self, *args):
        rc, out = self.run([GIT, *args])
        if rc != 0:
            self.test.fail(f"git {args[0]} nel sintetico uscito {rc}")
        return out

    def write(self, rel, text):
        path = os.path.join(self.root, *rel.split("/"))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)

    def commit(self, msg):
        self.git("add", "-A")
        self.git("commit", "-q", "-m", msg)
        return self.git("rev-parse", "HEAD").strip()

    # --- costruzione --------------------------------------------------------
    def _copy(self, rel, ignore=None):
        src = os.path.join(REPO, *rel.split("/"))
        dst = os.path.join(self.root, *rel.split("/"))
        if os.path.isdir(src):
            shutil.copytree(src, dst, ignore=ignore)
        else:
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil.copyfile(src, dst)

    def _build(self):
        self.git("init", "-q", "-b", "main")
        self.git("config", "user.name", "test-hook")
        self.git("config", "user.email", "test-hook@example.invalid")
        self.git("config", "core.autocrlf", "false")
        self._copy(".githooks")
        self._copy("scripts/py.sh")
        self._copy("skills/security-scanner",
                   ignore=shutil.ignore_patterns("tests", "__pycache__", "*.pyc"))
        self._copy("skills/crate-boundary-check",
                   ignore=shutil.ignore_patterns("tests", "__pycache__", "*.pyc"))
        self.write("README.md", "# sintetico\n")
        self.write("spikes/Cargo.toml",
                   '[workspace]\nresolver = "2"\nmembers = ["foglia"]\n')
        self.write("spikes/foglia/Cargo.toml",
                   '[package]\nname = "foglia"\nversion = "0.0.0"\nedition = "2021"\npublish = false\n')
        self.write("spikes/foglia/src/lib.rs", LIB_OK)
        if CARGO:
            rc, _ = self.run([CARGO, "generate-lockfile", "--offline",
                              "--manifest-path", "spikes/Cargo.toml"])
            if rc != 0:
                self.test.fail("cargo generate-lockfile --offline fallito nel sintetico")
        self.base = self.commit("base")

    # --- hook ---------------------------------------------------------------
    def pre_commit(self, env=None):
        return self.run([SH, ".githooks/pre-commit"], env=env)

    def push_base(self):
        rc, _ = self.run([GIT, "init", "-q", "--bare", self.remote], cwd=self.tmp)
        if rc != 0:
            self.test.fail("git init --bare del remoto fallito")
        self.git("remote", "add", "origin", self.remote)
        self.git("push", "-q", "origin", "main")
        return self.git("rev-parse", "refs/remotes/origin/main").strip()

    def pre_push(self, remote_sha, local_sha=None):
        local_sha = local_sha or self.git("rev-parse", "HEAD").strip()
        stdin = f"refs/heads/main {local_sha} refs/heads/main {remote_sha}\n".encode()
        return self.run([SH, ".githooks/pre-push", "origin", self.remote], stdin=stdin)

    def source(self, func, stdin_text):
        return self.run([SH, "-c", f". ./.githooks/controlli-comuni.sh; {func}"],
                        stdin=stdin_text.encode("utf-8"))


class HookTestCase(unittest.TestCase):
    def setUp(self):
        tools_or_fail(self, "sh", "git", "cargo")
        self.repo = RepoSintetico(self)
        self.addCleanup(self.repo.cleanup)


# =============================================================================
# Passi degli spike nel pre-commit (D-08, TEST-4)
# =============================================================================
class TestPreCommitSpike(HookTestCase):

    def test_precommit_runs_spike_fmt_on_rs_change(self):
        r = self.repo
        fake_dir = os.path.join(r.tmp, "finto-bin")
        os.makedirs(fake_dir)
        argv_log = os.path.join(r.tmp, "cargo-argv.log").replace("\\", "/")
        real = CARGO.replace("\\", "/")
        with open(os.path.join(fake_dir, "cargo"), "w", encoding="utf-8", newline="\n") as fh:
            fh.write("#!/bin/sh\n"
                     f"printf '%s\\n' \"$*\" >> '{argv_log}'\n"
                     f"exec '{real}' \"$@\"\n")
        os.chmod(os.path.join(fake_dir, "cargo"), 0o755)
        env = dict(r.env)
        env["PATH"] = fake_dir + os.pathsep + env.get("PATH", "")
        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.git("add", "-A")
        rc, out = r.pre_commit(env=env)
        self.assertTrue(has_line(out, f"PASS — {FMT}"), "manca 'PASS — cargo fmt (spikes)'")
        self.assertEqual(rc, 0, "pre-commit con un .rs ben formattato deve uscire 0")
        try:
            with open(argv_log, encoding="utf-8") as fh:
                calls = fh.read().splitlines()
        except OSError:
            calls = []
        self.assertTrue(any("fmt" in c and "--manifest-path spikes/Cargo.toml" in c for c in calls),
                        "cargo fmt non invocato con --manifest-path spikes/Cargo.toml")

    def test_precommit_fails_spike_fmt_on_bad_format(self):
        r = self.repo
        r.write("spikes/foglia/src/lib.rs", LIB_BAD_FMT)
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, f"FAIL — {FMT}"), "manca 'FAIL — cargo fmt (spikes)'")
        self.assertEqual(rc, 1)

    def test_precommit_declares_spike_tests_not_covered(self):
        r = self.repo
        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, f"NON COPERTO — {TEST_SPIKE}"),
                        "manca 'NON COPERTO — cargo test (spikes)' nel pre-commit")
        self.assertIn("spikes/tools/session.sh", out,
                      "il NON COPERTO deve nominare il runner (spikes/tools/session.sh)")

    def test_precommit_spike_fmt_not_applicable_without_rs(self):
        r = self.repo
        r.write("README.md", "# sintetico\n\nriga nuova\n")
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, f"NON APPLICABILE — {FMT} ({MOTIVO_NA})"),
                        "manca 'NON APPLICABILE — cargo fmt (spikes) (nessun file Rust degli spike nello scope)'")
        self.assertFalse(has_line(out, f"NON COPERTO — {TEST_SPIKE}"),
                         "senza codice degli spike nello scope non si dichiara NON COPERTO")
        self.assertEqual(rc, 0)

    def test_spike_fmt_fails_when_manifest_missing(self):
        r = self.repo
        r.git("rm", "-q", "spikes/Cargo.toml")
        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, f"FAIL — {FMT}"),
                        "con spikes/Cargo.toml assente il passo deve dare FAIL, non NON APPLICABILE")
        self.assertEqual(rc, 1)

    def test_spike_fmt_fails_when_cargo_missing(self):
        r = self.repo
        dirs = []
        for exe in (SH, GIT, sys.executable):
            d = os.path.dirname(exe)
            if d not in dirs:
                dirs.append(d)
        path = os.pathsep.join(dirs)
        if shutil.which("cargo", path=path):
            self.fail("il PATH ristretto contiene ancora cargo: il caso non si puo' provare")
        env = dict(r.env)
        env["PATH"] = path
        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.git("add", "-A")
        rc, out = r.pre_commit(env=env)
        self.assertTrue(has_line(out, f"FAIL — {FMT}"),
                        "con cargo assente dal PATH il passo deve dare FAIL")
        self.assertEqual(rc, 1)

    def test_non_coperto_does_not_block(self):
        r = self.repo
        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, f"NON COPERTO — {TEST_SPIKE}"),
                        "manca 'NON COPERTO — cargo test (spikes)'")
        self.assertEqual(rc, 0, "un NON COPERTO non blocca")


# =============================================================================
# Passi degli spike nel pre-push (D-08, TEST-5)
# =============================================================================
class TestPrePushSpike(HookTestCase):

    def test_prepush_runs_spike_clippy(self):
        r = self.repo
        remote_sha = r.push_base()
        r.write("spikes/foglia/src/lib.rs", LIB_LINT)
        r.commit("lint")
        rc, out = r.pre_push(remote_sha)
        self.assertTrue(has_line(out, f"FAIL — {CLIPPY}"),
                        "con clippy::needless_return manca 'FAIL — cargo clippy (spikes)'")
        self.assertEqual(rc, 1)

        r.write("spikes/foglia/src/lib.rs", LIB_OK_2)
        r.commit("senza lint")
        rc, out = r.pre_push(remote_sha)
        self.assertTrue(has_line(out, f"PASS — {CLIPPY}"), "manca 'PASS — cargo clippy (spikes)'")
        self.assertTrue(has_line(out, f"PASS — {FMT}"), "manca 'PASS — cargo fmt (spikes)'")
        self.assertTrue(has_line(out, f"NON COPERTO — {TEST_SPIKE}"),
                        "manca 'NON COPERTO — cargo test (spikes)' nel pre-push")
        self.assertEqual(rc, 0)

    def test_prepush_without_spike_rust_is_not_applicable(self):
        r = self.repo
        remote_sha = r.push_base()
        r.write("README.md", "# sintetico\n\nsolo documentazione\n")
        r.commit("solo md")
        rc, out = r.pre_push(remote_sha)
        self.assertTrue(has_line(out, f"NON APPLICABILE — {CLIPPY}"),
                        "manca 'NON APPLICABILE — cargo clippy (spikes)'")
        self.assertTrue(has_line(out, f"NON APPLICABILE — {FMT}"),
                        "manca 'NON APPLICABILE — cargo fmt (spikes)'")
        self.assertFalse(has_line(out, f"NON COPERTO — {TEST_SPIKE}"))
        self.assertEqual(rc, 0)

    # /verify 1 (2026-11-05): con uno sha remoto assente in locale (force push
    # senza fetch) git log falliva dentro la pipe e i passi davano PASS o
    # NON APPLICABILE su zero righe.
    def test_prepush_unknown_remote_sha_still_checks_secrets(self):
        r = self.repo
        r.push_base()
        r.write("config.txt", "GARAGE_SECRET" + "_KEY=abcd1234realsecret\n")
        r.commit("segreto")
        rc, out = r.pre_push("1" * 40)
        self.assertTrue(has_line(out, "FAIL — segreti"),
                        "con sha remoto ignoto il segreto in partenza deve dare 'FAIL — segreti'")
        self.assertEqual(rc, 1)

    def test_prepush_unknown_remote_sha_still_checks_spike_rust(self):
        r = self.repo
        r.push_base()
        r.write("spikes/foglia/src/lib.rs", LIB_LINT)
        r.commit("lint")
        rc, out = r.pre_push("1" * 40)
        self.assertTrue(has_line(out, f"FAIL — {CLIPPY}"),
                        "con sha remoto ignoto clippy degli spike non può risultare NON APPLICABILE")
        self.assertEqual(rc, 1)


# =============================================================================
# Scope degli spike (TEST-6, dubbio 1)
# =============================================================================
class TestToccaRustSpike(HookTestCase):

    def test_tocca_rust_spike_paths(self):
        r = self.repo
        casi = [
            ("spikes/s02-parser/src/bin/ocr/main.rs\n", 0),
            ("spikes/Cargo.toml\n", 0),
            ("spikes/s02-parser/Cargo.toml\n", 0),
            ("spikes/Cargo.lock\n", 0),
            ("README.md\nspikes/s04-jev/src/lib.rs\n", 0),
            ("foo/spikes/x.rs\n", 1),
            ("spikes/README.md\n", 1),
            ("Cargo.toml\n", 1),
            ("", 1),
        ]
        ko = []
        for i, (stdin, atteso) in enumerate(casi):
            rc, _ = r.source("tocca_rust_spike", stdin)
            if rc != atteso:
                ko.append((i, rc))
        self.assertEqual(ko, [], f"tocca_rust_spike diverso dall'atteso (caso, rc): {ko}")


# =============================================================================
# Commit di sola evidenza (D-08, D-03, TEST-9)
# =============================================================================
def _pre16_sha():
    """Il padre del primo commit di 01.6 che modifica security_scan.py, o None."""
    try:
        out = subprocess.run([GIT, "-C", REPO, "log", "--format=%H %s", "--diff-filter=M", "--",
                              "skills/security-scanner/security_scan.py"],
                             stdout=subprocess.PIPE, stderr=subprocess.DEVNULL).stdout.decode(
            "utf-8", errors="replace")
    except (OSError, TypeError):
        return None
    di_16 = [ln.split(" ", 1)[0] for ln in out.splitlines() if "(01.6-" in ln]
    if not di_16:
        return None
    p = subprocess.run([GIT, "-C", REPO, "rev-parse", di_16[-1] + "^"],
                       stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    sha = p.stdout.decode().strip()
    return sha if p.returncode == 0 and re.fullmatch(r"[0-9a-f]{40}", sha) else None


class TestEvidenceOnly(HookTestCase):

    def test_precommit_accepts_evidence_only_commit(self):
        r = self.repo
        contenuti = {"csv": "a,b\n1,2\n", "txt": "esito: ok\n", "json": '{"esito": "ok"}\n'}
        for ext, testo in contenuti.items():
            with self.subTest(ext=ext):
                r.write(f"spikes/s02-parser/results/x.{ext}", testo)
                r.git("add", "-A")
                rc, out = r.pre_commit()
                self.assertTrue(has_line(out, "PASS — security-scanner"),
                                f"commit di solo .{ext} sotto results/: manca 'PASS — security-scanner'")
                self.assertEqual(rc, 0, f"commit di solo .{ext} sotto results/ deve passare")
                r.commit(f"evidenza {ext}")

    def test_evidence_only_commit_rejected_with_pre16_scanner(self):
        pre16 = _pre16_sha()
        if pre16 is None:
            self.skipTest("PRE16 non risolvibile dalla storia del repo: RED di base solo nell'evidenza")
        r = self.repo
        p = subprocess.run([GIT, "-C", REPO, "show", f"{pre16}:skills/security-scanner/security_scan.py"],
                           stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
        if p.returncode != 0:
            self.fail("git show dello scanner di PRE16 fallito")
        with open(os.path.join(r.root, "skills", "security-scanner", "security_scan.py"), "wb") as fh:
            fh.write(p.stdout)
        r.commit("scanner pre-1.6")
        r.write("spikes/s02-parser/results/x.csv", "a,b\n1,2\n")
        r.git("add", "-A")
        rc, out = r.pre_commit()
        self.assertTrue(has_line(out, "FAIL — security-scanner (rc=2)"),
                        "con lo scanner pre-1.6 il commit di solo .csv deve dare rc=2")
        self.assertIn("security-scope-empty", out)
        self.assertEqual(rc, 1)


# =============================================================================
# Segreti: parita' con lo scanner sulle regole di D-02 (TEST-1, TEST-2)
# =============================================================================
def _v(n):
    """Valore alfanumerico deterministico di `n` caratteri (non un segnaposto)."""
    return (("q7Wm" + "Zp3K" + "x9Lr") * (n // 12 + 1))[:n]


def _hex(n):
    return (("a1b2" + "c3d4" + "e5f6") * (n // 12 + 1))[:n]


def parity_rows():
    """Righe (testo, atteso) composte a runtime. atteso True = segnalata."""
    S = "SEC" + "RET"
    s = S.lower()
    P = "PASS" + "WORD"
    AK = "API" + "_KEY"
    ACK = "ACC" + "ESS_KEY"
    T = "TO" + "KEN"
    GK = "G" + "K"
    SK = "s" + "k-"
    SKA = SK + "an" + "t-"
    v12 = _v(12)
    return [
        # gruppi di nome (env-secret-literal)
        (f"APP_{S}={v12}", True),                       # 0
        (f"DB_{P}={v12}", True),                        # 1
        (f"OPENAI_{AK}={v12}", True),                   # 2
        (f"STORAGE_{ACK}={v12}", True),                 # 3
        (f"SERVICE_{T}={v12}", True),                   # 4
        # prefissi noti (known-key-prefix)
        (f"chiave {SKA}{_v(24)}", True),                # 5
        (f"chiave {SK}{_v(20)}", True),                 # 6
        (f"chiave {SK}{_v(19)}", False),                # 7
        (f"chiave x{SK}{_v(24)}", False),               # 8  confine a sinistra
        (f"id {GK}{_hex(24)}", True),                   # 9
        (f"id {GK}{_hex(23)}", False),                  # 10
        (f"id {GK}{_hex(25)}", False),                  # 11
        (f"id x{GK}{_hex(24)}", False),                 # 12 confine a sinistra
        # valori numerici e contatori
        (f"MAX_{T}S=" + "40" + "96", False),            # 13 contatore, sole cifre
        (f"ADMIN_{P}_MAX=" + "1" + "2", True),          # 14 termine da segreto vince
        (f"API_{T}_TTL=" + "36" + "00", True),          # 15 termine da segreto vince
        (f"MAX_{T}S={v12}", True),                      # 16 contatore, valore alfanumerico
        (f"SESSION_{T}_TTL=" + "36" + "00s", False),    # 17 numero con unita'
        # /verify 1, 2026-11-05: l'unita' esenta solo sotto `token` senza termini forti
        (f"APP_{S}_WAIT=" + "25" + "0ms", True),
        (f'db_{P.lower()} = "' + "123456" + 'd"', True),
        (f'svc_{AK.lower()} = "' + "98765432" + 'b"', True),
        (f'{T.lower()}_{s} = "' + "98765432" + 'b"', True),
        (f'{T.lower()}ization_time="' + "2.5" + 'ms"', False),
        # segnaposto e valore uguale al nome
        (f"APP_{S}=" + "change" + "-me", False),        # 18
        (f"APP_{S}=" + "${" + "VAULT_REF}", False),     # 19 ${VAR}
        (f"APP_{S}=" + "${" + f"APP_{S}" + "}", False), # 20 ${NOME} con lo stesso nome
        (f"APP_{S}=" + "<" + "valore>", False),         # 21 <...>
        (f"APP_{S}=APP_{S}", True),                     # 22 valore uguale al nome
        (f"TEST_{T}=" + "te" + "st", True),             # 23 parola del nome come valore
        # dettagli della regex (TEST-2)
        (f"MY_{T}=" + "q", True),                       # 24 1 carattere
        (f"MY_{T}={_v(7)}", True),                      # 25 7 caratteri
        (f"export APP_{S}={v12}", True),                # 26
        (f"- APP_{S}={v12}", True),                     # 27
        (f'APP_{S}="{v12}"', True),                     # 28 virgolette doppie
        (f"APP_{S}={v12} # nota", True),                # 29 commento in coda
        (f"# APP_{S}={v12}", False),                    # 30 riga commento, senza virgolette
        (f'app_{s} = "{v12}"', True),                   # 31 minuscolo fra virgolette
        (f"app_{s}={v12}", False),                      # 32 minuscolo senza virgolette
    ]


class TestSecretParity(HookTestCase):

    def test_secret_parity_with_scanner(self):
        r = self.repo
        rows = parity_rows()
        self.assertGreaterEqual(len(rows), 16)
        texts = [t for t, _ in rows]
        self.assertEqual(len(set(texts)), len(texts), "righe della tabella non distinte")
        for t in texts:
            self.assertLess(len(t) + 1, 200)

        # Hook: righe `+...` a filtra_righe_segrete, l'uscita sono le righe segnalate.
        diff = "".join("+" + t + "\n" for t in texts)
        rc, out = r.source("filtra_righe_segrete", diff)
        hook_hits = {ln[1:] for ln in out.splitlines() if ln.startswith("+")}
        hook = [t in hook_hits for t in texts]

        # Scanner: le stesse righe in un .txt, finding dei segreti per numero di riga.
        txt = os.path.join(r.tmp, "parita.txt")
        with open(txt, "w", encoding="utf-8", newline="\n") as fh:
            fh.write("".join(t + "\n" for t in texts))
        p = subprocess.run([sys.executable, os.path.join(SKILL_DIR, "security_scan.py"),
                            "--json", "--fail-on", "HIGH", txt],
                           stdout=subprocess.PIPE, stderr=subprocess.PIPE, env=r.env)
        if p.returncode not in (0, 1):
            self.fail(f"scanner uscito {p.returncode} sulla tabella di parita'")
        try:
            data = json.loads(p.stdout.decode("utf-8"))
        except ValueError:
            self.fail("scanner: JSON non valido sulla tabella di parita'")
        lines = {f["line"] for f in data["findings"] if f["rule"] in SECRET_RULES}
        scanner = [(i + 1) in lines for i in range(len(texts))]

        attesi = [a for _, a in rows]
        hook_ko = [i for i in range(len(rows)) if hook[i] != attesi[i]]
        scan_ko = [i for i in range(len(rows)) if scanner[i] != attesi[i]]
        self.assertEqual(scan_ko, [], f"scanner diverso dall'atteso sulle righe {scan_ko}")
        self.assertEqual(hook_ko, [], f"hook diverso dall'atteso sulle righe {hook_ko}")


# =============================================================================
# Regole esistenti dell'hook (TEST-3)
# =============================================================================
class TestExistingRules(HookTestCase):

    def test_hook_keeps_existing_rules(self):
        r = self.repo
        dash = "-" * 5
        pg = "post" + "gres" + "://"
        rows = [
            (dash + "BEGIN RSA " + "PRIV" + "ATE KEY" + dash, True),              # 0 PEM
            ("TYPESAFE_" + "API_KEY=" + "change" + "me", True),                    # 1
            ("JWT_" + "SEC" + "RET=" + "redac" + "ted", True),                     # 2
            ("DATABASE_URL=" + pg + "app:" + _v(12) + "@db:5432/x", True),        # 3
            ("chiave " + "s" + "k-" + _v(20), True),                               # 4
            ("id " + "AK" + "IA" + _v(16).upper(), True),                          # 5
            ("tok " + "g" + "h" + "p" + "_" + _v(36), True),                       # 6
            ("DATABASE_URL=" + pg + "app:" + "change" + "-me" + "@db:5432/x", False),  # 7
        ]
        texts = [t for t, _ in rows]
        rc, out = r.source("filtra_righe_segrete", "".join("+" + t + "\n" for t in texts))
        hits = {ln[1:] for ln in out.splitlines() if ln.startswith("+")}
        ko = [i for i, (t, a) in enumerate(rows) if (t in hits) != a]
        self.assertEqual(ko, [], f"regole esistenti dell'hook diverse dall'atteso sulle righe {ko}")


# =============================================================================
# Nessuna via d'uscita (TEST-7)
# =============================================================================
# Letture ammesse di variabili non assegnate negli hook, con il motivo.
AMMESSE = {
    "CKA_PYTHON": "sceglie l'interprete in scripts/py.sh, non se eseguire i controlli",
    "IFS": "separatore della shell, salvato e ripristinato",
    "NF": "numero di campi di awk, non una variabile d'ambiente",
    "NR": "numero di record di awk, non una variabile d'ambiente",
}
# Funzioni in cui un `case` sui percorsi e' ammesso, con il motivo.
CASE_AMMESSI = {
    "tocca_rust_spike": "scope degli spike (D-08)",
    "e_nome_segreto": "nomi da segreto: un elenco che blocca, non che esenta",
}

_LETTURA = re.compile(r"\$\{?([A-Za-z_][A-Za-z0-9_]*)")
_ASSEGNA = re.compile(r"(?:^|[;&|(]|\bthen\b|\bdo\b|\belse\b)\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)=")
_READ = re.compile(r"\bread\s+((?:-\w+\s+)*)([A-Za-z_][A-Za-z0-9_ ]*)")
_FOR = re.compile(r"\bfor\s+([A-Za-z_][A-Za-z0-9_]*)\s+in\b")
_DEFAULT = re.compile(r"\$\{([A-Za-z_][A-Za-z0-9_]*):?[-=?]")
_FUNZ = re.compile(r"^([A-Za-z_][A-Za-z0-9_]*)\(\)\s*\{")


def _codice(text):
    return [ln for ln in text.splitlines() if not ln.lstrip().startswith("#")]


class TestNoBypass(unittest.TestCase):

    def test_no_bypass_variables(self):
        testi = {}
        for f in HOOK_FILES:
            with open(os.path.join(REPO, ".githooks", f), encoding="utf-8") as fh:
                testi[f] = _codice(fh.read())
        righe = [ln for lines in testi.values() for ln in lines]
        assegnate = set()
        for ln in righe:
            assegnate.update(_ASSEGNA.findall(ln))
            assegnate.update(_FOR.findall(ln))
            for m in _READ.finditer(ln):
                assegnate.update(m.group(2).split())
        lette = set()
        for ln in righe:
            lette.update(_LETTURA.findall(ln))
        estranee = sorted(lette - assegnate - set(AMMESSE))
        self.assertEqual(estranee, [], f"variabili lette e mai assegnate negli hook: {estranee}")

        default = sorted({n for ln in righe for n in _DEFAULT.findall(ln)} - assegnate)
        self.assertEqual(default, [], f"${{...:-...}} su variabili non assegnate: {default}")

        fuori = []
        for f, lines in testi.items():
            funz = None
            for i, ln in enumerate(lines):
                m = _FUNZ.match(ln)
                if m:
                    funz = m.group(1)
                elif ln.startswith("}"):
                    funz = None
                if re.search(r"(?:^|[;\s])case\s", ln) and funz not in CASE_AMMESSI:
                    fuori.append(f"{f}:{funz or '<radice>'}")
        self.assertEqual(fuori, [], f"rami case fuori dalle funzioni ammesse: {fuori}")


if __name__ == "__main__":
    unittest.main()
