//! TUF (The Update Framework) metadata freshness and UTC datetime helpers.
//!
//! This module owns the helpers that load a TUF role metadata document
//! (root / timestamp / snapshot / targets), parse the `signed` envelope,
//! verify role type, version floor, and expiry freshness, and push a
//! structured [`HostAdapterTufMetadataFreshnessRole`] entry describing the
//! observed metadata state.
//!
//! The datetime helpers (`parse_tuf_datetime_utc_to_unix`, `parse_fixed_i32`,
//! `days_in_month`, `is_leap_year`, `days_from_civil`) implement a pure,
//! dependency-free RFC 3339 UTC parser (`YYYY-MM-DDTHH:MM:SSZ`) backed by
//! Howard Hinnant's `days_from_civil` algorithm. They are consumed only by
//! the expiry freshness check and therefore stay private to this module.
//!
//! The public entrypoint that consumes these helpers is
//! [`crate::run_host_adapter_tuf_trusted_root_freshness_verification`],
//! which stays in `lib.rs` as part of the host-adapter verification domain.
//!
//! ## Visibility
//!
//! Only `verify_tuf_metadata_freshness_role` is `pub(crate)` and
//! re-exported at the crate root via `pub(crate) use`. The datetime helpers
//! are consumed exclusively inside this module and remain private.

use std::path::Path;

use serde_json::Value;

use crate::file_io::read_required_file;
use crate::host_adapter_types::HostAdapterTufMetadataFreshnessRole;

/// Load a TUF role metadata document, verify role type, version floor, and
/// expiry freshness against `update_start_time_unix`, and push a structured
/// [`HostAdapterTufMetadataFreshnessRole`] entry describing the result.
///
/// Every failure path still pushes a partial entry (with the fields it could
/// observe) so the caller can report the full state of every role it
/// attempted to verify, matching the accumulating-validation contract.
pub(crate) fn verify_tuf_metadata_freshness_role(
    expected_role: &str,
    metadata_path: &Path,
    min_version: Option<i64>,
    update_start_time_unix: i64,
    verified_roles: &mut Vec<HostAdapterTufMetadataFreshnessRole>,
    verified_evidence: &mut Vec<String>,
    reasons: &mut Vec<String>,
) {
    let metadata_path_string = metadata_path.to_string_lossy().to_string();
    let Some(bytes) = read_required_file(metadata_path, "tuf_metadata", reasons) else {
        verified_roles.push(HostAdapterTufMetadataFreshnessRole {
            role: expected_role.to_string(),
            metadata_path: metadata_path_string,
            version: None,
            min_version,
            expires: None,
            expires_unix: None,
        });
        return;
    };
    verified_evidence.push(format!("tuf_{expected_role}_metadata_loaded"));

    let value = match serde_json::from_slice::<Value>(&bytes) {
        Ok(value) => value,
        Err(err) => {
            reasons.push(format!("tuf_{expected_role}_metadata_json_invalid:{err}"));
            verified_roles.push(HostAdapterTufMetadataFreshnessRole {
                role: expected_role.to_string(),
                metadata_path: metadata_path_string,
                version: None,
                min_version,
                expires: None,
                expires_unix: None,
            });
            return;
        }
    };

    let signed = value.get("signed").and_then(Value::as_object);
    let observed_role = signed
        .and_then(|signed| signed.get("_type"))
        .and_then(Value::as_str);
    let version = signed
        .and_then(|signed| signed.get("version"))
        .and_then(Value::as_i64);
    let expires = signed
        .and_then(|signed| signed.get("expires"))
        .and_then(Value::as_str)
        .map(str::to_string);
    let expires_unix = expires
        .as_deref()
        .and_then(|value| parse_tuf_datetime_utc_to_unix(value, expected_role, reasons));

    if observed_role == Some(expected_role) {
        verified_evidence.push(format!("tuf_{expected_role}_role_type_matches"));
    } else {
        reasons.push(format!("tuf_{expected_role}_role_type_mismatch"));
    }

    match (version, min_version) {
        (Some(observed), Some(minimum)) if observed >= minimum => {
            verified_evidence.push(format!("tuf_{expected_role}_version_at_or_above_floor"));
        }
        (Some(_), Some(_)) => reasons.push(format!("tuf_{expected_role}_version_below_floor")),
        (Some(_), None) => verified_evidence.push(format!("tuf_{expected_role}_version_present")),
        (None, _) => reasons.push(format!("tuf_{expected_role}_version_missing")),
    }

    if let Some(expires_unix) = expires_unix {
        if expires_unix > update_start_time_unix {
            verified_evidence.push(format!("tuf_{expected_role}_expires_after_update_start"));
        } else {
            reasons.push(format!("tuf_{expected_role}_metadata_expired"));
        }
    } else if expires.is_none() {
        reasons.push(format!("tuf_{expected_role}_expires_missing"));
    }

    verified_roles.push(HostAdapterTufMetadataFreshnessRole {
        role: expected_role.to_string(),
        metadata_path: metadata_path_string,
        version,
        min_version,
        expires,
        expires_unix,
    });
}

