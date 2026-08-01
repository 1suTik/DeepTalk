-- 001_initial.sql — 会议历史、转写、问题、答案、资料与设置
-- 固定表：meetings / transcript_segments / questions / answers / profile_documents / settings

PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS settings (
    key   TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS meetings (
    id            TEXT PRIMARY KEY,
    started_at_ms INTEGER NOT NULL,
    ended_at_ms   INTEGER,
    pinned        INTEGER NOT NULL DEFAULT 0
);

CREATE TABLE IF NOT EXISTS transcript_segments (
    id            TEXT PRIMARY KEY,
    meeting_id    TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    speaker       TEXT NOT NULL,
    text          TEXT NOT NULL,
    started_at_ms INTEGER NOT NULL,
    ended_at_ms   INTEGER NOT NULL,
    is_final      INTEGER NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS questions (
    id             TEXT PRIMARY KEY,
    meeting_id     TEXT NOT NULL REFERENCES meetings(id) ON DELETE CASCADE,
    text           TEXT NOT NULL,
    confidence     REAL NOT NULL,
    detected_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS answers (
    id            TEXT PRIMARY KEY,
    question_id   TEXT NOT NULL REFERENCES questions(id) ON DELETE CASCADE,
    short_answer  TEXT NOT NULL,
    key_points    TEXT NOT NULL DEFAULT '[]',
    follow_ups    TEXT NOT NULL DEFAULT '[]',
    status        TEXT NOT NULL DEFAULT 'complete',
    created_at_ms INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS profile_documents (
    id             TEXT PRIMARY KEY,
    title          TEXT NOT NULL,
    original_path  TEXT NOT NULL UNIQUE,
    extracted_text TEXT NOT NULL,
    enabled        INTEGER NOT NULL DEFAULT 1,
    imported_at_ms INTEGER NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_transcript_meeting ON transcript_segments(meeting_id);
CREATE INDEX IF NOT EXISTS idx_questions_meeting ON questions(meeting_id);
CREATE INDEX IF NOT EXISTS idx_answers_question ON answers(question_id);
