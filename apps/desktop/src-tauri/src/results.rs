//! Bounded, metadata-only discovery of user-facing project files.
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

const MAX_DEPTH: usize = 4;
const MAX_VISITS: usize = 2_000;
const MAX_FILES: usize = 100;
const MAX_PATH_BYTES: usize = 1_024;

#[derive(Serialize)]
pub struct FilePage {
    pub files: Vec<ProjectFile>,
    pub truncated: bool,
}

#[derive(Serialize)]
pub struct ProjectFile {
    pub relative_path: String,
    pub kind: &'static str,
    pub size_bytes: u64,
    pub modified_at: u64,
}

#[tauri::command]
pub async fn list_project_files(project_root: String) -> Result<FilePage, &'static str> {
    let project = crate::project::inspect_project(project_root).await?;
    let root = PathBuf::from(project.project_root);
    tokio::task::spawn_blocking(move || scan_project_files(&root))
        .await
        .map_err(|_| "Não foi possível listar os arquivos do projeto.")?
}

fn file_kind(path: &Path) -> Option<&'static str> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    Some(match ext.as_str() {
        "html" | "htm" => "page",
        "png" | "jpg" | "jpeg" | "gif" | "webp" => "image",
        "md" | "txt" | "pdf" | "doc" | "docx" | "xls" | "xlsx" | "ppt" | "pptx" => "document",
        "csv" | "json" => "data",
        "mp3" | "wav" => "audio",
        "mp4" => "video",
        "zip" => "archive",
        _ => return None,
    })
}

fn excluded(name: &std::ffi::OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return true;
    };
    if name.starts_with('.') {
        return true;
    }
    matches!(
        name.to_ascii_lowercase().as_str(),
        "node_modules" | "target" | "vendor" | "cache" | "caches" | "__pycache__"
    )
}

#[cfg(windows)]
fn is_reparse(metadata: &fs::Metadata) -> bool {
    use std::os::windows::fs::MetadataExt;
    metadata.file_attributes() & 0x400 != 0
}

#[cfg(not(windows))]
fn is_reparse(_: &fs::Metadata) -> bool {
    false
}

fn scan_project_files(root: &Path) -> Result<FilePage, &'static str> {
    let root = root
        .canonicalize()
        .map_err(|_| "A pasta do projeto não está mais disponível.")?;
    let mut stack = vec![(root.clone(), 0usize)];
    let mut files = Vec::new();
    let mut visits = 0usize;
    let mut truncated = false;
    while let Some((directory, depth)) = stack.pop() {
        if visits >= MAX_VISITS {
            truncated = true;
            break;
        }
        let entries = match fs::read_dir(&directory) {
            Ok(entries) => entries,
            Err(_) => {
                truncated = true;
                continue;
            }
        };
        for entry in entries {
            if visits >= MAX_VISITS {
                truncated = true;
                break;
            }
            visits += 1;
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            let name = entry.file_name();
            if excluded(&name) {
                continue;
            }
            let path = entry.path();
            let metadata = match fs::symlink_metadata(&path) {
                Ok(m) => m,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            if metadata.file_type().is_symlink() || is_reparse(&metadata) {
                continue;
            }
            if metadata.is_dir() {
                if depth < MAX_DEPTH {
                    stack.push((path, depth + 1));
                } else {
                    truncated = true;
                }
                continue;
            }
            if !metadata.is_file() {
                continue;
            }
            let Some(kind) = file_kind(&path) else {
                continue;
            };
            let canonical = match path.canonicalize() {
                Ok(p) => p,
                Err(_) => {
                    truncated = true;
                    continue;
                }
            };
            let relative = match canonical.strip_prefix(&root) {
                Ok(p) => p,
                Err(_) => continue,
            };
            let relative_path = relative.to_string_lossy().replace('\\', "/");
            if relative_path.len() > MAX_PATH_BYTES {
                truncated = true;
                continue;
            }
            let modified_at = metadata
                .modified()
                .ok()
                .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                .map(|d| d.as_secs())
                .unwrap_or(0);
            files.push(ProjectFile {
                relative_path,
                kind,
                size_bytes: metadata.len(),
                modified_at,
            });
        }
    }
    files.sort_by(|a, b| {
        b.modified_at
            .cmp(&a.modified_at)
            .then_with(|| a.relative_path.cmp(&b.relative_path))
    });
    if files.len() > MAX_FILES {
        files.truncate(MAX_FILES);
        truncated = true;
    }
    Ok(FilePage { files, truncated })
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fixture(PathBuf);
    impl Fixture {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir()
                .join(format!("forge-results-{label}-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn put(&self, name: &str, contents: &[u8]) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, contents).unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn empty_and_supported_file_types_are_discovered_without_changing_contents() {
        let f = Fixture::new("types");
        assert!(scan_project_files(&f.0).unwrap().files.is_empty());
        for (name, kind) in [
            ("a.html", "page"),
            ("b.png", "image"),
            ("c.md", "document"),
            ("d.csv", "data"),
            ("e.wav", "audio"),
            ("f.mp4", "video"),
            ("g.zip", "archive"),
        ] {
            f.put(name, b"kept");
            assert_eq!(file_kind(Path::new(name)), Some(kind));
        }
        let page = scan_project_files(&f.0).unwrap();
        assert_eq!(page.files.len(), 7);
        for name in [
            "a.html", "b.png", "c.md", "d.csv", "e.wav", "f.mp4", "g.zip",
        ] {
            assert_eq!(fs::read(f.0.join(name)).unwrap(), b"kept");
        }
    }

    #[test]
    fn excludes_hidden_build_dependency_and_source_files() {
        let f = Fixture::new("exclude");
        for name in [
            ".hidden/a.md",
            "target/b.md",
            "node_modules/c.md",
            "vendor/d.md",
            ".git/e.md",
            ".forge/f.md",
            ".codex/g.md",
            "src/main.rs",
            "script.js",
        ] {
            f.put(name, b"x");
        }
        assert!(scan_project_files(&f.0).unwrap().files.is_empty());
    }

    #[test]
    fn results_are_newest_first_then_path_and_capped() {
        let f = Fixture::new("order");
        f.put("b.txt", b"b");
        f.put("a.txt", b"a");
        let page = scan_project_files(&f.0).unwrap();
        assert_eq!(page.files.len(), 2);
        assert!(page.files[0].modified_at >= page.files[1].modified_at);
        for pair in page.files.windows(2) {
            assert!(
                pair[0].modified_at > pair[1].modified_at
                    || (pair[0].modified_at == pair[1].modified_at
                        && pair[0].relative_path <= pair[1].relative_path)
            );
        }
        for i in 0..105 {
            f.put(&format!("{i:03}.txt"), b"x");
        }
        let page = scan_project_files(&f.0).unwrap();
        assert_eq!(page.files.len(), MAX_FILES);
        assert!(page.truncated);
    }
}
