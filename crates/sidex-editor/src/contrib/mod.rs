//! Editor contributions — feature modules ported from VS Code's
//! `src/vs/editor/contrib/`.
//!
//! Each submodule encapsulates the state and logic for a single editor feature
//! (find/replace, folding, hover, autocomplete, etc.) as a self-contained unit
//! that can be driven by the GPU renderer or a Tauri command layer.

pub mod bookmarks;
pub mod bracket_matching;
pub mod bracket_pair_colorization;
pub mod breakpoints;
pub mod breadcrumbs;
pub mod clipboard_operations;
pub mod code_action;
pub mod code_action_widget;
pub mod codelens;
pub mod color_decorators;
pub mod color_picker;
pub mod comment;
pub mod diagnostics;
pub mod error_navigation;
pub mod find;
pub mod find_widget;
pub mod formatting;
pub mod folding;
pub mod go_to_line;
pub mod hover;
pub mod image_preview;
pub mod indent_guide;
pub mod inlay_hints;
pub mod lines_operations;
pub mod linked_editing;
pub mod minimap;
pub mod multicursor;
pub mod parameter_hints;
pub mod peek_view;
pub mod rename;
pub mod rename_widget;
pub mod scroll_decorations;
pub mod smart_select;
pub mod snippet_controller;
pub mod sticky_scroll;
pub mod suggest;
pub mod toggle_word;
pub mod whitespace_renderer;
pub mod word_highlighter;
pub mod word_wrap;

pub mod coverage;
pub mod emmet;
pub mod git_blame;
pub mod git_decorations;
pub mod inline_diff;
pub mod local_history;
pub mod testing;

pub mod auto_close;
pub mod auto_indent;
pub mod conflict_decorations;
pub mod inline_suggest;
pub mod json_features;
pub mod snippet_engine;
pub mod suggest_widget;
