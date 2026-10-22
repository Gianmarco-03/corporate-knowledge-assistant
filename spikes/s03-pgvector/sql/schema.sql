-- spikes/s03-pgvector/sql/schema.sql — schema minimo dello spike S-03/A-03 (piano 01-10).
-- Idempotente: lo applicano la CLI (`load`) e i test db in uno schema temporaneo.
-- Riusato senza modifiche dal piano dell'indice (01-14).
--
-- doc_acl si popola da acl.json + classificazione del manifest; prima di ogni misura la
-- CLI la confronta con la matrice attesa calcolata in memoria (metrics::expected_visibility).
-- Una tabella di chunk per modello candidato: stessa dimensione 1024, nomi fissi scelti in
-- db.rs da un enum chiuso.
--
-- Modifica: Claude (gsd-executor 01-10) · 2026-10-22 · schema doc, doc_acl, chunk_<modello>

CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS doc (
  id uuid PRIMARY KEY,
  doc_key text NOT NULL UNIQUE,
  classification text NOT NULL
);

CREATE TABLE IF NOT EXISTS doc_acl (
  doc_id uuid NOT NULL REFERENCES doc (id),
  role text NOT NULL,
  PRIMARY KEY (doc_id, role)
);
CREATE INDEX IF NOT EXISTS doc_acl_role_doc_idx ON doc_acl (role, doc_id);

CREATE TABLE IF NOT EXISTS chunk_qwen3 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL
);
CREATE INDEX IF NOT EXISTS chunk_qwen3_tsv_idx ON chunk_qwen3 USING gin (tsv);

CREATE TABLE IF NOT EXISTS chunk_bgem3 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL
);
CREATE INDEX IF NOT EXISTS chunk_bgem3_tsv_idx ON chunk_bgem3 USING gin (tsv);

CREATE TABLE IF NOT EXISTS chunk_e5 (
  id uuid PRIMARY KEY,
  doc_id uuid NOT NULL REFERENCES doc (id),
  page int,
  section_path text,
  char_start int,
  char_end int,
  lang regconfig NOT NULL,
  body text NOT NULL,
  tsv tsvector GENERATED ALWAYS AS (to_tsvector(lang, body)) STORED,
  emb vector(1024) NOT NULL
);
CREATE INDEX IF NOT EXISTS chunk_e5_tsv_idx ON chunk_e5 USING gin (tsv);
