-- GRAFT schema 006
-- Local AI conversations. Provider credentials never live in SQLite.

CREATE TABLE IF NOT EXISTS ai_conversations (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    title       TEXT NOT NULL DEFAULT '',
    provider    TEXT NOT NULL,
    model       TEXT NOT NULL,
    endpoint    TEXT NOT NULL DEFAULT '',
    created_at  INTEGER NOT NULL,
    updated_at  INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS ai_conversations_updated_idx
    ON ai_conversations (updated_at DESC);

CREATE TABLE IF NOT EXISTS ai_messages (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    conversation_id INTEGER NOT NULL REFERENCES ai_conversations(id) ON DELETE CASCADE,
    role            TEXT NOT NULL,
    content         TEXT NOT NULL,
    context         TEXT NOT NULL DEFAULT '[]',
    created_at      INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS ai_messages_conversation_idx
    ON ai_messages (conversation_id, id);
