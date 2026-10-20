//! The settings file on disk (port of node/settings-file.ts). Every change: lock, read again, run the step on what is on disk, write
//! (O_EXCL temp 0600, fsync, rename, fsync folder), unlock. Refuses a folder or file that is a link or was swapped (containment, folder
//! identity and lock checks before each change to the folder). A file that cannot be used is set aside as `.damaged-<ISO time>`.
//!
//! Threat model, as the TS package states it: the checks guard against a folder or file that is a link, or was swapped, before an
//! operation (a misconfigured or stale link). A path-based call can still land elsewhere if the folder is swapped between its check and
//! the call; only a process running as the same user can do that, and it could write these files directly, so that race is accepted.

use crate::lock::{acquire_lock, pid_alive, temp_path_of, Lock, LockError, LOCK_LEASE_MS, LOCK_WAIT_MS};
use crate::value::{iso, now_ms, pretty};
use serde_json::Value;
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{DirBuilderExt, MetadataExt, OpenOptionsExt, PermissionsExt};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

pub const MAX_FILE_BYTES: u64 = 1024 * 1024;
pub const TEMP_MAX_AGE_MS: f64 = 60_000.0;
pub const DAMAGED_KEPT: usize = 3;

pub const MSG_LINKED: &str = "Settings cannot be saved: the settings folder or the settings file is a link, not a plain file";
pub const MSG_OUTSIDE: &str = "Settings cannot be saved: the settings file is no longer inside the app's data folder";
pub const MSG_UNWRITTEN: &str = "Settings could not be written to disk";
pub const MSG_LOCKED: &str = "The settings file is busy: another app or agent is changing it. Try again in a moment";
pub const MSG_CLOSED: &str = "Settings are closed; this change was not saved";
pub const MSG_TOO_LARGE: &str = "Too many settings changes to save";
pub const MSG_UNARCHIVED: &str = "Settings cannot be saved: the damaged settings file could not be set aside, so it is left as it is";

#[derive(Debug, Clone, PartialEq)]
pub enum FileRead {
    Missing,
    Ok(Value),
    /// Not JSON, too large, or not a file this owner accepts. In `update`, where it was set aside.
    Damaged(Option<PathBuf>),
    /// A link, a FIFO, a folder outside the root: neither read nor written.
    Refused(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct FileError {
    /// `locked`, `refused`, `unwritten`, `closed`, `too-large`.
    pub code: &'static str,
    pub error: String,
    pub unarchived: bool,
}

fn ferr(code: &'static str, error: &str) -> FileError {
    FileError { code, error: error.into(), unarchived: false }
}

pub enum Bounded {
    Ok(String),
    Missing,
    TooLarge,
    NotFile,
    Error,
}

/// Reads a regular file of at most `cap` bytes without following a link.
pub fn read_bounded(path: &Path, cap: u64) -> Bounded {
    let st = match fs::symlink_metadata(path) {
        Ok(s) => s,
        Err(e) if e.kind() == ErrorKind::NotFound => return Bounded::Missing,
        Err(_) => return Bounded::Error,
    };
    if !st.is_file() {
        return Bounded::NotFile;
    }
    if st.len() > cap {
        return Bounded::TooLarge;
    }
    let mut f = match OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path) {
        Ok(f) => f,
        Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Bounded::NotFile,
        Err(e) if e.kind() == ErrorKind::NotFound => return Bounded::Missing,
        Err(_) => return Bounded::Error,
    };
    match f.metadata() {
        Ok(m) if !m.is_file() => return Bounded::NotFile,
        Ok(m) if m.len() > cap => return Bounded::TooLarge,
        Err(_) => return Bounded::Error,
        _ => {}
    }
    let mut buf = Vec::new();
    if Read::by_ref(&mut f).take(cap + 1).read_to_end(&mut buf).is_err() {
        return Bounded::Error;
    }
    if buf.len() as u64 > cap {
        return Bounded::TooLarge;
    }
    Bounded::Ok(String::from_utf8_lossy(&buf).to_string())
}

#[derive(Clone, Copy, PartialEq)]
struct Folder {
    dev: u64,
    ino: u64,
}

enum Guard {
    Clear,
    Refused(String),
    Lost,
}

pub type AcceptFn = Box<dyn Fn(&Value) -> bool + Send + Sync>;

pub struct SettingsFile {
    pub path: PathBuf,
    dir: PathBuf,
    root: PathBuf,
    root_real: Mutex<Option<PathBuf>>,
    max_bytes: u64,
    pub wait_ms: u64,
    pub lease_ms: u64,
    accept: AcceptFn,
    closed: Mutex<Option<String>>,
    queue: Mutex<()>,
}

/// The result of one update step: what to write (None = nothing) and what to return.
pub struct Step<T> {
    pub write: Option<Value>,
    pub result: T,
}

impl SettingsFile {
    pub fn new(path: PathBuf, root: Option<PathBuf>, accept: AcceptFn) -> SettingsFile {
        let dir = path.parent().map(Path::to_path_buf).unwrap_or_else(|| PathBuf::from("."));
        SettingsFile {
            root: root.unwrap_or_else(|| dir.clone()),
            dir,
            path,
            root_real: Mutex::new(None),
            max_bytes: MAX_FILE_BYTES,
            wait_ms: LOCK_WAIT_MS,
            lease_ms: LOCK_LEASE_MS,
            accept,
            closed: Mutex::new(None),
            queue: Mutex::new(()),
        }
    }

