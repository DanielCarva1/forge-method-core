#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod agent;
mod codex_transport;
mod history;
mod preview;
mod preview_site;
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

#[tauri::command]
fn open_updates_page() -> Result<(), &'static str> {
    // No URL from the WebView is accepted here: only the project's public releases page.
    #[cfg(windows)]
    {
        #[link(name = "shell32")]
        unsafe extern "system" {
            fn ShellExecuteW(
                window: isize,
                operation: *const u16,
                file: *const u16,
                parameters: *const u16,
                directory: *const u16,
                show: i32,
            ) -> isize;
        }
        let operation: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        let url: Vec<u16> = "https://github.com/DanielCarva1/forge-method-core/releases"
            .encode_utf16()
            .chain(std::iter::once(0))
            .collect();
        let result = unsafe {
            ShellExecuteW(0, operation.as_ptr(), url.as_ptr(), std::ptr::null(), std::ptr::null(), 1)
        };
        if result <= 32 {
            Err("Não foi possível abrir o navegador. Copie o endereço mostrado na tela.")
        } else {
            Ok(())
        }
    }
    #[cfg(not(windows))]
    { Err("Abra o endereço mostrado na tela em seu navegador.") }
}

fn main() {
    let preview_site = std::sync::Arc::new(preview_site::PreviewSiteState::default());
    let preview_site_protocol = preview_site.clone();
    tauri::Builder::default()
        .register_uri_scheme_protocol("forgepreview", move |_context, request| {
            preview_site::serve(&preview_site_protocol, request)
        })
        .plugin(tauri_plugin_dialog::init())
        .manage(agent::AgentState::default())
        .manage(preview_site)
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
            open_updates_page,
            project::choose_project_folder,
            project::inspect_project,
            project::start_project,
            progress::inspect_progress,
            progress::inspect_direction_history,
            preview::choose_preview_file,
            preview::inspect_preview,
            preview::open_site_in_browser,
            preview::open_pdf_in_browser,
            preview::clear_preview_site,
            agent::connect_agent,
            agent::start_login,
            agent::finish_login,
            agent::cancel_login,
            agent::open_login_page,
            agent::list_conversations,
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
