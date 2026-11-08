-- GRAFT schema 005
-- Extract Studio audit trail and project relationship graph. Rows are written
-- only after a validated extraction, so the graph always reflects real results.

CREATE TABLE IF NOT EXISTS extraction_runs (
    id          INTEGER PRIMARY KEY,
    source      TEXT NOT NULL,
    target      TEXT NOT NULL,
    name        TEXT NOT NULL,
    strategy    TEXT NOT NULL,
    file_count  INTEGER NOT NULL,
    total_bytes INTEGER NOT NULL,
    backup_path TEXT,
    status      TEXT NOT NULL,
    message     TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS extraction_runs_recent_idx
    ON extraction_runs (created_at DESC, id DESC);
