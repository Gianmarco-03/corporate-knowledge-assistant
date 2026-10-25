//! CLI di misura dello spike S-03/A-03, parte indice (piano 01-14): `synth`, `threshold`,
//! `hnsw`, `hybrid`, `leak`, `ingest-under-load`. Usa la libreria del piano 01-10
//! (`s03_pgvector::db`, `s03_pgvector::metrics`) senza modificarla.
//!
//! STUB test-write 01-14: firme minime perché i test rossi del design compilino; corpo,
//! CLI e sottocomandi li scrive l'esecutore del piano (Task 1). Togliere anche
//! `allow(dead_code)` quando la CLI usa tutte le funzioni.
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
#![allow(dead_code)]

mod hybrid;
mod leak;
mod synth;

use uuid::Uuid;

fn main() -> anyhow::Result<()> {
    todo!("index-bench: CLI del piano 01-14 (Task 1)")
}

/// Recall filtrata dell'indice approssimato rispetto alla ricerca esatta: quota degli id
/// esatti presenti nei risultati approssimati. Esatto vuoto: valore documentato, mai NaN.
pub fn recall_vs_exact(exact: &[Uuid], approx: &[Uuid]) -> f64 {
    let _ = (exact, approx);
    todo!("recall_vs_exact")
}

/// Quota di query che restituiscono esattamente `k` risultati.
pub fn returns_k_rate(result_lens: &[usize], k: usize) -> f64 {
    let _ = (result_lens, k);
    todo!("returns_k_rate")
}

/// Supporto dei test d'integrazione `#[ignore]` (richiedono `DATABASE_URL` verso il Postgres
/// del compose). Lancio dal runner, dentro una sessione:
/// `cargo test -p s03-pgvector --bin index-bench -- --ignored --test-threads=1 <nome>`.
/// Ogni test lavora in uno schema temporaneo `test_<pid>_<uuid>` con le tabelle di
/// `sql/schema.sql` (piano 01-10) e, dove serve, di `sql/index.sql`, e lo elimina alla fine.
#[cfg(test)]
pub(crate) mod testdb {

    use std::path::{Path, PathBuf};

    use s03_pgvector::metrics::{self, Visibility};
    use sqlx::postgres::PgPoolOptions;
    use sqlx::{AssertSqlSafe, Executor, PgPool};
    use uuid::Uuid;

    pub const DIM: usize = 1024;

