//! Code actions (quick fixes, refactorings) — mirrors VS Code's
//! `CodeActionController` + `CodeActionModel` + light-bulb logic.

use sidex_text::Range;

/// The kind of a code action, following LSP's `CodeActionKind`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CodeActionKind {
    QuickFix,
    Refactor,
    RefactorExtract,
    RefactorInline,
    RefactorRewrite,
    RefactorMove,
    Source,
    SourceOrganizeImports,
    SourceFixAll,
    Other(String),
}

impl CodeActionKind {
    /// Returns `true` if this kind is a sub-kind of `parent`.
    #[must_use]
    pub fn is_sub_kind_of(&self, parent: &CodeActionKind) -> bool {
        if self == parent {
            return true;
        }
        match (self, parent) {
            (
                CodeActionKind::RefactorExtract
                | CodeActionKind::RefactorInline
                | CodeActionKind::RefactorRewrite
                | CodeActionKind::RefactorMove,
                CodeActionKind::Refactor,
            ) => true,
            (
                CodeActionKind::SourceOrganizeImports | CodeActionKind::SourceFixAll,
                CodeActionKind::Source,
            ) => true,
            _ => false,
        }
    }

    /// Returns the LSP string representation.
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::QuickFix => "quickfix",
            Self::Refactor => "refactor",
            Self::RefactorExtract => "refactor.extract",
            Self::RefactorInline => "refactor.inline",
            Self::RefactorRewrite => "refactor.rewrite",
            Self::RefactorMove => "refactor.move",
            Self::Source => "source",
            Self::SourceOrganizeImports => "source.organizeImports",
            Self::SourceFixAll => "source.fixAll",
            Self::Other(s) => s,
        }
    }
}

/// A single code action returned by the language server.
#[derive(Debug, Clone)]
pub struct CodeAction {
    /// Human-readable title.
    pub title: String,
    /// The kind of code action.
    pub kind: Option<CodeActionKind>,
    /// Whether this is the preferred action for a given diagnostic.
    pub is_preferred: bool,
    /// Whether this action is disabled (with a reason).
    pub disabled_reason: Option<String>,
    /// Opaque data passed back to the server on apply.
    pub data: Option<String>,
    /// Associated diagnostics that this action fixes.
    pub diagnostics: Vec<String>,
}

/// A code action filter for requesting specific kinds.
#[derive(Debug, Clone, Default)]
pub struct CodeActionFilter {
    /// Only return actions of these kinds.
    pub include: Vec<CodeActionKind>,
    /// Exclude actions of these kinds.
    pub exclude: Vec<CodeActionKind>,
    /// Only include preferred actions.
    pub only_preferred: bool,
    /// Include disabled actions.
    pub include_disabled: bool,
}

impl CodeActionFilter {
    /// Returns `true` if the given action passes this filter.
    #[must_use]
    pub fn matches(&self, action: &CodeAction) -> bool {
        if self.only_preferred && !action.is_preferred {
            return false;
        }
        if !self.include_disabled && action.disabled_reason.is_some() {
            return false;
        }
        if let Some(kind) = &action.kind {
            if !self.include.is_empty() && !self.include.iter().any(|k| kind.is_sub_kind_of(k)) {
                return false;
            }
            if self.exclude.iter().any(|k| kind.is_sub_kind_of(k)) {
                return false;
            }
        }
        true
    }
}

/// How a code action was triggered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CodeActionTriggerType {
    /// Automatically triggered (cursor change, diagnostic change).
    Auto,
    /// Explicitly invoked by the user.
    Invoke,
}

/// Light-bulb indicator state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LightBulbVisibility {
    #[default]
    Hidden,
    QuickFix,
    Refactor,
}

/// Auto-fix-on-save configuration.
#[derive(Debug, Clone)]
pub struct AutoFixConfig {
    /// Whether to run `source.fixAll` on save.
    pub fix_all_on_save: bool,
    /// Whether to run `source.organizeImports` on save.
    pub organize_imports_on_save: bool,
    /// Specific code action kinds to run on save.
    pub on_save_kinds: Vec<CodeActionKind>,
}

impl Default for AutoFixConfig {
    fn default() -> Self {
        Self {
            fix_all_on_save: false,
            organize_imports_on_save: false,
            on_save_kinds: Vec::new(),
        }
    }
}

/// Full state for the code-action feature.
#[derive(Debug, Clone, Default)]
pub struct CodeActionState {
    /// Available code actions for the current cursor position / selection.
    pub actions: Vec<CodeAction>,
    /// The range for which actions were computed.
    pub trigger_range: Option<Range>,
    /// Whether actions are being fetched.
    pub is_loading: bool,
    /// Light-bulb visibility in the gutter.
    pub light_bulb: LightBulbVisibility,
    /// The line where the light bulb is shown.
    pub light_bulb_line: Option<u32>,
    /// How this request was triggered.
    pub trigger_type: Option<CodeActionTriggerType>,
    /// Auto-fix configuration.
    pub auto_fix: AutoFixConfig,
}

impl CodeActionState {
    /// Starts fetching code actions for the given range.
    pub fn request_code_actions(&mut self, range: Range) {
        self.request_code_actions_with_trigger(range, CodeActionTriggerType::Auto);
    }

    /// Starts fetching code actions with explicit trigger type.
    pub fn request_code_actions_with_trigger(
        &mut self,
        range: Range,
        trigger: CodeActionTriggerType,
    ) {
        self.trigger_range = Some(range);
        self.trigger_type = Some(trigger);
        self.is_loading = true;
        self.actions.clear();
        self.light_bulb = LightBulbVisibility::Hidden;
    }

