//! Whisper 本地转写工作线程核心：加载模型（Vulkan 失败自动降级 CPU），
//! 对 16kHz 单声道 i16 音频生成最终文本。
//!
//! 流式集成（800ms 滚动窗口临时结果）由会话编排层（Task 9）驱动；
//! 本模块提供可独立验证的转写原语。

use std::path::Path;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WhisperBackend {
    Vulkan,
    Cpu,
}

#[derive(Debug, thiserror::Error)]
pub enum WhisperError {
    #[error("whisper error: {0}")]
    Whisper(String),
    #[error("model not found: {0}")]
    ModelNotFound(String),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct TranscriptSegment {
    pub text: String,
    pub start_ms: i64,
    pub end_ms: i64,
}

pub struct WhisperWorker {
    ctx: WhisperContext,
    params: FullParams<'static, 'static>,
    pub backend: WhisperBackend,
}

impl WhisperWorker {
    /// 加载模型：优先 Vulkan（GPU），初始化失败自动回退 CPU。
    pub fn new(model_path: &Path) -> Result<Self, WhisperError> {
        if !model_path.is_file() {
            return Err(WhisperError::ModelNotFound(model_path.display().to_string()));
        }
        let mut gpu_params = WhisperContextParameters::new();
        gpu_params.use_gpu(true);
        match WhisperContext::new_with_params(model_path, gpu_params) {
            Ok(ctx) => Ok(Self::build(ctx, WhisperBackend::Vulkan)),
            Err(_) => {
                let mut cpu_params = WhisperContextParameters::new();
                cpu_params.use_gpu(false);
                let ctx = WhisperContext::new_with_params(model_path, cpu_params)
                    .map_err(|e| WhisperError::Whisper(e.to_string()))?;
                Ok(Self::build(ctx, WhisperBackend::Cpu))
            }
        }
    }

    fn build(ctx: WhisperContext, backend: WhisperBackend) -> Self {
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_language(Some("auto"));
        params.set_n_threads(4);
        Self { ctx, params, backend }
    }

    /// 转写一段音频，返回按时间排序的文本片段。
    pub fn transcribe(&self, pcm: &[i16]) -> Result<Vec<TranscriptSegment>, WhisperError> {
        let samples: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
        let mut state = self
            .ctx
            .create_state()
            .map_err(|e| WhisperError::Whisper(e.to_string()))?;
        state
            .full(self.params.clone(), &samples)
            .map_err(|e| WhisperError::Whisper(e.to_string()))?;
        let mut out = Vec::new();
        for seg in state.as_iter() {
            out.push(TranscriptSegment {
                text: seg.to_str_lossy().unwrap_or_default().into_owned(),
                start_ms: seg.start_timestamp() / 10,
                end_ms: seg.end_timestamp() / 10,
            });
        }
        Ok(out)
    }

    /// 转写并拼接为单段文本（空音频返回空串）。
    pub fn transcribe_text(&self, pcm: &[i16]) -> Result<String, WhisperError> {
        let segs = self.transcribe(pcm)?;
        Ok(segs
            .into_iter()
            .map(|s| s.text)
            .collect::<Vec<_>>()
            .join(" ")
            .trim()
            .to_string())
    }
}

/// 读取 16kHz 单声道 16-bit PCM WAV 文件。
pub fn read_wav_pcm16(path: &Path) -> std::io::Result<(u32, Vec<i16>)> {
    let bytes = std::fs::read(path)?;
    if bytes.len() < 44 || &bytes[0..4] != b"RIFF" || &bytes[8..12] != b"WAVE" {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "not a RIFF/WAVE file"));
    }
    let channels = u16::from_le_bytes([bytes[22], bytes[23]]) as usize;
    let rate = u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]);
    let bits = u16::from_le_bytes([bytes[34], bytes[35]]) as usize;
    if channels != 1 || bits != 16 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "only 16-bit mono PCM is supported",
        ));
    }
    let mut pos = 12usize;
    let mut samples = Vec::new();
    while pos + 8 <= bytes.len() {
        let chunk_id = &bytes[pos..pos + 4];
        let chunk_len = u32::from_le_bytes([bytes[pos + 4], bytes[pos + 5], bytes[pos + 6], bytes[pos + 7]]) as usize;
        if chunk_id == b"data" {
            let data = &bytes[pos + 8..(pos + 8 + chunk_len).min(bytes.len())];
            samples = data
                .chunks_exact(2)
                .map(|c| i16::from_le_bytes([c[0], c[1]]))
                .collect();
            break;
        }
        pos += 8 + chunk_len;
    }
    if samples.is_empty() {
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "no data chunk"));
    }
    Ok((rate, samples))
}

