//! Code action lightbulb and quick fix menu widget — renders the gutter
//! lightbulb indicator and the pop-up action picker triggered by Ctrl+.
//! or clicking the lightbulb.

use super::code_action::{CodeAction, CodeActionKind, CodeActionState, LightBulbVisibility};

/// RGBA colour used for the quick-fix (yellow) lightbulb.
pub const LIGHTBULB_QUICKFIX_COLOR: [f32; 4] = [0.94, 0.76, 0.06, 1.0];
/// RGBA colour used for the refactor (blue) lightbulb.
pub const LIGHTBULB_REFACTOR_COLOR: [f32; 4] = [0.24, 0.56, 0.94, 1.0];

/// A single entry in the code-action menu.
#[derive(Debug, Clone)]
pub struct CodeActionMenuItem {
    pub action: CodeAction,
    pub is_separator_above: bool,
}

/// Full state for the code-action lightbulb + picker menu.
#[derive(Debug, Clone)]
pub struct CodeActionWidget {
    pub visible: bool,
    pub position: (f32, f32),
    pub actions: Vec<CodeActionMenuItem>,
    pub selected_index: usize,
    pub lightbulb_line: Option<u32>,
    pub lightbulb_kind: LightBulbVisibility,
}

impl Default for CodeActionWidget {
    fn default() -> Self {
        Self {
            visible: false,
            position: (0.0, 0.0),
            actions: Vec::new(),
            selected_index: 0,
            lightbulb_line: None,
            lightbulb_kind: LightBulbVisibility::Hidden,
        }
    }
}

impl CodeActionWidget {
    /// Updates the gutter lightbulb from the current [`CodeActionState`].
    pub fn sync_lightbulb(&mut self, state: &CodeActionState) {
        self.lightbulb_kind = state.light_bulb;
        self.lightbulb_line = state.light_bulb_line;
    }

    /// Shows a lightbulb in the gutter at `line` for the given actions.
    pub fn show_lightbulb(&mut self, line: u32, actions: &[CodeAction]) {
        self.lightbulb_line = Some(line);
        self.lightbulb_kind = lightbulb_kind_for(actions);
    }

    /// Hides the gutter lightbulb.
    pub fn hide_lightbulb(&mut self) {
        self.lightbulb_line = None;
        self.lightbulb_kind = LightBulbVisibility::Hidden;
    }

    /// Returns the RGBA colour for the current lightbulb state.
    #[must_use]
    pub fn lightbulb_color(&self) -> Option<[f32; 4]> {
        match self.lightbulb_kind {
            LightBulbVisibility::QuickFix => Some(LIGHTBULB_QUICKFIX_COLOR),
            LightBulbVisibility::Refactor => Some(LIGHTBULB_REFACTOR_COLOR),
            LightBulbVisibility::Hidden => None,
        }
    }

    /// Opens the code-action menu at `position` with grouped actions.
    pub fn show_menu(&mut self, position: (f32, f32), actions: Vec<CodeAction>) {
        self.position = position;
        self.actions = build_menu_items(actions);
        self.selected_index = 0;
        self.visible = true;
    }

    /// Closes the menu and resets selection.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.actions.clear();
        self.selected_index = 0;
    }

    pub fn select_next(&mut self) {
        if !self.actions.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.actions.len();
        }
    }

    pub fn select_previous(&mut self) {
        if !self.actions.is_empty() {
            self.selected_index = self
                .selected_index
                .checked_sub(1)
                .unwrap_or(self.actions.len() - 1);
        }
    }

    pub fn select_first(&mut self) {
        self.selected_index = 0;
    }

    pub fn select_last(&mut self) {
        if !self.actions.is_empty() {
            self.selected_index = self.actions.len() - 1;
        }
    }

    /// Returns the currently highlighted menu item, if any.
    #[must_use]
    pub fn selected_action(&self) -> Option<&CodeAction> {
        self.actions.get(self.selected_index).map(|i| &i.action)
    }

    /// Accepts the currently selected action and closes the menu.
    pub fn accept_selected(&mut self) -> Option<CodeAction> {
        let action = self.actions.get(self.selected_index).map(|i| i.action.clone());
        self.dismiss();
        action
    }

    #[must_use]
    pub fn has_lightbulb(&self) -> bool {
        self.lightbulb_kind != LightBulbVisibility::Hidden && self.lightbulb_line.is_some()
    }

    #[must_use]
    pub fn item_count(&self) -> usize {
        self.actions.len()
    }

    #[must_use]
    pub fn is_menu_visible(&self) -> bool {
        self.visible
    }
}

