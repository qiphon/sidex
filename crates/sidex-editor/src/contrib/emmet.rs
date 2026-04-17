//! Emmet abbreviation expansion for HTML and CSS.
//!
//! Covers roughly 90% of daily Emmet use: nested elements, class/id
//! shorthand, multiplication, text content, and common CSS shorthands.
//! Also provides an `EmmetEngine` controller, context detection,
//! completion suggestions, and tag wrapping.

use std::fmt::Write;

/// Expand an Emmet abbreviation for the given language.
///
/// Returns `None` if the input cannot be parsed as a valid abbreviation.
pub fn expand_abbreviation(input: &str, language: &str) -> Option<String> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    match language {
        "html" | "jsx" | "tsx" | "vue" | "svelte" | "erb" | "handlebars" | "php" => {
            expand_html(input)
        }
        "css" | "scss" | "less" | "sass" => expand_css(input),
        _ => None,
    }
}

/// Returns `true` if `input` looks like a plausible Emmet abbreviation in
/// the given language, suitable for triggering expansion on Tab.
pub fn looks_like_abbreviation(input: &str, language: &str) -> bool {
    let input = input.trim();
    if input.is_empty() || input.contains(' ') {
        return false;
    }
    match language {
        "css" | "scss" | "less" | "sass" => {
            CSS_SHORTHANDS.iter().any(|(k, _)| input.starts_with(k))
        }
        _ => input
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphabetic() || c == '!' || c == '.'),
    }
}

// ---------------------------------------------------------------------------
// HTML expansion
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
struct HtmlNode {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    attrs: Vec<(String, String)>,
    text: Option<String>,
    repeat: u32,
    children: Vec<HtmlNode>,
}

impl Default for HtmlNode {
    fn default() -> Self {
        Self {
            tag: "div".into(),
            id: None,
            classes: Vec::new(),
            attrs: Vec::new(),
            text: None,
            repeat: 1,
            children: Vec::new(),
        }
    }
}

fn expand_html(input: &str) -> Option<String> {
    if input == "!" || input == "html:5" {
        return Some(html5_boilerplate());
    }

    let nodes = parse_html_abbreviation(input)?;
    let mut out = String::new();
    for node in &nodes {
        render_html_node(node, &mut out, 0);
    }
    if out.ends_with('\n') {
        out.truncate(out.len() - 1);
    }
    Some(out)
}

fn parse_html_abbreviation(input: &str) -> Option<Vec<HtmlNode>> {
    let parts = split_siblings(input);
    let mut nodes = Vec::new();
    for part in &parts {
        nodes.push(parse_single_chain(part)?);
    }
    Some(nodes)
}

fn split_siblings(input: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut depth = 0u32;
    let mut start = 0;
    for (i, c) in input.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '+' if depth == 0 => {
                parts.push(&input[start..i]);
                start = i + 1;
            }
            _ => {}
        }
    }
    parts.push(&input[start..]);
    parts
}

fn parse_single_chain(input: &str) -> Option<HtmlNode> {
    if let Some(pos) = find_child_separator(input) {
        let parent_str = &input[..pos];
        let child_str = &input[pos + 1..];
        let mut parent = parse_element(parent_str)?;
        let child = parse_single_chain(child_str)?;
        parent.children.push(child);
        Some(parent)
    } else {
        parse_element(input)
    }
}

fn find_child_separator(input: &str) -> Option<usize> {
    let mut depth = 0u32;
    let mut in_brackets = false;
    let mut in_braces = false;
    for (i, c) in input.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.saturating_sub(1),
            '[' => in_brackets = true,
            ']' => in_brackets = false,
            '{' => in_braces = true,
            '}' => in_braces = false,
            '>' if depth == 0 && !in_brackets && !in_braces => return Some(i),
            _ => {}
        }
    }
    None
}

