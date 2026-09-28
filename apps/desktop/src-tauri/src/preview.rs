//! Bounded local preview. Project authority is still resolved by Forge.
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::Serialize;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tauri_plugin_dialog::DialogExt;

const MAX_IMAGE_BYTES: u64 = 5 * 1024 * 1024;
const MAX_TEXT_BYTES: u64 = 32 * 1024;
const MAX_HTML_BYTES: u64 = 512 * 1024;

#[derive(Serialize)]
pub struct Preview {
    kind: &'static str,
    content: String,
    relative_path: String,
    size_bytes: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    render_url: Option<String>,
}

#[tauri::command]
pub async fn choose_preview_file(
    window: tauri::WebviewWindow,
    project_root: String,
) -> Result<Option<String>, &'static str> {
    // This is only the dialog's starting point. inspect_preview remains the
    // authority for Forge project identity and file containment.
    let directory = Path::new(&project_root)
        .canonicalize()
        .map_err(|_| "A pasta do projeto não está mais disponível.")?;
    if !directory.is_dir() {
        return Err("A pasta do projeto não está mais disponível.");
    }
    let (sender, receiver) = tokio::sync::oneshot::channel();
    window
        .dialog()
        .file()
        .set_title("Escolha um arquivo deste projeto")
        .set_directory(directory)
        .set_parent(&window)
        .pick_file(move |selection| {
            let _ = sender.send(selection);
        });
    let selection = receiver
        .await
        .map_err(|_| "Não foi possível abrir a seleção de arquivos.")?;
    selection
        .map(|path| {
            path.into_path()
                .map_err(|_| "O arquivo escolhido não tem um caminho local válido.")
                .and_then(|path| {
                    path.into_os_string()
                        .into_string()
                        .map_err(|_| "O arquivo escolhido não tem um caminho de texto válido.")
                })
        })
        .transpose()
}

fn image_mime(extension: &str, bytes: &[u8]) -> Option<&'static str> {
    match extension {
        "png" if bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Some("image/png"),
        "jpg" | "jpeg" if bytes.starts_with(b"\xff\xd8\xff") => Some("image/jpeg"),
        "gif" if bytes.starts_with(b"GIF87a") || bytes.starts_with(b"GIF89a") => Some("image/gif"),
        "webp" if bytes.len() >= 12 && bytes.starts_with(b"RIFF") && &bytes[8..12] == b"WEBP" => {
            Some("image/webp")
        }
        _ => None,
    }
}

fn read_preview(root: &Path, requested: &Path) -> Result<Preview, &'static str> {
    if !requested.is_absolute() {
        return Err("Escolha um arquivo deste projeto.");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta do projeto.")?;
    let file = requested
        .canonicalize()
        .map_err(|_| "Este arquivo não está mais disponível.")?;
    let relative = file
        .strip_prefix(&root)
        .map_err(|_| "Este arquivo não pertence ao projeto aberto.")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Não foi possível ler este arquivo.")?;
    if !metadata.is_file() {
        return Err("Escolha um arquivo comum deste projeto.");
    }
    let extension = file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let is_image = matches!(extension.as_str(), "png" | "jpg" | "jpeg" | "gif" | "webp");
    let is_text = matches!(
        extension.as_str(),
        "txt"
            | "md"
            | "json"
            | "csv"
            | "html"
            | "htm"
            | "css"
            | "js"
            | "mjs"
            | "ts"
            | "rs"
            | "py"
            | "yaml"
            | "yml"
            | "toml"
    );
    if !is_image && !is_text {
        return Ok(Preview {
            kind: "file",
            content: String::new(),
            relative_path: relative.to_string_lossy().into_owned(),
            size_bytes: metadata.len(),
            render_url: None,
        });
    }
    let limit = if is_image {
        MAX_IMAGE_BYTES
    } else if matches!(extension.as_str(), "html" | "htm") {
        MAX_HTML_BYTES
    } else {
        MAX_TEXT_BYTES
    };
    if metadata.len() > limit {
        return Err("Este arquivo é grande demais para a prévia. Ele não foi alterado.");
    }
    let bytes = std::fs::read(&file).map_err(|_| "Não foi possível ler este arquivo.")?;
    if bytes.len() as u64 > limit {
        return Err("Este arquivo é grande demais para a prévia. Ele não foi alterado.");
    }
    let (kind, content) = if is_image {
        let mime = image_mime(&extension, &bytes)
            .ok_or("A imagem não corresponde ao formato esperado.")?;
        (
            "image",
            format!("data:{mime};base64,{}", STANDARD.encode(&bytes)),
        )
    } else {
        (
            "text",
            String::from_utf8(bytes).map_err(|_| "Este texto não usa UTF-8.")?,
        )
    };
    Ok(Preview {
        kind,
        content,
        relative_path: relative.to_string_lossy().into_owned(),
        size_bytes: metadata.len(),
        render_url: None,
    })
}

