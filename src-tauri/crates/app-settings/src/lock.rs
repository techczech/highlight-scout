//! The cross-process lock on a settings file (port of node/lock.ts): `<file>.lock` holding `{ pid, at, nonce, host }`, taken by
//! hard-linking a private metadata file (atomic, never a lock without its metadata). Stale when its process is dead (same host) or older
//! than the 30 s lease. Broken by renaming it to a private name and checking it is the lock judged; a lost race deletes the copy and
//! never puts a lock back. Every change the lock makes to the folder first asks `guard` (the folder is still the one checked).

use crate::value::{now_ms, rand_hex};
use serde_json::{json, Value};
use std::fs::{self, OpenOptions};
use std::io::{ErrorKind, Read, Write};
use std::os::unix::fs::{MetadataExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::time::Duration;

pub const LOCK_WAIT_MS: u64 = 5_000;
pub const LOCK_LEASE_MS: u64 = 30_000;
const MAX_LOCK_BYTES: u64 = 4096;

#[derive(Debug)]
pub enum LockError {
    Timeout,
    /// The folder failed the guard.
    Refused,
    Io(std::io::Error),
}

pub fn lock_path_of(file: &Path) -> PathBuf {
    let mut s = file.as_os_str().to_owned();
    s.push(".lock");
    PathBuf::from(s)
}

/// `.<name>.<pid>.<12 hex>.tmp` beside the file.
pub fn temp_path_of(file: &Path) -> PathBuf {
    let name = file.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
    file.with_file_name(format!(".{name}.{}.{}.tmp", std::process::id(), rand_hex(6)))
}

pub fn pid_alive(pid: i64) -> bool {
    if pid <= 0 || pid > i32::MAX as i64 {
        return false;
    }
    // SAFETY: kill with signal 0 only checks whether the process exists.
    let r = unsafe { libc::kill(pid as libc::pid_t, 0) };
    r == 0 || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
}

pub fn host_name() -> String {
    let mut buf = [0u8; 256];
    // SAFETY: the buffer is valid for its length; the result is NUL-terminated or truncated.
    let r = unsafe { libc::gethostname(buf.as_mut_ptr() as *mut libc::c_char, buf.len()) };
    if r != 0 {
        return String::new();
    }
    let end = buf.iter().position(|&b| b == 0).unwrap_or(buf.len());
    String::from_utf8_lossy(&buf[..end]).to_string()
}

struct Meta {
    pid: i64,
    at: f64,
    nonce: String,
    host: Option<String>,
}

fn parse_meta(text: &str) -> Option<Meta> {
    let j: Value = serde_json::from_str(text).ok()?;
    let o = j.as_object()?;
    let pid = o.get("pid").and_then(Value::as_f64)? as i64;
    let at = o.get("at").and_then(Value::as_f64).filter(|f| f.is_finite())?;
    let nonce = o.get("nonce")?.as_str()?.to_string();
    let host = o.get("host").and_then(Value::as_str).map(str::to_string);
    Some(Meta { pid, at, nonce, host })
}

struct Seen {
    dev: u64,
    ino: u64,
    mtime_ms: f64,
    text: String,
}

fn look(path: &Path) -> Option<Seen> {
    let st = fs::symlink_metadata(path).ok()?;
    let text = if st.is_file() && st.len() <= MAX_LOCK_BYTES {
        let mut f = OpenOptions::new().read(true).custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK).open(path).ok()?;
        let mut s = String::new();
        let _ = Read::by_ref(&mut f).take(MAX_LOCK_BYTES + 1).read_to_string(&mut s);
        s
    } else {
        String::new()
    };
    let mtime_ms = st.mtime() as f64 * 1000.0 + (st.mtime_nsec() as f64 / 1e6);
    Some(Seen { dev: st.dev(), ino: st.ino(), mtime_ms, text })
}

fn stale(seen: &Seen, lease_ms: u64, now: f64) -> bool {
    let m = parse_meta(&seen.text);
    let at = m.as_ref().map(|m| m.at.min(seen.mtime_ms)).unwrap_or(seen.mtime_ms);
    if now - at > lease_ms as f64 {
        return true;
    }
    match m {
        Some(m) => m.host.as_ref().is_none_or(|h| *h == host_name()) && !pid_alive(m.pid),
        None => false,
    }
}