fn parse_element(input: &str) -> Option<HtmlNode> {
    let input = input.trim();
    if input.is_empty() {
        return None;
    }

    let (main, repeat) = if let Some(pos) = input.rfind('*') {
        let after = &input[pos + 1..];
        if let Ok(n) = after.parse::<u32>() {
            (&input[..pos], n)
        } else {
            (input, 1)
        }
    } else {
        (input, 1)
    };

    let (main, text) = if let Some(open) = main.find('{') {
        if main.ends_with('}') {
            (
                &main[..open],
                Some(main[open + 1..main.len() - 1].to_string()),
            )
        } else {
            (main, None)
        }
    } else {
        (main, None)
    };

    let (main, attrs) = if let Some(open) = main.find('[') {
        if main.ends_with(']') {
            let attr_str = &main[open + 1..main.len() - 1];
            let attrs = parse_attrs(attr_str);
            (&main[..open], attrs)
        } else {
            (main, Vec::new())
        }
    } else {
        (main, Vec::new())
    };

    let mut tag = String::new();
    let mut id = None;
    let mut classes = Vec::new();
    let mut current = String::new();
    let mut mode: u8 = 0; // 0=tag, 1=id, 2=class

    for c in main.chars() {
        match c {
            '#' => {
                if mode == 0 {
                    tag.clone_from(&current);
                } else if mode == 1 {
                    id = Some(current.clone());
                } else {
                    classes.push(current.clone());
                }
                current.clear();
                mode = 1;
            }
            '.' => {
                if mode == 0 {
                    tag.clone_from(&current);
                } else if mode == 1 {
                    id = Some(current.clone());
                } else {
                    classes.push(current.clone());
                }
                current.clear();
                mode = 2;
            }
            _ => current.push(c),
        }
    }
    match mode {
        0 => tag = current,
        1 => id = Some(current),
        _ => classes.push(current),
    }

    if tag.is_empty() && (id.is_some() || !classes.is_empty()) {
        tag = "div".into();
    }

    if tag.is_empty() {
        return None;
    }

    Some(HtmlNode {
        tag,
        id,
        classes,
        attrs,
        text,
        repeat,
        children: Vec::new(),
    })
}

fn parse_attrs(input: &str) -> Vec<(String, String)> {
    let mut attrs = Vec::new();
    for part in input.split_whitespace() {
        if let Some((k, v)) = part.split_once('=') {
            let v = v.trim_matches('"').trim_matches('\'');
            attrs.push((k.to_string(), v.to_string()));
        } else {
            attrs.push((part.to_string(), String::new()));
        }
    }
    attrs
}

const SELF_CLOSING: &[&str] = &[
    "br", "hr", "img", "input", "meta", "link", "area", "base", "col", "embed", "source", "track",
    "wbr",
];

fn render_html_node(node: &HtmlNode, out: &mut String, indent: usize) {
    for _ in 0..node.repeat {
        let pad = "  ".repeat(indent);
        let _ = write!(out, "{pad}<{}", node.tag);

        if let Some(ref id) = node.id {
            let _ = write!(out, " id=\"{id}\"");
        }
        if !node.classes.is_empty() {
            let _ = write!(out, " class=\"{}\"", node.classes.join(" "));
        }
        for (k, v) in &node.attrs {
            if v.is_empty() {
                let _ = write!(out, " {k}=\"\"");
            } else {
                let _ = write!(out, " {k}=\"{v}\"");
            }
        }

        if SELF_CLOSING.contains(&node.tag.as_str()) {
            out.push_str(" />\n");
            continue;
        }

        out.push('>');

        if let Some(ref text) = node.text {
            let _ = write!(out, "{text}");
        }

        if node.children.is_empty() {
            let _ = write!(out, "</{}>", node.tag);
        } else {
            out.push('\n');
            for child in &node.children {
                render_html_node(child, out, indent + 1);
            }
            let _ = write!(out, "{pad}</{}>", node.tag);
        }
        out.push('\n');
    }
}