    pub fn max_bytes(&self) -> u64 {
        self.max_bytes
    }

    pub fn refuse_writes(&self, reason: Option<&str>) {
        *self.closed.lock().unwrap_or_else(|e| e.into_inner()) = Some(reason.unwrap_or(MSG_CLOSED).to_string());
    }

    pub fn read(&self) -> FileRead {
        if let Some(e) = self.containment(false) {
            return FileRead::Refused(e);
        }
        match read_bounded(&self.path, self.max_bytes) {
            Bounded::Missing => FileRead::Missing,
            Bounded::NotFile => FileRead::Refused(MSG_LINKED.into()),
            Bounded::Error => FileRead::Refused(MSG_UNWRITTEN.into()),
            Bounded::TooLarge => FileRead::Damaged(None),
            Bounded::Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(raw) if (self.accept)(&raw) => FileRead::Ok(raw),
                _ => FileRead::Damaged(None),
            },
        }
    }

    /// A read that waits for queued updates in this process.
    pub fn read_in_order(&self) -> FileRead {
        let _q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        self.read()
    }

    fn real_root(&self) -> std::io::Result<PathBuf> {
        let mut g = self.root_real.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(r) = g.as_ref() {
            return Ok(r.clone());
        }
        let r = fs::canonicalize(&self.root)?;
        *g = Some(r.clone());
        Ok(r)
    }

    /// The folder is not a link and resolves to the root; the file, if there, is a regular file.
    fn containment(&self, created: bool) -> Option<String> {
        match fs::symlink_metadata(&self.dir) {
            Ok(st) if st.file_type().is_symlink() => return Some(MSG_LINKED.into()),
            Ok(_) => {}
            Err(e) if !created && e.kind() == ErrorKind::NotFound => return None,
            Err(_) => return Some(MSG_OUTSIDE.into()),
        }
        let real = match (self.real_root(), fs::canonicalize(&self.dir)) {
            (Ok(r), Ok(d)) => r == d,
            (Err(e), _) | (_, Err(e)) if !created && e.kind() == ErrorKind::NotFound => return None,
            _ => false,
        };
        if !real {
            return Some(MSG_OUTSIDE.into());
        }
        if let Ok(cur) = fs::symlink_metadata(&self.path) {
            if !cur.is_file() {
                return Some(MSG_LINKED.into());
            }
        }
        None
    }

    fn folder_now(&self) -> Option<Folder> {
        let st = fs::symlink_metadata(&self.dir).ok()?;
        if !st.is_dir() {
            return None;
        }
        let real = fs::canonicalize(&self.dir).ok()?;
        if Some(real) != self.root_real.lock().ok()?.clone() {
            return None;
        }
        Some(Folder { dev: st.dev(), ino: st.ino() })
    }

    fn place(&self, folder: Folder) -> Option<String> {
        self.containment(true).or_else(|| if self.folder_now() == Some(folder) { None } else { Some(MSG_OUTSIDE.into()) })
    }

    fn guard(&self, folder: Folder, lock: &Lock) -> Guard {
        match self.place(folder) {
            Some(e) => Guard::Refused(e),
            None if lock.held() => Guard::Clear,
            None => Guard::Lost,
        }
    }

