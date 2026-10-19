-- Schema dello spike S-01 (piano 01-04). Idempotente: lo applica gc::apply_schema.
-- Un solo database dello spike; nel prodotto ogni servizio ha schema e credenziali propri.

-- Metadati dell'upload: una riga per richiesta riuscita (upload_id nuovo anche per i
-- duplicati, D-02). Il nome file del client non è memorizzato (T-08).
CREATE TABLE IF NOT EXISTS upload (
    id         uuid PRIMARY KEY,
    sha256     text NOT NULL,
    size_bytes bigint,
    status     text,
    created_at timestamptz
);
CREATE INDEX IF NOT EXISTS upload_sha256_idx ON upload (sha256);

-- Transactional outbox: event_id univoco per i consumer idempotenti.
CREATE TABLE IF NOT EXISTS outbox (
    event_id   uuid UNIQUE NOT NULL,
    payload    jsonb,
    created_at timestamptz
);

-- Protocollo a stati per hash fra upload e GC (RISK-004):
--   live     = l'oggetto può essere adottato; pending = adozioni A in corso;
--   deleting = il GC ha deciso di cancellarlo; nessuna adozione finché la riga esiste.
-- Il lock di riga serializza la marcatura del GC e l'adozione A.
CREATE TABLE IF NOT EXISTS blob (
    sha256     text PRIMARY KEY,
    state      text NOT NULL CHECK (state IN ('live', 'deleting')),
    pending    int NOT NULL DEFAULT 0 CHECK (pending >= 0),
    updated_at timestamptz NOT NULL DEFAULT now()
);

-- Baseline PG bytea (D-06): chunk da 1 MiB, niente compressione TOAST su file già
-- compressi.
CREATE TABLE IF NOT EXISTS blob_chunk (
    object_key text NOT NULL,
    seq        int NOT NULL,
    data       bytea NOT NULL,
    PRIMARY KEY (object_key, seq)
);
ALTER TABLE blob_chunk ALTER COLUMN data SET STORAGE EXTERNAL;
