use forge_core_store::{
    append_trace_event, query_trace_events, TraceEventQuery, TraceEventQueryReason,
    TraceEventQueryStatus,
};
use forge_core_trace::{TraceEvent, TraceEventKind};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

static TEMP_ROOT_COUNTER: AtomicU64 = AtomicU64::new(0);

fn fresh_temp_root(label: &str) -> PathBuf {
    let counter = TEMP_ROOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    let timestamp_nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before UNIX_EPOCH")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "forge-core-store-trace-{label}-{}-{timestamp_nanos}-{counter}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&path);
    fs::create_dir_all(&path).expect("create temp state root");
    path
}

fn event(run_id: &str, event_id: &str, kind: TraceEventKind) -> TraceEvent {
    TraceEvent::new(
        format!("trace.{run_id}"),
        run_id,
        event_id,
        kind,
        "2026-06-28T00:00:00Z",
        "test event",
    )
}

#[test]
fn appends_trace_event_under_state_root() {
    let state_root = fresh_temp_root("append");
    let event = event("run.one", "evt.0001", TraceEventKind::RunStarted);

    let written = append_trace_event(&state_root, &event).expect("append trace event");

    assert!(written.ends_with("traces/events.ndjson"));
    let text = fs::read_to_string(written).expect("read trace log");
    assert!(text.contains("\"kind\":\"trace_event\""));
    assert!(text.contains("\"run_id\":\"run.one\""));
    assert!(state_root.join("locks").join("append-json-line").exists());
    assert!(!state_root.join(".forge-method").join("locks").exists());
}

#[test]
fn queries_latest_run_from_trace_log() {
    let state_root = fresh_temp_root("query");
    append_trace_event(
        &state_root,
        &event("run.one", "evt.0001", TraceEventKind::RunStarted),
    )
    .expect("append first run");
    append_trace_event(
        &state_root,
        &event("run.two", "evt.0002", TraceEventKind::RunStarted),
    )
    .expect("append second run start");
    append_trace_event(
        &state_root,
        &event("run.two", "evt.0003", TraceEventKind::RunCompleted),
    )
    .expect("append second run completion");

    let result = query_trace_events(
        &state_root,
        &TraceEventQuery {
            latest_run: true,
            ..TraceEventQuery::default()
        },
    );

    assert_eq!(result.status, TraceEventQueryStatus::Matched);
    assert_eq!(result.scanned_events, 3);
    assert_eq!(result.returned_events, 2);
    assert!(result.events.iter().all(|event| event.run_id == "run.two"));
    assert_eq!(result.reasons, vec![TraceEventQueryReason::Matched]);
}

#[test]
fn missing_trace_log_is_noop() {
    let state_root = fresh_temp_root("missing");

    let result = query_trace_events(&state_root, &TraceEventQuery::default());

    assert_eq!(result.status, TraceEventQueryStatus::Noop);
    assert_eq!(result.returned_events, 0);
    assert_eq!(result.reasons, vec![TraceEventQueryReason::NoTraceFile]);
}
