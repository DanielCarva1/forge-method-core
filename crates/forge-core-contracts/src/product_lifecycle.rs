//! Host-neutral product lifecycle release contracts.
//!
//! These documents describe immutable local release bundles that the product
//! lifecycle adapter may verify and install. They are candidate input only:
//! deserialization does not grant distribution, filesystem, project, workflow,
//! release, host-selection, signing, trust, or private-key authority.

use crate::{RepoPath, RuntimeKind, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION: &str = "0.1";
pub const MAX_PRODUCT_LIFECYCLE_ASSETS: usize = 64;
pub const MAX_PRODUCT_LIFECYCLE_CHANGES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleReleaseDocument {
    pub schema_version: String,
    pub product_lifecycle_release: ProductLifecycleRelease,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleRelease {
    pub release_id: StableId,
    pub version: String,
    pub compatible_core_version: String,
    pub channel: ProductLifecycleChannel,
    pub source_ref: String,
    pub provenance_ref: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature_ref: Option<String>,
    pub rollback_ref: String,
    pub changes: Vec<ProductLifecycleChange>,
    pub assets: Vec<ProductLifecycleAsset>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductLifecycleChannel {
    Stable,
    Canary,
    Dev,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleChange {
    pub change_id: StableId,
    pub kind: ProductLifecycleChangeKind,
    pub summary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductLifecycleChangeKind {
    Added,
    Changed,
    Fixed,
    Security,
    Removed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleAsset {
    pub asset_id: StableId,
    pub kind: ProductLifecycleAssetKind,
    pub source_path: RepoPath,
    pub install_path: RepoPath,
    pub sha256: String,
    pub executable: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host: Option<RuntimeKind>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProductLifecycleAssetKind {
    CoreBinary,
    HostAdapter,
    HostConfiguration,
    Wrapper,
}

impl ProductLifecycleReleaseDocument {
    /// Validate the closed bundle shape before any local artifact is read.
    #[must_use]
    #[allow(clippy::too_many_lines)]
    pub fn validation_issues(&self) -> Vec<String> {
        let mut issues = Vec::new();
        if self.schema_version != PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION {
            issues.push("schema_version: unsupported schema version".to_owned());
        }
        let release = &self.product_lifecycle_release;
        if !valid_stable_id(&release.release_id.0) {
            issues.push("product_lifecycle_release.release_id: invalid stable id".to_owned());
        }
        for (field, value) in [
            ("version", release.version.as_str()),
            (
                "compatible_core_version",
                release.compatible_core_version.as_str(),
            ),
            ("source_ref", release.source_ref.as_str()),
            ("provenance_ref", release.provenance_ref.as_str()),
            ("rollback_ref", release.rollback_ref.as_str()),
        ] {
            if value.trim().is_empty() {
                issues.push(format!("product_lifecycle_release.{field}: required"));
            }
        }
        if release
            .signature_ref
            .as_deref()
            .is_some_and(|value| value.trim().is_empty())
        {
            issues.push("product_lifecycle_release.signature_ref: blank when present".to_owned());
        }
        if release.changes.is_empty() || release.changes.len() > MAX_PRODUCT_LIFECYCLE_CHANGES {
            issues.push(format!(
                "product_lifecycle_release.changes: expected 1..={MAX_PRODUCT_LIFECYCLE_CHANGES} items"
            ));
        }
        if release.assets.is_empty() || release.assets.len() > MAX_PRODUCT_LIFECYCLE_ASSETS {
            issues.push(format!(
                "product_lifecycle_release.assets: expected 1..={MAX_PRODUCT_LIFECYCLE_ASSETS} items"
            ));
        }

        let mut change_ids = BTreeSet::new();
        for (index, change) in release.changes.iter().enumerate() {
            if !valid_stable_id(&change.change_id.0) {
                issues.push(format!("changes[{index}].change_id: invalid stable id"));
            }
            if !change_ids.insert(change.change_id.0.as_str()) {
                issues.push(format!("changes[{index}].change_id: duplicate"));
            }
            let summary = change.summary.trim();
            if summary.is_empty() || summary.len() > 512 {
                issues.push(format!("changes[{index}].summary: expected 1..=512 bytes"));
            }
        }

        let mut asset_ids = BTreeSet::new();
        let mut install_paths = BTreeSet::new();
        let mut core_binary_count = 0usize;
        for (index, asset) in release.assets.iter().enumerate() {
            if !valid_stable_id(&asset.asset_id.0) {
                issues.push(format!("assets[{index}].asset_id: invalid stable id"));
            }
            if !asset_ids.insert(asset.asset_id.0.as_str()) {
                issues.push(format!("assets[{index}].asset_id: duplicate"));
            }
            if !valid_relative_path(&asset.source_path.0) {
                issues.push(format!("assets[{index}].source_path: unsafe relative path"));
            }
            if !valid_relative_path(&asset.install_path.0) {
                issues.push(format!(
                    "assets[{index}].install_path: unsafe relative path"
                ));
            } else if !install_paths.insert(asset.install_path.0.as_str()) {
                issues.push(format!("assets[{index}].install_path: duplicate"));
            }
            if !valid_sha256(&asset.sha256) {
                issues.push(format!("assets[{index}].sha256: invalid digest"));
            }
            match asset.kind {
                ProductLifecycleAssetKind::CoreBinary => {
                    core_binary_count += 1;
                    if asset.host.is_some() {
                        issues.push(format!("assets[{index}].host: forbidden for core binary"));
                    }
                }
                ProductLifecycleAssetKind::HostAdapter
                | ProductLifecycleAssetKind::HostConfiguration => {
                    if asset.host.is_none() {
                        issues.push(format!("assets[{index}].host: required for host asset"));
                    }
                }
                ProductLifecycleAssetKind::Wrapper => {
                    if asset.host.is_some() {
                        issues.push(format!("assets[{index}].host: forbidden for wrapper"));
                    }
                }
            }
            if asset.executable
                && matches!(asset.kind, ProductLifecycleAssetKind::HostConfiguration)
            {
                issues.push(format!(
                    "assets[{index}].executable: host configuration must not be executable"
                ));
            }
        }
        if core_binary_count != 1 {
            issues.push(
                "product_lifecycle_release.assets: exactly one core_binary is required".to_owned(),
            );
        }
        issues
    }
}

fn valid_stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

fn valid_relative_path(value: &str) -> bool {
    if value.is_empty() || value.len() > 512 || value.contains('\\') {
        return false;
    }
    let path = Path::new(value);
    !path.is_absolute()
        && path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
}

fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn document() -> ProductLifecycleReleaseDocument {
        ProductLifecycleReleaseDocument {
            schema_version: PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION.to_owned(),
            product_lifecycle_release: ProductLifecycleRelease {
                release_id: StableId("forge-core-0.12.1".to_owned()),
                version: "0.12.1".to_owned(),
                compatible_core_version: "0.12.1".to_owned(),
                channel: ProductLifecycleChannel::Stable,
                source_ref:
                    "git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567"
                        .to_owned(),
                provenance_ref: "provenance/forge-core-0.12.1.json".to_owned(),
                signature_ref: Some("signatures/forge-core-0.12.1.sig".to_owned()),
                rollback_ref: "releases/forge-core-0.12.0".to_owned(),
                changes: vec![ProductLifecycleChange {
                    change_id: StableId("change.lifecycle".to_owned()),
                    kind: ProductLifecycleChangeKind::Added,
                    summary: "Add the owned product lifecycle.".to_owned(),
                }],
                assets: vec![ProductLifecycleAsset {
                    asset_id: StableId("asset.forge-core".to_owned()),
                    kind: ProductLifecycleAssetKind::CoreBinary,
                    source_path: RepoPath("bin/forge-core".to_owned()),
                    install_path: RepoPath("bin/forge-core".to_owned()),
                    sha256: format!("sha256:{}", "a".repeat(64)),
                    executable: true,
                    host: None,
                }],
            },
        }
    }

    #[test]
    fn valid_release_bundle_is_closed_and_bounded() {
        assert!(document().validation_issues().is_empty());
    }

    #[test]
    fn release_bundle_rejects_paths_digests_duplicates_and_host_drift() {
        let mut document = document();
        let asset = &mut document.product_lifecycle_release.assets[0];
        asset.source_path = RepoPath("../forge-core".to_owned());
        asset.sha256 = format!("sha256:{}", "A".repeat(64));
        asset.host = Some(RuntimeKind::Codex);
        document
            .product_lifecycle_release
            .assets
            .push(document.product_lifecycle_release.assets[0].clone());
        let issues = document.validation_issues().join("\n");
        assert!(issues.contains("unsafe relative path"));
        assert!(issues.contains("invalid digest"));
        assert!(issues.contains("duplicate"));
        assert!(issues.contains("forbidden for core binary"));
        assert!(issues.contains("exactly one core_binary"));
    }

    #[test]
    fn release_bundle_rejects_unknown_fields() {
        let raw = r#"
schema_version: "0.1"
product_lifecycle_release:
  release_id: forge-core-0.12.1
  version: 0.12.1
  compatible_core_version: 0.12.1
  channel: stable
  source_ref: git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567
  provenance_ref: provenance.json
  rollback_ref: prior
  changes: []
  assets: []
  selected_host: codex
"#;
        assert!(yaml_serde::from_str::<ProductLifecycleReleaseDocument>(raw).is_err());
    }
}
