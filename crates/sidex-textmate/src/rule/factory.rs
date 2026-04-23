//! Rule factory — pattern → compiled [`super::Rule`] pipeline.
//!
//! Port of `RuleFactory` from upstream. Walks a [`super::raw::RawRule`]
//! tree and emits compiled rules into a [`super::RuleRegistry`].
//!
//! Include references are resolved eagerly: every `include` lookup that
//! succeeds is compiled into a [`super::RuleId`] and stored on the
//! parent. References that can't be resolved (missing external grammar,
//! missing local name) are counted in `has_missing_patterns` so the
//! caller can skip empty branches — identical to upstream behavior.

use super::include::{parse_include, IncludeReference};
use super::raw::{RawCaptures, RawRepository, RawRule};
use super::regex_source::RegExpSource;
use super::rules::{
    BeginEndRule, BeginWhileRule, CaptureRule, IncludeOnlyRule, MatchRule, Rule, RuleHeader,
    RuleRegistry,
};
use super::RuleId;

/// Grammar lookup trait — the factory calls into this to resolve
/// cross-grammar references (`source.other#repo-key`). Mirrors the
/// `IGrammarRegistry` upstream interface.
pub trait GrammarRegistry {
    /// Returns the top-level repository for an external grammar scope,
    /// or `None` when the grammar hasn't been registered yet.
    fn external_grammar_repository<'a>(
        &'a self,
        scope_name: &str,
        current_repository: &'a RawRepository,
    ) -> Option<&'a RawRepository>;
}

/// Trivial no-op registry for tests and standalone grammar compilation.
#[derive(Debug, Default, Clone, Copy)]
pub struct EmptyGrammarRegistry;

impl GrammarRegistry for EmptyGrammarRegistry {
    fn external_grammar_repository<'a>(
        &'a self,
        _scope_name: &str,
        _current_repository: &'a RawRepository,
    ) -> Option<&'a RawRepository> {
        None
    }
}

/// Result of compiling a `patterns` array. Matches upstream's
/// `ICompilePatternsResult`.
#[derive(Debug, Default, Clone)]
pub struct CompiledPatterns {
    pub patterns: Vec<RuleId>,
    pub has_missing_patterns: bool,
}

/// Walks [`RawRule`]s into compiled [`Rule`]s.
pub struct RuleFactory;

impl RuleFactory {
    /// Registers a [`CaptureRule`] with the rule registry. Port of
    /// `createCaptureRule`.
    pub fn create_capture_rule(
        registry: &mut RuleRegistry,
        location: Option<super::raw::Location>,
        name: Option<String>,
        content_name: Option<String>,
        retokenize_captured_with_rule_id: Option<RuleId>,
    ) -> RuleId {
        let id = registry.reserve();
        let rule = Rule::Capture(Box::new(CaptureRule {
            header: RuleHeader::new(location, id, name, content_name),
            retokenize_captured_with_rule_id,
        }));
        registry.set(id, rule);
        id
    }

    /// Compiles a raw rule (if it hasn't been compiled yet) and returns
    /// its id. Port of `getCompiledRuleId` — the cornerstone of the
    /// factory.
    #[allow(clippy::too_many_lines)]
    pub fn get_compiled_rule_id<G: GrammarRegistry>(
        desc: &mut RawRule,
        registry: &mut RuleRegistry,
        grammar_registry: &G,
        repository: &RawRepository,
    ) -> RuleId {
        if let Some(id) = desc.id {
            return RuleId(id);
        }

        let id = registry.reserve();
        desc.id = Some(id.0);

        Self::compile_body(desc, id, registry, grammar_registry, repository);
        id
    }

