use rusqlite::Connection;
use crate::index::sqlite;

/// True when OCR is supported on this platform.
pub fn available() -> bool { cfg!(target_os = "macos") }

/// Compute the Zotero asset path for an image highlight (mirrors map_row).
fn asset_for(archive: &str, id: &str, format: &str) -> Option<String> {
    if format == "image" {
        Some(format!("{}/readings/assets/{}.png", archive.trim_end_matches('/'), id))
    } else { None }
}

/// Testable batch driver: OCR each pending image-highlight via an injected
/// async fn, writing ocr_text (empty string = "tried, nothing found" so it
/// isn't retried). Returns the number of highlights written. Operates on an
/// owned/borrowed &Connection (NOT a Mutex guard held across await — the app
/// wrapper handles locking; this is for tests + single-threaded use).
pub async fn run_ocr<F, Fut>(conn: &Connection, archive: &str, only: Option<&[String]>, mut ocr_fn: F) -> usize
where F: FnMut(Vec<String>) -> Fut, Fut: std::future::Future<Output = String> {
    let pending = sqlite::ocr_pending(conn, only).unwrap_or_default();
    let mut n = 0;
    for (id, format, text) in pending {
        let asset = asset_for(archive, &id, &format);
        let sources = sqlite::ocr_sources(&format, asset.as_deref(), &text);
        if sources.is_empty() { continue; }
        let recognized = ocr_fn(sources).await;
        if sqlite::write_ocr(conn, &id, &recognized).is_ok() { n += 1; }
    }
    n
}

#[cfg(target_os = "macos")]
pub mod platform {
    use tauri::{AppHandle, Manager};
    use std::path::PathBuf;
    pub fn helper_path(app: &AppHandle) -> Option<PathBuf> {
        app.path().resolve("binaries/ocr-helper", tauri::path::BaseDirectory::Resource)
            .ok().filter(|p| p.exists())
    }
    /// Download (http/https) or locate (local path) each source, OCR via the
    /// helper, return concatenated recognized text.
    pub async fn ocr_sources(app: &AppHandle, sources: Vec<String>) -> String {
        let Some(helper) = helper_path(app) else { return String::new() };
        #[cfg(unix)]
        { use std::os::unix::fs::PermissionsExt;
          if let Ok(m) = std::fs::metadata(&helper) { let mut p = m.permissions(); p.set_mode(0o755); let _ = std::fs::set_permissions(&helper, p); } }
        let client = reqwest::Client::builder().timeout(std::time::Duration::from_secs(20)).build().ok();
        let mut files: Vec<(PathBuf, bool)> = Vec::new();
        for s in &sources {
            if s.starts_with("http://") || s.starts_with("https://") {
                if let Some(c) = &client {
                    if let Ok(r) = c.get(s).send().await {
                        if let Ok(r) = r.error_for_status() {
                            if let Ok(b) = r.bytes().await {
                                let tmp = std::env::temp_dir().join(format!("hs-ocr-{}.img", uuid::Uuid::new_v4()));
                                if std::fs::write(&tmp, &b).is_ok() { files.push((tmp, true)); }
                            }
                        }
                    }
                }
            } else {
                let p = PathBuf::from(s);
                if p.exists() { files.push((p, false)); }
            }
        }
        if files.is_empty() { return String::new(); }
        let args: Vec<String> = files.iter().map(|(p,_)| p.to_string_lossy().to_string()).collect();
        let out = tokio::process::Command::new(&helper).args(&args).output().await.ok();
        for (p, is_temp) in &files { if *is_temp { let _ = std::fs::remove_file(p); } }
        out.filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }
}

#[cfg(not(target_os = "macos"))]
pub mod platform {
    use tauri::AppHandle;
    pub async fn ocr_sources(_app: &AppHandle, _sources: Vec<String>) -> String { String::new() }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::index::sqlite::init_schema;
    #[tokio::test]
    async fn run_ocr_writes_only_for_image_highlights_and_makes_them_searchable() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w','w','W',NULL,'article','x',NULL,NULL,'t','t','{}')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('img','w','see ![image](https://p/a.jpg)','[]','plain','{}')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('noimg','w','plain text','[]','plain','{}')", []).unwrap();
        let n = run_ocr(&conn, "/tmp", None, |_s| async { "RECOGNISED".to_string() }).await;
        assert_eq!(n, 1);
        let got: Option<String> = conn.query_row("SELECT ocr_text FROM highlights WHERE id='img'", [], |r| r.get(0)).unwrap();
        assert_eq!(got.as_deref(), Some("RECOGNISED"));
        let none: Option<String> = conn.query_row("SELECT ocr_text FROM highlights WHERE id='noimg'", [], |r| r.get(0)).unwrap();
        assert_eq!(none, None);
        let cnt: i64 = conn.query_row("SELECT COUNT(*) FROM search_index WHERE search_index MATCH 'RECOGNISED'", [], |r| r.get(0)).unwrap();
        assert_eq!(cnt, 1);
    }
}
