//! Canonical diagnostic-code const-table (V1.B foundation).
//!
//! This module introduces a **data-driven** diagnostic-code vocabulary that runs
//! *alongside* the existing [`DiagnosticCode`](crate::DiagnosticCode) enum. It is
//! the canonical accumulator foundation that the later V2.B migration will
//! consume; **nothing here changes or replaces the existing enum** — both coexist
//! until callers are migrated, and the enum stays exactly as-is for backwards
//! compatibility.
//!
//! # Why data, not a bigger enum?
//!
//! Research across the four largest Rust linters and the rust-analyzer
//! codebase:
//!
//! - **rustc** (`DiagCtxt`): accumulate-then-`abort_if_errors`, with phase
//!   gates. The lint metadata is a `static` *table* (`Lint` struct), not an
//!   enum variant carrying its own description/severity.
//! - **`rust-analyzer`'s own source** *recommends against* a strongly-typed enum
//!   for diagnostics: an enum couples the set of known diagnostics to a single
//!   compile unit and makes external/config-driven diagnostics impossible.
//! - **`clippy` / `deno_lint` / `dprint`**: none of them use an enum for `code`.
//!   They all declare codes as a `const`/`static` table of `&'static str`
//!   identifiers, where the severity/category is *data* alongside the id,
//!   emitted by a single declaration macro (`declare_clippy_lint!`,
//!   `deno_lint::declare_deno_lint!`, etc.).
//!
//! The canonical `DiagnosticCode` enum in this crate (~90 variants) is currently
//! degraded to `format!("{:?}")` (a `Debug` string) at the CLI boundary, which
//! loses type information and produces ugly `YamlReadFailed` wire strings instead
//! of stable `yaml_read_failed` identifiers. The const-table here makes the
//! wire-format `code` a stable `&'static str` from the start; the metadata
//! (default severity, category, description) is lookupable data rather than
//! hardcoded per-variant.
//!
//! # What lives here
//!
//! - [`DiagnosticCodeDef`]: the const-table entry (struct of `&'static str` +
//!   default severity).
//! - [`declare_diagnostic_code!`]: the declaration macro that emits a
//!   `pub static <NAME>: DiagnosticCodeDef`.
//! - [`DiagnosticRegistry`]: a const-constructed lookup from code string →
//!   `&'static DiagnosticCodeDef`.
//!
//! This is infrastructure only. The V2.B task migrates the existing call sites
//! (graph / eval / eval-harness / CLI) onto these definitions; until then the
//! existing enum and its call sites are untouched and continue to compile.

use crate::DiagnosticSeverity;
use serde::Serialize;

/// Metadata for a diagnostic code, declared as a `const` (data, not enum
/// variant). Modeled on rustc's `Lint` struct and the `clippy` / `deno_lint` /
/// `dprint` const-table approach: the code is a stable `snake_case` `&'static str`
/// identifier, and severity/category are lookupable data rather than hardcoded
/// in an enum variant.
///
/// The wire format is the [`code` field](DiagnosticCodeDef::code) — a stable
/// string such as `"memory_authority_floor"`, **never** the `Debug` of an enum
/// variant. That keeps type information across the JSON/MCP boundary instead of
/// degrading it to `format!("{:?}")`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DiagnosticCodeDef {
    /// Stable, `snake_case` identifier (e.g. `"memory_authority_floor"`).
    /// This is the wire format — never `Debug` of an enum variant.
    pub code: &'static str,
    /// Human-readable summary (what this diagnostic means).
    pub description: &'static str,
    /// The subsystem this code belongs to (e.g. `"memory"`, `"graph"`,
    /// `"risk-audit"`).
    pub category: &'static str,
    /// Default severity; can be overridden by external config (the
    /// `ESLint` / `rustc` lint-level model: a code declares its *default* level,
    /// and a consumer config may promote/demote it).
    pub default_severity: DiagnosticSeverity,
}

