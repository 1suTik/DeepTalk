//! 轻量本地问题检测器：读取最近 20 秒 remote 最终文本，合并相邻短句，
//! 规则判定置信度（Auto ≥ 0.65 / Maybe 0.40-0.64 / 忽略 < 0.40），
//! 标准化文本哈希在 30 秒内去重。

use std::collections::{HashMap, VecDeque};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::question::normalizer;

const ZN_PARTICLES: &[&str] = &["吗", "呢", "吧"];
const ZN_QUESTION_WORDS: &[&str] = &[
    "为什么", "怎么", "如何", "什么", "多少", "是否", "能否", "哪些", "哪里", "谁", "多久", "几",
];
const ZN_IMPERATIVE: &[&str] = &[
    "请介绍", "介绍一下", "介绍下", "谈谈", "说说", "讲讲", "解释", "举例", "说明", "描述",
];
const EN_QUESTION_STARTS: &[&str] = &[
    "who", "what", "when", "where", "why", "how", "which", "does", "do", "did", "is", "are",
    "was", "were", "can", "could", "should", "would", "will", "have", "has",
];
const EN_IMPERATIVE: &[&str] = &[
    "describe", "explain", "tell me", "elaborate", "what about", "how about", "walk me through",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TriggerLevel {
    /// ≥ 0.65：自动触发生成答案。
    Auto,
    /// 0.40-0.64：仅显示「可能的问题」，由用户点击生成。
    Maybe,
}

#[derive(Debug, Clone)]
pub struct DetectedQuestion {
    pub id: String,
    pub text: String,
    pub normalized_text: String,
    pub confidence: f64,
    pub detected_at_ms: u64,
    pub level: TriggerLevel,
}

#[derive(Debug, Clone, Copy)]
pub struct QuestionConfig {
    pub window_ms: u64,
    pub dedup_ms: u64,
    pub auto_threshold: f64,
    pub maybe_threshold: f64,
}

impl Default for QuestionConfig {
    fn default() -> Self {
        Self {
            window_ms: 20_000,
            dedup_ms: 30_000,
            auto_threshold: 0.65,
            maybe_threshold: 0.40,
        }
    }
}

pub struct QuestionDetector {
    config: QuestionConfig,
    recent: VecDeque<(String, u64, u64)>,
    last_triggered: HashMap<String, u64>,
    id_counter: u64,
}

/// 纯规则判定：返回置信度（None = 忽略，低于 0.40）。
pub fn classify(text: &str) -> Option<f64> {
    let t = text.trim();
    if t.chars().count() < 4 {
        return None;
    }
    let lower = t.to_lowercase();
    let score = if ZN_PARTICLES.iter().any(|p| t.ends_with(*p)) {
        0.9
    } else if ZN_QUESTION_WORDS.iter().any(|w| t.contains(*w)) {
        0.85
    } else if ZN_IMPERATIVE.iter().any(|w| t.contains(*w)) {
        0.7
    } else if EN_QUESTION_STARTS
        .iter()
        .any(|w| lower == *w || lower.starts_with(&format!("{w} ")))
    {
        0.85
    } else if EN_IMPERATIVE.iter().any(|w| lower.contains(*w)) {
        0.75
    } else if t.ends_with('?') || t.ends_with('？') {
        0.5
    } else {
        0.0
    };
    if score >= 0.40 {
        Some(score)
    } else {
        None
    }
}

/// 合并窗口内的最终文本为单一候选。
pub fn merge_window(items: &[(String, u64, u64)]) -> String {
    items
        .iter()
        .map(|(text, ..)| text.trim())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ")
}

impl QuestionDetector {
    pub fn new(config: QuestionConfig) -> Self {
        Self {
            config,
            recent: VecDeque::new(),
            last_triggered: HashMap::new(),
            id_counter: 0,
        }
    }

    /// 收入一段最终文本（仅 remote 源），自动清理超出窗口的旧文本。
    pub fn push_final(&mut self, text: &str, start_ms: u64, end_ms: u64) {
        if text.trim().is_empty() {
            return;
        }
        self.prune(end_ms);
        // 与上一条间隙很小则合并为一条（相邻短句）。
        if let Some(last) = self.recent.back_mut() {
            if start_ms.saturating_sub(last.2) <= 2_000 {
                last.0.push(' ');
                last.0.push_str(text);
                last.2 = end_ms;
                return;
            }
        }
        self.recent.push_back((text.trim().to_string(), start_ms, end_ms));
    }

    /// 合并后的最近窗口文本（供全局快捷键手动提交）。
    pub fn window_text(&self, now_ms: u64) -> String {
        let items: Vec<(String, u64, u64)> = self
            .recent
            .iter()
            .filter(|(_, _, end)| now_ms.saturating_sub(*end) <= self.config.window_ms)
            .cloned()
            .collect();
        merge_window(&items)
    }

    /// 自动检测：命中且未在去重窗口内重复时返回问题。
    pub fn check(&mut self, now_ms: u64) -> Option<DetectedQuestion> {
        let text = self.window_text(now_ms);
        if text.trim().is_empty() {
            return None;
        }
        let confidence = classify(&text)?;
        let normalized = normalizer::normalize(&text);
        let dedup_ms = self.config.dedup_ms;
        if let Some(&last) = self.last_triggered.get(&normalized) {
            if now_ms.saturating_sub(last) < dedup_ms {
                return None;
            }
        }
        self.last_triggered.insert(normalized.clone(), now_ms);
        let question = self.build(&text, normalized, confidence, now_ms);
        // 已消费的窗口清空，避免同一文本叠加重复触发。
        self.recent.clear();
        Some(question)
    }

    /// 手动触发：将最近窗口整体作为问题提交（不做去重）。
    pub fn check_manual(&mut self, now_ms: u64) -> Option<DetectedQuestion> {
        let text = self.window_text(now_ms);
        if text.trim().is_empty() {
            return None;
        }
        let confidence = classify(&text)?;
        let normalized = normalizer::normalize(&text);
        let question = self.build(&text, normalized, confidence, now_ms);
        self.recent.clear();
        Some(question)
    }

    fn build(
        &mut self,
        text: &str,
        normalized: String,
        confidence: f64,
        now_ms: u64,
    ) -> DetectedQuestion {
        self.id_counter += 1;
        let level = if confidence >= self.config.auto_threshold {
            TriggerLevel::Auto
        } else {
            TriggerLevel::Maybe
        };
        DetectedQuestion {
            id: format!("q-{}-{now_ms}", self.id_counter),
            text: text.trim().to_string(),
            normalized_text: normalized,
            confidence,
            detected_at_ms: now_ms,
            level,
        }
    }

    fn prune(&mut self, now_ms: u64) {
        while let Some((_, _, end)) = self.recent.front() {
            if now_ms.saturating_sub(*end) <= self.config.window_ms {
                break;
            }
            self.recent.pop_front();
        }
    }
}

#[allow(dead_code)]
fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detector() -> QuestionDetector {
        QuestionDetector::new(QuestionConfig::default())
    }

    #[test]
    fn recognizes_interview_prompts() {
        assert!(classify("请介绍一下你负责的项目").is_some());
        assert!(classify("What was the hardest problem you solved?").is_some());
        assert!(classify("今天天气不错").is_none());
    }

    #[test]
    fn zh_interrogative_particles() {
        assert!(classify("这个方案可行吗").is_some());
        assert!(classify("你们为什么延迟这么高").is_some());
        assert!(classify("你们的延迟指标是多少").is_some());
        assert!(classify("这个模型能否在 CPU 上运行").is_some());
    }

    #[test]
    fn en_imperative_and_questions() {
        assert!(classify("Tell me about your role").is_some());
        assert!(classify("Can you describe the system?").is_some());
        assert!(classify("We shipped last week").is_none());
    }

    #[test]
    fn statements_are_ignored() {
        assert!(classify("我在做一个会议助手项目").is_none());
        assert!(classify("The meeting starts at ten").is_none());
        assert!(classify("好").is_none(), "too short must be ignored");
    }

    #[test]
    fn truncated_question_is_still_detected() {
        // 被截断的问题（无句末疑问词）仍通过命令式规则命中。
        assert!(classify("请你介绍一下你的项目").is_some());
    }

    #[test]
    fn confidence_bands() {
        let auto = classify("请介绍一下你负责的项目").unwrap();
        assert!(auto >= 0.65, "imperative must be auto band: {auto}");
        let maybe = classify("the hardest part is the latency?").unwrap();
        assert!(
            maybe >= 0.40 && maybe < 0.65,
            "bare question mark must be maybe band: {maybe}"
        );
    }

    #[test]
    fn merges_adjacent_short_sentences() {
        let mut d = detector();
        d.push_final("你能介绍一下", 1000, 2500);
        d.push_final("你负责的项目吗", 2600, 4000);
        let q = d.check(5000).expect("merged text must be detected");
        assert!(q.normalized_text.contains("介绍"));
        assert!(q.normalized_text.contains("项目"));
    }

    #[test]
    fn dedups_within_30_seconds() {
        let mut d = detector();
        d.push_final("请介绍一下你负责的项目", 0, 1000);
        assert!(d.check(2000).is_some());
        d.push_final("请介绍一下你负责的项目", 5000, 6000);
        assert!(d.check(7000).is_none(), "duplicate within 30s must be suppressed");
        d.push_final("请介绍一下你负责的项目", 40_000, 41_000);
        let q = d.check(42_000);
        assert!(q.is_some(), "after 30s the same question may trigger again");
    }

    #[test]
    fn window_prunes_old_text() {
        let mut d = detector();
        d.push_final("请介绍一下你负责的项目", 0, 1000);
        d.push_final("今天天气不错", 25_000, 26_000);
        // 旧问题已被清理出 20s 窗口，窗口内只有陈述句 → 不触发。
        assert!(d.check(30_000).is_none());
    }

    #[test]
    fn manual_trigger_uses_whole_window() {
        let mut d = detector();
        d.push_final("你能说说这个项目", 1000, 2500);
        d.push_final("的主要挑战吗", 2600, 3500);
        let q = d.check_manual(4000).expect("manual trigger must detect");
        assert_eq!(q.level, TriggerLevel::Auto);
    }
}
