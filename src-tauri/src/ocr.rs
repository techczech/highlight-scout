use rusqlite::Connection;
use scout_index::sqlite;
use std::sync::atomic::{AtomicBool, Ordering};

/// Sets the shared is_ocring flag true on creation, false on drop (panic-safe).
pub(crate) struct OcrGuard<'a>(&'a AtomicBool);
impl<'a> OcrGuard<'a> {
    pub(crate) fn acquire(flag: &'a AtomicBool) -> Self {
        flag.store(true, Ordering::SeqCst);
        OcrGuard(flag)
    }
}
impl Drop for OcrGuard<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

/// True when OCR is supported on this platform.
pub fn available() -> bool {
    cfg!(target_os = "macos")
}

/// Compute the Zotero asset path for an image highlight (mirrors map_row).
fn asset_for(archive: &str, id: &str, format: &str) -> Option<String> {
    if format == "image" {
        Some(format!(
            "{}/readings/assets/{}.png",
            archive.trim_end_matches('/'),
            id
        ))
    } else {
        None
    }
}

/// Identity of one OCR input: a local file by the SHA-256 of its bytes (so
/// the same picture under two asset names is one image), a URL by itself.
fn source_key(src: &str) -> String {
    if src.starts_with("http://") || src.starts_with("https://") {
        return src.to_string();
    }
    match std::fs::read(src) {
        Ok(bytes) => {
            use sha2::{Digest, Sha256};
            format!("sha256:{}", hex::encode(Sha256::digest(&bytes)))
        }
        Err(_) => src.to_string(),
    }
}

fn sources_key(sources: &[String]) -> String {
    sources
        .iter()
        .map(|s| source_key(s))
        .collect::<Vec<_>>()
        .join("\n")
}

/// One pending image highlight to OCR.
struct OcrJob {
    id: String,
    sources: Vec<String>,
}

/// Pending jobs plus the OCR text the index already holds, keyed by image.
/// A pending row whose images were already recognised under another row
/// (same URL, or an asset with the same bytes) reuses that text instead of
/// running OCR again.
struct OcrPlan {
    jobs: Vec<OcrJob>,
    known: std::collections::HashMap<String, String>,
}

impl OcrPlan {
    fn build(conn: &Connection, archive: &str, only: Option<&[String]>) -> OcrPlan {
        let jobs: Vec<OcrJob> = sqlite::ocr_pending(conn, only)
            .unwrap_or_default()
            .into_iter()
            .filter_map(|(id, format, text)| {
                let asset = asset_for(archive, &id, &format);
                let sources = sqlite::ocr_sources(&format, asset.as_deref(), &text);
                (!sources.is_empty()).then_some(OcrJob { id, sources })
            })
            .collect();
        let mut known = std::collections::HashMap::new();
        if !jobs.is_empty() {
            let done: Vec<(String, String, String, String)> = conn
                .prepare(
                    "SELECT id, format, text, ocr_text FROM highlights
                     WHERE ocr_text IS NOT NULL AND ocr_text != ''
                       AND (format='image' OR text LIKE '%![image](%')",
                )
                .and_then(|mut st| {
                    st.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
                        .collect()
                })
                .unwrap_or_default();
            for (id, format, text, ocr) in done {
                let asset = asset_for(archive, &id, &format);
                let sources = sqlite::ocr_sources(&format, asset.as_deref(), &text);
                if !sources.is_empty() {
                    known.entry(sources_key(&sources)).or_insert(ocr);
                }
            }
        }
        OcrPlan { jobs, known }
    }

    /// OCR text already known for this job's images, if any.
    fn reuse(&self, job: &OcrJob) -> (String, Option<String>) {
        let key = sources_key(&job.sources);
        let hit = self.known.get(&key).cloned();
        (key, hit)
    }

    fn remember(&mut self, key: String, text: &str) {
        if !text.is_empty() {
            self.known.insert(key, text.to_string());
        }
    }
}

/// Testable batch driver: OCR each pending image-highlight via an injected
/// async fn, writing ocr_text (empty string = "tried, nothing found"). Images
/// the index already holds OCR text for are not OCR'd again. Returns the
/// number of highlights written. Operates on a borrowed &Connection (NOT a
/// Mutex guard held across await — the app wrapper handles locking).
#[cfg_attr(not(test), allow(dead_code))]
pub async fn run_ocr<F, Fut>(
    conn: &Connection,
    archive: &str,
    only: Option<&[String]>,
    mut ocr_fn: F,
) -> usize
where
    F: FnMut(Vec<String>) -> Fut,
    Fut: std::future::Future<Output = String>,
{
    let mut plan = OcrPlan::build(conn, archive, only);
    let jobs = std::mem::take(&mut plan.jobs);
    let mut n = 0;
    for job in jobs {
        let (key, hit) = plan.reuse(&job);
        let recognized = match hit {
            Some(t) => t,
            None => ocr_fn(job.sources.clone()).await,
        };
        plan.remember(key, &recognized);
        if sqlite::write_ocr(conn, &job.id, &recognized).is_ok() {
            n += 1;
        }
    }
    n
}

