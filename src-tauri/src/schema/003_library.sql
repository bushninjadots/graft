-- GRAFT schema 003
-- Standalone Explore saved searches and the local Collections library.

CREATE TABLE IF NOT EXISTS saved_searches (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    query      TEXT NOT NULL DEFAULT '',
    language   TEXT NOT NULL DEFAULT '',
    topic      TEXT NOT NULL DEFAULT '',
    author     TEXT NOT NULL DEFAULT '',
    sort       TEXT NOT NULL DEFAULT 'best',
    created_at INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS collections (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    name        TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_repos (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    collection_id  INTEGER NOT NULL REFERENCES collections(id) ON DELETE CASCADE,
    owner          TEXT NOT NULL,
    name           TEXT NOT NULL,
    full_name      TEXT NOT NULL,
    html_url       TEXT NOT NULL DEFAULT '',
    description    TEXT,
    language       TEXT,
    private        INTEGER NOT NULL DEFAULT 0,
    topics         TEXT NOT NULL DEFAULT '[]',
    stars          INTEGER NOT NULL DEFAULT 0,
    forks          INTEGER NOT NULL DEFAULT 0,
    default_branch TEXT NOT NULL DEFAULT 'main',
    status         TEXT NOT NULL DEFAULT 'want_to_try',
    notes          TEXT NOT NULL DEFAULT '',
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL,
    UNIQUE(collection_id, full_name)
);

CREATE INDEX IF NOT EXISTS collection_repos_collection_idx
    ON collection_repos (collection_id, full_name COLLATE NOCASE);