/// Moves the lock aside and keeps it removed only if `is` says it is the lock meant.
fn remove_lock_if(file: &Path, is: &dyn Fn(&str, u64) -> bool, guard: &dyn Fn() -> bool) -> Result<bool, LockError> {
    let path = lock_path_of(file);
    let aside = temp_path_of(file);
    if !guard() {
        return Ok(false);
    }
    match fs::rename(&path, &aside) {
        Ok(()) => {}
        Err(e) if e.kind() == ErrorKind::NotFound => return Ok(false),
        Err(e) => return Err(LockError::Io(e)),
    }
    let ok = look(&aside).is_some_and(|s| is(&s.text, s.ino));
    if guard() {
        let _ = fs::remove_file(&aside);
    }
    Ok(ok)
}

pub struct Lock {
    file: PathBuf,
    pub nonce: String,
    dev: u64,
    ino: u64,
}

impl Lock {
    /// `<file>.lock` is the very file this holder linked and holds its nonce.
    pub fn held(&self) -> bool {
        look(&lock_path_of(&self.file)).is_some_and(|s| s.dev == self.dev && s.ino == self.ino && parse_meta(&s.text).is_some_and(|m| m.nonce == self.nonce))
    }
    pub fn release(&self, guard: &dyn Fn() -> bool) {
        if !self.held() {
            return;
        }
        let (ino, nonce) = (self.ino, self.nonce.clone());
        let _ = remove_lock_if(&self.file, &move |text, i| i == ino && parse_meta(text).is_some_and(|m| m.nonce == nonce), guard);
    }
}

fn create(file: &Path, guard: &dyn Fn() -> bool) -> Result<Option<Lock>, LockError> {
    let nonce = rand_hex(12);
    let tmp = temp_path_of(file);
    if !guard() {
        return Err(LockError::Refused);
    }
    let mut fh = OpenOptions::new().write(true).create_new(true).mode(0o600).custom_flags(libc::O_NOFOLLOW).open(&tmp).map_err(LockError::Io)?;
    let result = (|| {
        let meta = json!({ "pid": std::process::id(), "at": crate::value::num(now_ms()), "nonce": nonce, "host": host_name() });
        fh.write_all(meta.to_string().as_bytes()).map_err(LockError::Io)?;
        let st = fh.metadata().map_err(LockError::Io)?;
        drop(fh);
        if !guard() {
            return Err(LockError::Refused);
        }
        match fs::hard_link(&tmp, lock_path_of(file)) {
            Ok(()) => Ok(Some(Lock { file: file.to_path_buf(), nonce: nonce.clone(), dev: st.dev(), ino: st.ino() })),
            Err(e) if e.kind() == ErrorKind::AlreadyExists => Ok(None),
            Err(e) => Err(LockError::Io(e)),
        }
    })();
    if guard() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

fn break_stale(file: &Path, lease_ms: u64, guard: &dyn Fn() -> bool) -> Result<bool, LockError> {
    let Some(seen) = look(&lock_path_of(file)) else { return Ok(true) };
    if !stale(&seen, lease_ms, now_ms()) {
        return Ok(false);
    }
    let (ino, text) = (seen.ino, seen.text.clone());
    remove_lock_if(file, &move |t, i| i == ino && t == text, guard)
}

/// Takes the lock, waiting up to `wait_ms`.
pub fn acquire_lock(file: &Path, wait_ms: u64, lease_ms: u64, guard: &dyn Fn() -> bool) -> Result<Lock, LockError> {
    let until = now_ms() + wait_ms as f64;
    let mut delay = 2.0f64;
    let mut quick = 0;
    loop {
        if let Some(l) = create(file, guard)? {
            return Ok(l);
        }
        if break_stale(file, lease_ms, guard)? && quick < 20 {
            quick += 1;
            continue;
        }
        if now_ms() >= until {
            return Err(LockError::Timeout);
        }
        let jitter = 0.5 + (rand_hex(1).chars().next().and_then(|c| c.to_digit(16)).unwrap_or(8) as f64 / 16.0);
        let ms = delay.min((until - now_ms()).max(1.0)) * jitter;
        std::thread::sleep(Duration::from_micros((ms * 1000.0) as u64));
        delay = (delay * 2.0).min(40.0);
    }
}
