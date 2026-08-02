//! Whisper 模型管理：清单驱动（`models/models.json` 编译期内嵌）。
//!
//! v0.1.0 的模型来源以**用户本地导入**为主（`import_model` 计算并登记 SHA-256）；
//! 官方清单保留下载元数据，`download_with_resume` 供后续版本自动下载使用。

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MANIFEST_JSON: &str = include_str!("../../models/models.json");
const REGISTRY_FILE: &str = "registry.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelEntry {
    pub id: String,
    pub name: String,
    pub url: String,
    /// 首次导入校验通过后回填；未登记（空串）时导入按大小与文件头兜底匹配。
    pub sha256: String,
    pub size_bytes: u64,
    pub languages: Vec<String>,
    pub tier: String,
    pub backends: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelManifest {
    pub schema_version: u32,
    pub models: Vec<ModelEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImportedModel {
    pub id: String,
    pub file_name: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub imported_at_ms: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelRegistry {
    pub models: Vec<ImportedModel>,
}

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model not found: {0}")]
    NotFound(String),
    #[error("manifest entry {id} is missing field `{field}`")]
    MissingField { id: String, field: &'static str },
    #[error("sha256 mismatch for {path}: expected {expected}, got {actual}")]
    Sha256Mismatch {
        path: String,
        expected: String,
        actual: String,
    },
    #[error("file size mismatch for {path}: expected {expected}, got {actual}")]
    SizeMismatch { path: String, expected: u64, actual: u64 },
    #[error("download failed: {0}")]
    Http(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),
}

pub fn default_models_dir() -> PathBuf {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("MeetingAIAssistant").join("models")
}

pub fn load_manifest() -> Result<ModelManifest, ModelError> {
    Ok(serde_json::from_str(MANIFEST_JSON)?)
}

pub fn sha256_file(path: &Path) -> std::io::Result<String> {
    let bytes = fs::read(path)?;
    Ok(format!("{:x}", Sha256::digest(&bytes)))
}

pub fn verify_sha256(path: &Path, expected: &str) -> Result<(), ModelError> {
    let actual = sha256_file(path)?;
    if actual.eq_ignore_ascii_case(expected) {
        Ok(())
    } else {
        Err(ModelError::Sha256Mismatch {
            path: path.display().to_string(),
            expected: expected.to_string(),
            actual,
        })
    }
}

/// 校验清单条目的必需字段（sha256 可为空串，导入后回填）。
pub fn validate_entry(entry: &ModelEntry) -> Result<(), ModelError> {
    let check = |field: &'static str, ok: bool| -> Result<(), ModelError> {
        if ok {
            Ok(())
        } else {
            Err(ModelError::MissingField {
                id: entry.id.clone(),
                field,
            })
        }
    };
    check("id", !entry.id.is_empty())?;
    check("url", entry.url.starts_with("https://"))?;
    check("size_bytes", entry.size_bytes > 0)?;
    check("languages", !entry.languages.is_empty())?;
    check("tier", !entry.tier.is_empty())?;
    check("backends", !entry.backends.is_empty())?;
    Ok(())
}

pub struct ModelManager {
    models_dir: PathBuf,
    manifest: ModelManifest,
    registry: ModelRegistry,
}

impl ModelManager {
    pub fn new(models_dir: PathBuf) -> Result<Self, ModelError> {
        fs::create_dir_all(&models_dir)?;
        let manifest = load_manifest()?;
        for entry in &manifest.models {
            validate_entry(entry)?;
        }
        let registry_path = models_dir.join(REGISTRY_FILE);
        let registry = if registry_path.exists() {
            serde_json::from_str(&fs::read_to_string(&registry_path)?)?
        } else {
            ModelRegistry::default()
        };
        Ok(Self {
            models_dir,
            manifest,
            registry,
        })
    }

    pub fn models_dir(&self) -> &Path {
        &self.models_dir
    }

    pub fn manifest(&self) -> &ModelManifest {
        &self.manifest
    }

    pub fn registry(&self) -> &ModelRegistry {
        &self.registry
    }

    pub fn registry_mut(&mut self) -> &mut ModelRegistry {
        &mut self.registry
    }

    /// 持久化注册表（扫描导入等外部流程需要）。
    pub fn save_registry(&self) -> Result<(), ModelError> {
        let tmp = self.models_dir.join(format!("{REGISTRY_FILE}.tmp"));
        let mut f = fs::File::create(&tmp)?;
        f.write_all(serde_json::to_string_pretty(&self.registry)?.as_bytes())?;
        f.flush()?;
        self.atomic_replace(&tmp, &self.models_dir.join(REGISTRY_FILE))?;
        Ok(())
    }

    /// 本地导入模型：计算 SHA-256、复制到模型目录（临时文件 + 原子重命名）、登记注册表。
    /// 若源文件与清单中某条目大小一致，则自动采用该条目的 id。
    pub fn import_model(&mut self, source: &Path) -> Result<ImportedModel, ModelError> {
        if !source.is_file() {
            return Err(ModelError::NotFound(source.display().to_string()));
        }
        let sha = sha256_file(source)?;
        let size = fs::metadata(source)?.len();
        let matched = self
            .manifest
            .models
            .iter()
            .find(|e| e.size_bytes == size || (!e.sha256.is_empty() && e.sha256.eq_ignore_ascii_case(&sha)));
        let id = matched
            .map(|e| e.id.clone())
            .unwrap_or_else(|| file_stem(source));
        let file_name = format!("{id}.bin");

        let tmp = self.models_dir.join(format!("{file_name}.tmp-{}", std::process::id()));
        let final_path = self.models_dir.join(&file_name);
        fs::copy(source, &tmp)?;
        self.atomic_replace(&tmp, &final_path)?;

        let imported = ImportedModel {
            id,
            file_name,
            sha256: sha.clone(),
            size_bytes: size,
            imported_at_ms: now_ms(),
        };
        self.registry.models.retain(|m| m.id != imported.id);
        self.registry.models.push(imported.clone());
        self.save_registry()?;
        Ok(imported)
    }

    /// 解析模型路径：本地注册表优先，其次按清单已知文件名查找。
    pub fn resolve_path(&self, id: &str) -> Result<PathBuf, ModelError> {
        if let Some(m) = self.registry.models.iter().find(|m| m.id == id) {
            let p = self.models_dir.join(&m.file_name);
            if p.is_file() {
                return Ok(p);
            }
        }
        let known = self
            .manifest
            .models
            .iter()
            .find(|e| e.id == id)
            .map(|e| self.models_dir.join(format!("{}.bin", e.id)));
        if let Some(p) = known {
            if p.is_file() {
                return Ok(p);
            }
        }
        Err(ModelError::NotFound(id.to_string()))
    }

    pub fn remove_imported(&mut self, id: &str) -> Result<(), ModelError> {
        if let Some(m) = self.registry.models.iter().find(|m| m.id == id) {
            let _ = fs::remove_file(self.models_dir.join(&m.file_name));
            self.registry.models.retain(|m| m.id != id);
            self.save_registry()?;
        }
        Ok(())
    }

    /// 原子替换：写入临时文件后重命名；失败时删除临时文件。
    pub fn atomic_replace(&self, tmp: &Path, dest: &Path) -> std::io::Result<()> {
        match fs::rename(tmp, dest) {
            Ok(()) => Ok(()),
            Err(_) => {
                let _ = fs::remove_file(tmp);
                Err(std::io::Error::new(
                    std::io::ErrorKind::Other,
                    format!("atomic rename failed: {} -> {}", tmp.display(), dest.display()),
                ))
            }
        }
    }

    /// 断点续传下载：已存在部分文件时发送 Range 请求续传，完成后校验大小。
    pub fn download_with_resume(&self, url: &str, dest: &Path, expected_size: u64) -> Result<(), ModelError> {
        let existing = fs::metadata(dest).map(|m| m.len()).unwrap_or(0);
        if existing >= expected_size {
            return Ok(());
        }
        let client = reqwest::blocking::Client::new();
        let mut req = client.get(url);
        if existing > 0 {
            req = req.header(reqwest::header::RANGE, format!("bytes={existing}-"));
        }
        let mut resp = req.send().map_err(|e| ModelError::Http(e.to_string()))?;
        let mut out = fs::OpenOptions::new().create(true).append(true).open(dest)?;
        std::io::copy(&mut resp, &mut out)?;
        let len = fs::metadata(dest)?.len();
        if len != expected_size {
            return Err(ModelError::SizeMismatch {
                path: dest.display().to_string(),
                expected: expected_size,
                actual: len,
            });
        }
        Ok(())
    }
}

fn file_stem(path: &Path) -> String {
    path.file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| "imported-model".into())
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
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    fn temp_dir(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("maa-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_file(path: &Path, bytes: &[u8]) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(bytes).unwrap();
    }

    #[test]
    fn manifest_entries_are_valid() {
        let manifest = load_manifest().unwrap();
        assert_eq!(manifest.schema_version, 1);
        assert!(!manifest.models.is_empty());
        for entry in &manifest.models {
            validate_entry(entry).unwrap();
        }
    }

    #[test]
    fn wrong_hash_is_rejected() {
        let dir = temp_dir("hash");
        let f = dir.join("data.bin");
        write_file(&f, b"hello model");
        let err = verify_sha256(&f, "0000000000000000000000000000000000000000000000000000000000000000")
            .unwrap_err();
        assert!(matches!(err, ModelError::Sha256Mismatch { .. }));
    }

    #[test]
    fn correct_hash_is_accepted() {
        let dir = temp_dir("hash-ok");
        let f = dir.join("data.bin");
        write_file(&f, b"hello model");
        let expected = sha256_file(&f).unwrap();
        verify_sha256(&f, &expected).unwrap();
    }

    #[test]
    fn import_registers_model_and_resolves() {
        let dir = temp_dir("import");
        let src = dir.join("my-model.bin");
        write_file(&src, b"GGML fake model payload");
        let mut mgr = ModelManager::new(dir.join("models")).unwrap();
        let imported = mgr.import_model(&src).unwrap();
        assert_eq!(imported.sha256, sha256_file(&src).unwrap());
        assert_eq!(imported.size_bytes, src.metadata().unwrap().len());
        let resolved = mgr.resolve_path(&imported.id).unwrap();
        assert!(resolved.is_file());
        assert_eq!(fs::read(&resolved).unwrap(), fs::read(&src).unwrap());
    }

    #[test]
    fn import_missing_file_is_not_found() {
        let dir = temp_dir("import-missing");
        let mut mgr = ModelManager::new(dir.join("models")).unwrap();
        assert!(matches!(
            mgr.import_model(&dir.join("nope.bin")),
            Err(ModelError::NotFound(_))
        ));
    }

    #[test]
    fn import_matches_manifest_entry_by_size() {
        let dir = temp_dir("import-match");
        let manifest = load_manifest().unwrap();
        let entry = manifest.models.first().unwrap();
        let src = dir.join("whatever.bin");
        let payload = vec![0xABu8; entry.size_bytes as usize];
        write_file(&src, &payload);
        let mut mgr = ModelManager::new(dir.join("models")).unwrap();
        let imported = mgr.import_model(&src).unwrap();
        assert_eq!(imported.id, entry.id);
    }

    #[test]
    fn atomic_replace_failure_cleans_temp() {
        let dir = temp_dir("atomic");
        let tmp = dir.join("f.tmp");
        write_file(&tmp, b"x");
        let dest = dir.join("sub").join("f"); // 父目录不存在 → rename 失败
        let mgr = ModelManager::new(dir.join("models")).unwrap();
        assert!(mgr.atomic_replace(&tmp, &dest).is_err());
        assert!(!tmp.exists(), "temp file must be deleted on failure");
    }

    #[test]
    fn download_resume_completes_partial_file() {
        let content: Vec<u8> = (0..10_000u32).map(|i| (i % 251) as u8).collect();
        let server_content = content.clone();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 8192];
                let n = stream.read(&mut buf).unwrap();
                let req = String::from_utf8_lossy(&buf[..n]);
                let range = req
                    .lines()
                    .find_map(|l| l.to_lowercase().find("range: bytes=").map(|i| &l[i + 13..]));
                let mut out: Vec<u8> = Vec::new();
                match range {
                    Some(r) => {
                        let start: usize = r.trim_end_matches('-').parse().unwrap_or(0);
                        let body = &server_content[start..];
                        out.extend_from_slice(
                            format!(
                                "HTTP/1.1 206 Partial Content\r\nContent-Range: bytes {}-{}/{}\r\nContent-Length: {}\r\n\r\n",
                                start,
                                server_content.len() - 1,
                                server_content.len(),
                                body.len()
                            )
                            .as_bytes(),
                        );
                        out.extend_from_slice(body);
                    }
                    None => {
                        out.extend_from_slice(
                            format!(
                                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\n\r\n",
                                server_content.len()
                            )
                            .as_bytes(),
                        );
                        out.extend_from_slice(&server_content);
                    }
                }
                let _ = stream.write_all(&out);
                break;
            }
        });

        let url = format!("http://{addr}/model.bin");
        let dir = temp_dir("resume");
        let dest = dir.join("model.bin");
        let mgr = ModelManager::new(dir.join("models")).unwrap();
        // 预写前半部分，模拟中断的下载。
        write_file(&dest, &content[..4_000]);
        mgr.download_with_resume(&url, &dest, content.len() as u64)
            .unwrap();
        assert_eq!(fs::read(&dest).unwrap(), content);
        handle.join().unwrap();
    }

    #[test]
    fn download_detects_size_mismatch() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = thread::spawn(move || {
            for stream in listener.incoming() {
                let mut stream = stream.unwrap();
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf).unwrap();
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nshort");
                break;
            }
        });
        let dir = temp_dir("resume-bad");
        let dest = dir.join("m.bin");
        let mgr = ModelManager::new(dir.join("models")).unwrap();
        let err = mgr
            .download_with_resume(&format!("http://{addr}/m"), &dest, 100)
            .unwrap_err();
        assert!(matches!(err, ModelError::SizeMismatch { .. }));
        handle.join().unwrap();
    }

    #[test]
    fn registry_persists_across_instances() {
        let dir = temp_dir("registry");
        let src = dir.join("m.bin");
        write_file(&src, b"payload-12345");
        let mut mgr = ModelManager::new(dir.join("models")).unwrap();
        mgr.import_model(&src).unwrap();
        let mgr2 = ModelManager::new(dir.join("models")).unwrap();
        assert_eq!(mgr2.registry().models.len(), 1);
        assert!(mgr2.resolve_path(&mgr2.registry().models[0].id).is_ok());
    }
}
