//! WASAPI 采集：默认 render endpoint 的 loopback（系统音频）与默认 capture
//! endpoint（麦克风）。移植自 onetruedutchie-windows（MIT）`audio.rs` 的必要能力：
//! 默认端点选择、格式检测、音量计算、停止信号；系统与麦克风写入不同 channel，
//! 不做 sample-by-sample 混合。

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Sender;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use windows::Win32::Media::Audio::{
    eCapture, eConsole, eRender, IAudioCaptureClient, IAudioClient, IMMDeviceEnumerator,
    MMDeviceEnumerator, AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_LOOPBACK, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED,
};

use crate::audio::resample::Resampler;
use crate::audio::{AudioFrame, AudioSource};

/// `AUDCLNT_BUFFERFLAGS_SILENT` — 数据指针无意义，仅推进帧计数。
const BUFFERFLAGS_SILENT: u32 = 0x2;
const WAVE_FORMAT_IEEE_FLOAT: u16 = 0x0003;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xFFFE;

#[derive(Debug, thiserror::Error)]
pub enum AudioError {
    #[error("windows error: {0}")]
    Windows(#[from] windows::core::Error),
}

/// 系统音频：默认 render endpoint 的 loopback 采集。
pub fn run_loopback(tx: Sender<AudioFrame>, stop: &AtomicBool) -> Result<(), AudioError> {
    run_capture(AudioSource::System, true, tx, stop)
}

/// 麦克风：默认 capture endpoint 采集。
pub fn run_microphone(tx: Sender<AudioFrame>, stop: &AtomicBool) -> Result<(), AudioError> {
    run_capture(AudioSource::Microphone, false, tx, stop)
}

fn run_capture(
    source: AudioSource,
    loopback: bool,
    tx: Sender<AudioFrame>,
    stop: &AtomicBool,
) -> Result<(), AudioError> {
    unsafe {
        // 每个采集线程拥有自己的 COM 公寓（MTA），接口在单线程上创建与使用。
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        let result = capture_loop(source, loopback, tx, stop);
        CoUninitialize();
        result
    }
}

unsafe fn capture_loop(
    source: AudioSource,
    loopback: bool,
    tx: Sender<AudioFrame>,
    stop: &AtomicBool,
) -> Result<(), AudioError> {
    let enumerator: IMMDeviceEnumerator = CoCreateInstance(&MMDeviceEnumerator, None, CLSCTX_ALL)?;

    // loopback 打开的是 render 设备，只是带上 LOOPBACK 标志。
    let dataflow = if loopback { eRender } else { eCapture };
    let device = enumerator.GetDefaultAudioEndpoint(dataflow, eConsole)?;
    let client: IAudioClient = device.Activate(CLSCTX_ALL, None)?;

    // 共享模式给出设备混音格式（通常 32 位浮点、双声道、48kHz）。
    let pwfx = client.GetMixFormat()?;
    let wfx = &*pwfx;
    let in_rate = wfx.nSamplesPerSec;
    let channels = wfx.nChannels as usize;
    let bits = wfx.wBitsPerSample as usize;
    let block_align = wfx.nBlockAlign as usize;
    let is_float = sample_is_float(pwfx);

    let stream_flags = if loopback {
        AUDCLNT_STREAMFLAGS_LOOPBACK
    } else {
        0
    };
    // 200ms 缓冲区（100ns 单位）；共享模式周期必须为 0。
    let init = client.Initialize(
        AUDCLNT_SHAREMODE_SHARED,
        stream_flags,
        2_000_000,
        0,
        pwfx,
        None,
    );
    // GetMixFormat 分配的格式在 Initialize 拷贝后释放，无论成败。
    CoTaskMemFree(Some(pwfx as *const c_void));
    init?;

    let capture: IAudioCaptureClient = client.GetService()?;
    client.Start()?;

    let mut resampler = Resampler::new(in_rate);
    let mut scratch: Vec<i16> = Vec::new();

    while !stop.load(Ordering::Relaxed) {
        // 排空所有排队包，然后短睡；静音时 loopback 不产生包，由下游补零。
        loop {
            let packet = capture.GetNextPacketSize()?;
            if packet == 0 {
                break;
            }
            let mut data: *mut u8 = std::ptr::null_mut();
            let mut frames: u32 = 0;
            let mut flags: u32 = 0;
            capture.GetBuffer(&mut data, &mut frames, &mut flags, None, None)?;

            if frames > 0 {
                scratch.clear();
                if (flags & BUFFERFLAGS_SILENT) != 0 || data.is_null() {
                    for _ in 0..frames {
                        resampler.push(0.0, &mut scratch);
                    }
                } else {
                    let byte_len = frames as usize * block_align;
                    let raw = std::slice::from_raw_parts(data, byte_len);
                    for f in 0..frames as usize {
                        let frame = &raw[f * block_align..f * block_align + block_align];
                        let mono = frame_to_mono(frame, channels, bits, is_float);
                        resampler.push(mono, &mut scratch);
                    }
                }

                if !scratch.is_empty() {
                    let audio_frame = AudioFrame {
                        source,
                        samples_16khz_mono: std::mem::take(&mut scratch),
                        captured_at_ms: now_ms(),
                    };
                    if tx.send(audio_frame).is_err() {
                        // 接收端已关闭——会话结束。
                        let _ = client.Stop();
                        return Ok(());
                    }
                }
            }

            capture.ReleaseBuffer(frames)?;
        }

        std::thread::sleep(Duration::from_millis(8));
    }

    let _ = client.Stop();
    Ok(())
}

/// 一个交错帧的所有声道取均值，得到 [-1.0, 1.0] 的单声道样本。
/// 用 `from_le_bytes` 读取，不假设 WASAPI 缓冲区对 f32/i32 对齐。
#[inline]
fn frame_to_mono(frame: &[u8], channels: usize, bits: usize, is_float: bool) -> f32 {
    if channels == 0 {
        return 0.0;
    }
    let bytes_per = bits / 8;
    let mut acc = 0.0f32;
    for ch in 0..channels {
        let off = ch * bytes_per;
        if off + bytes_per > frame.len() {
            break;
        }
        let s = &frame[off..off + bytes_per];
        let v = if is_float && bytes_per == 4 {
            f32::from_le_bytes([s[0], s[1], s[2], s[3]])
        } else {
            match bytes_per {
                2 => i16::from_le_bytes([s[0], s[1]]) as f32 / 32_768.0,
                4 => i32::from_le_bytes([s[0], s[1], s[2], s[3]]) as f32 / 2_147_483_648.0,
                3 => {
                    // 24 位有符号小端，符号扩展为 i32。
                    let raw = (s[0] as i32) | ((s[1] as i32) << 8) | ((s[2] as i32) << 16);
                    let signed = (raw << 8) >> 8;
                    signed as f32 / 8_388_608.0
                }
                1 => (s[0] as f32 - 128.0) / 128.0,
                _ => 0.0,
            }
        };
        acc += v;
    }
    acc / channels as f32
}

/// 混音格式是否为 IEEE 浮点？同时处理普通 `WAVE_FORMAT_IEEE_FLOAT` 标签与
/// `WAVE_FORMAT_EXTENSIBLE`（真实类型在 SubFormat GUID 的 Data1：3=float / 1=PCM）。
unsafe fn sample_is_float(pwfx: *const WAVEFORMATEX) -> bool {
    let tag = (*pwfx).wFormatTag;
    if tag == WAVE_FORMAT_IEEE_FLOAT {
        return true;
    }
    if tag == WAVE_FORMAT_EXTENSIBLE {
        let ext = &*(pwfx as *const WAVEFORMATEXTENSIBLE);
        return ext.SubFormat.data1 == 3;
    }
    false
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
