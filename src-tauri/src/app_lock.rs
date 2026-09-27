//! A lock file in the app data dir that says "the app is running". The app
//! takes it at start; the command-line duplicate merge takes it too, so the
//! merge refuses while the app (and its sync) could be writing the archive.
//!
//! The file holds the owner's pid. A lock whose pid is no longer alive is
//! stale (the app was killed or quit without cleanup) and is taken over.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct AppLock {
    path: PathBuf,
    pid: u32,
}

/// Is a process with this pid alive? (`kill -0`, which signals nothing.)
pub fn pid_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(true)
}

impl AppLock {
    /// Take the lock at `path`, or say who holds it.
    pub fn acquire(path: &Path, is_alive: impl Fn(u32) -> bool) -> Result<AppLock, String> {
        let pid = std::process::id();
        for _ in 0..2 {
            if let Some(parent) = path.parent() {
                let _ = fs::create_dir_all(parent);
            }
            match fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
            {
                Ok(mut f) => {
                    f.write_all(pid.to_string().as_bytes())
                        .map_err(|e| e.to_string())?;
                    return Ok(AppLock {
                        path: path.to_path_buf(),
                        pid,
                    });
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
                    let holder = fs::read_to_string(path)
                        .ok()
                        .and_then(|t| t.trim().parse::<u32>().ok());
                    match holder {
                        Some(h) if is_alive(h) => {
                            return Err(format!(
                                "Highlight Scout is running (pid {h}); quit it first"
                            ))
                        }
                        // Stale (dead pid) or unreadable: take it over.
                        _ => {
                            let _ = fs::remove_file(path);
                        }
                    }
                }
                Err(e) => return Err(format!("cannot create lock {}: {e}", path.display())),
            }
        }
        Err(format!("could not take the lock {}", path.display()))
    }
}

impl Drop for AppLock {
    fn drop(&mut self) {
        // Remove only our own lock (never one a later owner took over).
        let ours = fs::read_to_string(&self.path)
            .ok()
            .and_then(|t| t.trim().parse::<u32>().ok())
            == Some(self.pid);
        if ours {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lock_path(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hs-lock-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("highlight-scout.lock")
    }

    #[test]
    fn a_held_lock_refuses_a_second_owner_and_frees_on_drop() {
        let p = lock_path("held");
        let first = AppLock::acquire(&p, |_| true).unwrap();
        let err = AppLock::acquire(&p, |_| true).unwrap_err();
        assert!(err.contains("is running"), "{err}");
        drop(first);
        assert!(!p.exists());
        assert!(AppLock::acquire(&p, |_| true).is_ok());
    }

    #[test]
    fn a_stale_lock_from_a_dead_pid_is_taken_over() {
        let p = lock_path("stale");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        fs::write(&p, "999999").unwrap();
        let lock = AppLock::acquire(&p, |pid| pid != 999999).unwrap();
        assert_eq!(
            fs::read_to_string(&p).unwrap(),
            std::process::id().to_string()
        );
        drop(lock);
    }

    #[test]
    fn this_process_counts_as_alive() {
        assert!(pid_alive(std::process::id()));
    }
}