fn local_html_path(root: &Path, requested: &Path) -> Result<PathBuf, &'static str> {
    if !requested.is_absolute() {
        return Err("Escolha uma página deste projeto.");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta do projeto.")?;
    let file = requested
        .canonicalize()
        .map_err(|_| "Esta página não está mais disponível.")?;
    file.strip_prefix(&root)
        .map_err(|_| "Esta página não pertence ao projeto aberto.")?;
    if !file.is_file()
        || !file.extension().and_then(|value| value.to_str()).is_some_and(|value| {
            value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm")
        })
    {
        return Err("Escolha uma página HTML deste projeto.");
    }
    Ok(file)
}

fn local_pdf_path(root: &Path, requested: &Path) -> Result<PathBuf, &'static str> {
    if !requested.is_absolute() {
        return Err("Escolha um PDF deste projeto.");
    }
    let root = root
        .canonicalize()
        .map_err(|_| "Não foi possível conferir a pasta do projeto.")?;
    let file = requested
        .canonicalize()
        .map_err(|_| "Este PDF não está mais disponível.")?;
    file.strip_prefix(&root)
        .map_err(|_| "Este PDF não pertence ao projeto aberto.")?;
    if !file.is_file()
        || !file
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value.eq_ignore_ascii_case("pdf"))
    {
        return Err("Escolha um PDF deste projeto.");
    }
    let mut header = [0_u8; 5];
    std::fs::File::open(&file)
        .and_then(|mut source| source.read_exact(&mut header))
        .map_err(|_| "Não foi possível conferir este PDF.")?;
    if &header != b"%PDF-" {
        return Err("Este arquivo não tem um cabeçalho PDF válido.");
    }
    Ok(file)
}

#[cfg(windows)]
fn default_browser_executable() -> Result<PathBuf, &'static str> {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;

    #[link(name = "shlwapi")]
    unsafe extern "system" {
        fn AssocQueryStringW(
            flags: u32,
            string_type: u32,
            association: *const u16,
            extra: *const u16,
            output: *mut u16,
            length: *mut u32,
        ) -> i32;
    }

    // The HTTPS protocol identifies the user's browser; the .html association
    // can instead point to an editor or another non-browser application.
    const ASSOCSTR_EXECUTABLE: u32 = 2;
    let association: Vec<u16> = "https".encode_utf16().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let mut length = 0;
    unsafe {
        AssocQueryStringW(
            0,
            ASSOCSTR_EXECUTABLE,
            association.as_ptr(),
            verb.as_ptr(),
            std::ptr::null_mut(),
            &mut length,
        );
    }
    if !(2..=32768).contains(&length) {
        return Err("Defina um navegador padrão no Windows para abrir este arquivo.");
    }
    let mut output = vec![0_u16; length as usize];
    let result = unsafe {
        AssocQueryStringW(
            0,
            ASSOCSTR_EXECUTABLE,
            association.as_ptr(),
            verb.as_ptr(),
            output.as_mut_ptr(),
            &mut length,
        )
    };
    if result != 0 {
        return Err("Não foi possível consultar o navegador padrão do Windows.");
    }
    let end = output
        .iter()
        .position(|&unit| unit == 0)
        .unwrap_or(output.len());
    if end == 0 {
        return Err("Não foi possível consultar o navegador padrão do Windows.");
    }
    Ok(PathBuf::from(OsString::from_wide(&output[..end])))
}

