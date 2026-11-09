-- GRAFT schema 007
-- Signed declarative plugins. Public verification material only; no secrets.

CREATE TABLE IF NOT EXISTS plugins (
    id           TEXT PRIMARY KEY,
    name         TEXT NOT NULL,
    version      TEXT NOT NULL,
    kind         TEXT NOT NULL,
    summary      TEXT NOT NULL DEFAULT '',
    author       TEXT NOT NULL DEFAULT '',
    manifest     TEXT NOT NULL,
    signature    TEXT NOT NULL,
    public_key   TEXT NOT NULL,
    fingerprint  TEXT NOT NULL,
    enabled      INTEGER NOT NULL DEFAULT 1,
    installed_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS plugin_keys (
    fingerprint TEXT PRIMARY KEY,
    public_key  TEXT NOT NULL,
    label       TEXT NOT NULL DEFAULT '',
    added_at    INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS plugins_kind_idx ON plugins(kind);