fn html5_boilerplate() -> String {
    "\
<!DOCTYPE html>
<html lang=\"en\">
<head>
  <meta charset=\"UTF-8\" />
  <meta name=\"viewport\" content=\"width=device-width, initial-scale=1.0\" />
  <title>Document</title>
</head>
<body>
  
</body>
</html>"
        .to_string()
}

// ---------------------------------------------------------------------------
// CSS expansion
// ---------------------------------------------------------------------------

const CSS_SHORTHANDS: &[(&str, &str)] = &[
    ("m0", "margin: 0;"),
    ("m10", "margin: 10px;"),
    ("mt", "margin-top: ;"),
    ("mr", "margin-right: ;"),
    ("mb", "margin-bottom: ;"),
    ("ml", "margin-left: ;"),
    ("p0", "padding: 0;"),
    ("p10", "padding: 10px;"),
    ("pt", "padding-top: ;"),
    ("pr", "padding-right: ;"),
    ("pb", "padding-bottom: ;"),
    ("pl", "padding-left: ;"),
    ("w", "width: ;"),
    ("w100p", "width: 100%;"),
    ("h", "height: ;"),
    ("h100p", "height: 100%;"),
    ("d", "display: ;"),
    ("dn", "display: none;"),
    ("db", "display: block;"),
    ("dib", "display: inline-block;"),
    ("di", "display: inline;"),
    ("df", "display: flex;"),
    ("dg", "display: grid;"),
    ("pos", "position: ;"),
    ("posa", "position: absolute;"),
    ("posr", "position: relative;"),
    ("posf", "position: fixed;"),
    ("poss", "position: sticky;"),
    ("t", "top: ;"),
    ("r", "right: ;"),
    ("b", "bottom: ;"),
    ("l", "left: ;"),
    ("fl", "float: left;"),
    ("fr", "float: right;"),
    ("fw", "font-weight: ;"),
    ("fwb", "font-weight: bold;"),
    ("fs", "font-size: ;"),
    ("ff", "font-family: ;"),
    ("ta", "text-align: ;"),
    ("tac", "text-align: center;"),
    ("tal", "text-align: left;"),
    ("tar", "text-align: right;"),
    ("td", "text-decoration: ;"),
    ("tdn", "text-decoration: none;"),
    ("tdu", "text-decoration: underline;"),
    ("tt", "text-transform: ;"),
    ("ttu", "text-transform: uppercase;"),
    ("ttl", "text-transform: lowercase;"),
    ("bg", "background: ;"),
    ("bgc", "background-color: ;"),
    ("bgi", "background-image: ;"),
    ("c", "color: ;"),
    ("op", "opacity: ;"),
    ("ov", "overflow: ;"),
    ("ovh", "overflow: hidden;"),
    ("ova", "overflow: auto;"),
    ("ovs", "overflow: scroll;"),
    ("bd", "border: ;"),
    ("bdn", "border: none;"),
    ("bdrs", "border-radius: ;"),
    ("bs", "box-shadow: ;"),
    ("bxz", "box-sizing: border-box;"),
    ("cur", "cursor: ;"),
    ("curp", "cursor: pointer;"),
    ("z", "z-index: ;"),
    ("lh", "line-height: ;"),
    ("va", "vertical-align: ;"),
    ("whs", "white-space: ;"),
    ("whsnw", "white-space: nowrap;"),
    ("trs", "transition: ;"),
    ("anim", "animation: ;"),
    ("fxd", "flex-direction: ;"),
    ("fxdc", "flex-direction: column;"),
    ("fxdr", "flex-direction: row;"),
    ("jc", "justify-content: ;"),
    ("jcc", "justify-content: center;"),
    ("jcsb", "justify-content: space-between;"),
    ("ai", "align-items: ;"),
    ("aic", "align-items: center;"),
    ("aifs", "align-items: flex-start;"),
    ("aife", "align-items: flex-end;"),
    ("fw1", "flex: 1;"),
    ("gap", "gap: ;"),
];