    /// Receives code actions from the provider and updates light-bulb state.
    pub fn receive_actions(&mut self, actions: Vec<CodeAction>) {
        self.is_loading = false;
        self.actions = actions;
        self.update_light_bulb();
    }

    /// Clears current actions.
    pub fn clear(&mut self) {
        self.actions.clear();
        self.is_loading = false;
        self.light_bulb = LightBulbVisibility::Hidden;
        self.light_bulb_line = None;
        self.trigger_range = None;
        self.trigger_type = None;
    }

    /// Returns the preferred action, if one exists.
    #[must_use]
    pub fn preferred_action(&self) -> Option<&CodeAction> {
        self.actions.iter().find(|a| a.is_preferred)
    }

    /// Returns only quick-fix actions.
    #[must_use]
    pub fn quick_fixes(&self) -> Vec<&CodeAction> {
        self.actions
            .iter()
            .filter(|a| matches!(a.kind, Some(CodeActionKind::QuickFix)))
            .collect()
    }

    /// Returns only refactoring actions.
    #[must_use]
    pub fn refactorings(&self) -> Vec<&CodeAction> {
        self.actions
            .iter()
            .filter(|a| {
                matches!(
                    a.kind,
                    Some(
                        CodeActionKind::Refactor
                            | CodeActionKind::RefactorExtract
                            | CodeActionKind::RefactorInline
                            | CodeActionKind::RefactorRewrite
                            | CodeActionKind::RefactorMove
                    )
                )
            })
            .collect()
    }

    /// Returns only source actions.
    #[must_use]
    pub fn source_actions(&self) -> Vec<&CodeAction> {
        self.actions
            .iter()
            .filter(|a| {
                matches!(
                    a.kind,
                    Some(
                        CodeActionKind::Source
                            | CodeActionKind::SourceOrganizeImports
                            | CodeActionKind::SourceFixAll
                    )
                )
            })
            .collect()
    }

    /// Returns actions filtered by the given filter.
    #[must_use]
    pub fn filtered_actions(&self, filter: &CodeActionFilter) -> Vec<&CodeAction> {
        self.actions.iter().filter(|a| filter.matches(a)).collect()
    }

    /// Returns the code action kinds that should be auto-applied on save.
    #[must_use]
    pub fn on_save_actions(&self) -> Vec<&CodeAction> {
        let mut result = Vec::new();
        if self.auto_fix.fix_all_on_save {
            result.extend(
                self.actions
                    .iter()
                    .filter(|a| matches!(a.kind, Some(CodeActionKind::SourceFixAll))),
            );
        }
        if self.auto_fix.organize_imports_on_save {
            result.extend(
                self.actions
                    .iter()
                    .filter(|a| matches!(a.kind, Some(CodeActionKind::SourceOrganizeImports))),
            );
        }
        for kind in &self.auto_fix.on_save_kinds {
            result.extend(
                self.actions
                    .iter()
                    .filter(|a| a.kind.as_ref().is_some_and(|k| k.is_sub_kind_of(kind))),
            );
        }
        result
    }

    fn update_light_bulb(&mut self) {
        if self.actions.is_empty() {
            self.light_bulb = LightBulbVisibility::Hidden;
            self.light_bulb_line = None;
            return;
        }

        let has_quickfix = self
            .actions
            .iter()
            .any(|a| matches!(a.kind, Some(CodeActionKind::QuickFix)));
        self.light_bulb = if has_quickfix {
            LightBulbVisibility::QuickFix
        } else {
            LightBulbVisibility::Refactor
        };
        self.light_bulb_line = self.trigger_range.map(|r| r.start.line);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sidex_text::Position;

    fn make_action(title: &str, kind: CodeActionKind) -> CodeAction {
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
    fn light_bulb_shows_for_quickfix() {
        let mut state = CodeActionState::default();
        let range = Range::new(Position::new(5, 0), Position::new(5, 10));
        state.request_code_actions(range);
        state.receive_actions(vec![CodeAction {
            title: "Fix import".into(),
            kind: Some(CodeActionKind::QuickFix),
            is_preferred: true,
            disabled_reason: None,
            data: None,
            diagnostics: Vec::new(),
        }]);
        assert_eq!(state.light_bulb, LightBulbVisibility::QuickFix);
        assert_eq!(state.light_bulb_line, Some(5));
    }

    #[test]
    fn filter_by_kind() {
        let mut state = CodeActionState::default();
        state.receive_actions(vec![
            make_action("fix", CodeActionKind::QuickFix),
            make_action("extract", CodeActionKind::RefactorExtract),
            make_action("organize", CodeActionKind::SourceOrganizeImports),
        ]);
        assert_eq!(state.quick_fixes().len(), 1);
        assert_eq!(state.refactorings().len(), 1);
        assert_eq!(state.source_actions().len(), 1);
    }

    #[test]
    fn code_action_filter() {
        let filter = CodeActionFilter {
            include: vec![CodeActionKind::Refactor],
            exclude: vec![],
            only_preferred: false,
            include_disabled: false,
        };
        let action = make_action("extract", CodeActionKind::RefactorExtract);
        assert!(filter.matches(&action));

        let qf = make_action("fix", CodeActionKind::QuickFix);
        assert!(!filter.matches(&qf));
    }

    #[test]
    fn kind_hierarchy() {
        assert!(CodeActionKind::RefactorExtract.is_sub_kind_of(&CodeActionKind::Refactor));
        assert!(CodeActionKind::SourceFixAll.is_sub_kind_of(&CodeActionKind::Source));
        assert!(!CodeActionKind::QuickFix.is_sub_kind_of(&CodeActionKind::Refactor));
    }
}
