//! One writer at a time over the archive and index.
//!
//! Every operation that writes Readwise/Zotero work files or rewrites them
//! (a manual import, the scheduled pass, the duplicate merge) must hold a
//! `Claim`. Claiming is atomic: whoever finds it taken is refused with a
//! message naming what is running. The scheduled pass takes one claim and
//! holds it across all its sources; the per-source steps take `&Claim` as
//! proof, so they cannot run unclaimed.

use std::sync::Mutex;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    ReadwiseImport,
    TweetsImport,
    ZoteroImport,
    SyncPass,
    MergeDuplicates,
}

impl Op {
    fn label(self) -> &'static str {
        match self {
            Op::ReadwiseImport => "a Readwise import",
            Op::TweetsImport => "a saved-tweets import",
            Op::ZoteroImport => "a Zotero import",
            Op::SyncPass => "a sync",
            Op::MergeDuplicates => "the duplicate merge",
        }
    }
}

#[derive(Debug, Default)]
pub struct BusyLock {
    holder: Mutex<Option<Op>>,
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
    pub fn try_claim(&self, op: Op) -> Result<Claim<'_>, String> {
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