fn expand_css(input: &str) -> Option<String> {
    for &(abbr, expansion) in CSS_SHORTHANDS {
        if input == abbr {
            return Some(expansion.to_string());
        }
    }

    if let Some(result) = expand_css_numeric(input) {
        return Some(result);
    }

    None
}

fn expand_css_numeric(input: &str) -> Option<String> {
    let prefixes: &[(&str, &str)] = &[
        ("m", "margin"),
        ("p", "padding"),
        ("w", "width"),
        ("h", "height"),
        ("t", "top"),
        ("r", "right"),
        ("b", "bottom"),
        ("l", "left"),
        ("fs", "font-size"),
        ("lh", "line-height"),
        ("bdrs", "border-radius"),
        ("gap", "gap"),
        ("mt", "margin-top"),
        ("mr", "margin-right"),
        ("mb", "margin-bottom"),
        ("ml", "margin-left"),
        ("pt", "padding-top"),
        ("pr", "padding-right"),
        ("pb", "padding-bottom"),
        ("pl", "padding-left"),
    ];

    for &(abbr, prop) in prefixes {
        if let Some(num_str) = input.strip_prefix(abbr) {
            if num_str.is_empty() {
                continue;
            }
            let first_char = num_str.chars().next()?;
            if !first_char.is_ascii_digit() && first_char != '-' {
                continue;
            }
            let (num, unit) = split_num_unit(num_str);
            let unit = if unit.is_empty() && num != "0" {
                "px"
            } else {
                unit
            };
            return Some(format!("{prop}: {num}{unit};"));
        }
    }
    None
}

fn split_num_unit(s: &str) -> (&str, &str) {
    let pos = s
        .find(|c: char| !c.is_ascii_digit() && c != '-' && c != '.')
        .unwrap_or(s.len());
    (&s[..pos], &s[pos..])
}

// ---------------------------------------------------------------------------
// EmmetEngine — controller for Emmet integration
// ---------------------------------------------------------------------------

const HTML_LANGUAGES: &[&str] = &[
    "html", "jsx", "tsx", "vue", "svelte", "erb", "handlebars", "php",
    "xml", "xsl", "pug", "slim",
];
const CSS_LANGUAGES: &[&str] = &["css", "scss", "less", "sass", "stylus"];

/// Top-level engine managing Emmet settings and expansion.
#[derive(Debug, Clone)]
pub struct EmmetEngine {
    pub enabled: bool,
    pub supported_languages: Vec<String>,
    pub show_suggestions_as_completions: bool,
}

impl Default for EmmetEngine {
    fn default() -> Self {
        let mut langs: Vec<String> = HTML_LANGUAGES.iter().map(|s| (*s).to_string()).collect();
        langs.extend(CSS_LANGUAGES.iter().map(|s| (*s).to_string()));
        Self {
            enabled: true,
            supported_languages: langs,
            show_suggestions_as_completions: true,
        }
    }
}

impl EmmetEngine {
    /// Creates a new engine with the default supported languages.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Whether the engine supports a given language id.
    #[must_use]
    pub fn supports_language(&self, language: &str) -> bool {
        self.enabled && self.supported_languages.iter().any(|l| l == language)
    }

    /// Tries to expand an abbreviation for the given language.
    pub fn expand(&self, abbr: &str, language: &str) -> Option<EmmetAbbreviation> {
        if !self.enabled || !self.supports_language(language) {
            return None;
        }
        let expanded = expand_abbreviation(abbr, language)?;
        let cursor_position = find_cursor_position(&expanded);
        Some(EmmetAbbreviation {
            raw: abbr.to_string(),
            expanded,
            cursor_position,
        })
    }

    /// Wraps selected text with an Emmet abbreviation.
    pub fn wrap_with_abbreviation(
        &self,
        selected_text: &str,
        abbr: &str,
        language: &str,
    ) -> Option<String> {
        if !self.supports_language(language) {
            return None;
        }
        let with_content = if abbr.contains('{') {
            abbr.to_string()
        } else {
            format!("{abbr}{{{selected_text}}}")
        };
        expand_abbreviation(&with_content, language)
    }

