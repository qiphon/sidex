//! Markdown preview — parse markdown and produce renderable output.
//!
//! Provides a simple markdown parser covering ~95% of typical usage:
//! headings, paragraphs, code blocks, lists, block quotes, tables,
//! images, thematic breaks, and common inline formatting.

/// A block-level markdown element.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownBlock {
    Heading(u8, String),
    Paragraph(Vec<InlineElement>),
    CodeBlock(String, String),
    List(bool, Vec<Vec<InlineElement>>),
    BlockQuote(Vec<MarkdownBlock>),
    ThematicBreak,
    Table(Vec<String>, Vec<Vec<String>>),
    Image(String, String),
}

/// An inline markdown element within a paragraph or list item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InlineElement {
    Text(String),
    Bold(String),
    Italic(String),
    Code(String),
    Link(String, String),
    Image(String, String),
    Strikethrough(String),
}

/// Holds parsed markdown and renders text output.
pub struct MarkdownRenderer {
    pub blocks: Vec<MarkdownBlock>,
}

impl MarkdownRenderer {
    pub fn new(text: &str) -> Self {
        Self {
            blocks: parse_markdown(text),
        }
    }

    /// Render the parsed blocks into plain text (for terminal/simple preview).
    #[must_use]
    pub fn render_plain(&self) -> String {
        let mut out = String::new();
        for block in &self.blocks {
            render_block_plain(block, &mut out, 0);
            out.push('\n');
        }
        out
    }
}

fn render_block_plain(block: &MarkdownBlock, out: &mut String, indent: usize) {
    let prefix: String = " ".repeat(indent);
    match block {
        MarkdownBlock::Heading(level, text) => {
            for _ in 0..*level {
                out.push('#');
            }
            out.push(' ');
            out.push_str(text);
            out.push('\n');
        }
        MarkdownBlock::Paragraph(inlines) => {
            out.push_str(&prefix);
            for inline in inlines {
                render_inline_plain(inline, out);
            }
            out.push('\n');
        }
        MarkdownBlock::CodeBlock(lang, code) => {
            out.push_str(&prefix);
            out.push_str("```");
            out.push_str(lang);
            out.push('\n');
            for line in code.lines() {
                out.push_str(&prefix);
                out.push_str(line);
                out.push('\n');
            }
            out.push_str(&prefix);
            out.push_str("```\n");
        }
        MarkdownBlock::List(ordered, items) => {
            for (i, item) in items.iter().enumerate() {
                out.push_str(&prefix);
                if *ordered {
                    out.push_str(&format!("{}. ", i + 1));
                } else {
                    out.push_str("- ");
                }
                for inline in item {
                    render_inline_plain(inline, out);
                }
                out.push('\n');
            }
        }
        MarkdownBlock::BlockQuote(blocks) => {
            for b in blocks {
                out.push_str(&prefix);
                out.push_str("> ");
                render_block_plain(b, out, indent + 2);
            }
        }
        MarkdownBlock::ThematicBreak => {
            out.push_str(&prefix);
            out.push_str("---\n");
        }
        MarkdownBlock::Table(headers, rows) => {
            out.push_str(&prefix);
            out.push_str(&headers.join(" | "));
            out.push('\n');
            out.push_str(&prefix);
            out.push_str(&headers.iter().map(|h| "-".repeat(h.len().max(3))).collect::<Vec<_>>().join(" | "));
            out.push('\n');
            for row in rows {
                out.push_str(&prefix);
                out.push_str(&row.join(" | "));
                out.push('\n');
            }
        }
        MarkdownBlock::Image(url, alt) => {
            out.push_str(&prefix);
            out.push_str(&format!("![{alt}]({url})\n"));
        }
    }
}

fn render_inline_plain(inline: &InlineElement, out: &mut String) {
    match inline {
        InlineElement::Text(t) => out.push_str(t),
        InlineElement::Bold(t) => {
            out.push_str("**");
            out.push_str(t);
            out.push_str("**");
        }
        InlineElement::Italic(t) => {
            out.push('*');
            out.push_str(t);
            out.push('*');
        }
        InlineElement::Code(t) => {
            out.push('`');
            out.push_str(t);
            out.push('`');
        }
        InlineElement::Link(text, url) => {
            out.push('[');
            out.push_str(text);
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        }
        InlineElement::Image(url, alt) => {
            out.push_str("![");
            out.push_str(alt);
            out.push_str("](");
            out.push_str(url);
            out.push(')');
        }
        InlineElement::Strikethrough(t) => {
            out.push_str("~~");
            out.push_str(t);
            out.push_str("~~");
        }
    }
}

