//! 会话编排器：把音频/ASR（PipelineSource）、问题检测、资料匹配与答案生成
//! 连成完整流水线，并处理"新问题取消未固定旧答案 / 固定后排队"的竞争策略。
//!
//! 事件流：`capture-state -> audio-level -> transcript-pending -> transcript-final
//! -> question-detected -> answer-started -> answer-delta -> answer-completed`。

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use serde::Serialize;
use tokio::sync::mpsc;
use tauri::Emitter;

use crate::answer::{AnswerEvent, AnswerProvider, AnswerRequest, CancellationToken};
use crate::profile::importer::{default_profiles_dir, ProfileImporter};
use crate::profile::matcher::{MatchResult, ProfileMatcher};
use crate::question::detector::{QuestionConfig, QuestionDetector, TriggerLevel};
use crate::storage::{AnswerRow, Db, QuestionRow, TranscriptRow};

const MAX_RECENT_TRANSCRIPT: usize = 10;
const PROMPT_LANGUAGE: &str = "中文";
/// 单场会议最多选用 3 份启用资料。
const MAX_PROFILE_DOCS: usize = 3;

// ---------------------------------------------------------------------------
// 流水线事件（来源 -> 编排器）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct TranscriptInfo {
    pub id: String,
    pub source: String,
    pub text: String,
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub is_final: bool,
}

#[derive(Debug, Clone)]
pub enum PipelineEvent {
    CaptureState { source: String, active: bool },
    AudioLevel { source: String, rms: f32, peak: f32 },
    TranscriptPending(TranscriptInfo),
    TranscriptFinal(TranscriptInfo),
}

/// 音频 + 本地转写来源。生产实现为真实 WASAPI + Silero + Whisper 线程；
/// 测试使用脚本化 FakeSource。
pub trait PipelineSource: Send + Sync {
    fn events(&self) -> mpsc::Receiver<PipelineEvent>;
    fn start(&self) -> Result<(), String>;
    fn stop(&self);
}

// ---------------------------------------------------------------------------
// 领域事件（编排器 -> 前端）
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct QuestionInfo {
    pub id: String,
    pub text: String,
    pub confidence: f64,
    pub detected_at_ms: u64,
    pub level: String,
    pub source_segment_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct AnswerInfo {
    pub question_id: String,
    pub short_answer: String,
    pub key_points: Vec<String>,
    pub follow_ups: Vec<String>,
    pub status: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone)]
pub enum OrchestrationEvent {
    CaptureState { source: String, active: bool, at_ms: u64 },
    AudioLevel { source: String, rms: f32, peak: f32, at_ms: u64 },
    TranscriptPending(TranscriptInfo),
    TranscriptFinal(TranscriptInfo),
    QuestionDetected(QuestionInfo),
    AnswerStarted { question_id: String, at_ms: u64 },
    AnswerDelta { question_id: String, delta: String },
    AnswerCompleted(AnswerInfo),
}

/// 事件出口：测试用 channel，生产用 Tauri 事件发射。
pub trait EventSink: Send + Sync {
    fn emit(&self, ev: &OrchestrationEvent);
}

// ---------------------------------------------------------------------------
// 答案生成竞争策略状态
// ---------------------------------------------------------------------------

struct ActiveAnswer {
    question_id: String,
    pinned: Arc<AtomicBool>,
    cancel: CancellationToken,
}

#[derive(Clone)]
struct QueuedQuestion {
    id: String,
    text: String,
}

#[derive(Default)]
struct AnswerState {
    current: Option<ActiveAnswer>,
    queued: Option<QueuedQuestion>,
}

enum AnswerCommand {
    Generate(QueuedQuestion),
    GenerateLast,
    Completed(String),
    PinCurrent,
    CancelCurrent,
}

/// 会话控制句柄：与运行中的编排器共享停止/固定/重新生成能力（供 Tauri 命令使用）。
#[derive(Clone)]
pub struct SessionControl {
    stop: CancellationToken,
    answer_tx: mpsc::Sender<AnswerCommand>,
}

impl SessionControl {
    pub fn stop(&self) {
        self.stop.cancel();
    }

    /// 固定当前答案（后续新问题进入等待队列）。
    pub async fn pin_current(&self) {
        let _ = self.answer_tx.send(AnswerCommand::PinCurrent).await;
    }

    /// 手动/重新生成：使用最近检测到的问题（Maybe 级别由用户点击生成）。
    pub async fn generate_last(&self) {
        let _ = self.answer_tx.send(AnswerCommand::GenerateLast).await;
    }

    /// 取消当前答案生成（保留已收到的内容，标记为 cancelled）。
    pub async fn cancel_current(&self) {
        let _ = self.answer_tx.send(AnswerCommand::CancelCurrent).await;
    }