    #[allow(clippy::too_many_lines)]
    fn compile_body<G: GrammarRegistry>(
        desc: &RawRule,
        id: RuleId,
        registry: &mut RuleRegistry,
        grammar_registry: &G,
        repository: &RawRepository,
    ) {
        let rule = if let Some(match_pattern) = desc.match_.clone() {
            let captures = Self::compile_captures(
                desc.captures.as_ref(),
                registry,
                grammar_registry,
                repository,
            );
            Rule::Match(Box::new(MatchRule {
                header: RuleHeader::new(desc.location.clone(), id, desc.name.clone(), None),
                match_regex: RegExpSource::new(&match_pattern, id.to_signed()),
                captures,
            }))
        } else if desc.begin.is_none() {
            // No `begin` → this is an include-only / container rule.
            let mut merged_repo = repository.clone();
            if let Some(local) = &desc.repository {
                merged_repo.extend(local.clone());
            }

            let patterns_override: Option<Vec<RawRule>> =
                if desc.patterns.is_none() && desc.include.is_some() {
                    Some(vec![RawRule {
                        include: desc.include.clone(),
                        ..RawRule::default()
                    }])
                } else {
                    None
                };

            let patterns_ref: &[RawRule] = patterns_override
                .as_deref()
                .or(desc.patterns.as_deref())
                .unwrap_or(&[]);
            let mut patterns_owned = patterns_ref.to_vec();
            let compiled = Self::compile_patterns(
                &mut patterns_owned,
                registry,
                grammar_registry,
                &merged_repo,
            );
            Rule::IncludeOnly(Box::new(IncludeOnlyRule {
                header: RuleHeader::new(
                    desc.location.clone(),
                    id,
                    desc.name.clone(),
                    desc.content_name.clone(),
                ),
                patterns: compiled.patterns,
                has_missing_patterns: compiled.has_missing_patterns,
            }))
        } else if desc.while_.is_some() {
            let begin = desc.begin.clone().unwrap_or_default();
            let while_pattern = desc.while_.clone().unwrap_or_default();
            let begin_captures_src = desc.begin_captures.as_ref().or(desc.captures.as_ref());
            let while_captures_src = desc.while_captures.as_ref().or(desc.captures.as_ref());
            let begin_captures =
                Self::compile_captures(begin_captures_src, registry, grammar_registry, repository);
            let while_captures =
                Self::compile_captures(while_captures_src, registry, grammar_registry, repository);
            let mut patterns_owned = desc.patterns.clone().unwrap_or_default();
            let compiled =
                Self::compile_patterns(&mut patterns_owned, registry, grammar_registry, repository);
            let while_regex = RegExpSource::new(&while_pattern, super::WHILE_RULE_ID);
            let while_has_back = while_regex.has_back_references();
            Rule::BeginWhile(Box::new(BeginWhileRule {
                header: RuleHeader::new(
                    desc.location.clone(),
                    id,
                    desc.name.clone(),
                    desc.content_name.clone(),
                ),
                begin: RegExpSource::new(&begin, id.to_signed()),
                begin_captures,
                while_regex,
                while_captures,
                while_has_back_references: while_has_back,
                patterns: compiled.patterns,
                has_missing_patterns: compiled.has_missing_patterns,
            }))
        } else {
            // begin/end rule.
            let begin = desc.begin.clone().unwrap_or_default();
            let end = desc.end.clone().unwrap_or_default();
            let begin_captures_src = desc.begin_captures.as_ref().or(desc.captures.as_ref());
            let end_captures_src = desc.end_captures.as_ref().or(desc.captures.as_ref());
            let begin_captures =
                Self::compile_captures(begin_captures_src, registry, grammar_registry, repository);
            let end_captures =
                Self::compile_captures(end_captures_src, registry, grammar_registry, repository);
            let mut patterns_owned = desc.patterns.clone().unwrap_or_default();
            let compiled =
                Self::compile_patterns(&mut patterns_owned, registry, grammar_registry, repository);
            let end_regex = RegExpSource::new(&end, super::END_RULE_ID);
            let end_has_back = end_regex.has_back_references();
            Rule::BeginEnd(Box::new(BeginEndRule {
                header: RuleHeader::new(
                    desc.location.clone(),
                    id,
                    desc.name.clone(),
                    desc.content_name.clone(),
                ),
                begin: RegExpSource::new(&begin, id.to_signed()),
                begin_captures,
                end: end_regex,
                end_has_back_references: end_has_back,
                end_captures,
                apply_end_pattern_last: desc.apply_end_pattern_last.unwrap_or(false),
                patterns: compiled.patterns,
                has_missing_patterns: compiled.has_missing_patterns,
            }))
        };

        registry.set(id, rule);
    }