    /// Runs `step` on what is on disk, under the lock; writes what it returns. May run `step` more than once (a lost lock retries).
    pub fn update<T>(&self, mut step: impl FnMut(&FileRead) -> Step<T>) -> Result<(T, bool), (FileError, Option<T>)> {
        let _q = self.queue.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(c) = self.closed.lock().unwrap_or_else(|e| e.into_inner()).clone() {
            return Err((FileError { code: "closed", error: c, unarchived: false }, None));
        }
        if let Some(e) = self.containment(false) {
            return Err((ferr("refused", &e), None));
        }
        if fs::DirBuilder::new().recursive(true).mode(0o700).create(&self.dir).is_err() {
            return Err((ferr("unwritten", MSG_UNWRITTEN), None));
        }
        let until = now_ms() + self.wait_ms as f64;
        let mut recovered: Option<PathBuf> = None;
        loop {
            if let Some(e) = self.containment(true) {
                return Err((ferr("refused", &e), None));
            }
            let Some(folder) = self.folder_now() else { return Err((ferr("refused", MSG_OUTSIDE), None)) };
            let folder_ok = || self.folder_now() == Some(folder);
            let wait = (until - now_ms()).max(0.0) as u64;
            let lock = match acquire_lock(&self.path, wait, self.lease_ms, &folder_ok) {
                Ok(l) => l,
                Err(LockError::Timeout) => return Err((ferr("locked", MSG_LOCKED), None)),
                Err(LockError::Refused) => return Err((ferr("refused", &self.place(folder).unwrap_or_else(|| MSG_OUTSIDE.into())), None)),
                Err(LockError::Io(_)) => return Err((ferr("unwritten", MSG_UNWRITTEN), None)),
            };
            let out = self.under_lock(folder, &lock, &mut recovered, &mut step);
            lock.release(&folder_ok);
            match out {
                Attempt::Done(r) => return r,
                Attempt::Lost(result) => {
                    if now_ms() >= until {
                        return Err((ferr("locked", MSG_LOCKED), result));
                    }
                }
            }
        }
    }

    fn under_lock<T>(&self, folder: Folder, lock: &Lock, recovered: &mut Option<PathBuf>, step: &mut impl FnMut(&FileRead) -> Step<T>) -> Attempt<T> {
        match self.sweep_now(&|| self.guard(folder, lock)) {
            Guard::Lost => return Attempt::Lost(None),
            Guard::Refused(e) => return Attempt::Done(Err((ferr("refused", &e), None))),
            Guard::Clear => {}
        }
        let mut read = self.read();
        if read == FileRead::Missing && recovered.is_some() {
            read = FileRead::Damaged(recovered.clone());
        } else if matches!(read, FileRead::Damaged(_)) {
            match self.set_aside(folder, lock) {
                SetAside::Lost => return Attempt::Lost(None),
                SetAside::Refused(e) => return Attempt::Done(Err((ferr("refused", &e), None))),
                SetAside::Unarchived => return Attempt::Done(Err((FileError { code: "refused", error: MSG_UNARCHIVED.into(), unarchived: true }, None))),
                SetAside::Moved(to) => {
                    *recovered = Some(to.clone());
                    read = FileRead::Damaged(Some(to));
                }
            }
        } else {
            *recovered = None;
        }
        let s = step(&read);
        let Some(write) = s.write else { return Attempt::Done(Ok((s.result, false))) };
        if let FileRead::Refused(e) = &read {
            return Attempt::Done(Err((ferr("refused", e), Some(s.result))));
        }
        let text = pretty(&write);
        if text.len() as u64 > self.max_bytes {
            return Attempt::Done(Err((ferr("too-large", MSG_TOO_LARGE), Some(s.result))));
        }
        match self.write(&text, folder, lock) {
            WriteOut::Lost => Attempt::Lost(Some(s.result)),
            WriteOut::Err(code, e) => Attempt::Done(Err((ferr(code, &e), Some(s.result)))),
            WriteOut::Ok => Attempt::Done(Ok((s.result, true))),
        }
    }

    /// Removes temporary files left by dead writers, if the lock is free at once.
    pub fn sweep(&self) {
        if self.containment(false).is_some() || fs::symlink_metadata(&self.dir).is_err() || self.containment(true).is_some() {
            return;
        }
        let Some(folder) = self.folder_now() else { return };
        let folder_ok = || self.folder_now() == Some(folder);
        let Ok(lock) = acquire_lock(&self.path, 0, self.lease_ms, &folder_ok) else { return };
        let _ = self.sweep_now(&|| self.guard(folder, &lock));
        lock.release(&folder_ok);
    }

