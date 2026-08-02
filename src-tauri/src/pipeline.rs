//! 生产流水线：WASAPI 采集（系统 + 可选麦克风，独立 channel 不混合）
//! -> Silero VAD 分段 -> Whisper 本地转写 -> PipelineEvent。
//!
//! - pending：说话过程中每 800ms 对最近 1.6s 滚动窗口生成临时文本
//! - final：VAD 完成一个语音片段后生成最终文本（只有 final 进入问题检测）

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc as std_mpsc, Arc, Mutex};
use std::time::Duration;

use tokio::sync::mpsc;

use crate::asr::whisper_worker::WhisperWorker;
use crate::audio::{AudioFrame, AudioSource};
use crate::session::{PipelineEvent, PipelineSource, TranscriptInfo};
use crate::vad::segmenter::{SegmentEvent, VadConfig, VadSegmenter};
use crate::vad::silero::SileroVad;

// Silero v6 ONNX 要求输入精确 512 样本（32ms @16kHz）。
const FRAME_SAMPLES: usize = 512;
const PENDING_INTERVAL_MS: u64 = 800;
const ROLLING_CAP_SAMPLES: usize = 16_000 * 1_600 / 1_000;
const MODEL_CANDIDATES: &[&str] = &["ggml-large-v3-turbo-q5_0.bin", "ggml-base-q5_0.bin"];

/// 生产音频/转写来源。事件出口为 tokio channel，采集与转写在专用线程运行。
pub struct RealPipeline {
    mic_enabled: bool,
    tx: mpsc::Sender<PipelineEvent>,
    rx: Mutex<Option<mpsc::Receiver<PipelineEvent>>>,
    started: AtomicBool,
    stop_flag: Arc<AtomicBool>,
    threads: Mutex<Vec<std::thread::JoinHandle<()>>>,
}

impl RealPipeline {
    pub fn new(mic_enabled: bool) -> Self {
        let (tx, rx) = mpsc::channel(64);
        Self {
            mic_enabled,
            tx,
            rx: Mutex::new(Some(rx)),
            started: AtomicBool::new(false),
            stop_flag: Arc::new(AtomicBool::new(false)),
            threads: Mutex::new(Vec::new()),
        }
    }

    /// 查找本地已导入的 Whisper 模型（用户导入，不做自动下载）。
    pub fn find_model() -> Result<PathBuf, String> {
        let dir = crate::asr::model_manager::default_models_dir();
        for name in MODEL_CANDIDATES {
            let p = dir.join(name);
            if p.is_file() {
                return Ok(p);
            }
        }
        Err("未找到本地 Whisper 模型，请先在设置页导入模型".into())
    }
}

impl PipelineSource for RealPipeline {
    fn events(&self) -> mpsc::Receiver<PipelineEvent> {
        self.rx
            .lock()
            .unwrap()
            .take()
            .expect("events() 只能调用一次")
    }

    fn start(&self) -> Result<(), String> {
        if self.started.swap(true, Ordering::SeqCst) {
            return Err("流水线已启动".into());
        }
        let model = Self::find_model()?;
        let (frame_tx, frame_rx) = std_mpsc::channel::<AudioFrame>();
        let stop = self.stop_flag.clone();
        let sys = crate::audio::spawn_system_capture(frame_tx.clone(), stop.clone())
            .map_err(|e| e.to_string())?;
        self.threads.lock().unwrap().push(sys);
        if self.mic_enabled {
            if let Ok(mic) = crate::audio::spawn_microphone_capture(frame_tx, stop.clone()) {
                self.threads.lock().unwrap().push(mic);
            }
        }
        let tx = self.tx.clone();
        let stop2 = self.stop_flag.clone();
        let models_dir = crate::asr::model_manager::default_models_dir();
        let silero_path = crate::vad::silero_model_path(&models_dir);
        let thread = std::thread::Builder::new()
            .name("pipeline-vad".into())
            .spawn(move || run_pipeline(frame_rx, tx, &model, &silero_path, stop2))
            .map_err(|e| e.to_string())?;
        self.threads.lock().unwrap().push(thread);
        Ok(())
    }

    fn stop(&self) {
        self.stop_flag.store(true, Ordering::SeqCst);
        let threads = std::mem::take(&mut *self.threads.lock().unwrap());
        for t in threads {
            let _ = t.join();
        }
    }
}

