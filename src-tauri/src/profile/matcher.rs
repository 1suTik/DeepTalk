//! 确定性关键词匹配：400-800 字符切块（80 字符重叠）、中英文 token、
//! 标题加权与 BM25 风格分数；最多 4 个片段、总长不超过 6000 字符。

#[derive(Debug, Clone)]
pub struct MatchResult {
    pub doc_id: String,
    pub doc_title: String,
    pub chunk_text: String,
    pub score: f64,
}

pub const CHUNK_MIN: usize = 400;
pub const CHUNK_MAX: usize = 800;
pub const CHUNK_OVERLAP: usize = 80;
pub const MAX_CHUNKS: usize = 4;
pub const MAX_TOTAL_CHARS: usize = 6_000;

const K1: f64 = 1.2;
const B: f64 = 0.75;
const TITLE_BOOST: f64 = 1.5;

/// 按段落切块：每块不超过 800 字符，相邻块保留 80 字符重叠；
/// 超长段落内部按相同规则切分。
pub fn chunk_text(text: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split('\n')
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .collect();
    let mut chunks: Vec<String> = Vec::new();
    let mut current = String::new();
    for p in paragraphs {
        let p_len = p.chars().count();
        if !current.is_empty() && current.chars().count() + p_len + 1 > CHUNK_MAX {
            let keep: String = current
                .chars()
                .skip(current.chars().count().saturating_sub(CHUNK_OVERLAP))
                .collect();
            chunks.push(std::mem::replace(&mut current, keep));
        }
        let mut pos = 0usize;
        while pos < p_len {
            let budget = if current.is_empty() {
                CHUNK_MAX
            } else {
                CHUNK_MAX - current.chars().count() - 1
            };
            let rest = p_len - pos;
            if rest <= budget {
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(&p.chars().skip(pos).collect::<String>());
                pos = p_len;
            } else {
                let take = budget.max(1);
                let piece: String = p.chars().skip(pos).take(take).collect();
                if !current.is_empty() {
                    current.push(' ');
                }
                current.push_str(&piece);
                chunks.push(current.clone());
                let keep: String = p
                    .chars()
                    .skip(pos + take - CHUNK_OVERLAP.min(take))
                    .take(CHUNK_OVERLAP)
                    .collect();
                current = keep;
                pos += take;
            }
        }
    }
    if !current.is_empty() {
        chunks.push(current);
    }
    chunks
}

/// 中英文 token：CJK 单字成 token，ASCII 单词按空白/标点切分并小写。
pub fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut word = String::new();
    for c in text.chars() {
        if c.is_ascii_alphanumeric() {
            word.push(c.to_ascii_lowercase());
        } else if c.is_alphanumeric() {
            if !word.is_empty() {
                tokens.push(std::mem::take(&mut word));
            }
            tokens.push(c.to_string());
        } else if !word.is_empty() {
            tokens.push(std::mem::take(&mut word));
        }
    }
    if !word.is_empty() {
        tokens.push(word);
    }
    tokens
}

pub struct ProfileMatcher {
    docs: Vec<(String, String, String)>, // (id, title, text)
}

impl ProfileMatcher {
    pub fn new() -> Self {
        Self { docs: Vec::new() }
    }

    pub fn set_docs(&mut self, docs: Vec<(String, String, String)>) {
        self.docs = docs;
    }

    pub fn doc_count(&self) -> usize {
        self.docs.len()
    }

    /// 查询匹配：返回按分数降序的片段（最多 MAX_CHUNKS，总长 ≤ MAX_TOTAL_CHARS）。
    pub fn match_chunks(&self, query: &str) -> Vec<MatchResult> {
        let qt = tokenize(query);
        if qt.is_empty() || self.docs.is_empty() {
            return Vec::new();
        }
        let n = self.docs.len() as f64;
        let avg_dl = 300.0_f64; // 简化固定平均块长（200-800 字符之间）
        let mut results: Vec<MatchResult> = Vec::new();
        for (id, title, text) in &self.docs {
            let title_tokens = tokenize(title);
            for chunk in chunk_text(text) {
                let chunk_tokens = tokenize(&chunk);
                let dl = chunk_tokens.len().max(1) as f64;
                let mut score = 0.0;
                for qtok in &qt {
                    let df = self
                        .docs
                        .iter()
                        .filter(|(_, t, c)| {
                            let mut toks = tokenize(t);
                            toks.extend(tokenize(c));
                            toks.contains(qtok)
                        })
                        .count()
                        .max(1) as f64;
                    let idf = (1.0 + (n - df + 0.5) / (df + 0.5)).ln();
                    let chunk_tf = chunk_tokens.iter().filter(|t| *t == qtok).count() as f64;
                    let title_tf = title_tokens.iter().filter(|t| *t == qtok).count() as f64;
                    let tf = chunk_tf + TITLE_BOOST * title_tf;
                    if tf > 0.0 {
                        score += idf * (tf * (K1 + 1.0)) / (tf + K1 * (1.0 - B + B * dl / avg_dl));
                    }
                }
                if score > 0.0 {
                    results.push(MatchResult {
                        doc_id: id.clone(),
                        doc_title: title.clone(),
                        chunk_text: chunk,
                        score,
                    });
                }
            }
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut out = Vec::new();
        let mut total = 0usize;
        for r in results {
            let len = r.chunk_text.chars().count();
            if out.len() >= MAX_CHUNKS || total + len > MAX_TOTAL_CHARS {
                break;
            }
            total += len;
            out.push(r);
        }
        out
    }
}

impl Default for ProfileMatcher {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project_doc() -> (String, String, String) {
        let text = "本项目通过 WASAPI loopback 采集系统音频，降低音频延迟（reduce audio latency）是关键目标。\n音频延迟优化：200ms 缓冲区，800ms 滚动窗口，Silero VAD 分段减少无效转写。\nWhisper large-v3-turbo 在 Vulkan 上本地推理，转写延迟低。\nVAD 以 30ms 帧检测语音起止，180ms 起段、600ms 收段。\n".repeat(3);
        ("proj".into(), "会议助手音频与转写".into(), text)
    }