/// Declares a diagnostic code as a `pub static` const-table entry.
///
/// # Usage
///
/// ```ignore
/// declare_diagnostic_code!(
///     MEMORY_AUTHORITY_FLOOR,
///     "memory_authority_floor",
///     "memory",
///     Error,
///     "memory authority must be at the floor of the operation graph"
/// );
/// ```
///
/// This generates:
///
/// ```ignore
/// pub static MEMORY_AUTHORITY_FLOOR: DiagnosticCodeDef = DiagnosticCodeDef {
///     code: "memory_authority_floor",
///     description: "memory authority must be at the floor of the operation graph",
///     category: "memory",
///     default_severity: DiagnosticSeverity::Error,
/// };
/// ```
///
/// Modeled on rustc's `declare_tool_lint!` and clippy's
/// `declare_clippy_lint!`: one macro line per code, metadata is data. The
/// `$sev` argument is an unqualified `DiagnosticSeverity` variant name
/// (`Error` / `Warning`) so the call site reads like the lint-level decl in
/// rustc (`Warn` / `Deny`).
#[macro_export]
macro_rules! declare_diagnostic_code {
    (
        $(#[doc = $doc:literal])*
        $id:ident,
        $code:literal,
        $category:literal,
        $sev:ident,
        $desc:literal
    ) => {
        $(#[doc = $doc])*
        pub static $id: $crate::codes::DiagnosticCodeDef = $crate::codes::DiagnosticCodeDef {
            code: $code,
            description: $desc,
            category: $category,
            default_severity: $crate::DiagnosticSeverity::$sev,
        };
    };
}

/// Registry of known diagnostic codes (const-table lookup).
///
/// Consumers query by code string to get metadata; this replaces the need for a
/// giant `match`-on-enum at every consumption site (the pattern that today
/// forces the CLI boundary into `format!("{:?}")`). A registry is constructed
/// from a `&'static` slice of `&'static DiagnosticCodeDef` references, so the
/// whole table can be assembled at compile time and held as a `const`.
///
/// This is the lookup seam the future config-driven severity-override layer
/// (ESLint/rustc lint-level model) will resolve codes through: a config entry
/// like `codes: { memory_authority_floor: warn }` is applied by looking the
/// code up here, reading [`DiagnosticCodeDef::default_severity`], and overriding
/// it. The registry itself stays read-only data.
pub struct DiagnosticRegistry {
    entries: &'static [&'static DiagnosticCodeDef],
}

impl DiagnosticRegistry {
    /// Construct a registry over a `&'static` table of code definitions.
    ///
    /// The slice is borrowed for `'static`, so the registry is freely copyable
    /// and storable in `const`s.
    #[must_use]
    pub const fn new(entries: &'static [&'static DiagnosticCodeDef]) -> Self {
        Self { entries }
    }

    /// Look a code up by its stable wire identifier.
    ///
    /// Returns the `'static` definition so callers can hold the metadata without
    /// borrowing the registry.
    #[must_use]
    pub fn lookup(&self, code: &str) -> Option<&'static DiagnosticCodeDef> {
        // `self.entries.iter()` yields `&&'static DiagnosticCodeDef`; `copied()`
        // collapses the outer reference to `Option<&'static DiagnosticCodeDef>`.
        self.entries
            .iter()
            .copied()
            .find(|entry| entry.code == code)
    }

    /// Iterate over every registered code definition.
    pub fn entries(&self) -> impl Iterator<Item = &'static DiagnosticCodeDef> {
        self.entries.iter().copied()
    }
}

// ---------------------------------------------------------------------------
// Seed const-table.
//
// These are a *seed* — a few representative codes that exercise every category
// and both severities, proving the macro + registry round-trip. The full
// migration of all ~90 existing `DiagnosticCode` enum variants into const-table
// entries is part of V2.B (it touches every call site, which is out of scope for
// this additive foundation). Declaring them here would duplicate the enum's
// authority source; instead V2.B will delete the enum variants as it migrates
// each caller onto the matching const entry.
// ---------------------------------------------------------------------------

