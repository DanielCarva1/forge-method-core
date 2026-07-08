//! Project profile detection for project-agnostic gates.
//!
//! `preflight` needs to know what kind of project it is running against so it
//! can select the right gates: `cargo check` makes sense for a Rust workspace
//! but fails (misleadingly) on a Node or QA-only project. This module detects
//! the profile from project-root markers (manifest files) — it never reads
//! file contents, only checks for file existence, mirroring the cheap
//! project-root predicate in `project_cmd.rs`.
//!
//! Detection is intentionally non-authoritative: a detected profile is a
//! default, not a verdict. The agent (or the user via `--profile`) can always
//! override it, and a `.forge-method/preflight.yaml` file (see
//! [`PreflightProfileDocument`]) pins the resolved profile plus its gate set
//! so subsequent runs do not re-detect.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::preflight_cmd::{GateKind, GateRequirement};

/// File name of the preflight profile document, relative to the sidecar
/// `.forge-method/` directory created by `forge-core project init`.
pub const PREFLIGHT_PROFILE_FILE_NAME: &str = "preflight.yaml";

/// Schema-version wire string for [`PreflightProfileDocument`].
pub const PREFLIGHT_PROFILE_SCHEMA_VERSION: &str = "forge_preflight_profile_v1";

/// Coarse project profile inferred from manifest markers in the project root.
///
/// The detection order (see [`ProjectProfile::detect`]) is stable and
/// documented; the first marker that matches wins. `Generic` is the fallback
/// when none of the known markers is present — a generic project still runs
/// the language-agnostic gates (`validate`, `regression_anchor`) and never
/// fails with a misleading "Cargo.toml not found" error.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectProfile {
    /// Rust workspace or single crate. Marker: `Cargo.toml`.
    Rust,
    /// Node / JavaScript / TypeScript. Marker: `package.json`.
    Node,
    /// Python. Markers: `pyproject.toml`, `setup.py`, `requirements.txt`.
    Python,
    /// Go. Marker: `go.mod`.
    Go,
    /// No recognised manifest; run language-agnostic gates only.
    Generic,
}

impl ProjectProfile {
    /// Stable wire name (`rust`, `node`, `python`, `go`, `generic`).
    ///
    /// These names are part of the preflight profile schema and the
    /// `--profile <name>` CLI flag; they are kept stable across releases.
    #[must_use]
    pub fn wire_name(self) -> &'static str {
        match self {
            Self::Rust => "rust",
            Self::Node => "node",
            Self::Python => "python",
            Self::Go => "go",
            Self::Generic => "generic",
        }
    }

    /// Parse a wire name back into a variant. Returns `None` for unknown
    /// names so callers can surface a typed "invalid profile" error.
    #[must_use]
    pub fn from_wire_name(name: &str) -> Option<Self> {
        match name {
            "rust" => Some(Self::Rust),
            "node" => Some(Self::Node),
            "python" => Some(Self::Python),
            "go" => Some(Self::Go),
            "generic" => Some(Self::Generic),
            _ => None,
        }
    }

    /// Detect the profile from the project root by probing for manifest files.
    ///
    /// The probe order is fixed: Rust → Node → Python → Go → Generic. The
    /// first hit wins, so a polyglot repo (e.g. Rust with a `package.json`
    /// for tooling) is classified by its primary manifest. Only file
    /// existence is checked — no contents are read — so this is cheap and
    /// never fails on a malformed manifest.
    #[must_use]
    pub fn detect(root: &Path) -> Self {
        if root.join("Cargo.toml").is_file() {
            Self::Rust
        } else if root.join("package.json").is_file() {
            Self::Node
        } else if root.join("pyproject.toml").is_file()
            || root.join("setup.py").is_file()
            || root.join("requirements.txt").is_file()
        {
            Self::Python
        } else if root.join("go.mod").is_file() {
            Self::Go
        } else {
            Self::Generic
        }
    }

    /// Built-in gate set for this profile.
    ///
    /// Every profile runs the language-agnostic gates (`validate`,
    /// `regression_anchor`). Rust additionally runs the four cargo gates
    /// (`type_check`, `format`, `clippy_pedantic`, `test`). Other language
    /// profiles do not have built-in language-specific gates — the agent can
    /// add custom shell gates (e.g. `npm test`, `pytest`) to the profile
    /// document. This keeps the core language-agnostic while still
    /// short-circuiting the common Rust case.
    #[must_use]
    pub fn default_gates(self) -> Vec<GateSpec> {
        match self {
            Self::Rust => vec![
                GateSpec::builtin(GateKind::TypeCheck, GateRequirement::Required),
                GateSpec::builtin(GateKind::Format, GateRequirement::Required),
                GateSpec::builtin(GateKind::ClippyPedantic, GateRequirement::Required),
                GateSpec::builtin(GateKind::Test, GateRequirement::Required),
                GateSpec::builtin(GateKind::Validate, GateRequirement::Required),
                GateSpec::builtin(GateKind::RegressionAnchor, GateRequirement::Required),
            ],
            Self::Node | Self::Python | Self::Go | Self::Generic => vec![
                GateSpec::builtin(GateKind::Validate, GateRequirement::Required),
                GateSpec::builtin(GateKind::RegressionAnchor, GateRequirement::Required),
            ],
        }
    }
}

