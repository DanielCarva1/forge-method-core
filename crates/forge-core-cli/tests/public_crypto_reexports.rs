//! Downstream compatibility guard for the historical crypto facade.

use forge_core_cli::{
    hex_sha256, source_ref_is_immutable, valid_sha256_digest, HostAdapterCommandKind,
};

#[test]
fn historical_crypto_items_remain_available_from_cli_root() {
    assert_eq!(
        hex_sha256(b"forge"),
        "71b41d6dd48dc58eba8f5cf9edf30fef6597fdf285a521bb8fcbad4b3d50887d"
    );
    assert!(valid_sha256_digest(&format!(
        "sha256:{}",
        hex_sha256(b"forge")
    )));
    assert!(source_ref_is_immutable(
        "git:137b3cf43b123d4b15c45b544a3e3060e714ffb9"
    ));
    assert_eq!(
        HostAdapterCommandKind::Validation,
        forge_core_cli::host_adapter_types::HostAdapterCommandKind::Validation
    );

    // Binding the function item is a compile-time downstream API assertion.
    let _verification_entrypoint = forge_core_cli::run_host_adapter_artifact_verification;
}

#[test]
fn historical_crypto_modules_remain_available_beneath_cli() {
    assert_eq!(forge_core_cli::hashing::hex_bytes(&[0x0f, 0xa0]), "0fa0");
    assert!(forge_core_cli::version_like("0.12.0-alpha.3"));
    assert!(forge_core_cli::rekor::parse_rekor_log_entry("{}").is_err());

    // This public module path was exported by the former wildcard facade.
    let _ = std::any::TypeId::of::<forge_core_cli::host_adapter_types::HostAdapterManifest>();
}
