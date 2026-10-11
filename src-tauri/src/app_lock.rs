//! "This process may write to the archive and the index": an OS advisory
//! lock (`flock`, through `std::fs::File::try_lock`) on a file in the app
//! data dir, held for the life of the process. The app takes it at start;
//! the headless import and the command-line duplicate merge take it too, so
//! only one Highlight Scout process writes at a time.
//!
//! The kernel does the excluding, and releases the lock when the holder's
//! file handle closes, however the process ends: there is no stale lock to
//! detect and nothing to take over. The lock file itself is never removed
//! (removing it would let a later process lock a different file of the same
//! name). The holder's pid is written into it for the refusal message only;
//! nothing decides on it.

use std::fs;
use std::io::{Seek, SeekFrom, Write};
use std::path::Path;

#[derive(Debug)]
pub struct AppLock {
    /// Holds the lock; closing it releases it.
    _file: fs::File,
}

impl AppLock {
    /// Take the lock at `path`, or say that another process holds it.
    pub fn acquire(path: &Path) -> Result<AppLock, String> {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(path)
            .map_err(|e| format!("cannot open lock {}: {e}", path.display()))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(fs::TryLockError::WouldBlock) => {
                let holder = fs::read_to_string(path)
                    .ok()
                    .and_then(|t| t.trim().parse::<u32>().ok())
                    .map(|pid| format!(" (pid {pid})"))
                    .unwrap_or_default();
                return Err(format!("Highlight Scout is running{holder}; quit it first"));
            }
            Err(fs::TryLockError::Error(e)) => {
                return Err(format!("cannot lock {}: {e}", path.display()))
            }
        }
        // For the message another process shows; failure to write it changes
        // nothing about who holds the lock.
        let _ = file
            .set_len(0)
            .and_then(|_| file.seek(SeekFrom::Start(0)))
            .and_then(|_| file.write_all(std::process::id().to_string().as_bytes()));
        Ok(AppLock { _file: file })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn lock_path(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("hs-lock-{}-{}", name, std::process::id()));
        let _ = fs::remove_dir_all(&d);
        d.join("highlight-scout.lock")
    }

    #[test]
    fn a_second_handle_cannot_take_the_lock_while_the_first_holds_it() {
        let p = lock_path("held");
        let first = AppLock::acquire(&p).unwrap();
        // Each acquire opens the file afresh, as a second process would.
        for _ in 0..3 {
            let err = AppLock::acquire(&p).unwrap_err();
            assert!(err.contains("is running"), "{err}");
            assert!(err.contains(&std::process::id().to_string()), "{err}");
        }
        drop(first);
        let second = AppLock::acquire(&p).unwrap();
        assert!(AppLock::acquire(&p).is_err());
        drop(second);
    }

    #[test]
    fn a_lock_file_left_behind_does_not_lock_anyone_out() {
        // The r2 finding's opening: a lock file that exists and is empty,
        // or names a live pid, while nobody holds the lock.
        let p = lock_path("leftover");
        fs::create_dir_all(p.parent().unwrap()).unwrap();
        for leftover in [String::new(), std::process::id().to_string()] {
            fs::write(&p, &leftover).unwrap();
            let lock = AppLock::acquire(&p).unwrap();
            assert!(AppLock::acquire(&p).is_err(), "one owner only");
            assert_eq!(
                fs::read_to_string(&p).unwrap(),
                std::process::id().to_string()
            );
            drop(lock);
            assert!(p.exists(), "the lock file is never removed");
        }
    }

    #[test]
    fn many_takers_at_once_get_exactly_one_lock() {
        let p = lock_path("race");
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (p, barrier) = (p.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    AppLock::acquire(&p).ok()
                })
            })
            .collect();
        let held: Vec<AppLock> = handles
            .into_iter()
            .filter_map(|h| h.join().unwrap())
            .collect();
        assert_eq!(held.len(), 1);
    }
}
