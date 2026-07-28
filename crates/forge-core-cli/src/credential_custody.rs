//! Private-key custody shared by CLI credential adapters.
//!
//! Public registry policy stays with each credential domain. This Module owns
//! opaque key naming, private directory permissions, exclusive creation,
//! durable writes, and zeroized key reads.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use ed25519_dalek::SigningKey;
use sha2::{Digest, Sha256};

use crate::cli_error::ExitError;

pub(crate) fn create_private_dir(path: &Path) -> Result<(), ExitError> {
    std::fs::create_dir_all(path)
        .map_err(|error| ExitError::env_config(format!("create secret directory: {error}")))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700))
            .map_err(|error| ExitError::env_config(format!("protect secret directory: {error}")))?;
    }
    Ok(())
}

pub(crate) fn secret_path(directory: &Path, credential_id: &str) -> PathBuf {
    directory.join(format!("{}.ed25519", hex(&Sha256::digest(credential_id))))
}

pub(crate) fn write_secret_new(path: &Path, bytes: &[u8; 32]) -> Result<(), ExitError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|error| ExitError::env_config(format!("create private key: {error}")))?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|error| ExitError::env_config(format!("persist private key: {error}")))
}

pub(crate) fn read_signing_key(path: &Path) -> Result<SigningKey, ExitError> {
    let mut bytes = [0_u8; 32];
    let mut file = File::open(path)
        .map_err(|error| ExitError::env_config(format!("open private key: {error}")))?;
    file.read_exact(&mut bytes)
        .map_err(|error| ExitError::env_config(format!("read private key: {error}")))?;
    let key = SigningKey::from_bytes(&bytes);
    bytes.fill(0);
    Ok(key)
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to a String cannot fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn fixture_root(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "forge-credential-custody-{label}-{}-{nonce}",
            std::process::id()
        ))
    }

    #[test]
    fn secret_path_is_deterministic_and_opaque() {
        let directory = Path::new("/operator/secrets");
        let first = secret_path(directory, "credential-one");
        assert_eq!(first, secret_path(directory, "credential-one"));
        assert_eq!(
            first,
            directory
                .join("020f85bc4a5074beee0ff7599adc11829ded3eaa0acf3f59f5c944e0907a0332.ed25519")
        );
        assert_eq!(first.parent(), Some(directory));
        assert_eq!(
            first.extension().and_then(|value| value.to_str()),
            Some("ed25519")
        );
        assert!(!first.to_string_lossy().contains("credential-one"));
    }

    #[test]
    fn private_directory_and_secret_use_restrictive_permissions() {
        let root = fixture_root("permissions");
        create_private_dir(&root).expect("create protected directory");
        let path = secret_path(&root, "credential-one");
        write_secret_new(&path, &[7_u8; 32]).expect("create protected secret");

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&root)
                    .expect("directory metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o700
            );
            assert_eq!(
                std::fs::metadata(&path)
                    .expect("secret metadata")
                    .permissions()
                    .mode()
                    & 0o777,
                0o600
            );
        }

        std::fs::remove_dir_all(root).expect("cleanup custody fixture");
    }

    #[test]
    fn secret_creation_is_exclusive_and_read_round_trips() {
        let root = fixture_root("round-trip");
        create_private_dir(&root).expect("create protected directory");
        let path = secret_path(&root, "credential-one");
        let bytes = [11_u8; 32];
        write_secret_new(&path, &bytes).expect("create secret");
        assert_eq!(
            read_signing_key(&path)
                .expect("read signing key")
                .to_bytes(),
            bytes
        );

        assert!(
            write_secret_new(&path, &[12_u8; 32]).is_err(),
            "exclusive creation must reject an existing credential"
        );
        assert_eq!(
            std::fs::read(&path).expect("read retained secret"),
            bytes,
            "failed replacement must not modify the original secret"
        );

        std::fs::remove_dir_all(root).expect("cleanup custody fixture");
    }

    #[test]
    fn missing_and_truncated_secrets_fail_closed() {
        let root = fixture_root("failures");
        create_private_dir(&root).expect("create protected directory");
        let missing = root.join("missing.ed25519");
        assert!(read_signing_key(&missing).is_err());

        let truncated = root.join("truncated.ed25519");
        std::fs::write(&truncated, [5_u8; 31]).expect("write malformed secret");
        assert!(read_signing_key(&truncated).is_err());

        std::fs::remove_dir_all(root).expect("cleanup custody fixture");
    }
}
