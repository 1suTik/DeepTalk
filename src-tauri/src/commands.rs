use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::State;

use crate::answer::{
    compatible::CompatibleProvider, deepseek::DeepSeekProvider, openai::OpenAiProvider,
};
use crate::answer::{AnswerError, AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken};
use crate::pipeline::RealPipeline;
use crate::session::{EventSink, Orchestrator, PipelineSource, TauriSink};
use crate::state::{SessionHandle, SessionManager, SessionState};
use crate::storage::{credential_account, CredentialStore, Db, Retention};

// ---------------------------------------------------------------------------
// 应用级状态
// ---------------------------------------------------------------------------

pub struct AppState {
    pub db: Db,
    pub credentials: CredentialStore,
}

impl AppState {
    pub fn new() -> Result<Self, String> {
        let data_dir = app_data_dir()?;
        let db = Db::open(&data_dir.join("history.db")).map_err(|e| e.to_string())?;
        Retention::spawn_periodic(db.clone());
        Ok(Self {
            db,
            credentials: CredentialStore::new(),
        })
    }
}

fn app_data_dir() -> Result<PathBuf, String> {
    let base = std::env::var("LOCALAPPDATA").unwrap_or_else(|_| ".".into());
    Ok(PathBuf::from(base).join("MeetingAIAssistant"))
}

// ---------------------------------------------------------------------------
// 会话命令（Task 9：完整流水线编排）
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn start_session(
    app: tauri::AppHandle,
    manager: State<'_, Arc<SessionManager>>,
    state: State<'_, AppState>,
) -> Result<SessionState, String> {
    if manager.state() != SessionState::Idle {
        return Err("会话已在运行".into());
    }
    let settings = AppSettings::load(&state.db, &state.credentials);
    // 依次检查：模型、provider 配置；任一步失败都直接返回，不启动任何资源。
    RealPipeline::find_model()?;
    let api_key = state
        .credentials
        .get(&credential_account(&settings.provider_kind))
        .ok()
        .flatten()
        .unwrap_or_default();
    if settings.provider_kind != "custom" && api_key.trim().is_empty() {
        return Err("请先在设置页配置 API Key".into());
    }
    let provider = provider_from_settings(&settings, Some(&api_key)).map_err(|e| e.to_string())?;
    let pipeline: Arc<dyn PipelineSource> =
        Arc::new(RealPipeline::new(settings.microphone_enabled));
    let started_at_ms = crate::storage::retention::now_ms();
    let meeting_id = format!("meeting-{started_at_ms}");
    manager.start().map_err(|e| format!("{e}"))?;
    state
        .db
        .create_meeting(&meeting_id, started_at_ms)
        .map_err(|e| e.to_string())?;
    let sink: Arc<dyn EventSink> = Arc::new(TauriSink::new(app.clone()));
    let (orch, ctl) = Orchestrator::new(pipeline, provider, state.db.clone(), sink, meeting_id);
    orch.load_enabled_profiles();
    let manager2 = manager.inner().clone();
    let task = tokio::spawn(async move {
        if let Err(e) = orch.run().await {
            tracing::error!("会话编排失败：{e}");
            let _ = manager2.fail(e);
        }
    });
    manager.attach(SessionHandle {
        ctl: ctl.clone(),
        task: task.abort_handle(),
    });
    Ok(manager.state())
}