    fn sweep_now(&self, guard: &dyn Fn() -> Guard) -> Guard {
        let base = self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let prefix = format!(".{base}.");
        let Ok(rd) = fs::read_dir(&self.dir) else { return Guard::Clear };
        for ent in rd.flatten() {
            let n = ent.file_name().to_string_lossy().to_string();
            let Some(rest) = n.strip_prefix(&prefix).and_then(|r| r.strip_suffix(".tmp")) else { continue };
            let Some((pid, hex)) = rest.split_once('.') else { continue };
            if pid.is_empty() || !pid.chars().all(|c| c.is_ascii_digit()) || hex.is_empty() || !hex.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()) {
                continue;
            }
            let p = self.dir.join(&n);
            let Ok(st) = fs::symlink_metadata(&p) else { continue };
            if !st.is_file() {
                continue;
            }
            let age = now_ms() - (st.mtime() as f64 * 1000.0 + st.mtime_nsec() as f64 / 1e6);
            if pid_alive(pid.parse().unwrap_or(0)) && age <= TEMP_MAX_AGE_MS {
                continue;
            }
            match guard() {
                Guard::Clear => {}
                g => return g,
            }
            let _ = fs::remove_file(&p);
        }
        Guard::Clear
    }

    fn set_aside(&self, folder: Folder, lock: &Lock) -> SetAside {
        let stamp = iso(now_ms()).replace([':', '.'], "-");
        for i in 0..100 {
            let mut name = self.path.as_os_str().to_owned();
            name.push(format!(".damaged-{stamp}{}", if i > 0 { format!("-{i}") } else { String::new() }));
            let to = PathBuf::from(name);
            if fs::symlink_metadata(&to).is_ok() {
                continue;
            }
            let Ok(was) = fs::symlink_metadata(&self.path) else { return SetAside::Unarchived };
            if !was.is_file() {
                return SetAside::Unarchived;
            }
            match self.guard(folder, lock) {
                Guard::Lost => return SetAside::Lost,
                Guard::Refused(e) => return SetAside::Refused(e),
                Guard::Clear => {}
            }
            if fs::rename(&self.path, &to).is_err() {
                return SetAside::Unarchived;
            }
            if self.folder_now() != Some(folder) {
                return SetAside::Refused(MSG_OUTSIDE.into());
            }
            match fs::symlink_metadata(&to) {
                Ok(m) if m.dev() == was.dev() && m.ino() == was.ino() => {}
                _ => return SetAside::Refused(MSG_OUTSIDE.into()),
            }
            if let Ok(f) = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(&to) {
                if let Ok(st) = f.metadata() {
                    if st.is_file() && st.dev() == was.dev() && st.ino() == was.ino() && self.folder_now() == Some(folder) {
                        let _ = f.set_permissions(fs::Permissions::from_mode(0o600));
                    }
                }
            }
            self.prune_damaged(folder, lock);
            return SetAside::Moved(to);
        }
        SetAside::Unarchived
    }

    fn prune_damaged(&self, folder: Folder, lock: &Lock) {
        let base = self.path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let prefix = format!("{base}.damaged-");
        let Ok(rd) = fs::read_dir(&self.dir) else { return };
        let mut ours: Vec<(String, String, u32)> = rd
            .flatten()
            .filter_map(|e| {
                let n = e.file_name().to_string_lossy().to_string();
                let rest = n.strip_prefix(&prefix)?.to_string();
                let (stamp, idx) = match rest.get(24..) {
                    Some("") => (rest.clone(), 0),
                    Some(tail) => (rest[..24].to_string(), tail.strip_prefix('-').filter(|t| (1..=2).contains(&t.len()))?.parse().ok()?),
                    None => return None,
                };
                damaged_stamp_ok(&stamp).then_some((n, stamp, idx))
            })
            .collect();
        ours.sort_by(|a, b| b.1.cmp(&a.1).then(b.2.cmp(&a.2)));
        for (n, _, _) in ours.into_iter().skip(DAMAGED_KEPT) {
            let p = self.dir.join(&n);
            if !fs::symlink_metadata(&p).is_ok_and(|m| m.is_file()) {
                continue;
            }
            if !matches!(self.guard(folder, lock), Guard::Clear) {
                return;
            }
            let _ = fs::remove_file(&p);
        }
    }

    fn write(&self, text: &str, folder: Folder, lock: &Lock) -> WriteOut {
        let name = temp_path_of(&self.path);
        match self.guard(folder, lock) {
            Guard::Lost => return WriteOut::Lost,
            Guard::Refused(e) => return WriteOut::Err("refused", e),
            Guard::Clear => {}
        }
        let mut fh = match OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&name) {
            Ok(f) => f,
            Err(_) => return WriteOut::Err("unwritten", MSG_UNWRITTEN.into()),
        };
        let mut renamed = false;
        let out = (|| {
            if fh.write_all(text.as_bytes()).is_err() || fh.sync_all().is_err() {
                return WriteOut::Err("unwritten", MSG_UNWRITTEN.into());
            }
            let Ok(written) = fh.metadata() else { return WriteOut::Err("unwritten", MSG_UNWRITTEN.into()) };
            match self.guard(folder, lock) {
                Guard::Lost => return WriteOut::Lost,
                Guard::Refused(e) => return WriteOut::Err("refused", e),
                Guard::Clear => {}
            }
            if fs::rename(&name, &self.path).is_err() {
                return WriteOut::Err("unwritten", MSG_UNWRITTEN.into());
            }
            renamed = true;
            if self.folder_now() != Some(folder) {
                return WriteOut::Err("refused", MSG_OUTSIDE.into());
            }
            match fs::symlink_metadata(&self.path) {
                Ok(m) if m.dev() == written.dev() && m.ino() == written.ino() => {}
                _ => return WriteOut::Err("refused", MSG_OUTSIDE.into()),
            }
            if sync_dir(&self.dir).is_err() {
                return WriteOut::Err("unwritten", MSG_UNWRITTEN.into());
            }
            WriteOut::Ok
        })();
        if !renamed && self.folder_now() == Some(folder) {
            let _ = fs::remove_file(&name);
        }
        out
    }
}

