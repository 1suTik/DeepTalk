//! 真实链路诊断：加载 Silero + Whisper -> loopback 采集 6 秒（请同时播放音频）
//! -> 打印帧数/音量 -> 对 zh fixture 做一次真实转写。
//! 运行：cargo run --manifest-path src-tauri/Cargo.toml --example pipeline_diag

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use meeting_ai_assistant_lib::asr::model_manager::default_models_dir;
use meeting_ai_assistant_lib::asr::whisper_worker::{fixture_path, WhisperWorker};
use meeting_ai_assistant_lib::audio::level;
use meeting_ai_assistant_lib::pipeline::RealPipeline;
use meeting_ai_assistant_lib::vad::silero::SileroVad;

fn main() {
    println!("== pipeline diag ==");
    let models = default_models_dir();

    // 1) Silero VAD
    let silero_path = meeting_ai_assistant_lib::vad::silero_model_path(&models);
    println!("silero path : {}", silero_path.display());
    match SileroVad::new(&silero_path) {
        Ok(_) => println!("silero      : OK"),
        Err(e) => {
            println!("silero      : FAIL -> {e}");
            return;
        }
    }

    // 2) Whisper 模型
    match RealPipeline::find_model() {
        Ok(m) => println!("whisper     : model OK ({})", m.display()),
        Err(e) => {
            println!("whisper     : FAIL -> {e}");
            return;
        }
    }
    let worker = match WhisperWorker::new(&RealPipeline::find_model().unwrap()) {
        Ok(w) => {
            println!("whisper     : loaded ({:?})", w.backend);
            w
        }
        Err(e) => {
            println!("whisper     : FAIL -> {e}");
            return;
        }
    };

    // 3) loopback 采集 6 秒（请同时播放抖音/音乐，否则静默不产生数据包）
    let (tx, rx) = std::sync::mpsc::channel();
    let stop = Arc::new(AtomicBool::new(false));
    let handle = match meeting_ai_assistant_lib::audio::spawn_system_capture(tx, stop.clone()) {
        Ok(h) => {
            println!("capture     : thread spawned");
            h
        }
        Err(e) => {
            println!("capture     : FAIL -> {e}");
            return;
        }
    };
    println!(">>> 请在 6 秒内播放音频（抖音/音乐）…");
    let deadline = std::time::Instant::now() + Duration::from_secs(6);
    let mut frames = 0u32;
    let mut max_rms = 0.0f64;
    let mut non_silent = 0u32;
    while std::time::Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(300)) {
            Ok(f) => {
                frames += 1;
                let r = level::rms(&f.samples_16khz_mono);
                max_rms = max_rms.max(r);
                if r > 0.005 {
                    non_silent += 1;
                }
            }
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                println!("  (300ms 无数据帧)");
            }
            Err(_) => {
                println!("capture     : channel closed");
                break;
            }
        }
    }
    println!("capture     : frames={frames}, non_silent={non_silent}, max_rms={max_rms:.4}");
    stop.store(true, Ordering::SeqCst);
    let _ = handle.join();

    // 4) 真实转写验证（zh fixture）
    let (_, pcm) = match meeting_ai_assistant_lib::asr::whisper_worker::read_wav_pcm16(&fixture_path("zh_question.wav")) {
        Ok(v) => v,
        Err(e) => {
            println!("fixture     : FAIL -> {e}");
            return;
        }
    };
    match worker.transcribe_text(&pcm) {
        Ok(text) => println!("transcribe  : OK -> {text:?}"),
        Err(e) => println!("transcribe  : FAIL -> {e}"),
    }
    println!("== diag done ==");
}
