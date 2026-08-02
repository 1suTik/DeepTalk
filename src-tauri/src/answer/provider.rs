//! 答案 Provider 核心：请求/事件类型、取消令牌、OpenAI-compatible SSE 流式引擎。
//!
//! 接口模式参照主流 AI/Agent 调用约定：
//! - DeepSeek 与 Custom 使用 OpenAI-compatible chat streaming wire format（`/chat/completions` + SSE）。
//! - OpenAI 使用官方 Responses API 流式事件（`/responses` + `response.output_text.delta`）。
//! - 超时：连接 15s / 总 60s；取消；网络错误最多自动重试 1 次（认证失败与限流不重试）。

use std::fmt;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::{mpsc, Notify};
use tracing::warn;

use crate::answer::prompt;

pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
pub const DEFAULT_TOTAL_TIMEOUT: Duration = Duration::from_secs(60);
const HEARTBEAT: Duration = Duration::from_millis(200);
const MAX_ATTEMPTS: usize = 2;
const RETRY_BACKOFF: Duration = Duration::from_millis(300);

/// 答案提供商类型。具体模型 ID 由设置页保存，不在代码中写死。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderKind {
    DeepSeek,
    OpenAi,
    Custom,
}

impl fmt::Display for ProviderKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            ProviderKind::DeepSeek => "deepseek",
            ProviderKind::OpenAi => "openai",
            ProviderKind::Custom => "custom",
        };
        f.write_str(s)
    }
}

/// 答案 Provider 配置（由设置页保存；API Key 来自 Windows Credential Manager）。
#[derive(Debug, Clone)]
pub struct AnswerConfig {
    pub kind: ProviderKind,
    pub base_url: String,
    pub model: String,
    pub api_key: String,
    pub connect_timeout: Duration,
    pub total_timeout: Duration,
}

impl AnswerConfig {
    pub fn new(
        kind: ProviderKind,
        base_url: impl Into<String>,
        model: impl Into<String>,
        api_key: impl Into<String>,
    ) -> Self {
        Self {
            kind,
            base_url: base_url.into(),
            model: model.into(),
            api_key: api_key.into(),
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            total_timeout: DEFAULT_TOTAL_TIMEOUT,
        }
    }
}

/// 答案生成请求：问题、近期转写与命中的资料片段。
#[derive(Debug, Clone)]
pub struct AnswerRequest {
    pub question_id: String,
    pub question: String,
    pub recent_transcript: Vec<String>,
    pub profile_context: Vec<String>,
    pub response_language: String,
}

/// 流式答案事件。固定输出顺序：`short_answer -> key_points -> follow_ups`。
#[derive(Debug, Clone, PartialEq)]
pub enum AnswerEvent {
    Started,
    ShortAnswerDelta(String),
    KeyPoints(Vec<String>),
    FollowUps(Vec<String>),
    Completed,
    Failed(String),
}

#[derive(Debug, thiserror::Error)]
pub enum AnswerError {
    #[error("认证失败：{0}")]
    Auth(String),
    #[error("请求被限流，请 {0:?} 秒后重试")]
    RateLimited(Option<u64>),
    #[error("网络错误：{0}")]
    Network(String),
    #[error("请求超时")]
    Timeout,
    #[error("请求已取消")]
    Cancelled,
    #[error("响应格式异常：{0}")]
    Malformed(String),
}

impl AnswerError {
    /// 是否允许自动重试（认证失败与限流不重试）。
    pub fn retryable(&self) -> bool {
        matches!(self, AnswerError::Network(_))
    }
}

/// 轻量取消令牌（基于 tokio Notify，无额外依赖）。
#[derive(Debug, Clone, Default)]
pub struct CancellationToken {
    cancelled: Arc<AtomicBool>,
    notify: Arc<Notify>,
}

impl CancellationToken {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst)
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::SeqCst);
        self.notify.notify_waiters();
    }

    /// 返回可等待的未来；已取消时立即就绪。可在 `tokio::select!` 中使用。
    pub async fn cancelled(&self) {
        if self.is_cancelled() {
            return;
        }
        let notified = self.notify.notified();
        tokio::pin!(notified);
        notified.as_mut().enable();
        if !self.is_cancelled() {
            notified.await;
        }
    }
}

