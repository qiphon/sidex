use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct ColorInfo {
    pub line: u32,
    pub column: u32,
    pub end_column: u32,
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
    pub original_text: String,
}

#[derive(Debug, Serialize)]
pub struct BracketPairInfo {
    pub open: char,
    pub close: char,
    pub nesting_level: u32,
    pub color_index: usize,
}

#[derive(Debug, Serialize)]
pub struct FoldRange {
    pub start_line: u32,
    pub end_line: u32,
    pub kind: Option<String>,
}

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
#[tauri::command]
pub fn editor_detect_colors(_line_text: String) -> Result<Vec<ColorInfo>, String> {
    Ok(Vec::new())
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::cast_possible_truncation
)]
#[tauri::command]
pub fn editor_compute_bracket_pairs(content: String) -> Result<Vec<BracketPairInfo>, String> {
    const COLORS: usize = 6;
    let pairs = [('(', ')'), ('[', ']'), ('{', '}')];
    let mut stack: Vec<(char, u32)> = Vec::new();
    let mut out: Vec<BracketPairInfo> = Vec::new();

    for ch in content.chars() {
        if let Some(&(_, close)) = pairs.iter().find(|p| p.0 == ch) {
            let level = stack.len() as u32;
            stack.push((close, level));
            out.push(BracketPairInfo {
                open: ch,
                close,
                nesting_level: level,
                color_index: (level as usize) % COLORS,
            });
        } else if let Some(pos) = stack.iter().rposition(|p| p.0 == ch) {
            stack.truncate(pos);
        }
    }
    Ok(out)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::unnecessary_wraps,
    clippy::cast_possible_truncation
)]
#[tauri::command]
pub fn editor_compute_folding_ranges(
    content: String,
    _language: String,
) -> Result<Vec<FoldRange>, String> {
    let lines: Vec<&str> = content.lines().collect();
    let mut ranges: Vec<FoldRange> = Vec::new();
    let indent_of = |line: &str| -> Option<u32> {
        if line.trim().is_empty() {
            return None;
        }
        let n = line.chars().take_while(|c| c.is_whitespace()).count();
        Some(n as u32)
    };

    let mut stack: Vec<(u32, u32)> = Vec::new();
    for (i, line) in lines.iter().enumerate() {
        let idx = i as u32;
        let Some(indent) = indent_of(line) else {
            continue;
        };
        while stack
            .last()
            .is_some_and(|&(top_indent, _)| indent <= top_indent)
        {
            let (_, start) = stack.pop().unwrap();
            if idx > 0 && idx - 1 > start {
                ranges.push(FoldRange {
                    start_line: start,
                    end_line: idx - 1,
                    kind: None,
                });
            }
        }
        stack.push((indent, idx));
    }
    let last_line = lines.len().saturating_sub(1) as u32;
    while let Some((_, start)) = stack.pop() {
        if last_line > start {
            ranges.push(FoldRange {
                start_line: start,
                end_line: last_line,
                kind: None,
            });
        }
    }
    ranges.sort_by_key(|r| r.start_line);
    Ok(ranges)
}

/// Ask the workbench to open a file (e.g. after a drag-and-drop from the OS).
/// Emits `sidex-open-file-preview`, which the frontend forwards to `vscode.open`.
#[tauri::command]
pub fn open_file_preview(app: tauri::AppHandle, path: String) -> Result<(), String> {
    use tauri::Emitter;
    app.emit("sidex-open-file-preview", path)
        .map_err(|e| format!("emit: {e}"))
}