    fn unrelated_doc() -> (String, String, String) {
        let text = "今天天气不错，适合出门散步。\n超市里买了牛奶和面包，价格实惠。\n周末计划去爬山，顺便看日出。\n".repeat(3);
        ("shop".into(), "生活记录".into(), text)
    }

    #[test]
    fn chunking_respects_bounds_and_overlap() {
        let long = "段落内容。".repeat(600); // 3000 chars
        let chunks = chunk_text(&long);
        assert!(chunks.len() >= 3);
        for c in &chunks {
            assert!(c.chars().count() <= CHUNK_MAX, "chunk too long");
        }
        for w in chunks.windows(2) {
            let tail: String = w[0]
                .chars()
                .skip(w[0].chars().count().saturating_sub(CHUNK_OVERLAP))
                .collect();
            assert!(
                w[1].starts_with(&tail),
                "next chunk must start with the previous tail"
            );
        }
    }

    #[test]
    fn tokenize_cjk_and_ascii() {
        let t = tokenize("音频延迟 optimization VAD");
        assert!(t.contains(&"音".into()));
        assert!(!t.contains(&"延迟".into()), "cjk must be per char");
        assert!(t.contains(&"optimization".into()));
        assert!(t.contains(&"vad".into()));
    }

    #[test]
    fn relevant_doc_ranks_first() {
        let mut m = ProfileMatcher::new();
        m.set_docs(vec![project_doc(), unrelated_doc()]);
        let results = m.match_chunks("音频延迟优化");
        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "proj", "audio latency question must hit the project doc");
    }

    #[test]
    fn english_query_hits_wasapi_doc() {
        let mut m = ProfileMatcher::new();
        m.set_docs(vec![project_doc(), unrelated_doc()]);
        let results = m.match_chunks("How do you reduce audio latency?");
        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "proj");
    }

    #[test]
    fn result_limits_are_enforced() {
        let mut m = ProfileMatcher::new();
        m.set_docs(vec![project_doc(), project_doc(), unrelated_doc()]);
        let results = m.match_chunks("音频 VAD Whisper WASAPI 转写");
        assert!(results.len() <= MAX_CHUNKS);
        let total: usize = results.iter().map(|r| r.chunk_text.chars().count()).sum();
        assert!(total <= MAX_TOTAL_CHARS, "total {total} exceeds {MAX_TOTAL_CHARS}");
    }

    #[test]
    fn title_terms_boost_score() {
        let mut m = ProfileMatcher::new();
        // 相同正文，标题含查询词的一方应得分更高。
        let body = "这是一个普通的项目描述文本，没有特别的关键词。\n".repeat(50);
        m.set_docs(vec![
            ("a".into(), "音频延迟优化方案".into(), body.clone()),
            ("b".into(), "完全无关的标题".into(), body),
        ]);
        let results = m.match_chunks("音频延迟");
        assert!(!results.is_empty());
        assert_eq!(results[0].doc_id, "a", "title match must rank first");
    }

    #[test]
    fn matching_is_deterministic() {
        let mut m = ProfileMatcher::new();
        m.set_docs(vec![project_doc(), unrelated_doc()]);
        let r1 = m.match_chunks("音频延迟优化");
        let r2 = m.match_chunks("音频延迟优化");
        let ids1: Vec<&str> = r1.iter().map(|r| r.doc_id.as_str()).collect();
        let ids2: Vec<&str> = r2.iter().map(|r| r.doc_id.as_str()).collect();
        assert_eq!(ids1, ids2);
    }
}
