//! `.workly/trash/`: list, restore, empty.

use crate::model::{Project, Task, parse_file};
use crate::write::{LogEntry, OwnWrites, atomic_write, log, own_rename};
use crate::{Error, Result, Workspace, ids, patch};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// One restorable thing: a task file or a whole project folder.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct TrashItem {
    /// Relative to the trash, which is also where it goes back to in the workspace.
    pub path: String,
    /// task | project | file (unreadable or not a task)
    pub kind: &'static str,
    /// Task id or project key; the file name when unreadable.
    pub id: String,
    pub title: String,
    pub deleted_at: Option<String>,
}

impl Workspace {
    fn trash_dir(&self) -> std::path::PathBuf {
        self.root.join(".workly/trash")
    }

    /// Project folders whose `_project.md` is in the trash, relative to it.
    fn trashed_projects(&self) -> Vec<&str> {
        self.index.trash.iter().filter_map(|p| p.strip_suffix("/_project.md")).collect()
    }

    /// Trash contents, a project folder as one item.
    pub fn trash_items(&self) -> Vec<TrashItem> {
        let projects = self.trashed_projects();
        let mut items: Vec<TrashItem> = projects.iter().map(|dir| self.trash_item(dir, "project")).collect();
        for path in &self.index.trash {
            if !projects.iter().any(|dir| path.starts_with(&format!("{dir}/"))) {
                items.push(self.trash_item(path, "task"));
            }
        }
        items.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then_with(|| a.path.cmp(&b.path)));
        items
    }

    fn trash_item(&self, path: &str, kind: &'static str) -> TrashItem {
        let file = if kind == "project" { format!("{path}/_project.md") } else { path.to_string() };
        let src = fs::read_to_string(self.trash_dir().join(&file)).unwrap_or_default();
        let name = path.rsplit('/').next().unwrap_or(path).to_string();
        let parsed = if kind == "project" {
            parse_file::<Project>(&file, &src).ok().map(|p| (p.key, p.title, p.extra))
        } else {
            parse_file::<Task>(&file, &src).ok().map(|t| (t.id, t.title, t.extra))
        };
        match parsed {
            Some((id, title, extra)) => {
                let deleted_at = extra.get("deleted_at").and_then(|v| v.as_str()).map(String::from);
                TrashItem { path: path.into(), kind, id, title, deleted_at }
            }
            None => TrashItem { path: path.into(), kind: "file", id: name.clone(), title: name, deleted_at: None },
        }
    }

    /// Move a trash item (task file or project folder, path relative to the
    /// trash) back to where it was.
    pub fn restore(&mut self, trash_path: &str, actor: &str) -> Result<()> {
        let is_project = self.trashed_projects().contains(&trash_path);
        if !is_project && !self.index.trash.iter().any(|p| p == trash_path) {
            return Err(Error::NotFound(format!("{trash_path} is not in the trash")));
        }
        let from = self.trash_dir().join(trash_path);
        let to = self.root.join(trash_path);
        if to.exists() {
            return Err(Error::Invalid(format!("{trash_path} already exists")));
        }
        let trashed = format!(".workly/trash/{trash_path}");
        if is_project {
            let key = self.trash_item(trash_path, "project").id;
            // Tasks deleted on their own before the project stay in the trash.
            merge_into(&from, &to, &self.own, &|f| stamped(f))?;
            log(&self.root, &LogEntry::new(actor, "project.restore", &key, Some("path"), trashed.into(), trash_path.into()))?;
            self.rescan();
            return Ok(());
        }
        // A task of a deleted project would land in a folder nobody scans.
        let dir = to.parent().unwrap();
        let project_tasks = dir.ends_with("tasks") && dir.parent().is_some_and(|p| p.join("_project.md").is_file());
        if !dir.is_dir() && !project_tasks {
            return Err(Error::Invalid(format!("the folder of {trash_path} is gone; restore its project first")));
        }
        // Unstamp inside the trash first, so the restored file appears once, final.
        let src = fs::read_to_string(&from)?;
        if let Ok(out) = patch::remove_field(&src, &["deleted_at"]) {
            atomic_write(&from, &out, &self.own)?;
        }
        own_rename(&from, &to, &self.own)?;
        let id = parse_file::<Task>(trash_path, &fs::read_to_string(&to)?).map(|t| t.id).unwrap_or_default();
        log(&self.root, &LogEntry::new(actor, "task.restore", &id, Some("path"), trashed.into(), trash_path.into()))?;
        self.rescan();
        Ok(())
    }

    /// Move every trash item to the macOS Trash (recoverable there), then drop
    /// the empty folders left behind. Ids stay taken through the log.
    pub fn empty_trash(&mut self, actor: &str) -> Result<usize> {
        let items = self.trash_items();
        if items.is_empty() {
            return Ok(0);
        }
        let paths: Vec<_> = items.iter().map(|i| self.trash_dir().join(&i.path)).collect();
        for p in &paths {
            self.own.expect(p, None);
        }
        system_trash(&paths)?;
        remove_empty_dirs(&self.trash_dir());
        // One line per project key and task id, so next_id keeps them taken,
        // also for files that never went through the app.
        let projects = items.iter().filter(|i| i.kind == "project").map(|i| (i.id.clone(), i.path.clone()));
        let tasks = self.index.trash.iter().filter_map(|p| {
            let (key, n) = ids::split_id(p.rsplit('/').next()?)?;
            Some((format!("{key}-{n}"), p.clone()))
        });
        for (id, path) in projects.chain(tasks).collect::<Vec<_>>() {
            let from = Value::from(format!(".workly/trash/{path}"));
            log(&self.root, &LogEntry::new(actor, "trash.empty", &id, Some("path"), from, Value::Null))?;
        }
        self.rescan();
        Ok(items.len())
    }
}

