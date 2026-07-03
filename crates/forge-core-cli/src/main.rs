use forge_core_cli::cli_error::ExitError;
use forge_core_cli::command_registry;
use forge_core_cli::tracing_init;
use std::env;

fn main() {
    // Initialize structured tracing BEFORE any dispatcher runs. The subscriber
    // is idempotent and writes to stderr; stdout (the JSON contract channel)
    // is untouched. Format is auto-selected from FORGE_LOG_FORMAT or stderr
    // TTY detection.
    tracing_init::init_subscriber();

    let args: Vec<String> = env::args().skip(1).collect();
    // No args → print help and exit 0. The previous default ran `validate`
    // silently, which surprised users who just wanted to see what the binary
    // does. `validate` is still the implicit subject of `--help`, but it must
    // be requested explicitly (either by name or by running it in a repo).
    if args.is_empty() {
        println!("{}", command_registry::global_usage());
        return;
    }
    let command = args.first().map_or("validate", String::as_str);

    // Root session span. When FORGE_AGENT_ID is set, every nested span carries
    // `agent_id`, which lets a trace store correlate multi-agent runs (host +
    // sub-agents, N parallel workers, etc.) without parsing argv. When unset,
    // the field stays Empty so a human-driven run is distinguishable from a
    // future agent that has not yet identified itself.
    let agent_id = tracing_init::current_agent_id();
    let session_span = tracing::info_span!(
        "forge_session",
        agent_id = tracing::field::Empty,
        command = %command,
    );
    if let Some(id) = &agent_id {
        session_span.record("agent_id", id.as_str());
    }

    // Dispatchers are sync; run them inside the session span so every nested
    // `#[instrument]` inherits `agent_id` + `command` automatically.
    let result: Result<(), ExitError> =
        session_span.in_scope(|| command_registry::dispatch(command, &args));

    // The single std::process::exit call in the entire forge-core-cli crate.
    // Every dispatcher returns Result<(), ExitError>; this block converts the
    // typed error back into the shell exit code and stderr text.
    match result {
        Ok(()) => {}
        Err(error) => {
            // The dispatcher already wrote any stdout/stderr it needed (JSON
            // envelope or text-mode failure line). The ExitError's own message
            // is non-empty only for direct usage / parse failures, where the
            // dispatcher did NOT print anything itself.
            let message = error.message();
            if !message.is_empty() {
                eprintln!("{message}");
            }
            std::process::exit(error.exit_code());
        }
    }
}