fn lightbulb_kind_for(actions: &[CodeAction]) -> LightBulbVisibility {
    if actions.is_empty() {
        return LightBulbVisibility::Hidden;
    }
    let has_qf = actions.iter().any(|a| matches!(a.kind, Some(CodeActionKind::QuickFix)));
    if has_qf { LightBulbVisibility::QuickFix } else { LightBulbVisibility::Refactor }
}

fn build_menu_items(actions: Vec<CodeAction>) -> Vec<CodeActionMenuItem> {
    if actions.is_empty() {
        return Vec::new();
    }
    let (mut qf, mut rf, mut sr, mut ot) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    for action in actions {
        match &action.kind {
            Some(CodeActionKind::QuickFix) => qf.push(action),
            Some(
                CodeActionKind::Refactor
                | CodeActionKind::RefactorExtract
                | CodeActionKind::RefactorInline
                | CodeActionKind::RefactorRewrite
                | CodeActionKind::RefactorMove,
            ) => rf.push(action),
            Some(
                CodeActionKind::Source
                | CodeActionKind::SourceOrganizeImports
                | CodeActionKind::SourceFixAll,
            ) => sr.push(action),
            _ => ot.push(action),
        }
    }
    let mut items = Vec::new();
    for group in [qf, rf, sr, ot] {
        if group.is_empty() {
            continue;
        }
        let sep = !items.is_empty();
        for (i, action) in group.into_iter().enumerate() {
            items.push(CodeActionMenuItem {
                action,
                is_separator_above: sep && i == 0,
            });
        }
    }
    items
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make(title: &str, kind: CodeActionKind) -> CodeAction {
        CodeAction {
            title: title.into(),
            kind: Some(kind),
            is_preferred: false,
            disabled_reason: None,
            data: None,
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn lightbulb_colors() {
        let mut w = CodeActionWidget::default();
        w.show_lightbulb(10, &[make("fix", CodeActionKind::QuickFix)]);
        assert_eq!(w.lightbulb_color(), Some(LIGHTBULB_QUICKFIX_COLOR));
        w.show_lightbulb(10, &[make("extract", CodeActionKind::RefactorExtract)]);
        assert_eq!(w.lightbulb_color(), Some(LIGHTBULB_REFACTOR_COLOR));
    }

    #[test]
    fn menu_grouping_and_separators() {
        let items = build_menu_items(vec![
            make("extract", CodeActionKind::RefactorExtract),
            make("fix import", CodeActionKind::QuickFix),
            make("organize", CodeActionKind::SourceOrganizeImports),
        ]);
        assert_eq!(items.len(), 3);
        assert_eq!(items[0].action.title, "fix import");
        assert!(!items[0].is_separator_above);
        assert!(items[1].is_separator_above);
    }

    #[test]
    fn keyboard_nav_and_accept() {
        let mut w = CodeActionWidget::default();
        w.show_menu((0.0, 0.0), vec![make("a", CodeActionKind::QuickFix), make("b", CodeActionKind::QuickFix)]);
        assert_eq!(w.selected_index, 0);
        w.select_next();
        assert_eq!(w.selected_index, 1);
        w.select_previous();
        assert_eq!(w.selected_index, 0);
        let action = w.accept_selected().unwrap();
        assert_eq!(action.title, "a");
        assert!(!w.visible);
    }

    #[test]
    fn dismiss_resets_state() {
        let mut w = CodeActionWidget::default();
        w.show_menu((50.0, 60.0), vec![make("fix", CodeActionKind::QuickFix)]);
        w.dismiss();
        assert!(!w.visible);
        assert!(w.actions.is_empty());
    }
}
