/// 统一输出格式：16kHz 单声道 i16。
pub const OUTPUT_SAMPLE_RATE: u32 = 16_000;

/// 钳位到 [-1.0, 1.0] 后转为 i16，峰值不溢出。
#[inline]
pub fn clamp_i16(v: f32) -> i16 {
    (v.clamp(-1.0, 1.0) * 32_767.0).round() as i16
}

/// 交错多声道浮点样本 → 单声道 i16（逐帧取均值后钳位）。
pub fn to_mono_i16(samples: &[f32], channels: usize) -> Vec<i16> {
    if channels == 0 {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(samples.len() / channels);
    for frame in samples.chunks_exact(channels) {
        let mono: f32 = frame.iter().sum::<f32>() / channels as f32;
        out.push(clamp_i16(mono));
    }
    out
}

/// 一次性线性插值重采样（i16 输入，任意输入/输出采样率）。
/// 输出第 j 个样本对应输入位置 `j * src_rate / dst_rate`，相邻样本线性插值。
pub fn resample_linear(src: &[i16], src_rate: u32, dst_rate: u32) -> Vec<i16> {
    if src_rate == dst_rate || src.len() <= 1 {
        return src.to_vec();
    }
    let step = src_rate as f64 / dst_rate as f64;
    let out_len = ((src.len() as f64 - 1.0) / step).floor() as usize + 1;
    let mut out = Vec::with_capacity(out_len);
    for j in 0..out_len {
        let pos = j as f64 * step;
        let i0 = pos.floor() as usize;
        let i1 = (i0 + 1).min(src.len() - 1);
        let frac = pos - i0 as f64;
        let v = src[i0] as f64 * (1.0 - frac) + src[i1] as f64 * frac;
        out.push(v.round().clamp(i16::MIN as f64, i16::MAX as f64) as i16);
    }
    out
}

/// 流式线性插值重采样器：从 `in_rate` 重采样到 `OUTPUT_SAMPLE_RATE`。
/// 对语音足够；适配任意设备采样率（48kHz、44.1kHz…）且无额外依赖。
pub struct Resampler {
    /// 每个输出样本消耗的输入样本数（in_rate / 16000）。
    step: f64,
    /// 当前输出游标在 [prev, cur] 输入段内的位置。
    pos: f64,
    prev: f32,
    has_prev: bool,
}

impl Resampler {
    pub fn new(in_rate: u32) -> Self {
        Self {
            step: in_rate as f64 / OUTPUT_SAMPLE_RATE as f64,
            pos: 0.0,
            prev: 0.0,
            has_prev: false,
        }
    }

    /// 推入一个输入样本；可能产出 0..n 个输出样本。
    #[inline]
    pub fn push(&mut self, cur: f32, out: &mut Vec<i16>) {
        if !self.has_prev {
            self.prev = cur;
            self.has_prev = true;
            return;
        }
        while self.pos < 1.0 {
            let f = self.pos as f32;
            let v = self.prev + (cur - self.prev) * f;
            out.push(clamp_i16(v));
            self.pos += self.step;
        }
        self.pos -= 1.0;
        self.prev = cur;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn one_second_48k_stereo_becomes_16000_mono_16k() {
        // 1 秒 48kHz 双声道浮点 = 96000 个交错样本。
        let samples: Vec<f32> = (0..96_000)
            .map(|i| ((i % 2) as f32 * 0.5) - 0.25)
            .collect();
        let mono = to_mono_i16(&samples, 2);
        assert_eq!(mono.len(), 48_000, "mono frame count for 1s at 48kHz");
        let out = resample_linear(&mono, 48_000, OUTPUT_SAMPLE_RATE);
        assert_eq!(out.len(), 16_000, "1s of audio must yield 16000 samples at 16kHz");
        assert!(out.iter().all(|&s| s.abs() <= i16::MAX as i16), "no sample may overflow");
    }

    #[test]
    fn peak_never_overflows() {
        // 满幅正弦 + 1.5 倍过载：钳位后峰值必须仍在 i16 范围内。
        let samples: Vec<f32> = (0..48_000)
            .map(|i| 1.5 * ((i as f32 * std::f32::consts::TAU * 440.0) / 48_000.0).sin())
            .collect();
        let mono = to_mono_i16(&samples, 1);
        assert_eq!(
            mono.iter().map(|&s| s as i32).max(),
            Some(32_767),
            "overload must clamp at i16 max"
        );
        assert_eq!(
            mono.iter().map(|&s| s as i32).min(),
            Some(-32_767),
            "overload must clamp at -i16 max"
        );
    }

    #[test]
    fn passthrough_at_same_rate() {
        let src = vec![0i16, 1000, -1000, 32_767];
        assert_eq!(resample_linear(&src, 16_000, 16_000), src);
    }

    #[test]
    fn decimation_48k_to_16k_is_exact_3x() {
        let src = vec![0i16; 3_000];
        let out = resample_linear(&src, 48_000, 16_000);
        assert_eq!(out.len(), 1_000);
        // 恒定信号经过线性插值保持原值。
        assert!(out.iter().all(|&s| s == 0));
    }

    #[test]
    fn non_integer_ratio_44k() {
        let src = vec![0i16; 44_100];
        let out = resample_linear(&src, 44_100, 16_000);
        let expected = 16_000;
        assert!(
            (out.len() as i64 - expected).abs() <= 2,
            "len = {}",
            out.len()
        );
    }

    #[test]
    fn mono_from_float_stereo_averages() {
        let mut frame = Vec::new();
        frame.extend_from_slice(&1.0f32.to_le_bytes());
        frame.extend_from_slice(&(-1.0f32).to_le_bytes());
        let samples: Vec<f32> = frame
            .chunks_exact(4)
            .map(|c| f32::from_le_bytes([c[0], c[1], c[2], c[3]]))
            .collect();
        assert_eq!(to_mono_i16(&samples, 2), vec![0]);
    }

    #[test]
    fn clamp_i16_bounds() {
        assert_eq!(clamp_i16(2.0), 32_767);
        assert_eq!(clamp_i16(-2.0), -32_767);
        assert_eq!(clamp_i16(0.0), 0);
        assert_eq!(clamp_i16(0.5), 16_384);
    }

    #[test]
    fn streaming_resampler_decimates_3x() {
        let mut r = Resampler::new(48_000);
        let mut out = Vec::new();
        for _ in 0..3_000 {
            r.push(0.25, &mut out);
        }
        assert_eq!(out.len(), 1_000);
        assert!(out.iter().all(|&s| s == 8_192));
    }
}