declare_diagnostic_code!(
    /// Memory authority must sit at the floor of the operation graph: an
    /// operation that mutates durable state without memory authority can run
    /// "below" the ledger it is supposed to write to, silently losing durable
    /// effect.
    MEMORY_AUTHORITY_FLOOR,
    "memory_authority_floor",
    "memory",
    Error,
    "memory authority must be at the floor of the operation graph"
);

declare_diagnostic_code!(
    /// A contract document referenced by `ref`/`contract_ref`/etc. does not
    /// resolve against the built reference index.
    MISSING_REFERENCE,
    "missing_reference",
    "graph",
    Error,
    "a contract reference does not resolve against the reference index"
);

declare_diagnostic_code!(
    /// `conflict_policy: silent_last_writer_wins` is the documented anti-pattern
    /// F07 exists to forbid. It is structurally permitted (so a deployment can
    /// opt in) but is flagged at warning severity so the opt-in is loud.
    GOVERNANCE_SILENT_LAST_WRITER_WINS,
    "governance_silent_last_writer_wins",
    "governance",
    Warning,
    "silent_last_writer_wins destroys the conflict signal (CRDT/XACML posture)"
);

declare_diagnostic_code!(
    /// An external risk-audit rule matched an AI-induced anti-pattern in the
    /// target source (F11). Default error; the rule's own severity may override
    /// it per-match.
    RISK_AUDIT_ANTIPATTERN_MATCHED,
    "risk_audit_antipattern_matched",
    "risk-audit",
    Error,
    "risk-audit rule matched an AI-induced anti-pattern"
);

/// The backing const slice for [`SEED_REGISTRY`]. Held as an explicit `const`
/// so the `&'static` borrow promoted into `SEED_REGISTRY` is well-formed (a
/// slice literal written inline inside `const` initializer is not itself
/// `&'static` without this intermediate binding).
const SEED_ENTRIES: &[&DiagnosticCodeDef] = &[
    &MEMORY_AUTHORITY_FLOOR,
    &MISSING_REFERENCE,
    &GOVERNANCE_SILENT_LAST_WRITER_WINS,
    &RISK_AUDIT_ANTIPATTERN_MATCHED,
];

/// The seed registry: every code declared above. New codes are added by
/// declaring them with [`declare_diagnostic_code!`] and appending a `&NAME`
/// entry to [`SEED_ENTRIES`]. V2.B will grow this into the full vocabulary as
/// callers migrate.
pub const SEED_REGISTRY: DiagnosticRegistry = DiagnosticRegistry::new(SEED_ENTRIES);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seed_registry_resolves_every_declared_code() {
        for entry in SEED_REGISTRY.entries() {
            assert_eq!(
                SEED_REGISTRY.lookup(entry.code).map(|e| e.code),
                Some(entry.code)
            );
        }
    }

    #[test]
    fn lookup_returns_static_metadata() {
        let def = SEED_REGISTRY
            .lookup("memory_authority_floor")
            .expect("seed code resolves");
        assert_eq!(def.code, "memory_authority_floor");
        assert_eq!(def.category, "memory");
        assert_eq!(def.default_severity, DiagnosticSeverity::Error);
        // The metadata is 'static — safe to hold without the registry.
        let held: &'static DiagnosticCodeDef = def;
        assert_eq!(held.code, "memory_authority_floor");
    }

    #[test]
    fn unknown_code_is_none() {
        assert!(SEED_REGISTRY.lookup("does_not_exist").is_none());
    }

    #[test]
    fn warning_severity_code_round_trips() {
        let def = SEED_REGISTRY
            .lookup("governance_silent_last_writer_wins")
            .expect("seed warning code resolves");
        assert_eq!(def.default_severity, DiagnosticSeverity::Warning);
    }
}
