//! Read-only, scriptless site preview on a separate local WebView origin.
//! Only the directory containing the explicitly selected HTML file is served.
use percent_encoding::percent_decode_str;
use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};
use tauri::http::{self, Request, Response, StatusCode};

const SITE_CSP: &str = "default-src 'none'; script-src 'none'; connect-src 'none'; style-src 'self' 'unsafe-inline'; img-src 'self' data:; font-src 'self' data:; frame-src 'none'; object-src 'none'; form-action 'none'; base-uri 'none'; sandbox";

#[derive(Clone)]
struct Session {
    root: PathBuf,
    token: String,
}

#[derive(Default)]
struct Inner {
    generation: u64,
    session: Option<Session>,
}

#[derive(Default)]
pub struct PreviewSiteState(Mutex<Inner>);

impl PreviewSiteState {
    pub fn clear(&self) -> u64 {
        let mut inner = self.0.lock().unwrap();
        inner.generation = inner.generation.wrapping_add(1);
        inner.session = None;
        inner.generation
    }

    pub fn install(&self, generation: u64, root: PathBuf, entry: &Path) -> Option<String> {
        let mut inner = self.0.lock().unwrap();
        if inner.generation != generation {
            return None;
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        let name = entry.file_name()?.to_str()?;
        let encoded =
            percent_encoding::utf8_percent_encode(name, percent_encoding::NON_ALPHANUMERIC);
        inner.session = Some(Session {
            root,
            token: token.clone(),
        });
        Some(format!("http://forgepreview.localhost/{token}/{encoded}"))
    }
}

fn response(status: StatusCode, mime: &'static str, body: Vec<u8>) -> Response<Vec<u8>> {
    Response::builder()
        .status(status)
        .header(http::header::CONTENT_TYPE, mime)
        .header(http::header::CONTENT_SECURITY_POLICY, SITE_CSP)
        .header(http::header::X_CONTENT_TYPE_OPTIONS, "nosniff")
        .header(http::header::CACHE_CONTROL, "no-store")
        .body(body)
        .unwrap()
}

fn mime_and_limit(path: &Path) -> Option<(&'static str, u64)> {
    match path.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "html" | "htm" => Some(("text/html; charset=utf-8", 512 * 1024)),
        "css" => Some(("text/css; charset=utf-8", 512 * 1024)),
        "png" => Some(("image/png", 5 * 1024 * 1024)),
        "jpg" | "jpeg" => Some(("image/jpeg", 5 * 1024 * 1024)),
        "gif" => Some(("image/gif", 5 * 1024 * 1024)),
        "webp" => Some(("image/webp", 5 * 1024 * 1024)),
        "woff" => Some(("font/woff", 2 * 1024 * 1024)),
        "woff2" => Some(("font/woff2", 2 * 1024 * 1024)),
        _ => None,
    }
}

fn read(session: &Session, uri: &str) -> Option<(&'static str, Vec<u8>)> {
    let url = tauri::Url::parse(uri).ok()?;
    // WebView2 navigates to the HTTP-mapped origin but Tauri can pass the
    // original custom-scheme URI to this handler.
    let mapped = url.scheme() == "http" && url.host_str() == Some("forgepreview.localhost");
    let original = url.scheme() == "forgepreview" && url.host_str() == Some("localhost");
    if !mapped && !original {
        return None;
    }
    let mut parts = url.path_segments()?;
    if parts.next()? != session.token {
        return None;
    }
    let segments: Vec<_> = parts.collect();
    if segments.is_empty() {
        return None;
    }
    let mut requested = session.root.clone();
    for segment in segments {
        let decoded = percent_decode_str(segment).decode_utf8().ok()?;
        if decoded.is_empty()
            || decoded == "."
            || decoded == ".."
            || decoded.contains(['/', '\\', '\0', ':'])
        {
            return None;
        }
        requested.push(decoded.as_ref());
    }
    let file = requested.canonicalize().ok()?;
    file.strip_prefix(&session.root).ok()?;
    let (mime, limit) = mime_and_limit(&file)?;
    let metadata = file.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > limit {
        return None;
    }
    let bytes = std::fs::read(file).ok()?;
    if bytes.len() as u64 > limit {
        return None;
    }
    Some((mime, bytes))
}

pub fn serve(state: &PreviewSiteState, request: Request<Vec<u8>>) -> Response<Vec<u8>> {
    if request.method() != http::Method::GET {
        return response(
            StatusCode::METHOD_NOT_ALLOWED,
            "text/plain; charset=utf-8",
            Vec::new(),
        );
    }
    let session = state.0.lock().unwrap().session.clone();
    if let Some((mime, bytes)) = session
        .as_ref()
        .and_then(|value| read(value, &request.uri().to_string()))
    {
        response(StatusCode::OK, mime, bytes)
    } else {
        response(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            Vec::new(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn serves_only_selected_site_tree_with_no_script_or_network_policy() {
        let base = std::env::temp_dir().join(format!("forge-site-preview-{}", std::process::id()));
        let site = base.join("site");
        fs::create_dir_all(site.join("assets")).unwrap();
        let index = site.join("index.html");
        fs::write(&index, "<h1>Local</h1><script>alert(1)</script>").unwrap();
        fs::write(site.join("assets/style.css"), "h1 {color: red}").unwrap();
        fs::write(base.join("private.html"), "private").unwrap();
        let state = PreviewSiteState::default();
        let generation = state.clear();
        let url = state
            .install(generation, site.canonicalize().unwrap(), &index)
            .unwrap();
        let request = Request::builder().uri(&url).body(Vec::new()).unwrap();
        let page = serve(&state, request);
        assert_eq!(page.status(), StatusCode::OK);
        assert_eq!(page.body(), b"<h1>Local</h1><script>alert(1)</script>");
        assert!(page.headers()[http::header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("script-src 'none'"));
        assert!(page.headers()[http::header::CONTENT_SECURITY_POLICY]
            .to_str()
            .unwrap()
            .contains("connect-src 'none'"));
        let original = Request::builder()
            .uri(url.replace("http://forgepreview.localhost", "forgepreview://localhost"))
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&state, original).status(), StatusCode::OK);
        let token = tauri::Url::parse(&url)
            .unwrap()
            .path_segments()
            .unwrap()
            .next()
            .unwrap()
            .to_owned();
        for path in [
            format!("/{token}/assets/style.css"),
            format!("/{token}/%2e%2e/private.html"),
            format!("/{token}/private.txt"),
        ] {
            let request = Request::builder()
                .uri(format!("http://forgepreview.localhost{path}"))
                .body(Vec::new())
                .unwrap();
            let result = serve(&state, request);
            assert_eq!(
                result.status() == StatusCode::OK,
                path == format!("/{token}/assets/style.css")
            );
        }
        for path in [
            "/index.html",
            "/assets/style.css",
            "/wrong-token/index.html",
        ] {
            let request = Request::builder()
                .uri(format!("http://forgepreview.localhost{path}"))
                .body(Vec::new())
                .unwrap();
            assert_eq!(serve(&state, request).status(), StatusCode::NOT_FOUND);
        }
        let asset = Request::builder()
            .uri(format!(
                "http://forgepreview.localhost/{token}/assets/style.css"
            ))
            .body(Vec::new())
            .unwrap();
        assert_eq!(serve(&state, asset).status(), StatusCode::OK);
        state.clear();
        let request = Request::builder().uri(url).body(Vec::new()).unwrap();
        assert_eq!(serve(&state, request).status(), StatusCode::NOT_FOUND);
        fs::remove_dir_all(base).unwrap();
    }
}