    pub fn stop_token(&self) -> CancellationToken {
        self.stop.clone()
    }
}

// ---------------------------------------------------------------------------
// 编排器
// ---------------------------------------------------------------------------

pub struct Orchestrator {
    source: Arc<dyn PipelineSource>,
    detector: Mutex<QuestionDetector>,
    matcher: Mutex<ProfileMatcher>,
    provider: Arc<dyn AnswerProvider>,
    db: Db,
    sink: Arc<dyn EventSink>,
    stop: CancellationToken,
    answer_state: Arc<Mutex<AnswerState>>,
    answer_tx: mpsc::Sender<AnswerCommand>,
    answer_rx: Mutex<Option<mpsc::Receiver<AnswerCommand>>>,
    recent_transcript: Mutex<VecDeque<String>>,
    last_question: Mutex<Option<QuestionInfo>>,
    last_level_log: Mutex<u64>,
    meeting_id: String,
    now: fn() -> u64,
}

impl Orchestrator {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Arc<dyn PipelineSource>,
        provider: Box<dyn AnswerProvider>,
        db: Db,
        sink: Arc<dyn EventSink>,
        meeting_id: String,
    ) -> (Self, SessionControl) {
        let (answer_tx, answer_rx) = mpsc::channel(16);
        let stop = CancellationToken::new();
        let orch = Self {
            source,
            detector: Mutex::new(QuestionDetector::new(QuestionConfig::default())),
            matcher: Mutex::new(ProfileMatcher::new()),
            provider: Arc::from(provider),
            db,
            sink,
            stop: stop.clone(),
            answer_state: Arc::new(Mutex::new(AnswerState::default())),
            answer_tx: answer_tx.clone(),
            answer_rx: Mutex::new(Some(answer_rx)),
            recent_transcript: Mutex::new(VecDeque::new()),
            last_question: Mutex::new(None),
            last_level_log: Mutex::new(0),
            meeting_id,
            now: crate::storage::retention::now_ms,
        };
        let ctl = SessionControl { stop, answer_tx };
        (orch, ctl)
    }

    /// 加载启用资料（最多 3 份）到匹配器；资料来自本机 profiles.json（Task 6 数据源）。
    pub fn load_enabled_profiles(&self) {
        let profiles = ProfileImporter::new(default_profiles_dir())
            .ok()
            .and_then(|imp| imp.list().ok())
            .unwrap_or_default();
        let docs: Vec<(String, String, String)> = profiles
            .into_iter()
            .filter(|p| p.enabled)
            .take(MAX_PROFILE_DOCS)
            .map(|p| (p.id, p.title, p.text))
            .collect();
        self.matcher.lock().unwrap().set_docs(docs);
    }

    /// 手动/重新生成：使用最近检测到的问题（Maybe 级别由用户点击生成）。
    async fn generate_last_inner(&self) {
        let question = self.last_question.lock().unwrap().clone();
        if let Some(q) = question {
            let _ = self
                .answer_tx
                .send(AnswerCommand::Generate(QueuedQuestion {
                    id: q.id,
                    text: q.text,
                }))
                .await;
        }
    }

    /// 主循环：消费流水线事件与答案命令，直到停止。
    pub async fn run(self) -> Result<(), String> {
        eprintln!("[session] orchestrator run: start");
        self.source.start()?;
        eprintln!("[session] orchestrator run: source started");
        let mut source_rx = self.source.events();
        let mut answer_rx = self
            .answer_rx
            .lock()
            .unwrap()
            .take()
            .expect("run 只能执行一次");
        let now = (self.now)();
        self.sink
            .emit(&OrchestrationEvent::CaptureState {
                source: "system".into(),
                active: true,
                at_ms: now,
            });
        eprintln!("[session] orchestrator run: entering event loop");
        loop {
            tokio::select! {
                _ = self.stop.cancelled() => break,
                ev = source_rx.recv() => match ev {
                    Some(e) => self.handle_pipeline(e).await,
                    None => {
                        eprintln!("[session] source channel closed");
                        break;
                    }
                },
                cmd = answer_rx.recv() => match cmd {
                    Some(AnswerCommand::Generate(q)) => self.generate(q).await,
                    Some(AnswerCommand::GenerateLast) => self.generate_last_inner().await,
                    Some(AnswerCommand::Completed(qid)) => self.on_answer_completed(&qid).await,
                    Some(AnswerCommand::PinCurrent) => self.pin_current_inner(),
                    Some(AnswerCommand::CancelCurrent) => self.cancel_current_inner(),
                    None => break,
                },
            }
        }
        eprintln!("[session] orchestrator run: stopping");
        // 停止：取消正在生成的答案并结束会议
        {
            let mut st = self.answer_state.lock().unwrap();
            if let Some(act) = st.current.take() {
                act.cancel.cancel();
            }
            st.queued = None;
        }
        self.source.stop();
        let _ = self.db.end_meeting(&self.meeting_id, (self.now)());
        self.sink
            .emit(&OrchestrationEvent::CaptureState {
                source: "system".into(),
                active: false,
                at_ms: (self.now)(),
            });
        Ok(())
    }

    async fn handle_pipeline(&self, ev: PipelineEvent) {
        let now = (self.now)();
        match &ev {
            PipelineEvent::AudioLevel { rms, .. } => {
                // 诊断：每秒打印一次音量（证明事件到达编排器）
                let now_sec = now / 1000;
                let mut last = self.last_level_log.lock().unwrap();
                if now_sec != *last {
                    *last = now_sec;
                    drop(last);
                    eprintln!("[orchestrator] audio-level rms={rms:.4}");
                }
            }
            other => eprintln!("[orchestrator] pipeline event: {other:?}"),
        }
        match ev {
            PipelineEvent::CaptureState { source, active } => {
                self.sink.emit(&OrchestrationEvent::CaptureState {
                    source,
                    active,
                    at_ms: now,
                });
            }
            PipelineEvent::AudioLevel { source, rms, peak } => {
                self.sink.emit(&OrchestrationEvent::AudioLevel {
                    source,
                    rms,
                    peak,
                    at_ms: now,
                });
            }
            PipelineEvent::TranscriptPending(info) => {
                self.sink.emit(&OrchestrationEvent::TranscriptPending(info));
            }
            PipelineEvent::TranscriptFinal(info) => {
                self.sink.emit(&OrchestrationEvent::TranscriptFinal(info.clone()));
                let _ = self.db.insert_transcript_segment(
                    &self.meeting_id,
                    &TranscriptRow {
                        id: info.id.clone(),
                        speaker: if info.source == "microphone" {
                            "local".into()
                        } else {
                            "remote".into()
                        },
                        text: info.text.clone(),
                        started_at_ms: info.started_at_ms,
                        ended_at_ms: info.ended_at_ms,
                        is_final: true,
                    },
                );
                // 仅系统音频（remote）进入问题检测
                if info.source == "system" {
                    if let Some(q) = self.detect_question(&info) {
                        let _ = self.answer_tx.send(AnswerCommand::Generate(q)).await;
                    }
                }
            }
        }
    }

    fn detect_question(&self, info: &TranscriptInfo) -> Option<QueuedQuestion> {
        let mut detector = self.detector.lock().unwrap();
        detector.push_final(&info.text, info.started_at_ms, info.ended_at_ms);
        let question = detector.check((self.now)())?;
        let level = match question.level {
            TriggerLevel::Auto => "auto",
            TriggerLevel::Maybe => "maybe",
        };
        let qinfo = QuestionInfo {
            id: question.id.clone(),
            text: question.text.clone(),
            confidence: question.confidence,
            detected_at_ms: question.detected_at_ms,
            level: level.to_string(),
            source_segment_ids: vec![info.id.clone()],
        };
        *self.last_question.lock().unwrap() = Some(qinfo.clone());
        self.sink
            .emit(&OrchestrationEvent::QuestionDetected(qinfo.clone()));
        let mut recent = self.recent_transcript.lock().unwrap();
        recent.push_back(info.text.clone());
        while recent.len() > MAX_RECENT_TRANSCRIPT {
            recent.pop_front();
        }
        drop(recent);
        if level == "auto" {
            Some(QueuedQuestion {
                id: qinfo.id,
                text: qinfo.text,
            })
        } else {
            None
        }
    }

    /// 开始一次答案生成（取消未固定的旧生成；已固定则进入单项队列）。
    async fn generate(&self, question: QueuedQuestion) {
        let cancel = {
            let mut st = self.answer_state.lock().unwrap();
            match &st.current {
                Some(act) if act.pinned.load(Ordering::SeqCst) => {
                    if st.queued.is_none() {
                        st.queued = Some(question);
                    }
                    return;
                }
                Some(act) => {
                    act.cancel.cancel();
                }
                None => {}
            }
            let cancel = CancellationToken::new();
            let pinned = Arc::new(AtomicBool::new(false));
            st.current = Some(ActiveAnswer {
                question_id: question.id.clone(),
                pinned,
                cancel: cancel.clone(),
            });
            cancel
        };

        let matched = self.match_context(&question.text);
        let recent = self
            .recent_transcript
            .lock()
            .unwrap()
            .iter()
            .cloned()
            .collect::<Vec<_>>();
        let request = AnswerRequest {
            question_id: question.id.clone(),
            question: question.text.clone(),
            recent_transcript: recent,
            profile_context: matched,
            response_language: PROMPT_LANGUAGE.into(),
        };
        let provider = self.provider.clone();
        let sink = self.sink.clone();
        let answer_tx = self.answer_tx.clone();
        let now = self.now;
        let db = self.db.clone();
        let meeting_id = self.meeting_id.clone();
        let qid = question.id.clone();
        let qtext = question.text.clone();
        tokio::spawn(async move {
            let started_at = now();
            let rx = provider.stream_answer(request, cancel.clone()).await;
            let mut short_answer = String::new();
            let mut key_points: Vec<String> = Vec::new();
            let mut follow_ups: Vec<String> = Vec::new();
            let mut status = "complete".to_string();
            match rx {
                Ok(mut rx) => {
                    sink.emit(&OrchestrationEvent::AnswerStarted {
                        question_id: qid.clone(),
                        at_ms: started_at,
                    });
                    loop {
                        match rx.recv().await {
                            Some(AnswerEvent::Started) => {}
                            Some(AnswerEvent::ShortAnswerDelta(d)) => {
                                short_answer.push_str(&d);
                                sink.emit(&OrchestrationEvent::AnswerDelta {
                                    question_id: qid.clone(),
                                    delta: d,
                                });
                            }
                            Some(AnswerEvent::KeyPoints(pts)) => key_points = pts,
                            Some(AnswerEvent::FollowUps(fu)) => follow_ups = fu,
                            Some(AnswerEvent::Completed) => break,
                            Some(AnswerEvent::Failed(msg)) => {
                                status = "failed".into();
                                tracing::warn!("答案生成失败：{msg}");
                                break;
                            }
                            None => {
                                if cancel.is_cancelled() {
                                    return;
                                }
                                if short_answer.trim().is_empty() {
                                    status = "failed".into();
                                }
                                break;
                            }
                        }
                    }
                }
                Err(e) => {
                    status = "failed".into();
                    tracing::warn!("答案请求失败：{e}");
                }
            }
            if cancel.is_cancelled() {
                return;
            }
            let finished_at = now();
            sink.emit(&OrchestrationEvent::AnswerCompleted(AnswerInfo {
                question_id: qid.clone(),
                short_answer: short_answer.clone(),
                key_points: key_points.clone(),
                follow_ups: follow_ups.clone(),
                status: status.clone(),
                created_at_ms: finished_at,
            }));
            let _ = db.insert_question(
                &meeting_id,
                &QuestionRow {
                    id: qid.clone(),
                    text: qtext,
                    confidence: 1.0,
                    detected_at_ms: started_at,
                },
            );
            let _ = db.insert_answer(&AnswerRow {
                id: format!("answer-{qid}"),
                question_id: qid.clone(),
                short_answer,
                key_points,
                follow_ups,
                status,
                created_at_ms: finished_at,
            });
            let _ = answer_tx.send(AnswerCommand::Completed(qid)).await;
        });
    }

    async fn on_answer_completed(&self, qid: &str) {
        let next = {
            let mut st = self.answer_state.lock().unwrap();
            let is_current = st
                .current
                .as_ref()
                .map(|a| a.question_id == qid)
                .unwrap_or(false);
            if !is_current {
                return;
            }
            st.current = None;
            st.queued.take()
        };
        if let Some(q) = next {
            let _ = self.answer_tx.send(AnswerCommand::Generate(q)).await;
        }
    }

    fn pin_current_inner(&self) {
        let st = self.answer_state.lock().unwrap();
        if let Some(act) = st.current.as_ref() {
            act.pinned.store(true, Ordering::SeqCst);
        }
    }

    /// 取消当前答案生成：通知生成任务停止并向前端发出 cancelled 状态。
    fn cancel_current_inner(&self) {
        let cancel = {
            let st = self.answer_state.lock().unwrap();
            st.current.as_ref().map(|act| act.cancel.clone())
        };
        if let Some(cancel) = cancel {
            cancel.cancel();
            let qid = self
                .answer_state
                .lock()
                .unwrap()
                .current
                .as_ref()
                .map(|act| act.question_id.clone())
                .unwrap_or_default();
            let now = (self.now)();
            self.sink.emit(&OrchestrationEvent::AnswerCompleted(AnswerInfo {
                question_id: qid,
                short_answer: String::new(),
                key_points: Vec::new(),
                follow_ups: Vec::new(),
                status: "cancelled".into(),
                created_at_ms: now,
            }));
        }
    }

    fn match_context(&self, question: &str) -> Vec<String> {
        let matcher = self.matcher.lock().unwrap();
        let results: Vec<MatchResult> = matcher.match_chunks(question);
        results
            .into_iter()
            .map(|r| format!("【{}】{}", r.doc_title, r.chunk_text))
            .collect()
    }
}

