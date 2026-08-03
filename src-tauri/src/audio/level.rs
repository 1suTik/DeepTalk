/// 线性 RMS（0.0-1.0 满幅刻度），用于音量表。
pub fn rms(samples: &[i16]) -> f64 {
    if samples.is_empty() {
        return 0.0;
    }
    let mut acc = 0.0f64;
    for &s in samples {
        let v = s as f64 / 32_768.0;
        acc += v * v;
    }
    (acc / samples.len() as f64).sqrt()
}

/// 线性峰值（0.0-1.0 满幅刻度）。
pub fn peak(samples: &[i16]) -> f64 {
    samples
        .iter()
        .map(|&s| (s as f64 / 32_768.0).abs())
        .fold(0.0f64, f64::max)
}

/// RMS 分贝值（20*log10，静音返回 -inf 由调用方处理）。
pub fn rms_db(samples: &[i16]) -> f64 {
    let r = rms(samples);
    if r <= 0.0 {
        return f64::NEG_INFINITY;
    }
    20.0 * r.log10()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rms_known_values() {
        assert_eq!(rms(&[]), 0.0);
        // 16384 / 32768 ≈ 0.5 满幅。
        let half = 16_384i16;
        assert!((rms(&[half, half, half, half]) - 0.5).abs() < 1e-3);
    }

    #[test]
    fn peak_known_values() {
        assert_eq!(peak(&[]), 0.0);
        assert!((peak(&[0, 16_384, -32_767]) - 1.0).abs() < 1e-3);
    }

    #[test]
    fn rms_db_silence_is_neg_inf() {
        assert_eq!(rms_db(&[]), f64::NEG_INFINITY);
        assert!(rms_db(&[16_384]).is_finite());
    }
}