#[tauri::command]
pub async fn stop_session(manager: State<'_, Arc<SessionManager>>) -> Result<SessionState, String> {
    let Some(handle) = manager.take_handle() else {
        let _ = manager.stop();
        return Ok(manager.state());
    };
    handle.ctl.stop();
    // 等待编排任务结束（最多 2 秒），超时则强制中止
    let task = handle.task;
    let deadline = tokio::time::Instant::now() + Duration::from_secs(2);
    while !task.is_finished() && tokio::time::Instant::now() < deadline {
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    if !task.is_finished() {
        task.abort();
    }
    manager.stop().map_err(|e| format!("{e}"))?;
    Ok(manager.state())
}

#[tauri::command]
pub fn session_state(manager: State<'_, Arc<SessionManager>>) -> SessionState {
    manager.state()
}

#[tauri::command]
pub async fn pin_current_answer(manager: State<'_, Arc<SessionManager>>) -> Result<(), String> {
    let Some(handle) = manager.handle() else {
        return Err("会话未运行".into());
    };
    handle.ctl.pin_current().await;
    Ok(())
}

#[tauri::command]
pub async fn generate_answer(manager: State<'_, Arc<SessionManager>>) -> Result<(), String> {
    let Some(handle) = manager.handle() else {
        return Err("会话未运行".into());
    };
    handle.ctl.generate_last().await;
    Ok(())
}

#[tauri::command]
pub async fn cancel_current_answer(manager: State<'_, Arc<SessionManager>>) -> Result<(), String> {
    let Some(handle) = manager.handle() else {
        return Err("会话未运行".into());
    };
    handle.ctl.cancel_current().await;
    Ok(())
}

// ---------------------------------------------------------------------------
// 设置命令（Task 8）
// ---------------------------------------------------------------------------

/// 设置页领域模型（API Key 永不回传，只回传 `has_api_key`）。
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub provider_kind: String,
    pub base_url: String,
    pub model: String,
    pub has_api_key: bool,
    pub retention_days: u64,
    pub microphone_enabled: bool,
    pub asr_model_id: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            provider_kind: "deepseek".into(),
            base_url: crate::answer::deepseek::DEEPSEEK_BASE_URL.into(),
            model: "deepseek-v4-flash".into(),
            has_api_key: false,
            retention_days: 7,
            microphone_enabled: false,
            asr_model_id: String::new(),
        }
    }
}

impl AppSettings {
    fn load(db: &Db, credentials: &CredentialStore) -> Self {
        let get = |k: &str, default: &str| {
            db.get_setting(k)
                .ok()
                .flatten()
                .unwrap_or_else(|| default.to_string())
        };
        let provider_kind = get("provider.kind", "deepseek");
        let has_api_key = credentials
            .get(&credential_account(&provider_kind))
            .ok()
            .flatten()
            .map(|s| !s.is_empty())
            .unwrap_or(false);
        Self {
            provider_kind,
            base_url: get(
                "provider.base_url",
                crate::answer::deepseek::DEEPSEEK_BASE_URL,
            ),
            model: get("provider.model", "deepseek-v4-flash"),
            has_api_key,
            retention_days: get("retention.days", "7").parse().unwrap_or(7),
            microphone_enabled: get("mic.enabled", "0") == "1",
            asr_model_id: get("asr.model_id", ""),
        }
    }

