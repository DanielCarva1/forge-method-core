//! Unit tests for the generic event-log mechanics, using a tiny dummy domain
//! (`Counter`) that exercises every code path: replay, the out-of-order guard,
//! cold-read `project_locked` (including a torn final line and a missing log),
//! `next_sequence`, `now_unix`, and `append_event`.

#![allow(clippy::cast_possible_truncation)]

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::{
    append_event, apply_event, event_envelope, next_sequence, now_unix, project_locked, replay,
    resolve_lock_path, EventLogError, EventLogLock, EventSourced, WalDurability,
};

// ---------------------------------------------------------------------------
// Dummy domain: a counter that increments and resets. Minimal but exercises
// every EventSourced associated type and method.
// ---------------------------------------------------------------------------

/// A dummy projection-diagnostic: `(code, message)` pair. Real domains use a
/// struct with a severity enum; this is the smallest thing that satisfies
/// `Clone + PartialEq + Eq + Debug`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct CounterDiagnostic {
    code: &'static str,
    message: String,
}

/// The dummy projection: a running total per label plus the sequence watermark.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
struct CounterProjection {
    sequence: u64,
    totals: BTreeMap<String, i64>,
    diagnostics: Vec<CounterDiagnostic>,
}

/// The dummy event. `Reset` carries no extra field beyond the envelope, which
/// stresses the `..` ignoring in the macro.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
enum CounterEvent {
    Incremented {
        sequence: u64,
        at_unix: u64,
        label: String,
        delta: i64,
    },
    Reset {
        sequence: u64,
        at_unix: u64,
    },
}

event_envelope!(CounterEvent, [Incremented, Reset]);

/// The dummy `EventSourced` impl.
struct CounterDomain;

impl EventSourced for CounterDomain {
    type Event = CounterEvent;
    type Projection = CounterProjection;
    type Diagnostic = CounterDiagnostic;

    fn apply(projection: &mut Self::Projection, event: &Self::Event) {
        match event {
            CounterEvent::Incremented { label, delta, .. } => {
                *projection.totals.entry(label.clone()).or_insert(0) += delta;
            }
            CounterEvent::Reset { .. } => {
                projection.totals.clear();
            }
        }
    }

    fn record_diagnostic(projection: &mut Self::Projection, diagnostic: Self::Diagnostic) {
        projection.diagnostics.push(diagnostic);
    }

    fn sequence_of(projection: &Self::Projection) -> u64 {
        projection.sequence
    }

    fn advance_sequence(projection: &mut Self::Projection, new_sequence: u64) {
        projection.sequence = new_sequence;
    }

    fn diagnostic_out_of_order_event_ignored(
        event_seq: u64,
        projection_seq: u64,
    ) -> Self::Diagnostic {
        CounterDiagnostic {
            code: crate::CODE_OUT_OF_ORDER_EVENT_IGNORED,
            message: format!(
                "event sequence {event_seq} <= projection sequence {projection_seq}; ignored"
            ),
        }
    }

    fn diagnostic_torn_final_line_skipped(
        line_number: usize,
        source: &serde_json::Error,
    ) -> Self::Diagnostic {
        CounterDiagnostic {
            code: crate::CODE_TORN_FINAL_LINE_SKIPPED,
            message: format!("skipped incomplete final line {line_number}: {source}"),
        }
    }
}

// ---------------------------------------------------------------------------
// Temp-dir helper (repo convention: no `tempfile` workspace dep).
// ---------------------------------------------------------------------------

fn temp_root(label: &str) -> PathBuf {
    let pid = std::process::id();
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    let path = std::env::temp_dir().join(format!("forge-eventlog-{label}-{pid}-{nanos}"));
    fs::create_dir_all(&path).expect("create temp root");
    path
}

const LOG_REL: &str = "counter/events.ndjson";
const LOCK_REL: &str = "locks/counter.log.lock";

fn inc(seq: u64, label: &str, delta: i64) -> CounterEvent {
    CounterEvent::Incremented {
        sequence: seq,
        at_unix: 1_700_000_000 + seq,
        label: label.into(),
        delta,
    }
}

