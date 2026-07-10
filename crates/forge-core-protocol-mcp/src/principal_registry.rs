//! Compatibility re-exports for the operator-owned principal registry.
//!
//! Verification and opaque authority construction now live in the
//! host-neutral `forge-core-authority` crate. This module preserves the P4b.1a
//! import path for MCP adapter consumers.

pub use forge_core_authority::principal_registry::*;
