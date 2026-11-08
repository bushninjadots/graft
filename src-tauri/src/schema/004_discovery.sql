-- GRAFT schema 004
-- Local repository visit history. Watchtower uses this to surface frequently
-- visited repositories and to derive similar-repository recommendations.

CREATE TABLE IF NOT EXISTS repo_visits (
    full_name       TEXT PRIMARY KEY,
    owner           TEXT NOT NULL,
    name            TEXT NOT NULL,
    html_url        TEXT NOT NULL DEFAULT '',
    description     TEXT,
    language        TEXT,
    topics          TEXT NOT NULL DEFAULT '[]',
    stars           INTEGER NOT NULL DEFAULT 0,
    forks           INTEGER NOT NULL DEFAULT 0,
    default_branch  TEXT NOT NULL DEFAULT 'main',
    private         INTEGER NOT NULL DEFAULT 0,
    visit_count     INTEGER NOT NULL DEFAULT 1,
    last_visited_at INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS repo_visits_frequency_idx
    ON repo_visits (visit_count DESC, last_visited_at DESC);