/// A single gate declaration in a [`PreflightProfileDocument`].
///
/// A gate is either:
/// - **built-in**: `command` is empty, `name` is one of the canonical
///   [`GateKind`] wire names. The core knows how to run it for the profile
///   (e.g. `type_check` runs `cargo check` under the Rust profile).
/// - **custom**: `command` is a non-empty argv (e.g.
///   `["npm", "test"]`). The preflight runs it as a subprocess and uses the
///   exit code as the verdict (0 = pass, non-zero = fail), mirroring the
///   pattern used by pre-commit's `language: system` hooks and CI runners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GateSpec {
    /// Wire name, stable across runs. For built-ins this must match a
    /// [`GateKind::wire_name`]; for custom gates it is an arbitrary
    /// identifier the project picks.
    pub name: String,
    /// Argv for a custom shell gate. Empty for built-in gates.
    #[serde(default)]
    pub command: Vec<String>,
    /// Whether this gate must pass for the run to be `Ready`.
    pub requirement: GateRequirement,
}

impl GateSpec {
    /// Construct a built-in gate spec (no command — the core resolves it).
    #[must_use]
    pub fn builtin(kind: GateKind, requirement: GateRequirement) -> Self {
        Self {
            name: kind.wire_name().to_string(),
            command: Vec::new(),
            requirement,
        }
    }

    /// Construct a custom shell gate spec.
    #[must_use]
    pub fn custom(name: String, command: Vec<String>, requirement: GateRequirement) -> Self {
        Self {
            name,
            command,
            requirement,
        }
    }

    /// Whether this spec is a built-in gate (empty command).
    #[must_use]
    pub fn is_builtin(&self) -> bool {
        self.command.is_empty()
    }
}

/// On-disk profile document, written to `.forge-method/preflight.yaml`.
///
/// `schema_version` must equal [`PREFLIGHT_PROFILE_SCHEMA_VERSION`]. The
/// document pins the resolved profile and an explicit gate list; when absent,
/// `preflight` falls back to [`ProjectProfile::detect`] +
/// [`ProjectProfile::default_gates`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreflightProfileDocument {
    /// Must equal [`PREFLIGHT_PROFILE_SCHEMA_VERSION`].
    pub schema_version: String,
    /// Resolved profile (informational; gates are authoritative).
    pub profile: ProjectProfile,
    /// Explicit gate list. May be empty to mean "use the profile defaults".
    pub gates: Vec<GateSpec>,
}

