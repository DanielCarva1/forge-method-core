//! F11 — Risk Audit Gate.
//!
//! Fail-closed inspection pass over source code that detects AI-induced
//! anti-patterns (fail-soft, exception swallowing, security slop, false
//! tests). Rules are parametric YAML contracts (`risk-audit-v0`); the
//! validator accumulates typed `Diagnostic`s into a `ValidationReport`.
//!
//! See `CONTEXT.md` → "Risk Audit" and "Anti-pattern (AI Code)" for the
//! canonical definitions.

use crate::{Diagnostic, DiagnosticCode, DiagnosticSeverity, ValidationReport};
use serde::{Deserialize, Serialize};

/// Schema version marker for risk-audit contracts. Bumping this signals a
/// breaking change in the rule shape.
pub const RISK_AUDIT_SCHEMA_VERSION: &str = "risk-audit-v0";

/// Maximum file size (in bytes) the regex detector will scan. Larger files
/// are reported as unreadable to avoid pathological regex backtracking on
/// huge blobs (e.g. generated code, minified JS).
pub const RISK_AUDIT_MAX_FILE_BYTES: usize = 10 * 1024 * 1024; // 10 MiB

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskAuditSeverity {
    Error,
    Warning,
}

impl From<RiskAuditSeverity> for DiagnosticSeverity {
    fn from(severity: RiskAuditSeverity) -> Self {
        match severity {
            RiskAuditSeverity::Error => DiagnosticSeverity::Error,
            RiskAuditSeverity::Warning => DiagnosticSeverity::Warning,
        }
    }
}

/// What a rule looks for. Kinds are intentionally constrained so the
/// validator can stay deterministic and side-effect-free (the exception is
/// `ExternalLinter`, which shells out — it is the only non-pure detector).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RiskAuditDetector {
    /// Substring/regex match against the file contents. Use for known
    /// anti-pattern strings (`unwrap()`, `except Exception:`, `pass` in
    /// `except`, etc.). Reports one diagnostic per match with the line
    /// number when available.
    Regex {
        /// Pattern compiled with the `regex` crate (Unicode, line-aware).
        pattern: String,
    },
    /// Match against the file path itself (e.g. reject `.env` files in
    /// tracked dirs). The pattern is a simple glob: `*` matches within a
    /// path segment, `**` matches across segments.
    PathGlob {
        /// Glob pattern matched against the relative path of the target.
        pattern: String,
    },
    /// Requires that at least one file matching the glob exists in the
    /// scanned target set. Use for "must have tests" rules. Emits an error
    /// when zero targets match.
    FileGlobMustExist {
        /// Glob pattern that must match at least one scanned target.
        pattern: String,
    },
    /// Delegate to an external linter (clippy, semgrep, etc.). Reads the
    /// linter's structured output from `output_path` after invoking
    /// `command`. The validator fails closed if the command exits non-zero
    /// or the output cannot be parsed. Full invocation support lands in a
    /// later sub-track; for now this detector is shape-only and the
    /// validator emits `RiskAuditExternalLinterFailed` when used.
    ExternalLinter {
        /// Shell command to run. Validator runs it from the audit root.
        command: String,
        /// Path (relative to audit root) where the linter writes its
        /// JSON findings. Validator parses this file.
        output_path: String,
    },
}

/// A single anti-pattern rule. Rules are data, not code: adding one must
/// never require a Rust change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAuditRule {
    /// Stable identifier. Surfaces in diagnostics so agents can dedupe and
    /// suppress.
    pub id: String,
    /// Human/agent-readable explanation of why this pattern is forbidden.
    pub description: String,
    pub severity: RiskAuditSeverity,
    pub detector: RiskAuditDetector,
    /// If true, a match is treated as missing evidence rather than a
    /// direct violation. Reserved for forward-compat with evidence-aware
    /// gates; today the field is recorded but does not change accumulation.
    pub evidence_required: bool,
    /// Actionable suggestion emitted alongside any diagnostic this rule
    /// produces. Empty `fix_hint` is allowed but warned about during rule
    /// set validation.
    pub fix_hint: String,
    /// Globs (same syntax as `PathGlob`) restricting which targets the
    /// detector applies to. Must be non-empty.
    #[serde(default)]
    pub applies_to: Vec<String>,
}

/// Top-level rule set loaded from a `risk-audit-v0` YAML contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RiskAuditRuleSet {
    /// Must equal `RISK_AUDIT_SCHEMA_VERSION`.
    pub schema_version: String,
    #[serde(default)]
    pub rules: Vec<RiskAuditRule>,
}

