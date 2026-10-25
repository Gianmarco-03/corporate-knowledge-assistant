//! `synth`: corpus sintetico a 10k/50k/100k/300k chunk (piano 01-14). Perturbazione gaussiana
//! dei vettori reali con seme fisso e rinormalizzazione; documenti sintetici con la stessa
//! distribuzione di classificazioni del corpus reale; ruoli sintetici `sel100`, `sel25`,
//! `sel5`, `sel1` in `doc_acl` per controllare la selettività. La regola di assegnazione è
//! ricalcolabile dal seme ([`expected_visibility`]): è la matrice attesa contro cui `leak`
//! conta i negati, indipendente da `doc_acl`.
//!
//! Ogni valore casuale viene da [`Rng`] (SplitMix64) inizializzato con il seme, la scala e
//! l'indice dell'elemento: la generazione non dipende dall'ordine né dalla versione di un
//! crate esterno, e un elemento si ricalcola da solo (streaming a lotti, nessun vettore
//! intero in memoria).
//!
//! **Modifica:** Claude (test-write 01-14) · 2026-10-25 · scheletro e test rossi del design
//! **Modifica:** Claude (gsd-executor 01-14) · 2026-10-25 · generazione deterministica, matrice attesa e caricamento a lotti
use std::collections::{BTreeMap, HashSet};

use anyhow::{Context, bail};
use pgvector::Vector;
use s03_pgvector::metrics::{self, Visibility};
use sqlx::{PgPool, Row};
use uuid::Uuid;

/// Seme fisso della generazione sintetica.
pub const SEED: u64 = 0x0114;

/// Ruoli sintetici e quota di chunk visibili attesa.
pub const SYNTH_ROLES: [(&str, f64); 4] = [
    ("sel100", 1.0),
    ("sel25", 0.25),
    ("sel5", 0.05),
    ("sel1", 0.01),
];

/// Chunk per documento sintetico: con 200…6.000 documenti l'1% è un numero intero di
/// documenti, quindi la selettività per chunk è esatta.
pub const CHUNKS_PER_DOC: usize = 50;

/// Norma attesa del rumore gaussiano aggiunto a ogni vettore reale.
pub const NOISE_NORM: f64 = 0.5;

/// Righe per istruzione di inserimento.
const INSERT_BATCH: usize = 500;

/// Etichette che separano i flussi casuali derivati dallo stesso seme.
const TAG_VEC: u64 = 0x5645_4354;
const TAG_CLASS: u64 = 0x434c_4153;
const TAG_SEL: u64 = 0x5345_4c00;
const TAG_DOC_ID: u64 = 0x444f_4349;
const TAG_CHUNK_ID: u64 = 0x4348_4b49;

/// Scala del corpus sintetico: enum chiuso, da cui deriva il nome della tabella.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Scale {
    N10k,
    N50k,
    N100k,
    N300k,
}

impl Scale {
    pub const ALL: [Scale; 4] = [Scale::N10k, Scale::N50k, Scale::N100k, Scale::N300k];

    /// Numero di chunk della scala.
    pub fn n(self) -> usize {
        match self {
            Scale::N10k => 10_000,
            Scale::N50k => 50_000,
            Scale::N100k => 100_000,
            Scale::N300k => 300_000,
        }
    }

    /// Tabella `chunk_synth_<n>`: `match` su enum chiuso, mai dall'input.
    pub fn table(self) -> &'static str {
        match self {
            Scale::N10k => "chunk_synth_10000",
            Scale::N50k => "chunk_synth_50000",
            Scale::N100k => "chunk_synth_100000",
            Scale::N300k => "chunk_synth_300000",
        }
    }

    /// Scala da un numero di chunk della riga di comando.
    pub fn from_n(n: usize) -> anyhow::Result<Scale> {
        Scale::ALL
            .into_iter()
            .find(|s| s.n() == n)
            .with_context(|| format!("scala {n} non prevista: 10000, 50000, 100000 o 300000"))
    }

    /// Documenti sintetici della scala.
    pub fn n_docs(self) -> usize {
        self.n() / CHUNKS_PER_DOC
    }

    /// Prefisso dei `doc_key` sintetici della scala (per `LIKE`).
    pub fn key_prefix(self) -> &'static str {
        match self {
            Scale::N10k => "synth-10000-",
            Scale::N50k => "synth-50000-",
            Scale::N100k => "synth-100000-",
            Scale::N300k => "synth-300000-",
        }
    }
}

