//! Explicit native actions for a discovered project result file.
use std::{
    fs::{self, OpenOptions},
    io::{self, Write},
    path::{Path, PathBuf},
    process::Command,
};
use tauri_plugin_dialog::DialogExt;

fn resolve_project_file(root: &Path, file: &Path) -> Result<PathBuf, &'static str> {
    if !root.is_absolute() || !file.is_absolute() {
        return Err("O caminho do arquivo do projeto é inválido.");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "A pasta do projeto não está mais disponível.")?;
    let file = file
        .canonicalize()
        .map_err(|_| "O arquivo do projeto não está mais disponível.")?;
    let metadata = fs::metadata(&file).map_err(|_| "Não foi possível conferir o arquivo.")?;
    if !file.starts_with(&root) || !metadata.is_file() {
        return Err("Escolha um arquivo existente dentro do projeto.");
    }
    Ok(file)
}

#[cfg(windows)]
fn explorer_selection_argument(file: &Path) -> Result<std::ffi::OsString, &'static str> {
    let path = file
        .to_str()
        .ok_or("O caminho do arquivo não pode ser exibido no Explorador.")?;
    let ordinary_path = if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!(r"\\{unc}")
    } else if let Some(drive_path) = path.strip_prefix(r"\\?\") {
        drive_path.to_owned()
    } else {
        path.to_owned()
    };
    let mut argument = std::ffi::OsString::from("/select,");
    argument.push(ordinary_path);
    Ok(argument)
}

#[tauri::command]
pub async fn reveal_project_file(
    project_root: String,
    file_path: String,
) -> Result<(), &'static str> {
    let project = crate::project::inspect_project(project_root).await?;
    let file = resolve_project_file(Path::new(&project.project_root), Path::new(&file_path))?;
    #[cfg(windows)]
    {
        let windir = std::env::var_os("WINDIR")
            .map(PathBuf::from)
            .filter(|path| path.is_absolute())
            .ok_or("Não foi possível localizar o Explorador de Arquivos.")?;
        let explorer = windir.join("explorer.exe");
        if !explorer.is_file() {
            return Err("Não foi possível localizar o Explorador de Arquivos.");
        }
        let argument = explorer_selection_argument(&file)?;
        Command::new(explorer)
            .arg(argument)
            .creation_flags(0x08000000)
            .spawn()
            .map_err(|_| "Não foi possível mostrar o arquivo no Explorador de Arquivos.")?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("Esta ação só está disponível no Windows.")
    }
}

fn validate_destination(path: &Path, source: &Path) -> Result<PathBuf, &'static str> {
    if !path.is_absolute() {
        return Err("Escolha um destino absoluto para a cópia.");
    }
    let parent = path
        .parent()
        .ok_or("O destino escolhido não é válido.")?
        .canonicalize()
        .map_err(|_| "A pasta de destino não está disponível.")?;
    if !parent.is_dir() {
        return Err("A pasta de destino não está disponível.");
    }
    let name = path
        .file_name()
        .ok_or("O destino escolhido não é válido.")?;
    let destination = parent.join(name);
    if destination == source || fs::symlink_metadata(&destination).is_ok() {
        return Err(
            "O destino já existe ou corresponde ao arquivo original; nada foi substituído.",
        );
    }
    Ok(destination)
}

fn copy_new_file(source: &Path, destination: &Path) -> Result<(), &'static str> {
    let mut input = fs::File::open(source)
        .map_err(|_| "Não foi possível ler o arquivo original; ele não foi alterado.")?;
    let mut output = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|_| "Não foi possível criar a cópia; o arquivo original não foi alterado.")?;
    io::copy(&mut input, &mut output).map_err(|_| {
        "A cópia pode estar incompleta no destino; o arquivo original não foi alterado."
    })?;
    output.flush().map_err(|_| {
        "A cópia pode estar incompleta no destino; o arquivo original não foi alterado."
    })
}

