//! SQLite 数据库：迁移、事务与 repository。
//!
//! 只保存文本与元数据；API Key 不落库（见 `credentials.rs`）。
//! 表固定为：meetings / transcript_segments / questions / answers / profile_documents / settings。

use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::json;

const MIGRATIONS: &[&str] = &[include_str!("../../migrations/001_initial.sql")];

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("数据库错误：{0}")]
    Sql(#[from] rusqlite::Error),
    #[error("数据库路径无效：{0}")]
    Path(String),
}

#[derive(Debug, Clone)]
pub struct TranscriptRow {
    pub id: String,
    pub speaker: String,
    pub text: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub struct QuestionRow {
    pub id: String,
    pub text: String,
    pub confidence: f64,
    pub detected_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct AnswerRow {
    pub id: String,
    pub question_id: String,
    pub short_answer: String,
    pub key_points: Vec<String>,
    pub follow_ups: Vec<String>,
    pub status: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ProfileDocRow {
    pub id: String,
    pub title: String,
    pub original_path: String,
    pub enabled: bool,
    pub imported_at_ms: u64,
}

/// 线程安全的数据库句柄（rusqlite Connection 非 Send，以 Arc<Mutex> 包装，可 Clone 共享）。
#[derive(Debug, Clone)]
pub struct Db {
    inner: Arc<Mutex<Connection>>,
}

impl Db {
    pub fn open(path: &Path) -> Result<Self, DbError> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| DbError::Path(e.to_string()))?;
        }
        Self::init(Connection::open(path)?)
    }

    pub fn open_in_memory() -> Result<Self, DbError> {
        Self::init(Connection::open_in_memory()?)
    }

    fn init(conn: Connection) -> Result<Self, DbError> {
        conn.pragma_update(None, "journal_mode", "WAL").ok();
        conn.pragma_update(None, "foreign_keys", "ON")?;
        for sql in MIGRATIONS {
            conn.execute_batch(sql)?;
        }
        Ok(Self {
            inner: Arc::new(Mutex::new(conn)),
        })
    }

    // ---- settings ---------------------------------------------------------

    pub fn get_setting(&self, key: &str) -> Result<Option<String>, DbError> {
        let conn = self.inner.lock().unwrap();
        Ok(conn
            .query_row(
                "SELECT value FROM settings WHERE key = ?1",
                params![key],
                |r| r.get(0),
            )
            .optional()?)
    }

