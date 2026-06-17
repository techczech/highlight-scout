// Semantic search via QMD (ADR-0005). We shell out to the qmd wrapper, which
// resolves its own Node + entry point absolutely, so it works under the GUI
// app's minimal PATH. The v2 archive's work Markdown is registered as the
// `highlight-scout` collection; query results map back to highlights by slug.

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tauri::Emitter;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

pub const COLLECTION: &str = "highlight-scout";

#[derive(Debug, Deserialize)]
pub struct QmdHit {
    #[serde(default)]
    #[allow(dead_code)] // hits arrive pre-ranked; we keep QMD's order
    pub score: f64,
    pub file: String,
    #[serde(default)]
    pub snippet: String,
}

fn qmd_bin() -> &'static str {
    if std::path::Path::new("/opt/homebrew/bin/qmd").exists() {
        "/opt/homebrew/bin/qmd"
    } else {
        "qmd"
    }
}

fn works_dir(archive_path: &str) -> String {
    format!("{}/readings/works", archive_path.trim_end_matches('/'))
}

/// Register the collection if it is not already present.
pub async fn ensure_collection(archive_path: &str) -> Result<()> {
    let list = Command::new(qmd_bin())
        .args(["collection", "list"])
        .output()
        .await?;
    let listed = String::from_utf8_lossy(&list.stdout);
    if listed.contains(COLLECTION) {
        return Ok(());
    }
    let works = works_dir(archive_path);
    let out = Command::new(qmd_bin())
        .args(["collection", "add", &works, "--name", COLLECTION, "--mask", "**/*.md"])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "qmd collection add failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let _ = Command::new(qmd_bin())
        .args([
            "context",
            "add",
            &format!("qmd://{}", COLLECTION),
            "Reading highlights: work-level Markdown with highlight blocks, notes, author/title metadata, and source links.",
        ])
        .output()
        .await;
    Ok(())
}

/// Re-index and embed, streaming qmd's output lines as progress events.
pub async fn reindex(archive_path: &str, window: &tauri::WebviewWindow) -> Result<()> {
    ensure_collection(archive_path).await?;
    stream(&["update"], "Indexing markdown…", window).await?;
    stream(&["embed"], "Generating embeddings (this can take a few minutes)…", window).await?;
    Ok(())
}

async fn stream(args: &[&str], label: &str, window: &tauri::WebviewWindow) -> Result<()> {
    let _ = window.emit(
        "import:progress",
        serde_json::json!({ "message": label, "current": 0, "total": 0 }),
    );
    let mut child = Command::new(qmd_bin())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // qmd reports progress on stderr; surface the latest line.
    if let Some(err) = child.stderr.take() {
        let mut lines = BufReader::new(err).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            let line = line.trim();
            if !line.is_empty() {
                let _ = window.emit(
                    "import:progress",
                    serde_json::json!({ "message": format!("{} {}", label, line), "current": 0, "total": 0 }),
                );
            }
        }
    }
    let status = child.wait().await?;
    if !status.success() {
        return Err(anyhow!("qmd {:?} failed", args));
    }
    Ok(())
}

/// Run a full QMD query (lexical + vector + HyDE + rerank) and return hits.
pub async fn query(query_text: &str, limit: usize) -> Result<Vec<QmdHit>> {
    let out = Command::new(qmd_bin())
        .args(["query", query_text, "-c", COLLECTION, "--json"])
        .output()
        .await?;
    if !out.status.success() {
        return Err(anyhow!(
            "qmd query failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let stdout = String::from_utf8_lossy(&out.stdout);
    let json = extract_json_array(&stdout)
        .ok_or_else(|| anyhow!("no JSON array in qmd output"))?;
    let hits: Vec<QmdHit> = serde_json::from_str(&json)?;
    Ok(hits.into_iter().take(limit).collect())
}

/// QMD prints progress before the JSON; slice from the first '[' to the last ']'.
fn extract_json_array(s: &str) -> Option<String> {
    let start = s.find('[')?;
    let end = s.rfind(']')?;
    if end > start {
        Some(s[start..=end].to_string())
    } else {
        None
    }
}

/// The work slug encoded in a qmd result path (basename without extension).
pub fn slug_from_file(file: &str) -> String {
    let base = file.rsplit('/').next().unwrap_or(file);
    base.strip_suffix(".md").unwrap_or(base).to_string()
}

/// The highlight quote text carried in a qmd snippet (blockquote `> …` lines).
pub fn quote_from_snippet(snippet: &str) -> Option<String> {
    snippet
        .lines()
        .filter_map(|l| {
            let t = l.trim_start();
            t.strip_prefix("> ").or_else(|| t.strip_prefix(">"))
        })
        .map(|s| s.trim().to_string())
        .filter(|s| s.len() > 8)
        .max_by_key(|s| s.len())
}
