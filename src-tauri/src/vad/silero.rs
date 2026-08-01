//! Silero VAD v5（ONNX）分类器：30ms 帧输入，输出语音概率。
//! 通过 Rust `ort` 运行，输入顺序固定为 input / sr / h / c。

use ort::session::Session;
use ort::value::Tensor;
use std::path::Path;

use crate::vad::VadClassifier;

const STATE_SIZE: usize = 64;

#[derive(Debug, thiserror::Error)]
pub enum SileroError {
    #[error("ort error: {0}")]
    Ort(#[from] ort::Error),
    #[error("model not found: {0}")]
    NotFound(String),
}

pub struct SileroVad {
    session: Session,
    h: Vec<f32>,
    c: Vec<f32>,
}

impl SileroVad {
    pub fn new(model_path: &Path) -> Result<Self, SileroError> {
        if !model_path.is_file() {
            return Err(SileroError::NotFound(model_path.display().to_string()));
        }
        let session = Session::builder()?.commit_from_file(model_path)?;
        Ok(Self {
            session,
            h: vec![0.0; 2 * STATE_SIZE],
            c: vec![0.0; 2 * STATE_SIZE],
        })
    }

    /// 输入 16kHz 单声道 f32 帧（建议 512 样本 ≈ 30ms），返回语音概率。
    pub fn classify(&mut self, frame: &[f32]) -> Result<f32, SileroError> {
        let input = Tensor::from_array(([1usize, frame.len()], frame.to_vec().into_boxed_slice()))?;
        let sr = Tensor::from_array(ndarray::arr0(16_000i64))?;
        let h = Tensor::from_array(([2usize, 1, STATE_SIZE], self.h.clone().into_boxed_slice()))?;
        let c = Tensor::from_array(([2usize, 1, STATE_SIZE], self.c.clone().into_boxed_slice()))?;
        let outputs = self.session.run([
            input.into(),
            sr.into(),
            h.into(),
            c.into(),
        ])?;
        let prob = outputs[0].try_extract_tensor::<f32>()?;
        let result = prob.1.first().copied().unwrap_or(0.0);
        if outputs.len() > 1 {
            let hn = outputs[1].try_extract_tensor::<f32>()?;
            self.h = hn.1.to_vec();
        }
        if outputs.len() > 2 {
            let cn = outputs[2].try_extract_tensor::<f32>()?;
            self.c = cn.1.to_vec();
        }
        Ok(result)
    }
}

impl VadClassifier for SileroVad {
    fn classify_frame(&mut self, frame: &[i16]) -> f32 {
        let f32_frame: Vec<f32> = frame.iter().map(|&s| s as f32 / 32768.0).collect();
        self.classify(&f32_frame).unwrap_or(0.0)
    }
}
