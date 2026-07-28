//! Canonical parsing of Effect-related command input vocabulary.
//!
//! Command adapters remain responsible for their own usage diagnostics. This
//! module owns only the stable string-to-type mapping so every adapter accepts
//! the same vocabulary.

use forge_core_contracts::{runtime::RuntimeKind, tool_effect::EffectTargetKind};
use forge_core_store::{EffectMetadataAdapterTrigger, EffectMetadataConsumerUse};

pub(crate) fn parse_target_kind(value: &str) -> Option<EffectTargetKind> {
    match value {
        "file_path" => Some(EffectTargetKind::FilePath),
        "glob" => Some(EffectTargetKind::Glob),
        "state_key" => Some(EffectTargetKind::StateKey),
        "artifact_id" => Some(EffectTargetKind::ArtifactId),
        "evidence_id" => Some(EffectTargetKind::EvidenceId),
        "ledger_stream" => Some(EffectTargetKind::LedgerStream),
        "request_stream" => Some(EffectTargetKind::RequestStream),
        "completion_id" => Some(EffectTargetKind::CompletionId),
        _ => None,
    }
}

pub(crate) fn parse_runtime_kind(value: &str) -> Option<RuntimeKind> {
    match value {
        "codex" => Some(RuntimeKind::Codex),
        "cursor" => Some(RuntimeKind::Cursor),
        "claude" => Some(RuntimeKind::Claude),
        "opencode" => Some(RuntimeKind::Opencode),
        "vscode" => Some(RuntimeKind::Vscode),
        "pidev" => Some(RuntimeKind::Pidev),
        "forge_standalone" => Some(RuntimeKind::ForgeStandalone),
        "custom" => Some(RuntimeKind::Custom),
        _ => None,
    }
}

pub(crate) fn parse_metadata_consumer_use(value: &str) -> Option<EffectMetadataConsumerUse> {
    match value {
        "discovery" => Some(EffectMetadataConsumerUse::Discovery),
        "diagnostics" => Some(EffectMetadataConsumerUse::Diagnostics),
        "handoff_context" => Some(EffectMetadataConsumerUse::HandoffContext),
        _ => None,
    }
}

pub(crate) fn parse_metadata_adapter_trigger(value: &str) -> Option<EffectMetadataAdapterTrigger> {
    match value {
        "evidence_discovery" => Some(EffectMetadataAdapterTrigger::EvidenceDiscovery),
        "diagnostics" => Some(EffectMetadataAdapterTrigger::Diagnostics),
        "handoff_preparation" => Some(EffectMetadataAdapterTrigger::HandoffPreparation),
        "manual_inspection" => Some(EffectMetadataAdapterTrigger::ManualInspection),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_unknown_effect_vocabulary() {
        assert_eq!(parse_target_kind("unknown"), None);
        assert_eq!(parse_runtime_kind("unknown"), None);
        assert_eq!(parse_metadata_consumer_use("unknown"), None);
        assert_eq!(parse_metadata_adapter_trigger("unknown"), None);
    }

    #[test]
    fn parsing_is_exhaustive_for_every_effect_vocabulary_variant() {
        let target_kinds = [
            ("file_path", EffectTargetKind::FilePath),
            ("glob", EffectTargetKind::Glob),
            ("state_key", EffectTargetKind::StateKey),
            ("artifact_id", EffectTargetKind::ArtifactId),
            ("evidence_id", EffectTargetKind::EvidenceId),
            ("ledger_stream", EffectTargetKind::LedgerStream),
            ("request_stream", EffectTargetKind::RequestStream),
            ("completion_id", EffectTargetKind::CompletionId),
        ];
        for (name, value) in target_kinds {
            assert_eq!(parse_target_kind(name), Some(value));
            assert_eq!(target_kind_name(value), name);
        }

        let runtime_kinds = [
            ("codex", RuntimeKind::Codex),
            ("cursor", RuntimeKind::Cursor),
            ("claude", RuntimeKind::Claude),
            ("opencode", RuntimeKind::Opencode),
            ("vscode", RuntimeKind::Vscode),
            ("pidev", RuntimeKind::Pidev),
            ("forge_standalone", RuntimeKind::ForgeStandalone),
            ("custom", RuntimeKind::Custom),
        ];
        for (name, value) in runtime_kinds {
            assert_eq!(parse_runtime_kind(name), Some(value));
            assert_eq!(runtime_kind_name(value), name);
        }

        let consumer_uses = [
            ("discovery", EffectMetadataConsumerUse::Discovery),
            ("diagnostics", EffectMetadataConsumerUse::Diagnostics),
            ("handoff_context", EffectMetadataConsumerUse::HandoffContext),
        ];
        for (name, value) in consumer_uses {
            assert_eq!(parse_metadata_consumer_use(name), Some(value));
            assert_eq!(consumer_use_name(value), name);
        }

        let adapter_triggers = [
            (
                "evidence_discovery",
                EffectMetadataAdapterTrigger::EvidenceDiscovery,
            ),
            ("diagnostics", EffectMetadataAdapterTrigger::Diagnostics),
            (
                "handoff_preparation",
                EffectMetadataAdapterTrigger::HandoffPreparation,
            ),
            (
                "manual_inspection",
                EffectMetadataAdapterTrigger::ManualInspection,
            ),
        ];
        for (name, value) in adapter_triggers {
            assert_eq!(parse_metadata_adapter_trigger(name), Some(value));
            assert_eq!(adapter_trigger_name(value), name);
        }
    }

    const fn target_kind_name(value: EffectTargetKind) -> &'static str {
        match value {
            EffectTargetKind::FilePath => "file_path",
            EffectTargetKind::Glob => "glob",
            EffectTargetKind::StateKey => "state_key",
            EffectTargetKind::ArtifactId => "artifact_id",
            EffectTargetKind::EvidenceId => "evidence_id",
            EffectTargetKind::LedgerStream => "ledger_stream",
            EffectTargetKind::RequestStream => "request_stream",
            EffectTargetKind::CompletionId => "completion_id",
        }
    }

    const fn runtime_kind_name(value: RuntimeKind) -> &'static str {
        match value {
            RuntimeKind::Codex => "codex",
            RuntimeKind::Cursor => "cursor",
            RuntimeKind::Claude => "claude",
            RuntimeKind::Opencode => "opencode",
            RuntimeKind::Vscode => "vscode",
            RuntimeKind::Pidev => "pidev",
            RuntimeKind::ForgeStandalone => "forge_standalone",
            RuntimeKind::Custom => "custom",
        }
    }

    const fn consumer_use_name(value: EffectMetadataConsumerUse) -> &'static str {
        match value {
            EffectMetadataConsumerUse::Discovery => "discovery",
            EffectMetadataConsumerUse::Diagnostics => "diagnostics",
            EffectMetadataConsumerUse::HandoffContext => "handoff_context",
        }
    }

    const fn adapter_trigger_name(value: EffectMetadataAdapterTrigger) -> &'static str {
        match value {
            EffectMetadataAdapterTrigger::EvidenceDiscovery => "evidence_discovery",
            EffectMetadataAdapterTrigger::Diagnostics => "diagnostics",
            EffectMetadataAdapterTrigger::HandoffPreparation => "handoff_preparation",
            EffectMetadataAdapterTrigger::ManualInspection => "manual_inspection",
        }
    }
}
