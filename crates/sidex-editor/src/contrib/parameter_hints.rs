//! Signature help / parameter hints — mirrors VS Code's
//! `ParameterHintsModel` + `ParameterHintsWidget`.

use sidex_text::Position;

/// A single parameter in a signature.
#[derive(Debug, Clone)]
pub struct ParameterInfo {
    /// Display label for this parameter (e.g. `x: i32`).
    pub label: String,
    /// Optional documentation.
    pub documentation: Option<String>,
}

/// A function/method signature displayed in the parameter hints popup.
#[derive(Debug, Clone)]
pub struct SignatureInfo {
    /// The full signature label (e.g. `fn foo(x: i32, y: &str) -> bool`).
    pub label: String,
    /// Optional documentation for the function.
    pub documentation: Option<String>,
    /// The parameters of this signature.
    pub parameters: Vec<ParameterInfo>,
}

/// Full state for the parameter-hints feature.
#[derive(Debug, Clone, Default)]
pub struct ParameterHintState {
    /// Whether the hints popup is visible.
    pub is_visible: bool,
    /// Available overloaded signatures.
    pub signatures: Vec<SignatureInfo>,
    /// Index of the currently displayed signature.
    pub active_signature: usize,
    /// Index of the currently highlighted parameter.
    pub active_parameter: usize,
    /// The position at which hints were triggered.
    pub trigger_position: Option<Position>,
    /// Characters that trigger signature help (e.g. `(`, `,`).
    pub trigger_characters: Vec<char>,
    /// Characters that re-trigger after already visible (e.g. `,`).
    pub retrigger_characters: Vec<char>,
}

impl ParameterHintState {
    /// Shows parameter hints with the given signatures.
    pub fn show(&mut self, pos: Position, signatures: Vec<SignatureInfo>, active_param: usize) {
        self.is_visible = !signatures.is_empty();
        self.trigger_position = Some(pos);
        self.signatures = signatures;
        self.active_signature = 0;
        self.active_parameter = active_param;
    }

    /// Hides the parameter hints popup.
    pub fn hide(&mut self) {
        self.is_visible = false;
        self.signatures.clear();
        self.active_signature = 0;
        self.active_parameter = 0;
        self.trigger_position = None;
    }

    /// Cycles to the next overloaded signature.
    pub fn next_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = (self.active_signature + 1) % self.signatures.len();
        }
    }

    /// Cycles to the previous overloaded signature.
    pub fn prev_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = if self.active_signature == 0 {
                self.signatures.len() - 1
            } else {
                self.active_signature - 1
            };
        }
    }

    /// Updates the active parameter index (e.g. when the user types a comma).
    pub fn set_active_parameter(&mut self, idx: usize) {
        self.active_parameter = idx;
    }

    /// Returns the currently active signature, if any.
    #[must_use]
    pub fn current_signature(&self) -> Option<&SignatureInfo> {
        self.signatures.get(self.active_signature)
    }

    /// Returns the currently highlighted parameter info, if any.
    #[must_use]
    pub fn current_parameter(&self) -> Option<&ParameterInfo> {
        self.current_signature()
            .and_then(|sig| sig.parameters.get(self.active_parameter))
    }

    /// Returns `true` if the given character should trigger signature help.
    #[must_use]
    pub fn is_trigger_char(&self, ch: char) -> bool {
        self.trigger_characters.contains(&ch)
    }

    /// Returns `true` if the given character should re-trigger while visible.
    #[must_use]
    pub fn is_retrigger_char(&self, ch: char) -> bool {
        self.retrigger_characters.contains(&ch)
    }

    /// Checks a typed character and returns `true` if the widget should
    /// dismiss (e.g. `)` closes hints).
    #[must_use]
    pub fn should_dismiss_on(&self, ch: char) -> bool {
        ch == ')'
    }

    /// Returns `(active_signature_1based, total_signatures)` for display.
    #[must_use]
    pub fn signature_count_display(&self) -> (usize, usize) {
        let total = self.signatures.len();
        if total == 0 {
            (0, 0)
        } else {
            (self.active_signature + 1, total)
        }
    }
}

// ── Display-ready types for the renderer ─────────────────────────

/// A single parameter prepared for rendering, with an `is_active` flag
/// to bold the current parameter.
#[derive(Debug, Clone)]
pub struct ParameterDisplay {
    /// Parameter label text.
    pub label: String,
    /// Optional parameter documentation.
    pub documentation: Option<String>,
    /// Whether this parameter is the currently active one (bold).
    pub is_active: bool,
}

/// A signature prepared for rendering.
#[derive(Debug, Clone)]
pub struct SignatureDisplay {
    /// Full signature label.
    pub label: String,
    /// Parameters with active-parameter marking.
    pub parameters: Vec<ParameterDisplay>,
    /// Optional signature-level documentation.
    pub documentation: Option<String>,
}

