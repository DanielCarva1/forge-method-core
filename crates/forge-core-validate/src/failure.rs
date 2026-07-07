//! Adjacently-tagged serde representation for typed failures (V2.D prep).
//!
//! This module is **preparation** for the typed failure vocabulary (V2.D) that
//! will live in `CliEnvelope`. It is not yet consumed by anything; it documents
//! and exercises the serde tagging strategy so V2.D can drop concrete types in
//! without re-litigating the representation.
//!
//! # Why adjacent tagging, never `untagged`
//!
//! Failures that round-trip through JSON (e.g. an error returned over the MCP
//! transport) must not lose their variant. serde offers three externally-visible
//! strategies for an enum:
//!
//! - **externally tagged** (serde default): `{"Variant": {...}}`. Round-trips
//!   the variant, but forbids `Variant` from being a unit variant alongside a
//!   newtype variant with the same name and is awkward when the payload is
//!   itself an enum.
//! - **internally tagged** (`#[serde(tag = "type")]`): `{"type": "Variant",
//!   ...fields}`. Requires the payload to be a struct (no newtype-of-enum, no
//!   tuple) — too restrictive for a failure vocabulary whose variants carry
//!   heterogeneous payloads.
//! - **adjacently tagged** (`#[serde(tag = "type", content = "data")]`):
//!   `{"type": "Variant", "data": {...}}`. Round-trips the variant, accepts any
//!   payload shape (unit / newtype / struct), and is unambiguous. **This is what
//!   we use.**
//! - **untagged** (`#[serde(untagged)]`): `{...}`. Loses fidelity — serde
//!   tries each variant in order and the variant identity is gone from the wire.
//!   This is the footgun serde issue #1307 documents (an `untagged` enum of
//!   struct variants silently collapses to the first structurally-compatible
//!   one). **Never use this for failures.**
//!
//! The canonical reference is serde issue #1307 ("untagged loses type
//! information"); the adjacently-tagged representation is the documented fix.
//!
//! # What V2.D will add
//!
//! V2.D will define `TypedFailure` here — a hand-rolled failure enum
//! (no `thiserror`, derive `Debug, Clone, PartialEq, Eq`) carrying the
//! canonical [`DiagnosticCode`](crate::DiagnosticCode) / future
//! [`DiagnosticCodeDef`](crate::codes::DiagnosticCodeDef) and enough context to
//! reconstruct the failure at the consumer. It will be annotated with the
//! adjacent-tag attribute shown in [`ADJACENT_TAG`]. Until then this module is
//! documentation + a `const` holding the attribute's field names, so the
//! representation is fixed in one place.

/// The field name serde uses to carry the failure's variant tag.
///
/// Used as `#[serde(tag = ADJACENT_TAG)]`.
pub const ADJACENT_TAG: &str = "type";

/// The field name serde uses to carry the failure's payload.
///
/// Used as `#[serde(content = ADJACENT_CONTENT)]`.
pub const ADJACENT_CONTENT: &str = "data";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adjacent_tag_field_names_are_stable() {
        // These names are wire-stable: changing them is a breaking change to the
        // CliEnvelope failure representation. Pin them in a test so a refactor
        // cannot silently rename them.
        assert_eq!(ADJACENT_TAG, "type");
        assert_eq!(ADJACENT_CONTENT, "data");
    }
}