/// Parse a markdown string into a sequence of blocks.
#[must_use]
pub fn parse_markdown(text: &str) -> Vec<MarkdownBlock> {
    let mut blocks = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        if trimmed.is_empty() {
            i += 1;
            continue;
        }

        // Thematic break
        if is_thematic_break(trimmed) {
            blocks.push(MarkdownBlock::ThematicBreak);
            i += 1;
            continue;
        }

        // Heading (ATX)
        if let Some(heading) = parse_atx_heading(trimmed) {
            blocks.push(heading);
            i += 1;
            continue;
        }

        // Fenced code block
        if trimmed.starts_with("```") {
            let lang = trimmed.trim_start_matches('`').trim().to_string();
            let mut code_lines = Vec::new();
            i += 1;
            while i < lines.len() {
                if lines[i].trim().starts_with("```") {
                    i += 1;
                    break;
                }
                code_lines.push(lines[i]);
                i += 1;
            }
            blocks.push(MarkdownBlock::CodeBlock(lang, code_lines.join("\n")));
            continue;
        }

        // Block quote
        if trimmed.starts_with('>') {
            let mut quote_lines = Vec::new();
            while i < lines.len() && (lines[i].trim().starts_with('>') || (!lines[i].trim().is_empty() && !lines[i].trim().starts_with('#'))) {
                let l = lines[i].trim();
                let stripped = l.strip_prefix('>').unwrap_or(l).trim_start();
                quote_lines.push(stripped);
                i += 1;
                if lines.get(i).map_or(true, |next| next.trim().is_empty()) {
                    break;
                }
            }
            let inner = quote_lines.join("\n");
            blocks.push(MarkdownBlock::BlockQuote(parse_markdown(&inner)));
            continue;
        }

        // Unordered list
        if matches!(trimmed.as_bytes().first(), Some(b'-' | b'*' | b'+'))
            && trimmed.len() > 1
            && trimmed.as_bytes()[1] == b' '
        {
            let mut items = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.is_empty() {
                    break;
                }
                if l.starts_with("- ") || l.starts_with("* ") || l.starts_with("+ ") {
                    items.push(parse_inlines(&l[2..]));
                } else {
                    // continuation line — append to last item
                    if let Some(last) = items.last_mut() {
                        last.push(InlineElement::Text(format!(" {}", l.trim())));
                    }
                }
                i += 1;
            }
            blocks.push(MarkdownBlock::List(false, items));
            continue;
        }

        // Ordered list
        if let Some(rest) = try_ordered_list_item(trimmed) {
            let mut items = Vec::new();
            items.push(parse_inlines(rest));
            i += 1;
            while i < lines.len() {
                let l = lines[i].trim();
                if l.is_empty() {
                    break;
                }
                if let Some(rest) = try_ordered_list_item(l) {
                    items.push(parse_inlines(rest));
                } else {
                    if let Some(last) = items.last_mut() {
                        last.push(InlineElement::Text(format!(" {}", l.trim())));
                    }
                }
                i += 1;
            }
            blocks.push(MarkdownBlock::List(true, items));
            continue;
        }

        // Table
        if i + 1 < lines.len() && looks_like_table_separator(lines[i + 1].trim()) {
            let headers: Vec<String> = parse_table_row(trimmed);
            i += 2; // skip header + separator
            let mut rows = Vec::new();
            while i < lines.len() {
                let l = lines[i].trim();
                if l.is_empty() || !l.contains('|') {
                    break;
                }
                rows.push(parse_table_row(l));
                i += 1;
            }
            blocks.push(MarkdownBlock::Table(headers, rows));
            continue;
        }

        // Image (block-level)
        if trimmed.starts_with("![") {
            if let Some(img) = parse_image(trimmed) {
                blocks.push(img);
                i += 1;
                continue;
            }
        }

        // Paragraph (default)
        let mut para_lines = Vec::new();
        while i < lines.len() && !lines[i].trim().is_empty() {
            let l = lines[i].trim();
            if l.starts_with('#')
                || l.starts_with("```")
                || l.starts_with('>')
                || is_thematic_break(l)
            {
                break;
            }
            para_lines.push(l);
            i += 1;
        }
        if !para_lines.is_empty() {
            let text = para_lines.join(" ");
            blocks.push(MarkdownBlock::Paragraph(parse_inlines(&text)));
        }
    }

    blocks
}

fn is_thematic_break(line: &str) -> bool {
    let s: String = line.chars().filter(|c| !c.is_whitespace()).collect();
    (s.len() >= 3)
        && (s.chars().all(|c| c == '-')
            || s.chars().all(|c| c == '*')
            || s.chars().all(|c| c == '_'))
}