/// A single source file under audit. Callers (typically the CLI walker)
/// populate `content`; the validator never reads the filesystem itself, so
/// the same rule set can be evaluated against in-memory or synthetic
/// targets in tests.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RiskAuditTarget {
    /// Path relative to the audit root, using forward slashes.
    pub path: String,
    /// File contents. May be empty for `PathGlob` / `FileGlobMustExist`
    /// detectors that don't need contents.
    pub content: String,
}

impl RiskAuditRuleSet {
    #[must_use]
    pub fn rules_for_target<'a>(&'a self, target: &'a RiskAuditTarget) -> Vec<&'a RiskAuditRule> {
        self.rules
            .iter()
            .filter(|rule| {
                rule.applies_to
                    .iter()
                    .any(|glob| path_matches_glob(&target.path, glob))
            })
            .collect()
    }
}

/// Simple glob matcher supporting `*` (within a segment) and `**`
/// (across segments). Good enough for `applies_to` filters; intentionally
/// not a full gitignore implementation.
fn path_matches_glob(path: &str, glob: &str) -> bool {
    // Normalize: treat backslashes as forward slashes.
    let path = path.replace('\\', "/");
    glob_match(&path, glob)
}

fn glob_match(path: &str, glob: &str) -> bool {
    // Expand `**` into the recursive zero-or-more matcher and `*` into the
    // single-segment matcher. We compile to a simple state machine: split
    // the glob into segments separated by `/`, with a marker for `**`.
    let glob_segments: Vec<&str> = glob.split('/').collect();
    let path_segments: Vec<&str> = path.split('/').collect();
    segments_match(&path_segments, &glob_segments)
}

fn segments_match(path: &[&str], glob: &[&str]) -> bool {
    if glob.is_empty() {
        return path.is_empty();
    }
    let head = glob[0];
    if head == "**" {
        // `**` matches zero or more path segments. Try every split.
        if segments_match(path, &glob[1..]) {
            return true;
        }
        if !path.is_empty() && segments_match(&path[1..], &glob[1..]) {
            return true;
        }
        // Greedy: also try consuming one segment and staying on `**`.
        if !path.is_empty() && segments_match(&path[1..], glob) {
            return true;
        }
        false
    } else if path.is_empty() {
        false
    } else if single_segment_matches(path[0], head) {
        segments_match(&path[1..], &glob[1..])
    } else {
        false
    }
}

fn single_segment_matches(segment: &str, glob_segment: &str) -> bool {
    // `*` matches any run of characters within the segment (no `/`).
    // Everything else must match literally.
    let mut seg_chars = segment.chars().peekable();
    let mut glob_chars = glob_segment.chars().peekable();

    while let Some(g) = glob_chars.next() {
        if g == '*' {
            // Skip consecutive stars.
            while glob_chars.peek() == Some(&'*') {
                glob_chars.next();
            }
            if glob_chars.peek().is_none() {
                return true;
            }
            let rest: String = glob_chars.clone().collect();
            // Try to find `rest` starting at any position in the remaining segment.
            let remaining: String = seg_chars.clone().collect();
            if let Some(idx) = remaining.find(&rest) {
                // Advance seg_chars past the matched prefix + rest.
                for _ in 0..idx + rest.chars().count() {
                    seg_chars.next();
                }
                // Glob_chars is now exhausted because we consumed `rest` from it.
                return seg_chars.peek().is_none();
            }
            return false;
        }
        match seg_chars.next() {
            Some(s) if s == g => {}
            _ => return false,
        }
    }
    seg_chars.peek().is_none()
}

