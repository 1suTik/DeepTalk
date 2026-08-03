//! 本地文档解析：PDF / DOCX / TXT / Markdown。
//! 只读取本地文件内容，不访问文档中的 URL 或外部资源。

use std::io::Read;
use std::path::Path;

use quick_xml::events::Event;

pub const MAX_DOC_SIZE: u64 = 5 * 1024 * 1024;
pub const MAX_DOCS: usize = 10;
const MAX_DECOMPRESSED: usize = 20 * 1024 * 1024;
const MAX_PDF_PAGES: usize = 500;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocumentKind {
    Pdf,
    Docx,
    Text,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct ExtractedDocument {
    pub kind: DocumentKind,
    pub title: String,
    pub paragraphs: Vec<String>,
    /// 规范化后的全文（段落以换行分隔）。
    pub text: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error("unsupported file type: {0}")]
    Unsupported(String),
    #[error("file too large: {0} bytes (max {MAX_DOC_SIZE})")]
    TooLarge(u64),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
    #[error("pdf parse error: {0}")]
    Pdf(String),
    #[error("docx parse error: {0}")]
    Docx(String),
    #[error("empty document")]
    Empty,
}

/// 按扩展名识别文档类型（pdf/docx/txt/md/markdown）。
pub fn detect_kind(path: &Path) -> Option<DocumentKind> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    match ext.as_str() {
        "pdf" => Some(DocumentKind::Pdf),
        "docx" => Some(DocumentKind::Docx),
        "txt" => Some(DocumentKind::Text),
        "md" | "markdown" => Some(DocumentKind::Markdown),
        _ => None,
    }
}

/// 解析文档为段落集合（保留标题为段落）。
pub fn extract(path: &Path) -> Result<ExtractedDocument, ExtractError> {
    let meta = std::fs::metadata(path)?;
    if meta.len() > MAX_DOC_SIZE {
        return Err(ExtractError::TooLarge(meta.len()));
    }
    let kind =
        detect_kind(path).ok_or_else(|| ExtractError::Unsupported(path.display().to_string()))?;
    let (title, paragraphs) = match kind {
        DocumentKind::Pdf => extract_pdf(path)?,
        DocumentKind::Docx => extract_docx(path)?,
        DocumentKind::Text => extract_plain(path, false)?,
        DocumentKind::Markdown => extract_plain(path, true)?,
    };
    if paragraphs.is_empty() {
        return Err(ExtractError::Empty);
    }
    let text = paragraphs.join("\n");
    Ok(ExtractedDocument {
        kind,
        title,
        paragraphs,
        text,
    })
}

/// 去除重复空白与控制字符（保留换行作为段落分隔后的空白折叠）。
pub fn normalize_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for c in text.chars() {
        match c {
            c if c.is_whitespace() => {
                if !prev_space {
                    out.push(' ');
                    prev_space = true;
                }
            }
            c if c.is_control() => {}
            c => {
                out.push(c);
                prev_space = false;
            }
        }
    }
    out.trim().to_string()
}

fn extract_plain(path: &Path, markdown: bool) -> Result<(String, Vec<String>), ExtractError> {
    let raw = std::fs::read(path)?;
    let raw = String::from_utf8_lossy(&raw).into_owned();
    // 统一换行：CRLF（Windows 文档常见）下 "\r\n\r\n" 不含连续 "\n\n"，
    // 直接 split 会把整篇当成一段，标题提取失效。
    let raw = raw.replace("\r\n", "\n");
    let mut title = String::new();
    let mut paragraphs = Vec::new();
    for para in raw.split("\n\n") {
        if markdown {
            let is_heading = para.trim_start().starts_with('#');
            let cleaned = normalize_text(para);
            let stripped = cleaned.trim_start_matches('#').trim_start().to_string();
            if stripped.is_empty() {
                continue;
            }
            if is_heading && title.is_empty() {
                title = stripped.clone();
            }
            paragraphs.push(stripped);
        } else {
            let cleaned = normalize_text(para);
            if cleaned.is_empty() {
                continue;
            }
            if title.is_empty() {
                title = cleaned.clone();
            }
            paragraphs.push(cleaned);
        }
    }
    Ok((title, paragraphs))
}

