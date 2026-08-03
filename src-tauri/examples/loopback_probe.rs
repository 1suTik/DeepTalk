//! 真实设备探针：验证 WASAPI loopback（系统音频）与麦克风采集。
//!
//! 预期行为：
//! - 播放测试音频时，10 秒内输出 `LOOPBACK_OK` 并报告 RMS/峰值；
//! - 麦克风无权限/无设备时输出 `MIC_UNAVAILABLE`，系统音频不受影响，进程退出码 0。

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc, Mutex};
use std::time::{Duration, Instant};

use meeting_ai_assistant_lib::audio::{level, wasapi, AudioFrame, AudioSource};

fn probe_loopback() -> bool {
    println!("probe: system loopback (up to 10s)");
    let (tx, rx) = mpsc::channel::<AudioFrame>();
    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let th = {
        let stop = stop.clone();
        let error = error.clone();
        std::thread::spawn(move || {
            if let Err(e) = wasapi::run_loopback(tx, &stop) {
                *error.lock().unwrap() = Some(format!("{e}"));
            }
        })
    };

    let start = Instant::now();
    let mut frames = 0usize;
    let mut samples: Vec<i16> = Vec::new();
    while start.elapsed() < Duration::from_secs(10) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(frame) => {
                frames += 1;
                samples.extend(frame.samples_16khz_mono);
                if frames >= 200 {
                    break;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = th.join();

    let err = error.lock().unwrap().clone();
    if let Some(e) = err {
        println!("LOOPBACK_FAIL: {e}");
        return false;
    }
    if frames == 0 {
        println!("LOOPBACK_FAIL: no audio frames received within 10s");
        return false;
    }
    println!(
        "LOOPBACK_OK frames={frames} samples={} rms={:.4} peak={:.4}",
        samples.len(),
        level::rms(&samples),
        level::peak(&samples)
    );
    true
}

fn probe_microphone() {
    println!("probe: microphone (up to 3s, non-fatal)");
    let (tx, rx) = mpsc::channel::<AudioFrame>();
    let stop = Arc::new(AtomicBool::new(false));
    let error: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));

    let th = {
        let stop = stop.clone();
        let error = error.clone();
        std::thread::spawn(move || {
            if let Err(e) = wasapi::run_microphone(tx, &stop) {
                *error.lock().unwrap() = Some(format!("{e}"));
            }
        })
    };

    let start = Instant::now();
    let mut frames = 0usize;
    while start.elapsed() < Duration::from_secs(3) {
        match rx.recv_timeout(Duration::from_millis(200)) {
            Ok(frame) => {
                if frame.source == AudioSource::Microphone {
                    frames += 1;
                }
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
    }

    stop.store(true, Ordering::Relaxed);
    let _ = th.join();

    let err = error.lock().unwrap().clone();
    if let Some(e) = err {
        println!("MIC_UNAVAILABLE: {e} (system audio continues to work)");
    } else if frames == 0 {
        println!("MIC_UNAVAILABLE: no microphone frames (system audio continues to work)");
    } else {
        println!("MIC_OK frames={frames}");
    }
}

fn main() {
    let loopback_ok = probe_loopback();
    probe_microphone();
    if loopback_ok {
        println!("PROBE_PASS");
    } else {
        println!("PROBE_FAIL");
        std::process::exit(1);
    }
}
