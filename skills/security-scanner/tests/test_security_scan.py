"""Test dello scanner di sicurezza (piano 01.6-01: D-01, D-05, D-06).

Solo libreria standard (`unittest`, `subprocess`, `tempfile`). Ogni test scrive
i file del caso in una cartella temporanea, lancia lo scanner come processo
(`--json --fail-on HIGH`) e confronta l'insieme delle coppie (`rule`, `line`)
dei finding con l'atteso, ristretto alla famiglia di regole del test.

Il confronto e' ristretto per famiglia per decisione del coordinatore (dubbio 1
del test design di 01.6-01): `archive-decompress` (MEDIUM) scatta su ogni
`<Nome>Decoder::new(`, e il confronto sull'insieme completo renderebbe
impossibili RED e GREEN. L'helper `scan_case` senza famiglia restituisce
l'insieme completo (lo riusa 01.6-02).

Un exit diverso da 0 e 1, o un JSON non valido, e' un `self.fail`: un rosso e'
sempre un FAIL, mai un ERROR.

I campioni Rust stanno in stringhe Python, che le regole Rust non leggono.
Nessun campione ha la forma di un segreto; se un test futuro ne avesse
bisogno, va composto a runtime per concatenazione (hard rule 5 del piano).

Lancio, dalla radice del repo:
    sh scripts/py.sh -m unittest discover -s skills/security-scanner/tests -p 'test_security_scan.py'

**Modifica:** Claude (test-write 01.6-01) · 2026-10-26 · test RED dello scanner per D-01, D-05, D-06
"""
import json
import os
import subprocess
import sys
import tempfile
import textwrap
import unittest

SCANNER = os.path.normpath(
    os.path.join(os.path.dirname(os.path.abspath(__file__)), os.pardir, "security_scan.py"))

# Famiglie di regole: il confronto di ogni test guarda solo la propria.
D01 = frozenset({"upload-in-memory", "upload-in-memory-unverified",
                 "upload-in-memory-capacity-unverified", "unbounded-copy-to-vec"})
D05 = frozenset({"no-leak-in-log", "no-leak-telemetry", "pii-in-log", "log-secret"})
D06 = frozenset({"endpoint-without-authz-decision"})
ARCHIVE = frozenset({"archive-decompress"})

UIM = "upload-in-memory"
UNV = "upload-in-memory-unverified"
CAP = "upload-in-memory-capacity-unverified"
COPY = "unbounded-copy-to-vec"
LEAK = "no-leak-in-log"
PII = "pii-in-log"
LOGSEC = "log-secret"
AUTHZ = "endpoint-without-authz-decision"


def src(text):
    """Toglie l'indentazione comune e la prima riga vuota del letterale."""
    return textwrap.dedent(text).lstrip("\n")


def line_of(code, needle, nth=1):
    """Numero di riga (da 1) della nth-esima riga di `code` che contiene `needle`."""
    seen = 0
    for i, line in enumerate(code.splitlines(), 1):
        if needle in line:
            seen += 1
            if seen == nth:
                return i
    raise AssertionError(f"riga con {needle!r} (occorrenza {nth}) assente dal caso")


class ScannerCase(unittest.TestCase):
    """Base condivisa: lancia lo scanner su un caso e restituisce i finding."""

    def run_scanner(self, files):
        """Scrive `files` ({nome: testo}) in una cartella temporanea e restituisce
        l'elenco dei finding del JSON dello scanner."""
        with tempfile.TemporaryDirectory() as tmp:
            for name, text in files.items():
                path = os.path.join(tmp, name)
                os.makedirs(os.path.dirname(path), exist_ok=True)
                with open(path, "w", encoding="utf-8", newline="\n") as fh:
                    fh.write(text)
            if not os.path.isfile(SCANNER):
                self.fail(f"scanner non trovato: {SCANNER}")
            proc = subprocess.run(
                [sys.executable, SCANNER, "--json", "--fail-on", "HIGH", tmp],
                capture_output=True, text=True, encoding="utf-8", errors="replace")
        if proc.returncode not in (0, 1):
            self.fail(f"exit dello scanner {proc.returncode} (atteso 0 o 1); "
                      f"stderr: {proc.stderr[-500:]!r}")
        try:
            data = json.loads(proc.stdout)
        except ValueError as exc:
            self.fail(f"JSON dello scanner non valido: {exc}; stdout: {proc.stdout[:500]!r}")
        if not isinstance(data, dict) or not isinstance(data.get("findings"), list):
            self.fail(f"JSON dello scanner senza elenco `findings`: {proc.stdout[:500]!r}")
        return data["findings"]

    def scan_list(self, files, family=None):
        """Elenco ordinato delle coppie (rule, line), ristretto a `family` se data."""
        out = []
        for f in self.run_scanner(files):
            if family is None or f.get("rule") in family:
                out.append((f.get("rule"), f.get("line")))
        return sorted(out, key=lambda x: (str(x[0]), x[1] if isinstance(x[1], int) else -1))

    def scan_case(self, files, family=None):
        """Insieme delle coppie (rule, line), ristretto a `family` se data;
        senza famiglia, l'insieme completo."""
        return set(self.scan_list(files, family))

    def assertFindings(self, files, family, expected):
        got = self.scan_case(files, family)
        self.assertEqual(got, set(expected),
                         f"finding della famiglia {sorted(family) if family else 'tutte'}: "
                         f"osservati {sorted(got)}, attesi {sorted(set(expected))}")

    def assertRust(self, code, family, expected, name="case.rs"):
        self.assertFindings({name: code}, family, expected)


# ==========================================================================
# D-01: letture limitate sull'output del decoder, io::copy verso Vec,
# capacita' del buffer presa dall'input
# ==========================================================================


