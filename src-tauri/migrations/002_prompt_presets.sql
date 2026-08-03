-- 002_prompt_presets.sql — 提示词方案（内置预设不进库，仅存用户自定义；激活选择存 settings）
-- 表固定：prompt_presets

CREATE TABLE IF NOT EXISTS prompt_presets (
    id            TEXT PRIMARY KEY,
    name          TEXT NOT NULL,
    system_prompt TEXT NOT NULL,
    user_prompt   TEXT NOT NULL,
    builtin       INTEGER NOT NULL DEFAULT 0,
    created_at_ms INTEGER NOT NULL
);