#[cfg(target_os = "macos")]
pub async fn run_ocr_app(
    app: &tauri::AppHandle,
    window: &tauri::WebviewWindow,
    archive: &str,
    only: Option<&[String]>,
) -> usize {
    use crate::AppState;
    use tauri::{Emitter, Manager};
    // Build the plan under a SHORT lock, then drop the guard before awaiting.
    let mut plan = {
        let state = app.state::<AppState>();
        let conn = state.db.lock().unwrap();
        OcrPlan::build(&conn, archive, only)
    };
    let jobs = std::mem::take(&mut plan.jobs);
    let total = jobs.len();
    let mut n = 0usize;
    for (i, job) in jobs.into_iter().enumerate() {
        let (key, hit) = plan.reuse(&job);
        let recognized = match hit {
            Some(t) => t,
            None => platform::ocr_sources(app, job.sources.clone()).await, // NO lock held here
        };
        plan.remember(key, &recognized);
        {
            let state = app.state::<AppState>();
            let conn = state.db.lock().unwrap();
            if sqlite::write_ocr(&conn, &job.id, &recognized).is_ok() {
                n += 1;
            }
        }
        let _ = window.emit(
            "import:progress",
            serde_json::json!({
            "message": format!("OCR {}/{}", i + 1, total), "current": i + 1, "total": total }),
        );
    }
    let _ = window.emit(
        "import:complete",
        serde_json::json!({ "message": format!("OCR'd {} image highlight(s)", n) }),
    );
    n
}

#[cfg(not(target_os = "macos"))]
pub async fn run_ocr_app(
    _app: &tauri::AppHandle,
    _window: &tauri::WebviewWindow,
    _archive: &str,
    _only: Option<&[String]>,
) -> usize {
    0
}

/// Spawn a background OCR pass over all pending images if enabled + idle.
/// Called after an import. No-op when disabled / already running / non-macOS.
pub fn maybe_auto_ocr(app: &tauri::AppHandle, window: tauri::WebviewWindow) {
    use tauri::Manager;
    if !available() {
        return;
    }
    let state = app.state::<crate::AppState>();
    if !state.config().ocr_on_import {
        return;
    }
    if state.is_ocring.swap(true, Ordering::SeqCst) {
        return;
    }
    let archive = state.config().archive_path.clone();
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        // OcrGuard ensures is_ocring is cleared even if run_ocr_app panics.
        let state2 = app2.state::<crate::AppState>();
        let _guard = OcrGuard::acquire(&state2.is_ocring);
        let _ = run_ocr_app(&app2, &window, &archive, None).await;
    });
}

#[cfg(target_os = "macos")]
pub mod platform {
    use std::path::PathBuf;
    use tauri::{AppHandle, Manager};
    pub fn helper_path(app: &AppHandle) -> Option<PathBuf> {
        app.path()
            .resolve("binaries/ocr-helper", tauri::path::BaseDirectory::Resource)
            .ok()
            .filter(|p| p.exists())
    }
    /// Download (http/https) or locate (local path) each source, OCR via the
    /// helper, return concatenated recognized text.
    pub async fn ocr_sources(app: &AppHandle, sources: Vec<String>) -> String {
        let Some(helper) = helper_path(app) else {
            return String::new();
        };
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            if let Ok(m) = std::fs::metadata(&helper) {
                let mut p = m.permissions();
                p.set_mode(0o755);
                let _ = std::fs::set_permissions(&helper, p);
            }
        }
        let client = crate::http::builder()
            .timeout(std::time::Duration::from_secs(20))
            .build()
            .ok();
        let mut files: Vec<(PathBuf, bool)> = Vec::new();
        for s in &sources {
            if s.starts_with("http://") || s.starts_with("https://") {
                if let Some(c) = &client {
                    if let Ok(r) = c.get(s).send().await {
                        if let Ok(r) = r.error_for_status() {
                            if let Ok(b) = r.bytes().await {
                                let tmp = std::env::temp_dir()
                                    .join(format!("hs-ocr-{}.img", uuid::Uuid::new_v4()));
                                if std::fs::write(&tmp, &b).is_ok() {
                                    files.push((tmp, true));
                                }
                            }
                        }
                    }
                }
            } else {
                let p = PathBuf::from(s);
                if p.exists() {
                    files.push((p, false));
                }
            }
        }
        if files.is_empty() {
            return String::new();
        }
        let args: Vec<String> = files
            .iter()
            .map(|(p, _)| p.to_string_lossy().to_string())
            .collect();
        let out = tokio::process::Command::new(&helper)
            .args(&args)
            .output()
            .await
            .ok();
        for (p, is_temp) in &files {
            if *is_temp {
                let _ = std::fs::remove_file(p);
            }
        }
        out.filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
            .unwrap_or_default()
    }
}

