//! 音频采集模块：WASAPI 分别采集系统音频（loopback）与可选麦克风，
//! 统一转换为 16kHz 单声道 i16 后按来源分 channel 输出。

pub mod level;
pub mod resample;
pub mod wasapi;

use std::io;
use std::sync::atomic::AtomicBool;
use std::sync::{mpsc, Arc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AudioSource {
    System,
    Microphone,
}

/// 一帧 16kHz 单声道 i16 音频，带来源标记与采集时刻。
#[derive(Debug, Clone)]
pub struct AudioFrame {
    pub source: AudioSource,
    pub samples_16khz_mono: Vec<i16>,
    pub captured_at_ms: u64,
}

pub type CaptureSender = mpsc::Sender<AudioFrame>;
pub type CaptureReceiver = mpsc::Receiver<AudioFrame>;

/// 启动系统音频（loopback）采集线程；失败仅记录日志。
pub fn spawn_system_capture(
    tx: CaptureSender,
    stop: Arc<AtomicBool>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wasapi-sys".into())
        .spawn(move || {
            if let Err(e) = wasapi::run_loopback(tx, &stop) {
                tracing::error!(error = %e, "system audio (loopback) capture failed");
            }
        })
}

/// 启动麦克风采集线程；失败不阻断系统音频（非致命）。
pub fn spawn_microphone_capture(
    tx: CaptureSender,
    stop: Arc<AtomicBool>,
) -> io::Result<std::thread::JoinHandle<()>> {
    std::thread::Builder::new()
        .name("wasapi-mic".into())
        .spawn(move || {
            if let Err(e) = wasapi::run_microphone(tx, &stop) {
                tracing::warn!(error = %e, "microphone capture failed (continuing without mic)");
            }
        })
}
