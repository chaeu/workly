//! Atomic writes, the change log and tracking of our own writes.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

/// What the core expects a path to look like after its own change:
/// `Some(hash)` = this content, `None` = gone. Lets the watcher skip our writes.
#[derive(Clone, Default)]
pub struct OwnWrites(Arc<Mutex<HashMap<PathBuf, Option<u64>>>>);

impl OwnWrites {
    pub fn expect(&self, path: &Path, state: Option<u64>) {
        self.0.lock().unwrap().insert(path.to_path_buf(), state);
    }

    /// True when `path` is in the state we left it in. Consumes the entry,
    /// so a later external change to the same path is never swallowed.
    pub fn consume(&self, path: &Path) -> bool {
        let Some(expected) = self.0.lock().unwrap().remove(path) else { return false };
        expected == fs::read(path).ok().map(|b| hash(&b))
    }
}

pub fn hash(bytes: &[u8]) -> u64 {
    let mut h = DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

/// Write to a temp file in the same folder, then rename over the target.
pub fn atomic_write(path: &Path, content: &str, own: &OwnWrites) -> io::Result<()> {
    let name = path.file_name().ok_or_else(|| io::Error::other("no file name"))?;
    let tmp = path.with_file_name(format!(".{}.workly-tmp", name.to_string_lossy()));
    let result = (|| {
        let mut f = fs::File::create(&tmp)?;
        f.write_all(content.as_bytes())?;
        f.sync_all()?;
        own.expect(path, Some(hash(content.as_bytes())));
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    result
}

/// Rename that the watcher will not report back.
pub fn own_rename(from: &Path, to: &Path, own: &OwnWrites) -> io::Result<()> {
    if let Some(dir) = to.parent() {
        fs::create_dir_all(dir)?;
    }
    let bytes = fs::read(from)?;
    own.expect(from, None);
    own.expect(to, Some(hash(&bytes)));
    fs::rename(from, to)
}

#[derive(Serialize)]
pub struct LogEntry<'a> {
    pub ts: String,
    pub actor: &'a str,
    pub kind: &'a str,
    pub id: &'a str,
    pub field: Option<&'a str>,
    pub from: Value,
    pub to: Value,
}

impl<'a> LogEntry<'a> {
    pub fn new(actor: &'a str, kind: &'a str, id: &'a str, field: Option<&'a str>, from: Value, to: Value) -> Self {
        LogEntry { ts: chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string(), actor, kind, id, field, from, to }
    }
}

/// Append one JSON line to `.workly/log/YYYY-MM.jsonl` (month of `ts`).
pub fn log(root: &Path, entry: &LogEntry) -> io::Result<()> {
    let dir = root.join(".workly/log");
    fs::create_dir_all(&dir)?;
    let line = serde_json::to_string(entry)? + "\n";
    let mut f = fs::OpenOptions::new().create(true).append(true).open(dir.join(format!("{}.jsonl", &entry.ts[..7])))?;
    f.write_all(line.as_bytes())
}