/// 可替换的答案 Provider 接口。
pub trait AnswerProvider: Send + Sync {
    /// 发起流式请求，返回事件接收端；错误通过 `AnswerEvent::Failed` 上报，
    /// 取消后不再发送任何事件（接收端直接关闭）。
    fn stream_answer<'a>(
        &'a self,
        request: AnswerRequest,
        cancel: CancellationToken,
    ) -> futures_util::future::BoxFuture<'a, Result<mpsc::Receiver<AnswerEvent>, AnswerError>>;
}

// ---------------------------------------------------------------------------
// 引擎
// ---------------------------------------------------------------------------

/// API 风格：OpenAI-compatible chat streaming 或 OpenAI Responses 流式事件。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ApiStyle {
    ChatCompletions,
    Responses,
}

#[derive(Debug, Clone)]
pub(crate) struct OpenAiCompatibleClient {
    cfg: AnswerConfig,
    style: ApiStyle,
}

impl OpenAiCompatibleClient {
    pub(crate) fn new(cfg: AnswerConfig, style: ApiStyle) -> Result<Self, AnswerError> {
        if !(cfg.base_url.starts_with("http://") || cfg.base_url.starts_with("https://")) {
            return Err(AnswerError::Malformed(format!(
                "base_url 必须以 http:// 或 https:// 开头：{}",
                cfg.base_url
            )));
        }
        Ok(Self { cfg, style })
    }