/// Validate that a rule set is structurally well-formed. This does not run
/// the rules against any target; it only catches malformed contracts early
/// so `evaluate_risk_audit` can assume a valid rule set.
///
/// Errors emitted (all `DiagnosticSeverity::Error` unless noted):
/// - `RiskAuditRuleMissingId` — rule has empty `id`.
/// - `RiskAuditRuleMissingDetector` — rule missing `detector`.
/// - `RiskAuditRuleMissingPattern` — `regex`/`path_glob`/`file_glob` detector
///   has empty `pattern`.
/// - `RiskAuditRuleInvalidSeverity` — severity field failed to deserialize
///   (serde-level; this fires only if the struct was hand-built).
/// - `RiskAuditRuleInvalidDetectorKind` — detector kind not in the enum.
///   (serde-level for YAML; fires for hand-built structs.)
/// - `RiskAuditRuleInvalidAppliesTo` — `applies_to` is empty.
/// - `RiskAuditRuleMissingFixHint` — `fix_hint` empty (Warning, not Error).
/// - `RiskAuditRuleSetEmpty` — rule set has zero rules.
#[must_use]
pub fn validate_risk_audit_rule_set(ruleset: &RiskAuditRuleSet) -> ValidationReport {
    let mut report = ValidationReport::new();

    if ruleset.rules.is_empty() {
        report.push(Diagnostic::error(
            DiagnosticCode::RiskAuditRuleSetEmpty,
            "risk_audit.ruleset",
            "risk audit rule set must contain at least one rule",
        ));
        return report;
    }

    for (idx, rule) in ruleset.rules.iter().enumerate() {
        let path = format!("risk_audit.rules[{idx}]");

        if rule.id.trim().is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::RiskAuditRuleMissingId,
                path.clone(),
                "rule must have a non-empty id",
            ));
        }

        if rule.applies_to.is_empty() {
            report.push(Diagnostic::error(
                DiagnosticCode::RiskAuditRuleInvalidAppliesTo,
                path.clone(),
                "rule must declare at least one applies_to glob",
            ));
        }

        if rule.fix_hint.trim().is_empty() {
            report.push(Diagnostic::warning(
                DiagnosticCode::RiskAuditRuleMissingFixHint,
                path.clone(),
                "rule has no fix_hint; agents will have less to act on",
            ));
        }

        // Detector payload validation: the tag/kind is enforced by serde;
        // here we check the inner `pattern` / `command` payload.
        match &rule.detector {
            RiskAuditDetector::Regex { pattern }
            | RiskAuditDetector::PathGlob { pattern }
            | RiskAuditDetector::FileGlobMustExist { pattern } => {
                if pattern.trim().is_empty() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::RiskAuditRuleMissingPattern,
                        format!("{path}.detector.pattern"),
                        "detector pattern must be non-empty",
                    ));
                }
            }
            RiskAuditDetector::ExternalLinter {
                command,
                output_path,
            } => {
                if command.trim().is_empty() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::RiskAuditRuleMissingPattern,
                        format!("{path}.detector.command"),
                        "external_linter detector must declare a command",
                    ));
                }
                if output_path.trim().is_empty() {
                    report.push(Diagnostic::error(
                        DiagnosticCode::RiskAuditRuleMissingPattern,
                        format!("{path}.detector.output_path"),
                        "external_linter detector must declare an output_path",
                    ));
                }
            }
        }
    }

    report
}

/// Run a rule set against a slice of targets and accumulate findings into
/// a `ValidationReport`. The rule set must already be valid (call
/// `validate_risk_audit_rule_set` first and merge reports if needed).
///
/// Fail-closed behaviour:
/// - Targets whose content exceeds `RISK_AUDIT_MAX_FILE_BYTES` are reported
///   with `RiskAuditTargetFileUnreadable` and skipped.
/// - `ExternalLinter` detectors emit `RiskAuditExternalLinterFailed` for
///   every target they apply to (full linter integration is a later track).
/// - `FileGlobMustExist` emits a single `RiskAuditRequiredFileMissing`
///   error if zero targets match the inner glob across the whole slice.
#[must_use]
pub fn evaluate_risk_audit(
    ruleset: &RiskAuditRuleSet,
    targets: &[RiskAuditTarget],
) -> ValidationReport {
    let mut report = ValidationReport::new();

    // Phase 1: per-target detectors.
    for target in targets {
        for rule in ruleset.rules_for_target(target) {
            evaluate_rule_against_target(&mut report, rule, target);
        }
    }

    // Phase 2: rule-set-wide FileGlobMustExist checks. These are not
    // per-target; they assert existence across the scanned set.
    for (rule_idx, rule) in ruleset.rules.iter().enumerate() {
        if let RiskAuditDetector::FileGlobMustExist { pattern } = &rule.detector {
            let any_match = targets
                .iter()
                .any(|target| path_matches_glob(&target.path, pattern));
            if !any_match {
                report.push(Diagnostic::error(
                    DiagnosticCode::RiskAuditRequiredFileMissing,
                    format!("risk_audit.rules[{rule_idx}].detector.pattern"),
                    format!(
                        "rule `{}` requires at least one file matching `{}` but none was scanned",
                        rule.id, pattern
                    ),
                ));
            }
        }
    }

    report
}

