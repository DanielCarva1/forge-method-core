#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod project;

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
        .invoke_handler(tauri::generate_handler![app_info, project::inspect_project])
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
