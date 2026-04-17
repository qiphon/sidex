use serde::Serialize;
use sidex_editor::contrib::bracket_pair_colorization::compute_bracket_pairs;
use sidex_editor::contrib::color_decorators::detect_colors;
use sidex_editor::contrib::folding::FoldingModel;
use sidex_text::Buffer;

// ── Response structs ────────────────────────────────────────────────────────

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

// ── Commands ────────────────────────────────────────────────────────────────

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
#[tauri::command]
pub fn editor_detect_colors(line_text: String) -> Result<Vec<ColorInfo>, String> {
    let decorators = detect_colors(&line_text, 0);
    Ok(decorators
        .into_iter()
        .map(|d| ColorInfo {
            line: d.line,
            column: d.column,
            end_column: d.end_column,
            r: d.color.r,
            g: d.color.g,
            b: d.color.b,
            a: d.color.a,
            original_text: d.original_text,
        })
        .collect())
}

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
#[tauri::command]
pub fn editor_compute_bracket_pairs(content: String) -> Result<Vec<BracketPairInfo>, String> {
    let pairs = compute_bracket_pairs(&content, &[('(', ')'), ('[', ']'), ('{', '}')], 6);
    Ok(pairs
        .into_iter()
        .map(|p| BracketPairInfo {
            open: p.open,
            close: p.close,
            nesting_level: p.nesting_level,
            color_index: p.color_index,
        })
        .collect())
}

#[allow(clippy::needless_pass_by_value, clippy::unnecessary_wraps)]
#[tauri::command]
pub fn editor_compute_folding_ranges(
    content: String,
    language: String,
) -> Result<Vec<FoldRange>, String> {
    let buf = Buffer::from_str(&content);
    let mut regions = FoldingModel::compute_from_indentation(&buf, 4);

    let markers = FoldingModel::compute_from_markers(&buf, "#region", "#endregion");
    regions.extend(markers);

    let import_kw: &[&str] = match language.as_str() {
        "rust" => &["use "],
        "typescript" | "javascript" | "typescriptreact" | "javascriptreact" => &["import "],
        "python" => &["import ", "from "],
        "go" => &["import"],
        _ => &[],
    };
    if !import_kw.is_empty() {
        regions.extend(FoldingModel::compute_import_regions(&buf, import_kw));
    }

    regions.sort_by_key(|r| r.start_line);
    regions.dedup_by(|a, b| a.start_line == b.start_line && a.end_line == b.end_line);

    Ok(regions
        .into_iter()
        .map(|r| FoldRange {
            start_line: r.start_line,
            end_line: r.end_line,
            kind: r.kind.map(|k| match k {
                sidex_editor::contrib::folding::FoldKind::Region => "region",
                sidex_editor::contrib::folding::FoldKind::Imports => "imports",
                sidex_editor::contrib::folding::FoldKind::Comment => "comment",
            }.to_owned()),
        })
        .collect())
}
