//! Debounced file watcher that reports external changes only.

use crate::write::OwnWrites;
use notify::{RecommendedWatcher, RecursiveMode};
use notify_debouncer_mini::{DebounceEventResult, Debouncer, new_debouncer};
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Paths relative to the workspace.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub enum Change {
    Changed(String),
    Removed(String),
}

/// Keeps watching until dropped.
pub struct Watcher(#[allow(dead_code)] Debouncer<RecommendedWatcher>);

pub fn watch(
    root: PathBuf,
    exclude: Vec<String>,
    own: OwnWrites,
    mut on_change: impl FnMut(Vec<Change>) + Send + 'static,
) -> notify::Result<Watcher> {
    let base = root.clone();
    let mut debouncer = new_debouncer(Duration::from_millis(150), move |res: DebounceEventResult| {
        let Ok(events) = res else { return };
        let mut changes = Vec::new();
        for e in events {
            let Some(rel) = relevant(&base, &e.path, &exclude) else { continue };
            if own.consume(&e.path) {
                continue;
            }
            let change = if e.path.exists() { Change::Changed(rel) } else { Change::Removed(rel) };
            if !changes.contains(&change) {
                changes.push(change);
            }
        }
        if !changes.is_empty() {
            on_change(changes);
        }
    })?;
    debouncer.watcher().watch(&root, RecursiveMode::Recursive)?;
    Ok(Watcher(debouncer))
}

/// Markdown files, folders (moved or deleted as a whole) and the two config
/// files. Skips dotfiles (temp files, .DS_Store) and excluded folders.
fn relevant(root: &Path, path: &Path, exclude: &[String]) -> Option<String> {
    let rel = path.strip_prefix(root).ok()?;
    let s = rel.to_string_lossy();
    if s == ".workly/config.yml" || s == ".workly/process.yml" {
        return Some(s.into_owned());
    }
    if rel.components().any(|c| exclude.iter().any(|x| c.as_os_str() == x.as_str())) {
        return None;
    }
    let name = rel.file_name()?.to_string_lossy();
    let md_or_dir = match rel.extension() {
        Some(ext) => ext == "md",
        None => true,
    };
    (!name.starts_with('.') && md_or_dir).then(|| s.into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filters_paths() {
        let root = Path::new("/ws");
        let ex = vec![".workly".to_string(), ".git".to_string()];
        let r = |p: &str| relevant(root, Path::new(p), &ex);
        assert_eq!(r("/ws/inbox/IN-1-a.md").as_deref(), Some("inbox/IN-1-a.md"));
        assert_eq!(r("/ws/projects/x").as_deref(), Some("projects/x"));
        assert_eq!(r("/ws/.workly/config.yml").as_deref(), Some(".workly/config.yml"));
        assert_eq!(r("/ws/.workly/log/2026-10.jsonl"), None);
        assert_eq!(r("/ws/.workly/trash/a/WR-1-x.md"), None);
        assert_eq!(r("/ws/inbox/.IN-1-a.md.workly-tmp"), None);
        assert_eq!(r("/ws/.DS_Store"), None);
        assert_eq!(r("/ws/a/b.png"), None);
        assert_eq!(r("/elsewhere/a.md"), None);
    }
}
