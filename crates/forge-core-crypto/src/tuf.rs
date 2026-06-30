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
//! Only [`verify_tuf_metadata_freshness_role`] is `pub(crate)` and
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