    fn compile_captures<G: GrammarRegistry>(
        captures: Option<&RawCaptures>,
        registry: &mut RuleRegistry,
        grammar_registry: &G,
        repository: &RawRepository,
    ) -> Vec<Option<RuleId>> {
        let Some(captures) = captures else {
            return Vec::new();
        };

        let mut max_id: usize = 0;
        for key in captures.keys() {
            if key == "$vscodeTextmateLocation" {
                continue;
            }
            if let Ok(n) = key.parse::<usize>() {
                if n > max_id {
                    max_id = n;
                }
            }
        }

        let mut result: Vec<Option<RuleId>> = vec![None; max_id + 1];

        for (key, value) in captures {
            if key == "$vscodeTextmateLocation" {
                continue;
            }
            let Ok(n) = key.parse::<usize>() else {
                continue;
            };
            let mut value = value.clone();
            let retokenize = if value.patterns.is_some() {
                Some(Self::get_compiled_rule_id(
                    &mut value,
                    registry,
                    grammar_registry,
                    repository,
                ))
            } else {
                None
            };
            let capture_id = Self::create_capture_rule(
                registry,
                value.location.clone(),
                value.name.clone(),
                value.content_name.clone(),
                retokenize,
            );
            if n < result.len() {
                result[n] = Some(capture_id);
            }
        }

        result
    }

    fn compile_patterns<G: GrammarRegistry>(
        patterns: &mut [RawRule],
        registry: &mut RuleRegistry,
        grammar_registry: &G,
        repository: &RawRepository,
    ) -> CompiledPatterns {
        let mut out: Vec<RuleId> = Vec::new();

        for pattern in patterns.iter_mut() {
            let rule_id: Option<RuleId> = if let Some(include) = pattern.include.clone() {
                match parse_include(&include) {
                    IncludeReference::Base | IncludeReference::SelfRef => {
                        // `$base` / `$self` read the same key out of the
                        // current repository — upstream does the same.
                        Self::compile_from_repo(&include, repository, registry, grammar_registry)
                    }
                    IncludeReference::Relative { rule_name } => {
                        Self::compile_from_repo(&rule_name, repository, registry, grammar_registry)
                    }
                    IncludeReference::TopLevel { scope_name }
                    | IncludeReference::TopLevelRepository { scope_name, .. } => {
                        let external_include = match parse_include(&include) {
                            IncludeReference::TopLevelRepository { rule_name, .. } => {
                                Some(rule_name)
                            }
                            _ => None,
                        };
                        grammar_registry
                            .external_grammar_repository(&scope_name, repository)
                            .and_then(|external_repo| {
                                let key =
                                    external_include.as_deref().unwrap_or("$self").to_string();
                                Self::compile_from_repo(
                                    &key,
                                    external_repo,
                                    registry,
                                    grammar_registry,
                                )
                            })
                    }
                }
            } else {
                Some(Self::get_compiled_rule_id(
                    pattern,
                    registry,
                    grammar_registry,
                    repository,
                ))
            };

            if let Some(id) = rule_id {
                // Skip rules that compiled into an empty include-only /
                // begin-end / begin-while body when upstream also skips.
                let skip = matches!(
                    registry.get(id),
                    Some(Rule::IncludeOnly(r)) if r.has_missing_patterns && r.patterns.is_empty()
                ) || matches!(
                    registry.get(id),
                    Some(Rule::BeginEnd(r)) if r.has_missing_patterns && r.patterns.is_empty()
                ) || matches!(
                    registry.get(id),
                    Some(Rule::BeginWhile(r)) if r.has_missing_patterns && r.patterns.is_empty()
                );
                if !skip {
                    out.push(id);
                }
            }
        }

        let expected = patterns.len();
        CompiledPatterns {
            has_missing_patterns: expected != out.len(),
            patterns: out,
        }
    }