fn extract_pdf(path: &Path) -> Result<(String, Vec<String>), ExtractError> {
    let doc = lopdf::Document::load(path).map_err(|e| ExtractError::Pdf(e.to_string()))?;
    if doc.get_pages().len() > MAX_PDF_PAGES {
        return Err(ExtractError::Pdf("too many pages".into()));
    }
    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let flush = |current: &mut String, lines: &mut Vec<String>| {
        let s = normalize_text(current);
        if !s.is_empty() {
            lines.push(s);
        }
        current.clear();
    };
    for (_, page_id) in doc.get_pages() {
        let content = doc
            .get_and_decode_page_content(page_id)
            .map_err(|e| ExtractError::Pdf(e.to_string()))?;
        for op in &content.operations {
            match op.operator.as_str() {
                "Tj" => {
                    if let Some(lopdf::Object::String(text, _)) = op.operands.first() {
                        current.push_str(&String::from_utf8_lossy(text));
                    }
                }
                "TJ" => {
                    if let Some(lopdf::Object::Array(items)) = op.operands.first() {
                        for item in items {
                            if let lopdf::Object::String(text, _) = item {
                                current.push_str(&String::from_utf8_lossy(text));
                            }
                        }
                    }
                }
                "Td" | "TD" | "T*" | "Tj_old" => flush(&mut current, &mut lines),
                _ => {}
            }
        }
    }
    flush(&mut current, &mut lines);
    let title = lines.first().cloned().unwrap_or_default();
    Ok((title, lines))
}

fn extract_docx(path: &Path) -> Result<(String, Vec<String>), ExtractError> {
    let file = std::fs::File::open(path)?;
    let mut archive = zip::ZipArchive::new(file).map_err(|e| ExtractError::Docx(e.to_string()))?;
    if archive.len() > 200 {
        return Err(ExtractError::Docx("too many entries".into()));
    }
    let mut document_xml = None;
    for i in 0..archive.len() {
        let entry = archive
            .by_index(i)
            .map_err(|e| ExtractError::Docx(e.to_string()))?;
        if entry.name() == "word/document.xml" {
            if entry.size() > MAX_DECOMPRESSED as u64 {
                return Err(ExtractError::Docx("document.xml too large".into()));
            }
            let mut buf = Vec::with_capacity(entry.size() as usize);
            let reader = entry;
            let mut limited = reader.take(MAX_DECOMPRESSED as u64 + 1);
            limited
                .read_to_end(&mut buf)
                .map_err(|e| ExtractError::Docx(e.to_string()))?;
            if buf.len() > MAX_DECOMPRESSED {
                return Err(ExtractError::Docx("document.xml too large".into()));
            }
            document_xml = Some(buf);
            break;
        }
    }
    let xml = document_xml.ok_or_else(|| ExtractError::Docx("word/document.xml missing".into()))?;
    parse_docx_xml(&xml)
}

