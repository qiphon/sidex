//! Hover information display — mirrors VS Code's `ContentHoverController` +
//! `ContentHoverWidget`.
//!
//! Manages the state for showing hover tooltips with markdown content, code
//! blocks, and diagnostic information at a document position.

use sidex_text::{Position, Range};

/// A source/provider that contributed hover content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HoverSource {
    /// From the language server (type info, documentation).
    Language,
    /// From diagnostics (errors, warnings).
    Diagnostic,
    /// From a color decorator.
    ColorDecorator,
    /// From an extension.
    Extension(String),
}

/// The content to display inside a hover tooltip.
#[derive(Debug, Clone)]
pub struct HoverContent {
    /// The rendered content.
    pub value: HoverContentValue,
    /// Which provider produced this content.
    pub source: HoverSource,
    /// Optional range in the document this hover applies to.
    pub range: Option<Range>,
    /// Sort order (lower = higher priority, rendered first).
    pub priority: i32,
}

/// The actual value of a hover content entry.
#[derive(Debug, Clone)]
pub enum HoverContentValue {
    /// Markdown-formatted text.
    Markdown(String),
    /// A fenced code block with optional language tag.
    CodeBlock {
        language: Option<String>,
        code: String,
    },
}

/// Configuration for hover behaviour.
#[derive(Debug, Clone)]
pub struct HoverConfig {
    /// Whether hover is enabled. `"on"`, `"off"`, or `"onKeyboardModifier"`.
    pub enabled: HoverEnabled,
    /// Whether the hover should stay visible when the mouse moves over it.
    pub sticky: bool,
    /// Delay in milliseconds before showing the hover (default 500ms).
    pub delay_ms: u64,
    /// Delay in milliseconds before hiding (for sticky mode, default 300ms).
    pub hiding_delay_ms: u64,
}

/// Hover enabled mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HoverEnabled {
    On,
    Off,
    OnKeyboardModifier,
}

impl Default for HoverConfig {
    fn default() -> Self {
        Self {
            enabled: HoverEnabled::On,
            sticky: true,
            delay_ms: 500,
            hiding_delay_ms: 300,
        }
    }
}

/// Full state for the hover feature.
#[derive(Debug, Clone)]
pub struct HoverState {
    /// Whether the hover popup is currently visible.
    pub is_visible: bool,
    /// The document position that triggered the hover.
    pub position: Option<Position>,
    /// Content sections to render (merged from multiple providers).
    pub contents: Vec<HoverContent>,
    /// Configuration.
    pub config: HoverConfig,
    /// Whether a hover request is currently in-flight.
    pub is_loading: bool,
    /// Pixel coordinates for positioning (set by the renderer).
    pub anchor_x: f32,
    pub anchor_y: f32,
    /// Whether the mouse is currently over the hover widget (stay-open).
    pub mouse_over_widget: bool,
    /// Whether the mouse is currently down (for color picker interaction).
    pub mouse_down: bool,
    /// Number of pending provider responses.
    pub pending_providers: u32,
    /// Whether a keyboard modifier triggered this hover.
    pub triggered_by_keyboard: bool,
}

impl Default for HoverState {
    fn default() -> Self {
        Self {
            is_visible: false,
            position: None,
            contents: Vec::new(),
            config: HoverConfig::default(),
            is_loading: false,
            anchor_x: 0.0,
            anchor_y: 0.0,
            mouse_over_widget: false,
            mouse_down: false,
            pending_providers: 0,
            triggered_by_keyboard: false,
        }
    }
}

impl HoverState {
    /// Initiates a hover request at the given position.  The caller is
    /// responsible for scheduling the actual LSP request after `delay_ms`.
    pub fn request_hover(&mut self, pos: Position) {
        if self.config.enabled == HoverEnabled::Off {
            return;
        }
        self.position = Some(pos);
        self.is_loading = true;
        self.contents.clear();
        self.pending_providers = 0;
    }

    /// Initiates a hover from keyboard (e.g. Ctrl+K Ctrl+I).
    pub fn request_hover_keyboard(&mut self, pos: Position) {
        self.triggered_by_keyboard = true;
        self.request_hover(pos);
    }

