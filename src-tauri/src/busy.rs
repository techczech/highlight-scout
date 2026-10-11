//! One writer at a time over the archive and index.
//!
//! Every operation that writes Readwise/Zotero work files or rewrites them
//! (a manual import, the scheduled pass, the duplicate merge) must hold a
//! `Claim`. Claiming is atomic: whoever finds it taken is refused with a
//! message naming what is running. The scheduled pass takes one claim and
//! holds it across all its sources; the per-source steps take `&Claim` as
//! proof, so they cannot run unclaimed.
//!
//! A claim is also the proof that this process may write at all: an app
//! that started without the app lock (`app_lock`; another Highlight Scout
//! process holds it) is given a `BusyLock::read_only()`, which refuses every
//! claim. Reading and search need no claim and go on working.

use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    ReadwiseImport,
    TweetsImport,
    FileImport,
    ZoteroImport,
    SyncPass,
    MergeDuplicates,
}

impl Op {
    fn label(self) -> &'static str {
        match self {
            Op::ReadwiseImport => "a Readwise import",
            Op::TweetsImport => "a saved-tweets import",
            Op::FileImport => "a file import",
            Op::ZoteroImport => "a Zotero import",
            Op::SyncPass => "a sync",
            Op::MergeDuplicates => "the duplicate merge",
        }
    }
}

/// What every write is refused with in a process without the app lock.
pub const NOT_THE_WRITER: &str = "Another Highlight Scout process is writing to the archive";

#[derive(Debug, Default)]
pub struct BusyLock {
    holder: Mutex<Option<Op>>,
    read_only: bool,
}

/// Held for the whole operation; released on drop (panic-safe).
#[derive(Debug)]
pub struct Claim<'a> {
    lock: &'a BusyLock,
    op: Op,
}

impl Claim<'_> {
    pub fn op(&self) -> Op {
        self.op
    }
}

impl BusyLock {
    /// For a process that does not hold the app lock: no claim is granted.
    pub fn read_only() -> BusyLock {
        BusyLock {
            holder: Mutex::new(None),
            read_only: true,
        }
    }

    /// Ok when this process may write to the archive and the index. For the
    /// few writers that take no claim (OCR text, the R2 restore).
    pub fn check_writer(&self) -> Result<(), String> {
        if self.read_only {
            return Err(NOT_THE_WRITER.to_string());
        }
        Ok(())
    }

    pub fn try_claim(&self, op: Op) -> Result<Claim<'_>, String> {
        self.check_writer()?;
        let mut holder = self.holder.lock().unwrap_or_else(|p| p.into_inner());
        if let Some(running) = *holder {
            return Err(format!(
                "Can't start {}: {} is running. Try again when it finishes.",
                op.label(),
                running.label()
            ));
        }
        *holder = Some(op);
        Ok(Claim { lock: self, op })
    }

    pub fn is_busy(&self) -> bool {
        self.holder
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .is_some()
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        *self.lock.holder.lock().unwrap_or_else(|p| p.into_inner()) = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_merge_is_refused_during_an_import_and_an_import_during_a_merge() {
        let lock = BusyLock::default();
        let import = lock.try_claim(Op::ReadwiseImport).unwrap();
        let err = lock.try_claim(Op::MergeDuplicates).unwrap_err();
        assert!(err.contains("a Readwise import is running"), "{err}");
        drop(import);

        let merge = lock.try_claim(Op::MergeDuplicates).unwrap();
        let err = lock.try_claim(Op::ReadwiseImport).unwrap_err();
        assert!(err.contains("the duplicate merge is running"), "{err}");
        assert!(lock.try_claim(Op::SyncPass).is_err());
        drop(merge);
        assert!(!lock.is_busy());
    }

    #[test]
    fn a_process_without_the_app_lock_is_refused_every_write() {
        // The app as it starts while another process holds the app lock.
        let p = std::env::temp_dir().join(format!(
            "hs-busy-nolock-{}/highlight-scout.lock",
            std::process::id()
        ));
        let _other_process = crate::app_lock::AppLock::acquire(&p).unwrap();
        let busy = crate::writer_gate(&crate::app_lock::AppLock::acquire(&p));
        for op in [
            Op::ReadwiseImport,
            Op::TweetsImport,
            Op::FileImport,
            Op::ZoteroImport,
            Op::SyncPass,
            Op::MergeDuplicates,
        ] {
            assert_eq!(busy.try_claim(op).unwrap_err(), NOT_THE_WRITER);
        }
        assert_eq!(busy.check_writer().unwrap_err(), NOT_THE_WRITER);
        assert!(!busy.is_busy());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());

        // With the lock, claims are granted as before.
        let p = p.with_file_name("free.lock");
        let lock = crate::app_lock::AppLock::acquire(&p);
        assert!(crate::writer_gate(&lock).try_claim(Op::SyncPass).is_ok());
        let _ = std::fs::remove_dir_all(p.parent().unwrap());
    }

    #[test]
    fn claims_race_to_exactly_one_winner() {
        let lock = std::sync::Arc::new(BusyLock::default());
        let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
        let handles: Vec<_> = (0..8)
            .map(|_| {
                let (lock, barrier) = (lock.clone(), barrier.clone());
                std::thread::spawn(move || {
                    barrier.wait();
                    let won = lock.try_claim(Op::SyncPass).map(std::mem::forget).is_ok();
                    won as usize
                })
            })
            .collect();
        let winners: usize = handles.into_iter().map(|h| h.join().unwrap()).sum();
        assert_eq!(winners, 1);
    }
}