fn damaged_stamp_ok(s: &str) -> bool {
    // YYYY-MM-DDTHH-MM-SS-mmmZ
    let b = s.as_bytes();
    b.len() == 24
        && b.iter().enumerate().all(|(i, &c)| match i {
            4 | 7 | 13 | 16 | 19 => c == b'-',
            10 => c == b'T',
            23 => c == b'Z',
            _ => c.is_ascii_digit(),
        })
}

enum Attempt<T> {
    Done(Result<(T, bool), (FileError, Option<T>)>),
    Lost(Option<T>),
}

enum SetAside {
    Moved(PathBuf),
    Refused(String),
    Unarchived,
    Lost,
}

enum WriteOut {
    Ok,
    Lost,
    Err(&'static str, String),
}

/// fsync a folder; a platform that cannot is not an error.
pub fn sync_dir(dir: &Path) -> std::io::Result<()> {
    let f = match fs::File::open(dir) {
        Ok(f) => f,
        Err(e) if matches!(e.raw_os_error(), Some(libc::EISDIR | libc::EPERM | libc::EACCES | libc::EINVAL | libc::ENOTSUP)) => return Ok(()),
        Err(e) => return Err(e),
    };
    match f.sync_all() {
        Ok(()) => Ok(()),
        Err(e) if matches!(e.raw_os_error(), Some(libc::EINVAL | libc::ENOTSUP)) => Ok(()),
        Err(e) => Err(e),
    }
}

/// A folder pinned at one moment (not a link, its real path, its device and inode), for writes beside the settings file that are not
/// the settings file (an app's legacy file, a migration stamp). Every read and write checks the folder is still the pinned one first,
/// and a write checks again before its rename and after it (the threat model above: a link or a swapped folder before an operation is
/// refused; a same-user race between a check and its call is accepted).
#[derive(Debug, Clone)]
pub struct FolderGuard {
    dir: PathBuf,
    real: PathBuf,
    id: (u64, u64),
}

/// A plain file read through a [`FolderGuard`]: its text, its modification time (ms since 1970) and its identity.
#[derive(Debug, Clone, PartialEq)]
pub struct GuardedRead {
    pub text: String,
    pub mtime_ms: f64,
    pub id: (u64, u64),
}

impl FolderGuard {
    /// Pins `dir` as it is now; refuses a link, a missing folder or anything that is not a folder.
    pub fn pin(dir: &Path) -> Result<FolderGuard, String> {
        let st = fs::symlink_metadata(dir).map_err(|_| MSG_OUTSIDE.to_string())?;
        if st.file_type().is_symlink() {
            return Err(MSG_LINKED.into());
        }
        if !st.is_dir() {
            return Err(MSG_OUTSIDE.into());
        }
        let real = fs::canonicalize(dir).map_err(|_| MSG_OUTSIDE.to_string())?;
        Ok(FolderGuard { dir: dir.to_path_buf(), real, id: (st.dev(), st.ino()) })
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The folder is still the pinned one: not a link, the same real path, the same device and inode.
    pub fn check(&self) -> Result<(), String> {
        let st = fs::symlink_metadata(&self.dir).map_err(|_| MSG_OUTSIDE.to_string())?;
        if st.file_type().is_symlink() {
            return Err(MSG_LINKED.into());
        }
        if !st.is_dir() || (st.dev(), st.ino()) != self.id {
            return Err(MSG_OUTSIDE.into());
        }
        match fs::canonicalize(&self.dir) {
            Ok(r) if r == self.real => Ok(()),
            _ => Err(MSG_OUTSIDE.into()),
        }
    }

    fn child(&self, name: &str) -> Result<PathBuf, String> {
        if name.is_empty() || name == "." || name == ".." || name.contains('/') || name.contains('\0') {
            return Err(MSG_OUTSIDE.into());
        }
        Ok(self.dir.join(name))
    }

    /// Reads the plain file `name` in the folder without following a link. None when there is no such file; an error for a link, a
    /// FIFO or anything else that is not a plain file, or when the folder is no longer the pinned one.
    pub fn read(&self, name: &str) -> Result<Option<GuardedRead>, String> {
        let path = self.child(name)?;
        self.check()?;
        let mut f = match OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == ErrorKind::NotFound => return Ok(None),
            Err(e) if e.raw_os_error() == Some(libc::ELOOP) => return Err(MSG_LINKED.into()),
            Err(_) => return Err(MSG_UNWRITTEN.into()),
        };
        let m = f.metadata().map_err(|_| MSG_UNWRITTEN.to_string())?;
        if !m.is_file() {
            return Err(MSG_LINKED.into());
        }
        let mut buf = Vec::new();
        f.read_to_end(&mut buf).map_err(|_| MSG_UNWRITTEN.to_string())?;
        self.check()?;
        Ok(Some(GuardedRead { text: String::from_utf8_lossy(&buf).to_string(), mtime_ms: m.mtime() as f64 * 1000.0 + m.mtime_nsec() as f64 / 1e6, id: (m.dev(), m.ino()) }))
    }

