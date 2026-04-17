//! # sidex-text
//!
//! Rope-based text buffer for the `SideX` editor.
//!
//! This crate provides the foundational text storage layer, replacing Monaco's
//! piece table with a Rust [`ropey::Rope`]-backed buffer. It supports efficient
//! editing of large files, position/offset conversions, UTF-16 interop for LSP,
//! and line-ending detection/normalization.

mod buffer;
pub mod diff;
mod edit;
pub mod encoding;
mod line_ending;
mod position;
mod range;
pub mod search;
pub mod text_model;
mod utf16;
pub mod word_boundary;

pub use buffer::{
    Buffer, BufferSnapshot, EditResult, IndentGuide, IndentInfo, WordAtPosition, WordInfo, WordType,
};
pub use edit::{ChangeEvent, EditOperation};
pub use encoding::{EncodingService, encoding_from_label, ALL_ENCODINGS};
pub use line_ending::{
    count_line_endings, detect_line_ending, line_ending_label, normalize_line_endings, LineEnding,
};
pub use position::Position;
pub use range::Range;
pub use search::{
    FindMatch, FindMatchesOptions, Match, SearchOptions, TextSearchEngine, LIMIT_FIND_COUNT,
};
pub use text_model::{TextModel, TextModelOptions};
pub use utf16::{
    char_col_to_utf16_col, lsp_position_to_position, position_to_lsp_position,
    utf16_col_to_char_col, Utf16Position,
};
pub use word_boundary::{
    default_word_definition, get_word_at_position, get_word_until_position, WordRange,
};
