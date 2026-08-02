//! 资料导入管理：原文件路径与提取文本存本机，不上传原文件。

use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::profile::extractor::{self, ExtractedDocument, MAX_DOCS};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImportedProfile {
    pub id: String,
    pub title: String,
    pub original_path: String,
    pub text: String,
    pub imported_at_ms: u64,
    pub enabled: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("extract error: {0}")]
    Extract(#[from] extractor::ExtractError),
    #[error("too many documents (max {MAX_DOCS})")]
    TooManyDocuments,
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

const PROFILES_FILE: &str = "profiles.json";

pub fn default_profiles_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base)
        .join("MeetingAIAssistant")
        .join("profiles")
}

pub struct ProfileImporter {
    storage_dir: PathBuf,
}

impl ProfileImporter {
    pub fn new(storage_dir: PathBuf) -> Result<Self, ImportError> {
        fs::create_dir_all(&storage_dir)?;
        Ok(Self { storage_dir })
    }

    pub fn list(&self) -> Result<Vec<ImportedProfile>, ImportError> {
        let path = self.storage_dir.join(PROFILES_FILE);
        if !path.exists() {
            return Ok(Vec::new());
        }
        Ok(serde_json::from_str(&fs::read_to_string(path)?)?)
    }

    /// 导入文档：解析后与提取文本一起存入本地，返回登记记录。
    /// 同一原文件路径重复导入时直接返回已有记录。
    pub fn import(&mut self, path: &Path) -> Result<ImportedProfile, ImportError> {
        let mut profiles = self.list()?;
        let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
        let canonical_str = canonical.display().to_string();
        if let Some(existing) = profiles.iter().find(|p| p.original_path == canonical_str) {
            return Ok(existing.clone());
        }
        if profiles.len() >= MAX_DOCS {
            return Err(ImportError::TooManyDocuments);
        }
        let doc: ExtractedDocument = extractor::extract(path)?;
        let imported = ImportedProfile {
            id: format!("profile-{}", now_ms()),
            title: doc.title,
            original_path: canonical_str,
            text: doc.text,
            imported_at_ms: now_ms(),
            enabled: true,
        };
        profiles.push(imported.clone());
        self.save_all(&profiles)?;
        Ok(imported)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), ImportError> {
        let mut profiles = self.list()?;
        profiles.retain(|p| p.id != id);
        self.save_all(&profiles)
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool) -> Result<(), ImportError> {
        let mut profiles = self.list()?;
        if let Some(p) = profiles.iter_mut().find(|p| p.id == id) {
            p.enabled = enabled;
        }
        self.save_all(&profiles)
    }

    fn save_all(&self, profiles: &[ImportedProfile]) -> Result<(), ImportError> {
        let path = self.storage_dir.join(PROFILES_FILE);
        let tmp = self.storage_dir.join(format!("{PROFILES_FILE}.tmp"));
        fs::write(&tmp, serde_json::to_string_pretty(profiles)?)?;
        match fs::rename(&tmp, &path) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = fs::remove_file(&tmp);
                Err(ImportError::Io(std::io::Error::other("rename failed")))
            }
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("maa-imp-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn sample_md() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/documents")
            .join("sample.md")
    }

    #[test]
    fn import_stores_and_lists() {
        let dir = temp_dir("basic");
        let mut imp = ProfileImporter::new(dir.join("store")).unwrap();
        let p = imp.import(&sample_md()).unwrap();
        assert_eq!(p.title, "会议助手项目简介");
        assert!(p.text.contains("WASAPI"));
        assert!(p.enabled);
        assert_eq!(imp.list().unwrap().len(), 1);
        // 新实例可读回
        let imp2 = ProfileImporter::new(dir.join("store")).unwrap();
        assert_eq!(imp2.list().unwrap().len(), 1);
    }

    #[test]
    fn import_removes_and_toggles() {
        let dir = temp_dir("toggle");
        let mut imp = ProfileImporter::new(dir.join("store")).unwrap();
        let id = imp.import(&sample_md()).unwrap().id;
        imp.set_enabled(&id, false).unwrap();
        assert!(!imp.list().unwrap()[0].enabled);
        imp.remove(&id).unwrap();
        assert!(imp.list().unwrap().is_empty());
    }

    #[test]
    fn max_docs_is_enforced() {
        let dir = temp_dir("maxdocs");
        let mut imp = ProfileImporter::new(dir.join("store")).unwrap();
        let src = sample_md();
        // 复制为不同文件名导入，避免去重逻辑干扰。
        for i in 0..MAX_DOCS {
            let copy = dir.join(format!("copy-{i}.md"));
            fs::copy(&src, &copy).unwrap();
            imp.import(&copy).unwrap();
        }
        let extra = dir.join("copy-extra.md");
        fs::copy(&src, &extra).unwrap();
        assert!(matches!(
            imp.import(&extra),
            Err(ImportError::TooManyDocuments)
        ));
    }

    #[test]
    fn duplicate_path_is_deduped() {
        let dir = temp_dir("dedup");
        let mut imp = ProfileImporter::new(dir.join("store")).unwrap();
        imp.import(&sample_md()).unwrap();
        imp.import(&sample_md()).unwrap();
        assert_eq!(imp.list().unwrap().len(), 1);
    }
}
