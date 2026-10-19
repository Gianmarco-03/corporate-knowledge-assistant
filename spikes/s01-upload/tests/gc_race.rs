//! TEST-5: race fra GC e adozione sulle interleaving enumerate, con barriere `GcHooks`,
//! per gli adapter `garage` e `pgbytea`. TEST-9 (parte): GC interrotto fra delete e
//! cancellazione della riga riprende senza righe `deleting` residue; adozione
//! abbandonata (crash fra PUT e transazione) raccolta.
//!
//! Prova delle sole interleaving enumerate, non dell'assenza di ogni race.
//! Richiede `postgres` e `garage` avviati nella sessione (`session.sh up "$CKA_RUN_ID"
//! postgres garage`) e l'env del runner (`DATABASE_URL`, `S3_*`, `GARAGE_*`).
//! Scrive `results/gc_race.csv`.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use futures::future::BoxFuture;
use s01_upload::gc::{self, GcHooks, NoopHooks};
use s01_upload::sink::{BlobSink, GarageSink, PgByteaSink};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tokio::sync::Notify;
use uuid::Uuid;

const DAY: Duration = gc::DEFAULT_MIN_AGE;

/// Barriera: segnala l'arrivo al punto e, se richiesto, attende il rilascio.
#[derive(Default)]
struct Barrier {
    reached: Notify,
    release: Notify,
}

#[derive(Default)]
struct BarrierHooks {
    after_mark: Option<Barrier>,
    after_delete: Option<Barrier>,
    /// Dopo il delete resta fermo per sempre: il test interrompe il task (caso c).
    hang_after_delete: bool,
}

impl GcHooks for BarrierHooks {
    fn after_mark<'a>(&'a self, _sha: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(b) = &self.after_mark {
                b.reached.notify_one();
                b.release.notified().await;
            }
        })
    }
    fn after_delete<'a>(&'a self, _sha: &'a str) -> BoxFuture<'a, ()> {
        Box::pin(async move {
            if let Some(b) = &self.after_delete {
                b.reached.notify_one();
                if self.hang_after_delete {
                    std::future::pending::<()>().await;
                }
                b.release.notified().await;
            }
        })
    }
}

struct Fixture {
    sha: String,
    path: PathBuf,
    size: i64,
    _dir: tempfile::TempDir,
}

fn fixture(tag: &str) -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("blob.bin");
    let seed = format!("gc-race {tag} {}\n", Uuid::new_v4());
    let data = seed.repeat((128 * 1024) / seed.len() + 1).into_bytes();
    std::fs::write(&path, &data).expect("write");
    Fixture {
        sha: hex::encode(Sha256::digest(&data)),
        path,
        size: data.len() as i64,
        _dir: dir,
    }
}

async fn reset(db: &PgPool) {
    sqlx::raw_sql("TRUNCATE upload, outbox, blob, blob_chunk")
        .execute(db)
        .await
        .expect("truncate");
}

/// Orfano: oggetto nello storage, riga `blob` senza riga `upload`.
async fn seed_blob(db: &PgPool, sink: &dyn BlobSink, f: &Fixture, pending: i32, age_hours: i32) {
    sink.put(&f.sha, &f.path).await.expect("put");
    sqlx::query(
        "INSERT INTO blob (sha256, state, pending, updated_at) \
         VALUES ($1, 'live', $2, now() - ($3::int * interval '1 hour'))",
    )
    .bind(&f.sha)
    .bind(pending)
    .bind(age_hours)
    .execute(db)
    .await
    .expect("seed blob");
}