fn parse_docx_xml(xml: &[u8]) -> Result<(String, Vec<String>), ExtractError> {
    let mut reader = quick_xml::Reader::from_reader(xml);
    reader.config_mut().trim_text(true);
    let mut paragraphs: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut in_paragraph = false;
    let mut title = String::new();
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) => match e.local_name().as_ref() {
                b"p" => {
                    in_paragraph = true;
                    current.clear();
                }
                b"tab" if in_paragraph => {
                    current.push(' ');
                }
                _ => {}
            },
            Ok(Event::End(e)) => {
                if e.local_name().as_ref() == b"p" && in_paragraph {
                    let cleaned = normalize_text(&current);
                    if !cleaned.is_empty() {
                        paragraphs.push(cleaned);
                    }
                    in_paragraph = false;
                }
            }
            Ok(Event::Text(t)) => {
                if in_paragraph {
                    if let Ok(text) = t.decode() {
                        current.push_str(&text);
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(ExtractError::Docx(format!("xml error: {e}"))),
            _ => {}
        }
    }
    if !paragraphs.is_empty() {
        title = paragraphs[0].clone();
    }
    Ok((title, paragraphs))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn fixture(name: &str) -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../tests/fixtures/documents")
            .join(name)
    }

    fn temp_file(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("maa-profile-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join(name);
        let mut f = std::fs::File::create(&p).unwrap();
        f.write_all(bytes).unwrap();
        p
    }

    #[test]
    fn detects_kinds_by_extension() {
        assert_eq!(detect_kind(Path::new("a.pdf")), Some(DocumentKind::Pdf));
        assert_eq!(detect_kind(Path::new("a.docx")), Some(DocumentKind::Docx));
        assert_eq!(detect_kind(Path::new("a.txt")), Some(DocumentKind::Text));
        assert_eq!(detect_kind(Path::new("a.md")), Some(DocumentKind::Markdown));
        assert_eq!(
            detect_kind(Path::new("a.markdown")),
            Some(DocumentKind::Markdown)
        );
        assert_eq!(detect_kind(Path::new("a.exe")), None);
        assert_eq!(detect_kind(Path::new("noextension")), None);
    }

    #[test]
    fn extracts_markdown_with_headings() {
        let doc = extract(&fixture("sample.md")).unwrap();
        assert_eq!(doc.kind, DocumentKind::Markdown);
        assert_eq!(doc.title, "会议助手项目简介");
        assert!(doc.paragraphs.len() >= 5);
        assert!(doc.text.contains("WASAPI loopback"));
        assert!(doc.text.contains("音频延迟优化"));
    }

    #[test]
    fn extracts_docx_paragraphs_and_title() {
        let doc = extract(&fixture("sample.docx")).unwrap();
        assert_eq!(doc.kind, DocumentKind::Docx);
        assert_eq!(doc.title, "会议助手项目简介");
        assert!(doc.paragraphs.iter().any(|p| p.contains("WASAPI")));
        assert!(doc.text.contains("Whisper large-v3-turbo"));
        assert!(
            !doc.text.contains("example.com"),
            "external rels must not be followed"
        );
    }

    #[test]
    fn extracts_pdf_text() {
        let doc = extract(&fixture("sample.pdf")).unwrap();
        assert_eq!(doc.kind, DocumentKind::Pdf);
        assert!(doc.text.contains("WASAPI loopback"));
        assert!(doc.text.contains("Latency optimization"));
    }

    #[test]
    fn empty_file_is_rejected() {
        let p = temp_file("empty.md", b"");
        assert!(matches!(extract(&p), Err(ExtractError::Empty)));
    }

    #[test]
    fn oversized_file_is_rejected() {
        let big = vec![b'x'; (MAX_DOC_SIZE + 1024) as usize];
        let p = temp_file("big.pdf", &big);
        assert!(matches!(extract(&p), Err(ExtractError::TooLarge(_))));
    }

    #[test]
    fn corrupted_docx_is_rejected() {
        let p = temp_file("broken.docx", b"this is not a zip archive");
        assert!(matches!(extract(&p), Err(ExtractError::Docx(_))));
    }

    #[test]
    fn txt_with_control_chars_is_normalized() {
        let p = temp_file(
            "ctrl.txt",
            b"line one\x00\x01\x02  \t  line two\n\n  spaced  \n",
        );
        let doc = extract(&p).unwrap();
        assert!(doc.text.contains("line one"));
        assert!(doc.text.contains("line two"));
        assert!(!doc.text.contains('\u{1}'), "control chars must be removed");
    }

    #[test]
    fn normalize_collapses_whitespace() {
        assert_eq!(normalize_text("a\t\tb   c"), "a b c");
        assert_eq!(normalize_text("  lead and trail  "), "lead and trail");
        assert_eq!(normalize_text("多  个   空格"), "多 个 空格");
        assert!(!normalize_text("x\u{0}\u{1}\u{2}y").contains('\u{0}'));
    }
}
