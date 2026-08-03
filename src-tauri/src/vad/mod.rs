pub mod segmenter;
pub mod silero;

use std::path::Path;

/// VAD 分类器：对一帧 16kHz 单声道样本返回语音概率 [0.0, 1.0]。
pub trait VadClassifier {
    fn classify_frame(&mut self, frame: &[i16]) -> f32;
}

/// Silero VAD ONNX 模型路径（与 Whisper 模型同目录）。
pub fn silero_model_path(models_dir: &Path) -> std::path::PathBuf {
    models_dir.join("silero_vad.onnx")
}
