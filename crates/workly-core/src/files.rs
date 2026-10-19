//! Notes and docs inside a project folder: the file tree, the body editor,
//! new files and folders, delete to the trash.

use crate::scan::rel;
use crate::write::{LogEntry, atomic_write, hash, log, own_rename};
use crate::{Error, Result, Workspace};
use serde::Serialize;
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// The editor autosaves; one `file.body` log line per file per this many seconds.
const BODY_LOG_EVERY: std::time::Duration = std::time::Duration::from_secs(10 * 60);

/// Top-level project folders the Files tab never writes: tasks have the
/// detail card, agent/ its own section behind the agent switch.
const LOCKED: [&str; 2] = ["tasks", "agent"];

/// One entry of a project's file tree. `children` is `None` for a file.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct FileNode {
    pub name: String,
    /// Relative to the workspace.
    pub path: String,
    pub children: Option<Vec<FileNode>>,
}

impl Workspace {
    /// Everything below the project folder except `_project.md`, `tasks/`,
    /// `agent/`, dotfiles, `*.code-workspace` and nested projects; empty
    /// folders included. Folders first, then by name.
    pub fn project_files(&self, key: &str) -> Result<Vec<FileNode>> {
        Ok(self.tree(&self.project_dir(key)?, true))
    }

    fn tree(&self, dir: &Path, top: bool) -> Vec<FileNode> {
        let mut nodes: Vec<FileNode> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter_map(|e| {
                let name = e.file_name().to_string_lossy().into_owned();
                let path = e.path();
                // file_type does not follow symlinks, so linked folders are not walked.
                let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
                let hidden = name.starts_with('.')
                    || name.ends_with(".code-workspace")
                    || (top && (name == "_project.md" || LOCKED.contains(&name.as_str())))
                    || (is_dir && path.join("_project.md").is_file())
                    || (!is_dir && path.is_dir());
                if hidden {
                    return None;
                }
                Some(FileNode { children: is_dir.then(|| self.tree(&path, false)), path: rel(&self.root, &path), name })
            })
            .collect();
        nodes.sort_by(|a, b| (a.children.is_none(), &a.name).cmp(&(b.children.is_none(), &b.name)));
        nodes
    }

    /// Project key, absolute path and project folder for a workspace-relative
    /// path (it must exist). Refused outside every project folder, under
    /// `tasks/` and `agent/`, and under any dot folder (`.workly/`).
    fn project_target(&self, rel_path: &str) -> Result<(String, PathBuf, PathBuf)> {
        let abs = self.resolve(rel_path)?;
        // Nested projects: the deepest folder owns the path.
        let (key, dir) = self
            .index
            .projects
            .iter()
            .map(|p| (&p.project.key, self.root.join(&p.path)))
            .filter(|(_, dir)| abs.starts_with(dir))
            .max_by_key(|(_, dir)| dir.as_os_str().len())
            .ok_or_else(|| Error::Invalid(format!("{rel_path} is not inside a project")))?;
        let inner = abs.strip_prefix(&dir).unwrap();
        let first = inner.components().next().map(|c| c.as_os_str().to_string_lossy().into_owned());
        if first.is_some_and(|f| LOCKED.contains(&f.as_str())) || inner.components().any(|c| c.as_os_str().to_string_lossy().starts_with('.')) {
            return Err(Error::Invalid(format!("{rel_path} cannot be changed here")));
        }
        Ok((key.clone(), abs, dir))
    }

    /// Replace the body of a Markdown file in a project; the frontmatter (if
    /// any) stays byte-identical. `expected` is the body the caller started
    /// from; if the file has another one now, nothing is written. Logged at
    /// most once per file per `BODY_LOG_EVERY` (autosave writes often).
    pub fn set_body(&mut self, rel_path: &str, text: &str, expected: &str, actor: &str) -> Result<()> {
        let (key, file, _) = self.project_target(rel_path)?;
        if !file.is_file() || file.extension().is_none_or(|e| e != "md") {
            return Err(Error::Invalid(format!("{rel_path} is not a Markdown file")));
        }
        let src = fs::read_to_string(&file)?;
        let p = crate::frontmatter::split(&src);
        if p.body != expected {
            return Err(Error::Conflict(format!("{rel_path} changed outside")));
        }
        if p.body == text {
            return Ok(());
        }
        // A closing fence at the very end of the file has no line break yet.
        let gap = if !p.fence.is_empty() && !p.fence.ends_with('\n') && !text.is_empty() { crate::frontmatter::eol_of(&src) } else { "" };
        atomic_write(&file, &[p.head, p.frontmatter, p.fence, gap, text].concat(), &self.own)?;
        // ponytail: in-memory throttle, so an app restart logs the next save again.
        let now = std::time::Instant::now();
        if self.body_logged.get(&file).is_none_or(|t| now.duration_since(*t) >= BODY_LOG_EVERY) {
            let path = rel(&self.root, &file);
            log(&self.root, &LogEntry::new(actor, "file.body", &key, Some("path"), Value::Null, path.into()))?;
            self.body_logged.insert(file, now);
        }
        self.rescan();
        Ok(())
    }