    /// Returns completion suggestions for the given prefix.
    #[must_use]
    pub fn suggest_completions(&self, prefix: &str, language: &str) -> Vec<EmmetCompletionItem> {
        if !self.enabled || prefix.is_empty() || !self.supports_language(language) {
            return Vec::new();
        }
        suggest_emmet_completions(prefix, language)
    }
}

/// Result of expanding an Emmet abbreviation.
#[derive(Debug, Clone)]
pub struct EmmetAbbreviation {
    pub raw: String,
    pub expanded: String,
    pub cursor_position: Option<(u32, u32)>,
}

/// A completion item from Emmet suggestion.
#[derive(Debug, Clone)]
pub struct EmmetCompletionItem {
    pub label: String,
    pub detail: String,
    pub insert_text: String,
}

/// Determines whether the cursor position is in an Emmet-expandable context.
#[must_use]
pub fn is_emmet_context(language: &str, line_text: &str, column: u32) -> bool {
    if !HTML_LANGUAGES.contains(&language) && !CSS_LANGUAGES.contains(&language) {
        return false;
    }
    let col = column as usize;
    let before: String = line_text.chars().take(col).collect();
    let trimmed = before.trim_start();
    if trimmed.is_empty() {
        return false;
    }
    // Not inside a string or comment (simple heuristic)
    let in_string = before.chars().filter(|&c| c == '"' || c == '\'').count() % 2 != 0;
    if in_string {
        return false;
    }
    looks_like_abbreviation(trimmed.split_whitespace().last().unwrap_or(""), language)
}

/// Generates completion suggestions for a given prefix and language.
#[must_use]
pub fn suggest_emmet_completions(prefix: &str, syntax: &str) -> Vec<EmmetCompletionItem> {
    let mut items = Vec::new();

    if CSS_LANGUAGES.contains(&syntax) {
        for &(abbr, expansion) in CSS_SHORTHANDS {
            if abbr.starts_with(prefix) {
                items.push(EmmetCompletionItem {
                    label: abbr.to_string(),
                    detail: expansion.to_string(),
                    insert_text: expansion.to_string(),
                });
            }
        }
    } else if HTML_LANGUAGES.contains(&syntax) {
        let common_tags = [
            "div", "span", "p", "a", "ul", "ol", "li", "h1", "h2", "h3",
            "h4", "h5", "h6", "section", "article", "header", "footer",
            "nav", "main", "form", "input", "button", "table", "tr", "td",
            "th", "img", "link", "meta", "script", "style",
        ];
        for tag in &common_tags {
            if tag.starts_with(prefix) {
                if let Some(expanded) = expand_abbreviation(tag, syntax) {
                    items.push(EmmetCompletionItem {
                        label: tag.to_string(),
                        detail: expanded.clone(),
                        insert_text: expanded,
                    });
                }
            }
        }
    }
    items
}