#[tauri::command]
pub async fn save_project_file_copy(
    window: tauri::WebviewWindow,
    project_root: String,
    file_path: String,
) -> Result<Option<String>, &'static str> {
    let project = crate::project::inspect_project(project_root).await?;
    let source = resolve_project_file(Path::new(&project.project_root), Path::new(&file_path))?;
    let basename = source
        .file_name()
        .ok_or("O arquivo do projeto não tem um nome válido.")?
        .to_string_lossy()
        .into_owned();
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_title("Salvar uma cópia deste arquivo")
        .set_file_name(basename)
        .set_parent(&window)
        .save_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível abrir a seleção do destino.")?;
    let Some(selection) = selection else {
        return Ok(None);
    };
    let destination = selection
        .into_path()
        .map_err(|_| "O destino escolhido não tem um caminho local válido.")?;
    // The project may have changed while the native dialog was open.
    let project = crate::project::inspect_project(project.project_root).await?;
    let source = resolve_project_file(Path::new(&project.project_root), Path::new(&file_path))?;
    let destination = validate_destination(&destination, &source)?;
    let source_for_copy = source.clone();
    let destination_for_copy = destination.clone();
    tokio::task::spawn_blocking(move || copy_new_file(&source_for_copy, &destination_for_copy))
        .await
        .map_err(|_| {
            "A cópia pode estar incompleta no destino; o arquivo original não foi alterado."
        })??;
    destination
        .into_os_string()
        .into_string()
        .map(Some)
        .map_err(|_| "A cópia foi salva, mas o caminho não pode ser exibido como texto.")
}

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("forge-result-actions-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn resolves_only_existing_regular_files_inside_canonical_root() {
        let fixture = Fixture::new();
        let root = fixture.0.join("project");
        fs::create_dir_all(&root).unwrap();
        let file = root.join("page.html");
        fs::write(&file, b"hello").unwrap();
        assert_eq!(
            resolve_project_file(&root, &file).unwrap(),
            file.canonicalize().unwrap()
        );
        assert!(resolve_project_file(&root, &root.join("missing")).is_err());
        assert!(resolve_project_file(&root, &root).is_err());
        let outside = fixture.0.join("outside.txt");
        fs::write(&outside, b"outside").unwrap();
        assert!(resolve_project_file(&root, &outside).is_err());
        assert!(resolve_project_file(Path::new("relative"), &file).is_err());
    }

    #[test]
    fn copies_exact_bytes_without_changing_original_and_refuses_existing_or_same_path() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source.bin");
        let bytes = [0, 1, 2, 255, 0, 128];
        fs::write(&source, bytes).unwrap();
        let source = source.canonicalize().unwrap();
        let destination = fixture.0.join("copy.bin");
        let destination = validate_destination(&destination, &source).unwrap();
        copy_new_file(&source, &destination).unwrap();
        assert_eq!(fs::read(&destination).unwrap(), bytes);
        assert_eq!(fs::read(&source).unwrap(), bytes);
        assert!(validate_destination(&destination, &source).is_err());
        assert!(validate_destination(&source, &source).is_err());
        fs::write(fixture.0.join("existing"), b"keep").unwrap();
        let existing = fixture.0.join("existing");
        assert!(copy_new_file(&source, &existing).is_err());
        assert_eq!(fs::read(existing).unwrap(), b"keep");
    }

    #[test]
    fn failed_destination_does_not_modify_source_or_make_directories() {
        let fixture = Fixture::new();
        let source = fixture.0.join("source");
        fs::write(&source, b"original").unwrap();
        let source = source.canonicalize().unwrap();
        let missing_parent = fixture.0.join("missing").join("copy");
        assert!(validate_destination(&missing_parent, &source).is_err());
        assert!(!missing_parent.parent().unwrap().exists());
        assert!(copy_new_file(&source, &fixture.0).is_err());
        assert_eq!(fs::read(source).unwrap(), b"original");
    }

    #[cfg(windows)]
    #[test]
    fn explorer_selection_argument_uses_ordinary_unicode_path_as_one_argument() {
        let path = Path::new(r"\\?\C:\Users\Pessoa\Meu projeto\página.html");
        assert_eq!(
            explorer_selection_argument(path).unwrap(),
            std::ffi::OsString::from(r"/select,C:\Users\Pessoa\Meu projeto\página.html")
        );
        let unc = Path::new(r"\\?\UNC\server\share\Meu projeto\página.html");
        assert_eq!(
            explorer_selection_argument(unc).unwrap(),
            std::ffi::OsString::from(r"/select,\\server\share\Meu projeto\página.html")
        );
    }
}