    /// Writes `name` in the folder atomically (O_EXCL temp 0600 without following links, fsync, rename, fsync folder). With `expect`,
    /// the file must still read as exactly that text just before the rename (compare and swap): an edit made in between is never
    /// overwritten; the write is refused instead. An existing `name` that is a link or not a plain file is refused.
    pub fn write(&self, name: &str, text: &str, expect: Option<&GuardedRead>) -> Result<(), String> {
        let path = self.child(name)?;
        self.check()?;
        if let Ok(cur) = fs::symlink_metadata(&path) {
            if !cur.is_file() {
                return Err(MSG_LINKED.into());
            }
        }
        let tmp = temp_path_of(&path);
        let mut fh = OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&tmp).map_err(|_| MSG_UNWRITTEN.to_string())?;
        let out = (|| {
            fh.write_all(text.as_bytes()).and_then(|_| fh.sync_all()).map_err(|_| MSG_UNWRITTEN.to_string())?;
            let written = fh.metadata().map_err(|_| MSG_UNWRITTEN.to_string())?;
            if let Some(want) = expect {
                match self.read(name)? {
                    Some(now) if now.text == want.text && now.id == want.id => {}
                    _ => return Err(MSG_CHANGED.to_string()),
                }
            }
            self.check()?;
            fs::rename(&tmp, &path).map_err(|_| MSG_UNWRITTEN.to_string())?;
            self.check()?;
            match fs::symlink_metadata(&path) {
                Ok(m) if m.dev() == written.dev() && m.ino() == written.ino() => {}
                _ => return Err(MSG_OUTSIDE.to_string()),
            }
            sync_dir(&self.dir).map_err(|_| MSG_UNWRITTEN.to_string())
        })();
        if out.is_err() && self.check().is_ok() {
            let _ = fs::remove_file(&tmp);
        }
        out
    }
}

pub const MSG_CHANGED: &str = "The file changed while it was being updated, so it was left as it is";
