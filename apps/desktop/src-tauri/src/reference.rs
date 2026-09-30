//! Explicit user selection of a local reference. No reading, copying or upload.
use std::path::Path;
use tauri_plugin_dialog::DialogExt;

fn reference_path(path: &Path) -> Result<String, &'static str> {
    let path = path
        .canonicalize()
        .map_err(|_| "Este arquivo não está mais disponível.")?;
    if !path.is_file() {
        return Err("Escolha um arquivo, não uma pasta.");
    }
    let path = path
        .to_str()
        .filter(|path| path.len() <= 4096 && !path.chars().any(char::is_control))
        .ok_or("O caminho deste arquivo não pode ser usado na mensagem.")?;
    if let Some(rest) = path.strip_prefix(r"\\?\UNC\") {
        Ok(format!(r"\\{rest}"))
    } else {
        Ok(path.strip_prefix(r"\\?\").unwrap_or(path).to_owned())
    }
}

#[tauri::command]
pub async fn choose_reference_file(
    window: tauri::WebviewWindow,
) -> Result<Option<String>, &'static str> {
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_title("Escolha um arquivo como referência")
        .set_parent(&window)
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível abrir a seleção de arquivos.")?;
    selection
        .map(|selection| {
            let path = selection
                .into_path()
                .map_err(|_| "Escolha um arquivo deste computador.")?;
            reference_path(&path)
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_existing_regular_files_are_referenced_without_mutation() {
        let root = std::env::temp_dir().join(format!("forge-reference-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("referência.txt");
        std::fs::write(&file, "unchanged").unwrap();
        assert!(reference_path(&file).unwrap().ends_with("referência.txt"));
        assert!(reference_path(&root).is_err());
        assert!(reference_path(&root.join("missing")).is_err());
        assert_eq!(std::fs::read_to_string(&file).unwrap(), "unchanged");
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(root).unwrap();
    }
}