    pub fn manifest_dir() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    }

    pub fn acl_json() -> String {
        let p = manifest_dir().join("../data/corpus/acl.json");
        std::fs::read_to_string(&p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }

    /// Matrice attesa per un insieme di documenti `(doc_key, classification)`, da acl.json.
    pub fn visibility_for(docs: &[(&str, &str)]) -> Visibility {
        let manifest: Vec<serde_json::Value> = docs
            .iter()
            .map(|(k, c)| serde_json::json!({"doc_id": k, "classification": c}))
            .collect();
        metrics::expected_visibility(&acl_json(), &serde_json::to_string(&manifest).unwrap())
            .expect("matrice attesa da acl.json")
    }

    pub struct Scratch {
        pub pool: PgPool,
        admin: PgPool,
        schema: String,
    }

    impl Scratch {
        pub async fn new() -> Self {
            let url = std::env::var("DATABASE_URL").expect("DATABASE_URL richiesta dai test db");
            let admin = PgPoolOptions::new()
                .max_connections(1)
                .connect(&url)
                .await
                .expect("connessione");
            sqlx::raw_sql("CREATE EXTENSION IF NOT EXISTS vector SCHEMA public")
                .execute(&admin)
                .await
                .expect("estensione vector");
            // Nome generato dal test (pid + uuid), nessun valore in ingresso: DDL non parametrizzabile.
            let schema = format!("test_{}_{}", std::process::id(), Uuid::new_v4().simple());
            sqlx::raw_sql(AssertSqlSafe(format!("CREATE SCHEMA {schema}")))
                .execute(&admin)
                .await
                .expect("schema temporaneo");
            let sp: std::sync::Arc<str> = format!("SET search_path TO {schema}, public").into();
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .after_connect(move |conn, _meta| {
                    let sp = sp.clone();
                    Box::pin(async move {
                        conn.execute(sqlx::raw_sql(AssertSqlSafe(sp))).await?;
                        Ok(())
                    })
                })
                .connect(&url)
                .await
                .expect("pool con search_path");
            let ddl = std::fs::read_to_string(manifest_dir().join("sql/schema.sql"))
                .expect("sql/schema.sql del piano 01-10");
            sqlx::raw_sql(AssertSqlSafe(ddl))
                .execute(&pool)
                .await
                .expect("schema.sql");
            Scratch {
                pool,
                admin,
                schema,
            }
        }

        /// Applica `sql/index.sql` istruzione per istruzione: `CREATE INDEX CONCURRENTLY` non
        /// può stare in un blocco transazionale né in una query multi-istruzione.
        pub async fn apply_index_sql(&self) {
            let p = manifest_dir().join("sql/index.sql");
            let ddl = std::fs::read_to_string(&p)
                .unwrap_or_else(|e| panic!("sql/index.sql del piano 01-14 ({}): {e}", p.display()));
            for stmt in split_sql(&ddl) {
                sqlx::raw_sql(AssertSqlSafe(stmt.clone()))
                    .execute(&self.pool)
                    .await
                    .unwrap_or_else(|e| panic!("index.sql: {stmt}: {e}"));
            }
        }

        pub async fn drop(self) {
            self.pool.close().await;
            let _ = sqlx::raw_sql(AssertSqlSafe(format!(
                "DROP SCHEMA {} CASCADE",
                self.schema
            )))
            .execute(&self.admin)
            .await;
        }

        /// Documento con le righe di `doc_acl` derivate da acl.json per la sua classificazione.
        pub async fn doc(&self, doc_key: &str, classification: &str) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query("INSERT INTO doc (id, doc_key, classification) VALUES ($1, $2, $3)")
                .bind(id)
                .bind(doc_key)
                .bind(classification)
                .execute(&self.pool)
                .await
                .expect("insert doc");
            let vis = visibility_for(&[(doc_key, classification)]);
            for (role, docs) in &vis {
                if docs.contains(doc_key) {
                    sqlx::query("INSERT INTO doc_acl (doc_id, role) VALUES ($1, $2)")
                        .bind(id)
                        .bind(role)
                        .execute(&self.pool)
                        .await
                        .expect("insert doc_acl");
                }
            }
            id
        }

        /// Chunk nella tabella del modello scelto (bge-m3, `RISULTATI-embedding.md` §Esito).
        pub async fn chunk(&self, doc_id: Uuid, body: &str, emb: Vec<f32>) -> Uuid {
            let id = Uuid::new_v4();
            sqlx::query(
                "INSERT INTO chunk_bgem3 (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
                 VALUES ($1, $2, 1, NULL, 0, 4, 'italian'::regconfig, $3, $4)",
            )
            .bind(id)
            .bind(doc_id)
            .bind(body)
            .bind(pgvector::Vector::from(emb))
            .execute(&self.pool)
            .await
            .expect("insert chunk");
            id
        }

        /// Molti chunk dello stesso documento in una sola istruzione.
        pub async fn chunks_bulk(&self, doc_id: Uuid, body: &str, embs: Vec<Vec<f32>>) {
            let n = embs.len();
            let ids: Vec<Uuid> = (0..n).map(|_| Uuid::new_v4()).collect();
            let docs = vec![doc_id; n];
            let bodies = vec![body.to_string(); n];
            let vecs: Vec<pgvector::Vector> =
                embs.into_iter().map(pgvector::Vector::from).collect();
            sqlx::query(
                "INSERT INTO chunk_bgem3 (id, doc_id, page, section_path, char_start, char_end, lang, body, emb) \
                 SELECT u.id, u.doc_id, 1, NULL, 0, 4, 'italian'::regconfig, u.body, u.emb \
                 FROM UNNEST($1::uuid[], $2::uuid[], $3::text[], $4::vector[]) AS u(id, doc_id, body, emb)",
            )
            .bind(ids)
            .bind(docs)
            .bind(bodies)
            .bind(vecs)
            .execute(&self.pool)
            .await
            .expect("insert chunk a lotti");
        }
    }

    /// Istruzioni SQL separate da `;` a fine riga, commenti `--` di riga esclusi.
    pub fn split_sql(ddl: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut cur = String::new();
        for line in ddl.lines() {
            let t = line.trim();
            if t.starts_with("--") || t.is_empty() {
                continue;
            }
            cur.push_str(line);
            cur.push('\n');
            if t.ends_with(';') {
                out.push(std::mem::take(&mut cur));
            }
        }
        if !cur.trim().is_empty() {
            out.push(cur);
        }
        out
    }

    /// Vettore unitario lungo l'asse 0 (il vettore di query).
    pub fn query_vec() -> Vec<f32> {
        axis(0)
    }

    pub fn axis(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[i] = 1.0;
        v
    }

    /// Vettore lontano dalla query, distanza coseno crescente con `i`.
    pub fn far(i: usize) -> Vec<f32> {
        let mut v = vec![0.0; DIM];
        v[1] = 1.0;
        v[0] = -(i as f32) * 0.01;
        v
    }

    /// Vettore vicinissimo alla query, distinto per `i` (rumore deterministico).
    pub fn near(i: usize) -> Vec<f32> {
        let mut v = query_vec();
        v[2 + (i % (DIM - 2))] = 0.001 * (1 + i / (DIM - 2)) as f32;
        v
    }

    pub fn roles(r: &[&str]) -> Vec<String> {
        r.iter().map(|s| s.to_string()).collect()
    }

    pub fn union(a: Visibility, b: Visibility) -> Visibility {
        let mut out = a;
        for (role, docs) in b {
            out.entry(role).or_default().extend(docs);
        }
        out
    }

    pub fn repo_root() -> PathBuf {
        manifest_dir().join("../..")
    }

    pub fn read(p: &Path) -> String {
        std::fs::read_to_string(p).unwrap_or_else(|e| panic!("{}: {e}", p.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::testdb::{manifest_dir, read, repo_root};
    use super::*;
    use std::collections::{BTreeMap, BTreeSet};
    use std::process::Command;

    fn id(n: u128) -> Uuid {
        Uuid::from_u128(n)
    }

    // TEST-13 (RISK-7): recall filtrata contro l'esatto e tasso di k risultati, valori noti.
    #[test]
    fn test13_recall_vs_exact_known_values() {
        let (a, b, c, d, x, y) = (id(1), id(2), id(3), id(4), id(90), id(91));
        assert!((recall_vs_exact(&[a, b, c, d], &[a, c, x, y]) - 0.5).abs() < 1e-12);
        assert_eq!(recall_vs_exact(&[a, b, c, d], &[]), 0.0);
        let empty = recall_vs_exact(&[], &[a]);
        assert!(
            empty.is_finite() && (0.0..=1.0).contains(&empty),
            "esatto vuoto: valore documentato in [0, 1], mai NaN (got {empty})"
        );
        assert!((returns_k_rate(&[10, 10, 7, 10], 10) - 0.75).abs() < 1e-12);
        assert_eq!(returns_k_rate(&[10, 10], 10), 1.0);
    }

    /// Codice dei file di `src/bin/index-bench/`, senza il modulo di test.
    fn bench_sources() -> Vec<(String, String)> {
        let dir = manifest_dir().join("src/bin/index-bench");
        let mut out = Vec::new();
        for e in std::fs::read_dir(&dir).expect("src/bin/index-bench") {
            let p = e.unwrap().path();
            if p.extension().is_some_and(|x| x == "rs") {
                let src = read(&p);
                let code = src.split("#[cfg(test)]").next().unwrap().to_owned();
                out.push((p.file_name().unwrap().to_string_lossy().into_owned(), code));
            }
        }
        out
    }

    // TEST-9 (RISK-4): nessun SQL composto con format!/concat!, parametri con bind.
    #[test]
    fn test9_sql_only_bound_no_composed_strings() {
        // Parole chiave SQL in maiuscolo, come le scrivono le query del crate.
        const KW: [&str; 8] = [
            "SELECT ", "INSERT ", "UPDATE ", "DELETE ", "WITH ", "CREATE ", "SET ", " FROM ",
        ];
        let mut binds = 0;
        for (f, code) in bench_sources() {
            assert!(
                !code.contains("concat!"),
                "{f}: concat! vietato (tabelle da enum chiuso)"
            );
            for (n, line) in code.lines().enumerate() {
                let t = line.trim_start();
                if t.starts_with("//") {
                    continue;
                }
                if t.contains("format!") {
                    assert!(
                        !KW.iter().any(|k| t.contains(k)),
                        "{f}:{}: SQL composto con format!: {t}",
                        n + 1
                    );
                }
            }
            binds += code.matches(".bind(").count();
        }
        assert!(
            binds > 0,
            "le query di index-bench legano i parametri con bind"
        );
    }

    // TEST-9 (RISK-4): lo scanner di sicurezza esce 0 su src/bin e sql.
    #[test]
    #[ignore = "richiede python e lo scanner del repo (skills/security-scanner)"]
    fn test9_security_scanner_clean() {
        let root = repo_root();
        let st = Command::new("python")
            .arg(root.join("skills/security-scanner/security_scan.py"))
            .arg(manifest_dir().join("src/bin"))
            .arg(manifest_dir().join("sql"))
            .status()
            .expect("python");
        assert!(st.success(), "security_scan.py: {st}");
    }

    // ---- Evidenze del Task 2 (dopo le misure) ------------------------------------------------

    fn results() -> std::path::PathBuf {
        manifest_dir().join("results/index")
    }

    /// CSV come header + righe (campi separati da virgola, nessun campo quotato nei file).
    fn csv(name: &str) -> (Vec<String>, Vec<Vec<String>>) {
        let s = read(&results().join(name));
        let mut lines = s.lines().filter(|l| !l.trim().is_empty());
        let header = lines
            .next()
            .unwrap_or_else(|| panic!("{name} vuoto"))
            .split(',')
            .map(str::to_owned)
            .collect();
        let rows = lines
            .map(|l| l.split(',').map(str::to_owned).collect())
            .collect();
        (header, rows)
    }

    const SCALES: [&str; 4] = ["10000", "50000", "100000", "300000"];
    const LEAK_ROLES: [&str; 9] = [
        "admin",
        "manager",
        "employee",
        "guest",
        "restricted-only",
        "sel100",
        "sel25",
        "sel5",
        "sel1",
    ];

    // TEST-5 (RISK-2): leak.csv completo per scala × config × ruolo × parte, e 0 negati.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/leak.csv e recall.csv dopo le misure"]
    fn test5_leak_csv_complete_and_zero() {
        let (h, rows) = csv("leak.csv");
        assert_eq!(
            h,
            [
                "chunks",
                "config",
                "role",
                "part",
                "queries",
                "denied_in_topk"
            ]
        );
        let leaks: Vec<_> = rows.iter().filter(|r| r[5] != "0").collect();
        assert!(
            leaks.is_empty(),
            "chunk negati nei top-k (gate fallito, RISULTATI deve dichiararlo): {leaks:?}"
        );
        let configs: BTreeSet<&str> = rows.iter().map(|r| r[1].as_str()).collect();
        assert!(
            configs.iter().any(|c| c.contains("exact")),
            "manca la configurazione esatta: {configs:?}"
        );
        let (_, rec) = csv("recall.csv");
        let hnsw: BTreeSet<(&str, &str, &str)> = rec
            .iter()
            .map(|r| (r[1].as_str(), r[2].as_str(), r[3].as_str()))
            .collect();
        assert!(
            configs.len() > hnsw.len(),
            "attese la configurazione esatta più ogni combinazione HNSW di recall.csv ({}), trovate {configs:?}",
            hnsw.len()
        );
        let mut seen: BTreeMap<(&str, &str, &str, &str), u64> = BTreeMap::new();
        for r in &rows {
            let q: u64 = r[4].parse().expect("queries intero");
            *seen.entry((&r[0], &r[1], &r[2], &r[3])).or_default() += q;
        }
        let mut holes = Vec::new();
        for s in SCALES {
            for c in &configs {
                for role in LEAK_ROLES {
                    for part in ["sem", "lex"] {
                        if seen.get(&(s, c, role, part)).copied().unwrap_or(0) == 0 {
                            holes.push(format!("{s}/{c}/{role}/{part}"));
                        }
                    }
                }
            }
        }
        assert!(holes.is_empty(), "griglia del leak incompleta: {holes:?}");
    }

    // TEST-15 (RISK-9): ingestion sotto carico per 5 minuti, righe al secondo, errori dichiarati.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/ingest_under_load.csv"]
    fn test15_ingest_under_load_evidence() {
        let (h, rows) = csv("ingest_under_load.csv");
        assert_eq!(h, ["t", "query_p50", "query_p95", "ingest_rows", "errors"]);
        assert!(
            rows.len() >= 300,
            "attese ≥ 300 righe (5 minuti), trovate {}",
            rows.len()
        );
        let ingest: Vec<u64> = rows
            .iter()
            .map(|r| r[3].parse().expect("ingest_rows"))
            .collect();
        assert!(
            ingest.windows(2).all(|w| w[0] <= w[1]),
            "ingest_rows deve essere non decrescente"
        );
        let errors: u64 = rows
            .iter()
            .map(|r| r[4].parse::<u64>().expect("errors"))
            .sum();
        let ris = read(&manifest_dir().join("RISULTATI-indice.md"));
        assert!(
            ris.contains("ingest_under_load.csv") && ris.to_lowercase().contains("errori"),
            "RISULTATI-indice deve riportare gli errori dell'ingestion concorrente (somma = {errors})"
        );
    }

    // TEST-17 (RISK-11): header, scale, piani EXPLAIN, hw.json e link di RISULTATI e ADR-0006.
    #[test]
    #[ignore = "evidenza del Task 2: richiede results/index/, RISULTATI.md e ADR-0006"]
    fn test17_evidence_and_links() {
        let (h, thr) = csv("threshold.csv");
        assert_eq!(
            h,
            [
                "chunks",
                "method",
                "parallel_workers",
                "selectivity",
                "run",
                "queries",
                "p50_ms",
                "p95_ms",
                "p99_ms"
            ]
        );
        for s in SCALES {
            for w in ["0", "4"] {
                for run in ["2", "3"] {
                    assert!(
                        thr.iter().any(|r| r[0] == s && r[2] == w && r[4] == run),
                        "threshold.csv: manca chunks={s} parallel_workers={w} run={run}"
                    );
                }
            }
        }
        let combos: BTreeSet<(&str, &str, &str)> = thr
            .iter()
            .map(|r| (r[0].as_str(), r[1].as_str(), r[2].as_str()))
            .collect();
        let plans: Vec<_> = std::fs::read_dir(results().join("plans"))
            .expect("results/index/plans")
            .map(|e| e.unwrap().path())
            .filter(|p| p.extension().is_some_and(|x| x == "json"))
            .collect();
        assert!(
            plans.len() >= combos.len(),
            "un piano EXPLAIN per configurazione di threshold: {} piani per {} configurazioni",
            plans.len(),
            combos.len()
        );
        for p in &plans {
            let v: serde_json::Value =
                serde_json::from_str(&read(p)).unwrap_or_else(|e| panic!("{}: {e}", p.display()));
            assert!(
                v.to_string().contains("\"Plan\""),
                "{}: manca \"Plan\"",
                p.display()
            );
        }
        let (lh, lat) = csv("latency.csv");
        assert_eq!(lh, ["chunks", "stage", "run", "p50_ms", "p95_ms", "p99_ms"]);
        for st in ["embed_query", "sql", "end_to_end"] {
            assert!(
                lat.iter().any(|r| r[1] == st),
                "latency.csv: manca lo stage {st}"
            );
        }
        let (rh, _) = csv("recall.csv");
        assert_eq!(
            rh,
            [
                "chunks",
                "index_type",
                "ef_search",
                "iterative_scan",
                "selectivity",
                "recall_vs_exact",
                "returns_k_rate",
                "build_s",
                "index_mb"
            ]
        );
        let (hh, _) = csv("hybrid.csv");
        assert_eq!(
            hh,
            [
                "mode",
                "lang",
                "queries",
                "recall_at_10",
                "ndcg_at_10",
                "p95_ms"
            ]
        );

        let hw: serde_json::Value =
            serde_json::from_str(&read(&results().join("hw.json"))).expect("hw.json valido");
        let images = hw["images"].as_object().expect("hw.json: images");
        assert!(
            images.keys().any(|k| k.contains("pgvector")),
            "hw.json: digest pgvector"
        );
        assert!(
            images
                .keys()
                .any(|k| k.contains("text-embeddings-inference")),
            "hw.json: digest TEI"
        );
        assert!(
            hw["tools"]["pgvector"].is_string(),
            "hw.json: tools.pgvector"
        );

        let ris = read(&manifest_dir().join("RISULTATI.md"));
        for l in [
            "RISULTATI-embedding.md",
            "RISULTATI-indice.md",
            "results/embedding/hw.json",
            "results/index/hw.json",
        ] {
            assert!(ris.contains(l), "RISULTATI.md di S-03 deve collegare {l}");
            assert!(manifest_dir().join(l).exists(), "{l} inesistente");
        }
        let adr =
            read(&repo_root().join("docs/adr/0006-indice-vettoriale-ricerca-esatta-ibrida.md"));
        assert!(
            adr.contains("s03-pgvector/RISULTATI.md"),
            "ADR-0006: link a RISULTATI.md"
        );
        for tok in adr.split(|c: char| c.is_whitespace() || "()[]<>`\"'".contains(c)) {
            if let Some(i) = tok.find("spikes/") {
                let p = tok[i..].trim_end_matches(['.', ',', ';', ':']);
                assert!(
                    repo_root().join(p).exists(),
                    "ADR-0006 cita un percorso inesistente: {p}"
                );
            }
        }
    }

    // ---- Controlli git ------------------------------------------------------------------------

    const BASE: &str = "4586ff305fd115a5b85ee83eaea5b3920ed3927f";
    const SOGLIE: &str = "## Soglie (scritte PRIMA della misura)";

    fn git(args: &[&str]) -> String {
        let out = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(args)
            .output()
            .expect("git");
        assert!(
            out.status.success(),
            "git {args:?}: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        String::from_utf8(out.stdout).unwrap()
    }

    fn soglie_section(md: &str) -> String {
        let start = md.find(SOGLIE).expect("sezione Soglie");
        let rest = &md[start + SOGLIE.len()..];
        let end = rest.find("\n## ").unwrap_or(rest.len());
        rest[..end].to_owned()
    }

    // TEST-10 (RISK-5): Soglie committate prima di ogni file in results/index/ e invariate.
    #[test]
    #[ignore = "verifica git del Task 2: richiede la storia del branch con results/index/"]
    fn test10_thresholds_committed_before_results() {
        let ris = "spikes/s03-pgvector/RISULTATI-indice.md";
        let intro = git(&["log", "--reverse", "--format=%H", "-S", SOGLIE, "--", ris]);
        let intro = intro
            .lines()
            .next()
            .expect("commit che introduce le Soglie")
            .to_owned();
        let first_res = git(&[
            "log",
            "--reverse",
            "--diff-filter=A",
            "--format=%H",
            "--",
            "spikes/s03-pgvector/results/index",
        ]);
        let first_res = first_res
            .lines()
            .next()
            .expect("primo commit di results/index")
            .to_owned();
        assert_ne!(intro, first_res, "Soglie e risultati nello stesso commit");
        let st = Command::new("git")
            .arg("-C")
            .arg(repo_root())
            .args(["merge-base", "--is-ancestor", &intro, &first_res])
            .status()
            .expect("git");
        assert!(
            st.success(),
            "le Soglie ({intro}) non precedono i risultati ({first_res})"
        );
        let at_intro = soglie_section(&git(&["show", &format!("{intro}:{ris}")]));
        let at_head = soglie_section(&read(&repo_root().join(ris)));
        assert_eq!(
            at_intro, at_head,
            "le Soglie sono state ritoccate dopo l'introduzione"
        );
        for needle in ["50 ms", "0,95", "100%", "0,03", "200 ms", "2 ×"] {
            assert!(at_head.contains(needle), "Soglie: manca «{needle}»");
        }
    }

    // TEST-14 (RISK-8): il piano non tocca i file del piano 01-10.
    #[test]
    #[ignore = "verifica git: richiede il repo sull'host (in container .git del worktree non risolve)"]
    fn test14_plan_0110_files_untouched() {
        let changed = git(&["diff", "--name-only", &format!("{BASE}..HEAD")]);
        let forbidden = [
            "spikes/s03-pgvector/src/lib.rs",
            "spikes/s03-pgvector/src/chunk.rs",
            "spikes/s03-pgvector/src/tei.rs",
            "spikes/s03-pgvector/src/db.rs",
            "spikes/s03-pgvector/src/metrics.rs",
            "spikes/s03-pgvector/src/main.rs",
            "spikes/s03-pgvector/sql/schema.sql",
            "spikes/s03-pgvector/RISULTATI-embedding.md",
        ];
        let touched: Vec<&str> = changed.lines().filter(|l| forbidden.contains(l)).collect();
        assert!(
            touched.is_empty(),
            "file del piano 01-10 modificati: {touched:?}"
        );
    }
}