    fn save(
        &self,
        db: &Db,
        credentials: &CredentialStore,
        api_key: Option<&str>,
    ) -> Result<(), String> {
        db.set_setting("provider.kind", &self.provider_kind)
            .map_err(|e| e.to_string())?;
        db.set_setting("provider.base_url", &self.base_url)
            .map_err(|e| e.to_string())?;
        db.set_setting("provider.model", &self.model)
            .map_err(|e| e.to_string())?;
        db.set_setting("retention.days", &self.retention_days.to_string())
            .map_err(|e| e.to_string())?;
        db.set_setting(
            "mic.enabled",
            if self.microphone_enabled { "1" } else { "0" },
        )
        .map_err(|e| e.to_string())?;
        db.set_setting("asr.model_id", &self.asr_model_id)
            .map_err(|e| e.to_string())?;
        if let Some(key) = api_key {
            credentials
                .set(&credential_account(&self.provider_kind), key)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> AppSettings {
    AppSettings::load(&state.db, &state.credentials)
}

#[tauri::command]
pub fn save_settings(
    state: State<'_, AppState>,
    settings: AppSettings,
    api_key: Option<String>,
) -> Result<(), String> {
    settings.save(&state.db, &state.credentials, api_key.as_deref())
}

/// 用给定配置构造 provider（供连接测试与 Task 9 编排复用）。
pub fn provider_from_settings(
    settings: &AppSettings,
    api_key: Option<&str>,
) -> Result<Box<dyn AnswerProvider>, AnswerError> {
    let key = api_key.unwrap_or("").to_string();
    let model = settings.model.clone();
    match settings.provider_kind.as_str() {
        "openai" => Ok(Box::new(OpenAiProvider::new(model, key)?)),
        "custom" => Ok(Box::new(CompatibleProvider::new(
            settings.base_url.clone(),
            model,
            key,
        )?)),
        _ => Ok(Box::new(DeepSeekProvider::new(model, key)?)),
    }
}

/// 连接测试：发起一次流式请求，取首个 delta 或完成状态。
/// 真实网络请求不在单元测试中执行（使用假 provider 验证该函数逻辑）。
pub async fn run_provider_test(
    provider: &dyn AnswerProvider,
    request: AnswerRequest,
    cancel: CancellationToken,
) -> Result<String, String> {
    let mut rx = provider
        .stream_answer(request, cancel)
        .await
        .map_err(|e| e.to_string())?;
    let mut first_delta: Option<String> = None;
    loop {
        match rx.recv().await {
            Some(AnswerEvent::Started) => {}
            Some(AnswerEvent::ShortAnswerDelta(d)) => {
                if first_delta.is_none() {
                    first_delta = Some(d.trim().to_string());
                }
            }
            Some(AnswerEvent::Completed) => break,
            Some(AnswerEvent::Failed(msg)) => return Err(msg),
            Some(AnswerEvent::KeyPoints(_)) | Some(AnswerEvent::FollowUps(_)) => {}
            None => break,
        }
    }
    match first_delta {
        Some(d) => Ok(format!("连接成功，首个输出：{d}")),
        None => Err("连接成功但未收到内容".into()),
    }
}

#[tauri::command]
pub async fn test_provider_connection(
    _state: State<'_, AppState>,
    settings: AppSettings,
    api_key: Option<String>,
) -> Result<String, String> {
    let provider =
        provider_from_settings(&settings, api_key.as_deref()).map_err(|e| e.to_string())?;
    let request = AnswerRequest {
        question_id: "connectivity-test".into(),
        question: "请回复“连接成功”即可，不需要展开。".into(),
        recent_transcript: vec![],
        profile_context: vec![],
        response_language: "中文".into(),
    };
    let cancel = CancellationToken::new();
    let result = tokio::time::timeout(
        std::time::Duration::from_secs(20),
        run_provider_test(provider.as_ref(), request, cancel),
    )
    .await;
    match result {
        Ok(Ok(ok)) => Ok(ok),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("连接超时".into()),
    }
}

#[tauri::command]
pub fn clear_all_data(state: State<'_, AppState>) -> Result<(), String> {
    state.db.purge_all().map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// 模型管理命令（Task 11：本地导入体验，不做自动下载）
// ---------------------------------------------------------------------------

/// 清单模型 + 本地导入状态（设置页模型卡片）。
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: String,
    pub name: String,
    pub size_bytes: u64,
    pub tier: String,
    pub imported: bool,
    pub sha256_ok: bool,
}

/// 设置页：模型清单与导入状态。
/// 轻量实现：状态取自注册表已登记的 SHA-256 与文件存在性，**不重新哈希模型文件**
/// （574MB 全量哈希会阻塞主线程导致窗口未响应；完整校验由「扫描并校验」在后台线程执行）。
#[tauri::command]
pub fn list_models() -> Result<Vec<ModelStatus>, String> {
    let mgr = crate::asr::model_manager::ModelManager::new(
        crate::asr::model_manager::default_models_dir(),
    )
    .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for entry in &mgr.manifest().models {
        let imported = if entry.id == "silero-vad-v6" {
            // Silero 固定落盘为 silero_vad.onnx（VAD 加载路径）
            mgr.models_dir().join("silero_vad.onnx").is_file()
        } else {
            mgr.resolve_path(&entry.id).is_ok()
        };
        // 用导入时登记的 SHA-256 与清单比对；silero 固定文件名也视为已导入
        let registered_sha = mgr
            .registry()
            .models
            .iter()
            .find(|m| m.id == entry.id)
            .map(|m| m.sha256.clone())
            .unwrap_or_default();
        let sha256_ok = if entry.sha256.is_empty() {
            imported
        } else {
            !registered_sha.is_empty() && registered_sha.eq_ignore_ascii_case(&entry.sha256)
        };
        out.push(ModelStatus {
            id: entry.id.clone(),
            name: entry.name.clone(),
            size_bytes: entry.size_bytes,
            tier: entry.tier.clone(),
            imported,
            sha256_ok,
        });
    }
    Ok(out)
}

/// 扫描模型目录中未登记的候选文件并按清单（SHA-256/大小）匹配导入；
/// Silero 条目额外落盘为 `silero_vad.onnx`（VAD 固定文件名）。
pub fn scan_and_import(
    dir: &std::path::Path,
) -> Result<Vec<crate::asr::model_manager::ImportedModel>, String> {
    let mut mgr = crate::asr::model_manager::ModelManager::new(dir.to_path_buf())
        .map_err(|e| e.to_string())?;
    let registry_ids: std::collections::HashSet<String> =
        mgr.registry().models.iter().map(|m| m.id.clone()).collect();
    let mut imported: Vec<crate::asr::model_manager::ImportedModel> = Vec::new();
    let mut scanned: Vec<std::path::PathBuf> = Vec::new();
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            let p = e.path();
            if !p.is_file() {
                continue;
            }
            let is_candidate = p
                .extension()
                .map(|x| {
                    let x = x.to_string_lossy().to_lowercase();
                    x == "bin" || x == "onnx"
                })
                .unwrap_or(false);
            if is_candidate
                && !p
                    .file_name()
                    .map(|n| n.to_string_lossy().starts_with("silero_vad.onnx"))
                    .unwrap_or(false)
            {
                scanned.push(p);
            }
        }
    }
    for p in scanned {
        let matched_id = {
            let size = std::fs::metadata(&p).map(|m| m.len()).unwrap_or(0);
            let sha = crate::asr::model_manager::sha256_file(&p).unwrap_or_default();
            mgr.manifest()
                .models
                .iter()
                .find(|e| {
                    if registry_ids.contains(&e.id) {
                        return false; // 已登记条目不重复导入
                    }
                    size == e.size_bytes
                        || (!e.sha256.is_empty() && e.sha256.eq_ignore_ascii_case(&sha))
                })
                .map(|e| e.id.clone())
        };
        if let Some(id) = matched_id {
            if id == "silero-vad-v6" {
                // 固定落盘为 silero_vad.onnx（VAD 加载路径），并登记注册表
                let dest = dir.join("silero_vad.onnx");
                std::fs::copy(&p, &dest).map_err(|e| e.to_string())?;
                let sha = crate::asr::model_manager::sha256_file(&dest).unwrap_or_default();
                let size = std::fs::metadata(&dest).map(|m| m.len()).unwrap_or(0);
                let entry = crate::asr::model_manager::ImportedModel {
                    id: id.clone(),
                    file_name: "silero_vad.onnx".into(),
                    sha256: sha,
                    size_bytes: size,
                    imported_at_ms: crate::storage::retention::now_ms(),
                };
                mgr.registry_mut().models.retain(|m| m.id != id);
                mgr.registry_mut().models.push(entry.clone());
                mgr.save_registry().map_err(|e| e.to_string())?;
                imported.push(entry);
            } else {
                match mgr.import_model(&p) {
                    Ok(m) => imported.push(m),
                    Err(e) => tracing::warn!("模型导入失败 {}: {e}", p.display()),
                }
            }
        }
    }
    Ok(imported)
}

/// 扫描并按清单校验导入（可能对 GB 级模型做哈希，在后台线程执行避免卡 UI）。
#[tauri::command]
pub async fn scan_and_import_models(
) -> Result<Vec<crate::asr::model_manager::ImportedModel>, String> {
    let dir = crate::asr::model_manager::default_models_dir();
    tokio::task::spawn_blocking(move || scan_and_import(&dir))
        .await
        .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::AnswerEvent;
    use tokio::sync::mpsc;

    /// 假 provider：直接通过 channel 发出事件，模拟真实流。
    struct FakeProvider {
        events: Vec<AnswerEvent>,
    }

    impl AnswerProvider for FakeProvider {
        fn stream_answer<'a>(
            &'a self,
            _request: AnswerRequest,
            cancel: CancellationToken,
        ) -> futures_util::future::BoxFuture<'a, Result<mpsc::Receiver<AnswerEvent>, AnswerError>>
        {
            Box::pin(async move {
                let (tx, rx) = mpsc::channel(16);
                let events = self.events.clone();
                tokio::spawn(async move {
                    for ev in events {
                        if cancel.is_cancelled() {
                            return;
                        }
                        if tx.send(ev).await.is_err() {
                            return;
                        }
                        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
                    }
                });
                Ok(rx)
            })
        }
    }

