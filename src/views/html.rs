/// Rich text parsing for HN comment HTML.
///
/// HN comments use a limited HTML subset:
/// - `<p>` - paragraph breaks
/// - `<i>` - italic text
/// - `<code>` - inline code
/// - `<pre><code>` - code blocks
/// - `<a href="...">text</a>` - links
/// - `>` at line start - quote blocks

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineStyle {
    Plain,
    Italic,
    Code,
    Link { url: String },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StyledSpan {
    pub text: String,
    pub style: InlineStyle,
}

impl StyledSpan {
    pub fn plain(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: InlineStyle::Plain,
        }
    }

    pub fn italic(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: InlineStyle::Italic,
        }
    }

    pub fn code(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: InlineStyle::Code,
        }
    }

    pub fn link(text: impl Into<String>, url: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            style: InlineStyle::Link { url: url.into() },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paragraph {
    pub spans: Vec<StyledSpan>,
    pub is_code_block: bool,
    pub is_quote: bool,
}

impl Paragraph {
    pub const fn new(spans: Vec<StyledSpan>) -> Self {
        Self {
            spans,
            is_code_block: false,
            is_quote: false,
        }
    }

    pub const fn code_block(spans: Vec<StyledSpan>) -> Self {
        Self {
            spans,
            is_code_block: true,
            is_quote: false,
        }
    }

    pub const fn quote(spans: Vec<StyledSpan>) -> Self {
        Self {
            spans,
            is_code_block: false,
            is_quote: true,
        }
    }
}

/// Parse HN comment HTML into structured paragraphs with styled spans.
pub fn parse_comment_html(html: &str) -> Vec<Paragraph> {
    let mut paragraphs = Vec::new();
    let parts: Vec<&str> = html.split("<p>").collect();
    for (i, part) in parts.iter().enumerate() {
        let part = part.replace("</p>", "");
        if i == 0 && part.trim().is_empty() {
            continue;
        }
        if part.contains("<pre>") || part.contains("<pre><code>") {
            paragraphs.extend(extract_code_blocks(&part));
        } else if let Some(para) = parse_text_part(&part) {
            paragraphs.push(para);
        }
    }
    paragraphs
}

fn parse_text_part(part: &str) -> Option<Paragraph> {
    // Convert <br> to newlines but keep as single paragraph
    let text = part
        .replace("<br>", "\n")
        .replace("<br/>", "\n")
        .replace("<br />", "\n");
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    // Check if this is a quote (starts with >)
    if trimmed.starts_with('>') || trimmed.starts_with("&gt;") {
        let quote_text = trimmed
            .trim_start_matches('>')
            .trim_start_matches("&gt;")
            .trim_start();
        let spans = parse_inline_tags(quote_text);
        Some(Paragraph::quote(spans))
    } else {
        let spans = parse_inline_tags(trimmed);
        Some(Paragraph::new(spans))
    }
}

fn extract_code_blocks(text: &str) -> Vec<Paragraph> {
    let mut result = Vec::new();
    let mut remaining = text;
    while let Some(pre_start) = remaining.find("<pre>") {
        // Text before code block
        let before = &remaining[..pre_start];
        if !before.trim().is_empty() {
            let spans = parse_inline_tags(before.trim());
            if !spans.is_empty() {
                result.push(Paragraph::new(spans));
            }
        }
        // Find end of pre block
        let after_pre = &remaining[pre_start + 5..];
        let pre_end = after_pre.find("</pre>").unwrap_or(after_pre.len());
        let code_content = &after_pre[..pre_end];
        // Strip <code> tags if present
        let code = code_content
            .trim_start_matches("<code>")
            .trim_end_matches("</code>")
            .trim();
        if !code.is_empty() {
            result.push(Paragraph::code_block(vec![StyledSpan::code(code)]));
        }
        remaining = if pre_end + 6 < after_pre.len() {
            &after_pre[pre_end + 6..]
        } else {
            ""
        };
    }
    // Any remaining text after code blocks
    if !remaining.trim().is_empty() {
        let spans = parse_inline_tags(remaining.trim());
        if !spans.is_empty() {
            result.push(Paragraph::new(spans));
        }
    }
    result
}

fn parse_inline_tags(text: &str) -> Vec<StyledSpan> {
    let mut spans = Vec::new();
    let mut remaining = text;
    while !remaining.is_empty() {
        // Find the next tag
        if let Some(tag_start) = remaining.find('<') {
            // Add plain text before tag
            if tag_start > 0 {
                let plain = &remaining[..tag_start];
                if !plain.is_empty() {
                    spans.push(StyledSpan::plain(plain));
                }
            }
            let after_bracket = &remaining[tag_start + 1..];
            // Determine tag type
            if after_bracket.starts_with("i>") {
                // Italic
                let content_start = tag_start + 3;
                if let Some(end) = remaining[content_start..].find("</i>") {
                    let content = &remaining[content_start..content_start + end];
                    spans.push(StyledSpan::italic(content));
                    remaining = &remaining[content_start + end + 4..];
                } else {
                    // Unclosed tag, treat as plain
                    spans.push(StyledSpan::plain(&remaining[tag_start..tag_start + 3]));
                    remaining = &remaining[tag_start + 3..];
                }
            } else if after_bracket.starts_with("code>") {
                // Inline code
                let content_start = tag_start + 6;
                if let Some(end) = remaining[content_start..].find("</code>") {
                    let content = &remaining[content_start..content_start + end];
                    spans.push(StyledSpan::code(content));
                    remaining = &remaining[content_start + end + 7..];
                } else {
                    spans.push(StyledSpan::plain(&remaining[tag_start..tag_start + 6]));
                    remaining = &remaining[tag_start + 6..];
                }
            } else if after_bracket.starts_with("a ") {
                // Link - find href and content
                if let Some((link_text, url, end_pos)) = parse_link(&remaining[tag_start..]) {
                    spans.push(StyledSpan::link(link_text, url));
                    remaining = &remaining[tag_start + end_pos..];
                } else {
                    spans.push(StyledSpan::plain("<"));
                    remaining = after_bracket;
                }
            } else if after_bracket.starts_with("b>") {
                // Bold - treat as italic since HN doesn't really use bold
                let content_start = tag_start + 3;
                if let Some(end) = remaining[content_start..].find("</b>") {
                    let content = &remaining[content_start..content_start + end];
                    spans.push(StyledSpan::italic(content));
                    remaining = &remaining[content_start + end + 4..];
                } else {
                    spans.push(StyledSpan::plain(&remaining[tag_start..tag_start + 3]));
                    remaining = &remaining[tag_start + 3..];
                }
            } else {
                // Unknown tag, skip it
                if let Some(close) = after_bracket.find('>') {
                    remaining = &remaining[tag_start + close + 2..];
                } else {
                    spans.push(StyledSpan::plain("<"));
                    remaining = after_bracket;
                }
            }
        } else {
            // No more tags, add remaining as plain text
            if !remaining.is_empty() {
                spans.push(StyledSpan::plain(remaining));
            }
            break;
        }
    }
    // Normalize whitespace in spans
    normalize_spans(spans)
}

fn parse_link(text: &str) -> Option<(String, String, usize)> {
    // text starts with "<a "
    let href_start = text.find("href=\"").or_else(|| text.find("href='"))?;
    let quote_char = text.chars().nth(href_start + 5)?;
    let url_start = href_start + 6;
    let url_end = text[url_start..].find(quote_char)?;
    let url = &text[url_start..url_start + url_end];
    // Find >
    let content_start = text[url_start + url_end..].find('>')? + url_start + url_end + 1;
    // Find </a>
    let content_end = text[content_start..].find("</a>")?;
    let link_text = &text[content_start..content_start + content_end];
    Some((
        link_text.to_string(),
        decode_entities(url),
        content_start + content_end + 4,
    ))
}

fn normalize_spans(spans: Vec<StyledSpan>) -> Vec<StyledSpan> {
    spans
        .into_iter()
        .map(|mut s| {
            // Decode HTML entities in all spans
            s.text = decode_entities(&s.text);
            s
        })
        .filter(|s| !s.text.is_empty())
        .collect()
}

fn decode_entities(text: &str) -> String {
    text.replace("&gt;", ">")
        .replace("&lt;", "<")
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#x27;", "'")
        .replace("&#39;", "'")
        .replace("&#x2F;", "/")
        .replace("&#34;", "\"")
}

/// Legacy function for backward compatibility - strips HTML to plain text.
#[allow(dead_code)]
pub fn strip_html(html: &str) -> String {
    let paragraphs = parse_comment_html(html);
    paragraphs
        .iter()
        .map(|p| {
            p.spans
                .iter()
                .map(|s| s.text.as_str())
                .collect::<Vec<_>>()
                .join("")
        })
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_plain_text() {
        let result = parse_comment_html("Hello world");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans.len(), 1);
        assert_eq!(result[0].spans[0].text, "Hello world");
        assert!(matches!(result[0].spans[0].style, InlineStyle::Plain));
    }

    #[test]
    fn test_parse_italic() {
        let result = parse_comment_html("This is <i>italic</i> text");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans.len(), 3);
        assert_eq!(result[0].spans[0].text, "This is ");
        assert_eq!(result[0].spans[1].text, "italic");
        assert!(matches!(result[0].spans[1].style, InlineStyle::Italic));
        assert_eq!(result[0].spans[2].text, " text");
    }

    #[test]
    fn test_parse_code() {
        let result = parse_comment_html("Use <code>println!</code> macro");
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans.len(), 3);
        assert_eq!(result[0].spans[1].text, "println!");
        assert!(matches!(result[0].spans[1].style, InlineStyle::Code));
    }

    #[test]
    fn test_parse_link() {
        let result = parse_comment_html(r#"Check <a href="https://example.com">this link</a> out"#);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].spans.len(), 3);
        assert_eq!(result[0].spans[1].text, "this link");
        assert!(matches!(
            &result[0].spans[1].style,
            InlineStyle::Link { url } if url == "https://example.com"
        ));
    }

    #[test]
    fn test_parse_link_with_encoded_url() {
        let result =
            parse_comment_html(r#"<a href="https:&#x2F;&#x2F;example.com&#x2F;path">link</a>"#);
        assert_eq!(result.len(), 1);
        assert!(matches!(
            &result[0].spans[0].style,
            InlineStyle::Link { url } if url == "https://example.com/path"
        ));
    }

    #[test]
    fn test_parse_paragraphs() {
        let result = parse_comment_html("First paragraph<p>Second paragraph");
        // Each <p> becomes a separate paragraph
        assert_eq!(result.len(), 2);
        let text1: String = result[0].spans.iter().map(|s| s.text.as_str()).collect();
        let text2: String = result[1].spans.iter().map(|s| s.text.as_str()).collect();
        assert!(text1.contains("First paragraph"));
        assert!(text2.contains("Second paragraph"));
    }

    #[test]
    fn test_parse_quote() {
        let result = parse_comment_html("&gt; This is quoted text");
        assert_eq!(result.len(), 1);
        assert!(result[0].is_quote);
        assert_eq!(result[0].spans[0].text, "This is quoted text");
    }

    #[test]
    fn test_parse_code_block() {
        let result = parse_comment_html("<pre><code>fn main() {}</code></pre>");
        assert_eq!(result.len(), 1);
        assert!(result[0].is_code_block);
        assert_eq!(result[0].spans[0].text, "fn main() {}");
    }

    #[test]
    fn test_parse_mixed_content() {
        let html = "&gt; Quoted intro<p>Some <i>italic</i> and <code>code</code> here";
        let result = parse_comment_html(html);
        assert_eq!(result.len(), 2);
        assert!(result[0].is_quote);
        assert!(!result[1].is_quote);
        assert!(!result[1].is_code_block);
    }

    #[test]
    fn test_decode_entities() {
        let result = parse_comment_html("&lt;script&gt; &amp; &quot;test&quot;");
        let text: String = result[0].spans.iter().map(|s| s.text.as_str()).collect();
        assert_eq!(text, "<script> & \"test\"");
    }

    #[test]
    fn test_strip_html_backward_compat() {
        assert_eq!(strip_html("<i>italic</i>"), "italic");
        assert_eq!(strip_html("<code>code</code>"), "code");
        assert_eq!(strip_html("&lt;tag&gt;"), "<tag>");
    }
}