impl SignatureDisplay {
    /// Builds a `SignatureDisplay` from a `SignatureInfo` and an active
    /// parameter index.
    #[must_use]
    pub fn from_info(info: &SignatureInfo, active_parameter: usize) -> Self {
        let parameters = info
            .parameters
            .iter()
            .enumerate()
            .map(|(i, p)| ParameterDisplay {
                label: p.label.clone(),
                documentation: p.documentation.clone(),
                is_active: i == active_parameter,
            })
            .collect();
        Self {
            label: info.label.clone(),
            parameters,
            documentation: info.documentation.clone(),
        }
    }
}

/// The full parameter hints widget view-model exposed to the renderer.
#[derive(Debug, Clone)]
pub struct ParameterHintsWidget {
    /// Whether the widget is visible.
    pub visible: bool,
    /// Rendered signatures with active-parameter marking.
    pub signatures: Vec<SignatureDisplay>,
    /// Index of the currently displayed signature.
    pub active_signature: usize,
    /// Index of the active parameter.
    pub active_parameter: usize,
    /// Pixel position for the popup `(x, y)`.
    pub position: (f32, f32),
    /// Whether multiple overloads are available (shows up/down arrows).
    pub has_overloads: bool,
}

impl Default for ParameterHintsWidget {
    fn default() -> Self {
        Self {
            visible: false,
            signatures: Vec::new(),
            active_signature: 0,
            active_parameter: 0,
            position: (0.0, 0.0),
            has_overloads: false,
        }
    }
}

impl ParameterHintsWidget {
    /// Builds the widget from a `ParameterHintState` and a pixel position.
    #[must_use]
    pub fn from_state(state: &ParameterHintState, position: (f32, f32)) -> Self {
        if !state.is_visible {
            return Self::default();
        }
        let signatures: Vec<SignatureDisplay> = state
            .signatures
            .iter()
            .map(|sig| SignatureDisplay::from_info(sig, state.active_parameter))
            .collect();
        Self {
            visible: state.is_visible,
            signatures,
            active_signature: state.active_signature,
            active_parameter: state.active_parameter,
            position,
            has_overloads: state.signatures.len() > 1,
        }
    }

    /// Shows the widget with the given signatures at the given position.
    pub fn show(&mut self, signatures: Vec<SignatureDisplay>, position: (f32, f32)) {
        if signatures.is_empty() {
            self.dismiss();
            return;
        }
        self.has_overloads = signatures.len() > 1;
        self.signatures = signatures;
        self.active_signature = 0;
        self.active_parameter = 0;
        self.position = position;
        self.visible = true;
    }

    /// Updates which parameter is bolded.
    pub fn update_active_parameter(&mut self, index: usize) {
        self.active_parameter = index;
        for sig in &mut self.signatures {
            for (i, param) in sig.parameters.iter_mut().enumerate() {
                param.is_active = i == index;
            }
        }
    }

