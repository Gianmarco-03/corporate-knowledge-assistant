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


if __name__ == "__main__":
    unittest.main()