fn parse_atx_heading(line: &str) -> Option<MarkdownBlock> {
    let bytes = line.as_bytes();
    let mut level = 0u8;
    for &b in bytes {
        if b == b'#' {
            level += 1;
        } else {
            break;
        }
    }
    if level == 0 || level > 6 {
        return None;
    }
    if line.len() > level as usize && line.as_bytes()[level as usize] != b' ' {
        return None;
    }
    let text = line[level as usize..].trim().trim_end_matches('#').trim();
    Some(MarkdownBlock::Heading(level, text.to_string()))
}

fn try_ordered_list_item(line: &str) -> Option<&str> {
    let dot_pos = line.find(". ")?;
    let prefix = &line[..dot_pos];
    if prefix.chars().all(|c| c.is_ascii_digit()) && !prefix.is_empty() {
        Some(&line[dot_pos + 2..])
    } else {
        None
    }
}

fn looks_like_table_separator(line: &str) -> bool {
    if !line.contains('-') {
        return false;
    }
    line.split('|')
        .filter(|s| !s.trim().is_empty())
        .all(|cell| {
            let c = cell.trim();
            c.chars().all(|ch| ch == '-' || ch == ':' || ch == ' ')
                && c.contains('-')
        })
}

fn parse_table_row(line: &str) -> Vec<String> {
    let stripped = line.strip_prefix('|').unwrap_or(line);
    let stripped = stripped.strip_suffix('|').unwrap_or(stripped);
    stripped
        .split('|')
        .map(|cell| cell.trim().to_string())
        .collect()
}

fn parse_image(line: &str) -> Option<MarkdownBlock> {
    // ![alt](url)
    let rest = line.strip_prefix("![")?;
    let close_bracket = rest.find(']')?;
    let alt = &rest[..close_bracket];
    let after = &rest[close_bracket + 1..];
    let after = after.strip_prefix('(')?;
    let close_paren = after.find(')')?;
    let url = &after[..close_paren];
    Some(MarkdownBlock::Image(url.to_string(), alt.to_string()))
}