class D01ReadToEnd(ScannerCase):

    # ---- rossi attesi sulla base ----

    def test_read_to_end_take_after_decoder_is_allowed(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let _ = ZlibDecoder::new(src).take(N).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, set())

    def test_read_to_end_bare_reader_with_take_is_allowed(self):
        code = src("""
            fn f(r: impl Read) {
                let mut v = Vec::new();
                let _ = r.take(N).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, set())

    def test_read_to_end_on_reader_built_elsewhere_is_unverified(self):
        code = src("""
            fn f(mut limited: impl Read) {
                let mut v = Vec::new();
                let _ = limited.read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "read_to_end("))})

    def test_read_to_end_multiline_statement_is_unverified(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let _ = ZlibDecoder::new(src)
                    .take(N)
                    .read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, ".read_to_end("))})

    def test_read_to_end_in_match_scrutinee_is_flagged(self):
        code = src("""
            fn f(mut r: impl Read) {
                let mut v = Vec::new();
                match r.read_to_end(&mut v) {
                    Ok(_) => {}
                    Err(_) => {}
                }
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "read_to_end("))})

    def test_read_to_end_in_closure_is_flagged(self):
        code = src("""
            fn f(o: Option<Box<dyn Read>>) {
                let mut v = Vec::new();
                let x = o.and_then(|mut d| d.read_to_end(&mut v));
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "read_to_end("))})

    def test_read_to_end_paren_mut_take_is_allowed(self):
        code = src("""
            fn f(mut stdout: impl Read) {
                let mut v = Vec::new();
                let _ = (&mut stdout).take(TSV_CAP + 1).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, set())

    def test_prescan_flate_form_is_allowed(self):
        code = src("""
            fn image_prefix(cur: &[u8], meter: &mut Meter) -> Result<Vec<u8>, PrefixError> {
                let cap = IMAGE_HEADER_SCAN_BYTES as u64 + 1;
                let mut out = Vec::new();
                let read = ZlibDecoder::new(cur).take(cap).read_to_end(&mut out);
                let n = read.map_err(|_| PrefixError::Prefilter("prefilter"))?;
                meter.add_work(n as u64).map_err(PrefixError::Work)?;
                Ok(out)
            }
            """)
        self.assertRust(code, D01, set())

    def test_async_read_to_end_take_is_allowed(self):
        code = src("""
            async fn f(r: impl AsyncRead + Unpin) -> std::io::Result<()> {
                let mut v = Vec::new();
                r.take(N).read_to_end(&mut v).await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, set())

    def test_async_read_to_end_ufcs_is_unverified(self):
        code = src("""
            async fn f(mut r: impl AsyncRead + Unpin) -> std::io::Result<()> {
                let mut v = Vec::new();
                tokio::io::AsyncReadExt::read_to_end(&mut r, &mut v).await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "read_to_end("))})

    # ---- verdi sulla base: comportamento che non deve cambiare ----

    def test_read_to_end_take_inside_decoder_is_high(self):
        code = src("""
            fn f(src: impl Read) {
                let mut v = Vec::new();
                let _ = ZlibDecoder::new(src.take(N)).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_read_to_end_decoder_without_take_is_high(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let _ = ZlibDecoder::new(src).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_async_read_to_end_decoder_without_take_is_high(self):
        code = src("""
            async fn f(r: impl AsyncBufRead + Unpin) -> std::io::Result<()> {
                let mut v = Vec::new();
                GzipDecoder::new(r).read_to_end(&mut v).await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_option_take_then_read_to_end_is_high(self):
        code = src("""
            fn f(mut child: Child) {
                let mut v = Vec::new();
                let _ = child.stdout.take().unwrap().read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_read_to_end_multiline_decoder_without_take_is_high(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let r = ZlibDecoder::new(src)
                    .read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, ".read_to_end("))})

    def test_nested_decoder_take_inside_outer_is_high(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let _ = GzDecoder::new(ZlibDecoder::new(src).take(N)).read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_take_without_args_after_decoder_is_high(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                let _ = ZlibDecoder::new(src).take().read_to_end(&mut v);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_read_to_end_in_if_condition_is_flagged(self):
        code = src("""
            fn f(src: &[u8]) {
                let mut v = Vec::new();
                if ZlibDecoder::new(src).read_to_end(&mut v).is_ok() {
                    return;
                }
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_read_to_end_file_open_without_take_is_high(self):
        code = src("""
            fn f(p: &Path) -> std::io::Result<()> {
                let mut v = Vec::new();
                File::open(p)?.read_to_end(&mut v)?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_to_end("))})

    def test_bytes_await_still_high(self):
        code = src("""
            async fn f(resp: Response) -> Result<(), Error> {
                let b = resp.bytes().await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, ".bytes()"))})

    def test_to_bytes_usize_max_still_high(self):
        code = src("""
            async fn f(body: Body) -> Result<(), Error> {
                let b = to_bytes(body, usize::MAX).await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "to_bytes("))})

    def test_archive_decompress_still_fires_on_decoder(self):
        # Pin (TEST-1): `archive-decompress` resta dove scatta oggi.
        code = "ZlibDecoder::new(src).take(N).read_to_end(&mut v);\n"
        self.assertRust(code, ARCHIVE, {("archive-decompress", 1)})


class D01ReadUntil(ScannerCase):

    # ---- rossi attesi sulla base ----

    def test_read_until_on_reader_built_elsewhere_is_unverified(self):
        code = src("""
            fn f(mut reader: impl BufRead) {
                let mut line = Vec::new();
                let _ = reader.read_until(b'\\n', &mut line);
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "read_until("))})

    def test_read_until_bufreader_without_take_is_high(self):
        code = src("""
            fn f(f: File) {
                let mut line = Vec::new();
                let _ = BufReader::new(f).read_until(b'\\n', &mut line);
            }
            """)
        self.assertRust(code, D01, {(UIM, line_of(code, "read_until("))})

    # ---- verdi sulla base ----

    def test_read_until_bufreader_over_take_is_allowed(self):
        code = src("""
            fn f(f: File) {
                let mut line = Vec::new();
                let _ = BufReader::new(f.take(N)).read_until(b'\\n', &mut line);
            }
            """)
        self.assertRust(code, D01, set())


class D01IoCopy(ScannerCase):

    # ---- rossi attesi sulla base ----

    def test_io_copy_to_unrecognized_destination_is_unverified(self):
        code = src("""
            fn f(src: &[u8], out: &mut Vec<u8>) -> std::io::Result<()> {
                let mut v = Vec::<u8>::new();
                std::io::copy(&mut ZlibDecoder::new(src), &mut v)?;
                let mut c = Cursor::new(Vec::new());
                std::io::copy(&mut GzDecoder::new(src), &mut c)?;
                std::io::copy(&mut ZlibDecoder::new(src), out)?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {
            (UNV, line_of(code, "io::copy(", 1)),
            (UNV, line_of(code, "io::copy(", 2)),
            (UNV, line_of(code, "io::copy(", 3)),
        })

    def test_tokio_io_copy_decoder_to_vec_is_high(self):
        code = src("""
            async fn f(r: impl AsyncBufRead + Unpin) -> std::io::Result<()> {
                let mut v = Vec::new();
                tokio::io::copy(&mut GzipDecoder::new(r), &mut v).await?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(COPY, line_of(code, "io::copy("))})

    def test_io_copy_decoder_without_take_to_vec_is_high(self):
        code = src("""
            fn f(src: &[u8]) -> std::io::Result<()> {
                let mut v = Vec::new();
                std::io::copy(&mut ZlibDecoder::new(src), &mut v)?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(COPY, line_of(code, "io::copy("))})

    def test_io_copy_take_inside_decoder_to_vec_is_high(self):
        code = src("""
            fn f(src: impl Read) -> std::io::Result<()> {
                let mut v = Vec::new();
                std::io::copy(&mut ZlibDecoder::new(src.take(N)), &mut v)?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, {(COPY, line_of(code, "io::copy("))})

    def test_io_copy_reader_built_elsewhere_to_vec_is_unverified(self):
        code = src("""
            fn f(mut limited: impl Read) -> std::io::Result<Vec<u8>> {
                let mut out = Vec::new();
                std::io::copy(&mut limited, &mut out)?;
                Ok(out)
            }
            """)
        self.assertRust(code, D01, {(UNV, line_of(code, "io::copy("))})

    # ---- verdi sulla base ----

    def test_io_copy_take_after_decoder_to_vec_is_allowed(self):
        code = src("""
            fn f(src: &[u8]) -> std::io::Result<()> {
                let mut v = Vec::new();
                std::io::copy(&mut ZlibDecoder::new(src).take(N), &mut v)?;
                Ok(())
            }
            """)
        self.assertRust(code, D01, set())

    def test_io_copy_bounded_reader_to_vec_is_allowed(self):
        # Forma di spikes/s02-parser/src/bin/parse-parent.rs:251.
        code = src("""
            fn read_bounded(r: impl Read) -> anyhow::Result<Vec<u8>> {
                let mut buf = Vec::new();
                std::io::copy(&mut r.take(MAX_JOB_BYTES as u64 + 1), &mut buf)?;
                Ok(buf)
            }
            """)
        self.assertRust(code, D01, set())

    def test_io_copy_to_sink_is_not_flagged(self):
        # Forma del test di spikes/s02-parser/src/prescan.rs (inflated_len).
        code = src("""
            fn inflated_len(data: &[u8]) -> u64 {
                std::io::copy(
                    &mut flate2::read::ZlibDecoder::new(data),
                    &mut std::io::sink(),
                )
                .unwrap_or_else(|e| panic!("fixture: object stream non decodificabile: {e}"))
            }
            """)
        self.assertRust(code, D01, set())

    def test_io_copy_trailing_combinator_on_next_line_is_allowed(self):
        # Forma di spikes/s02-parser/src/bin/parse-child.rs:156.
        code = src("""
            fn f(entry: impl Read) -> Result<Vec<u8>, String> {
                let mut xml = Vec::new();
                std::io::copy(&mut entry.take(CAP), &mut xml)
                    .map_err(|e| e.to_string())?;
                Ok(xml)
            }
            """)
        self.assertRust(code, D01, set())


class D01WithCapacity(ScannerCase):

    # ---- rossi attesi sulla base ----

    def test_with_capacity_turbofish_is_unverified(self):
        code = src("""
            fn f(len: usize) {
                let v: Vec<u8> = Vec::<u8>::with_capacity(len);
            }
            """)
        self.assertRust(code, D01, {(CAP, line_of(code, "with_capacity("))})

    def test_with_capacity_from_input_is_unverified(self):
        code = src("""
            fn f(hdr: &Header, n: usize, input: &str) {
                let v = Vec::with_capacity(hdr.len as usize);
                let b = BytesMut::with_capacity(n);
                let s = String::with_capacity(input.len());
                let a: Vec<u8> = Vec::with_capacity(4096);
                let c: Vec<u8> = Vec::with_capacity(MAX_LEN);
                let d = String::with_capacity(64);
            }
            """)
        self.assertRust(code, D01, {
            (CAP, line_of(code, "Vec::with_capacity(hdr.len")),
            (CAP, line_of(code, "BytesMut::with_capacity(n)")),
            (CAP, line_of(code, "String::with_capacity(input.len())")),
        })

    # ---- verdi sulla base ----

    def test_with_capacity_constant_expression_is_clean(self):
        code = src("""
            fn f() {
                let a: Vec<u8> = Vec::with_capacity(4 * MIB);
                let b: Vec<u8> = Vec::with_capacity(MAX_LEN + 1);
            }
            """)
        self.assertRust(code, D01, set())


# ==========================================================================
# D-05: macro di log spezzate da rustfmt su piu' righe
# ==========================================================================


class D05MultilineLog(ScannerCase):

    def assertNoDuplicates(self, code, family):
        got = self.scan_list({"case.rs": code}, family)
        self.assertEqual(len(got), len(set(got)), f"finding duplicati: {got}")

    # ---- rossi attesi sulla base ----

    def test_multiline_log_macro_with_chunk_is_flagged(self):
        code = src("""
            tracing::info!(
                "testo {}",
                chunk.text
            );
            """)
        self.assertRust(code, D05, {(LEAK, 1)})
        self.assertNoDuplicates(code, D05)

    def test_multiline_println_with_token_is_log_secret(self):
        code = src("""
            println!(
                "{}",
                token
            );
            """)
        self.assertRust(code, D05, {(LOGSEC, 1)})
        self.assertNoDuplicates(code, D05)

    def test_multiline_log_paren_inside_string_is_flagged(self):
        code = src("""
            tracing::info!(
                "a) {}",
                chunk.text
            );
            """)
        self.assertRust(code, D05, {(LEAK, 1)})
        self.assertNoDuplicates(code, D05)

    def test_multiline_log_with_email_is_pii(self):
        code = src("""
            tracing::warn!(
                "{}",
                user.email
            );
            """)
        self.assertRust(code, D05, {(PII, 1)})
        self.assertNoDuplicates(code, D05)

    def test_multiline_log_char_paren_is_flagged(self):
        code = src("""
            tracing::info!(
                "{} {}",
                ')',
                chunk.text
            );
            """)
        self.assertRust(code, D05, {(LEAK, 1)})
        self.assertNoDuplicates(code, D05)

    # ---- verdi sulla base ----

    def test_multiline_log_macro_with_only_counts_is_clean(self):
        code = src("""
            tracing::info!(
                "chunk indicizzati: {}",
                n
            );
            """)
        self.assertRust(code, D05, set())

    def test_singleline_finding_not_duplicated(self):
        code = 'info!("{}", chunk.text);\n'
        self.assertRust(code, D05, {(LEAK, 1)})
        got = self.scan_list({"case.rs": code}, D05)
        self.assertEqual(got, [(LEAK, 1)], f"atteso un solo no-leak-in-log, osservati {got}")


# ==========================================================================
# D-06: handler Axum registrati o in un modulo di handler, anche senza argomenti
# ==========================================================================


class D06Handlers(ScannerCase):

    # ---- rossi attesi sulla base ----

    def test_routed_handler_without_args_is_flagged(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/x", get(stats))
            }

            async fn stats() -> &'static str {
                "ok"
            }
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn stats"))})

    def test_multiline_route_registration_is_seen(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route(
                    "/u",
                    post(upload)
                )
            }

            async fn upload() {}
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn upload"))})

    def test_handler_module_async_fn_without_args_is_flagged(self):
        code = src("""
            pub async fn health() -> &'static str {
                "ok"
            }
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn health"))},
                        name="handlers.rs")

    def test_qualified_route_name_is_seen(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/u", post(handlers::upload))
            }

            async fn upload() {}
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn upload"))})

    def test_method_router_chain_second_handler_is_seen(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/x", get(a).post(b))
            }

            async fn a() {}

            async fn b() {}
            """)
        self.assertRust(code, D06, {
            (AUTHZ, line_of(code, "async fn a")),
            (AUTHZ, line_of(code, "async fn b")),
        })

    def test_routed_handler_with_unmotivated_waiver_is_flagged(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/x", get(stats))
            }

            // security-scanner: public-endpoint
            async fn stats() -> &'static str {
                "ok"
            }
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn stats"))})

    # ---- verdi sulla base ----

    def test_extractor_handler_without_identity_still_flagged(self):
        code = src("""
            async fn h(Json(b): Json<T>) {}
            """)
        self.assertRust(code, D06, {(AUTHZ, line_of(code, "async fn h"))})

    def test_hashmap_get_is_not_a_route(self):
        code = src("""
            fn lookup(cache: &HashMap<u32, u32>) {
                let x = cache.get(key);
            }

            async fn key() {}
            """)
        self.assertRust(code, D06, set())

    def test_routed_handler_with_motivated_waiver_is_clean(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/x", get(stats))
            }

            // security-scanner: public-endpoint - health del bench locale
            async fn stats() -> &'static str {
                "ok"
            }
            """)
        self.assertRust(code, D06, set())

    def test_routed_handler_with_identity_extractor_is_clean(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/x", get(stats))
            }

            async fn stats(user: AuthenticatedUser) -> &'static str {
                "ok"
            }
            """)
        self.assertRust(code, D06, set())

    def test_closure_route_is_not_flagged(self):
        code = src("""
            pub fn router() -> Router {
                Router::new().route("/healthz", get(|| async { "ok" }))
            }
            """)
        self.assertRust(code, D06, set())

    def test_unrouted_async_fn_without_extractors_is_clean(self):
        code = src("""
            async fn helper() {}
            """)
        self.assertRust(code, D06, set())


# ==========================================================================
# Piano 01.6-02: D-02 (segreti), D-03 (copertura), D-04 (allowlist del
# marcatore), D-07 (esito 2 senza percorsi).
#
# Hard rule 1 del piano: ogni campione di segreto (valori, prefissi, chiavi
# Garage, blocchi PEM, valori uguali al nome) e ogni nome di variabile di
# prova si compone a runtime con i generatori qui sotto; il sorgente di
# questo file non contiene alcun letterale con la forma di un segreto, ne'
# un assegnamento che le regole dei segreti (di base o del GREEN) leggano.
# Le costanti di modulo evitano percio' i termini da segreto nel nome.
#
# Confronto ristretto alla famiglia dei segreti (decisione 1 del
# coordinatore), salvo `test_text_files_get_only_secret_rules`, che guarda
# l'insieme completo. Ogni subprocess passa `cwd=` esplicito; ogni rosso e'
# un FAIL, mai un ERROR (eccezioni e JSON non valido diventano `self.fail`).
#
# **Modifica:** Claude (test-write 01.6-02) · 2026-10-27 · test RED dello scanner per D-02, D-03, D-04, D-07
# ==========================================================================

import builtins
import contextlib
import importlib.util
import io
import shutil
import string
from unittest import mock

SEGRETI = frozenset({"hardcoded-password", "hardcoded-apikey", "rust-const-secret",
                     "env-secret-literal", "connstring-inline-pwd", "private-key-block",
                     "sql-role-password", "jwt-literal", "known-key-prefix",
                     "unreadable-file", "fixture-marker-not-allowlisted"})
RCS = frozenset({"rust-const-secret"})
ENVLIT = "env-secret-literal"
RULE_HP = "hardcoded-password"
HKEY = "hardcoded-apikey"
KPFX = "known-key-prefix"
PEMRULE = "private-key-block"
UNREAD = "unreadable-file"
MARKRULE = "fixture-marker-not-allowlisted"
MARKER_NAME = ".security-scan-fixture"
ALLOWLIST_NAME = "fixture-allowlist.json"
HOSTILE_REL = "spikes/data/hostile"

_ALFABETO = string.ascii_letters + string.digits
_ESA = "0123456789abcdef"


def alnum(n):
    """n caratteri alfanumerici deterministici, mai un segnaposto."""
    return "".join(_ALFABETO[(7 * i + 3) % len(_ALFABETO)] for i in range(n))


def digits(n):
    return "".join(str((3 * i + 1) % 10) for i in range(n))


def hexs(n, upper=False):
    out = "".join(_ESA[(5 * i + 2) % 16] for i in range(n))
    return out.upper() if upper else out


def sk_key(n, ant=False):
    """Prefisso noto `sk-` (o `sk-ant-`) seguito da n caratteri, composto a runtime."""
    parts = ["sk"] + (["ant"] if ant else []) + [alnum(n)]
    return "-".join(parts)


def gk_key(n, upper=False):
    """Chiave Garage: `GK` seguito da n cifre esadecimali, composta a runtime."""
    return "".join(("G", "K", hexs(n, upper)))


def pem_header():
    return "".join(("-" * 5, " ".join(("BEGIN", "PRIVATE", "KEY")), "-" * 5))


def vname(*parts):
    """Nome di variabile di prova, costruito come stringa."""
    return "_".join(parts)


def env_line(name, value):
    return "".join((name, "=", value, "\n"))


def load_scanner_module():
    spec = importlib.util.spec_from_file_location("security_scan_01_6_02", SCANNER)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def write_tree(root, files):
    for name, text in files.items():
        path = os.path.join(root, *name.split("/"))
        os.makedirs(os.path.dirname(path), exist_ok=True)
        with open(path, "w", encoding="utf-8", newline="\n") as fh:
            fh.write(text)


def synthetic_allowlist():
    return json.dumps([{"path": HOSTILE_REL,
                        "motivo": "fixture ostili di S-02, copia di prova del test"}])


class Scanner0162(unittest.TestCase):
    """Helper dei test di 01.6-02: radice temporanea risolta, cwd esplicito."""

    def tmpdir(self):
        d = tempfile.TemporaryDirectory()
        self.addCleanup(d.cleanup)
        return os.path.realpath(d.name)

    def run_cli(self, args, cwd, scanner=SCANNER):
        if not os.path.isfile(scanner):
            self.fail(f"scanner non trovato: {scanner}")
        return subprocess.run([sys.executable, scanner] + list(args), cwd=cwd,
                              capture_output=True, text=True, encoding="utf-8",
                              errors="replace")

    def run_json(self, args, cwd, scanner=SCANNER):
        proc = self.run_cli(["--json", "--fail-on", "HIGH"] + list(args), cwd, scanner)
        try:
            data = json.loads(proc.stdout)
        except ValueError as exc:
            self.fail(f"JSON dello scanner non valido (exit {proc.returncode}): {exc}; "
                      f"stdout: {proc.stdout[:300]!r}; stderr: {proc.stderr[-300:]!r}")
        if not isinstance(data, dict):
            self.fail(f"JSON dello scanner non e' un oggetto: {proc.stdout[:300]!r}")
        return proc.returncode, data

    @staticmethod
    def rel(path, root):
        p = path.replace("\\", "/")
        if os.path.isabs(path):
            try:
                p = os.path.relpath(path, root).replace("\\", "/")
            except ValueError:
                pass
        while p.startswith("./"):
            p = p[2:]
        return p

    def findings_of(self, data, root, family=SEGRETI):
        found = data.get("findings")
        if not isinstance(found, list):
            self.fail(f"JSON senza elenco `findings`: {json.dumps(data)[:400]}")
        return {(self.rel(f.get("file", ""), root), f.get("rule"), f.get("line"))
                for f in found if family is None or f.get("rule") in family}

    def scan_tree(self, files, family=SEGRETI):
        """Scrive `files`, lancia lo scanner sulla radice (cwd = radice) e
        restituisce l'insieme (file relativo, regola, riga) della famiglia."""
        root = self.tmpdir()
        write_tree(root, files)
        rc, data = self.run_json([root], root)
        if rc not in (0, 1):
            self.fail(f"exit dello scanner {rc} (atteso 0 o 1): {json.dumps(data)[:400]}")
        return self.findings_of(data, root, family)

    def assertTree(self, files, expected, family=SEGRETI):
        got = self.scan_tree(files, family)
        self.assertEqual(got, set(expected),
                         f"osservati {sorted(got, key=str)}, attesi {sorted(set(expected), key=str)}")

    def assertOne(self, name, text, expected, family=SEGRETI):
        """Un solo file: confronto sulle coppie (regola, riga)."""
        self.assertTree({name: text}, {(name, r, ln) for (r, ln) in expected}, family)

    def copied_scanner(self, root, allowlist_text):
        """Copia lo scanner in <root>/skills/security-scanner/ (radice del repo
        della copia = <root>), con l'allowlist data accanto (None = assente)."""
        d = os.path.join(root, "skills", "security-scanner")
        os.makedirs(d, exist_ok=True)
        if not os.path.isfile(SCANNER):
            self.fail(f"scanner non trovato: {SCANNER}")
        dst = os.path.join(d, "security_scan.py")
        shutil.copyfile(SCANNER, dst)
        if allowlist_text is not None:
            with open(os.path.join(d, ALLOWLIST_NAME), "w", encoding="utf-8",
                      newline="\n") as fh:
                fh.write(allowlist_text)
        return dst


# ---- D-02: nomi, prefissi noti, valore uguale al nome ----------------------


class D02SecretNames(Scanner0162):

    # ---- rossi attesi sulla base ----

    def test_env_secret_key_name_is_flagged(self):
        self.assertOne("app.env", env_line(vname("GARAGE", "SECRET", "KEY"), alnum(24)),
                       {(ENVLIT, 1)})

    def test_env_access_key_name_is_flagged(self):
        self.assertOne("app.env", env_line(vname("GARAGE", "ACCESS", "KEY"), alnum(24)),
                       {(ENVLIT, 1)})

    def test_env_token_name_is_flagged(self):
        self.assertOne("app.env", env_line(vname("SERVICE", "TOKEN"), alnum(24)),
                       {(ENVLIT, 1)})

    def test_secret_term_before_counter_suffix_password_is_flagged(self):
        self.assertOne("app.env", env_line(vname("ADMIN", "PASSWORD", "MAX"), digits(12)),
                       {(ENVLIT, 1)})

    def test_secret_term_before_counter_suffix_token_is_flagged(self):
        self.assertOne("app.env", env_line(vname("API", "TOKEN", "TTL"), digits(12)),
                       {(ENVLIT, 1)})

    def test_counter_name_with_non_numeric_value_is_flagged(self):
        self.assertOne("app.env", env_line(vname("SVC", "MAX", "TOKENS"), alnum(16)),
                       {(ENVLIT, 1)})

    def test_counter_suffix_in_middle_is_not_counter(self):
        self.assertOne("app.env", env_line(vname("SVC", "MAX", "TOKENS", "ID"), digits(12)),
                       {(ENVLIT, 1)})

    def test_secret_term_before_counter_suffix_access_key_is_flagged(self):
        text = (env_line(vname("SVC", "ACCESS", "KEY", "MAX", "TOKENS"), digits(12))
                + env_line(vname("SVC", "SECRET", "TTL"), digits(12)))
        self.assertOne("app.env", text, {(ENVLIT, 1), (ENVLIT, 2)})

    # ---- verdi sulla base ----

    def test_numeric_token_counter_is_clean(self):
        self.assertOne("app.env", env_line(vname("TEI", "MAX", "BATCH", "TOKENS"), "16384"),
                       set())

    def test_numeric_exemption_only_for_env_rule(self):
        name = vname("api", "key")
        self.assertOne("cfg.py", f'{name} = "{digits(12)}"\n', {(HKEY, 1)})

    def test_numeric_value_under_secret_names_is_flagged(self):
        text = (env_line(vname("GARAGE", "ADMIN", "PASSWORD"), digits(12))
                + env_line(vname("APP", "SECRET"), digits(12))
                + env_line(vname("SVC", "API", "KEY"), digits(12)))
        self.assertOne("app.env", text, {(ENVLIT, 1), (ENVLIT, 2), (ENVLIT, 3)})

    def test_secret_term_beats_counter_name(self):
        self.assertOne("app.env", env_line(vname("MAX", "ADMIN", "PASSWORD"), digits(12)),
                       {(ENVLIT, 1)})

    def test_rust_const_secret_still_fires(self):
        name = vname("SVC", "TOKEN")
        code = f'const {name}: &str = "{alnum(16)}";\n'
        self.assertOne("case.rs", code, {("rust-const-secret", 1)}, family=RCS)


class D02Placeholders(Scanner0162):

    # ---- rossi attesi sulla base ----

    def test_value_equal_to_name_is_not_placeholder(self):
        word = "".join(("pass", "word"))
        self.assertOne("script.py", f'{word} = "{word}"\n', {(RULE_HP, 1)})

    def test_env_value_equal_to_name_word_is_flagged(self):
        name = vname("SERVICE", "TOKEN")
        text = env_line(name, name.split("_")[-1].lower()) + env_line(name, name)
        self.assertOne("app.env", text, {(ENVLIT, 1), (ENVLIT, 2)})

    def test_password_and_secret_no_longer_placeholders(self):
        sname = vname("APP", "SECRET")
        pword = "".join(("pass", "word"))
        pname = vname("db", pword)
        files = {
            "app.env": env_line(sname, sname.split("_")[-1].lower()),
            "script.py": f'{pname} = "{pword}"\n',
        }
        self.assertTree(files, {("app.env", ENVLIT, 1), ("script.py", RULE_HP, 1)})

    # ---- verdi sulla base ----

    def test_env_reference_to_same_name_is_placeholder(self):
        name = vname("SERVICE", "TOKEN")
        text = env_line(name, "${" + name + "}") + env_line(name, "$" + name)
        self.assertOne("app.env", text, set())

    def test_documented_placeholders_are_clean(self):
        # `example_key` e non `example-key`: la lista chiusa ha `example\w*`, e
        # `\w` non comprende il trattino (vedi dubbi del rapporto test-write).
        text = (env_line(vname("APP", "SECRET"), "${VAR}")
                + env_line(vname("DB", "PASSWORD"), "<valore>")
                + env_line(vname("SVC", "API", "KEY"), "changeme")
                + env_line(vname("SVC", "API", "KEY"), "_".join(("example", "key")))
                + env_line(vname("DB", "PASSWORD"), "-".join(("your", "token"))))
        self.assertOne("app.env", text, set())


class D02KnownPrefixes(Scanner0162):

    # ---- rossi attesi sulla base ----

    def test_known_prefix_sk_ant_is_flagged(self):
        self.assertOne("note.md", "chiave: " + sk_key(40, ant=True) + "\n", {(KPFX, 1)})

    def test_known_prefix_garage_key_is_flagged(self):
        code = "fn f() {\n    let id = \"" + gk_key(24) + "\";\n}\n"
        self.assertOne("case.rs", code, {(KPFX, 2)})

    def test_sk_prefix_length_boundary(self):
        text = "a " + sk_key(19) + "\n" + "b " + sk_key(20) + "\n"
        self.assertOne("note.md", text, {(KPFX, 2)})

    def test_garage_key_uppercase_hex_is_flagged(self):
        code = "fn f() {\n    let id = \"" + gk_key(24, upper=True) + "\";\n}\n"
        self.assertOne("case.rs", code, {(KPFX, 2)})

    def test_known_prefix_in_comment_is_flagged(self):
        self.assertOne("case.rs", "// " + sk_key(30, ant=True) + "\n", {(KPFX, 1)})

    def test_known_prefix_in_test_file_is_flagged(self):
        code = "fn f() {\n    let k = \"" + sk_key(30, ant=True) + "\";\n}\n"
        self.assertTree({"tests/x.rs": code}, {("tests/x.rs", KPFX, 2)})

    # ---- verdi sulla base ----

    def test_sk_test_sentinel_is_clean(self):
        code = 'fn f() -> String {\n    format!("sk-test-{SENTINEL}")\n}\n'
        self.assertOne("case.rs", code, set())

    def test_sk_inside_word_is_clean(self):
        self.assertOne("note.md", "corpus: syn-05-risk-report-confidential-en\n", set())

    def test_garage_key_23_hex_is_clean(self):
        self.assertOne("note.md", "id " + gk_key(23) + "\n", set())

    def test_garage_key_25_hex_is_clean(self):
        self.assertOne("note.md", "id " + gk_key(25) + "\n", set())


# ---- D-03: file testuali, file illeggibili, potatura ridotta ---------------


class D03Coverage(Scanner0162):

    # ---- rossi attesi sulla base ----

    def test_secret_in_txt_is_flagged(self):
        self.assertOne("evidenza.txt", "valore " + sk_key(30, ant=True) + "\n", {(KPFX, 1)})

    def test_secret_in_csv_is_flagged(self):
        self.assertOne("dati.csv", "id,valore\n1," + sk_key(30, ant=True) + "\n", {(KPFX, 2)})

    def test_secret_in_jsonl_is_flagged(self):
        self.assertOne("righe.jsonl", '{"v": "' + sk_key(30, ant=True) + '"}\n', {(KPFX, 1)})

    def test_private_key_in_pem_is_flagged(self):
        text = pem_header() + "\n" + alnum(40) + "\n"
        self.assertOne("chiave.pem", text, {(PEMRULE, 1)})

    def test_secret_in_extensionless_hook_is_flagged(self):
        text = "#!/bin/sh\necho " + sk_key(30, ant=True) + "\n"
        self.assertOne("pre-commit", text, {(KPFX, 2)})

    def test_scope_with_only_csv_is_scanned(self):
        root = self.tmpdir()
        write_tree(root, {"dati.csv": "id,valore\n1,abc\n"})
        rc, data = self.run_json([root], root)
        self.assertEqual(rc, 0, f"exit {rc}, atteso 0: {json.dumps(data)[:300]}")
        self.assertEqual(data.get("scanned_files"), 1)

    def test_unreadable_file_is_blocking(self):
        root = self.tmpdir()
        target = os.path.join(root, "cartella.md")
        os.makedirs(target)
        try:
            mod = load_scanner_module()
            res = mod.scan_file(target)
        except Exception as exc:  # un rosso e' un FAIL, mai un ERROR
            self.fail(f"scan_file ha sollevato {exc!r}")
        got = [(f[0], f[1], f[3]) for f in res]
        self.assertEqual(got, [("CRITICAL", UNREAD, 0)], f"osservati {res!r}")

    def test_dist_build_coverage_are_scanned(self):
        line = env_line(vname("APP", "SECRET"), alnum(24))
        files = {"a.md": "testo\n", "dist/app.env": line, "build/app.env": line,
                 "coverage/app.env": line}
        self.assertTree(files, {("dist/app.env", ENVLIT, 1), ("build/app.env", ENVLIT, 1),
                                ("coverage/app.env", ENVLIT, 1)})

    def test_text_family_extensions_are_scanned(self):
        text = "valore " + sk_key(30, ant=True) + "\n"
        for name in ("dati.tsv", "righe.ndjson", "run.log", "chiave.key", "cert.crt",
                     ".gitignore"):
            with self.subTest(file=name):
                self.assertOne(name, text, {(KPFX, 1)})
        with self.subTest(file="doc.pdf"):
            root = self.tmpdir()
            write_tree(root, {"doc.pdf": text, "a.md": "testo\n"})
            rc, data = self.run_json([root], root)
            self.assertIn(rc, (0, 1), json.dumps(data)[:300])
            self.assertEqual(data.get("scanned_files"), 1)
            self.assertEqual({f for f in self.findings_of(data, root, None)
                              if f[0] == "doc.pdf"}, set())
        with self.subTest(file="commento.txt"):
            self.assertOne("commento.txt", "# " + sk_key(30, ant=True) + "\n", {(KPFX, 1)})

    def test_unreadable_file_blocks_cli_exit(self):
        root = self.tmpdir()
        write_tree(root, {"a.md": "testo\n", "b.md": "testo\n"})
        blocked = os.path.normcase(os.path.abspath(os.path.join(root, "b.md")))
        real_open = builtins.open

        def fake_open(file, *args, **kwargs):
            if isinstance(file, (str, bytes, os.PathLike)):
                p = os.fsdecode(os.fspath(file))
                if os.path.normcase(os.path.abspath(p)) == blocked:
                    raise PermissionError(13, "accesso negato (prova)", p)
            return real_open(file, *args, **kwargs)

        buf = io.StringIO()
        try:
            mod = load_scanner_module()
            argv = ["security_scan.py", "--json", "--fail-on", "HIGH", root]
            with mock.patch("builtins.open", fake_open), \
                    mock.patch.object(sys, "argv", argv), \
                    contextlib.redirect_stdout(buf):
                rc = mod.main()
        except BaseException as exc:  # SystemExit compreso: FAIL, mai ERROR
            self.fail(f"main() ha sollevato {exc!r}; stdout: {buf.getvalue()[:300]!r}")
        try:
            data = json.loads(buf.getvalue())
        except ValueError as exc:
            self.fail(f"JSON non valido: {exc}; stdout: {buf.getvalue()[:300]!r}")
        self.assertEqual(rc, 1, f"exit {rc}, atteso 1: {buf.getvalue()[:400]}")
        unread = [f for f in data.get("findings", []) if f.get("rule") == UNREAD]
        self.assertEqual([(f.get("severity"), f.get("line")) for f in unread],
                         [("CRITICAL", 0)])
        self.assertEqual(data.get("scanned_files"), 2)

    # ---- verdi sulla base ----

    def test_text_files_get_only_secret_rules(self):
        tls = "".join(("danger_accept_invalid", "_certs(", "true)"))
        url = "".join(('"', "http", "://", "esempio.org/a", '"'))
        files = {"dati.csv": "a," + tls + "\n" + "b," + url + "\n", "leggimi.md": "testo\n"}
        self.assertTree(files, set(), family=None)

    def test_target_and_node_modules_still_pruned(self):
        line = env_line(vname("APP", "SECRET"), alnum(24))
        files = {"a.md": "testo\n", "target/app.env": line, "node_modules/app.env": line}
        self.assertTree(files, set())


# ---- D-04: allowlist del marcatore -----------------------------------------


class D04FixtureAllowlist(Scanner0162):

    def marker(self, rel):
        return {rel + "/" + MARKER_NAME: "marcatore di prova\n"}

    @staticmethod
    def excluded_ends_with(data, suffix):
        return any(str(e).replace("\\", "/").rstrip("/").endswith(suffix)
                   for e in data.get("excluded_dirs", []))

    # ---- rossi attesi sulla base ----

    def test_marker_outside_allowlist_is_blocking(self):
        root = self.tmpdir()
        files = {"a.md": "testo\n",
                 "altro/app.env": env_line(vname("APP", "SECRET"), alnum(24))}
        files.update(self.marker("altro"))
        write_tree(root, files)
        rc, data = self.run_json([root], root)
        got = self.findings_of(data, root)
        self.assertIn(("altro/" + MARKER_NAME, MARKRULE, 0), got, f"osservati {sorted(got)}")
        self.assertIn(("altro/app.env", ENVLIT, 1), got, f"osservati {sorted(got)}")
        self.assertEqual(rc, 1)

    def test_marker_count_printed_in_summary(self):
        root = self.tmpdir()
        write_tree(root, {"a.md": "testo\n"})
        proc = self.run_cli(["--fail-on", "HIGH", root], root)
        self.assertEqual(proc.returncode, 0, proc.stdout + proc.stderr)
        self.assertIn("Escluse per marcatore (0)", proc.stdout)

    def test_allowlist_unreadable_exits_two(self):
        root = self.tmpdir()
        scanner = self.copied_scanner(root, "{ non e' JSON")
        write_tree(root, {"caso/a.md": "testo\n"})
        proc = self.run_cli(["--json", "--fail-on", "HIGH", os.path.join(root, "caso")],
                            root, scanner)
        self.assertEqual(proc.returncode, 2, proc.stdout[:300] + proc.stderr[-300:])
        self.assertIn("fixture-allowlist-unreadable", proc.stdout + proc.stderr)

    def test_allowlist_is_exact_match_not_prefix(self):
        root = self.tmpdir()
        files = {"a.md": "testo\n",
                 HOSTILE_REL + "-x/app.env": env_line(vname("APP", "SECRET"), alnum(24))}
        files.update(self.marker(HOSTILE_REL + "-x"))
        write_tree(root, files)
        rc, data = self.run_json([root], root)
        got = self.findings_of(data, root)
        self.assertIn((HOSTILE_REL + "-x/" + MARKER_NAME, MARKRULE, 0), got,
                      f"osservati {sorted(got)}")
        self.assertIn((HOSTILE_REL + "-x/app.env", ENVLIT, 1), got,
                      f"directory non scansionata: osservati {sorted(got)}")

    def test_allowlist_entry_invalid_exits_two(self):
        cases = {
            "assente": None,
            "senza_motivo": json.dumps([{"path": HOSTILE_REL}]),
            "path_con_puntini": json.dumps([{"path": "../" + HOSTILE_REL, "motivo": "m"}]),
        }
        for label, content in cases.items():
            with self.subTest(allowlist=label):
                self._assert_allowlist_rejected(content)
        with self.subTest(allowlist="path_assoluto"):
            absolute = os.path.join(self.tmpdir(), *HOSTILE_REL.split("/")).replace("\\", "/")
            self._assert_allowlist_rejected(json.dumps([{"path": absolute, "motivo": "m"}]))

    def _assert_allowlist_rejected(self, content):
        root = self.tmpdir()
        scanner = self.copied_scanner(root, content)
        write_tree(root, {"caso/a.md": "testo\n"})
        proc = self.run_cli(["--json", "--fail-on", "HIGH", os.path.join(root, "caso")],
                            root, scanner)
        self.assertEqual(proc.returncode, 2, proc.stdout[:300] + proc.stderr[-300:])
        self.assertIn("fixture-allowlist-unreadable", proc.stdout + proc.stderr)

    def test_nested_copy_of_hostile_path_is_not_allowlisted(self):
        root = self.tmpdir()
        scanner = self.copied_scanner(root, synthetic_allowlist())
        files = {"altro/a.md": "testo\n", "altro/" + HOSTILE_REL + "/b.md": "testo\n"}
        files.update(self.marker("altro/" + HOSTILE_REL))
        write_tree(root, files)
        cwd = os.path.join(root, "altro")
        rc, data = self.run_json(["."], cwd, scanner)
        rules = {f.get("rule") for f in data.get("findings", [])}
        self.assertIn(MARKRULE, rules, f"osservati {json.dumps(data)[:400]}")
        self.assertFalse(self.excluded_ends_with(data, HOSTILE_REL),
                         f"copia annidata esclusa: {data.get('excluded_dirs')}")

    def test_excluded_count_in_json(self):
        root = self.tmpdir()
        files = {"a.md": "testo\n"}
        files.update(self.marker(HOSTILE_REL))
        write_tree(root, files)
        rc, data = self.run_json([root], root)
        self.assertEqual(data.get("excluded_count"), 1, json.dumps(data)[:400])

    # ---- verdi sulla base ----

    def test_allowlisted_marker_relative_to_repo_root(self):
        root = self.tmpdir()
        scanner = self.copied_scanner(root, synthetic_allowlist())
        files = {"spikes/data/a.md": "testo\n",
                 HOSTILE_REL + "/app.env": env_line(vname("APP", "SECRET"), alnum(24))}
        files.update(self.marker(HOSTILE_REL))
        write_tree(root, files)
        cwd = os.path.join(root, "spikes")
        for radice in ("data", "."):
            with self.subTest(radice=radice):
                rc, data = self.run_json([radice], cwd, scanner)
                self.assertEqual(rc, 0, json.dumps(data)[:400])
                self.assertTrue(self.excluded_ends_with(data, "data/hostile"),
                                f"non esclusa: {data.get('excluded_dirs')}")
                self.assertEqual(data.get("findings"), [], json.dumps(data)[:400])

    def test_allowlisted_marker_is_excluded(self):
        root = self.tmpdir()
        files = {"a.md": "testo\n",
                 HOSTILE_REL + "/app.env": env_line(vname("APP", "SECRET"), alnum(24))}
        files.update(self.marker(HOSTILE_REL))
        write_tree(root, files)
        rc, data = self.run_json([root], root)
        self.assertEqual(rc, 0, json.dumps(data)[:400])
        self.assertTrue(self.excluded_ends_with(data, HOSTILE_REL),
                        f"non esclusa: {data.get('excluded_dirs')}")
        self.assertEqual(self.findings_of(data, root), set())


# ---- D-07: esito 2 senza percorsi o con percorsi inesistenti ---------------


class D07NotRun(Scanner0162):

    # ---- rossi attesi sulla base ----

    def test_no_paths_exits_two_with_json(self):
        root = self.tmpdir()
        proc = self.run_cli(["--json"], root)
        try:
            data = json.loads(proc.stdout)
        except ValueError as exc:
            self.fail(f"JSON non valido senza percorsi (exit {proc.returncode}): {exc}; "
                      f"stderr: {proc.stderr[-300:]!r}")
        self.assertEqual(proc.returncode, 2)
        self.assertEqual(data.get("status"), "not-run", proc.stdout[:300])

    def test_missing_path_mixed_with_existing_exits_two(self):
        root = self.tmpdir()
        write_tree(root, {"ok/a.md": "testo\n"})
        esistente = os.path.join(root, "ok")
        mancante = os.path.join(root, "non-esiste")
        proc = self.run_cli(["--fail-on", "HIGH", esistente, mancante], root)
        self.assertEqual(proc.returncode, 2, proc.stdout[:300] + proc.stderr[-300:])
        self.assertIn("security-missing-paths", proc.stdout + proc.stderr)
        rc, data = self.run_json([esistente, mancante], root)
        self.assertEqual(rc, 2, json.dumps(data)[:300])
        self.assertTrue(data.get("missing_paths"), json.dumps(data)[:300])

    # ---- verdi sulla base ----

    def test_missing_path_only_exits_two(self):
        root = self.tmpdir()
        proc = self.run_cli(["--fail-on", "HIGH", os.path.join(root, "non-esiste")], root)
        self.assertEqual(proc.returncode, 2, proc.stdout[:300] + proc.stderr[-300:])

    def test_empty_dir_exits_two(self):
        root = self.tmpdir()
        proc = self.run_cli(["--fail-on", "HIGH", root], root)
        self.assertEqual(proc.returncode, 2, proc.stdout[:300] + proc.stderr[-300:])



# ---- D-02, precisazioni dell'utente del 2026-10-29 ---------------------------
# (A) nei file Python `env-secret-literal` conta un valore come letterale solo se
#     fra virgolette; gli altri tipi di file non cambiano.
# (B) in tutte le regole dei segreti un valore fatto solo di un numero con
#     un'unita' di misura (durata, dimensione, percentuale) non e' un segreto.


def num_unit(unit, int_part="2", frac=None):
    """Numero decimale seguito da un'unita', composto a runtime."""
    return "".join((int_part, ".", frac if frac is not None else digits(6), unit))


def py_assign(name, expr):
    return "".join((name, " = ", expr, "\n"))


def kv_quoted(name, value):
    return "".join((name, "=", '"', value, '"', "\n"))


class D02PythonQuotedOnly(Scanner0162):

    # ---- rossi attesi sul GREEN del 2026-10-29 ----

    def test_py_tuple_value_is_not_env_literal(self):
        expr = "".join(("(", '"alfa"', ", ", '"beta"', ")"))
        self.assertOne("mod.py", py_assign(vname("SVC", "SECRET", "TERMS"), expr), set())

    def test_py_empty_list_value_is_not_env_literal(self):
        self.assertOne("mod.py", py_assign(vname("SVC", "BURNED", "SECRETS"), "[]"), set())

    def test_py_set_expression_value_is_not_env_literal(self):
        expr = " | ".join(("ALL", "{TEXT}"))
        self.assertOne("mod.py", py_assign(vname("SVC", "SECRET", "KINDS"), expr), set())

    def test_py_call_value_is_not_env_literal(self):
        expr = "".join(("re.compile(r", '"', "alfa|beta", '"', ")"))
        self.assertOne("mod.py", py_assign(vname("SVC", "SECRET", "NAME", "TERM"), expr), set())

    def test_py_dict_value_is_not_env_literal(self):
        expr = "".join(("{", '"alfa"', ", ", '"beta"', "}"))
        self.assertOne("mod.py", py_assign(vname("SVC", "SECRET", "RULES"), expr), set())

    def test_py_bare_identifier_value_is_not_env_literal(self):
        expr = vname("load", "service", "value")
        self.assertOne("mod.py", py_assign(vname("SVC", "TOKEN"), expr), set())

    def test_py_environ_call_value_is_not_env_literal(self):
        expr = "".join(("os.environ.get(", '"', vname("SVC", "API", "KEY"), '"', ")"))
        self.assertOne("mod.py", py_assign(vname("SVC", "API", "KEY"), expr), set())

    # ---- guardie: verdi anche sul GREEN (non indebolimento) ----

    def test_py_double_quoted_long_value_stays_critical(self):
        expr = "".join(('"', alnum(24), '"'))
        self.assertOne("mod.py", py_assign(vname("SVC", "API", "KEY"), expr),
                       {(ENVLIT, 1), (HKEY, 1)})

    def test_py_single_quoted_long_value_stays_env_literal(self):
        expr = "".join(("'", alnum(24), "'"))
        got = self.scan_tree({"mod.py": py_assign(vname("SVC", "TOKEN"), expr)})
        self.assertIn(("mod.py", ENVLIT, 1), got, sorted(got, key=str))

    def test_env_bare_value_stays_env_literal(self):
        self.assertOne("app.env", env_line(vname("SVC", "TOKEN"), alnum(24)), {(ENVLIT, 1)})

    def test_sh_bare_value_stays_env_literal(self):
        self.assertOne("run.sh", env_line(vname("SVC", "SECRET"), alnum(24)), {(ENVLIT, 1)})

    def test_sh_bare_expression_like_value_stays_env_literal(self):
        # (A) vale solo per Python: in uno script shell un valore nudo resta letterale.
        self.assertOne("run.sh", env_line(vname("SVC", "SECRET", "KINDS"), alnum(12)),
                       {(ENVLIT, 1)})


class D02NumberWithUnit(Scanner0162):

    # ---- rossi attesi sul GREEN del 2026-10-29 ----

    def test_txt_duration_ms_under_token_name_is_clean(self):
        text = kv_quoted(vname("tokenization", "time"), num_unit("ms"))
        self.assertOne("run.txt", text, set())

    def test_log_duration_microseconds_under_token_name_is_clean(self):
        text = kv_quoted(vname("queue", "token", "time"), num_unit("µs", "344"))
        self.assertOne("run.log", text, set())

    def test_log_line_with_several_durations_is_clean(self):
        text = "".join(("embed{", vname("total", "time"), '="', num_unit("ms", "18"), '" ',
                        vname("tokenization", "time"), '="', num_unit("ms"), '"}', "\n"))
        self.assertOne("run.log", text, set())

    def test_txt_size_mib_under_token_name_is_clean(self):
        text = kv_quoted(vname("token", "cache", "size"), num_unit("MiB", "1536", "25"))
        self.assertOne("run.txt", text, set())

    def test_txt_percentage_under_token_name_is_clean(self):
        text = kv_quoted(vname("token", "share"), num_unit("%", "12"))
        self.assertOne("run.txt", text, set())

    def test_env_duration_under_secret_name_is_clean(self):
        # (B) vale per tutte le regole dei segreti, `env-secret-literal` compresa.
        self.assertOne("app.env", env_line(vname("SVC", "TOKEN", "WAIT"), num_unit("ms", "250")),
                       set())

    # ---- guardie: verdi anche sul GREEN (non indebolimento) ----

    def test_txt_random_alnum_under_token_name_stays_flagged(self):
        val = alnum(16)
        self.assertTrue(any(c.isdigit() for c in val) and any(c.isalpha() for c in val))
        got = self.scan_tree({"run.txt": kv_quoted(vname("token", "value"), val)})
        self.assertTrue({r for (_f, r, ln) in got if ln == 1}, sorted(got, key=str))

    def test_txt_number_with_non_unit_suffix_stays_flagged(self):
        text = kv_quoted(vname("tokenization", "time"), num_unit("zq"))
        got = self.scan_tree({"run.txt": text})
        self.assertTrue({r for (_f, r, ln) in got if ln == 1}, sorted(got, key=str))

    def test_env_number_with_non_unit_suffix_stays_env_literal(self):
        self.assertOne("app.env", env_line(vname("SVC", "TOKEN", "WAIT"), num_unit("zq", "250")),
                       {(ENVLIT, 1)})

    def test_txt_unit_then_secret_tail_stays_flagged(self):
        # Il numero con unita' deve essere l'intero valore, non un prefisso.
        text = kv_quoted(vname("token", "value"), num_unit("ms") + alnum(12))
        got = self.scan_tree({"run.txt": text})
        self.assertTrue({r for (_f, r, ln) in got if ln == 1}, sorted(got, key=str))


if __name__ == "__main__":
    unittest.main()
