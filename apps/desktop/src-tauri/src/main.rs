#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod codex_transport;
mod history;
mod progress;
mod project;
use tauri::Manager;

#[derive(serde::Serialize)]
struct AppInfo {
    name: &'static str,
    version: &'static str,
}

/// App identity only: this does not report agent or project readiness.
#[tauri::command]
fn app_info() -> AppInfo {
    AppInfo {
        name: "Forge",
        version: env!("CARGO_PKG_VERSION"),
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(agent::AgentState::default())
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                if !agent::begin_close(&window.state::<agent::AgentState>()) {
                    return;
                }
                let window = window.clone();
                tauri::async_runtime::spawn(async move {
                    agent::shutdown(&window.state::<agent::AgentState>()).await;
                    let _ = window.destroy();
                });
            }
        })
        .invoke_handler(tauri::generate_handler![
            app_info,
            project::choose_project_folder,
            project::inspect_project,
            progress::inspect_progress,
            agent::connect_agent,
            agent::send_message,
            agent::interrupt_agent,
            agent::disconnect_agent
        ])
        .run(tauri::generate_context!())
        .expect("failed to run the Forge desktop application");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_identity_reports_the_app_not_an_agent() {
        let info = app_info();
        assert_eq!(info.name, "Forge");
        assert!(!info.version.is_empty());
    }
}