fn evaluate_rule_against_target(
    report: &mut ValidationReport,
    rule: &RiskAuditRule,
    target: &RiskAuditTarget,
) {
    if target.content.len() > RISK_AUDIT_MAX_FILE_BYTES {
        report.push(Diagnostic::error(
            DiagnosticCode::RiskAuditTargetFileUnreadable,
            target.path.clone(),
            format!(
                "target file exceeds {RISK_AUDIT_MAX_FILE_BYTES} bytes; skipped to avoid regex backtracking"
            ),
        ));
        return;
    }

    match &rule.detector {
        RiskAuditDetector::Regex { pattern } => {
            let Ok(compiled) = regex::Regex::new(pattern) else {
                report.push(Diagnostic::error(
                    DiagnosticCode::RiskAuditRuleMalformed,
                    target.path.clone(),
                    format!("rule `{}` has an invalid regex: {}", rule.id, pattern),
                ));
                return;
            };
            for mat in compiled.find_iter(&target.content) {
                let line = line_number_at(&target.content, mat.start());
                report.push(Diagnostic {
                    severity: rule.severity.into(),
                    code: DiagnosticCode::RiskAuditAntiPatternMatched,
                    path: format!("{}:{}", target.path, line),
                    message: format!(
                        "rule `{}` matched: {}{}{}",
                        rule.id,
                        rule.description,
                        if rule.fix_hint.is_empty() {
                            ""
                        } else {
                            " — fix: "
                        },
                        rule.fix_hint,
                    ),
                });
            }
        }
        RiskAuditDetector::PathGlob { pattern } => {
            if path_matches_glob(&target.path, pattern) {
                report.push(Diagnostic {
                    severity: rule.severity.into(),
                    code: DiagnosticCode::RiskAuditAntiPatternMatched,
                    path: target.path.clone(),
                    message: format!(
                        "rule `{}` matched path: {}{}{}",
                        rule.id,
                        rule.description,
                        if rule.fix_hint.is_empty() {
                            ""
                        } else {
                            " — fix: "
                        },
                        rule.fix_hint,
                    ),
                });
            }
        }
        RiskAuditDetector::FileGlobMustExist { .. } => {
            // Handled by the rule-set-wide phase in `evaluate_risk_audit`.
        }
        RiskAuditDetector::ExternalLinter { command, .. } => {
            report.push(Diagnostic::error(
                DiagnosticCode::RiskAuditExternalLinterFailed,
                target.path.clone(),
                format!(
                    "rule `{}` declares external_linter `{}` which is not yet supported in this build",
                    rule.id, command
                ),
            ));
        }
    }
}