fn find_cursor_position(text: &str) -> Option<(u32, u32)> {
    for (line_idx, line) in text.lines().enumerate() {
        if let Some(col) = line.find("></") {
            return Some((line_idx as u32, (col + 1) as u32));
        }
    }
    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return Some((line_idx as u32, line.len() as u32));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn html_simple_tag() {
        let out = expand_abbreviation("p", "html").unwrap();
        assert_eq!(out, "<p></p>");
    }

    #[test]
    fn html_class_and_id() {
        let out = expand_abbreviation("div.class#id", "html").unwrap();
        assert!(out.contains("class=\"class\""));
        assert!(out.contains("id=\"id\""));
    }

    #[test]
    fn html_nesting() {
        let out = expand_abbreviation("ul>li", "html").unwrap();
        assert!(out.contains("<ul>"));
        assert!(out.contains("<li>"));
        assert!(out.contains("</li>"));
        assert!(out.contains("</ul>"));
    }

    #[test]
    fn html_multiplication() {
        let out = expand_abbreviation("li*3", "html").unwrap();
        assert_eq!(out.matches("<li>").count(), 3);
    }

    #[test]
    fn html_text_content() {
        let out = expand_abbreviation("p{hello}", "html").unwrap();
        assert!(out.contains("hello"));
    }

    #[test]
    fn html_self_closing() {
        let out = expand_abbreviation("img", "html").unwrap();
        assert!(out.contains("/>"));
    }

    #[test]
    fn html_boilerplate() {
        let out = expand_abbreviation("!", "html").unwrap();
        assert!(out.contains("<!DOCTYPE html>"));
    }

    #[test]
    fn html_nested_with_multiply() {
        let out = expand_abbreviation("div.class#id>span*3", "html").unwrap();
        assert_eq!(out.matches("<span>").count(), 3);
        assert!(out.contains("id=\"id\""));
    }

    #[test]
    fn css_simple() {
        assert_eq!(expand_abbreviation("m0", "css").unwrap(), "margin: 0;");
        assert_eq!(expand_abbreviation("p0", "css").unwrap(), "padding: 0;");
        assert_eq!(expand_abbreviation("df", "css").unwrap(), "display: flex;");
    }

    #[test]
    fn css_numeric() {
        assert_eq!(expand_abbreviation("m10", "css").unwrap(), "margin: 10px;");
        assert_eq!(expand_abbreviation("p20", "css").unwrap(), "padding: 20px;");
        assert_eq!(
            expand_abbreviation("fs16", "css").unwrap(),
            "font-size: 16px;"
        );
    }

    #[test]
    fn css_numeric_with_unit() {
        assert_eq!(expand_abbreviation("w100%", "css").unwrap(), "width: 100%;");
        assert_eq!(
            expand_abbreviation("h50vh", "css").unwrap(),
            "height: 50vh;"
        );
    }

    #[test]
    fn unknown_css_returns_none() {
        assert!(expand_abbreviation("?!#$", "css").is_none());
    }

    #[test]
    fn looks_like_abbrev() {
        assert!(looks_like_abbreviation("div.foo", "html"));
        assert!(looks_like_abbreviation("m10", "css"));
        assert!(!looks_like_abbreviation("", "html"));
        assert!(!looks_like_abbreviation("hello world", "html"));
    }

    #[test]
    fn emmet_engine_expand() {
        let engine = EmmetEngine::new();
        let result = engine.expand("div.foo", "html").unwrap();
        assert!(result.expanded.contains("class=\"foo\""));
        assert_eq!(result.raw, "div.foo");
    }

    #[test]
    fn emmet_engine_unsupported_language() {
        let engine = EmmetEngine::new();
        assert!(engine.expand("div", "rust").is_none());
    }

    #[test]
    fn emmet_engine_disabled() {
        let mut engine = EmmetEngine::new();
        engine.enabled = false;
        assert!(engine.expand("div", "html").is_none());
    }

    #[test]
    fn emmet_context_detection() {
        assert!(is_emmet_context("html", "  div.foo", 9));
        assert!(!is_emmet_context("rust", "div.foo", 7));
        assert!(!is_emmet_context("html", "", 0));
    }

    #[test]
    fn emmet_suggestions() {
        let items = suggest_emmet_completions("m", "css");
        assert!(!items.is_empty());
        assert!(items.iter().any(|i| i.label.starts_with("m")));
    }

    #[test]
    fn emmet_html_suggestions() {
        let items = suggest_emmet_completions("di", "html");
        assert!(items.iter().any(|i| i.label == "div"));
    }

    #[test]
    fn wrap_with_abbreviation() {
        let engine = EmmetEngine::new();
        let wrapped = engine.wrap_with_abbreviation("hello", "p", "html").unwrap();
        assert!(wrapped.contains("<p>"));
        assert!(wrapped.contains("hello"));
    }
}
