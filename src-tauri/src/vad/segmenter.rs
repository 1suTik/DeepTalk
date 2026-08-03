//! VAD 分段状态机（与具体 VAD 分类器解耦，测试使用 FakeVad）。
//!
//! 固定策略：30ms 帧、300ms 前置缓存、连续 180ms 语音开始片段、
//! 连续 600ms 静音结束片段、单段最长 25 秒。

use std::collections::VecDeque;

#[derive(Debug, Clone, Copy)]
pub struct VadConfig {
    pub frame_ms: u32,
    pub pre_roll_ms: u32,
    pub speech_start_ms: u32,
    pub silence_end_ms: u32,
    pub max_segment_ms: u32,
}

impl Default for VadConfig {
    fn default() -> Self {
        Self {
            frame_ms: 30,
            pre_roll_ms: 300,
            speech_start_ms: 180,
            silence_end_ms: 600,
            max_segment_ms: 25_000,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SegmentEvent {
    /// 一个完整的语音片段（16kHz 单声道 i16）。
    SegmentCompleted(Vec<i16>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SegState {
    Silence,
    InSpeech,
}

pub struct VadSegmenter {
    #[allow(dead_code)]
    config: VadConfig,
    #[allow(dead_code)]
    sample_rate: u32,
    #[allow(dead_code)]
    frame_samples: usize,
    pre_roll_samples: usize,
    speech_start_frames: u32,
    silence_end_frames: u32,
    max_segment_samples: usize,
    state: SegState,
    pre_roll: VecDeque<i16>,
    segment: Vec<i16>,
    speech_run: u32,
    silence_run: u32,
}

impl VadSegmenter {
    pub fn new(config: VadConfig, sample_rate: u32) -> Self {
        let frame_samples = (sample_rate as u64 * config.frame_ms as u64 / 1000) as usize;
        Self {
            config,
            sample_rate,
            frame_samples,
            pre_roll_samples: (sample_rate as u64 * config.pre_roll_ms as u64 / 1000) as usize,
            speech_start_frames: config.speech_start_ms / config.frame_ms,
            silence_end_frames: config.silence_end_ms / config.frame_ms,
            max_segment_samples: (sample_rate as u64 * config.max_segment_ms as u64 / 1000)
                as usize,
            state: SegState::Silence,
            pre_roll: VecDeque::with_capacity(64),
            segment: Vec::new(),
            speech_run: 0,
            silence_run: 0,
        }
    }

    /// 喂入一帧（30ms）音频与对应语音概率，返回完成的事件。
    pub fn feed(&mut self, prob: f32, frame: &[i16]) -> Vec<SegmentEvent> {
        let is_speech = prob >= 0.5;
        let mut events = Vec::new();
        match self.state {
            SegState::Silence => {
                // 缓存前置音频（环形，最多 pre_roll_ms）。
                for &s in frame {
                    if self.pre_roll.len() >= self.pre_roll_samples {
                        self.pre_roll.pop_front();
                    }
                    self.pre_roll.push_back(s);
                }
                if is_speech {
                    self.speech_run += 1;
                    if self.speech_run >= self.speech_start_frames {
                        self.begin_segment();
                    }
                } else {
                    self.speech_run = 0;
                }
            }
            SegState::InSpeech => {
                if is_speech {
                    self.segment.extend_from_slice(frame);
                    self.silence_run = 0;
                } else {
                    self.silence_run += 1;
                }
                let too_long = self.segment.len() >= self.max_segment_samples;
                if self.silence_run >= self.silence_end_frames || too_long {
                    events.push(SegmentEvent::SegmentCompleted(std::mem::take(
                        &mut self.segment,
                    )));
                    self.end_segment();
                }
            }
        }
        events
    }

    fn begin_segment(&mut self) {
        self.segment = self.pre_roll.iter().copied().collect();
        self.pre_roll.clear();
        self.speech_run = 0;
        self.silence_run = 0;
        self.state = SegState::InSpeech;
    }

    fn end_segment(&mut self) {
        self.state = SegState::Silence;
        self.speech_run = 0;
        self.silence_run = 0;
        self.pre_roll.clear();
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frames(n: u32, sample: i16) -> Vec<Vec<i16>> {
        (0..n).map(|_| vec![sample; 480]).collect::<Vec<_>>()
    }

    fn feed_all(
        seg: &mut VadSegmenter,
        probs: &[f32],
        frames_in: &[Vec<i16>],
    ) -> Vec<SegmentEvent> {
        let mut events = Vec::new();
        for (p, f) in probs.iter().zip(frames_in) {
            events.extend(seg.feed(*p, f));
        }
        events
    }

    #[test]
    fn silence_produces_no_segments() {
        let mut seg = VadSegmenter::new(VadConfig::default(), 16_000);
        let input = frames(100, 0);
        let probs = vec![0.0; 100];
        assert!(feed_all(&mut seg, &probs, &input).is_empty());
    }

    #[test]
    fn two_speech_bursts_produce_two_segments() {
        let mut seg = VadSegmenter::new(VadConfig::default(), 16_000);
        // 0.6s 静音 → 0.6s 语音 → 0.75s 静音 → 0.6s 语音 → 1s 静音
        let input = frames(115, 7);
        let mut probs = vec![0.0; 20];
        probs.extend(vec![0.9; 20]);
        probs.extend(vec![0.0; 25]);
        probs.extend(vec![0.9; 20]);
        probs.extend(vec![0.0; 30]);
        assert_eq!(probs.len(), 115);
        let events = feed_all(&mut seg, &probs, &input);
        assert_eq!(events.len(), 2);
        let SegmentEvent::SegmentCompleted(a) = &events[0];
        let SegmentEvent::SegmentCompleted(b) = &events[1];
        assert!(!a.is_empty());
        assert!(!b.is_empty());
        // 每段 = 300ms 前置缓存（10 帧） + 14 帧语音 = 24 帧 = 11520 样本。
        assert_eq!(a.len(), 11_520);
        assert_eq!(b.len(), 11_520);
    }

    #[test]
    fn segment_includes_pre_roll_audio() {
        let mut seg = VadSegmenter::new(VadConfig::default(), 16_000);
        // 10 帧静音（前置缓存用，样本值 = 帧序号），然后 10 帧语音，再 25 帧静音收尾。
        let silence: Vec<Vec<i16>> = (0..10).map(|i| vec![i as i16; 480]).collect();
        let speech: Vec<Vec<i16>> = (0..10).map(|_| vec![99; 480]).collect();
        let tail: Vec<Vec<i16>> = (0..25).map(|_| vec![0; 480]).collect();
        let mut probs = vec![0.0; 10];
        probs.extend(vec![0.9; 10]);
        probs.extend(vec![0.0; 25]);
        let events = feed_all(&mut seg, &probs, &[silence, speech, tail].concat());
        assert_eq!(events.len(), 1);
        let SegmentEvent::SegmentCompleted(seg_samples) = &events[0];
        // 语音在第 6 帧开始时，前置缓存为帧 6-15：值 6,7,8,9 的静音帧 + 6 个语音帧。
        assert_eq!(seg_samples[0..480], vec![6i16; 480]);
        assert_eq!(seg_samples[480..960], vec![7i16; 480]);
        assert_eq!(seg_samples[960..1440], vec![8i16; 480]);
        assert_eq!(seg_samples[1440..1920], vec![9i16; 480]);
        assert_eq!(seg_samples[1920..4800], vec![99i16; 2880]);
        // 段长 = 前置 10 帧 + 语音 4 帧（帧 16-19）。
        assert_eq!(seg_samples.len(), 6_720);
    }

    #[test]
    fn short_noise_does_not_start_segment() {
        let mut seg = VadSegmenter::new(VadConfig::default(), 16_000);
        // 100ms 语音噪声（< 180ms 阈值）随后静音 → 不产出片段。
        let input = frames(30, 0);
        let mut probs = vec![0.0; 10];
        probs.extend(vec![0.9; 4]);
        probs.extend(vec![0.0; 16]);
        let events = feed_all(&mut seg, &probs, &input);
        assert!(events.is_empty());
    }

    #[test]
    fn long_speech_splits_at_25_seconds() {
        let mut seg = VadSegmenter::new(VadConfig::default(), 16_000);
        // 26s 连续语音（866 帧）。
        let input = frames(866, 3);
        let probs = vec![0.9; 866];
        let events = feed_all(&mut seg, &probs, &input);
        // 段 = 前置 4800 + 824×480 = 400320 样本（≥ 25s 后的首个帧边界）。
        assert_eq!(events.len(), 1);
        let SegmentEvent::SegmentCompleted(a) = &events[0];
        assert_eq!(a.len(), 400_320);
        assert!(a.len() >= 400_000, "segment must exceed the 25s limit");
        // 继续喂 1s 语音 + 0.7s 静音 → 第二段完成。
        let input2 = frames(57, 3);
        let mut probs2 = vec![0.9; 34];
        probs2.extend(vec![0.0; 23]);
        let events2 = feed_all(&mut seg, &probs2, &input2);
        assert_eq!(events2.len(), 1);
        let SegmentEvent::SegmentCompleted(b) = &events2[0];
        assert!(!b.is_empty());
    }
}