/// Parse inline elements from a text string.
#[must_use]
pub fn parse_inlines(text: &str) -> Vec<InlineElement> {
    let mut elements = Vec::new();
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;
    let mut current = String::new();

    while i < len {
        // Strikethrough ~~text~~
        if i + 1 < len && chars[i] == '~' && chars[i + 1] == '~' {
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 2;
            let mut inner = String::new();
            while i + 1 < len && !(chars[i] == '~' && chars[i + 1] == '~') {
                inner.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2; // skip closing ~~
            }
            elements.push(InlineElement::Strikethrough(inner));
            continue;
        }

        // Bold **text** or __text__
        if i + 1 < len
            && ((chars[i] == '*' && chars[i + 1] == '*')
                || (chars[i] == '_' && chars[i + 1] == '_'))
        {
            let marker = chars[i];
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 2;
            let mut inner = String::new();
            while i + 1 < len && !(chars[i] == marker && chars[i + 1] == marker) {
                inner.push(chars[i]);
                i += 1;
            }
            if i + 1 < len {
                i += 2;
            }
            elements.push(InlineElement::Bold(inner));
            continue;
        }

        // Italic *text* or _text_
        if (chars[i] == '*' || chars[i] == '_')
            && (i + 1 < len && chars[i + 1] != chars[i])
        {
            let marker = chars[i];
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 1;
            let mut inner = String::new();
            while i < len && chars[i] != marker {
                inner.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            elements.push(InlineElement::Italic(inner));
            continue;
        }

        // Inline code `text`
        if chars[i] == '`' {
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 1;
            let mut inner = String::new();
            while i < len && chars[i] != '`' {
                inner.push(chars[i]);
                i += 1;
            }
            if i < len {
                i += 1;
            }
            elements.push(InlineElement::Code(inner));
            continue;
        }

        // Image ![alt](url)
        if chars[i] == '!' && i + 1 < len && chars[i + 1] == '[' {
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 2;
            let mut alt = String::new();
            while i < len && chars[i] != ']' {
                alt.push(chars[i]);
                i += 1;
            }
            i += 1; // skip ]
            if i < len && chars[i] == '(' {
                i += 1;
                let mut url = String::new();
                while i < len && chars[i] != ')' {
                    url.push(chars[i]);
                    i += 1;
                }
                if i < len {
                    i += 1; // skip )
                }
                elements.push(InlineElement::Image(url, alt));
            }
            continue;
        }

        // Link [text](url)
        if chars[i] == '[' {
            if !current.is_empty() {
                elements.push(InlineElement::Text(std::mem::take(&mut current)));
            }
            i += 1;
            let mut link_text = String::new();
            while i < len && chars[i] != ']' {
                link_text.push(chars[i]);
                i += 1;
            }
            i += 1; // skip ]
            if i < len && chars[i] == '(' {
                i += 1;
                let mut url = String::new();
                while i < len && chars[i] != ')' {
                    url.push(chars[i]);
                    i += 1;
                }
                if i < len {
                    i += 1; // skip )
                }
                elements.push(InlineElement::Link(link_text, url));
            } else {
                elements.push(InlineElement::Text(format!("[{link_text}]")));
            }
            continue;
        }

        current.push(chars[i]);
        i += 1;
    }

    if !current.is_empty() {
        elements.push(InlineElement::Text(current));
    }

    elements
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn heading_levels() {
        let blocks = parse_markdown("# H1\n## H2\n### H3");
        assert_eq!(blocks.len(), 3);
        assert_eq!(blocks[0], MarkdownBlock::Heading(1, "H1".into()));
        assert_eq!(blocks[1], MarkdownBlock::Heading(2, "H2".into()));
        assert_eq!(blocks[2], MarkdownBlock::Heading(3, "H3".into()));
    }

    #[test]
    fn paragraph_with_inlines() {
        let blocks = parse_markdown("Hello **world** and *italic*");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Paragraph(inlines) => {
                assert_eq!(inlines.len(), 4);
                assert_eq!(inlines[0], InlineElement::Text("Hello ".into()));
                assert_eq!(inlines[1], InlineElement::Bold("world".into()));
                assert_eq!(inlines[2], InlineElement::Text(" and ".into()));
                assert_eq!(inlines[3], InlineElement::Italic("italic".into()));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn fenced_code_block() {
        let blocks = parse_markdown("```rust\nfn main() {}\n```");
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0],
            MarkdownBlock::CodeBlock("rust".into(), "fn main() {}".into())
        );
    }

    #[test]
    fn unordered_list() {
        let blocks = parse_markdown("- one\n- two\n- three");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::List(ordered, items) => {
                assert!(!ordered);
                assert_eq!(items.len(), 3);
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn ordered_list() {
        let blocks = parse_markdown("1. first\n2. second");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::List(ordered, items) => {
                assert!(ordered);
                assert_eq!(items.len(), 2);
            }
            other => panic!("expected List, got {other:?}"),
        }
    }

    #[test]
    fn thematic_break() {
        let blocks = parse_markdown("---");
        assert_eq!(blocks, vec![MarkdownBlock::ThematicBreak]);
    }

    #[test]
    fn block_quote() {
        let blocks = parse_markdown("> quoted text");
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::BlockQuote(inner) => {
                assert_eq!(inner.len(), 1);
            }
            other => panic!("expected BlockQuote, got {other:?}"),
        }
    }

    #[test]
    fn table_parsing() {
        let md = "| Name | Value |\n| --- | --- |\n| a | 1 |\n| b | 2 |";
        let blocks = parse_markdown(md);
        assert_eq!(blocks.len(), 1);
        match &blocks[0] {
            MarkdownBlock::Table(headers, rows) => {
                assert_eq!(headers, &["Name", "Value"]);
                assert_eq!(rows.len(), 2);
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn inline_code_and_links() {
        let inlines = parse_inlines("see `code` and [link](http://x.com)");
        assert_eq!(inlines.len(), 4);
        assert_eq!(inlines[0], InlineElement::Text("see ".into()));
        assert_eq!(inlines[1], InlineElement::Code("code".into()));
        assert_eq!(inlines[2], InlineElement::Text(" and ".into()));
        assert_eq!(
            inlines[3],
            InlineElement::Link("link".into(), "http://x.com".into())
        );
    }

    #[test]
    fn strikethrough() {
        let inlines = parse_inlines("~~deleted~~");
        assert_eq!(inlines, vec![InlineElement::Strikethrough("deleted".into())]);
    }

    #[test]
    fn renderer_plain() {
        let r = MarkdownRenderer::new("# Title\n\nHello world");
        let out = r.render_plain();
        assert!(out.contains("# Title"));
        assert!(out.contains("Hello world"));
    }

    #[test]
    fn image_block() {
        let blocks = parse_markdown("![alt text](http://img.png)");
        assert_eq!(blocks.len(), 1);
        assert_eq!(
            blocks[0],
            MarkdownBlock::Image("http://img.png".into(), "alt text".into())
        );
    }
}