// ---------------------------------------------------------------------------
// Tauri 事件出口
// ---------------------------------------------------------------------------

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CaptureStatePayload {
    source: String,
    active: bool,
    at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AudioLevelPayload {
    source: String,
    rms: f32,
    peak: f32,
    at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct TranscriptPayload {
    id: String,
    speaker: String,
    text: String,
    started_at_ms: u64,
    ended_at_ms: u64,
    is_final: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct QuestionPayload {
    id: String,
    source_segment_ids: Vec<String>,
    normalized_text: String,
    confidence: f64,
    detected_at_ms: u64,
    level: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnswerStartedPayload {
    question_id: String,
    at_ms: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnswerDeltaPayload {
    question_id: String,
    delta: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AnswerCompletedPayload {
    question_id: String,
    short_answer: String,
    key_points: Vec<String>,
    follow_ups: Vec<String>,
    status: String,
    created_at_ms: u64,
}

/// 生产事件出口：通过 Tauri AppHandle 向前端发射稳定事件。
pub struct TauriSink {
    app: tauri::AppHandle,
}

impl TauriSink {
    pub fn new(app: tauri::AppHandle) -> Self {
        Self { app }
    }
}

impl EventSink for TauriSink {
    fn emit(&self, ev: &OrchestrationEvent) {
        let result = match ev {
            OrchestrationEvent::CaptureState { source, active, at_ms } => self.app.emit(
                "capture-state",
                CaptureStatePayload {
                    source: source.clone(),
                    active: *active,
                    at_ms: *at_ms,
                },
            ),
            OrchestrationEvent::AudioLevel { source, rms, peak, at_ms } => self.app.emit(
                "audio-level",
                AudioLevelPayload {
                    source: source.clone(),
                    rms: *rms,
                    peak: *peak,
                    at_ms: *at_ms,
                },
            ),
            OrchestrationEvent::TranscriptPending(info)
            | OrchestrationEvent::TranscriptFinal(info) => {
                let event = if matches!(ev, OrchestrationEvent::TranscriptPending(_)) {
                    "transcript-pending"
                } else {
                    "transcript-final"
                };
                self.app.emit(
                    event,
                    TranscriptPayload {
                        id: info.id.clone(),
                        speaker: if info.source == "microphone" {
                            "local".into()
                        } else {
                            "remote".into()
                        },
                        text: info.text.clone(),
                        started_at_ms: info.started_at_ms,
                        ended_at_ms: info.ended_at_ms,
                        is_final: info.is_final,
                    },
                )
            }
            OrchestrationEvent::QuestionDetected(q) => self.app.emit(
                "question-detected",
                QuestionPayload {
                    id: q.id.clone(),
                    source_segment_ids: q.source_segment_ids.clone(),
                    normalized_text: q.text.clone(),
                    confidence: q.confidence,
                    detected_at_ms: q.detected_at_ms,
                    level: q.level.clone(),
                },
            ),
            OrchestrationEvent::AnswerStarted { question_id, at_ms } => self.app.emit(
                "answer-started",
                AnswerStartedPayload {
                    question_id: question_id.clone(),
                    at_ms: *at_ms,
                },
            ),
            OrchestrationEvent::AnswerDelta { question_id, delta } => self.app.emit(
                "answer-delta",
                AnswerDeltaPayload {
                    question_id: question_id.clone(),
                    delta: delta.clone(),
                },
            ),
            OrchestrationEvent::AnswerCompleted(a) => self.app.emit(
                "answer-completed",
                AnswerCompletedPayload {
                    question_id: a.question_id.clone(),
                    short_answer: a.short_answer.clone(),
                    key_points: a.key_points.clone(),
                    follow_ups: a.follow_ups.clone(),
                    status: a.status.clone(),
                    created_at_ms: a.created_at_ms,
                },
            ),
        };
        if let Err(e) = result {
            eprintln!("[sink] emit failed: {e}");
        }
    }
}

// ---------------------------------------------------------------------------
// 测试
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::answer::AnswerError;
    use std::sync::mpsc as std_mpsc;

    /// 脚本化事件来源：测试通过 `push` 注入流水线事件。
    struct FakeSource {
        script: Arc<Mutex<std_mpsc::Receiver<PipelineEvent>>>,
        started: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
    }

    impl FakeSource {
        fn new() -> (Arc<Self>, std_mpsc::Sender<PipelineEvent>) {
            let (tx, rx) = std_mpsc::channel();
            let src = Arc::new(Self {
                script: Arc::new(Mutex::new(rx)),
                started: Arc::new(AtomicBool::new(false)),
                stopped: Arc::new(AtomicBool::new(false)),
            });
            (src, tx)
        }
    }

    impl PipelineSource for FakeSource {
        fn events(&self) -> mpsc::Receiver<PipelineEvent> {
            let (tx, rx) = mpsc::channel(64);
            let script = self.script.clone();
            std::thread::spawn(move || {
                loop {
                    let ev = script.lock().unwrap().recv();
                    match ev {
                        Ok(e) => {
                            if tx.blocking_send(e).is_err() {
                                break;
                            }
                        }
                        Err(_) => break,
                    }
                }
            });
            rx
        }

        fn start(&self) -> Result<(), String> {
            self.started.store(true, Ordering::SeqCst);
            Ok(())
        }

        fn stop(&self) {
            self.stopped.store(true, Ordering::SeqCst);
        }
    }

    /// 脚本化答案 Provider：每个流式请求注册独立脚本队列，测试脚本路由到最近注册的请求。
    enum ProviderScript {
        Emit(AnswerEvent),
    }

    struct FakeProvider {
        queues: Arc<Mutex<Vec<std_mpsc::Sender<ProviderScript>>>>,
    }

    impl FakeProvider {
        fn new() -> (Box<Self>, std_mpsc::Sender<ProviderScript>) {
            let queues: Arc<Mutex<Vec<std_mpsc::Sender<ProviderScript>>>> =
                Arc::new(Mutex::new(Vec::new()));
            let (script_tx, script_rx) = std_mpsc::channel();
            let q = queues.clone();
            std::thread::spawn(move || {
                while let Ok(script) = script_rx.recv() {
                    let latest = q.lock().unwrap().last().cloned();
                    if let Some(tx) = latest {
                        let _ = tx.send(script);
                    }
                }
            });
            (Box::new(Self { queues }), script_tx)
        }
    }

    impl AnswerProvider for FakeProvider {
        fn stream_answer<'a>(
            &'a self,
            _request: AnswerRequest,
            cancel: CancellationToken,
        ) -> futures_util::future::BoxFuture<
            'a,
            Result<mpsc::Receiver<AnswerEvent>, AnswerError>,
        > {
            Box::pin(async move {
                let (tx, rx) = mpsc::channel(32);
                let (script_tx, script_rx) = std_mpsc::channel();
                self.queues.lock().unwrap().push(script_tx);
                let script_rx = Arc::new(Mutex::new(script_rx));
                tokio::spawn(async move {
                    loop {
                        if cancel.is_cancelled() {
                            return;
                        }
                        let script_rx = script_rx.clone();
                        let item = tokio::task::spawn_blocking(move || {
                            script_rx
                                .lock()
                                .unwrap()
                                .recv_timeout(std::time::Duration::from_millis(30))
                        })
                        .await
                        .ok()
                        .and_then(|r| r.ok());
                        if let Some(ProviderScript::Emit(ev)) = item {
                            if tx.send(ev).await.is_err() {
                                return;
                            }
                        }
                    }
                });
                Ok(rx)
            })
        }
    }

    struct SinkChannel {
        tx: std_mpsc::Sender<OrchestrationEvent>,
    }

    impl EventSink for SinkChannel {
        fn emit(&self, ev: &OrchestrationEvent) {
            let _ = self.tx.send(ev.clone());
        }
    }

    fn new_orchestrator(
        source: Arc<FakeSource>,
        provider: Box<dyn AnswerProvider>,
    ) -> (Orchestrator, SessionControl, std_mpsc::Receiver<OrchestrationEvent>) {
        let (sink_tx, sink_rx) = std_mpsc::channel();
        let db = Db::open_in_memory().unwrap();
        db.create_meeting("m1", 1_000).unwrap();
        let (orch, ctl) = Orchestrator::new(
            source,
            provider,
            db,
            Arc::new(SinkChannel { tx: sink_tx }),
            "m1".into(),
        );
        (orch, ctl, sink_rx)
    }

    fn final_event(source: &str, text: &str, start_ago_ms: u64, end_ago_ms: u64) -> PipelineEvent {
        let now = crate::storage::retention::now_ms();
        let start = now.saturating_sub(start_ago_ms);
        let end = now.saturating_sub(end_ago_ms);
        PipelineEvent::TranscriptFinal(TranscriptInfo {
            id: format!("seg-{start}"),
            source: source.into(),
            text: text.into(),
            started_at_ms: start,
            ended_at_ms: end,
            is_final: true,
        })
    }

    fn recv_until(
        rx: &std_mpsc::Receiver<OrchestrationEvent>,
        deadline: std::time::Duration,
    ) -> Vec<OrchestrationEvent> {
        let mut out = Vec::new();
        loop {
            match rx.recv_timeout(deadline) {
                Ok(ev) => out.push(ev),
                Err(_) => break,
            }
        }
        out
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn orchestrates_full_event_flow_in_order() {
        let (source, src_tx) = FakeSource::new();
        let (provider, prov_tx) = FakeProvider::new();
        let (orch, ctl, sink_rx) = new_orchestrator(source.clone(), provider);
        orch.load_enabled_profiles();

        let run = tokio::spawn(async move { orch.run().await });

        std::thread::sleep(std::time::Duration::from_millis(100));
        src_tx
            .send(PipelineEvent::AudioLevel {
                source: "system".into(),
                rms: 0.2,
                peak: 0.5,
            })
            .unwrap();
        src_tx
            .send(PipelineEvent::TranscriptPending(TranscriptInfo {
                id: "seg-p1".into(),
                source: "system".into(),
                text: "请介绍一下你负责的".into(),
                started_at_ms: 2000,
                ended_at_ms: 2800,
                is_final: false,
            }))
            .unwrap();
        src_tx
            .send(final_event("system", "请介绍一下你负责的项目", 4000, 2500))
            .unwrap();
        // 等待 question-detected -> answer-started 到达后再放行 provider 脚本
        // （脚本路由到最近注册的请求队列）
        let mut events: Vec<OrchestrationEvent> = Vec::new();
        loop {
            match sink_rx.recv_timeout(std::time::Duration::from_millis(1500)) {
                Ok(ev) => {
                    let started = matches!(&ev, OrchestrationEvent::AnswerStarted { .. });
                    events.push(ev);
                    if started {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::Started))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::ShortAnswerDelta(
                "我负责音频模块\n".into(),
            )))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::KeyPoints(vec!["要点一".into()])))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::Completed))
            .unwrap();
        loop {
            match sink_rx.recv_timeout(std::time::Duration::from_millis(1500)) {
                Ok(ev) => events.push(ev),
                Err(_) => break,
            }
        }

        let kinds: Vec<&str> = events
            .iter()
            .map(|e| match e {
                OrchestrationEvent::CaptureState { .. } => "capture-state",
                OrchestrationEvent::AudioLevel { .. } => "audio-level",
                OrchestrationEvent::TranscriptPending(_) => "transcript-pending",
                OrchestrationEvent::TranscriptFinal(_) => "transcript-final",
                OrchestrationEvent::QuestionDetected(_) => "question-detected",
                OrchestrationEvent::AnswerStarted { .. } => "answer-started",
                OrchestrationEvent::AnswerDelta { .. } => "answer-delta",
                OrchestrationEvent::AnswerCompleted(_) => "answer-completed",
            })
            .collect();
        assert_eq!(
            kinds,
            vec![
                "capture-state",
                "audio-level",
                "transcript-pending",
                "transcript-final",
                "question-detected",
                "answer-started",
                "answer-delta",
                "answer-completed",
            ],
            "事件顺序错误: {kinds:?}"
        );
        match events.last().unwrap() {
            OrchestrationEvent::AnswerCompleted(a) => {
                assert_eq!(a.status, "complete");
                assert!(a.short_answer.contains("我负责音频模块"));
                assert_eq!(a.key_points, vec!["要点一".to_string()]);
            }
            other => panic!("expected AnswerCompleted, got {other:?}"),
        }
        ctl.stop();
        drop(src_tx);
        let _ = run.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn new_question_cancels_unpinned_ongoing_answer() {
        let (source, src_tx) = FakeSource::new();
        let (provider, prov_tx) = FakeProvider::new();
        let (orch, ctl, sink_rx) = new_orchestrator(source.clone(), provider);

        let run = tokio::spawn(async move { orch.run().await });

        std::thread::sleep(std::time::Duration::from_millis(100));
        src_tx
            .send(final_event("system", "请介绍一下你负责的项目", 4000, 2500))
            .unwrap();
        prov_tx.send(ProviderScript::Emit(AnswerEvent::Started)).unwrap();
        // 第二个问题到来时第一个答案仍在生成（脚本挂起）
        std::thread::sleep(std::time::Duration::from_millis(300));
        src_tx
            .send(final_event("system", "那项目的音频延迟怎么优化的", 2000, 500))
            .unwrap();

        let events = recv_until(&sink_rx, std::time::Duration::from_millis(1200));
        let started: Vec<String> = events
            .iter()
            .filter_map(|e| match e {
                OrchestrationEvent::AnswerStarted { question_id, .. } => Some(question_id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(started.len(), 2, "应取消旧生成并开始新生成: {events:?}");
        assert_ne!(started[0], started[1]);
        ctl.stop();
        drop(src_tx);
        drop(prov_tx);
        let _ = run.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn pinned_answer_queues_next_question() {
        let (source, src_tx) = FakeSource::new();
        let (provider, prov_tx) = FakeProvider::new();
        let (orch, ctl, sink_rx) = new_orchestrator(source.clone(), provider);

        let run = tokio::spawn(async move { orch.run().await });

        std::thread::sleep(std::time::Duration::from_millis(100));
        // 问题 1 开始生成（挂起不完成）
        src_tx
            .send(final_event("system", "请介绍一下你负责的项目", 4000, 2500))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        ctl.pin_current().await; // 固定当前答案
        // 问题 2 到来 -> 排队（不取消问题 1）
        src_tx
            .send(final_event("system", "项目的音频延迟怎么优化", 2000, 500))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        // 问题 1 完成
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::ShortAnswerDelta(
                "问题一答案\n".into(),
            )))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::Completed))
            .unwrap();
        // 等待问题 1 的 AnswerCompleted 到达（此时 q1 的 provider 脚本循环已退出，
        // 避免其误食问题 2 的脚本条目）
        let mut all: Vec<OrchestrationEvent> = Vec::new();
        loop {
            match sink_rx.recv_timeout(std::time::Duration::from_millis(1500)) {
                Ok(ev) => {
                    let is_completed = matches!(&ev, OrchestrationEvent::AnswerCompleted(_));
                    all.push(ev);
                    if is_completed {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        std::thread::sleep(std::time::Duration::from_millis(400));
        // 问题 2 开始生成并完成
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::ShortAnswerDelta(
                "问题二答案\n".into(),
            )))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::Completed))
            .unwrap();
        loop {
            match sink_rx.recv_timeout(std::time::Duration::from_millis(1500)) {
                Ok(ev) => all.push(ev),
                Err(_) => break,
            }
        }

        let started: Vec<String> = all
            .iter()
            .filter_map(|e| match e {
                OrchestrationEvent::AnswerStarted { question_id, .. } => Some(question_id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(started.len(), 2, "两个问题都应生成: {all:?}");
        let completed: Vec<String> = all
            .iter()
            .filter_map(|e| match e {
                OrchestrationEvent::AnswerCompleted(a) => Some(a.question_id.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(completed.len(), 2, "两个答案都应完成: {all:?}");
        ctl.stop();
        drop(src_tx);
        drop(prov_tx);
        let _ = run.await.unwrap();
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn stop_cancels_running_answer_and_closes_capture() {
        let (source, src_tx) = FakeSource::new();
        let (provider, prov_tx) = FakeProvider::new();
        let (orch, ctl, sink_rx) = new_orchestrator(source.clone(), provider);

        let run = tokio::spawn(async move { orch.run().await });
        std::thread::sleep(std::time::Duration::from_millis(100));
        src_tx
            .send(final_event("system", "请介绍一下你负责的项目", 4000, 2500))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(300));
        ctl.stop(); // 停止：取消生成
        std::thread::sleep(std::time::Duration::from_millis(300));
        let _ = run.await.unwrap();
        assert!(
            source.stopped.load(Ordering::SeqCst),
            "流水线应被关闭"
        );
        let tail = recv_until(&sink_rx, std::time::Duration::from_millis(300));
        assert!(
            tail.last()
                .map(|e| matches!(
                    e,
                    OrchestrationEvent::CaptureState { active: false, .. }
                ))
                .unwrap_or(false),
            "停止后应发送采集结束事件"
        );
        drop(prov_tx);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn maybe_question_requires_manual_generation() {
        let (source, src_tx) = FakeSource::new();
        let (provider, prov_tx) = FakeProvider::new();
        let (orch, ctl, sink_rx) = new_orchestrator(source.clone(), provider);

        let run = tokio::spawn(async move { orch.run().await });
        std::thread::sleep(std::time::Duration::from_millis(100));
        // "？" 结尾 -> 0.5 置信度 -> Maybe 级别，不自动生成
        src_tx
            .send(final_event("system", "你们项目遇到最大的困难？", 4000, 2500))
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(400));
        let events = recv_until(&sink_rx, std::time::Duration::from_millis(200));
        assert!(
            events
                .iter()
                .any(|e| matches!(e, OrchestrationEvent::QuestionDetected(q) if q.level == "maybe")),
            "应发出 maybe 级别问题: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, OrchestrationEvent::AnswerStarted { .. })),
            "maybe 问题不应自动生成"
        );
        // 用户点击生成
        ctl.generate_last().await;
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::ShortAnswerDelta(
                "手动生成的答案\n".into(),
            )))
            .unwrap();
        prov_tx
            .send(ProviderScript::Emit(AnswerEvent::Completed))
            .unwrap();
        let events2 = recv_until(&sink_rx, std::time::Duration::from_millis(1000));
        assert!(
            events2
                .iter()
                .any(|e| matches!(e, OrchestrationEvent::AnswerStarted { .. })),
            "手动生成应开始: {events2:?}"
        );
        ctl.stop();
        drop(src_tx);
        drop(prov_tx);
        let _ = run.await.unwrap();
    }
}