    #[tokio::test]
    async fn provider_test_reports_first_delta() {
        let provider = FakeProvider {
            events: vec![
                AnswerEvent::Started,
                AnswerEvent::ShortAnswerDelta("你好\n".into()),
                AnswerEvent::KeyPoints(vec!["要点".into()]),
                AnswerEvent::Completed,
            ],
        };
        let req = AnswerRequest {
            question_id: "t".into(),
            question: "问题".into(),
            recent_transcript: vec![],
            profile_context: vec![],
            response_language: "中文".into(),
        };
        let out = run_provider_test(&provider, req, CancellationToken::new())
            .await
            .unwrap();
        assert!(out.contains("连接成功"));
        assert!(out.contains("你好"));
    }

    #[tokio::test]
    async fn provider_test_propagates_failure() {
        let provider = FakeProvider {
            events: vec![
                AnswerEvent::Started,
                AnswerEvent::Failed("认证失败：HTTP 401".into()),
            ],
        };
        let req = AnswerRequest {
            question_id: "t".into(),
            question: "问题".into(),
            recent_transcript: vec![],
            profile_context: vec![],
            response_language: "中文".into(),
        };
        let err = run_provider_test(&provider, req, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(err.contains("认证失败"), "{err}");
    }

    #[test]
    fn settings_load_uses_defaults_when_empty() {
        let db = Db::open_in_memory().unwrap();
        let creds = CredentialStore::new();
        // 使用绝不会存在的 provider 名，避免本机 Credential Manager 中真实保存的 key 影响断言
        db.set_setting("provider.kind", "never-used-provider")
            .unwrap();
        let s = AppSettings::load(&db, &creds);
        assert_eq!(s.provider_kind, "never-used-provider");
        assert_eq!(s.retention_days, 7);
        assert!(!s.microphone_enabled);
        assert!(!s.has_api_key);
    }

    #[test]
    fn settings_save_and_reload_roundtrip() {
        let db = Db::open_in_memory().unwrap();
        let creds = CredentialStore::new();
        let s = AppSettings {
            provider_kind: "custom".into(),
            base_url: "http://127.0.0.1:11434/v1".into(),
            model: "qwen3:8b".into(),
            has_api_key: false,
            retention_days: 3,
            microphone_enabled: true,
            asr_model_id: "turbo".into(),
        };
        s.save(&db, &creds, None).unwrap();
        let loaded = AppSettings::load(&db, &creds);
        assert_eq!(loaded.provider_kind, "custom");
        assert_eq!(loaded.base_url, "http://127.0.0.1:11434/v1");
        assert_eq!(loaded.model, "qwen3:8b");
        assert_eq!(loaded.retention_days, 3);
        assert!(loaded.microphone_enabled);
        assert_eq!(loaded.asr_model_id, "turbo");
    }

    #[test]
    fn credential_id_is_kind_scoped() {
        assert_eq!(credential_account("deepseek"), "api-key:deepseek");
        assert_eq!(credential_account("openai"), "api-key:openai");
    }

    #[test]
    fn scan_ignores_unmatched_files_and_tmp_artifacts() {
        let dir = std::env::temp_dir().join(format!("maa-scan-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // 未匹配清单的随机文件不应被登记
        std::fs::write(dir.join("random.bin"), vec![0u8; 1024]).unwrap();
        std::fs::write(dir.join("notes.txt"), b"not a model").unwrap();
        let imported = super::scan_and_import(&dir).unwrap();
        assert!(imported.is_empty(), "无匹配文件不得导入");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