/// 测试用：优先在默认模型目录中查找本地导入的 ggml 模型。
pub fn model_path_for_tests() -> Option<std::path::PathBuf> {
    let dir = crate::asr::model_manager::default_models_dir();
    for name in ["ggml-base-q5_0.bin", "ggml-large-v3-turbo-q5_0.bin"] {
        let p = dir.join(name);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

pub fn fixture_path(name: &str) -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../tests/fixtures/audio")
        .join(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::resample::OUTPUT_SAMPLE_RATE;

    #[test]
    fn missing_model_returns_error() {
        match WhisperWorker::new(Path::new("Z:/no/such/model.bin")) {
            Err(WhisperError::ModelNotFound(_)) => {}
            _ => panic!("expected ModelNotFound error"),
        }    }

    #[test]
    fn pcm_to_f32_mapping() {
        let pcm = vec![-32_768i16, 0, 32_767];
        let f32v: Vec<f32> = pcm.iter().map(|&s| s as f32 / 32768.0).collect();
        assert!((f32v[0] - (-1.0)).abs() < 1e-4);
        assert_eq!(f32v[1], 0.0);
        assert!((f32v[2] - 0.99997).abs() < 1e-4);
    }

    #[test]
    fn wav_reader_reads_fixtures() {
        let (rate, samples) = read_wav_pcm16(&fixture_path("silence.wav")).unwrap();
        assert_eq!(rate, 16_000);
        assert!(samples.iter().all(|&s| s == 0));
        let (rate2, samples2) = read_wav_pcm16(&fixture_path("zh_question.wav")).unwrap();
        assert_eq!(rate2, 16_000);
        assert!(!samples2.is_empty());
        assert!(samples2.iter().any(|&s| s != 0));
    }

    #[test]
    fn wav_reader_rejects_bad_format() {
        let dir = std::env::temp_dir().join(format!("maa-wav-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let f = dir.join("bad.wav");
        std::fs::write(&f, b"not a wav at all").unwrap();
        assert!(read_wav_pcm16(&f).is_err());
    }

    /// 需要用户已本地导入模型（默认模型目录）时运行：
    /// `cargo test -- --ignored asr::whisper_worker`
    #[test]
    #[ignore = "requires a locally imported whisper model"]
    fn transcribes_zh_and_en_fixtures() {
        let Some(model) = model_path_for_tests() else {
            eprintln!("skipped: no whisper model imported yet");
            return;
        };
        let worker = WhisperWorker::new(&model).unwrap();
        eprintln!("backend: {:?}", worker.backend);

        let (_, zh) = read_wav_pcm16(&fixture_path("zh_question.wav")).unwrap();
        let zh_text = worker.transcribe_text(&zh).unwrap();
        eprintln!("zh: {zh_text}");
        assert!(!zh_text.is_empty(), "zh fixture must produce text");

        let (_, en) = read_wav_pcm16(&fixture_path("en_question.wav")).unwrap();
        let en_text = worker.transcribe_text(&en).unwrap();
        eprintln!("en: {en_text}");
        assert!(!en_text.is_empty(), "en fixture must produce text");

        let (_, silence) = read_wav_pcm16(&fixture_path("silence.wav")).unwrap();
        let silence_text = worker.transcribe_text(&silence).unwrap();
        eprintln!("silence: '{silence_text}'");
        assert!(silence_text.is_empty(), "silence must not produce text");
    }

    /// 需要 44.1kHz 源时验证重采样到 16k 后再转写（模型依赖，忽略）。
    #[test]
    #[ignore = "requires a locally imported whisper model"]
    fn transcribes_after_resample_if_needed() {
        let Some(model) = model_path_for_tests() else {
            return;
        };
        let worker = WhisperWorker::new(&model).unwrap();
        let (rate, samples) = read_wav_pcm16(&fixture_path("en_question.wav")).unwrap();
        let pcm = if rate == OUTPUT_SAMPLE_RATE {
            samples
        } else {
            crate::audio::resample::resample_linear(&samples, rate, OUTPUT_SAMPLE_RATE)
        };
        let text = worker.transcribe_text(&pcm).unwrap();
        assert!(!text.is_empty());
    }
}