/// 单个来源的分段状态：VAD 分段器 + 滚动窗口 + pending 节流。
struct SourceSeg {
    segmenter: VadSegmenter,
    rolling: Vec<i16>,
    last_pending_ms: u64,
    /// 不足 30ms 帧的样本累积（WASAPI 每包约 10ms，需拼帧）。
    frame_buf: Vec<i16>,
}

impl SourceSeg {
    fn new() -> Self {
        Self {
            segmenter: VadSegmenter::new(VadConfig::default(), 16_000),
            rolling: Vec::with_capacity(ROLLING_CAP_SAMPLES),
            last_pending_ms: 0,
            frame_buf: Vec::with_capacity(FRAME_SAMPLES * 2),
        }
    }
}

struct Segmenters {
    system: SourceSeg,
    microphone: SourceSeg,
}

impl Segmenters {
    fn new() -> Self {
        Self {
            system: SourceSeg::new(),
            microphone: SourceSeg::new(),
        }
    }

    fn for_source(&mut self, source: &str) -> &mut SourceSeg {
        if source == "microphone" {
            &mut self.microphone
        } else {
            &mut self.system
        }
    }
}

fn run_pipeline(
    frame_rx: std_mpsc::Receiver<AudioFrame>,
    tx: mpsc::Sender<PipelineEvent>,
    model: &Path,
    silero_path: &Path,
    stop: Arc<AtomicBool>,
) {
    let mut silero = match SileroVad::new(silero_path) {
        Ok(v) => {
            eprintln!("[pipeline] SileroVad loaded OK");
            v
        }
        Err(e) => {
            eprintln!("[pipeline] SileroVad init FAIL: {e}");
            return;
        }
    };
    let worker = match WhisperWorker::new(model) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("[pipeline] WhisperWorker init FAIL: {e}");
            return;
        }
    };
    tracing::info!(backend = ?worker.backend, "本地转写模型已加载");
    let mut segs = Segmenters::new();
    while !stop.load(Ordering::SeqCst) {
        let frame = match frame_rx.recv_timeout(Duration::from_millis(50)) {
            Ok(f) => f,
            Err(std_mpsc::RecvTimeoutError::Timeout) => continue,
            Err(_) => break,
        };
        process_frame(&frame, &mut silero, &mut segs, &worker, &tx);
    }
    let _ = tx.blocking_send(PipelineEvent::CaptureState {
        source: "system".into(),
        active: false,
    });
}

