//! 文本规范化：用于问题去重哈希与检索匹配。

/// 全角 → 半角（中文标点映射为 ASCII）。
pub fn fullwidth_to_halfwidth(text: &str) -> String {
    text.chars()
        .map(|c| match c {
            '，' => ',',
            '。' => '.',
            '？' => '?',
            '！' => '!',
            '：' => ':',
            '；' => ';',
            '（' => '(',
            '）' => ')',
            '“' | '”' => '"',
            '‘' | '’' => '\'',
            '、' => ',',
            '　' => ' ',
            c => c,
        })
        .collect()
}

fn is_punct(c: char) -> bool {
    c.is_ascii_punctuation() || matches!(c, '，' | '。' | '？' | '！' | '：' | '；' | '、' | '（' | '）' | '「' | '」' | '『' | '』' | '·' | '…' | '—' | '‘' | '’' | '“' | '”')
}

/// 去重/匹配用规范化：去除标点、折叠空白、转小写。
pub fn normalize(text: &str) -> String {
    let half = fullwidth_to_halfwidth(text);
    let mut s: String = half.chars().filter(|c| !is_punct(*c) && !c.is_whitespace()).collect();
    if s.is_empty() {
        return s;
    }
    s = s.to_lowercase();
    s.split_whitespace().collect::<Vec<_>>().join(" ").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_punctuation_and_case() {
        assert_eq!(
            normalize("What was the hardest problem you solved?"),
            "whatwasthehardestproblemyousolved"
        );
        assert_eq!(normalize("请介绍一下你负责的项目？"), "请介绍一下你负责的项目");
    }

    #[test]
    fn collapses_whitespace() {
        assert_eq!(normalize("  你能    介绍一下  吗  "), "你能介绍一下吗");
    }

    #[test]
    fn maps_fullwidth_punct() {
        assert_eq!(fullwidth_to_halfwidth("？：，。！"), "?:,.!");
    }
}