/// Move the files below `from` to the same places below `to`, except those
/// `stay` keeps; then drop the folders left empty. Refuses before moving
/// anything if a target file exists.
pub(crate) fn merge_into(from: &Path, to: &Path, own: &OwnWrites, stay: &dyn Fn(&Path) -> bool) -> Result<()> {
    let files: Vec<PathBuf> = files_below(from).into_iter().filter(|f| !stay(f)).collect();
    if let Some(f) = files.iter().find(|f| to.join(f.strip_prefix(from).unwrap()).exists()) {
        return Err(Error::Invalid(format!("{} exists in both places", f.strip_prefix(from).unwrap().display())));
    }
    move_dir(from, to, own, stay)
}

fn move_dir(from: &Path, to: &Path, own: &OwnWrites, stay: &dyn Fn(&Path) -> bool) -> Result<()> {
    fs::create_dir_all(to)?;
    for e in fs::read_dir(from)?.flatten() {
        let (src, dest) = (e.path(), to.join(e.file_name()));
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            move_dir(&src, &dest, own, stay)?;
        } else if stay(&src) {
            continue;
        } else if src.is_symlink() {
            own.expect(&src, None);
            fs::rename(&src, &dest)?;
        } else {
            own_rename(&src, &dest, own)?;
        }
    }
    let _ = fs::remove_dir(from);
    Ok(())
}

/// Files and links below `dir`; folders are walked, not listed.
fn files_below(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            out.extend(files_below(&e.path()));
        } else {
            out.push(e.path());
        }
    }
    out
}

/// A Markdown file that `delete_task` stamped with `deleted_at`.
fn stamped(file: &Path) -> bool {
    file.extension().is_some_and(|e| e == "md")
        && fs::read_to_string(file).is_ok_and(|src| patch::get_field(&src, &["deleted_at"]).is_ok_and(|v| !v.is_null()))
}

/// NSFileManager, not Finder: no Automation prompt, no sound.
fn system_trash(paths: &[std::path::PathBuf]) -> Result<()> {
    #[cfg(target_os = "macos")]
    let ctx = {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        let mut ctx = trash::TrashContext::default();
        ctx.set_delete_method(DeleteMethod::NsFileManager);
        ctx
    };
    #[cfg(not(target_os = "macos"))]
    let ctx = trash::TrashContext::default();
    ctx.delete_all(paths).map_err(|e| Error::Io(std::io::Error::other(e.to_string())))
}

/// Remove empty folders below `dir` (not `dir` itself). `remove_dir` only
/// succeeds on empty folders, so nothing with content is touched.
fn remove_empty_dirs(dir: &Path) {
    for e in fs::read_dir(dir).into_iter().flatten().flatten() {
        if e.file_type().is_ok_and(|t| t.is_dir()) {
            remove_empty_dirs(&e.path());
            let _ = fs::remove_dir(e.path());
        }
    }
}