fn reset(seq: u64) -> CounterEvent {
    CounterEvent::Reset {
        sequence: seq,
        at_unix: 1_700_000_000 + seq,
    }
}

// ---------------------------------------------------------------------------
// replay + out-of-order guard
// ---------------------------------------------------------------------------

#[test]
fn replay_empty_yields_default_projection() {
    let projection: CounterProjection = replay::<CounterDomain>([]);
    assert_eq!(projection.sequence, 0);
    assert!(projection.totals.is_empty());
    assert!(projection.diagnostics.is_empty());
}

#[test]
fn replay_folds_events_and_advances_sequence() {
    let events = [inc(1, "a", 5), inc(2, "a", 3), inc(3, "b", 10)];
    let projection = replay::<CounterDomain>(events);
    assert_eq!(projection.sequence, 3);
    assert_eq!(projection.totals["a"], 8);
    assert_eq!(projection.totals["b"], 10);
    assert!(projection.diagnostics.is_empty());
}

#[test]
fn replay_reset_clears_totals() {
    let events = [inc(1, "a", 5), reset(2)];
    let projection = replay::<CounterDomain>(events);
    assert_eq!(projection.sequence, 2);
    assert!(projection.totals.is_empty());
}

#[test]
fn apply_event_ignores_out_of_order_without_regressing() {
    // Apply seq=5, then a stray seq=2 must NOT regress state or insert.
    let mut projection = replay::<CounterDomain>([inc(5, "a", 5)]);
    assert_eq!(projection.sequence, 5);
    assert_eq!(projection.totals["a"], 5);

    apply_event::<CounterDomain>(&mut projection, &inc(2, "b", 99));

    assert_eq!(projection.sequence, 5, "sequence must not regress");
    assert!(
        !projection.totals.contains_key("b"),
        "stale event must not apply"
    );
    assert_eq!(projection.diagnostics.len(), 1);
    assert_eq!(
        projection.diagnostics[0].code,
        crate::CODE_OUT_OF_ORDER_EVENT_IGNORED
    );
}