/// Parse a TUF metadata `expires` field formatted as
/// `YYYY-MM-DDTHH:MM:SSZ` (RFC 3339 UTC, fixed-width) into a Unix epoch
/// second timestamp. Returns `None` and pushes a diagnostic reason on any
/// structural or calendar-range violation.
fn parse_tuf_datetime_utc_to_unix(
    value: &str,
    role: &str,
    reasons: &mut Vec<String>,
) -> Option<i64> {
    if value.len() != 20 || !value.ends_with('Z') {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    if value.as_bytes().get(4) != Some(&b'-')
        || value.as_bytes().get(7) != Some(&b'-')
        || value.as_bytes().get(10) != Some(&b'T')
        || value.as_bytes().get(13) != Some(&b':')
        || value.as_bytes().get(16) != Some(&b':')
    {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    let (Some(year), Some(month), Some(day), Some(hour), Some(minute), Some(second)) = (
        parse_fixed_i32(value, 0, 4),
        parse_fixed_i32(value, 5, 7),
        parse_fixed_i32(value, 8, 10),
        parse_fixed_i32(value, 11, 13),
        parse_fixed_i32(value, 14, 16),
        parse_fixed_i32(value, 17, 19),
    ) else {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    };
    if !(1..=12).contains(&month)
        || !(1..=days_in_month(year, month)).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=59).contains(&second)
    {
        reasons.push(format!("tuf_{role}_expires_format_invalid"));
        return None;
    }
    let days = days_from_civil(year, month, day);
    Some(days * 86_400 + i64::from(hour * 3_600 + minute * 60 + second))
}

/// Parse a fixed-width ASCII integer slice `[start, end)` as `i32`.
fn parse_fixed_i32(value: &str, start: usize, end: usize) -> Option<i32> {
    value.get(start..end)?.parse::<i32>().ok()
}

/// Return the number of days in the given `(year, month)`, honoring leap
/// years for February.
fn days_in_month(year: i32, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if is_leap_year(year) => 29,
        2 => 28,
        _ => 0,
    }
}