#[cfg(windows)]
fn local_file_url(file: &Path) -> Result<String, &'static str> {
    tauri::Url::from_file_path(file)
        .map(|url| url.to_string())
        .map_err(|_| "Não foi possível preparar o endereço deste arquivo.")
}

#[tauri::command]
pub async fn open_site_in_browser(
    project_root: String,
    file_path: String,
) -> Result<(), &'static str> {
    let project = crate::project::inspect_project(project_root).await?;
    let file = local_html_path(Path::new(&project.project_root), Path::new(&file_path))?;
    #[cfg(windows)]
    {
        let url = local_file_url(&file)?;
        let browser = default_browser_executable()?;
        std::process::Command::new(browser)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|_| "Não foi possível solicitar a abertura desta página no navegador.")
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("Abra esta página HTML em um navegador para usá-la.")
    }
}

#[tauri::command]
pub async fn open_pdf_in_browser(
    project_root: String,
    file_path: String,
) -> Result<(), &'static str> {
    let project = crate::project::inspect_project(project_root).await?;
    let file = local_pdf_path(Path::new(&project.project_root), Path::new(&file_path))?;
    #[cfg(windows)]
    {
        let url = local_file_url(&file)?;
        let browser = default_browser_executable()?;
        std::process::Command::new(browser)
            .arg(url)
            .spawn()
            .map(|_| ())
            .map_err(|_| "Não foi possível solicitar a abertura deste PDF no navegador.")
    }
    #[cfg(not(windows))]
    {
        let _ = file;
        Err("Abra este PDF em um navegador para vê-lo.")
    }
}

#[tauri::command]
pub async fn inspect_preview(
    project_root: String,
    file_path: String,
    site: tauri::State<'_, Arc<crate::preview_site::PreviewSiteState>>,
) -> Result<Preview, &'static str> {
    let generation = site.clear();
    let project = crate::project::inspect_project(project_root).await?;
    let root = PathBuf::from(project.project_root);
    let file = PathBuf::from(file_path);
    let mut preview = tokio::task::spawn_blocking({
        let root = root.clone();
        let file = file.clone();
        move || read_preview(&root, &file)
    })
    .await
    .map_err(|_| "Não foi possível preparar a prévia.")??;
    if file
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|value| {
            value.eq_ignore_ascii_case("html") || value.eq_ignore_ascii_case("htm")
        })
    {
        let canonical_root = root
            .canonicalize()
            .map_err(|_| "Não foi possível conferir a pasta do projeto.")?;
        let canonical_file = file
            .canonicalize()
            .map_err(|_| "Este arquivo não está mais disponível.")?;
        canonical_file
            .strip_prefix(&canonical_root)
            .map_err(|_| "Este arquivo não pertence ao projeto aberto.")?;
        let site_root = canonical_file
            .parent()
            .ok_or("Este arquivo não tem uma pasta válida.")?
            .to_path_buf();
        preview.render_url = site.install(generation, site_root, &canonical_file);
    }
    Ok(preview)
}