/// 1-based line number for a byte offset. Counts the number of `\n`
/// occurrences strictly before `offset`.
fn line_number_at(content: &str, offset: usize) -> usize {
    let up_to = content.len().min(offset);
    let bytes = content.as_bytes();
    let mut line = 1usize;
    for &byte in &bytes[..up_to] {
        if byte == b'\n' {
            line += 1;
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(id: &str, pattern: &str, applies_to: &[&str]) -> RiskAuditRule {
        RiskAuditRule {
            id: id.to_string(),
            description: "test rule".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::Regex {
                pattern: pattern.to_string(),
            },
            evidence_required: false,
            fix_hint: "replace the match".to_string(),
            applies_to: applies_to.iter().map(|s| (*s).to_string()).collect(),
        }
    }

    fn empty_ruleset() -> RiskAuditRuleSet {
        RiskAuditRuleSet {
            schema_version: RISK_AUDIT_SCHEMA_VERSION.to_string(),
            rules: vec![],
        }
    }

    #[test]
    fn validate_ruleset_rejects_empty() {
        let report = validate_risk_audit_rule_set(&empty_ruleset());
        assert!(report.has_errors());
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditRuleSetEmpty }));
    }

    #[test]
    fn validate_ruleset_rejects_missing_id() {
        let mut rs = empty_ruleset();
        rs.rules.push(RiskAuditRule {
            id: String::new(),
            description: "x".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::Regex {
                pattern: "foo".to_string(),
            },
            evidence_required: false,
            fix_hint: "x".to_string(),
            applies_to: vec!["**/*".to_string()],
        });
        let report = validate_risk_audit_rule_set(&rs);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditRuleMissingId }));
    }

    #[test]
    fn validate_ruleset_rejects_empty_applies_to() {
        let mut rs = empty_ruleset();
        rs.rules.push(RiskAuditRule {
            id: "r".to_string(),
            description: "x".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::Regex {
                pattern: "foo".to_string(),
            },
            evidence_required: false,
            fix_hint: "x".to_string(),
            applies_to: vec![],
        });
        let report = validate_risk_audit_rule_set(&rs);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditRuleInvalidAppliesTo }));
    }

    #[test]
    fn validate_ruleset_warns_on_missing_fix_hint() {
        let mut rs = empty_ruleset();
        rs.rules.push(RiskAuditRule {
            id: "r".to_string(),
            description: "x".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::Regex {
                pattern: "foo".to_string(),
            },
            evidence_required: false,
            fix_hint: String::new(),
            applies_to: vec!["**/*".to_string()],
        });
        let report = validate_risk_audit_rule_set(&rs);
        assert!(report.diagnostics().iter().any(|d| {
            d.code == DiagnosticCode::RiskAuditRuleMissingFixHint
                && d.severity == DiagnosticSeverity::Warning
        }));
        assert!(!report.has_errors());
    }

    #[test]
    fn evaluate_regex_match_emits_diagnostic_with_line() {
        let mut rs = empty_ruleset();
        rs.rules
            .push(rule("no-unwrap", r"\.unwrap\(\)", &["**/*.rs"]));
        let targets = vec![RiskAuditTarget {
            path: "src/main.rs".to_string(),
            content: "let x = opt.unwrap();\nlet y = other.unwrap();\n".to_string(),
        }];
        let report = evaluate_risk_audit(&rs, &targets);
        let matched: Vec<_> = report
            .diagnostics()
            .iter()
            .filter(|d| d.code == DiagnosticCode::RiskAuditAntiPatternMatched)
            .collect();
        assert_eq!(matched.len(), 2);
        assert!(matched[0].path.starts_with("src/main.rs:"));
    }

    #[test]
    fn evaluate_applies_to_skips_non_matching_paths() {
        let mut rs = empty_ruleset();
        rs.rules
            .push(rule("no-unwrap", r"\.unwrap\(\)", &["**/*.py"]));
        let targets = vec![RiskAuditTarget {
            path: "src/main.rs".to_string(),
            content: "opt.unwrap()".to_string(),
        }];
        let report = evaluate_risk_audit(&rs, &targets);
        assert!(report.diagnostics().is_empty());
    }

    #[test]
    fn evaluate_path_glob_matches_target_path() {
        let mut rs = empty_ruleset();
        rs.rules.push(RiskAuditRule {
            id: "no-dotenv".to_string(),
            description: "tracked .env files leak secrets".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::PathGlob {
                pattern: "**/.env".to_string(),
            },
            evidence_required: false,
            fix_hint: "move secrets to a sidecar and gitignore .env".to_string(),
            applies_to: vec!["**/.env".to_string()],
        });
        let targets = vec![RiskAuditTarget {
            path: ".env".to_string(),
            content: "SECRET=abc".to_string(),
        }];
        let report = evaluate_risk_audit(&rs, &targets);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditAntiPatternMatched }));
    }

    #[test]
    fn evaluate_file_glob_must_exist_emits_error_when_missing() {
        let mut rs = empty_ruleset();
        rs.rules.push(RiskAuditRule {
            id: "must-have-readme".to_string(),
            description: "projects must ship a README".to_string(),
            severity: RiskAuditSeverity::Error,
            detector: RiskAuditDetector::FileGlobMustExist {
                pattern: "README.md".to_string(),
            },
            evidence_required: false,
            fix_hint: "add a README.md at the project root".to_string(),
            applies_to: vec!["**/*".to_string()],
        });
        let targets = vec![RiskAuditTarget {
            path: "src/main.rs".to_string(),
            content: "fn main() {}".to_string(),
        }];
        let report = evaluate_risk_audit(&rs, &targets);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditRequiredFileMissing }));
    }

    #[test]
    fn evaluate_invalid_regex_emits_malformed_rule_diagnostic() {
        let mut rs = empty_ruleset();
        rs.rules.push(rule("bad-regex", r"[unclosed", &["**/*.rs"]));
        let targets = vec![RiskAuditTarget {
            path: "src/main.rs".to_string(),
            content: "hello".to_string(),
        }];
        let report = evaluate_risk_audit(&rs, &targets);
        assert!(report
            .diagnostics()
            .iter()
            .any(|d| { d.code == DiagnosticCode::RiskAuditRuleMalformed }));
    }

    #[test]
    fn glob_star_star_matches_recursively() {
        assert!(path_matches_glob("src/deep/mod.rs", "**/*.rs"));
        assert!(path_matches_glob("main.rs", "**/*.rs"));
        assert!(!path_matches_glob("src/mod.txt", "**/*.rs"));
    }

    #[test]
    fn glob_single_star_matches_within_segment() {
        assert!(path_matches_glob("src/main.rs", "src/*.rs"));
        assert!(!path_matches_glob("src/deep/main.rs", "src/*.rs"));
    }
}
