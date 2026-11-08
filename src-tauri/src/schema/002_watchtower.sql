-- GRAFT schema 002
-- Watchtower keeps favourites and observed events locally. GitHub remains the
-- source of truth for repositories and event payloads.

CREATE TABLE IF NOT EXISTS watched_repos (
    id             INTEGER PRIMARY KEY AUTOINCREMENT,
    owner          TEXT NOT NULL,
    name           TEXT NOT NULL,
    full_name      TEXT NOT NULL UNIQUE,
    html_url       TEXT NOT NULL DEFAULT '',
    description    TEXT,
    language       TEXT,
    private        INTEGER NOT NULL DEFAULT 0,
    etag           TEXT,
    actions_etag   TEXT,
    initialized    INTEGER NOT NULL DEFAULT 0,
    last_polled_at INTEGER,
    last_error     TEXT,
    created_at     INTEGER NOT NULL,
    updated_at     INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS watch_events (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    repo_id     INTEGER NOT NULL REFERENCES watched_repos(id) ON DELETE CASCADE,
    event_key   TEXT NOT NULL,
    category    TEXT NOT NULL,
    kind        TEXT NOT NULL,
    action      TEXT NOT NULL DEFAULT '',
    title       TEXT NOT NULL,
    detail      TEXT NOT NULL DEFAULT '',
    actor       TEXT NOT NULL DEFAULT '',
    url         TEXT NOT NULL DEFAULT '',
    occurred_at TEXT NOT NULL DEFAULT '',
    seen        INTEGER NOT NULL DEFAULT 0,
    created_at  INTEGER NOT NULL,
    UNIQUE(repo_id, event_key)
);

CREATE INDEX IF NOT EXISTS watch_events_repo_idx ON watch_events (repo_id, id DESC);
CREATE INDEX IF NOT EXISTS watch_events_unread_idx ON watch_events (seen, id DESC);