#[tauri::command]
pub fn clear_preview_site(site: tauri::State<'_, Arc<crate::preview_site::PreviewSiteState>>) {
    site.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn shows_project_text_and_reloads_updates_without_interpreting_html() {
        let root = std::env::temp_dir().join(format!("forge-preview-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("page.html");
        fs::write(&file, "<script>first</script>").unwrap();
        let first = read_preview(&root, &file).unwrap();
        assert_eq!(first.kind, "text");
        assert_eq!(first.content, "<script>first</script>");
        fs::write(&file, "<script>updated</script>").unwrap();
        assert_eq!(
            read_preview(&root, &file).unwrap().content,
            "<script>updated</script>"
        );
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn rejects_outside_files_and_describes_unsupported_formats_without_reading_them() {
        let base = std::env::temp_dir().join(format!("forge-preview-scope-{}", std::process::id()));
        let root = base.join("project");
        fs::create_dir_all(&root).unwrap();
        let outside = base.join("outside.txt");
        fs::write(&outside, "private").unwrap();
        assert!(read_preview(&root, &outside).is_err());
        let svg = root.join("active.svg");
        fs::write(&svg, "<svg onload='alert(1)'></svg>").unwrap();
        let file = read_preview(&root, &svg).unwrap();
        assert_eq!(file.kind, "file");
        assert_eq!(file.relative_path, "active.svg");
        assert!(file.content.is_empty());
        assert_eq!(file.size_bytes, fs::metadata(&svg).unwrap().len());
        fs::remove_dir_all(&base).unwrap();
    }

    #[test]
    fn rejects_image_extension_without_matching_signature() {
        let root = std::env::temp_dir().join(format!("forge-preview-image-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let file = root.join("picture.png");
        fs::write(&file, b"<svg onload='alert(1)'>").unwrap();
        assert!(read_preview(&root, &file).is_err());
        fs::write(&file, b"\x89PNG\r\n\x1a\nfixture").unwrap();
        assert!(read_preview(&root, &file)
            .unwrap()
            .content
            .starts_with("data:image/png;base64,"));
        fs::remove_dir_all(&root).unwrap();
    }

    #[test]
    fn html_preview_accepts_a_bounded_page_larger_than_plain_text_limit() {
        let root = std::env::temp_dir().join(format!("forge-preview-html-{}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let html = root.join("index.htm");
        fs::write(&html, "x".repeat(40 * 1024)).unwrap();
        assert_eq!(read_preview(&root, &html).unwrap().content.len(), 40 * 1024);
        let text = root.join("large.txt");
        fs::write(&text, "x".repeat(40 * 1024)).unwrap();
        assert!(read_preview(&root, &text).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn browser_opening_accepts_only_html_inside_the_project() {
        let base = std::env::temp_dir().join(format!("forge-open-site-{}", std::process::id()));
        let root = base.join("project");
        fs::create_dir_all(&root).unwrap();
        let inside = root.join("index.html");
        let script = root.join("site.js");
        let outside = base.join("outside.html");
        fs::write(&inside, "<h1>Local</h1>").unwrap();
        fs::write(&script, "alert(1)").unwrap();
        fs::write(&outside, "<h1>Outside</h1>").unwrap();
        assert_eq!(local_html_path(&root, &inside).unwrap(), inside.canonicalize().unwrap());
        assert!(local_html_path(&root, &script).is_err());
        assert!(local_html_path(&root, &outside).is_err());
        assert!(local_html_path(&root, Path::new("index.html")).is_err());
        #[cfg(windows)]
        {
            let spaced = root.join("a page #1.html");
            let url = local_file_url(&spaced).unwrap();
            assert!(url.starts_with("file:///"));
            assert!(url.contains("a%20page%20%231.html"));
        }
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn pdf_browser_opening_accepts_only_real_project_pdfs() {
        let base = std::env::temp_dir().join(format!("forge-open-pdf-{}", std::process::id()));
        let root = base.join("project");
        fs::create_dir_all(&root).unwrap();
        let inside = root.join("report.PDF");
        let wrong_header = root.join("fake.pdf");
        let wrong_extension = root.join("report.txt");
        let outside = base.join("outside.pdf");
        fs::write(&inside, b"%PDF-1.4\nfixture").unwrap();
        fs::write(&wrong_header, b"<html>not a pdf</html>").unwrap();
        fs::write(&wrong_extension, b"%PDF-1.4\nfixture").unwrap();
        fs::write(&outside, b"%PDF-1.4\nfixture").unwrap();
        assert_eq!(
            local_pdf_path(&root, &inside).unwrap(),
            inside.canonicalize().unwrap()
        );
        assert!(local_pdf_path(&root, &wrong_header).is_err());
        assert!(local_pdf_path(&root, &wrong_extension).is_err());
        assert!(local_pdf_path(&root, &outside).is_err());
        assert!(local_pdf_path(&root, Path::new("report.PDF")).is_err());
        fs::remove_dir_all(base).unwrap();
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "Requires a Windows user with a configured default browser; does not launch it"]
    fn default_browser_association_points_to_an_executable() {
        assert!(default_browser_executable().unwrap().is_file());
    }
}