    /// Registers that N providers will respond.
    pub fn expect_providers(&mut self, count: u32) {
        self.pending_providers = count;
    }

    /// Adds content from a single provider. When all providers have responded
    /// the hover becomes visible (if non-empty).
    pub fn add_provider_result(&mut self, contents: Vec<HoverContent>) {
        self.contents.extend(contents);
        self.pending_providers = self.pending_providers.saturating_sub(1);
        if self.pending_providers == 0 {
            self.finalize();
        }
    }

    /// Resolves a hover request with content from a single provider and makes
    /// the popup visible (convenience for single-provider case).
    pub fn show_hover(&mut self, pos: Position, contents: Vec<HoverContent>) {
        self.position = Some(pos);
        self.contents = contents;
        self.is_loading = false;
        self.is_visible = !self.contents.is_empty();
        self.sort_contents();
    }

    /// Hides the hover popup and clears its content.
    pub fn hide_hover(&mut self) {
        if self.should_stay_open() {
            return;
        }
        self.force_hide();
    }

    /// Force hides regardless of sticky state.
    pub fn force_hide(&mut self) {
        self.is_visible = false;
        self.is_loading = false;
        self.contents.clear();
        self.position = None;
        self.mouse_over_widget = false;
        self.triggered_by_keyboard = false;
    }

    /// Returns `true` when the hover should stay visible (mouse over widget
    /// in sticky mode, or mouse down on color picker).
    #[must_use]
    pub fn should_stay_open(&self) -> bool {
        if self.mouse_down {
            return true;
        }
        self.config.sticky && self.mouse_over_widget
    }

    /// Notifies that the mouse has entered/left the hover widget.
    pub fn set_mouse_over_widget(&mut self, over: bool) {
        self.mouse_over_widget = over;
    }

    /// Notifies mouse down/up state.
    pub fn set_mouse_down(&mut self, down: bool) {
        self.mouse_down = down;
    }

    /// Returns `true` when the hover has content to render.
    #[must_use]
    pub fn has_content(&self) -> bool {
        !self.contents.is_empty()
    }

    /// Returns only the markdown contents for simple rendering.
    #[must_use]
    pub fn markdown_contents(&self) -> Vec<&str> {
        self.contents
            .iter()
            .filter_map(|c| match &c.value {
                HoverContentValue::Markdown(s) => Some(s.as_str()),
                HoverContentValue::CodeBlock { .. } => None,
            })
            .collect()
    }

    /// Returns only code block contents.
    #[must_use]
    pub fn code_blocks(&self) -> Vec<(&Option<String>, &str)> {
        self.contents
            .iter()
            .filter_map(|c| match &c.value {
                HoverContentValue::CodeBlock { language, code } => Some((language, code.as_str())),
                HoverContentValue::Markdown(_) => None,
            })
            .collect()
    }

    /// Adds diagnostic content to the hover (shown when hovering over a squiggly).
    pub fn add_diagnostic_hover(
        &mut self,
        message: &str,
        severity_label: &str,
        source: Option<&str>,
        code: Option<&str>,
        has_quick_fixes: bool,
    ) {
        let mut md = format!("**{severity_label}**");
        if let Some(src) = source {
            md.push_str(&format!(" [{src}]"));
        }
        if let Some(c) = code {
            md.push_str(&format!("({c})"));
        }
        md.push_str(": ");
        md.push_str(message);

        if has_quick_fixes {
            md.push_str("\n\n---\n\nQuick Fix... (Ctrl+.)");
        }

        self.contents.push(HoverContent {
            value: HoverContentValue::Markdown(md),
            source: HoverSource::Diagnostic,
            range: self.position.map(|p| {
                sidex_text::Range::new(p, p)
            }),
            priority: -10,
        });

        if self.pending_providers == 0 && !self.is_loading {
            self.is_visible = !self.contents.is_empty();
            self.sort_contents();
        }
    }

