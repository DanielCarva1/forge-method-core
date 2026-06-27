use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The canonical phases of the Forge Method funnel of autonomy, including the
/// transient routing state.
///
/// Variant names mirror the on-disk tags used by the authoritative workflow
/// catalog (`4-build-verify`, `5-ready-operate`), so no translation layer is
/// needed between documents and the enum.
///
/// Ordering is meaningful and is the backbone of the funnel: `Discovery`
/// carries the heaviest human contact and structure; `BuildVerify` is
/// near-silent autonomous execution; `Evolve` closes the loop. The engine
/// derives funnel-of-autonomy *density* from a phase's rank (DC4).
///
/// `Route` (rank 0) is the transient initialization state before the project
/// enters the funnel. The string tag `"anytime"` is NOT a project phase — it is
/// a workflow-eligibility wildcard (a workflow tagged `"anytime"` may run in any
/// phase); it is therefore deliberately absent from this enum and handled in
/// the eligibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub enum Phase {
    #[serde(rename = "0-route")]
    Route,
    #[serde(rename = "1-discovery")]
    Discovery,
    #[serde(rename = "2-specification")]
    Specification,
    #[serde(rename = "3-plan")]
    Plan,
    #[serde(rename = "4-build-verify")]
    BuildVerify,
    #[serde(rename = "5-ready-operate")]
    ReadyOperate,
    #[serde(rename = "6-evolve")]
    Evolve,
}

/// The eligibility wildcard. A workflow whose `phases` set contains this string
/// is eligible in every project phase.
pub const ANYTIME: &str = "anytime";

impl Phase {
    /// All seven canonical phases in funnel order (Route first).
    pub const ALL: [Phase; 7] = [
        Phase::Route,
        Phase::Discovery,
        Phase::Specification,
        Phase::Plan,
        Phase::BuildVerify,
        Phase::ReadyOperate,
        Phase::Evolve,
    ];

    /// Ordinal (0..=6) used for funnel-density comparisons.
    #[must_use]
    pub fn rank(self) -> u8 {
        match self {
            Phase::Route => 0,
            Phase::Discovery => 1,
            Phase::Specification => 2,
            Phase::Plan => 3,
            Phase::BuildVerify => 4,
            Phase::ReadyOperate => 5,
            Phase::Evolve => 6,
        }
    }

    /// Permissive parser used by the engine to categorize the free-form phase
    /// strings carried on `Workflow` / `Operation` documents.
    ///
    /// Accepts the canonical forms (`"3-plan"`, `"4-build-verify"`), bare names
    /// (`"plan"`, `"build-verify"`, `"build"`, `"spec"`), and bare ordinals
    /// (`"3"`, `"4"`). Returns `None` for `"anytime"`, empty, or unrecognized
    /// labels — `"anytime"` is intentionally `None` because it is an
    /// eligibility wildcard, not a phase (see [`ANYTIME`]).
    #[must_use]
    pub fn parse(raw: &str) -> Option<Phase> {
        let trimmed = raw.trim().to_ascii_lowercase();
        if trimmed.is_empty() || trimmed == ANYTIME {
            return None;
        }
        // Bare ordinal.
        if let Ok(n) = trimmed.parse::<u8>() {
            return match n {
                0 => Some(Phase::Route),
                1 => Some(Phase::Discovery),
                2 => Some(Phase::Specification),
                3 => Some(Phase::Plan),
                4 => Some(Phase::BuildVerify),
                5 => Some(Phase::ReadyOperate),
                6 => Some(Phase::Evolve),
                _ => None,
            };
        }
        // Canonical "N-name" or bare name: strip a leading "<digits>-" prefix.
        let name = trimmed
            .split_once('-')
            .filter(|(num, _)| num.chars().all(|c| c.is_ascii_digit()))
            .map_or(trimmed.as_str(), |(_, rest)| rest);
        match name {
            "route" => Some(Phase::Route),
            "discovery" => Some(Phase::Discovery),
            "specification" | "spec" => Some(Phase::Specification),
            "plan" => Some(Phase::Plan),
            "build-verify" | "buildverify" | "build" => Some(Phase::BuildVerify),
            "ready-operate" | "readyoperate" | "ready" => Some(Phase::ReadyOperate),
            "evolve" => Some(Phase::Evolve),
            _ => None,
        }
    }

    /// True if the given phase tag makes a workflow eligible to run in the
    /// provided project phase. Handles the `"anytime"` wildcard.
    #[must_use]
    pub fn tag_eligible(tag: &str, current: Phase) -> bool {
        if tag.trim().eq_ignore_ascii_case(ANYTIME) {
            return true;
        }
        Phase::parse(tag) == Some(current)
    }
}

impl std::fmt::Display for Phase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Phase::Route => "0-route",
            Phase::Discovery => "1-discovery",
            Phase::Specification => "2-specification",
            Phase::Plan => "3-plan",
            Phase::BuildVerify => "4-build-verify",
            Phase::ReadyOperate => "5-ready-operate",
            Phase::Evolve => "6-evolve",
        };
        f.write_str(s)
    }
}

#[cfg(test)]
mod tests {
    use super::Phase;

    #[test]
    fn round_trips_canonical_forms() {
        for phase in Phase::ALL {
            let yaml = serde_yaml::to_string(&phase).expect("serialize phase");
            let back: Phase = serde_yaml::from_str(&yaml).expect("deserialize phase");
            assert_eq!(phase, back, "round-trip failed for {phase}");
        }
    }

    #[test]
    fn parse_accepts_canonical_compound_bare_and_ordinal() {
        // canonical compound names (match authoritative catalog)
        assert_eq!(Phase::parse("4-build-verify"), Some(Phase::BuildVerify));
        assert_eq!(Phase::parse("5-ready-operate"), Some(Phase::ReadyOperate));
        assert_eq!(Phase::parse("3-plan"), Some(Phase::Plan));
        assert_eq!(Phase::parse("0-route"), Some(Phase::Route));
        // bare names
        assert_eq!(Phase::parse("build-verify"), Some(Phase::BuildVerify));
        assert_eq!(Phase::parse("build"), Some(Phase::BuildVerify));
        assert_eq!(Phase::parse("spec"), Some(Phase::Specification));
        // bare ordinals
        assert_eq!(Phase::parse("4"), Some(Phase::BuildVerify));
        assert_eq!(Phase::parse("0"), Some(Phase::Route));
        // anytime is NOT a phase
        assert_eq!(Phase::parse("anytime"), None);
        assert_eq!(Phase::parse(""), None);
        assert_eq!(Phase::parse("nope"), None);
    }

    #[test]
    fn rank_is_monotonic_in_funnel_order() {
        let ranks: Vec<u8> = Phase::ALL.iter().map(|p| p.rank()).collect();
        assert_eq!(ranks, vec![0, 1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn anytime_tag_is_eligible_in_every_phase() {
        for phase in Phase::ALL {
            assert!(
                Phase::tag_eligible("anytime", phase),
                "anytime should be eligible in {phase}"
            );
        }
    }

    #[test]
    fn tag_eligible_matches_only_its_own_phase() {
        assert!(Phase::tag_eligible("3-plan", Phase::Plan));
        assert!(!Phase::tag_eligible("3-plan", Phase::Discovery));
        assert!(Phase::tag_eligible("4-build-verify", Phase::BuildVerify));
    }
}
