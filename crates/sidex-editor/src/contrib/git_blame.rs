//! Inline blame annotations — GitLens-style blame shown at the end of lines.
//!
//! Shows the author, relative time, and commit message as dimmed text after
//! the last character on the current line.

use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Blame data for a single line.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BlameAnnotation {
    pub line: u32,
    pub author: String,
    pub date: String,
    pub message: String,
    pub commit_hash: String,
}

impl BlameAnnotation {
    /// Format the annotation for inline display.
    ///
    /// Example output: `"John Doe, 2 hours ago • Fix login bug"`
    pub fn inline_text(&self) -> String {
        let relative = format_relative_time(&self.date);
        format!("{}, {} \u{2022} {}", self.author, relative, self.message)
    }

    /// Format a detailed tooltip string for hover.
    pub fn tooltip(&self) -> String {
        let relative = format_relative_time(&self.date);
        format!(
            "Commit: {}\nAuthor: {}\nDate: {} ({})\n\n{}",
            self.commit_hash, self.author, self.date, relative, self.message,
        )
    }
}

/// State tracking for blame annotations across a document.
#[derive(Debug, Clone, Default)]
pub struct BlameState {
    pub annotations: HashMap<u32, BlameAnnotation>,
    pub current_line_annotation: Option<BlameAnnotation>,
    pub is_loading: bool,
    enabled: bool,
}

impl BlameState {
    pub fn new() -> Self {
        Self {
            enabled: true,
            ..Self::default()
        }
    }

    /// Whether inline blame display is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Toggle blame display on/off.
    pub fn toggle(&mut self) {
        self.enabled = !self.enabled;
    }

    /// Replace all cached annotations (typically after loading blame data).
    pub fn set_annotations(&mut self, annotations: Vec<BlameAnnotation>) {
        self.annotations.clear();
        for ann in annotations {
            self.annotations.insert(ann.line, ann);
        }
        self.is_loading = false;
    }

    /// Clear all blame data (e.g. when switching files).
    pub fn clear(&mut self) {
        self.annotations.clear();
        self.current_line_annotation = None;
        self.is_loading = false;
    }

    /// Update the annotation shown on the current cursor line.
    pub fn update_current_line(&mut self, line: u32) {
        self.current_line_annotation = self.annotations.get(&line).cloned();
    }

    /// Get the annotation for a specific line, if loaded.
    pub fn annotation_at(&self, line: u32) -> Option<&BlameAnnotation> {
        self.annotations.get(&line)
    }

    /// Mark that blame data is being loaded asynchronously.
    pub fn start_loading(&mut self) {
        self.is_loading = true;
    }

    /// The inline text for the current cursor line (or `None` if unavailable).
    pub fn current_inline_text(&self) -> Option<String> {
        if !self.enabled {
            return None;
        }
        self.current_line_annotation
            .as_ref()
            .map(BlameAnnotation::inline_text)
    }
}

/// Rendering style for blame annotations.
#[derive(Debug, Clone, Copy)]
pub struct BlameStyle {
    /// Horizontal gap between the end of text and the annotation, in pixels.
    pub left_margin: f32,
    /// Opacity of the annotation text (0.0 = invisible, 1.0 = fully opaque).
    pub opacity: f32,
    /// Font size for the annotation text.
    pub font_size: f32,
}

impl Default for BlameStyle {
    fn default() -> Self {
        Self {
            left_margin: 24.0,
            opacity: 0.45,
            font_size: 12.0,
        }
    }
}

/// Convert a Unix timestamp (as a string) to a relative time description.
fn format_relative_time(date_str: &str) -> String {
    let Ok(ts) = date_str.parse::<u64>() else {
        return date_str.to_string();
    };

    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);

    if now < ts {
        return "just now".to_string();
    }

    let diff = now - ts;
    let minutes = diff / 60;
    let hours = minutes / 60;
    let days = hours / 24;
    let weeks = days / 7;
    let months = days / 30;
    let years = days / 365;

    if years > 0 {
        plural(years, "year")
    } else if months > 0 {
        plural(months, "month")
    } else if weeks > 0 {
        plural(weeks, "week")
    } else if days > 0 {
        plural(days, "day")
    } else if hours > 0 {
        plural(hours, "hour")
    } else if minutes > 0 {
        plural(minutes, "minute")
    } else {
        "just now".to_string()
    }
}

fn plural(n: u64, unit: &str) -> String {
    if n == 1 {
        format!("1 {unit} ago")
    } else {
        format!("{n} {unit}s ago")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_text_format() {
        let ann = BlameAnnotation {
            line: 1,
            author: "Alice".to_string(),
            date: "0".to_string(),
            message: "initial commit".to_string(),
            commit_hash: "abc1234".to_string(),
        };
        let text = ann.inline_text();
        assert!(text.contains("Alice"));
        assert!(text.contains("initial commit"));
        assert!(text.contains('\u{2022}'));
    }

    #[test]
    fn tooltip_contains_details() {
        let ann = BlameAnnotation {
            line: 5,
            author: "Bob".to_string(),
            date: "1700000000".to_string(),
            message: "fix bug".to_string(),
            commit_hash: "def5678".to_string(),
        };
        let tt = ann.tooltip();
        assert!(tt.contains("def5678"));
        assert!(tt.contains("Bob"));
    }

    #[test]
    fn blame_state_toggle() {
        let mut state = BlameState::new();
        assert!(state.is_enabled());
        state.toggle();
        assert!(!state.is_enabled());
        assert!(state.current_inline_text().is_none());
    }

    #[test]
    fn blame_state_set_and_query() {
        let mut state = BlameState::new();
        state.set_annotations(vec![
            BlameAnnotation {
                line: 1,
                author: "A".to_string(),
                date: "0".to_string(),
                message: "msg".to_string(),
                commit_hash: "aaa".to_string(),
            },
            BlameAnnotation {
                line: 2,
                author: "B".to_string(),
                date: "0".to_string(),
                message: "msg2".to_string(),
                commit_hash: "bbb".to_string(),
            },
        ]);

        assert!(state.annotation_at(1).is_some());
        assert!(state.annotation_at(3).is_none());

        state.update_current_line(2);
        let text = state.current_inline_text().unwrap();
        assert!(text.contains('B'));
    }

    #[test]
    fn clear_resets_state() {
        let mut state = BlameState::new();
        state.set_annotations(vec![BlameAnnotation {
            line: 1,
            author: "A".to_string(),
            date: "0".to_string(),
            message: "m".to_string(),
            commit_hash: "x".to_string(),
        }]);
        state.update_current_line(1);
        state.clear();
        assert!(state.annotations.is_empty());
        assert!(state.current_line_annotation.is_none());
    }

    #[test]
    fn relative_time_units() {
        assert_eq!(format_relative_time("not-a-number"), "not-a-number");
    }
}
