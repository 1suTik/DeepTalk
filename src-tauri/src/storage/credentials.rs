//! API Key 凭据存储：仅写入 Windows Credential Manager（keyring），不落数据库。

use keyring::{Entry, Error as KeyringError};

pub const CREDENTIAL_SERVICE: &str = "MeetingAIAssistant";

/// account 引用形如 `api-key:<provider-kind>`，SQLite 只保存该引用。
pub fn credential_account(provider_kind: &str) -> String {
    format!("api-key:{provider_kind}")
}

#[derive(Debug, thiserror::Error)]
pub enum CredError {
    #[error("凭据存储错误：{0}")]
    Keyring(#[from] KeyringError),
}

pub struct CredentialStore;

impl CredentialStore {
    pub fn new() -> Self {
        Self
    }

    /// 保存或覆盖密钥。空字符串视为删除。
    pub fn set(&self, account: &str, secret: &str) -> Result<(), CredError> {
        let entry = Entry::new(CREDENTIAL_SERVICE, account)?;
        if secret.is_empty() {
            let _ = entry.delete_credential();
            return Ok(());
        }
        entry.set_password(secret)?;
        Ok(())
    }

    /// 读取密钥；不存在返回 None。
    pub fn get(&self, account: &str) -> Result<Option<String>, CredError> {
        let entry = Entry::new(CREDENTIAL_SERVICE, account)?;
        match entry.get_password() {
            Ok(p) => Ok(Some(p)),
            Err(KeyringError::NoEntry) => Ok(None),
            Err(e) => Err(CredError::Keyring(e)),
        }
    }

    pub fn delete(&self, account: &str) -> Result<(), CredError> {
        let entry = Entry::new(CREDENTIAL_SERVICE, account)?;
        let _ = entry.delete_credential();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credentials_roundtrip_and_delete() {
        let store = CredentialStore::new();
        let account = format!("api-key:test-{}", std::process::id());
        store.set(&account, "sk-test-secret").unwrap();
        assert_eq!(store.get(&account).unwrap().as_deref(), Some("sk-test-secret"));
        store.delete(&account).unwrap();
        assert_eq!(store.get(&account).unwrap(), None);
    }

    #[test]
    fn empty_secret_deletes_entry() {
        let store = CredentialStore::new();
        let account = format!("api-key:test-empty-{}", std::process::id());
        store.set(&account, "secret").unwrap();
        store.set(&account, "").unwrap();
        assert_eq!(store.get(&account).unwrap(), None);
    }

    #[test]
    fn missing_entry_returns_none() {
        let store = CredentialStore::new();
        assert_eq!(
            store
                .get(&format!("api-key:never-exists-{}", std::process::id()))
                .unwrap(),
            None
        );
    }
}
