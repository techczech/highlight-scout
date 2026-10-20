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
}