    pub fn set_setting(&self, key: &str, value: &str) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT(key) DO UPDATE SET value = excluded.value",
            params![key, value],
        )?;
        Ok(())
    }

    pub fn all_settings(&self) -> Result<Vec<(String, String)>, DbError> {
        let conn = self.inner.lock().unwrap();
        let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
        let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- meetings ---------------------------------------------------------

    pub fn create_meeting(&self, id: &str, started_at_ms: u64) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO meetings (id, started_at_ms) VALUES (?1, ?2)",
            params![id, started_at_ms as i64],
        )?;
        Ok(())
    }

    pub fn end_meeting(&self, id: &str, ended_at_ms: u64) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "UPDATE meetings SET ended_at_ms = ?2 WHERE id = ?1",
            params![id, ended_at_ms as i64],
        )?;
        Ok(())
    }

    pub fn pin_meeting(&self, id: &str, pinned: bool) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "UPDATE meetings SET pinned = ?2 WHERE id = ?1",
            params![id, pinned as i64],
        )?;
        Ok(())
    }

    // ---- transcript / questions / answers ----------------------------------

    pub fn insert_transcript_segment(
        &self,
        meeting_id: &str,
        seg: &TranscriptRow,
    ) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO transcript_segments
             (id, meeting_id, speaker, text, started_at_ms, ended_at_ms, is_final)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                seg.id,
                meeting_id,
                seg.speaker,
                seg.text,
                seg.started_at_ms as i64,
                seg.ended_at_ms as i64,
                seg.is_final as i64,
            ],
        )?;
        Ok(())
    }

    /// 事务内批量插入转写段；任一条失败则全部回滚。
    pub fn insert_transcript_segments(
        &self,
        meeting_id: &str,
        segs: &[TranscriptRow],
    ) -> Result<(), DbError> {
        let mut conn = self.inner.lock().unwrap();
        let tx = conn.transaction()?;
        for seg in segs {
            tx.execute(
                "INSERT INTO transcript_segments
                 (id, meeting_id, speaker, text, started_at_ms, ended_at_ms, is_final)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    seg.id,
                    meeting_id,
                    seg.speaker,
                    seg.text,
                    seg.started_at_ms as i64,
                    seg.ended_at_ms as i64,
                    seg.is_final as i64,
                ],
            )?;
        }
        tx.commit()?;
        Ok(())
    }

    pub fn insert_question(
        &self,
        meeting_id: &str,
        q: &QuestionRow,
    ) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO questions (id, meeting_id, text, confidence, detected_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![q.id, meeting_id, q.text, q.confidence, q.detected_at_ms as i64],
        )?;
        Ok(())
    }

    pub fn insert_answer(&self, a: &AnswerRow) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO answers (id, question_id, short_answer, key_points, follow_ups, status, created_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                a.id,
                a.question_id,
                a.short_answer,
                json!(a.key_points).to_string(),
                json!(a.follow_ups).to_string(),
                a.status,
                a.created_at_ms as i64,
            ],
        )?;
        Ok(())
    }

    pub fn insert_profile_document(
        &self,
        id: &str,
        title: &str,
        original_path: &str,
        extracted_text: &str,
        enabled: bool,
        imported_at_ms: u64,
    ) -> Result<(), DbError> {
        let conn = self.inner.lock().unwrap();
        conn.execute(
            "INSERT INTO profile_documents (id, title, original_path, extracted_text, enabled, imported_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![id, title, original_path, extracted_text, enabled as i64, imported_at_ms as i64],
        )?;
        Ok(())
    }

    pub fn profile_documents(&self) -> Result<Vec<ProfileDocRow>, DbError> {
        let conn = self.inner.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, original_path, enabled, imported_at_ms
             FROM profile_documents ORDER BY imported_at_ms",
        )?;
        let rows = stmt.query_map([], |r| {
            Ok(ProfileDocRow {
                id: r.get(0)?,
                title: r.get(1)?,
                original_path: r.get(2)?,
                enabled: r.get::<_, i64>(3)? != 0,
                imported_at_ms: r.get::<_, i64>(4)? as u64,
            })
        })?;
        Ok(rows.collect::<Result<Vec<_>, _>>()?)
    }

    // ---- 计数（测试辅助） ---------------------------------------------------

    pub fn count_meetings(&self) -> Result<i64, DbError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM meetings", [], |r| r.get(0))?)
    }

    pub fn count_segments(&self) -> Result<i64, DbError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM transcript_segments", [], |r| r.get(0))?)
    }

    pub fn count_questions(&self) -> Result<i64, DbError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM questions", [], |r| r.get(0))?)
    }

    pub fn count_answers(&self) -> Result<i64, DbError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM answers", [], |r| r.get(0))?)
    }

    pub fn count_profile_documents(&self) -> Result<i64, DbError> {
        Ok(self
            .inner
            .lock()
            .unwrap()
            .query_row("SELECT COUNT(*) FROM profile_documents", [], |r| r.get(0))?)
    }

    // ---- 保留策略 -----------------------------------------------------------

    /// 删除超过保留天数、已结束且未固定的会议（级联删除关联转写/问题/答案），
    /// 单个事务提交；返回删除的会议数。
    pub fn purge_expired(&self, retention_days: u64, now_ms: u64) -> Result<usize, DbError> {
        let cutoff = now_ms.saturating_sub(retention_days * 24 * 3600 * 1000) as i64;
        let mut conn = self.inner.lock().unwrap();
        let tx = conn.transaction()?;
        let deleted = tx.execute(
            "DELETE FROM meetings
             WHERE pinned = 0 AND ended_at_ms IS NOT NULL AND ended_at_ms < ?1",
            params![cutoff],
        )?;
        tx.commit()?;
        Ok(deleted)
    }

    /// 清除全部数据：会议、转写、问题、答案与资料（设置与凭据保留）。
    pub fn purge_all(&self) -> Result<(), DbError> {
        let mut conn = self.inner.lock().unwrap();
        let tx = conn.transaction()?;
        for table in [
            "answers",
            "questions",
            "transcript_segments",
            "meetings",
            "profile_documents",
        ] {
            tx.execute(&format!("DELETE FROM {table}"), [])?;
        }
        tx.commit()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY_MS: u64 = 24 * 3600 * 1000;
    const NOW: u64 = 1_800_000_000_000;

    fn mem_db() -> Db {
        Db::open_in_memory().unwrap()
    }

    fn seed_meeting(db: &Db, id: &str, ended_ago_ms: u64, pinned: bool) {
        let started = NOW - ended_ago_ms - 60_000;
        db.create_meeting(id, started).unwrap();
        db.end_meeting(id, NOW - ended_ago_ms).unwrap();
        db.pin_meeting(id, pinned).unwrap();
        db.insert_transcript_segment(
            id,
            &TranscriptRow {
                id: format!("seg-{id}"),
                speaker: "remote".into(),
                text: "转写内容".into(),
                started_at_ms: started,
                ended_at_ms: started + 1000,
                is_final: true,
            },
        )
        .unwrap();
        db.insert_question(
            id,
            &QuestionRow {
                id: format!("q-{id}"),
                text: "请介绍项目".into(),
                confidence: 0.9,
                detected_at_ms: started + 500,
            },
        )
        .unwrap();
        db.insert_answer(&AnswerRow {
            id: format!("a-{id}"),
            question_id: format!("q-{id}"),
            short_answer: "短答".into(),
            key_points: vec!["要点一".into()],
            follow_ups: vec!["追问一".into()],
            status: "complete".into(),
            created_at_ms: started + 600,
        })
        .unwrap();
    }

    #[test]
    fn migration_creates_all_tables() {
        let db = mem_db();
        let conn = db.inner.lock().unwrap();
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .unwrap();
        let tables: Vec<String> = stmt
            .query_map([], |r| r.get(0))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap();
        for expected in [
            "answers",
            "meetings",
            "profile_documents",
            "questions",
            "settings",
            "transcript_segments",
        ] {
            assert!(tables.contains(&expected.to_string()), "缺少表 {expected}: {tables:?}");
        }
    }

    #[test]
    fn transaction_rolls_back_on_failure() {
        let db = mem_db();
        db.create_meeting("m1", NOW).unwrap();
        let segs = vec![
            TranscriptRow {
                id: "seg-1".into(),
                speaker: "remote".into(),
                text: "第一条".into(),
                started_at_ms: NOW,
                ended_at_ms: NOW + 100,
                is_final: true,
            },
            TranscriptRow {
                id: "seg-1".into(), // 重复主键 -> 失败
                speaker: "remote".into(),
                text: "第二条".into(),
                started_at_ms: NOW + 200,
                ended_at_ms: NOW + 300,
                is_final: true,
            },
        ];
        assert!(db.insert_transcript_segments("m1", &segs).is_err());
        assert_eq!(db.count_segments().unwrap(), 0, "事务必须整体回滚");
    }

    #[test]
    fn purge_expired_deletes_old_unpinned_with_cascade() {
        let db = mem_db();
        seed_meeting(&db, "old", 8 * DAY_MS, false); // 8 天前，未固定 -> 删除
        seed_meeting(&db, "pinned", 8 * DAY_MS, true); // 8 天前，固定 -> 保留
        seed_meeting(&db, "recent", 3 * DAY_MS, false); // 3 天前 -> 保留

        let deleted = db.purge_expired(7, NOW).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.count_meetings().unwrap(), 2);
        assert_eq!(db.count_segments().unwrap(), 2, "级联删除失败");
        assert_eq!(db.count_questions().unwrap(), 2);
        assert_eq!(db.count_answers().unwrap(), 2);
    }

    #[test]
    fn purge_respects_custom_retention_days() {
        let db = mem_db();
        seed_meeting(&db, "two-days", 2 * DAY_MS, false);
        let deleted = db.purge_expired(1, NOW).unwrap();
        assert_eq!(deleted, 1);
        assert_eq!(db.count_meetings().unwrap(), 0);
    }

    #[test]
    fn purge_all_clears_data_tables() {
        let db = mem_db();
        seed_meeting(&db, "m1", 1, false);
        db.set_setting("provider.kind", "deepseek").unwrap();
        db.insert_profile_document("p1", "简历", "C:\\resume.pdf", "正文", true, NOW)
            .unwrap();
        db.purge_all().unwrap();
        assert_eq!(db.count_meetings().unwrap(), 0);
        assert_eq!(db.count_segments().unwrap(), 0);
        assert_eq!(db.count_questions().unwrap(), 0);
        assert_eq!(db.count_answers().unwrap(), 0);
        assert_eq!(db.count_profile_documents().unwrap(), 0);
        assert_eq!(db.get_setting("provider.kind").unwrap().as_deref(), Some("deepseek"), "设置应保留");
    }

    #[test]
    fn settings_roundtrip_and_upsert() {
        let db = mem_db();
        db.set_setting("retention.days", "7").unwrap();
        assert_eq!(db.get_setting("retention.days").unwrap().as_deref(), Some("7"));
        assert_eq!(db.get_setting("missing").unwrap(), None);
        db.set_setting("retention.days", "14").unwrap(); // upsert
        assert_eq!(db.get_setting("retention.days").unwrap().as_deref(), Some("14"));
        let all = db.all_settings().unwrap();
        assert_eq!(all, vec![("retention.days".to_string(), "14".to_string())]);
    }

    #[test]
    fn meeting_pin_and_end_roundtrip() {
        let db = mem_db();
        db.create_meeting("m1", NOW).unwrap();
        db.end_meeting("m1", NOW + 5000).unwrap();
        db.pin_meeting("m1", true).unwrap();
        db.pin_meeting("m1", false).unwrap();
        assert_eq!(db.count_meetings().unwrap(), 1);
    }
}