#[cfg(not(target_os = "macos"))]
pub mod platform {
    use tauri::AppHandle;
    pub async fn ocr_sources(_app: &AppHandle, _sources: Vec<String>) -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use scout_index::sqlite::init_schema;

    // The same image under another highlight (same URL, or an asset with the
    // same bytes) already has OCR text: reuse it, do not OCR again.
    #[tokio::test]
    async fn images_the_index_already_recognised_are_not_ocrd_again() {
        let dir = std::env::temp_dir().join(format!("hs-ocr-reuse-{}", std::process::id()));
        let assets = dir.join("readings/assets");
        std::fs::create_dir_all(&assets).unwrap();
        std::fs::write(assets.join("done-img.png"), b"same pixels").unwrap();
        std::fs::write(assets.join("new-img.png"), b"same pixels").unwrap();
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w','w','W',NULL,'article','x',NULL,NULL,'t','t','{}')", []).unwrap();
        for (id, text, format, ocr) in [
            (
                "done-url",
                "a ![image](https://p/a.jpg)",
                "plain",
                Some("KNOWN URL TEXT"),
            ),
            ("new-url", "b ![image](https://p/a.jpg)", "plain", None),
            ("done-img", "", "image", Some("KNOWN ASSET TEXT")),
            ("new-img", "", "image", None),
            ("fresh", "c ![image](https://p/other.jpg)", "plain", None),
            (
                "fresh-twin",
                "d ![image](https://p/other.jpg)",
                "plain",
                None,
            ),
        ] {
            conn.execute(
                "INSERT INTO highlights (id,work_id,text,tags,format,source_data,ocr_text) VALUES (?1,'w',?2,'[]',?3,'{}',?4)",
                rusqlite::params![id, text, format, ocr],
            )
            .unwrap();
        }
        let calls = std::cell::RefCell::new(Vec::<Vec<String>>::new());
        let n = run_ocr(&conn, dir.to_str().unwrap(), None, |s| {
            calls.borrow_mut().push(s);
            async { "NEW TEXT".to_string() }
        })
        .await;
        assert_eq!(n, 4);
        // Only the never-seen image is OCR'd, once for both of its rows.
        assert_eq!(*calls.borrow(), [vec!["https://p/other.jpg".to_string()]]);
        let ocr = |id: &str| -> String {
            conn.query_row("SELECT ocr_text FROM highlights WHERE id=?1", [id], |r| {
                r.get(0)
            })
            .unwrap()
        };
        assert_eq!(ocr("new-url"), "KNOWN URL TEXT");
        assert_eq!(ocr("new-img"), "KNOWN ASSET TEXT");
        assert_eq!(ocr("fresh-twin"), "NEW TEXT");
        let _ = std::fs::remove_dir_all(&dir);
    }
    #[tokio::test]
    async fn run_ocr_writes_only_for_image_highlights_and_makes_them_searchable() {
        let conn = Connection::open_in_memory().unwrap();
        init_schema(&conn).unwrap();
        conn.execute("INSERT INTO works (id,slug,title,author,work_type,source_system,source_id,url,imported_at,updated_at,source_data) VALUES ('w','w','W',NULL,'article','x',NULL,NULL,'t','t','{}')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('img','w','see ![image](https://p/a.jpg)','[]','plain','{}')", []).unwrap();
        conn.execute("INSERT INTO highlights (id,work_id,text,tags,format,source_data) VALUES ('noimg','w','plain text','[]','plain','{}')", []).unwrap();
        let n = run_ocr(&conn, "/tmp", None, |_s| async { "RECOGNISED".to_string() }).await;
        assert_eq!(n, 1);
        let got: Option<String> = conn
            .query_row("SELECT ocr_text FROM highlights WHERE id='img'", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(got.as_deref(), Some("RECOGNISED"));
        let none: Option<String> = conn
            .query_row(
                "SELECT ocr_text FROM highlights WHERE id='noimg'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(none, None);
        let cnt: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM search_index WHERE search_index MATCH 'RECOGNISED'",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(cnt, 1);
    }
}