    fn compile_from_repo<G: GrammarRegistry>(
        key: &str,
        repository: &RawRepository,
        registry: &mut RuleRegistry,
        grammar_registry: &G,
    ) -> Option<RuleId> {
        if registry.compiling_keys.contains(key) {
            return registry.key_to_id.get(key).copied();
        }

        let raw = repository.get(key)?;

        if let Some(existing_id) = raw.id {
            return Some(RuleId(existing_id));
        }

        let raw = raw.clone();

        let id = registry.reserve();
        registry.key_to_id.insert(key.to_string(), id);
        registry.compiling_keys.insert(key.to_string());

        Self::compile_body(&raw, id, registry, grammar_registry, repository);

        registry.compiling_keys.remove(key);
        Some(id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn compile_minimal_match_rule() {
        let mut registry = RuleRegistry::new();
        let mut desc = RawRule {
            match_: Some(r"\bfoo\b".to_string()),
            name: Some("keyword.test".to_string()),
            ..RawRule::default()
        };
        let id = RuleFactory::get_compiled_rule_id(
            &mut desc,
            &mut registry,
            &EmptyGrammarRegistry,
            &RawRepository::new(),
        );
        let rule = registry.get(id).unwrap();
        assert!(matches!(rule, Rule::Match(_)));
    }

    #[test]
    fn compile_include_only_falls_back_to_include_field() {
        let mut registry = RuleRegistry::new();
        let mut desc = RawRule {
            include: Some("#entity".to_string()),
            ..RawRule::default()
        };
        let mut repo = RawRepository::new();
        repo.insert(
            "entity".to_string(),
            RawRule {
                match_: Some(r"\b[A-Z][A-Za-z0-9]*\b".to_string()),
                ..RawRule::default()
            },
        );
        let id = RuleFactory::get_compiled_rule_id(
            &mut desc,
            &mut registry,
            &EmptyGrammarRegistry,
            &repo,
        );
        let rule = registry.get(id).unwrap();
        assert!(matches!(rule, Rule::IncludeOnly(_)));
    }

    #[test]
    fn compile_begin_end_rule_has_end_pattern() {
        let mut registry = RuleRegistry::new();
        let mut desc = RawRule {
            begin: Some(r#"""#.to_string()),
            end: Some(r#"""#.to_string()),
            name: Some("string.quoted".to_string()),
            ..RawRule::default()
        };
        let id = RuleFactory::get_compiled_rule_id(
            &mut desc,
            &mut registry,
            &EmptyGrammarRegistry,
            &RawRepository::new(),
        );
        match registry.get(id).unwrap() {
            Rule::BeginEnd(rule) => {
                assert_eq!(rule.begin.source(), r#"""#);
                assert_eq!(rule.end.source(), r#"""#);
            }
            _ => panic!("expected BeginEnd"),
        }
    }

    #[test]
    fn circular_include_does_not_overflow() {
        let mut registry = RuleRegistry::new();

        let mut repo = RawRepository::new();
        repo.insert(
            "a".to_string(),
            RawRule {
                patterns: Some(vec![RawRule {
                    include: Some("#b".to_string()),
                    ..RawRule::default()
                }]),
                ..RawRule::default()
            },
        );
        repo.insert(
            "b".to_string(),
            RawRule {
                patterns: Some(vec![RawRule {
                    include: Some("#a".to_string()),
                    ..RawRule::default()
                }]),
                ..RawRule::default()
            },
        );

        let mut top = RawRule {
            patterns: Some(vec![RawRule {
                include: Some("#a".to_string()),
                ..RawRule::default()
            }]),
            ..RawRule::default()
        };

        let id = RuleFactory::get_compiled_rule_id(
            &mut top,
            &mut registry,
            &EmptyGrammarRegistry,
            &repo,
        );
        assert!(registry.get(id).is_some());
    }
}