    /// Cycles to the next overloaded signature.
    pub fn next_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = (self.active_signature + 1) % self.signatures.len();
        }
    }

    /// Cycles to the previous overloaded signature.
    pub fn prev_signature(&mut self) {
        if !self.signatures.is_empty() {
            self.active_signature = if self.active_signature == 0 {
                self.signatures.len() - 1
            } else {
                self.active_signature - 1
            };
        }
    }

    /// Returns the currently active signature for rendering.
    #[must_use]
    pub fn current_signature(&self) -> Option<&SignatureDisplay> {
        self.signatures.get(self.active_signature)
    }

    /// Returns the active parameter documentation (for the docs area).
    #[must_use]
    pub fn active_parameter_docs(&self) -> Option<&str> {
        self.current_signature()
            .and_then(|sig| sig.parameters.get(self.active_parameter))
            .and_then(|p| p.documentation.as_deref())
    }

    /// Dismisses the widget.
    pub fn dismiss(&mut self) {
        self.visible = false;
        self.signatures.clear();
        self.active_signature = 0;
        self.active_parameter = 0;
        self.has_overloads = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_sig() -> SignatureInfo {
        SignatureInfo {
            label: "fn foo(a: i32, b: &str)".into(),
            documentation: None,
            parameters: vec![
                ParameterInfo {
                    label: "a: i32".into(),
                    documentation: None,
                },
                ParameterInfo {
                    label: "b: &str".into(),
                    documentation: None,
                },
            ],
        }
    }

    #[test]
    fn show_and_navigate() {
        let mut state = ParameterHintState::default();
        state.show(Position::new(1, 5), vec![make_sig(), make_sig()], 0);
        assert!(state.is_visible);
        assert_eq!(state.active_signature, 0);

        state.next_signature();
        assert_eq!(state.active_signature, 1);

        state.next_signature();
        assert_eq!(state.active_signature, 0); // wraps
    }

    #[test]
    fn current_parameter() {
        let mut state = ParameterHintState::default();
        state.show(Position::new(0, 0), vec![make_sig()], 1);
        let param = state.current_parameter().unwrap();
        assert_eq!(param.label, "b: &str");
    }

    // ── Auto-trigger tests ───────────────────────────────────────

    #[test]
    fn trigger_and_retrigger_chars() {
        let mut state = ParameterHintState::default();
        state.trigger_characters = vec!['(', ','];
        state.retrigger_characters = vec![','];

        assert!(state.is_trigger_char('('));
        assert!(state.is_trigger_char(','));
        assert!(!state.is_trigger_char(')'));

        assert!(state.is_retrigger_char(','));
        assert!(!state.is_retrigger_char('('));
    }

    #[test]
    fn dismiss_on_close_paren() {
        let state = ParameterHintState::default();
        assert!(state.should_dismiss_on(')'));
        assert!(!state.should_dismiss_on(','));
    }

    #[test]
    fn signature_count_display() {
        let mut state = ParameterHintState::default();
        assert_eq!(state.signature_count_display(), (0, 0));

        state.show(Position::new(0, 0), vec![make_sig(), make_sig()], 0);
        assert_eq!(state.signature_count_display(), (1, 2));
        state.next_signature();
        assert_eq!(state.signature_count_display(), (2, 2));
    }

    // ── SignatureDisplay tests ───────────────────────────────────

    #[test]
    fn signature_display_marks_active() {
        let sig = make_sig();
        let display = SignatureDisplay::from_info(&sig, 0);
        assert!(display.parameters[0].is_active);
        assert!(!display.parameters[1].is_active);

        let display2 = SignatureDisplay::from_info(&sig, 1);
        assert!(!display2.parameters[0].is_active);
        assert!(display2.parameters[1].is_active);
    }

    // ── ParameterHintsWidget tests ───────────────────────────────

    #[test]
    fn widget_from_state() {
        let mut state = ParameterHintState::default();
        state.show(Position::new(1, 5), vec![make_sig(), make_sig()], 0);

        let widget = ParameterHintsWidget::from_state(&state, (100.0, 50.0));
        assert!(widget.visible);
        assert_eq!(widget.signatures.len(), 2);
        assert!(widget.has_overloads);
        assert_eq!(widget.position, (100.0, 50.0));
    }

    #[test]
    fn widget_from_hidden_state() {
        let state = ParameterHintState::default();
        let widget = ParameterHintsWidget::from_state(&state, (0.0, 0.0));
        assert!(!widget.visible);
    }

    #[test]
    fn widget_show_and_dismiss() {
        let mut widget = ParameterHintsWidget::default();
        let sigs = vec![
            SignatureDisplay::from_info(&make_sig(), 0),
            SignatureDisplay::from_info(&make_sig(), 0),
        ];
        widget.show(sigs, (50.0, 60.0));
        assert!(widget.visible);
        assert!(widget.has_overloads);

        widget.dismiss();
        assert!(!widget.visible);
        assert!(widget.signatures.is_empty());
    }

    #[test]
    fn widget_show_empty_dismisses() {
        let mut widget = ParameterHintsWidget::default();
        widget.show(vec![], (0.0, 0.0));
        assert!(!widget.visible);
    }

    #[test]
    fn widget_navigate_signatures() {
        let mut widget = ParameterHintsWidget::default();
        let sigs = vec![
            SignatureDisplay::from_info(&make_sig(), 0),
            SignatureDisplay::from_info(&make_sig(), 0),
        ];
        widget.show(sigs, (0.0, 0.0));

        assert_eq!(widget.active_signature, 0);
        widget.next_signature();
        assert_eq!(widget.active_signature, 1);
        widget.next_signature();
        assert_eq!(widget.active_signature, 0);
        widget.prev_signature();
        assert_eq!(widget.active_signature, 1);
    }

    #[test]
    fn widget_update_active_parameter() {
        let mut widget = ParameterHintsWidget::default();
        let sigs = vec![SignatureDisplay::from_info(&make_sig(), 0)];
        widget.show(sigs, (0.0, 0.0));

        assert!(widget.signatures[0].parameters[0].is_active);
        widget.update_active_parameter(1);
        assert!(!widget.signatures[0].parameters[0].is_active);
        assert!(widget.signatures[0].parameters[1].is_active);
    }

    #[test]
    fn widget_active_parameter_docs() {
        let mut sig = make_sig();
        sig.parameters[1].documentation = Some("second param doc".to_string());

        let mut widget = ParameterHintsWidget::default();
        let sigs = vec![SignatureDisplay::from_info(&sig, 1)];
        widget.show(sigs, (0.0, 0.0));
        widget.update_active_parameter(1);

        assert_eq!(widget.active_parameter_docs(), Some("second param doc"));
    }
}