    /// New empty Markdown file `name` (".md" added when missing) in the
    /// project folder `dir` (workspace-relative). Returns its path.
    pub fn create_file(&mut self, key: &str, dir: &str, name: &str, actor: &str) -> Result<String> {
        let name = check_name(name)?;
        let name = if name.ends_with(".md") { name.to_string() } else { format!("{name}.md") };
        let file = self.new_entry(key, dir, &name)?;
        self.own.expect(&file, Some(hash(b"")));
        fs::OpenOptions::new().write(true).create_new(true).open(&file)?;
        self.log_create(key, &file, actor)
    }

    /// New empty folder `name` in the project folder `dir`. Returns its path.
    pub fn create_folder(&mut self, key: &str, dir: &str, name: &str, actor: &str) -> Result<String> {
        let name = check_name(name)?;
        let folder = self.new_entry(key, dir, name)?;
        fs::create_dir(&folder)?;
        self.log_create(key, &folder, actor)
    }

    fn new_entry(&self, key: &str, dir: &str, name: &str) -> Result<PathBuf> {
        let (owner, abs, project) = self.project_target(dir)?;
        if owner != key {
            return Err(Error::Invalid(format!("{dir} is not in project {key}")));
        }
        if !abs.is_dir() {
            return Err(Error::Invalid(format!("{dir} is not a folder")));
        }
        if abs == project && LOCKED.contains(&name) {
            return Err(Error::Invalid(format!("{name} is reserved")));
        }
        let path = abs.join(name);
        if path.exists() || path.is_symlink() {
            return Err(Error::Invalid(format!("{} already exists", rel(&self.root, &path))));
        }
        Ok(path)
    }

    fn log_create(&mut self, key: &str, path: &Path, actor: &str) -> Result<String> {
        let path = rel(&self.root, path);
        log(&self.root, &LogEntry::new(actor, "file.create", key, Some("path"), Value::Null, path.clone().into()))?;
        self.rescan();
        Ok(path)
    }

    /// Move a file or folder (with its contents) of a project to
    /// `.workly/trash/<same relative path>`. Not the project folder itself
    /// and not `_project.md`: that is deleting the project.
    pub fn delete_path(&mut self, rel_path: &str, actor: &str) -> Result<()> {
        let (key, from, dir) = self.project_target(rel_path)?;
        if from == dir || from == dir.join("_project.md") {
            return Err(Error::Invalid("delete the whole project from the Projects page".into()));
        }
        // Through a link the real path differs; deleting would move the link's target.
        if from != self.root.join(rel_path) {
            return Err(Error::Invalid(format!("{rel_path} is a link; delete it in Finder")));
        }
        let path = rel(&self.root, &from);
        let to = self.root.join(".workly/trash").join(&path);
        if from.is_dir() && to.is_dir() {
            crate::trash::merge_into(&from, &to, &self.own, &|_| false)?;
        } else if to.exists() {
            return Err(Error::Invalid(format!("trash already has {path}")));
        } else if from.is_dir() {
            fs::create_dir_all(to.parent().unwrap())?;
            self.own.expect(&from, None);
            fs::rename(&from, &to)?;
        } else {
            own_rename(&from, &to, &self.own)?;
        }
        let trashed = format!(".workly/trash/{path}");
        log(&self.root, &LogEntry::new(actor, "file.delete", &key, Some("path"), path.into(), trashed.into()))?;
        self.rescan();
        Ok(())
    }

    /// Body of `_templates/project.md` (built-in when the workspace has none),
    /// for "Insert template" on an empty project description.
    pub fn project_template(&self) -> String {
        let src = fs::read_to_string(self.root.join("_templates/project.md")).unwrap_or_else(|_| crate::project::WORKSPACE_FILES[0].1.into());
        crate::frontmatter::split(&src).body.to_string()
    }
}

/// A file or folder name typed by the user: trimmed, not empty, no path
/// separator, no leading dot (also rules out `.` and `..`).
fn check_name(name: &str) -> Result<&str> {
    let name = name.trim();
    if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\', ':', '\0']) {
        return Err(Error::Invalid(format!("'{name}' is not a valid name (no '/', ':' or leading '.')")));
    }
    Ok(name)
}