impl PreflightProfileDocument {
    /// Build a fresh document for a freshly-detected profile, using the
    /// profile's default gate set. This is what `preflight init` writes.
    #[must_use]
    pub fn for_detected_profile(profile: ProjectProfile) -> Self {
        Self {
            schema_version: PREFLIGHT_PROFILE_SCHEMA_VERSION.to_string(),
            profile,
            gates: profile.default_gates(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profile_wire_name_round_trip() {
        for profile in [
            ProjectProfile::Rust,
            ProjectProfile::Node,
            ProjectProfile::Python,
            ProjectProfile::Go,
            ProjectProfile::Generic,
        ] {
            let wire = profile.wire_name();
            assert_eq!(ProjectProfile::from_wire_name(wire), Some(profile));
        }
        assert_eq!(ProjectProfile::from_wire_name("nonsense"), None);
    }

    fn fresh_temp_dir(label: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "forge-preflight-profile-{label}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).expect("create tempdir");
        dir
    }

    #[test]
    fn detect_rust_from_cargo_toml() {
        let dir = fresh_temp_dir("rust");
        std::fs::write(dir.join("Cargo.toml"), "").expect("write marker");
        assert_eq!(ProjectProfile::detect(&dir), ProjectProfile::Rust);
    }

    #[test]
    fn detect_node_from_package_json() {
        let dir = fresh_temp_dir("node");
        std::fs::write(dir.join("package.json"), "{}").expect("write marker");
        assert_eq!(ProjectProfile::detect(&dir), ProjectProfile::Node);
    }

    #[test]
    fn detect_python_from_any_of_three_markers() {
        for marker in ["pyproject.toml", "setup.py", "requirements.txt"] {
            let dir = fresh_temp_dir(&format!("python-{marker}"));
            std::fs::write(dir.join(marker), "").expect("write marker");
            assert_eq!(
                ProjectProfile::detect(&dir),
                ProjectProfile::Python,
                "marker {marker} should detect Python"
            );
        }
    }

    #[test]
    fn detect_go_from_go_mod() {
        let dir = fresh_temp_dir("go");
        std::fs::write(dir.join("go.mod"), "").expect("write marker");
        assert_eq!(ProjectProfile::detect(&dir), ProjectProfile::Go);
    }

    #[test]
    fn detect_generic_when_no_marker_present() {
        let dir = fresh_temp_dir("generic");
        assert_eq!(ProjectProfile::detect(&dir), ProjectProfile::Generic);
    }

    #[test]
    fn detect_rust_wins_over_node_in_polyglot_repo() {
        // Fixed probe order: Rust first.
        let dir = fresh_temp_dir("polyglot");
        std::fs::write(dir.join("Cargo.toml"), "").expect("write rust marker");
        std::fs::write(dir.join("package.json"), "{}").expect("write node marker");
        assert_eq!(ProjectProfile::detect(&dir), ProjectProfile::Rust);
    }

    #[test]
    fn default_gates_for_rust_runs_all_six_builtin_gates() {
        let gates = ProjectProfile::Rust.default_gates();
        let names: Vec<&str> = gates.iter().map(|g| g.name.as_str()).collect();
        assert_eq!(
            names,
            [
                "type_check",
                "format",
                "clippy_pedantic",
                "test",
                "validate",
                "regression_anchor"
            ]
        );
        // All built-ins (no shell command).
        assert!(gates.iter().all(GateSpec::is_builtin));
    }

    #[test]
    fn default_gates_for_generic_runs_only_language_agnostic_gates() {
        for profile in [
            ProjectProfile::Node,
            ProjectProfile::Python,
            ProjectProfile::Go,
            ProjectProfile::Generic,
        ] {
            let gates = profile.default_gates();
            let names: Vec<&str> = gates.iter().map(|g| g.name.as_str()).collect();
            assert_eq!(
                names,
                ["validate", "regression_anchor"],
                "profile {profile:?} should run only language-agnostic gates"
            );
        }
    }

    #[test]
    fn profile_document_serializes_and_round_trips() {
        let doc = PreflightProfileDocument::for_detected_profile(ProjectProfile::Node);
        let yaml = yaml_serde::to_string(&doc).expect("serialize");
        let back: PreflightProfileDocument = yaml_serde::from_str(&yaml).expect("deserialize");
        assert_eq!(back, doc);
        assert_eq!(back.schema_version, PREFLIGHT_PROFILE_SCHEMA_VERSION);
    }

    #[test]
    fn profile_document_rejects_unknown_field() {
        let yaml = format!(
            "schema_version: {PREFLIGHT_PROFILE_SCHEMA_VERSION}\n\
             profile: rust\n\
             gates: []\n\
             bogus: true\n"
        );
        let err = yaml_serde::from_str::<PreflightProfileDocument>(&yaml);
        assert!(err.is_err(), "deny_unknown_fields must reject 'bogus'");
    }

    #[test]
    fn custom_gate_spec_carries_command() {
        let spec = GateSpec::custom(
            "api_contract_test".to_string(),
            vec!["npx".to_string(), "my-api-cli".to_string()],
            GateRequirement::Required,
        );
        assert!(!spec.is_builtin());
        assert_eq!(
            spec.command,
            vec!["npx".to_string(), "my-api-cli".to_string()]
        );
    }
}