    fn endpoint(&self) -> &'static str {
        match self.style {
            ApiStyle::ChatCompletions => "chat/completions",
            ApiStyle::Responses => "responses",
        }
    }

    fn request_body(&self, request: &AnswerRequest) -> Value {
        match self.style {
            ApiStyle::ChatCompletions => json!({
                "model": self.cfg.model,
                "stream": true,
                "temperature": 0.3,
                "messages": [
                    { "role": "system", "content": prompt::build_system_prompt(&request.response_language) },
                    { "role": "user", "content": prompt::build_user_prompt(request) },
                ],
            }),
            ApiStyle::Responses => json!({
                "model": self.cfg.model,
                "stream": true,
                "temperature": 0.3,
                "instructions": prompt::build_system_prompt(&request.response_language),
                "input": [ { "role": "user", "content": prompt::build_user_prompt(request) } ],
            }),
        }
    }

    fn extract_delta(&self, json: &Value) -> Option<String> {
        match self.style {
            ApiStyle::ChatCompletions => chat_delta(json),
            ApiStyle::Responses => responses_delta(json),
        }
    }

    pub(crate) async fn stream_answer(
        &self,
        request: AnswerRequest,
        cancel: CancellationToken,
    ) -> Result<mpsc::Receiver<AnswerEvent>, AnswerError> {
        let body = self.request_body(&request);
        let url = format!(
            "{}/{}",
            self.cfg.base_url.trim_end_matches('/'),
            self.endpoint()
        );
        let (tx, rx) = mpsc::channel(64);
        let client = reqwest::Client::builder()
            .connect_timeout(self.cfg.connect_timeout)
            .build()
            .map_err(|e| AnswerError::Network(e.to_string()))?;
        let me = self.clone();
        let total_timeout = self.cfg.total_timeout;
        tokio::spawn(async move {
            let _ = send_control(&tx, AnswerEvent::Started, &cancel).await;
            let result = tokio::time::timeout(
                total_timeout,
                me.run(&client, &url, body, cancel.clone(), &tx),
            )
            .await;
            match result {
                Ok(Ok(())) => {}
                Ok(Err(AnswerError::Cancelled)) => {}
                Ok(Err(e)) => {
                    let _ = send_control(&tx, AnswerEvent::Failed(e.to_string()), &cancel).await;
                }
                Err(_) => {
                    let _ =
                        send_control(&tx, AnswerEvent::Failed("请求超时".into()), &cancel).await;
                }
            }
        });
        Ok(rx)
    }

    /// 带重试的请求-流式读取循环。网络错误最多重试一次；认证失败与限流不重试。
    async fn run(
        &self,
        client: &reqwest::Client,
        url: &str,
        body: Value,
        cancel: CancellationToken,
        tx: &mpsc::Sender<AnswerEvent>,
    ) -> Result<(), AnswerError> {
        let mut attempts = 0usize;
        loop {
            if cancel.is_cancelled() {
                return Err(AnswerError::Cancelled);
            }
            let send = client
                .post(url)
                .bearer_auth(&self.cfg.api_key)
                .json(&body)
                .send();
            let cancelled = cancel.cancelled();
            tokio::pin!(cancelled);
            let resp = tokio::select! {
                result = send => match result {
                    Ok(resp) => resp,
                    Err(e) => return Err(map_send_error(e)),
                },
                _ = &mut cancelled => return Err(AnswerError::Cancelled),
            };
            let status = resp.status();
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return Err(AnswerError::Auth(format!("HTTP {status}")));
            }
            if status == StatusCode::TOO_MANY_REQUESTS {
                let retry_after = resp
                    .headers()
                    .get(reqwest::header::RETRY_AFTER)
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.parse::<u64>().ok());
                return Err(AnswerError::RateLimited(retry_after));
            }
            if status.is_server_error() || status == StatusCode::REQUEST_TIMEOUT {
                if attempts + 1 < MAX_ATTEMPTS && !cancel.is_cancelled() {
                    attempts += 1;
                    tokio::time::sleep(RETRY_BACKOFF).await;
                    continue;
                }
                return Err(AnswerError::Network(format!("HTTP {status}")));
            }
            if !status.is_success() {
                return Err(AnswerError::Malformed(format!("HTTP {status}")));
            }
            match self.read_stream(resp, tx, &cancel).await {
                Ok(()) => return Ok(()),
                Err(AnswerError::Network(_))
                    if attempts + 1 < MAX_ATTEMPTS && !cancel.is_cancelled() =>
                {
                    attempts += 1;
                    tokio::time::sleep(RETRY_BACKOFF).await;
                }
                Err(e) => return Err(e),
            }
        }
    }

    /// 读取 SSE 流：按行解析 `data:` 字段，累积分段并发送事件。
    /// 流提前关闭且未发出任何内容时返回网络错误（可重试）；已收到内容则按成功结束。
    async fn read_stream(
        &self,
        resp: reqwest::Response,
        tx: &mpsc::Sender<AnswerEvent>,
        cancel: &CancellationToken,
    ) -> Result<(), AnswerError> {
        let mut stream = resp.bytes_stream();
        let mut line_buf: Vec<u8> = Vec::new();
        let mut acc = SectionAccumulator::new();
        let mut emitted = false;
        let mut done = false;
        loop {
            if cancel.is_cancelled() {
                return Err(AnswerError::Cancelled);
            }
            let chunk = tokio::time::timeout(HEARTBEAT, stream.next()).await;
            let chunk = match chunk {
                Ok(Some(Ok(bytes))) => bytes,
                Ok(Some(Err(e))) => return Err(map_send_error(e)),
                Ok(None) => {
                    if done || emitted || acc.has_content() {
                        break;
                    }
                    return Err(AnswerError::Network("服务端提前关闭了流".into()));
                }
                Err(_) => continue,
            };
            for &b in chunk.iter() {
                line_buf.push(b);
                if b == b'\n' {
                    let line = std::mem::take(&mut line_buf);
                    if let Some(data) = sse_data(&line) {
                        if data == "[DONE]" {
                            done = true;
                            continue;
                        }
                        match serde_json::from_str::<Value>(data) {
                            Ok(json) => {
                                if let Some(content) = self.extract_delta(&json) {
                                    acc.push_line(&content, tx, cancel, &mut emitted).await?;
                                }
                            }
                            Err(e) => {
                                warn!("answer provider 返回无法解析的 JSON 片段：{e}");
                                acc.degrade();
                            }
                        }
                    }
                }
            }
        }
        acc.finish(tx, cancel, &mut emitted).await?;
        send_control(tx, AnswerEvent::Completed, cancel).await?;
        Ok(())
    }
}

