//! Compiled rules.
//!
//! Port of the `Rule` hierarchy from upstream. `TextMate` has four
//! runtime rule shapes — `MatchRule`, `BeginEndRule`, `BeginWhileRule`,
//! `IncludeOnlyRule` — plus the special `CaptureRule` that only appears
//! as an entry inside a capture table. We represent them as an enum so
//! the tokenizer hot path can dispatch with a single match.

use crate::utils::CaptureIndex;

use super::raw::Location;
use super::regex_source::RegExpSource;
use super::source_list::RegExpSourceList;
use super::RuleId;

/// Special rule-id sentinels from upstream. `-1` marks a begin/end end
/// pattern, `-2` marks a begin/while while pattern. The tokenizer uses
/// these values in the same slot the compiled scanner returns.
pub const END_RULE_ID: i32 = -1;
pub const WHILE_RULE_ID: i32 = -2;

/// Metadata shared by every rule variant — `$location`, scope name(s),
/// id. Matches the abstract `Rule` base class.
#[derive(Debug, Clone)]
pub struct RuleHeader {
    pub location: Option<Location>,
    pub id: RuleId,
    pub name: Option<String>,
    pub name_is_capturing: bool,
    pub content_name: Option<String>,
    pub content_name_is_capturing: bool,
}

impl RuleHeader {
    pub fn new(
        location: Option<Location>,
        id: RuleId,
        name: Option<String>,
        content_name: Option<String>,
    ) -> Self {
        let name_is_capturing = crate::utils::regex_source::has_captures(name.as_deref());
        let content_name_is_capturing =
            crate::utils::regex_source::has_captures(content_name.as_deref());
        Self {
            location,
            id,
            name,
            name_is_capturing,
            content_name,
            content_name_is_capturing,
        }
    }

    /// Upstream's `getName(lineText, captures)` — substitutes `$N`
    /// captures into the scope name when it references them.
    pub fn name_with_captures(
        &self,
        line_text: Option<&str>,
        captures: Option<&[Option<CaptureIndex>]>,
    ) -> Option<String> {
        match &self.name {
            None => None,
            Some(name) if !self.name_is_capturing => Some(name.clone()),
            Some(name) => match (line_text, captures) {
                (Some(text), Some(caps)) => Some(crate::utils::regex_source::replace_captures(
                    name, text, caps,
                )),
                _ => Some(name.clone()),
            },
        }
    }

    /// Upstream's `getContentName(lineText, captures)`.
    pub fn content_name_with_captures(
        &self,
        line_text: &str,
        captures: &[Option<CaptureIndex>],
    ) -> Option<String> {
        match &self.content_name {
            None => None,
            Some(name) if !self.content_name_is_capturing => Some(name.clone()),
            Some(name) => Some(crate::utils::regex_source::replace_captures(
                name, line_text, captures,
            )),
        }
    }
}

/// A single `match` rule — emits a scoped token when the pattern hits.
#[derive(Debug, Clone)]
pub struct MatchRule {
    pub header: RuleHeader,
    pub match_regex: RegExpSource<i32>,
    pub captures: Vec<Option<RuleId>>,
}

/// `begin` / `end` rule — pushes onto the rule stack on `begin`, pops on `end`.
#[derive(Debug, Clone)]
pub struct BeginEndRule {
    pub header: RuleHeader,
    pub begin: RegExpSource<i32>,
    pub begin_captures: Vec<Option<RuleId>>,
    pub end: RegExpSource<i32>,
    pub end_has_back_references: bool,
    pub end_captures: Vec<Option<RuleId>>,
    pub apply_end_pattern_last: bool,
    pub patterns: Vec<RuleId>,
    pub has_missing_patterns: bool,
}

/// `begin` / `while` rule — the while pattern has to re-match on every line.
#[derive(Debug, Clone)]
pub struct BeginWhileRule {
    pub header: RuleHeader,
    pub begin: RegExpSource<i32>,
    pub begin_captures: Vec<Option<RuleId>>,
    pub while_regex: RegExpSource<i32>,
    pub while_captures: Vec<Option<RuleId>>,
    pub while_has_back_references: bool,
    pub patterns: Vec<RuleId>,
    pub has_missing_patterns: bool,
}

/// Include-only rule — a bag of sub-rules with no own `match` / `begin`.
#[derive(Debug, Clone)]
pub struct IncludeOnlyRule {
    pub header: RuleHeader,
    pub patterns: Vec<RuleId>,
    pub has_missing_patterns: bool,
}