#[test]
fn apply_event_allows_equal_sequence_on_empty_projection() {
    // sequence 0 is the empty-projection watermark; an event with sequence 0
    // is `<= 0` but the `current > 0` guard means the empty case applies it
    // (matches the copied PEPs: only a *non-empty* projection guards).
    let mut projection = CounterProjection::default();
    apply_event::<CounterDomain>(&mut projection, &inc(1, "a", 7));
    assert_eq!(projection.sequence, 1);
    assert_eq!(projection.totals["a"], 7);
    assert!(projection.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// next_sequence + now_unix
// ---------------------------------------------------------------------------

#[test]
fn next_sequence_starts_at_one_and_saturates() {
    let empty = CounterProjection::default();
    assert_eq!(next_sequence::<CounterDomain>(&empty), 1);

    let high = CounterProjection {
        sequence: u64::MAX,
        ..CounterProjection::default()
    };
    assert_eq!(
        next_sequence::<CounterDomain>(&high),
        u64::MAX,
        "saturating add must not overflow"
    );
}

#[test]
fn now_unix_is_reasonable() {
    let t = now_unix();
    // 2024-01-01 ≈ 1_704_067_200. Just sanity-check it's a recent-ish epoch
    // second and not zero (clock failures return 0, which we tolerate but
    // shouldn't see on a test host).
    assert!(
        t >= 1_700_000_000,
        "now_unix returned {t}, expected a 2024+ epoch second"
    );
}

// ---------------------------------------------------------------------------
// project_locked: missing log, clean log, torn tail
// ---------------------------------------------------------------------------

#[test]
fn project_locked_missing_log_yields_empty_projection() {
    let root = temp_root("missing");
    let projection =
        project_locked::<CounterDomain>(&root, LOG_REL).expect("missing log is not an error");
    assert_eq!(projection.sequence, 0);
    assert!(projection.totals.is_empty());
}

#[test]
fn project_locked_reads_clean_log() {
    let root = temp_root("clean");
    let log_dir = root.join("counter");
    fs::create_dir_all(&log_dir).unwrap();
    let log_path = root.join(LOG_REL);
    let line1 = serde_json::to_string(&inc(1, "a", 5)).unwrap();
    let line2 = serde_json::to_string(&inc(2, "b", 10)).unwrap();
    fs::write(&log_path, format!("{line1}\n{line2}\n")).unwrap();

    let projection = project_locked::<CounterDomain>(&root, LOG_REL).expect("clean read");
    assert_eq!(projection.sequence, 2);
    assert_eq!(projection.totals["a"], 5);
    assert_eq!(projection.totals["b"], 10);
    assert!(projection.diagnostics.is_empty(), "no torn line expected");
}

#[test]
fn project_locked_tolerates_torn_final_line() {
    let root = temp_root("torn");
    let log_dir = root.join("counter");
    fs::create_dir_all(&log_dir).unwrap();
    let log_path = root.join(LOG_REL);
    let good1 = serde_json::to_string(&inc(1, "a", 5)).unwrap();
    let good2 = serde_json::to_string(&inc(2, "a", 3)).unwrap();
    // Two good lines (monotonic seq 1, 2), then a torn (truncated-JSON) final
    // line missing its closing `}`. The torn line must be skipped with a
    // diagnostic, and the two good lines must apply cleanly.
    fs::write(&log_path, format!("{good1}\n{good2}\n{{\"sequence\":3")).unwrap();

    let projection =
        project_locked::<CounterDomain>(&root, LOG_REL).expect("torn tail is tolerated");
    assert_eq!(projection.sequence, 2, "two good lines applied");
    assert_eq!(projection.totals["a"], 8, "5 + 3");
    assert_eq!(
        projection.diagnostics.len(),
        1,
        "exactly one torn-line diagnostic"
    );
    assert_eq!(
        projection.diagnostics[0].code,
        crate::CODE_TORN_FINAL_LINE_SKIPPED
    );
}

#[test]
fn project_locked_hard_fails_on_schema_drift_mid_log() {
    // A non-final line that parses as JSON but not as CounterEvent is schema
    // drift — a hard Parse error, not a torn-line skip.
    let root = temp_root("drift");
    let log_dir = root.join("counter");
    fs::create_dir_all(&log_dir).unwrap();
    let log_path = root.join(LOG_REL);
    let good = serde_json::to_string(&inc(1, "a", 5)).unwrap();
    // Well-formed JSON object, but no `sequence`/`at_unix`/`label`/`delta` —
    // deserializing into CounterEvent must fail.
    let drift = r#"{"totally_unknown_field":42}"#;
    fs::write(&log_path, format!("{good}\n{drift}\n")).unwrap();

    let result = project_locked::<CounterDomain>(&root, LOG_REL);
    match result {
        Err(EventLogError::Parse { line_number, .. }) => {
            assert_eq!(line_number, 2, "the schema-drift line is line 2");
        }
        other => panic!("expected Parse error on line 2, got {other:?}"),
    }
}

#[test]
fn project_locked_skips_blank_lines() {
    let root = temp_root("blanks");
    let log_dir = root.join("counter");
    fs::create_dir_all(&log_dir).unwrap();
    let log_path = root.join(LOG_REL);
    let good = serde_json::to_string(&inc(1, "a", 5)).unwrap();
    // Blank/whitespace lines interspersed must be skipped, not error.
    fs::write(&log_path, format!("\n{good}\n   \n")).unwrap();

    let projection = project_locked::<CounterDomain>(&root, LOG_REL).expect("blanks skipped");
    assert_eq!(projection.sequence, 1);
    assert_eq!(projection.totals["a"], 5);
    assert!(projection.diagnostics.is_empty());
}

// ---------------------------------------------------------------------------
// EventLogLock + append_event round-trip
// ---------------------------------------------------------------------------

#[test]
fn lock_acquires_and_drop_releases() {
    let root = temp_root("lock-cycle");
    let lock1 = EventLogLock::acquire::<CounterDiagnostic>(&root, LOCK_REL).expect("first acquire");
    assert!(lock1.path().ends_with(LOCK_REL));

    // After dropping, we can acquire again (the fs4 advisory lock is released
    // by EffectStoreLock's Drop). We cannot easily assert "would block" here
    // without a second process, but re-acquire succeeding proves release.
    drop(lock1);
    let _lock2 =
        EventLogLock::acquire::<CounterDiagnostic>(&root, LOCK_REL).expect("re-acquire after drop");
}

#[test]
fn append_event_writes_and_is_readable_by_project_locked() {
    let root = temp_root("append-read");
    let lock = EventLogLock::acquire::<CounterDiagnostic>(&root, LOCK_REL).expect("acquire");

    let e1 = inc(1, "a", 5);
    let e2 = inc(2, "a", 3);
    // NoSync: this is a test.
    let _path = append_event::<CounterDomain>(&root, LOG_REL, &e1, WalDurability::NoSync, &lock)
        .expect("append e1");
    append_event::<CounterDomain>(&root, LOG_REL, &e2, WalDurability::NoSync, &lock)
        .expect("append e2");

    // Release the lock before reading (project_locked doesn't need it, but we
    // must not hold two fs4 locks on overlapping paths in one process).
    drop(lock);

    let projection = project_locked::<CounterDomain>(&root, LOG_REL).expect("read back");
    assert_eq!(projection.sequence, 2);
    assert_eq!(projection.totals["a"], 8);
}

#[test]
fn replay_is_deterministic() {
    // The Fowler replay-determinism guarantee: same stream ⇒ same projection.
    let stream = [inc(1, "a", 5), inc(2, "b", 10), reset(3), inc(4, "c", 1)];
    let first = replay::<CounterDomain>(stream.clone());
    let second = replay::<CounterDomain>(stream);
    assert_eq!(first, second, "replay must be deterministic");
    assert_eq!(first.sequence, 4);
}

// ---------------------------------------------------------------------------
// resolve_lock_path: the only pub fn with no direct test.
// ---------------------------------------------------------------------------

#[test]
fn resolve_lock_path_joins_root_and_relative() {
    let path = resolve_lock_path(std::path::Path::new("/state/root"), LOCK_REL);
    assert_eq!(
        path,
        PathBuf::from("/state/root").join(LOCK_REL),
        "resolve_lock_path must be a plain root.join(relative)"
    );
    assert!(
        path.ends_with(LOCK_REL),
        "the relative component must be preserved verbatim"
    );
}

// ---------------------------------------------------------------------------
// Out-of-order guard: the == boundary on a non-empty projection.
// ---------------------------------------------------------------------------

#[test]
fn apply_event_ignores_equal_sequence_on_non_empty_projection() {
    // The guard is `event_seq <= current && current > 0`. seq=3 applied, then a
    // second seq=3 must be ignored (the `<=` half of the guard, with `current >
    // 0`). This pins the equality boundary that the existing tests left
    // implicit: `apply_event_ignores_out_of_order_without_regressing` only
    // exercises seq < current, and `..._allows_equal_sequence_on_empty_projection`
    // only the empty case.
    let mut projection = replay::<CounterDomain>([inc(3, "a", 5)]);
    assert_eq!(projection.sequence, 3);

    // A duplicate seq=3 for a different label: must NOT apply (guard rejects
    // `==` on a non-empty projection) and must NOT regress the watermark.
    apply_event::<CounterDomain>(&mut projection, &inc(3, "b", 99));

    assert_eq!(
        projection.sequence, 3,
        "duplicate sequence must not advance"
    );
    assert!(
        !projection.totals.contains_key("b"),
        "duplicate-sequence event must not apply"
    );
    assert_eq!(
        projection.diagnostics.len(),
        1,
        "the duplicate must record exactly one out-of-order diagnostic"
    );
    assert_eq!(
        projection.diagnostics[0].code,
        crate::CODE_OUT_OF_ORDER_EVENT_IGNORED
    );
}

// ---------------------------------------------------------------------------
// project_locked: the Read error path (a non-NotFound I/O failure).
// ---------------------------------------------------------------------------

#[test]
fn project_locked_read_errors_when_log_path_is_a_directory() {
    // The only NotFound-excluded read failure we can induce deterministically
    // and cross-platform: make the log path itself a directory. `fs::read` then
    // fails with `IsADirectory` (Unix) / `PermissionDenied` (Windows) — either
    // way a non-NotFound error that exercises the `Read` arm.
    let root = temp_root("read-is-dir");
    let log_path = root.join(LOG_REL);
    fs::create_dir_all(&log_path).expect("create the log path AS a directory");

    let result = project_locked::<CounterDomain>(&root, LOG_REL);
    match result {
        Err(EventLogError::Read { path, .. }) => {
            assert_eq!(path, log_path, "the Read error must carry the log path");
        }
        other => panic!("expected Read error (log is a dir), got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// append_event: the Append error path (store helper fails to write).
// ---------------------------------------------------------------------------

#[test]
fn append_event_fails_when_a_path_component_is_a_file() {
    // Induce a deterministic Append failure by making an ANCESTOR of the log
    // file a regular file: the store must create `counter/events.ndjson`, but
    // `counter` is a file, so descending into it fails (NotADirectory on Unix,
    // an equivalent on Windows). This exercises the `Append` arm without a
    // second process or platform-specific permission games.
    let root = temp_root("append-notdir");
    // Create `counter` as a plain file (the store would otherwise mkdir it).
    fs::write(root.join("counter"), b"blocker").expect("seed file ancestor");

    let lock = EventLogLock::acquire::<CounterDiagnostic>(&root, LOCK_REL).expect("acquire");
    let result = append_event::<CounterDomain>(
        &root,
        LOG_REL,
        &inc(1, "a", 1),
        WalDurability::NoSync,
        &lock,
    );
    match result {
        Err(EventLogError::Append { .. }) => {}
        other => panic!("expected Append error, got {other:?}"),
    }
}

// ---------------------------------------------------------------------------
// EventLogError::Display: every variant renders a non-empty message.
// The Display impl is hand-rolled; a regression here would only surface in
// operator-facing log lines, so we pin the format.
// ---------------------------------------------------------------------------

#[test]
fn eventlog_error_display_renders_every_variant() {
    // Pin the hand-rolled Display format for each variant. These messages are
    // what operators see in logs; a regression here only surfaces at runtime.
    let lock = EventLogError::<CounterDiagnostic>::Lock {
        path: PathBuf::from("/x/lock"),
        source: "held".into(),
    };
    assert!(
        lock.to_string()
            .contains("acquire event-log lock at /x/lock"),
        "Lock Display: {lock}"
    );

    let append = EventLogError::<CounterDiagnostic>::Append {
        path: PathBuf::from("/x/log"),
        source: "io".into(),
    };
    assert!(
        append.to_string().contains("append event to /x/log"),
        "Append Display: {append}"
    );

    let serialize = EventLogError::<CounterDiagnostic>::Serialize {
        source: "json".into(),
    };
    assert!(
        serialize.to_string().contains("serialize event failed"),
        "Serialize Display: {serialize}"
    );

    let read = EventLogError::<CounterDiagnostic>::Read {
        path: PathBuf::from("/x/log"),
        source: "io".into(),
    };
    assert!(
        read.to_string().contains("read event log at /x/log"),
        "Read Display: {read}"
    );

    let parse = EventLogError::<CounterDiagnostic>::Parse {
        path: PathBuf::from("/x/log"),
        line_number: 7,
        source: "shape".into(),
    };
    let parse_msg = parse.to_string();
    assert!(
        parse_msg.contains("parse event at /x/log:7"),
        "Parse Display must include path:line, got: {parse_msg}"
    );

    let diag = EventLogError::ProjectionDiagnostic(CounterDiagnostic {
        code: "torn_final_line_skipped",
        message: "skipped".into(),
    });
    assert!(
        diag.to_string().contains("projection diagnostic"),
        "ProjectionDiagnostic Display: {diag}"
    );
}