/// Transazione breve dell'upload: riga `upload`, riga `outbox`, adozione B.
async fn finish_upload(db: &PgPool, f: &Fixture) {
    let mut tx = db.begin().await.expect("begin");
    sqlx::query(
        "INSERT INTO upload (id, sha256, size_bytes, status, created_at) \
         VALUES ($1, $2, $3, 'accepted', now())",
    )
    .bind(Uuid::new_v4())
    .bind(&f.sha)
    .bind(f.size)
    .execute(&mut *tx)
    .await
    .expect("upload row");
    sqlx::query(
        "INSERT INTO outbox (event_id, payload, created_at) VALUES ($1, $2::text::jsonb, now())",
    )
    .bind(Uuid::new_v4())
    .bind(format!(r#"{{"sha256":"{}"}}"#, f.sha))
    .execute(&mut *tx)
    .await
    .expect("outbox row");
    gc::adopt_commit(&mut tx, &f.sha)
        .await
        .expect("adopt_commit");
    tx.commit().await.expect("commit");
}

/// Upload committato e vecchio: non va mai raccolto.
async fn seed_committed(db: &PgPool, sink: &dyn BlobSink, tag: &str) -> Fixture {
    let f = fixture(tag);
    seed_blob(db, sink, &f, 1, 48).await;
    finish_upload(db, &f).await;
    sqlx::query("UPDATE blob SET updated_at = now() - interval '48 hours' WHERE sha256 = $1")
        .bind(&f.sha)
        .execute(db)
        .await
        .expect("age");
    f
}

async fn blob_row(db: &PgPool, sha: &str) -> Option<(String, i32)> {
    sqlx::query_as::<_, (String, i32)>("SELECT state, pending FROM blob WHERE sha256 = $1")
        .bind(sha)
        .fetch_optional(db)
        .await
        .expect("blob row")
}

async fn readable(sink: &dyn BlobSink, sha: &str) -> bool {
    matches!(sink.get_range(sha, 0..16).await, Ok(b) if !b.is_empty())
}

/// Ogni riga `upload` ha l'oggetto leggibile: restituisce `(upload_rows, unreadable)`.
async fn invariant(db: &PgPool, sink: &dyn BlobSink) -> (u64, u64) {
    let shas: Vec<(String,)> = sqlx::query_as("SELECT sha256 FROM upload")
        .fetch_all(db)
        .await
        .expect("upload rows");
    let mut unreadable = 0;
    for (sha,) in &shas {
        if !readable(sink, sha).await {
            unreadable += 1;
        }
    }
    (shas.len() as u64, unreadable)
}

async fn wait(n: &Notify, what: &str) {
    tokio::time::timeout(Duration::from_secs(20), n.notified())
        .await
        .unwrap_or_else(|_| panic!("barriera non raggiunta: {what}"));
}

/// (a) GC marca `deleting` → l'upload dello stesso hash entra in adozione A → attende,
/// poi ricrea l'oggetto.
async fn case_a(db: &PgPool, sink: Arc<dyn BlobSink>) -> anyhow::Result<()> {
    let f = fixture("a");
    seed_blob(db, sink.as_ref(), &f, 0, 48).await;
    let hooks = Arc::new(BarrierHooks {
        after_mark: Some(Barrier::default()),
        ..Default::default()
    });
    let gc_task = {
        let (db, sink, hooks) = (db.clone(), sink.clone(), hooks.clone());
        tokio::spawn(async move { gc::run(&db, sink.as_ref(), DAY, hooks.as_ref()).await })
    };
    wait(
        &hooks.after_mark.as_ref().expect("barrier").reached,
        "after_mark",
    )
    .await;
    anyhow::ensure!(
        blob_row(db, &f.sha).await.map(|r| r.0) == Some("deleting".into()),
        "dopo il passo (1) la riga deve essere deleting"
    );
    let adopt = {
        let (db, sha) = (db.clone(), f.sha.clone());
        tokio::spawn(async move { gc::adopt_begin(&db, &sha).await })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    if adopt.is_finished() {
        let early = adopt.await?;
        anyhow::ensure!(early.is_err(), "adozione riuscita su una riga deleting");
    } else {
        hooks
            .after_mark
            .as_ref()
            .expect("barrier")
            .release
            .notify_one();
        gc_task.await??;
        // Se il budget di retry si è esaurito, il client ritenta dopo il 503.
        if adopt.await?.is_err() {
            gc::adopt_begin(db, &f.sha).await?;
        }
        return finish_case_a(db, sink.as_ref(), &f).await;
    }
    hooks
        .after_mark
        .as_ref()
        .expect("barrier")
        .release
        .notify_one();
    gc_task.await??;
    gc::adopt_begin(db, &f.sha).await?;
    finish_case_a(db, sink.as_ref(), &f).await
}

async fn finish_case_a(db: &PgPool, sink: &dyn BlobSink, f: &Fixture) -> anyhow::Result<()> {
    sink.put(&f.sha, &f.path).await?;
    finish_upload(db, f).await;
    anyhow::ensure!(readable(sink, &f.sha).await, "oggetto non ricreato");
    anyhow::ensure!(
        blob_row(db, &f.sha).await == Some(("live".into(), 0)),
        "riga blob attesa live/0"
    );
    Ok(())
}

/// (b) Adozione A committata → il GC salta l'hash.
async fn case_b(db: &PgPool, sink: Arc<dyn BlobSink>) -> anyhow::Result<()> {
    let f = fixture("b");
    seed_blob(db, sink.as_ref(), &f, 0, 48).await;
    gc::adopt_begin(db, &f.sha).await?;
    gc::run(db, sink.as_ref(), DAY, &NoopHooks).await?;
    anyhow::ensure!(
        readable(sink.as_ref(), &f.sha).await,
        "GC ha cancellato un hash in adozione"
    );
    anyhow::ensure!(
        blob_row(db, &f.sha).await.map(|r| r.0) == Some("live".into()),
        "riga in adozione non più live"
    );
    sink.put(&f.sha, &f.path).await?;
    finish_upload(db, &f).await;
    gc::run(db, sink.as_ref(), DAY, &NoopHooks).await?;
    anyhow::ensure!(
        readable(sink.as_ref(), &f.sha).await,
        "oggetto adottato raccolto"
    );
    Ok(())
}

/// (c) GC interrotto fra delete e cancellazione della riga → rilancio, nessuna riga
/// `deleting` residua.
async fn case_c(db: &PgPool, sink: Arc<dyn BlobSink>) -> anyhow::Result<()> {
    let f = fixture("c");
    seed_blob(db, sink.as_ref(), &f, 0, 48).await;
    let hooks = Arc::new(BarrierHooks {
        after_delete: Some(Barrier::default()),
        hang_after_delete: true,
        ..Default::default()
    });
    let gc_task = {
        let (db, sink, hooks) = (db.clone(), sink.clone(), hooks.clone());
        tokio::spawn(async move { gc::run(&db, sink.as_ref(), DAY, hooks.as_ref()).await })
    };
    wait(
        &hooks.after_delete.as_ref().expect("barrier").reached,
        "after_delete",
    )
    .await;
    gc_task.abort();
    let _ = gc_task.await;
    anyhow::ensure!(
        blob_row(db, &f.sha).await.map(|r| r.0) == Some("deleting".into()),
        "dopo l'interruzione la riga resta deleting"
    );
    gc::run(db, sink.as_ref(), DAY, &NoopHooks).await?;
    let residue: (i64,) = sqlx::query_as("SELECT count(*) FROM blob WHERE state = 'deleting'")
        .fetch_one(db)
        .await?;
    anyhow::ensure!(residue.0 == 0, "righe deleting residue: {}", residue.0);
    anyhow::ensure!(
        !readable(sink.as_ref(), &f.sha).await,
        "oggetto orfano non cancellato"
    );
    Ok(())
}

/// (d) Adozione abbandonata (crash fra PUT e transazione) più vecchia di N (N = 0) →
/// azzerata e raccolta.
async fn case_d(db: &PgPool, sink: Arc<dyn BlobSink>) -> anyhow::Result<()> {
    let f = fixture("d");
    seed_blob(db, sink.as_ref(), &f, 1, 1).await;
    gc::run(db, sink.as_ref(), Duration::ZERO, &NoopHooks).await?;
    if blob_row(db, &f.sha).await.is_some() {
        // L'azzeramento e la raccolta possono stare in due giri.
        gc::run(db, sink.as_ref(), Duration::ZERO, &NoopHooks).await?;
    }
    anyhow::ensure!(
        blob_row(db, &f.sha).await.is_none(),
        "adozione abbandonata non raccolta"
    );
    anyhow::ensure!(
        !readable(sink.as_ref(), &f.sha).await,
        "oggetto abbandonato ancora presente"
    );
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[cfg_attr(
    not(target_os = "linux"),
    ignore = "richiede postgres e garage della sessione: nel runner via session.sh oneshot"
)]
async fn test5_gc_race_interleavings_enumerate() {
    let url = common::env::required("DATABASE_URL").expect("DATABASE_URL");
    let db = sqlx::postgres::PgPoolOptions::new()
        .max_connections(8)
        .connect(&url)
        .await
        .expect("connect postgres");
    gc::apply_schema(&db).await.expect("schema");

    let adapters: Vec<(&str, Arc<dyn BlobSink>)> = vec![
        (
            "garage",
            Arc::new(GarageSink::from_env().expect("garage env")),
        ),
        ("pgbytea", Arc::new(PgByteaSink::new(db.clone()))),
    ];
    type Case = for<'a> fn(&'a PgPool, Arc<dyn BlobSink>) -> BoxFuture<'a, anyhow::Result<()>>;
    let cases: [(&str, Case); 4] = [
        ("a_mark_then_adopt", |db, s| Box::pin(case_a(db, s))),
        ("b_adopt_then_gc", |db, s| Box::pin(case_b(db, s))),
        ("c_gc_interrupted", |db, s| Box::pin(case_c(db, s))),
        ("d_abandoned_adoption", |db, s| Box::pin(case_d(db, s))),
    ];

    let mut rows = Vec::new();
    let mut failures = Vec::new();
    for (adapter, sink) in &adapters {
        for (name, case) in &cases {
            reset(&db).await;
            let kept = seed_committed(&db, sink.as_ref(), &format!("kept-{name}")).await;
            let res = case(&db, sink.clone()).await;
            // Il caso (d) usa N = 0: l'upload committato resta comunque protetto.
            let kept_ok = readable(sink.as_ref(), &kept.sha).await;
            let (upload_rows, unreadable) = invariant(&db, sink.as_ref()).await;
            let objects = sink.count().await.unwrap_or(u64::MAX);
            let pass = res.is_ok() && kept_ok && unreadable == 0;
            if !pass {
                failures.push(format!(
                    "{adapter}/{name}: {:?}, kept_ok={kept_ok}, unreadable={unreadable}",
                    res.err()
                ));
            }
            rows.push([
                adapter.to_string(),
                name.to_string(),
                upload_rows.to_string(),
                objects.to_string(),
                unreadable.to_string(),
                pass.to_string(),
            ]);
        }
    }

    let csv_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("results/gc_race.csv");
    std::fs::create_dir_all(csv_path.parent().expect("parent")).expect("results/");
    let mut csv = common::csv::Writer::create(
        &csv_path,
        &[
            "adapter",
            "interleaving",
            "upload_rows",
            "objects",
            "unreadable",
            "pass",
        ],
    )
    .expect("csv");
    for r in &rows {
        let fields: Vec<&str> = r.iter().map(String::as_str).collect();
        csv.write_row(&fields).expect("csv row");
    }
    drop(csv);

    assert!(
        failures.is_empty(),
        "interleaving fallite:\n{}",
        failures.join("\n")
    );
}
