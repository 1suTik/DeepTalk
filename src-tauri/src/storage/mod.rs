//! 本地存储：SQLite 历史、Windows Credential Manager 凭据与 7 天保留策略。

pub mod credentials;
pub mod database;
pub mod retention;

pub use credentials::{credential_account, CredentialStore};
pub use database::{
    AnswerRow, Db, DbError, ProfileDocRow, PromptPresetRow, QuestionRow, TranscriptRow,
};
pub use retention::Retention;
