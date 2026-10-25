-- spikes/s03-pgvector/sql/index.sql — indice vettoriale dello spike S-03/A-03 (piano 01-14).
--
-- Si applica DOPO sql/schema.sql del piano 01-10, che non si modifica. Un'istruzione per
-- volta, ciascuna chiusa da `;` a fine riga: `CREATE INDEX CONCURRENTLY` non può stare in
-- un blocco transazionale né in una query multi-istruzione.
--
-- Prima parte (sottocomando `prepare`, idempotente):
--   - colonna `emb_h halfvec(1024)` generata da `emb` sulla tabella del modello scelto
--     (bge-m3, RISULTATI-embedding.md §Esito) e sulle tabelle sintetiche;
--   - tabelle sintetiche `chunk_synth_<n>` con la stessa forma di `chunk_bgem3`, più
--     l'indice su `doc_id` (chiave esterna) e il GIN su `tsv`.
-- I ruoli sintetici `sel100`, `sel25`, `sel5`, `sel1` non hanno DDL: sono righe di
-- `doc_acl` scritte da `index-bench synth`, solo sui documenti sintetici.
--
-- Seconda parte (dopo la riga `-- HNSW`): gli indici HNSW (m = 16, ef_construction = 64)
-- delle tabelle sintetiche. Non li applica `prepare`: li costruisce `index-bench hnsw`,
-- un'istruzione alla volta, misurandone tempo e dimensione, e li rimuove `drop-hnsw`.
-- Nessun indice HNSW sulla tabella reale `chunk_bgem3`: la ricerca esatta del piano 01-10
-- (export-candidates del piano 01-15) resta esatta.
--
-- Modifica: Claude (gsd-executor 01-14) · 2026-10-25 · colonna halfvec, tabelle sintetiche e indici HNSW

ALTER TABLE chunk_bgem3 ADD COLUMN IF NOT EXISTS emb_h halfvec(1024)
  GENERATED ALWAYS AS (emb::halfvec(1024)) STORED;

CREATE TABLE IF NOT EXISTS chunk_synth_10000 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL,
  emb_h halfvec(1024) GENERATED ALWAYS AS (emb::halfvec(1024)) STORED
);
CREATE INDEX IF NOT EXISTS chunk_synth_10000_tsv_idx ON chunk_synth_10000 USING gin (tsv);
CREATE INDEX IF NOT EXISTS chunk_synth_10000_doc_idx ON chunk_synth_10000 (doc_id);

CREATE TABLE IF NOT EXISTS chunk_synth_50000 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL,
  emb_h halfvec(1024) GENERATED ALWAYS AS (emb::halfvec(1024)) STORED
);
CREATE INDEX IF NOT EXISTS chunk_synth_50000_tsv_idx ON chunk_synth_50000 USING gin (tsv);
CREATE INDEX IF NOT EXISTS chunk_synth_50000_doc_idx ON chunk_synth_50000 (doc_id);

CREATE TABLE IF NOT EXISTS chunk_synth_100000 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL,
  emb_h halfvec(1024) GENERATED ALWAYS AS (emb::halfvec(1024)) STORED
);
CREATE INDEX IF NOT EXISTS chunk_synth_100000_tsv_idx ON chunk_synth_100000 USING gin (tsv);
CREATE INDEX IF NOT EXISTS chunk_synth_100000_doc_idx ON chunk_synth_100000 (doc_id);

CREATE TABLE IF NOT EXISTS chunk_synth_300000 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL,
  emb_h halfvec(1024) GENERATED ALWAYS AS (emb::halfvec(1024)) STORED
);
CREATE INDEX IF NOT EXISTS chunk_synth_300000_tsv_idx ON chunk_synth_300000 USING gin (tsv);
CREATE INDEX IF NOT EXISTS chunk_synth_300000_doc_idx ON chunk_synth_300000 (doc_id);

-- HNSW
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_10000_emb_hnsw ON chunk_synth_10000
  USING hnsw (emb vector_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_10000_embh_hnsw ON chunk_synth_10000
  USING hnsw (emb_h halfvec_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_50000_emb_hnsw ON chunk_synth_50000
  USING hnsw (emb vector_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_50000_embh_hnsw ON chunk_synth_50000
  USING hnsw (emb_h halfvec_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_100000_emb_hnsw ON chunk_synth_100000
  USING hnsw (emb vector_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_100000_embh_hnsw ON chunk_synth_100000
  USING hnsw (emb_h halfvec_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_300000_emb_hnsw ON chunk_synth_300000
  USING hnsw (emb vector_cosine_ops) WITH (m = 16, ef_construction = 64);
CREATE INDEX CONCURRENTLY IF NOT EXISTS chunk_synth_300000_embh_hnsw ON chunk_synth_300000
  USING hnsw (emb_h halfvec_cosine_ops) WITH (m = 16, ef_construction = 64);
