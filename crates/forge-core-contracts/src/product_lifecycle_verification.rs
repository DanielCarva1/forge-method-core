#![allow(clippy::missing_errors_doc)]

//! Closed candidate inputs for product-lifecycle artifact verification.
//!
//! These inputs name the exact local evidence that an install or update owner
//! must pass again to the existing cryptographic verifiers while holding the
//! exact candidate asset bytes. They intentionally contain no verification
//! result, trust decision, installation permission, activation permission, or
//! host selection. A prior serialized result is therefore not an admission.

use crate::{ProductLifecycleReleaseDocument, RepoPath, StableId};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Component, Path};

pub const PRODUCT_LIFECYCLE_TRUSTED_VERIFICATION_SCHEMA_VERSION: &str = "0.1";
pub const PRODUCT_LIFECYCLE_CHECKSUM_ADMISSION_SCHEMA_VERSION: &str = "0.1";
pub const MAX_PRODUCT_LIFECYCLE_ISSUER_CERTIFICATES: usize = 16;

/// Checksum-only candidate admission material. It is deliberately a separate
/// document from trusted verification input so a checksum match cannot be
/// confused with signature, provenance, identity/root, or transparency proof.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleChecksumAdmissionDocument {
    pub schema_version: String,
    pub product_lifecycle_checksum_admission: ProductLifecycleChecksumAdmission,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleChecksumAdmission {
    pub release_id: StableId,
    pub assets: Vec<ProductLifecycleChecksumAssetInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleChecksumAssetInput {
    pub asset_id: StableId,
    pub asset_source_path: RepoPath,
    pub expected_sha256: String,
}

/// Closed, per-asset input for a fresh cryptographic verification invocation.
/// The verifier must open `asset_source_path` as the exact candidate bytes and
/// rerun every represented check; this document cannot carry a cached result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleTrustedVerificationInputDocument {
    pub schema_version: String,
    pub product_lifecycle_trusted_verification_input: ProductLifecycleTrustedVerificationInput,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleTrustedVerificationInput {
    pub release_id: StableId,
    pub assets: Vec<ProductLifecycleAssetTrustedVerificationInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleAssetTrustedVerificationInput {
    pub asset_id: StableId,
    /// Must exactly name the release asset source path whose no-follow bytes
    /// are later installed.
    pub asset_source_path: RepoPath,
    /// Must exactly equal the release asset digest.
    pub expected_sha256: String,
    pub detached_provenance: ProductLifecycleDetachedProvenanceInput,
    pub sigstore: ProductLifecycleSigstoreSubjectInput,
}

/// Inputs for the existing detached provenance signature and exact SLSA
/// subject verifier. Paths are candidate evidence only; private signing keys
/// are neither represented nor permitted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleDetachedProvenanceInput {
    pub provenance_path: RepoPath,
    pub signature_path: RepoPath,
    pub public_key_path: RepoPath,
    pub transparency_log_path: RepoPath,
    pub expected_builder_id: String,
    pub expected_source_uri: String,
    /// Must exactly equal the immutable `source_ref` in the release document.
    pub expected_source_ref: String,
}

/// Inputs for the existing Sigstore DSSE/in-toto subject, trusted identity/root,
/// and Rekor inclusion verifier. The trust policy is supplied as evidence to be
/// freshly evaluated; it is not a trust grant carried by this contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProductLifecycleSigstoreSubjectInput {
    pub bundle_path: RepoPath,
    pub trust_policy_path: RepoPath,
    pub certificate_path: RepoPath,
    pub issuer_certificate_paths: Vec<RepoPath>,
    pub rekor_log_entry_path: RepoPath,
    pub rekor_public_key_path: RepoPath,
    pub expected_rekor_log_id: String,
    pub expected_predicate_type: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProductLifecycleVerificationInputError {
    UnsupportedSchemaVersion { found: String },
    InvalidIdentifier { path: String },
    InvalidPath { path: String },
    InvalidDigest { path: String },
    RequiredField { path: String },
    DuplicateAsset { asset_id: String },
    MissingReleaseAsset { asset_id: String },
    ReleaseAssetMismatch { asset_id: String, field: String },
    DuplicateEvidencePath { path: String },
    IssuerCertificateLimitExceeded,
}

impl ProductLifecycleChecksumAdmissionDocument {
    /// Validate only the checksum admission shape and exact release bindings.
    /// A successful result is not trusted verification or install authority.
    pub fn validate_for_release(
        &self,
        release: &ProductLifecycleReleaseDocument,
    ) -> Result<(), ProductLifecycleVerificationInputError> {
        validate_schema(
            &self.schema_version,
            PRODUCT_LIFECYCLE_CHECKSUM_ADMISSION_SCHEMA_VERSION,
        )?;
        validate_release_id(
            &self.product_lifecycle_checksum_admission.release_id,
            release,
        )?;
        validate_complete_asset_set(
            &self.product_lifecycle_checksum_admission.assets,
            release,
            |input| &input.asset_id,
            |input| &input.asset_source_path,
            |input| &input.expected_sha256,
        )
    }
}

impl ProductLifecycleTrustedVerificationInputDocument {
    /// Validate that every release asset has closed, exact rerun inputs for all
    /// required cryptographic verification stages. This validates inputs only;
    /// callers must still invoke the existing verifiers against exact bytes.
    pub fn validate_for_release(
        &self,
        release: &ProductLifecycleReleaseDocument,
    ) -> Result<(), ProductLifecycleVerificationInputError> {
        validate_schema(
            &self.schema_version,
            PRODUCT_LIFECYCLE_TRUSTED_VERIFICATION_SCHEMA_VERSION,
        )?;
        let input = &self.product_lifecycle_trusted_verification_input;
        validate_release_id(&input.release_id, release)?;
        validate_complete_asset_set(
            &input.assets,
            release,
            |asset| &asset.asset_id,
            |asset| &asset.asset_source_path,
            |asset| &asset.expected_sha256,
        )?;

        for asset in &input.assets {
            validate_trusted_asset(asset, release)?;
        }
        Ok(())
    }
}

fn validate_release_id(
    release_id: &StableId,
    release: &ProductLifecycleReleaseDocument,
) -> Result<(), ProductLifecycleVerificationInputError> {
    required_id(release_id, "release_id")?;
    if release_id == &release.product_lifecycle_release.release_id {
        Ok(())
    } else {
        Err(
            ProductLifecycleVerificationInputError::ReleaseAssetMismatch {
                asset_id: release_id.0.clone(),
                field: "release_id".to_owned(),
            },
        )
    }
}

fn validate_complete_asset_set<T>(
    inputs: &[T],
    release: &ProductLifecycleReleaseDocument,
    id: impl Fn(&T) -> &StableId,
    source_path: impl Fn(&T) -> &RepoPath,
    expected_sha256: impl Fn(&T) -> &str,
) -> Result<(), ProductLifecycleVerificationInputError> {
    if inputs.len() != release.product_lifecycle_release.assets.len() {
        return Err(
            ProductLifecycleVerificationInputError::MissingReleaseAsset {
                asset_id: "release asset set".to_owned(),
            },
        );
    }
    let mut input_ids = BTreeSet::new();
    for input in inputs {
        let input_id = id(input);
        required_id(input_id, "assets.asset_id")?;
        if !input_ids.insert(input_id.0.as_str()) {
            return Err(ProductLifecycleVerificationInputError::DuplicateAsset {
                asset_id: input_id.0.clone(),
            });
        }
        validate_path(source_path(input), "assets.asset_source_path")?;
        validate_digest(expected_sha256(input), "assets.expected_sha256")?;
        let Some(release_asset) = release
            .product_lifecycle_release
            .assets
            .iter()
            .find(|asset| asset.asset_id == *input_id)
        else {
            return Err(
                ProductLifecycleVerificationInputError::MissingReleaseAsset {
                    asset_id: input_id.0.clone(),
                },
            );
        };
        if source_path(input) != &release_asset.source_path {
            return Err(
                ProductLifecycleVerificationInputError::ReleaseAssetMismatch {
                    asset_id: input_id.0.clone(),
                    field: "asset_source_path".to_owned(),
                },
            );
        }
        if expected_sha256(input) != release_asset.sha256.as_str() {
            return Err(
                ProductLifecycleVerificationInputError::ReleaseAssetMismatch {
                    asset_id: input_id.0.clone(),
                    field: "expected_sha256".to_owned(),
                },
            );
        }
    }
    Ok(())
}

fn validate_trusted_asset(
    asset: &ProductLifecycleAssetTrustedVerificationInput,
    release: &ProductLifecycleReleaseDocument,
) -> Result<(), ProductLifecycleVerificationInputError> {
    let mut evidence_paths = BTreeSet::new();
    evidence_paths.insert(asset.asset_source_path.0.clone());
    let provenance = &asset.detached_provenance;
    for (path, value) in [
        (
            "detached_provenance.provenance_path",
            &provenance.provenance_path,
        ),
        (
            "detached_provenance.signature_path",
            &provenance.signature_path,
        ),
        (
            "detached_provenance.public_key_path",
            &provenance.public_key_path,
        ),
        (
            "detached_provenance.transparency_log_path",
            &provenance.transparency_log_path,
        ),
    ] {
        validate_unique_evidence_path(value, path, &mut evidence_paths)?;
    }
    for (path, value) in [
        (
            "detached_provenance.expected_builder_id",
            &provenance.expected_builder_id,
        ),
        (
            "detached_provenance.expected_source_uri",
            &provenance.expected_source_uri,
        ),
        (
            "detached_provenance.expected_source_ref",
            &provenance.expected_source_ref,
        ),
    ] {
        required_text(value, path)?;
    }
    if provenance.expected_source_ref != release.product_lifecycle_release.source_ref {
        return Err(
            ProductLifecycleVerificationInputError::ReleaseAssetMismatch {
                asset_id: asset.asset_id.0.clone(),
                field: "detached_provenance.expected_source_ref".to_owned(),
            },
        );
    }

    let sigstore = &asset.sigstore;
    for (path, value) in [
        ("sigstore.bundle_path", &sigstore.bundle_path),
        ("sigstore.trust_policy_path", &sigstore.trust_policy_path),
        ("sigstore.certificate_path", &sigstore.certificate_path),
        (
            "sigstore.rekor_log_entry_path",
            &sigstore.rekor_log_entry_path,
        ),
        (
            "sigstore.rekor_public_key_path",
            &sigstore.rekor_public_key_path,
        ),
    ] {
        validate_unique_evidence_path(value, path, &mut evidence_paths)?;
    }
    if sigstore.issuer_certificate_paths.is_empty()
        || sigstore.issuer_certificate_paths.len() > MAX_PRODUCT_LIFECYCLE_ISSUER_CERTIFICATES
    {
        return Err(ProductLifecycleVerificationInputError::IssuerCertificateLimitExceeded);
    }
    for path in &sigstore.issuer_certificate_paths {
        validate_unique_evidence_path(
            path,
            "sigstore.issuer_certificate_paths",
            &mut evidence_paths,
        )?;
    }
    for (path, value) in [
        (
            "sigstore.expected_rekor_log_id",
            &sigstore.expected_rekor_log_id,
        ),
        (
            "sigstore.expected_predicate_type",
            &sigstore.expected_predicate_type,
        ),
    ] {
        required_text(value, path)?;
    }
    Ok(())
}

fn validate_unique_evidence_path(
    value: &RepoPath,
    field: &str,
    paths: &mut BTreeSet<String>,
) -> Result<(), ProductLifecycleVerificationInputError> {
    validate_path(value, field)?;
    if paths.insert(value.0.clone()) {
        Ok(())
    } else {
        Err(
            ProductLifecycleVerificationInputError::DuplicateEvidencePath {
                path: value.0.clone(),
            },
        )
    }
}

fn validate_schema(
    actual: &str,
    expected: &str,
) -> Result<(), ProductLifecycleVerificationInputError> {
    if actual == expected {
        Ok(())
    } else {
        Err(
            ProductLifecycleVerificationInputError::UnsupportedSchemaVersion {
                found: actual.to_owned(),
            },
        )
    }
}

fn required_id(value: &StableId, path: &str) -> Result<(), ProductLifecycleVerificationInputError> {
    if value.0.trim().is_empty() {
        Err(ProductLifecycleVerificationInputError::InvalidIdentifier {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn required_text(value: &str, path: &str) -> Result<(), ProductLifecycleVerificationInputError> {
    if value.trim().is_empty() || value.bytes().any(|byte| byte.is_ascii_control()) {
        Err(ProductLifecycleVerificationInputError::RequiredField {
            path: path.to_owned(),
        })
    } else {
        Ok(())
    }
}

fn validate_path(
    value: &RepoPath,
    path: &str,
) -> Result<(), ProductLifecycleVerificationInputError> {
    if value.0.is_empty() || value.0.len() > 512 || value.0.contains('\\') {
        return Err(ProductLifecycleVerificationInputError::InvalidPath {
            path: path.to_owned(),
        });
    }
    let parsed = Path::new(&value.0);
    if parsed.is_absolute()
        || !parsed
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(ProductLifecycleVerificationInputError::InvalidPath {
            path: path.to_owned(),
        });
    }
    Ok(())
}

fn validate_digest(value: &str, path: &str) -> Result<(), ProductLifecycleVerificationInputError> {
    if value.len() == 71
        && value.starts_with("sha256:")
        && value["sha256:".len()..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        Ok(())
    } else {
        Err(ProductLifecycleVerificationInputError::InvalidDigest {
            path: path.to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        ProductLifecycleAsset, ProductLifecycleAssetKind, ProductLifecycleChange,
        ProductLifecycleChangeKind, ProductLifecycleChannel, ProductLifecycleRelease,
        PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION,
    };

    fn path(value: &str) -> RepoPath {
        RepoPath(value.to_owned())
    }

    fn release() -> ProductLifecycleReleaseDocument {
        ProductLifecycleReleaseDocument {
            schema_version: PRODUCT_LIFECYCLE_RELEASE_SCHEMA_VERSION.to_owned(),
            product_lifecycle_release: ProductLifecycleRelease {
                release_id: StableId("release.fixture".to_owned()),
                version: "0.12.0".to_owned(),
                compatible_core_version: "0.12.0".to_owned(),
                channel: ProductLifecycleChannel::Stable,
                source_ref:
                    "git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567"
                        .to_owned(),
                provenance_ref: "provenance.json".to_owned(),
                signature_ref: Some("provenance.sig".to_owned()),
                rollback_ref: "prior".to_owned(),
                changes: vec![ProductLifecycleChange {
                    change_id: StableId("change.fixture".to_owned()),
                    kind: ProductLifecycleChangeKind::Security,
                    summary: "Fixture.".to_owned(),
                }],
                assets: vec![ProductLifecycleAsset {
                    asset_id: StableId("asset.fixture".to_owned()),
                    kind: ProductLifecycleAssetKind::CoreBinary,
                    source_path: path("assets/forge-core"),
                    install_path: path("bin/forge-core"),
                    sha256: format!("sha256:{}", "a".repeat(64)),
                    executable: true,
                    host: None,
                }],
            },
        }
    }

    fn trusted_input() -> ProductLifecycleTrustedVerificationInputDocument {
        ProductLifecycleTrustedVerificationInputDocument {
            schema_version: PRODUCT_LIFECYCLE_TRUSTED_VERIFICATION_SCHEMA_VERSION.to_owned(),
            product_lifecycle_trusted_verification_input: ProductLifecycleTrustedVerificationInput {
                release_id: StableId("release.fixture".to_owned()),
                assets: vec![ProductLifecycleAssetTrustedVerificationInput {
                    asset_id: StableId("asset.fixture".to_owned()),
                    asset_source_path: path("assets/forge-core"),
                    expected_sha256: format!("sha256:{}", "a".repeat(64)),
                    detached_provenance: ProductLifecycleDetachedProvenanceInput {
                        provenance_path: path("evidence/provenance.json"),
                        signature_path: path("evidence/provenance.sig"),
                        public_key_path: path("evidence/provenance.pub"),
                        transparency_log_path: path("evidence/transparency.json"),
                        expected_builder_id: "https://builder.example.invalid/workflow".to_owned(),
                        expected_source_uri: "https://example.invalid/forge".to_owned(),
                        expected_source_ref: "git+https://example.invalid/forge@0123456789abcdef0123456789abcdef01234567".to_owned(),
                    },
                    sigstore: ProductLifecycleSigstoreSubjectInput {
                        bundle_path: path("evidence/bundle.json"),
                        trust_policy_path: path("evidence/trust-policy.yaml"),
                        certificate_path: path("evidence/certificate.der"),
                        issuer_certificate_paths: vec![path("evidence/issuer.der")],
                        rekor_log_entry_path: path("evidence/rekor.json"),
                        rekor_public_key_path: path("evidence/rekor.pub"),
                        expected_rekor_log_id: "log.fixture".to_owned(),
                        expected_predicate_type: "https://slsa.dev/provenance/v1".to_owned(),
                    },
                }],
            },
        }
    }

    #[test]
    fn trusted_input_binds_all_cryptographic_rerun_material_to_exact_asset() {
        assert!(trusted_input().validate_for_release(&release()).is_ok());
    }

    #[test]
    fn trusted_input_rejects_asset_digest_or_release_subject_drift() {
        let release = release();
        let mut input = trusted_input();
        input.product_lifecycle_trusted_verification_input.assets[0].expected_sha256 =
            format!("sha256:{}", "b".repeat(64));
        assert!(matches!(
            input.validate_for_release(&release),
            Err(ProductLifecycleVerificationInputError::ReleaseAssetMismatch { field, .. })
                if field == "expected_sha256"
        ));

        let mut input = trusted_input();
        input.product_lifecycle_trusted_verification_input.assets[0]
            .detached_provenance
            .expected_source_ref = "git+https://example.invalid/forge@other".to_owned();
        assert!(matches!(
            input.validate_for_release(&release),
            Err(ProductLifecycleVerificationInputError::ReleaseAssetMismatch { field, .. })
                if field == "detached_provenance.expected_source_ref"
        ));
    }

    #[test]
    fn checksum_input_stays_separate_from_trusted_verification() {
        let checksum = ProductLifecycleChecksumAdmissionDocument {
            schema_version: PRODUCT_LIFECYCLE_CHECKSUM_ADMISSION_SCHEMA_VERSION.to_owned(),
            product_lifecycle_checksum_admission: ProductLifecycleChecksumAdmission {
                release_id: StableId("release.fixture".to_owned()),
                assets: vec![ProductLifecycleChecksumAssetInput {
                    asset_id: StableId("asset.fixture".to_owned()),
                    asset_source_path: path("assets/forge-core"),
                    expected_sha256: format!("sha256:{}", "a".repeat(64)),
                }],
            },
        };
        assert!(checksum.validate_for_release(&release()).is_ok());
        let raw = r#"
schema_version: "0.1"
product_lifecycle_checksum_admission:
  release_id: release.fixture
  assets: []
  verification_result: passed
"#;
        assert!(yaml_serde::from_str::<ProductLifecycleChecksumAdmissionDocument>(raw).is_err());
    }
}