/// Capture rule — a nested pattern inside a `captures` map. Never
/// appears as a standalone scanner entry.
#[derive(Debug, Clone)]
pub struct CaptureRule {
    pub header: RuleHeader,
    pub retokenize_captured_with_rule_id: Option<RuleId>,
}

/// Enum over the five variants. Wrapped in the compiled rule table so
/// an `i32` id can dispatch quickly via `match` at tokenization time.
#[derive(Debug, Clone)]
pub enum Rule {
    Match(Box<MatchRule>),
    BeginEnd(Box<BeginEndRule>),
    BeginWhile(Box<BeginWhileRule>),
    IncludeOnly(Box<IncludeOnlyRule>),
    Capture(Box<CaptureRule>),
}

impl Rule {
    pub fn header(&self) -> &RuleHeader {
        match self {
            Self::Match(r) => &r.header,
            Self::BeginEnd(r) => &r.header,
            Self::BeginWhile(r) => &r.header,
            Self::IncludeOnly(r) => &r.header,
            Self::Capture(r) => &r.header,
        }
    }

    /// Upstream's `collectPatterns(grammar, out)` — pushes this rule's
    /// "scanner contribution" onto `out`. `CaptureRule` is the only
    /// variant that can't contribute (upstream throws); here we return
    /// `Err` so the caller decides what to do.
    pub fn collect_patterns(
        &self,
        registry: &RuleRegistry,
        out: &mut RegExpSourceList<i32>,
    ) -> Result<(), &'static str> {
        match self {
            Self::Match(r) => {
                out.push(r.match_regex.clone_inner());
                Ok(())
            }
            Self::BeginEnd(r) => {
                out.push(r.begin.clone_inner());
                Ok(())
            }
            Self::BeginWhile(r) => {
                out.push(r.begin.clone_inner());
                Ok(())
            }
            Self::IncludeOnly(r) => {
                for id in &r.patterns {
                    if let Some(rule) = registry.get(*id) {
                        rule.collect_patterns(registry, out)?;
                    }
                }
                Ok(())
            }
            Self::Capture(_) => Err("CaptureRule cannot collect patterns"),
        }
    }
}

/// Registry of compiled rules keyed by [`RuleId`]. Equivalent to the
/// `IRuleRegistry` interface + the `_ruleId2desc` map upstream keeps
/// inside `Grammar`. Rules are stored behind `Arc` so references can
/// be handed back through the [`crate::tokenizer::GrammarRuntime`]
/// trait without wrestling with lock-guard lifetimes.
#[derive(Debug, Default)]
pub struct RuleRegistry {
    rules: Vec<Option<std::sync::Arc<Rule>>>,
    pub(crate) key_to_id: std::collections::HashMap<String, RuleId>,
    pub(crate) compiling_keys: std::collections::HashSet<String>,
}

impl RuleRegistry {
    pub fn new() -> Self {
        // Rule ids start at 1 upstream (`0` is reserved). Seed with an
        // empty slot so indexing stays unsigned-friendly.
        Self {
            rules: vec![None],
            key_to_id: std::collections::HashMap::new(),
            compiling_keys: std::collections::HashSet::new(),
        }
    }

    /// Allocates a new id without storing a rule yet — mirrors upstream's
    /// `registerRule` factory pattern where the id is known before the
    /// rule body is built.
    pub fn reserve(&mut self) -> RuleId {
        let id = u32::try_from(self.rules.len()).unwrap_or(u32::MAX);
        self.rules.push(None);
        RuleId(id)
    }

    pub fn set(&mut self, id: RuleId, rule: Rule) {
        let slot = self.rules.get_mut(id.0 as usize).expect("reserved id");
        *slot = Some(std::sync::Arc::new(rule));
    }

    pub fn get(&self, id: RuleId) -> Option<&Rule> {
        self.rules
            .get(id.0 as usize)
            .and_then(|slot| slot.as_deref())
    }

    /// Returns a cloned `Arc<Rule>` for callers that need to hold the
    /// rule across method boundaries.
    pub fn get_arc(&self, id: RuleId) -> Option<std::sync::Arc<Rule>> {
        self.rules
            .get(id.0 as usize)
            .and_then(|slot| slot.as_ref().map(std::sync::Arc::clone))
    }

    pub fn len(&self) -> usize {
        self.rules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.rules.len() <= 1
    }
}