/// 从一行 SSE 数据中提取 `data:` 字段值；空行/注释/事件行返回 None。
fn sse_data(line: &[u8]) -> Option<&str> {
    let text = std::str::from_utf8(line.trim_ascii()).ok()?;
    let text = text.strip_prefix("data:")?;
    let data = text.trim_start();
    if data.is_empty() {
        None
    } else {
        Some(data)
    }
}

/// 去掉要点行前缀（"- " 或 "• "）。
fn strip_bullet(line: &str) -> String {
    let t = line.trim_start();
    let t = t
        .strip_prefix("- ")
        .or_else(|| t.strip_prefix("• "))
        .unwrap_or(t);
    t.trim().to_string()
}

const MARKER_SHORT_ANSWER: &str = "[短答]";
const MARKER_KEY_POINTS: &str = "[要点]";
const MARKER_FOLLOW_UPS: &str = "[追问]";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Section {
    ShortAnswer,
    KeyPoints,
    FollowUps,
}

/// 按固定顺序（短答 -> 要点 -> 追问）累积流式内容；
/// 解析异常后降级：保留已收到的短答，后续内容一律作为普通要点。
struct SectionAccumulator {
    section: Section,
    key_points: Vec<String>,
    follow_ups: Vec<String>,
    degraded: bool,
}

impl SectionAccumulator {
    fn new() -> Self {
        Self {
            section: Section::ShortAnswer,
            key_points: Vec::new(),
            follow_ups: Vec::new(),
            degraded: false,
        }
    }

    fn degrade(&mut self) {
        self.degraded = true;
    }

    async fn push_line(
        &mut self,
        line: &str,
        tx: &mpsc::Sender<AnswerEvent>,
        cancel: &CancellationToken,
        emitted: &mut bool,
    ) -> Result<(), AnswerError> {
        let trimmed = line.trim();
        if self.degraded {
            if !matches!(
                trimmed,
                MARKER_SHORT_ANSWER | MARKER_KEY_POINTS | MARKER_FOLLOW_UPS
            ) {
                self.key_points.push(trimmed.to_string());
            }
            return Ok(());
        }
        match trimmed {
            MARKER_SHORT_ANSWER => self.section = Section::ShortAnswer,
            MARKER_KEY_POINTS => self.section = Section::KeyPoints,
            MARKER_FOLLOW_UPS => {
                self.section = Section::FollowUps;
                self.flush_key_points(tx, cancel, emitted).await?;
            }
            _ => match self.section {
                Section::ShortAnswer => {
                    // 短答按 token 流原样拼接：模型每个 SSE 事件可能只有 1-2 个字，
                    // 不能加空格或换行（否则观感是逐字空格）；空行才是段落分隔。
                    let delta = if trimmed.is_empty() {
                        "\n\n".to_string()
                    } else {
                        line.trim_end().to_string()
                    };
                    send_content(tx, AnswerEvent::ShortAnswerDelta(delta), cancel, emitted).await?;
                }
                Section::KeyPoints => self.key_points.push(strip_bullet(trimmed)),
                Section::FollowUps => self.follow_ups.push(strip_bullet(trimmed)),
            },
        }
        Ok(())
    }

    async fn flush_key_points(
        &mut self,
        tx: &mpsc::Sender<AnswerEvent>,
        cancel: &CancellationToken,
        emitted: &mut bool,
    ) -> Result<(), AnswerError> {
        if !self.key_points.is_empty() {
            let points = std::mem::take(&mut self.key_points);
            send_content(tx, AnswerEvent::KeyPoints(points), cancel, emitted).await?;
        }
        Ok(())
    }

    fn has_content(&self) -> bool {
        !self.key_points.is_empty() || !self.follow_ups.is_empty()
    }

    /// 流结束时刷新所有未发送的分段。
    async fn finish(
        mut self,
        tx: &mpsc::Sender<AnswerEvent>,
        cancel: &CancellationToken,
        emitted: &mut bool,
    ) -> Result<(), AnswerError> {
        self.flush_key_points(tx, cancel, emitted).await?;
        if !self.follow_ups.is_empty() {
            send_content(
                tx,
                AnswerEvent::FollowUps(std::mem::take(&mut self.follow_ups)),
                cancel,
                emitted,
            )
            .await?;
        }
        Ok(())
    }
}