fn process_frame(
    frame: &AudioFrame,
    silero: &mut SileroVad,
    segs: &mut Segmenters,
    worker: &WhisperWorker,
    tx: &mpsc::Sender<PipelineEvent>,
) {
    let source = match frame.source {
        AudioSource::System => "system",
        AudioSource::Microphone => "microphone",
    };
    let rms = crate::audio::level::rms(&frame.samples_16khz_mono);
    let peak = crate::audio::level::peak(&frame.samples_16khz_mono);
    let _ = tx.try_send(PipelineEvent::AudioLevel {
        source: source.into(),
        rms: rms as f32,
        peak: peak as f32,
    });
    let now = frame.captured_at_ms;
    let s = segs.for_source(source);
    // WASAPI 每包约 10ms（160 样本 @16kHz），累积到 512 样本（32ms）再喂 VAD
    s.frame_buf.extend_from_slice(&frame.samples_16khz_mono);
    let mut pos = 0usize;
    while s.frame_buf.len() - pos >= FRAME_SAMPLES {
        let chunk = &s.frame_buf[pos..pos + FRAME_SAMPLES];
        let f32_frame: Vec<f32> = chunk.iter().map(|&s| s as f32 / 32768.0).collect();
        let prob = match silero.classify(&f32_frame) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!(error = %e, "silero classify 失败");
                0.0
            }
        };
        for ev in s.segmenter.feed(prob, chunk) {
            let SegmentEvent::SegmentCompleted(pcm) = ev;
            let transcribe_started = std::time::Instant::now();
            let text = worker.transcribe_text(&pcm).unwrap_or_default();
            let duration_ms = transcribe_started.elapsed().as_millis() as u64;
            if !text.trim().is_empty() {
                let ended = now;
                let started = ended.saturating_sub(pcm.len() as u64 / 16);
                // 时间戳：speech_started / speech_ended / transcript_final
                tracing::info!(
                    point = "speech_started",
                    id = format!("seg-{started}-{ended}"),
                    at_ms = started
                );
                tracing::info!(
                    point = "speech_ended",
                    id = format!("seg-{started}-{ended}"),
                    at_ms = ended
                );
                tracing::info!(
                    point = "transcript_final",
                    id = format!("seg-{started}-{ended}"),
                    duration_ms = duration_ms
                );
                let _ = tx.blocking_send(PipelineEvent::TranscriptFinal(TranscriptInfo {
                    id: format!("seg-{started}-{ended}"),
                    source: source.into(),
                    text,
                    started_at_ms: started,
                    ended_at_ms: ended,
                    is_final: true,
                }));
            }
        }
        // 滚动临时结果：说话期间每 800ms 转写最近 1.6s 窗口
        if prob >= 0.5 {
            s.rolling.extend_from_slice(chunk);
            while s.rolling.len() > ROLLING_CAP_SAMPLES {
                s.rolling.drain(0..s.rolling.len() - ROLLING_CAP_SAMPLES);
            }
            if now >= s.last_pending_ms + PENDING_INTERVAL_MS {
                let snapshot = s.rolling.clone();
                let text = worker.transcribe_text(&snapshot).unwrap_or_default();
                if !text.trim().is_empty() {
                    let _ = tx.blocking_send(PipelineEvent::TranscriptPending(TranscriptInfo {
                        id: format!("pending-{now}"),
                        source: source.into(),
                        text,
                        started_at_ms: now.saturating_sub(1_600),
                        ended_at_ms: now,
                        is_final: false,
                    }));
                }
                s.last_pending_ms = now;
            }
        } else {
            s.rolling.clear();
        }
        pos += FRAME_SAMPLES;
    }
    if pos > 0 {
        s.frame_buf.drain(0..pos);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::asr::whisper_worker::{fixture_path, read_wav_pcm16};

    #[test]
    fn frame_constants_are_consistent() {
        assert_eq!(FRAME_SAMPLES, 512);
        assert_eq!(ROLLING_CAP_SAMPLES, 25_600);
    }

    /// 真实链路验证：fixture 按 10ms（160 样本）分包模拟 WASAPI 包，
    /// 走 process_frame 的拼帧 -> VAD -> 分段 -> 转写全流程。
    /// 运行：`cargo test -- --ignored pipeline::tests::real_vad_from_10ms_packets`
    #[test]
    #[ignore = "requires locally imported whisper model and silero onnx"]
    fn real_vad_from_10ms_packets() {
        let models = crate::asr::model_manager::default_models_dir();
        let silero_path = crate::vad::silero_model_path(&models);
        // 诊断：打印模型输入/输出名
        let session =
            ort::session::Session::builder().expect("builder").commit_from_file(&silero_path).expect("session");
        let inputs: Vec<String> = session.inputs().iter().map(|i| i.name().to_string()).collect();
        let outputs: Vec<String> = session.outputs().iter().map(|o| o.name().to_string()).collect();
        eprintln!("[diag] inputs={inputs:?} outputs={outputs:?}");
        let mut silero = SileroVad::new(&silero_path).expect("silero onnx");
        let worker =
            WhisperWorker::new(&RealPipeline::find_model().expect("whisper model")).expect("worker");
        eprintln!("backend: {:?}", worker.backend);
        let (_, pcm) = read_wav_pcm16(&fixture_path("zh_question.wav")).expect("fixture");
        let mut segs = Segmenters::new();
        let (tx, rx) = mpsc::channel(64);
        let mut rx = rx;
        let mut packet = 0u64;
        for chunk in pcm.chunks(160) {
            if chunk.len() < 160 {
                break;
            }
            let frame = AudioFrame {
                source: AudioSource::System,
                samples_16khz_mono: chunk.to_vec(),
                captured_at_ms: packet * 10,
            };
            process_frame(&frame, &mut silero, &mut segs, &worker, &tx);
            packet += 1;
        }
        let mut events: Vec<PipelineEvent> = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            events.push(ev);
        }
        eprintln!("events: {events:?}");
        assert!(
            events
                .iter()
                .any(|e| matches!(e, PipelineEvent::TranscriptFinal(_))),
            "应产出最终转写: {events:?}"
        );
    }
}
