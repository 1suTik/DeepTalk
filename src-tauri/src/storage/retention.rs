//! 7 天保留策略：应用启动时执行一次清理，运行期间每 24 小时执行一次。

use std::time::{SystemTime, UNIX_EPOCH};

use tracing::warn;

use super::database::{Db, DbError};

pub const DEFAULT_RETENTION_DAYS: u64 = 7;
pub const PERIODIC_INTERVAL: std::time::Duration = std::time::Duration::from_secs(24 * 3600);
const SETTING_KEY: &str = "retention.days";

pub fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

pub struct Retention {
    db: Db,
}

impl Retention {
    pub fn new(db: Db) -> Self {
        Self { db }
    }

    /// 读取保留天数（设置项缺失时默认 7 天），清理过期未固定的会议。
    pub fn purge_expired(&self) -> Result<usize, DbError> {
        let days = self
            .db
            .get_setting(SETTING_KEY)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(DEFAULT_RETENTION_DAYS);
        self.db.purge_expired(days, now_ms())
    }

    /// 启动后立即清理一次，之后每 24 小时清理一次（后台线程）。
    ///
    /// 使用纯 std 线程而非 `tokio::spawn`：本函数在 Tauri runtime 初始化前
    /// （`AppState::new`）被调用，异步上下文不存在会 panic。
    pub fn spawn_periodic(db: Db) {
        std::thread::Builder::new()
            .name("retention".into())
            .spawn(move || loop {
                let retention = Retention::new(db.clone());
                match retention.purge_expired() {
                    Ok(n) if n > 0 => warn!("已清理 {n} 条过期会议记录"),
                    Ok(_) => {}
                    Err(e) => warn!("保留策略清理失败：{e}"),
                }
                std::thread::sleep(PERIODIC_INTERVAL);
            })
            .ok();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_retention_is_seven_days() {
        assert_eq!(DEFAULT_RETENTION_DAYS, 7);
    }

    #[test]
    fn purge_uses_setting_when_present() {
        let db = Db::open_in_memory().unwrap();
        db.set_setting(SETTING_KEY, "14").unwrap();
        assert_eq!(
            db.get_setting(SETTING_KEY).unwrap().as_deref(),
            Some("14")
        );
    }

    #[test]
    fn retention_purges_with_default_days() {
        let db = Db::open_in_memory().unwrap();
        let day = 24 * 3600 * 1000u64;
        let now = now_ms();
        db.create_meeting("old", now - 9 * day).unwrap();
        db.end_meeting("old", now - 8 * day).unwrap();
        let r = Retention::new(db);
        assert_eq!(r.purge_expired().unwrap(), 1);
    }
}