/// 发送内容事件并标记已发出（用于决定是否可重试）。
async fn send_content(
    tx: &mpsc::Sender<AnswerEvent>,
    event: AnswerEvent,
    cancel: &CancellationToken,
    emitted: &mut bool,
) -> Result<(), AnswerError> {
    if cancel.is_cancelled() {
        return Err(AnswerError::Cancelled);
    }
    tx.send(event).await.map_err(|_| AnswerError::Cancelled)?;
    *emitted = true;
    Ok(())
}

/// 发送控制事件（Started/Completed/Failed），不计入内容标记。
async fn send_control(
    tx: &mpsc::Sender<AnswerEvent>,
    event: AnswerEvent,
    cancel: &CancellationToken,
) -> Result<(), AnswerError> {
    if cancel.is_cancelled() {
        return Err(AnswerError::Cancelled);
    }
    tx.send(event).await.map_err(|_| AnswerError::Cancelled)
}

fn map_send_error(e: reqwest::Error) -> AnswerError {
    if e.is_timeout() {
        AnswerError::Timeout
    } else {
        AnswerError::Network(e.to_string())
    }
}

fn chat_delta(json: &Value) -> Option<String> {
    json.get("choices")?
        .as_array()?
        .first()?
        .get("delta")?
        .get("content")?
        .as_str()
        .map(|s| s.to_string())
}