/// SplitMix64: generatore piccolo e riproducibile (misura, non crittografia).
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(seed)
    }

    pub fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// Uniforme in (0, 1), estremi esclusi.
    pub fn unit(&mut self) -> f64 {
        ((self.next_u64() >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    /// Normale standard (Box-Muller).
    pub fn gauss(&mut self) -> f64 {
        let (u1, u2) = (self.unit(), self.unit());
        (-2.0 * u1.ln()).sqrt() * (std::f64::consts::TAU * u2).cos()
    }

    /// Intero uniforme in `0..n` (n > 0).
    pub fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// Seme derivato da seme di base, etichetta del flusso e indice dell'elemento.
pub fn mix(seed: u64, tag: u64, i: u64) -> u64 {
    let mut r = Rng::new(seed ^ tag.rotate_left(17) ^ i.wrapping_mul(0xD1B5_4A32_D192_ED03));
    r.next_u64()
}

/// Seme della scala: le scale hanno flussi indipendenti.
fn scale_seed(seed: u64, scale: Scale) -> u64 {
    mix(seed, 0x5343_414c, scale.n() as u64)
}

/// Uuid deterministico da seme, etichetta e indice.
fn det_uuid(seed: u64, tag: u64, i: u64) -> Uuid {
    let hi = mix(seed, tag, i);
    let lo = mix(seed, tag ^ 0xFFFF, i);
    Uuid::from_u128((u128::from(hi) << 64) | u128::from(lo))
}

/// Il vettore sintetico `i`: vettore reale `i mod base.len()` più rumore gaussiano di norma
/// attesa [`NOISE_NORM`], rinormalizzato a norma 1.
pub fn perturb_one(base: &[Vec<f32>], i: usize, seed: u64) -> Vec<f32> {
    let b = &base[i % base.len()];
    let sigma = NOISE_NORM / (b.len() as f64).sqrt();
    let mut rng = Rng::new(mix(seed, TAG_VEC, i as u64));
    let v: Vec<f64> = b
        .iter()
        .map(|x| f64::from(*x) + sigma * rng.gauss())
        .collect();
    let norm = v.iter().map(|x| x * x).sum::<f64>().sqrt();
    v.into_iter().map(|x| (x / norm) as f32).collect()
}

/// `n` vettori sintetici dai vettori reali `base`: perturbazione gaussiana con seme `seed`,
/// poi norma L2 = 1. Stesso seme, stesso risultato bit a bit.
pub fn perturb(base: &[Vec<f32>], n: usize, seed: u64) -> Vec<Vec<f32>> {
    (0..n).map(|i| perturb_one(base, i, seed)).collect()
}

/// Permutazione di `0..n` (Fisher-Yates) dal seme.
fn permutation(n: usize, seed: u64) -> Vec<usize> {
    let mut p: Vec<usize> = (0..n).collect();
    let mut rng = Rng::new(seed);
    for i in (1..n).rev() {
        let j = rng.below(i + 1);
        p.swap(i, j);
    }
    p
}

/// Classificazioni di `n_docs` documenti sintetici con le stesse proporzioni di `real`
/// (metodo dei resti più grandi, a parità di resto in ordine di nome), in ordine mescolato
/// dal seme.
pub fn synth_classifications(real: &[&str], n_docs: usize, seed: u64) -> Vec<String> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for c in real {
        *counts.entry(c).or_default() += 1;
    }
    let total = real.len();
    if total == 0 {
        return Vec::new();
    }
    let mut quota: Vec<(&str, usize, usize)> = counts
        .iter()
        .map(|(c, k)| (*c, k * n_docs / total, (k * n_docs) % total))
        .collect();
    let assigned: usize = quota.iter().map(|q| q.1).sum();
    let mut order: Vec<usize> = (0..quota.len()).collect();
    order.sort_by(|a, b| {
        quota[*b]
            .2
            .cmp(&quota[*a].2)
            .then(quota[*a].0.cmp(quota[*b].0))
    });
    for idx in order.into_iter().take(n_docs - assigned) {
        quota[idx].1 += 1;
    }
    let mut out: Vec<String> = Vec::with_capacity(n_docs);
    for (c, k, _) in &quota {
        out.extend(std::iter::repeat_n((*c).to_string(), *k));
    }
    let perm = permutation(n_docs, mix(seed, TAG_CLASS, 0));
    perm.into_iter().map(|i| out[i].clone()).collect()
}

/// Un documento sintetico: chiave, id, classificazione e rango nella permutazione dei ruoli
/// sintetici (visibile a `selNN` se il rango è sotto la quota del ruolo).
#[derive(Clone, Debug)]
pub struct SynthDoc {
    pub key: String,
    pub id: Uuid,
    pub classification: String,
    pub sel_rank: usize,
}

/// Documenti sintetici della scala, ricalcolati dal seme e dalle classificazioni dei
/// documenti reali (in qualunque ordine: si ordinano per chiave).
pub fn synth_docs(scale: Scale, seed: u64, base_docs: &[(String, String)]) -> Vec<SynthDoc> {
    let mut base: Vec<&(String, String)> = base_docs.iter().collect();
    base.sort();
    let classes: Vec<&str> = base.iter().map(|(_, c)| c.as_str()).collect();
    let ss = scale_seed(seed, scale);
    let n_docs = scale.n_docs();
    let cls = synth_classifications(&classes, n_docs, ss);
    let perm = permutation(n_docs, mix(ss, TAG_SEL, 0));
    let mut rank = vec![0usize; n_docs];
    for (r, d) in perm.into_iter().enumerate() {
        rank[d] = r;
    }
    cls.into_iter()
        .enumerate()
        .map(|(i, classification)| SynthDoc {
            key: synth_key(scale, i),
            id: det_uuid(ss, TAG_DOC_ID, i as u64),
            classification,
            sel_rank: rank[i],
        })
        .collect()
}

/// `doc_key` del documento sintetico `i` della scala.
pub fn synth_key(scale: Scale, i: usize) -> String {
    format!("{}{i:05}", scale.key_prefix())
}

/// Documenti visibili a un ruolo sintetico: i primi `round(quota × documenti)` della
/// permutazione (insiemi annidati).
fn sel_visible(share: f64, n_docs: usize, rank: usize) -> bool {
    rank < (share * n_docs as f64).round() as usize
}

/// Matrice attesa sui documenti sintetici della scala, per tutti i ruoli e profili di
/// `acl.json` e per i ruoli sintetici, ricalcolata dal seme e dai documenti reali
/// `(doc_key, classification)`. Non legge `doc_acl`.
pub fn expected_visibility(
    scale: Scale,
    seed: u64,
    acl_json: &str,
    base_docs: &[(String, String)],
) -> anyhow::Result<Visibility> {
    let docs = synth_docs(scale, seed, base_docs);
    let manifest: Vec<serde_json::Value> = docs
        .iter()
        .map(|d| serde_json::json!({"doc_id": d.key, "classification": d.classification}))
        .collect();
    let mut vis = metrics::expected_visibility(acl_json, &serde_json::to_string(&manifest)?)?;
    let n_docs = docs.len();
    for (role, share) in SYNTH_ROLES {
        let set: HashSet<String> = docs
            .iter()
            .filter(|d| sel_visible(share, n_docs, d.sel_rank))
            .map(|d| d.key.clone())
            .collect();
        vis.insert(role.to_string(), set);
    }
    Ok(vis)
}

/// Documenti reali di base: quelli con chunk in `chunk_bgem3` (mai i sintetici).
pub async fn base_docs(pool: &PgPool) -> anyhow::Result<Vec<(String, String)>> {
    let rows = sqlx::query(
        "SELECT d.doc_key, d.classification FROM doc d \
         WHERE EXISTS (SELECT 1 FROM chunk_bgem3 c WHERE c.doc_id = d.id) ORDER BY d.doc_key",
    )
    .fetch_all(pool)
    .await
    .context("documenti di base")?;
    rows.iter()
        .map(|r| Ok((r.try_get("doc_key")?, r.try_get("classification")?)))
        .collect()
}

/// Chunk reali di base (vettore, testo, lingua) in ordine di `id`.
pub struct BaseChunks {
    pub vecs: Vec<Vec<f32>>,
    pub bodies: Vec<String>,
    pub langs: Vec<String>,
}

pub async fn base_chunks(pool: &PgPool) -> anyhow::Result<BaseChunks> {
    let rows =
        sqlx::query("SELECT c.emb, c.body, c.lang::text AS lang FROM chunk_bgem3 c ORDER BY c.id")
            .fetch_all(pool)
            .await
            .context("chunk di base")?;
    let mut b = BaseChunks {
        vecs: Vec::with_capacity(rows.len()),
        bodies: Vec::with_capacity(rows.len()),
        langs: Vec::with_capacity(rows.len()),
    };
    for r in rows {
        let v: Vector = r.try_get("emb")?;
        b.vecs.push(v.to_vec());
        b.bodies.push(r.try_get("body")?);
        b.langs.push(r.try_get("lang")?);
    }
    if b.vecs.is_empty() {
        bail!(
            "chunk_bgem3 vuota: caricare prima il corpus reale (piano 01-10, load --model bgem3)"
        );
    }
    Ok(b)
}

/// Id del chunk sintetico `i` della scala.
pub fn synth_chunk_id(scale: Scale, seed: u64, i: usize) -> Uuid {
    det_uuid(scale_seed(seed, scale), TAG_CHUNK_ID, i as u64)
}

/// Seme dei vettori della scala.
pub fn vec_seed(scale: Scale, seed: u64) -> u64 {
    scale_seed(seed, scale)
}

/// Inserisce un lotto di chunk nella tabella della scala.
pub(crate) async fn insert_lot<'c, E>(ex: E, scale: Scale, rows: InsertLot) -> anyhow::Result<()>
where
    E: sqlx::Executor<'c, Database = sqlx::Postgres>,
{
    sqlx::query(insert_sql(scale))
        .bind(rows.ids)
        .bind(rows.doc_ids)
        .bind(rows.char_ends)
        .bind(rows.langs)
        .bind(rows.bodies)
        .bind(rows.vecs)
        .execute(ex)
        .await
        .context("insert dei chunk sintetici (UNNEST)")?;
    Ok(())
}

/// Colonne di un lotto di inserimento.
#[derive(Default)]
pub(crate) struct InsertLot {
    pub ids: Vec<Uuid>,
    pub doc_ids: Vec<Uuid>,
    pub char_ends: Vec<i32>,
    pub langs: Vec<String>,
    pub bodies: Vec<String>,
    pub vecs: Vec<Vector>,
}

impl InsertLot {
    pub fn push(&mut self, id: Uuid, doc_id: Uuid, body: &str, lang: &str, v: Vec<f32>) {
        self.ids.push(id);
        self.doc_ids.push(doc_id);
        self.char_ends
            .push(i32::try_from(body.chars().count()).unwrap_or(i32::MAX));
        self.langs.push(lang.to_string());
        self.bodies.push(body.to_string());
        self.vecs.push(Vector::from(v));
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }
}

/// Righe `(doc_id, role)` di `doc_acl` per una matrice su chiavi note.
pub(crate) async fn insert_acl<'c, E>(
    ex: E,
    vis: &Visibility,
    ids: &std::collections::HashMap<String, Uuid>,
) -> anyhow::Result<()>
where
    E: sqlx::Executor<'c, Database = sqlx::Postgres>,
{
    let mut doc_ids = Vec::new();
    let mut roles = Vec::new();
    let mut keys: Vec<(&String, &HashSet<String>)> = vis.iter().collect();
    keys.sort_by(|a, b| a.0.cmp(b.0));
    for (role, docs) in keys {
        let mut ds: Vec<&String> = docs.iter().collect();
        ds.sort();
        for k in ds {
            let id = ids
                .get(k)
                .with_context(|| format!("documento {k} senza id"))?;
            doc_ids.push(*id);
            roles.push(role.clone());
        }
    }
    sqlx::query(
        "INSERT INTO doc_acl (doc_id, role) SELECT u.doc_id, u.role \
         FROM UNNEST($1::uuid[], $2::text[]) AS u (doc_id, role)",
    )
    .bind(doc_ids)
    .bind(roles)
    .execute(ex)
    .await
    .context("insert doc_acl sintetica")?;
    Ok(())
}

/// Genera la scala nel database: documenti sintetici in `doc`, righe di `doc_acl` per i ruoli
/// di `acl.json` e per i ruoli sintetici, chunk in `chunk_synth_<n>` dai vettori di
/// `chunk_bgem3` (inserimento a lotti). Rigenerare una scala la sostituisce.
pub async fn generate(
    pool: &PgPool,
    scale: Scale,
    seed: u64,
    acl_json: &str,
) -> anyhow::Result<()> {
    let base = base_docs(pool).await?;
    let chunks = base_chunks(pool).await?;
    let docs = synth_docs(scale, seed, &base);
    let vis = expected_visibility(scale, seed, acl_json, &base)?;

    // Pulizia della scala: chunk, righe di doc_acl e documenti con il prefisso della scala.
    let like = format!("{}%", scale.key_prefix());
    sqlx::query(truncate_sql(scale))
        .execute(pool)
        .await
        .context("truncate della scala")?;
    sqlx::query("DELETE FROM doc_acl WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)")
        .bind(&like)
        .execute(pool)
        .await
        .context("pulizia doc_acl")?;
    sqlx::query("DELETE FROM doc WHERE doc_key LIKE $1")
        .bind(&like)
        .execute(pool)
        .await
        .context("pulizia doc")?;

    sqlx::query(
        "INSERT INTO doc (id, doc_key, classification) SELECT u.id, u.doc_key, u.classification \
         FROM UNNEST($1::uuid[], $2::text[], $3::text[]) AS u (id, doc_key, classification)",
    )
    .bind(docs.iter().map(|d| d.id).collect::<Vec<_>>())
    .bind(docs.iter().map(|d| d.key.clone()).collect::<Vec<_>>())
    .bind(
        docs.iter()
            .map(|d| d.classification.clone())
            .collect::<Vec<_>>(),
    )
    .execute(pool)
    .await
    .context("insert dei documenti sintetici")?;
    let ids: std::collections::HashMap<String, Uuid> =
        docs.iter().map(|d| (d.key.clone(), d.id)).collect();
    insert_acl(pool, &vis, &ids).await?;

    let vs = vec_seed(scale, seed);
    let n_base = chunks.vecs.len();
    let mut lot = InsertLot::default();
    for i in 0..scale.n() {
        let b = i % n_base;
        let doc = &docs[i / CHUNKS_PER_DOC];
        lot.push(
            synth_chunk_id(scale, seed, i),
            doc.id,
            &chunks.bodies[b],
            &chunks.langs[b],
            perturb_one(&chunks.vecs, i, vs),
        );
        if lot.len() == INSERT_BATCH {
            insert_lot(pool, scale, std::mem::take(&mut lot)).await?;
        }
    }
    if lot.len() > 0 {
        insert_lot(pool, scale, lot).await?;
    }
    for sql in [
        vacuum_sql(scale),
        "VACUUM (ANALYZE) doc",
        "VACUUM (ANALYZE) doc_acl",
    ] {
        sqlx::raw_sql(sql)
            .execute(pool)
            .await
            .context("vacuum analyze")?;
    }
    Ok(())
}

// ---- SQL per scala: un letterale per tabella, scelto con `match` su enum chiuso ----------

/// Inserimento a lotti nella tabella della scala (`UNNEST` di array legati).
pub(crate) fn insert_sql(scale: Scale) -> &'static str {
    match scale {
        Scale::N10k => {
            "INSERT INTO chunk_synth_10000 (id, doc_id, char_start, char_end, lang, body, emb)
SELECT u.id, u.doc_id, 0, u.char_end, u.lang::regconfig, u.body, u.emb
FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[], $4::text[], $5::text[], $6::vector[])
  AS u (id, doc_id, char_end, lang, body, emb)"
        }
        Scale::N50k => {
            "INSERT INTO chunk_synth_50000 (id, doc_id, char_start, char_end, lang, body, emb)
SELECT u.id, u.doc_id, 0, u.char_end, u.lang::regconfig, u.body, u.emb
FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[], $4::text[], $5::text[], $6::vector[])
  AS u (id, doc_id, char_end, lang, body, emb)"
        }
        Scale::N100k => {
            "INSERT INTO chunk_synth_100000 (id, doc_id, char_start, char_end, lang, body, emb)
SELECT u.id, u.doc_id, 0, u.char_end, u.lang::regconfig, u.body, u.emb
FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[], $4::text[], $5::text[], $6::vector[])
  AS u (id, doc_id, char_end, lang, body, emb)"
        }
        Scale::N300k => {
            "INSERT INTO chunk_synth_300000 (id, doc_id, char_start, char_end, lang, body, emb)
SELECT u.id, u.doc_id, 0, u.char_end, u.lang::regconfig, u.body, u.emb
FROM UNNEST($1::uuid[], $2::uuid[], $3::int4[], $4::text[], $5::text[], $6::vector[])
  AS u (id, doc_id, char_end, lang, body, emb)"
        }
    }
}