    /// Convenience: creates a full diagnostic hover with all fields at a position.
    pub fn show_diagnostic_hover(
        &mut self,
        pos: Position,
        message: &str,
        severity_label: &str,
        source: Option<&str>,
        code: Option<&str>,
        has_quick_fixes: bool,
    ) {
        self.position = Some(pos);
        self.add_diagnostic_hover(message, severity_label, source, code, has_quick_fixes);
    }

    /// Returns `true` if the hover contains diagnostic content.
    #[must_use]
    pub fn has_diagnostic_content(&self) -> bool {
        self.contents
            .iter()
            .any(|c| c.source == HoverSource::Diagnostic)
    }

    /// Sets the pixel anchor for the tooltip.
    pub fn set_anchor(&mut self, x: f32, y: f32) {
        self.anchor_x = x;
        self.anchor_y = y;
    }

    fn finalize(&mut self) {
        self.is_loading = false;
        self.sort_contents();
        self.is_visible = !self.contents.is_empty();
    }

    fn sort_contents(&mut self) {
        self.contents.sort_by_key(|c| c.priority);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn md(text: &str) -> HoverContent {
        HoverContent {
            value: HoverContentValue::Markdown(text.to_string()),
            source: HoverSource::Language,
            range: None,
            priority: 0,
        }
    }

    #[test]
    fn show_and_hide() {
        let mut state = HoverState::default();
        assert!(!state.is_visible);

        let pos = Position::new(5, 10);
        state.show_hover(pos, vec![md("hello")]);
        assert!(state.is_visible);
        assert!(state.has_content());

        state.hide_hover();
        assert!(!state.is_visible);
        assert!(!state.has_content());
    }

    #[test]
    fn request_sets_loading() {
        let mut state = HoverState::default();
        state.request_hover(Position::new(1, 0));
        assert!(state.is_loading);
        assert!(!state.is_visible);
    }

    #[test]
    fn sticky_hover_stays_open() {
        let mut state = HoverState::default();
        state.config.sticky = true;
        state.show_hover(Position::new(0, 0), vec![md("hi")]);
        state.set_mouse_over_widget(true);
        assert!(state.should_stay_open());

        state.hide_hover();
        assert!(state.is_visible); // stayed open because sticky + mouse over
    }

    #[test]
    fn multi_provider_merge() {
        let mut state = HoverState::default();
        state.request_hover(Position::new(0, 0));
        state.expect_providers(2);

        state.add_provider_result(vec![HoverContent {
            value: HoverContentValue::Markdown("type info".into()),
            source: HoverSource::Language,
            range: None,
            priority: 0,
        }]);
        assert!(!state.is_visible); // still waiting

        state.add_provider_result(vec![HoverContent {
            value: HoverContentValue::Markdown("diagnostic".into()),
            source: HoverSource::Diagnostic,
            range: None,
            priority: 10,
        }]);
        assert!(state.is_visible); // all providers responded
        assert_eq!(state.contents.len(), 2);
        assert_eq!(state.contents[0].priority, 0); // sorted by priority
    }

    #[test]
    fn disabled_hover() {
        let mut state = HoverState::default();
        state.config.enabled = HoverEnabled::Off;
        state.request_hover(Position::new(0, 0));
        assert!(!state.is_loading);
    }

    #[test]
    fn diagnostic_hover_adds_content() {
        let mut state = HoverState::default();
        state.show_diagnostic_hover(
            Position::new(5, 10),
            "unused variable `x`",
            "Warning",
            Some("rust-analyzer"),
            Some("unused_variables"),
            true,
        );
        assert!(state.is_visible);
        assert!(state.has_diagnostic_content());
        let md = state.markdown_contents();
        assert!(!md.is_empty());
        assert!(md[0].contains("unused variable"));
        assert!(md[0].contains("rust-analyzer"));
        assert!(md[0].contains("Quick Fix"));
    }

    #[test]
    fn diagnostic_hover_without_quick_fixes() {
        let mut state = HoverState::default();
        state.show_diagnostic_hover(
            Position::new(1, 0),
            "type mismatch",
            "Error",
            None,
            None,
            false,
        );
        assert!(state.is_visible);
        let md = state.markdown_contents();
        assert!(!md[0].contains("Quick Fix"));
    }
}