/// Proleptic Gregorian leap-year predicate.
fn is_leap_year(year: i32) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Howard Hinnant's `days_from_civil` algorithm: convert a proleptic
/// Gregorian `(year, month, day)` into a count of days since the Unix epoch
/// (1970-01-01). Produces negative values for dates before the epoch.
fn days_from_civil(year: i32, month: i32, day: i32) -> i64 {
    let year = year - i32::from(month <= 2);
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let day_of_year = (153 * month + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    i64::from(era * 146_097 + day_of_era - 719_468)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_adapter_types::HostAdapterTufMetadataFreshnessRole;
    use serde_json::json;
    use std::fs;
    use std::path::{Path, PathBuf};

    // ---- fixture helpers -------------------------------------------------

    /// Write a TUF role metadata document to a temp file and return its path.
    /// The caller owns the parent dir lifetime via `dir`.
    fn write_metadata(dir: &Path, role: &str, signed_body: serde_json::Value) -> PathBuf {
        let path = dir.join(format!("{role}.json"));
        let doc = json!({
            "signatures": [],
            "signed": signed_body,
        });
        fs::write(&path, serde_json::to_vec(&doc).expect("serialize metadata"))
            .expect("write metadata file");
        path
    }

    /// Build a minimal `signed` body for a TUF role.
    fn signed_body(role: &str, version: i64, expires: &str) -> serde_json::Value {
        json!({
            "_type": role,
            "spec_version": "1.0.0",
            "version": version,
            "expires": expires,
        })
    }

    /// Sentinel-bearing temp dir that deletes itself on drop.
    struct ScopedTempDir(PathBuf);
    impl ScopedTempDir {
        fn new(label: &str) -> Self {
            let mut path = std::env::temp_dir();
            path.push(format!("forge-tuf-test-{}-{}", label, std::process::id(),));
            fs::create_dir_all(&path).expect("create temp dir");
            ScopedTempDir(path)
        }
        fn path(&self) -> &Path {
            &self.0
        }
    }
    impl Drop for ScopedTempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    // ---- verify_tuf_metadata_freshness_role: fresh metadata --------------

    #[test]
    fn verify_role_pushes_evidence_when_metadata_is_fresh() {
        let dir = ScopedTempDir::new("fresh");
        let path = write_metadata(
            dir.path(),
            "root",
            signed_body("root", 3, "2030-01-01T00:00:00Z"),
        );
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles.len(), 1);
        let role = &roles[0];
        assert_eq!(role.role, "root");
        assert_eq!(role.version, Some(3));
        assert_eq!(role.min_version, Some(3));
        assert_eq!(role.expires.as_deref(), Some("2030-01-01T00:00:00Z"));
        // 2030-01-01T00:00:00Z = 1893456000.
        assert_eq!(role.expires_unix, Some(1_893_456_000));
        assert!(reasons.is_empty(), "reasons should be empty: {reasons:?}");
        assert!(evidence.contains(&"tuf_root_metadata_loaded".to_string()));
        assert!(evidence.contains(&"tuf_root_role_type_matches".to_string()));
        assert!(evidence.contains(&"tuf_root_version_at_or_above_floor".to_string()));
        assert!(evidence.contains(&"tuf_root_expires_after_update_start".to_string()));
    }

    #[test]
    fn verify_role_marks_expired_when_expires_before_update_start() {
        let dir = ScopedTempDir::new("expired");
        let path = write_metadata(
            dir.path(),
            "root",
            signed_body("root", 1, "2020-01-01T00:00:00Z"),
        );
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(1),
            // 2020-01-01 = 1577836800; one second after expiry.
            1_577_836_801,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles.len(), 1);
        // 2020-01-01T00:00:00Z = 1577836800.
        assert_eq!(roles[0].expires_unix, Some(1_577_836_800));
        assert!(reasons.contains(&"tuf_root_metadata_expired".to_string()));
        assert!(!evidence.contains(&"tuf_root_expires_after_update_start".to_string()));
    }

    #[test]
    fn verify_role_reports_version_rollback_below_floor() {
        let dir = ScopedTempDir::new("rollback");
        let path = write_metadata(
            dir.path(),
            "root",
            signed_body("root", 2, "2030-01-01T00:00:00Z"),
        );
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(5),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles[0].version, Some(2));
        assert!(reasons.contains(&"tuf_root_version_below_floor".to_string()));
        assert!(!evidence.contains(&"tuf_root_version_at_or_above_floor".to_string()));
    }

    #[test]
    fn verify_role_reports_version_missing_when_signed_lacks_version() {
        let dir = ScopedTempDir::new("no-version");
        // Body without `version` field.
        let body = json!({"_type": "root", "expires": "2030-01-01T00:00:00Z"});
        let path = write_metadata(dir.path(), "root", body);
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(5),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles[0].version, None);
        assert!(reasons.contains(&"tuf_root_version_missing".to_string()));
    }

    #[test]
    fn verify_role_marks_version_present_when_no_floor_configured() {
        let dir = ScopedTempDir::new("no-floor");
        let path = write_metadata(
            dir.path(),
            "root",
            signed_body("root", 1, "2030-01-01T00:00:00Z"),
        );
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            None,
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert!(evidence.contains(&"tuf_root_version_present".to_string()));
        assert!(!reasons.iter().any(|r| r.contains("version_below_floor")));
    }

    #[test]
    fn verify_role_reports_role_type_mismatch() {
        let dir = ScopedTempDir::new("role-mismatch");
        // Document claims to be "timestamp" but we ask for "root".
        let path = write_metadata(
            dir.path(),
            "root",
            signed_body("timestamp", 3, "2030-01-01T00:00:00Z"),
        );
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert!(reasons.contains(&"tuf_root_role_type_mismatch".to_string()));
        assert!(!evidence.contains(&"tuf_root_role_type_matches".to_string()));
    }

    #[test]
    fn verify_role_handles_expires_missing() {
        let dir = ScopedTempDir::new("no-expires");
        let body = json!({"_type": "root", "version": 1});
        let path = write_metadata(dir.path(), "root", body);
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            None,
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles[0].expires, None);
        assert_eq!(roles[0].expires_unix, None);
        assert!(reasons.contains(&"tuf_root_expires_missing".to_string()));
    }

    #[test]
    fn verify_role_reports_expires_format_invalid_pushing_partial_entry() {
        let dir = ScopedTempDir::new("bad-expiry");
        let path = write_metadata(dir.path(), "root", signed_body("root", 3, "not-a-date"));
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        // Expires string is observed but unix parse failed.
        assert_eq!(roles[0].expires.as_deref(), Some("not-a-date"));
        assert_eq!(roles[0].expires_unix, None);
        assert!(reasons.contains(&"tuf_root_expires_format_invalid".to_string()));
    }

    #[test]
    fn verify_role_reports_read_failure_and_pushes_partial_entry() {
        let dir = ScopedTempDir::new("read-fail");
        let missing = dir.path().join("does-not-exist.json");
        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &missing,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        // File missing → partial entry pushed, no evidence, reason recorded.
        assert_eq!(roles.len(), 1);
        let role: &HostAdapterTufMetadataFreshnessRole = &roles[0];
        assert_eq!(role.version, None);
        assert_eq!(role.expires, None);
        // `read_required_file` uses the literal label `tuf_metadata`.
        assert!(reasons
            .iter()
            .any(|r| r.starts_with("tuf_metadata_read_failed")));
        assert!(evidence.is_empty());
    }

    #[test]
    fn verify_role_reports_json_invalid() {
        let dir = ScopedTempDir::new("bad-json");
        let path = dir.path().join("root.json");
        fs::write(&path, b"this is { not valid json").expect("write bad json");

        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        assert_eq!(roles.len(), 1);
        assert_eq!(roles[0].version, None);
        // metadata_loaded is pushed only on a successful read, which happened,
        // so evidence has the load marker but nothing further.
        assert!(evidence.contains(&"tuf_root_metadata_loaded".to_string()));
        assert!(reasons
            .iter()
            .any(|r| r.starts_with("tuf_root_metadata_json_invalid")));
    }

    #[test]
    fn verify_role_no_signed_envelope_reports_all_missing() {
        let dir = ScopedTempDir::new("no-signed");
        // Top-level object without `signed`.
        let path = dir.path().join("root.json");
        fs::write(
            &path,
            serde_json::to_vec(&json!({"signatures": []})).expect("serialize"),
        )
        .expect("write");

        let mut roles = Vec::new();
        let mut evidence = Vec::new();
        let mut reasons = Vec::new();

        verify_tuf_metadata_freshness_role(
            "root",
            &path,
            Some(3),
            1_000_000_000,
            &mut roles,
            &mut evidence,
            &mut reasons,
        );

        // _type absent → mismatch; version absent → missing; expires absent → missing.
        assert!(reasons.contains(&"tuf_root_role_type_mismatch".to_string()));
        assert!(reasons.contains(&"tuf_root_version_missing".to_string()));
        assert!(reasons.contains(&"tuf_root_expires_missing".to_string()));
        assert_eq!(roles[0].version, None);
    }

    // ---- parse_tuf_datetime_utc_to_unix: happy path + KAT ----------------

    #[test]
    fn parse_datetime_returns_epoch_zero_for_1970() {
        let mut reasons = Vec::new();
        let unix = parse_tuf_datetime_utc_to_unix("1970-01-01T00:00:00Z", "root", &mut reasons);
        assert_eq!(unix, Some(0));
        assert!(reasons.is_empty());
    }

    #[test]
    fn parse_datetime_returns_known_value_for_2030_root() {
        // 2030-01-01T00:00:00Z = 1893456000 (canonical root metadata date
        // used by the E2E suite).
        let mut reasons = Vec::new();
        let unix = parse_tuf_datetime_utc_to_unix("2030-01-01T00:00:00Z", "root", &mut reasons);
        assert_eq!(unix, Some(1_893_456_000));
        assert!(reasons.is_empty());
    }

    #[test]
    fn parse_datetime_encodes_time_of_day() {
        // 2020-01-01T12:30:45Z = 1577881845.
        let mut reasons = Vec::new();
        let unix = parse_tuf_datetime_utc_to_unix("2020-01-01T12:30:45Z", "root", &mut reasons);
        assert_eq!(unix, Some(1_577_881_845));
        assert!(reasons.is_empty());
    }

    #[test]
    fn parse_datetime_returns_negative_for_pre_epoch() {
        // 1969-12-31T23:59:59Z = -1.
        let mut reasons = Vec::new();
        let unix = parse_tuf_datetime_utc_to_unix("1969-12-31T23:59:59Z", "root", &mut reasons);
        assert_eq!(unix, Some(-1));
        assert!(reasons.is_empty());
    }

    // ---- parse_tuf_datetime_utc_to_unix: format rejection ----------------

    #[test]
    fn parse_datetime_rejects_wrong_length() {
        let mut reasons = Vec::new();
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00:00:00", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00:00:00ZZ", "root", &mut reasons),
            None
        );
        assert!(reasons
            .iter()
            .all(|r| r == "tuf_root_expires_format_invalid"));
        assert_eq!(
            reasons.len(),
            2,
            "each malformed input must push exactly one reason"
        );
    }

    #[test]
    fn parse_datetime_rejects_missing_z_suffix() {
        let mut reasons = Vec::new();
        // Same length but wrong terminator.
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00:00:00+", "root", &mut reasons),
            None
        );
        assert!(reasons.contains(&"tuf_root_expires_format_invalid".to_string()));
    }

    #[test]
    fn parse_datetime_rejects_wrong_separators() {
        let mut reasons = Vec::new();
        // Length 20, ends with Z, but wrong field separators.
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030/01/01T00:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01 00:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00-00-00Z", "root", &mut reasons),
            None
        );
        assert_eq!(reasons.len(), 3);
    }

    #[test]
    fn parse_datetime_rejects_non_numeric_fields() {
        let mut reasons = Vec::new();
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("20XX-01-01T00:00:00Z", "root", &mut reasons),
            None
        );
        assert!(reasons.contains(&"tuf_root_expires_format_invalid".to_string()));
    }

    // ---- parse_tuf_datetime_utc_to_unix: calendar range ------------------

    #[test]
    fn parse_datetime_rejects_month_zero_and_thirteen() {
        let mut reasons = Vec::new();
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-00-01T00:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-13-01T00:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(reasons.len(), 2);
    }

    #[test]
    fn parse_datetime_rejects_day_out_of_month_range() {
        let mut reasons = Vec::new();
        // April has 30 days.
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-04-31T00:00:00Z", "root", &mut reasons),
            None
        );
        // February (non-leap 2030) has 28 days.
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-02-29T00:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(reasons.len(), 2);
    }

    #[test]
    fn parse_datetime_accepts_feb_29_in_leap_year() {
        let mut reasons = Vec::new();
        // 2024 is a leap year; 2024-02-29T00:00:00Z = 1709164800 must parse.
        let unix = parse_tuf_datetime_utc_to_unix("2024-02-29T00:00:00Z", "root", &mut reasons);
        assert_eq!(unix, Some(1_709_164_800));
        assert!(reasons.is_empty());
    }

    #[test]
    fn parse_datetime_rejects_hour_minute_second_overflow() {
        let mut reasons = Vec::new();
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T24:00:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00:60:00Z", "root", &mut reasons),
            None
        );
        assert_eq!(
            parse_tuf_datetime_utc_to_unix("2030-01-01T00:00:60Z", "root", &mut reasons),
            None
        );
        assert_eq!(reasons.len(), 3);
    }

    #[test]
    fn parse_datetime_pushes_role_scoped_reason() {
        let mut reasons = Vec::new();
        // Role "timestamp" should appear in the reason, not "root".
        let _ = parse_tuf_datetime_utc_to_unix("bad", "timestamp", &mut reasons);
        assert!(reasons.contains(&"tuf_timestamp_expires_format_invalid".to_string()));
    }

    // ---- parse_fixed_i32 -------------------------------------------------

    #[test]
    fn parse_fixed_i32_reads_decimal_slice() {
        assert_eq!(parse_fixed_i32("2030", 0, 4), Some(2030));
        assert_eq!(parse_fixed_i32("2030-01", 5, 7), Some(1));
    }

    #[test]
    fn parse_fixed_i32_returns_none_on_non_digits() {
        assert_eq!(parse_fixed_i32("20ab", 0, 4), None);
    }

    #[test]
    fn parse_fixed_i32_returns_none_on_out_of_range() {
        // Slice beyond string length → None (via `str::get` returning None).
        assert_eq!(parse_fixed_i32("2030", 0, 10), None);
    }

    #[test]
    fn parse_fixed_i32_accepts_negative_when_in_range() {
        // i32 parse handles a leading minus.
        assert_eq!(parse_fixed_i32("-5", 0, 2), Some(-5));
    }

    // ---- days_in_month ---------------------------------------------------

    #[test]
    fn days_in_month_returns_31_for_long_months() {
        for m in [1, 3, 5, 7, 8, 10, 12] {
            assert_eq!(days_in_month(2023, m), 31, "month {m}");
        }
    }

    #[test]
    fn days_in_month_returns_30_for_short_months() {
        for m in [4, 6, 9, 11] {
            assert_eq!(days_in_month(2023, m), 30, "month {m}");
        }
    }

    #[test]
    fn days_in_month_returns_28_for_february_in_common_year() {
        assert_eq!(days_in_month(2023, 2), 28);
        // 1900 is divisible by 100 but not 400 → not a leap year.
        assert_eq!(days_in_month(1900, 2), 28);
    }

    #[test]
    fn days_in_month_returns_29_for_february_in_leap_year() {
        assert_eq!(days_in_month(2024, 2), 29);
        // 2000 is divisible by 400 → leap year.
        assert_eq!(days_in_month(2000, 2), 29);
    }

    #[test]
    fn days_in_month_returns_zero_for_invalid_month() {
        assert_eq!(days_in_month(2023, 0), 0);
        assert_eq!(days_in_month(2023, 13), 0);
    }

    // ---- is_leap_year ----------------------------------------------------

    #[test]
    fn is_leap_year_handles_common_divisible_by_four() {
        assert!(is_leap_year(2024));
        assert!(!is_leap_year(2023));
    }

    #[test]
    fn is_leap_year_rejects_century_not_divisible_by_400() {
        assert!(!is_leap_year(1900));
        assert!(!is_leap_year(2100));
    }

    #[test]
    fn is_leap_year_accepts_century_divisible_by_400() {
        assert!(is_leap_year(2000));
        assert!(is_leap_year(1600));
    }

    // ---- days_from_civil: KAT table --------------------------------------
    //
    // Reference values computed independently from the Unix epoch (Python
    // `datetime`), pinning the algorithm so a refactor cannot silently shift
    // the day count.

    #[test]
    fn days_from_civil_epoch_is_zero() {
        assert_eq!(days_from_civil(1970, 1, 1), 0);
    }

    #[test]
    fn days_from_civil_day_before_epoch_is_negative_one() {
        assert_eq!(days_from_civil(1969, 12, 31), -1);
    }

    #[test]
    fn days_from_civil_matches_reference_table() {
        // (year, month, day, expected_days_since_epoch)
        let cases: &[(i32, i32, i32, i64)] = &[
            (1970, 1, 1, 0),       // epoch
            (1969, 12, 31, -1),    // pre-epoch
            (1900, 1, 1, -25_567), // deep pre-epoch, non-leap century
            (2000, 1, 1, 10_957),  // Y2K leap-year boundary
            (2000, 2, 29, 11_016), // leap day of a 400-year leap year
            (2001, 2, 28, 11_381), // common-year February
            (2020, 1, 1, 18_262),
            (2024, 2, 29, 19_782), // leap day, divisible-by-4 leap year
            (2024, 3, 1, 19_783),  // day after leap day (month rollover)
            (2030, 1, 1, 21_915),  // canonical root-metadata date
        ];
        for &(y, m, d, expected) in cases {
            assert_eq!(
                days_from_civil(y, m, d),
                expected,
                "days_from_civil({y}, {m}, {d})"
            );
        }
    }

    #[test]
    fn days_from_civil_january_february_use_prior_year_era() {
        // The algorithm shifts Jan/Feb into the prior year; verify a January
        // date lands on the same day count as a December date roughly 1 year
        // earlier (366 days for the 2020 leap-year span).
        // 2020-01-01 → 18262; 2021-01-01 → 18628 (366 days later).
        assert_eq!(
            days_from_civil(2021, 1, 1) - days_from_civil(2020, 1, 1),
            366
        );
    }

    #[test]
    fn days_from_civil_round_trips_across_full_year_span() {
        // A non-leap year is 365 days.
        let start = days_from_civil(2022, 1, 1);
        let next = days_from_civil(2023, 1, 1);
        assert_eq!(next - start, 365);
    }
}