/// Svuota la tabella della scala.
pub(crate) fn truncate_sql(scale: Scale) -> &'static str {
    match scale {
        Scale::N10k => "TRUNCATE chunk_synth_10000",
        Scale::N50k => "TRUNCATE chunk_synth_50000",
        Scale::N100k => "TRUNCATE chunk_synth_100000",
        Scale::N300k => "TRUNCATE chunk_synth_300000",
    }
}

/// Statistiche e visibility map dopo il caricamento.
pub(crate) fn vacuum_sql(scale: Scale) -> &'static str {
    match scale {
        Scale::N10k => "VACUUM (ANALYZE) chunk_synth_10000",
        Scale::N50k => "VACUUM (ANALYZE) chunk_synth_50000",
        Scale::N100k => "VACUUM (ANALYZE) chunk_synth_100000",
        Scale::N300k => "VACUUM (ANALYZE) chunk_synth_300000",
    }
}

/// Chunk dei documenti con `doc_key LIKE $1` (pulizia di ingest).
pub(crate) fn delete_prefix_chunks_sql(scale: Scale) -> &'static str {
    match scale {
        Scale::N10k => {
            "DELETE FROM chunk_synth_10000 WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)"
        }
        Scale::N50k => {
            "DELETE FROM chunk_synth_50000 WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)"
        }
        Scale::N100k => {
            "DELETE FROM chunk_synth_100000 WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)"
        }
        Scale::N300k => {
            "DELETE FROM chunk_synth_300000 WHERE doc_id IN (SELECT id FROM doc WHERE doc_key LIKE $1)"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testdb::{self, Scratch, axis, near, union};
    use s03_pgvector::db;

    // TEST-9 (RISK-4): tabelle sintetiche da enum chiuso.
    #[test]
    fn test9_scale_tables_come_from_closed_enum() {
        assert_eq!(Scale::N10k.table(), "chunk_synth_10000");
        assert_eq!(Scale::N50k.table(), "chunk_synth_50000");
        assert_eq!(Scale::N100k.table(), "chunk_synth_100000");
        assert_eq!(Scale::N300k.table(), "chunk_synth_300000");
        let ns: Vec<usize> = Scale::ALL.iter().map(|s| s.n()).collect();
        assert_eq!(ns, [10_000, 50_000, 100_000, 300_000]);
    }

    fn base_vectors() -> Vec<Vec<f32>> {
        (0..5).map(|i| near(i * 37)).collect()
    }

    // TEST-16 (RISK-10): stesso seme → stessi vettori bit a bit, norma 1, proporzioni.
    #[test]
    fn test16_synth_is_deterministic_and_normalized() {
        let base = base_vectors();
        let a = perturb(&base, 50, SEED);
        let b = perturb(&base, 50, SEED);
        assert_eq!(a.len(), 50);
        let bits = |v: &Vec<Vec<f32>>| -> Vec<Vec<u32>> {
            v.iter()
                .map(|x| x.iter().map(|f| f.to_bits()).collect())
                .collect()
        };
        assert_eq!(bits(&a), bits(&b), "stesso seme, vettori diversi");
        for (i, v) in a.iter().enumerate() {
            assert_eq!(v.len(), testdb::DIM);
            let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
            assert!((norm - 1.0).abs() <= 1e-5, "vettore {i}: norma {norm}");
        }
        let c = perturb(&base, 50, SEED + 1);
        assert_ne!(
            bits(&a),
            bits(&c),
            "il seme deve influire sulla perturbazione"
        );
        assert!(
            a.iter().any(|v| !base.contains(v)),
            "i vettori sintetici devono essere perturbati, non copie dei reali"
        );

        let mut real = Vec::new();
        for (c, k) in [
            ("public", 4),
            ("internal", 3),
            ("confidential", 2),
            ("restricted", 1),
        ] {
            real.extend(std::iter::repeat_n(c, k));
        }
        let cls = synth_classifications(&real, 100, SEED);
        assert_eq!(
            cls,
            synth_classifications(&real, 100, SEED),
            "non deterministico"
        );
        assert_eq!(cls.len(), 100);
        for (c, want) in [
            ("public", 40),
            ("internal", 30),
            ("confidential", 20),
            ("restricted", 10),
        ] {
            let got = cls.iter().filter(|x| *x == c).count() as i64;
            assert!(
                (got - want).abs() <= 1,
                "{c}: {got} documenti, attesi {want} ± 1"
            );
        }
    }

    /// Documenti reali di base nello schema di test: uno per classificazione, 5 chunk ciascuno.
    async fn seed_base(s: &Scratch) -> Vec<(String, String)> {
        let mut out = Vec::new();
        for (i, c) in ["public", "internal", "confidential", "restricted"]
            .iter()
            .enumerate()
        {
            let key = format!("base-{c}");
            let d = s.doc(&key, c).await;
            for j in 0..5 {
                s.chunk(d, "testo di base", axis(2 + i * 5 + j)).await;
            }
            out.push((key, c.to_string()));
        }
        out
    }

    // TEST-11 (RISK-6): quota di chunk visibili ai ruoli sintetici entro ±0,5 punti dal target.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose) e sql/index.sql"]
    async fn test11_synth_role_selectivity_matches_target() {
        let s = Scratch::new().await;
        s.apply_index_sql().await;
        seed_base(&s).await;
        generate(&s.pool, Scale::N10k, SEED, &testdb::acl_json())
            .await
            .expect("synth --n 10000");
        let total: i64 = sqlx::query_scalar("SELECT count(*) FROM chunk_synth_10000")
            .fetch_one(&s.pool)
            .await
            .expect("count");
        assert_eq!(total, 10_000);
        for (role, target) in SYNTH_ROLES {
            let share: f64 = sqlx::query_scalar(
                "SELECT (count(*) FILTER (WHERE EXISTS (SELECT 1 FROM doc_acl a \
                 WHERE a.doc_id = c.doc_id AND a.role = $1)))::float8 / count(*) \
                 FROM chunk_synth_10000 c",
            )
            .bind(role)
            .fetch_one(&s.pool)
            .await
            .expect("selettività");
            assert!(
                (share - target).abs() <= 0.005,
                "{role}: selettività {share:.4}, attesa {target} ± 0,005"
            );
        }
        s.drop().await;
    }

    // TEST-7 (RISK-3, RISK-6): dopo synth, doc_acl coincide esattamente con la matrice attesa
    // (acl.json + documenti di base per i ruoli reali, regola del seme per quelli sintetici).
    // I ruoli sintetici valgono solo sui documenti sintetici.
    #[tokio::test]
    #[ignore = "richiede DATABASE_URL (Postgres del compose) e sql/index.sql"]
    async fn test7_doc_acl_matches_expected_after_synth() {
        let s = Scratch::new().await;
        s.apply_index_sql().await;
        let base = seed_base(&s).await;
        let acl = testdb::acl_json();
        generate(&s.pool, Scale::N10k, SEED, &acl)
            .await
            .expect("synth");
        let base_ref: Vec<(&str, &str)> =
            base.iter().map(|(k, c)| (k.as_str(), c.as_str())).collect();
        let expected = union(
            testdb::visibility_for(&base_ref),
            expected_visibility(Scale::N10k, SEED, &acl, &base).expect("matrice sintetica"),
        );
        let actual = db::read_doc_acl(&s.pool).await.expect("doc_acl");
        let diff = db::acl_diff(&expected, &actual);
        assert!(
            diff.is_empty(),
            "doc_acl diversa dalla matrice attesa ({} differenze): {:?}",
            diff.len(),
            &diff[..diff.len().min(20)]
        );
        s.drop().await;
    }
}