fn responses_delta(json: &Value) -> Option<String> {
    match json.get("type")?.as_str()? {
        "response.output_text.delta" => json.get("delta")?.as_str().map(|s| s.to_string()),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use std::net::SocketAddr;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use std::time::Duration;

    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::sync::mpsc;

    use super::*;

    // ---- mock SSE 服务器 --------------------------------------------------

    struct MockServer {
        addr: SocketAddr,
        hits: Arc<AtomicUsize>,
    }

    impl MockServer {
        /// 每个连接回调一次 behavior(hit_no)，返回依次写入的字节块（块间 20ms 间隔）。
        async fn start(behavior: impl Fn(usize) -> Vec<Vec<u8>> + Send + Sync + 'static) -> Self {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let addr = listener.local_addr().unwrap();
            let hits = Arc::new(AtomicUsize::new(0));
            let hits_task = hits.clone();
            let behavior = Arc::new(behavior);
            tokio::spawn(async move {
                loop {
                    let (mut sock, _) = match listener.accept().await {
                        Ok(pair) => pair,
                        Err(_) => break,
                    };
                    let behavior = behavior.clone();
                    let hits = hits_task.clone();
                    tokio::spawn(async move {
                        let mut buf = [0u8; 4096];
                        let mut filled = 0usize;
                        loop {
                            match sock.read(&mut buf[filled..]).await {
                                Ok(0) => break,
                                Ok(n) => {
                                    filled += n;
                                    if buf[..filled].windows(4).any(|w| w == b"\r\n\r\n") {
                                        break;
                                    }
                                }
                                Err(_) => break,
                            }
                        }
                        let hit = hits.fetch_add(1, Ordering::SeqCst) + 1;
                        let chunks = behavior(hit);
                        for chunk in &chunks {
                            if sock.write_all(chunk).await.is_err() {
                                break;
                            }
                            tokio::time::sleep(Duration::from_millis(20)).await;
                        }
                        if chunks.is_empty() {
                            // 不响应也不关闭：用于超时测试
                            tokio::time::sleep(Duration::from_secs(10)).await;
                        } else {
                            let _ = sock.shutdown().await;
                        }
                    });
                }
            });
            Self { addr, hits }
        }

        fn url(&self) -> String {
            format!("http://{}/v1", self.addr)
        }
    }

    fn one(resp: Vec<u8>) -> Vec<Vec<u8>> {
        vec![resp]
    }

    /// 构造 OpenAI-compatible chat SSE 响应（逐行 delta）。
    fn chat_sse(deltas: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        );
        for d in deltas {
            let line = format!(
                "data: {{\"choices\":[{{\"delta\":{{\"content\":{}}}}}]}}\n\n",
                serde_json::to_string(d).unwrap()
            );
            out.extend_from_slice(line.as_bytes());
        }
        out.extend_from_slice(b"data: [DONE]\n\n");
        out
    }

    /// 构造 OpenAI Responses API SSE 响应（逐行 delta）。
    fn responses_sse(deltas: &[&str]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend_from_slice(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        );
        for d in deltas {
            let line = format!(
                "event: response.output_text.delta\ndata: {{\"type\":\"response.output_text.delta\",\"delta\":{}}}\n\n",
                serde_json::to_string(d).unwrap()
            );
            out.extend_from_slice(line.as_bytes());
        }
        out.extend_from_slice(b"data: [DONE]\n\n");
        out
    }

    fn json_error(status: u16, message: &str) -> Vec<u8> {
        let body = serde_json::json!({ "error": { "message": message } }).to_string();
        let reason = match status {
            401 => "Unauthorized",
            429 => "Too Many Requests",
            _ => "Error",
        };
        format!(
            "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}"
        )
        .into_bytes()
    }

    fn client_for(base_url: String, style: ApiStyle) -> OpenAiCompatibleClient {
        let mut cfg = AnswerConfig::new(ProviderKind::Custom, base_url, "test-model", "test-key");
        cfg.total_timeout = Duration::from_secs(5);
        OpenAiCompatibleClient::new(cfg, style).unwrap()
    }

    fn sample_request() -> AnswerRequest {
        AnswerRequest {
            question_id: "q-1".into(),
            question: "请介绍一下你负责的项目".into(),
            recent_transcript: vec!["我们负责音频采集模块。".into()],
            profile_context: vec!["项目：WASAPI 音频采集，延迟 80ms".into()],
            response_language: "中文".into(),
        }
    }

    /// 收集事件直到通道关闭或等待超时。
    async fn collect(rx: &mut mpsc::Receiver<AnswerEvent>, wait: Duration) -> Vec<AnswerEvent> {
        let mut out = Vec::new();
        while let Ok(Some(ev)) = tokio::time::timeout(wait, rx.recv()).await {
            out.push(ev);
        }
        out
    }

    // ---- 单元测试：SSE 行解析与分段累加器 --------------------------------

    #[test]
    fn parses_sse_data_lines() {
        assert_eq!(sse_data(b"data: hello\n"), Some("hello"));
        assert_eq!(sse_data(b"data:{\"a\":1}\r\n"), Some("{\"a\":1}"));
        assert_eq!(sse_data(b"event: x\n"), None);
        assert_eq!(sse_data(b"\n"), None);
        assert_eq!(sse_data(b": comment\n"), None);
    }

    #[test]
    fn strips_bullets() {
        assert_eq!(strip_bullet("- 要点一"), "要点一");
        assert_eq!(strip_bullet("• 要点二"), "要点二");
        assert_eq!(strip_bullet("普通行"), "普通行");
    }

    #[tokio::test]
    async fn accumulator_orders_sections() {
        let (tx, mut rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let mut acc = SectionAccumulator::new();
        let mut emitted = false;
        for line in [
            "你好，",
            "[要点]",
            "- 要点一",
            "- 要点二",
            "[追问]",
            "- 追问一",
        ] {
            acc.push_line(line, &tx, &cancel, &mut emitted)
                .await
                .unwrap();
        }
        acc.finish(&tx, &cancel, &mut emitted).await.unwrap();
        drop(tx);
        let events: Vec<AnswerEvent> = {
            let mut v = Vec::new();
            while let Some(ev) = rx.recv().await {
                v.push(ev);
            }
            v
        };
        assert_eq!(
            events,
            vec![
                AnswerEvent::ShortAnswerDelta("你好，".into()),
                AnswerEvent::KeyPoints(vec!["要点一".into(), "要点二".into()]),
                AnswerEvent::FollowUps(vec!["追问一".into()]),
            ]
        );
    }

    #[tokio::test]
    async fn accumulator_degrades_after_malformed() {
        let (tx, mut rx) = mpsc::channel(16);
        let cancel = CancellationToken::new();
        let mut acc = SectionAccumulator::new();
        let mut emitted = false;
        acc.push_line("短答内容", &tx, &cancel, &mut emitted)
            .await
            .unwrap();
        acc.degrade();
        acc.push_line("[要点]", &tx, &cancel, &mut emitted)
            .await
            .unwrap();
        acc.push_line("普通降级行", &tx, &cancel, &mut emitted)
            .await
            .unwrap();
        acc.finish(&tx, &cancel, &mut emitted).await.unwrap();
        drop(tx);
        let events: Vec<AnswerEvent> = {
            let mut v = Vec::new();
            while let Some(ev) = rx.recv().await {
                v.push(ev);
            }
            v
        };
        // 短答保留；后续内容降级成普通要点
        assert_eq!(
            events,
            vec![
                AnswerEvent::ShortAnswerDelta("短答内容".into()),
                AnswerEvent::KeyPoints(vec!["普通降级行".into()]),
            ]
        );
    }

    // ---- 集成测试：mock server -------------------------------------------

    #[tokio::test]
    async fn streams_short_answer_key_points_and_follow_ups_in_order() {
        let server = MockServer::start(|_| {
            one(chat_sse(&[
                "你好，",
                "我负责音频采集模块。",
                "[要点]",
                "- 低延迟采集",
                "- 独立声道",
                "[追问]",
                "- 如何优化延迟？",
            ]))
        })
        .await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert_eq!(events.first(), Some(&AnswerEvent::Started));
        assert_eq!(events[1], AnswerEvent::ShortAnswerDelta("你好，".into()));
        assert_eq!(
            events[2],
            AnswerEvent::ShortAnswerDelta("我负责音频采集模块。".into())
        );
        assert!(
            events.contains(&AnswerEvent::KeyPoints(vec![
                "低延迟采集".into(),
                "独立声道".into()
            ])),
            "events: {events:?}"
        );
        assert!(events.contains(&AnswerEvent::FollowUps(vec!["如何优化延迟？".into()])));
        assert_eq!(events.last(), Some(&AnswerEvent::Completed));
    }

    #[tokio::test]
    async fn reassembles_utf8_split_across_chunks() {
        // 一行 SSE 数据在 TCP 分块中被拆开，且拆在 UTF-8 字符中间（"你好" = E4 BD A0 E5 A5 BD）
        let mut part1 = b"data: {\"choices\":[{\"delta\":{\"content\":\"".to_vec();
        part1.extend_from_slice(&[0xE4, 0xBD]);
        let mut part2 = vec![0xA0, 0xE5, 0xA5, 0xBD];
        part2.extend_from_slice(br#""}}]}"#);
        part2.extend_from_slice(b"\n\ndata: [DONE]\n\n");
        let server = MockServer::start(move |_| {
            let mut resp =
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                    .to_vec();
            resp.extend_from_slice(&part1);
            vec![resp, part2.clone()]
        })
        .await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert!(
            events.contains(&AnswerEvent::ShortAnswerDelta("你好".into())),
            "events: {events:?}"
        );
        assert_eq!(events.last(), Some(&AnswerEvent::Completed));
    }

    #[tokio::test]
    async fn rejects_401_without_retry() {
        let server = MockServer::start(|_| one(json_error(401, "invalid api key"))).await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert_eq!(server.hits.load(Ordering::SeqCst), 1, "认证失败不得重试");
        match events.last() {
            Some(AnswerEvent::Failed(msg)) => assert!(msg.contains("认证失败"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn rejects_429_without_retry() {
        let resp = b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 30\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{\"error\":{\"message\":\"rate limited\"}}".to_vec();
        let server = MockServer::start(move |_| one(resp.clone())).await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert_eq!(server.hits.load(Ordering::SeqCst), 1, "限流不得重试");
        match events.last() {
            Some(AnswerEvent::Failed(msg)) => assert!(msg.contains("限流"), "{msg}"),
            other => panic!("expected Failed, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn retries_once_when_stream_cut_before_content() {
        let cut = b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\nda".to_vec();
        let full = chat_sse(&["你好", "[要点]", "- 要点一", "[追问]", "- 追问一"]);
        let server = MockServer::start(move |hit| {
            if hit == 1 {
                one(cut.clone())
            } else {
                one(full.clone())
            }
        })
        .await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert_eq!(server.hits.load(Ordering::SeqCst), 2, "网络中断应重试一次");
        let deltas: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                AnswerEvent::ShortAnswerDelta(d) => Some(d.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(deltas, vec!["你好".to_string()]);
        assert_eq!(events.last(), Some(&AnswerEvent::Completed));
    }

    #[tokio::test]
    async fn keeps_content_when_stream_cut_after_deltas() {
        let cut = "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\ndata: {\"choices\":[{\"delta\":{\"content\":\"部分内容\"}}]}\n\n"
            .as_bytes()
            .to_vec();
        let server = MockServer::start(move |_| one(cut.clone())).await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert_eq!(server.hits.load(Ordering::SeqCst), 1);
        // 已收到内容则按成功结束，保留内容
        assert!(events.contains(&AnswerEvent::ShortAnswerDelta("部分内容".into())));
        assert_eq!(events.last(), Some(&AnswerEvent::Completed));
    }

    #[tokio::test]
    async fn cancellation_stops_further_events() {
        let chunks = [
            "data: {\"choices\":[{\"delta\":{\"content\":\"第一段\"}}]}\n\n"
                .as_bytes()
                .to_vec(),
            "data: {\"choices\":[{\"delta\":{\"content\":\"第二段\"}}]}\n\n"
                .as_bytes()
                .to_vec(),
            b"data: [DONE]\n\n".to_vec(),
        ];
        let server = MockServer::start(move |_| {
            let mut resp =
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n"
                    .to_vec();
            resp.extend_from_slice(&chunks[0]);
            vec![resp, chunks[1].clone(), chunks[2].clone()]
        })
        .await;
        let client = client_for(server.url(), ApiStyle::ChatCompletions);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel.clone())
            .await
            .unwrap();
        assert_eq!(rx.recv().await, Some(AnswerEvent::Started));
        assert_eq!(
            rx.recv().await,
            Some(AnswerEvent::ShortAnswerDelta("第一段".into()))
        );
        cancel.cancel();
        let rest = collect(&mut rx, Duration::from_millis(800)).await;
        assert!(rest.is_empty(), "取消后不得再发送任何事件: {rest:?}");
    }

    #[tokio::test]
    async fn total_timeout_fires_when_server_never_responds() {
        let server = MockServer::start(|_| Vec::new()).await;
        let mut cfg =
            AnswerConfig::new(ProviderKind::Custom, server.url(), "test-model", "test-key");
        cfg.total_timeout = Duration::from_millis(400);
        let client = OpenAiCompatibleClient::new(cfg, ApiStyle::ChatCompletions).unwrap();
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(3)).await;
        assert_eq!(events.first(), Some(&AnswerEvent::Started));
        match events.last() {
            Some(AnswerEvent::Failed(msg)) => assert!(msg.contains("超时"), "{msg}"),
            other => panic!("expected Failed(timeout), got {other:?}"),
        }
    }

    #[tokio::test]
    async fn openai_responses_api_style_streams_deltas() {
        let server = MockServer::start(|_| {
            one(responses_sse(&[
                "你好",
                "，这是 Responses API",
                "[要点]",
                "- 要点一",
            ]))
        })
        .await;
        let client = client_for(server.url(), ApiStyle::Responses);
        let cancel = CancellationToken::new();
        let mut rx = client
            .stream_answer(sample_request(), cancel)
            .await
            .unwrap();
        let events = collect(&mut rx, Duration::from_secs(2)).await;
        assert!(events.contains(&AnswerEvent::ShortAnswerDelta("你好".into())));
        assert!(events.contains(&AnswerEvent::ShortAnswerDelta(
            "，这是 Responses API".into()
        )));
        assert!(events.contains(&AnswerEvent::KeyPoints(vec!["要点一".into()])));
        assert_eq!(events.last(), Some(&AnswerEvent::Completed));
    }

    #[tokio::test]
    async fn rejects_non_http_base_url() {
        let cfg = AnswerConfig::new(ProviderKind::Custom, "ftp://bad", "m", "k");
        assert!(OpenAiCompatibleClient::new(cfg, ApiStyle::ChatCompletions).is_err());
    }
}
