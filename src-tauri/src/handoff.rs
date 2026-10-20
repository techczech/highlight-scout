//! The "Analyse in ArchiveScout" hand-off: is ArchiveScout installed, and
//! where its release page is. The link itself (`archivescout://explore?…`)
//! is built in the front end (`src/lib/archivescout.ts`) from the search.

use std::path::{Path, PathBuf};

/// Where the installed app may be: the system and the user's Applications.
pub fn candidates(home: Option<&Path>) -> Vec<PathBuf> {
    let mut v = vec![PathBuf::from("/Applications/ArchiveScout.app")];
    if let Some(h) = home {
        v.push(h.join("Applications/ArchiveScout.app"));
    }
    v
}

pub fn installed_in(candidates: &[PathBuf]) -> bool {
    candidates.iter().any(|p| p.is_dir())
}

pub fn installed() -> bool {
    installed_in(&candidates(dirs::home_dir().as_deref()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installed_when_an_app_bundle_exists_in_any_candidate() {
        let root = std::env::temp_dir().join(format!("hs-handoff-{}", std::process::id()));
        let apps = root.join("Applications/ArchiveScout.app");
        let c = candidates(Some(&root));
        assert_eq!(c.len(), 2);
        assert!(c[1].ends_with("Applications/ArchiveScout.app"));
        assert!(!installed_in(&[c[1].clone()]));
        std::fs::create_dir_all(&apps).unwrap();
        assert!(installed_in(&[c[1].clone()]));
        assert!(!installed_in(&[]));
        std::fs::remove_dir_all(root).unwrap();
    }

    /// The opener may hand ArchiveScout its explore links and nothing else of its scheme.
    #[test]
    fn the_opener_may_open_only_archivescout_explore_links() {
        let caps: serde_json::Value = serde_json::from_str(include_str!("../capabilities/default.json")).unwrap();
        let urls: Vec<String> = caps["permissions"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|p| p["identifier"] == "opener:allow-open-url")
            .flat_map(|p| p["allow"].as_array().cloned().unwrap_or_default())
            .filter_map(|a| a["url"].as_str().map(str::to_string))
            .filter(|u| u.starts_with("archivescout:"))
            .collect();
        // Exactly the explore host: bare, or with a query. `?` alone is a glob wildcard, hence `[?]`.
        assert_eq!(urls, ["archivescout://explore", "archivescout://explore[?]*"]);
        // The opener matches with glob::Pattern::matches (tauri-plugin-opener scope.rs).
        let pats: Vec<glob::Pattern> = urls.iter().map(|u| glob::Pattern::new(u).unwrap()).collect();
        let allowed = |u: &str| pats.iter().any(|p| p.matches(u));
        for ok in ["archivescout://explore?mode=kwic&q=scaffolding&in=writing", "archivescout://explore", "archivescout://explore?"] {
            assert!(allowed(ok), "{ok}");
        }
        for other in [
            "archivescout://explore-other?q=x",
            "archivescout://explorer",
            "archivescout://explore.evil?q=x",
            "archivescout://exploreX",
            "archivescout://explore/x",
            "archivescout://explore#x",
            "archivescout://corpora?q=x",
            "archivescout://settings",
            "archivescout://",
        ] {
            assert!(!allowed(other), "{other}");
        }
    }
}
