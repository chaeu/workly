//! All file logic for Workly workspaces. App and CLI only call into this crate.

pub mod frontmatter;
pub mod ids;
pub mod model;
pub mod patch;
pub mod project;
pub mod scan;
pub mod settings;
pub mod watch;
pub mod write;

use model::{Config, ParseError, Process, Task, parse_file};
use scan::{Index, rel};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use write::{LogEntry, OwnWrites, atomic_write, log, own_rename};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

const DEFAULT_TASK_TEMPLATE: &str =
    "---\nid: {{id}}\ntitle: {{title}}\nstatus: todo\npriority: 2\ndue: null\ntags: []\ncreated: {{date}}\ndone_at: null\n---\n";

#[derive(Debug)]
pub enum Error {
    /// Bad input: unknown field, invalid value, target exists. CLI exit 1.
    Invalid(String),
    /// Unknown task, project or trash entry. CLI exit 2.
    NotFound(String),
    /// Not allowed for this actor, e.g. an agent setting `done`. CLI exit 3.
    InvalidTransition(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Invalid(m) | Error::NotFound(m) | Error::InvalidTransition(m) => f.write_str(m),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

pub type Result<T> = std::result::Result<T, Error>;

pub struct Workspace {
    root: PathBuf,
    pub config: Config,
    pub process: Option<Process>,
    pub index: Index,
    own: OwnWrites,
}

impl Workspace {
    pub fn open(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref();
        // Canonical, because the watcher reports canonical paths (/private/var/...).
        let root = root.canonicalize().map_err(|e| Error::NotFound(format!("workspace {}: {e}", root.display())))?;
        let mut ws = Workspace { root, config: Config::default(), process: None, index: Index::default(), own: OwnWrites::default() };
        ws.rescan();
        Ok(ws)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Reload config, process and the full index from disk.
    // ponytail: full rescan on every change (fixture: ~1 ms); go incremental if big workspaces get slow.
    pub fn rescan(&mut self) {
        let mut errors = Vec::new();
        self.config = self.load_yaml(".workly/config.yml", &mut errors).unwrap_or_default();
        self.process = self.load_yaml::<Process>(".workly/process.yml", &mut errors);
        if let Some(p) = &self.process {
            let path = ".workly/process.yml".to_string();
            errors.extend(p.validate().into_iter().map(|message| ParseError { path: path.clone(), line: None, message }));
        }
        self.index = scan::scan(&self.root, &self.config);
        self.index.errors.splice(0..0, errors);
    }

    fn load_yaml<T: serde::de::DeserializeOwned>(&self, path: &str, errors: &mut Vec<ParseError>) -> Option<T> {
        let src = fs::read_to_string(self.root.join(path)).ok()?;
        serde_yaml::from_str(&src)
            .map_err(|e| errors.push(ParseError { path: path.into(), line: e.location().map(|l| l.line()), message: e.to_string() }))
            .ok()
    }

    /// Watch the workspace; `on_change` sees external changes only.
    pub fn watch(&self, on_change: impl FnMut(Vec<watch::Change>) + Send + 'static) -> Result<watch::Watcher> {
        watch::watch(self.root.clone(), self.config.scan_exclude.clone(), self.own.clone(), on_change)
            .map_err(|e| Error::Io(std::io::Error::other(e)))
    }

    /// Next free id for a key: highest number in the workspace and trash + 1.
    /// Broken task files count too, by file name.
    pub fn next_id(&self, key: &str) -> String {
        let idx = &self.index;
        let file_name = |p: &str| p.rsplit('/').next().unwrap_or(p).to_string();
        let names: Vec<String> = idx
            .tasks
            .iter()
            .flat_map(|t| [t.task.id.clone(), file_name(&t.path)])
            .chain(idx.errors.iter().map(|e| file_name(&e.path)))
            .chain(idx.trash.iter().map(|p| file_name(p)))
            .collect();
        format!("{key}-{}", ids::next_number(key, names.iter().map(String::as_str)))
    }

    fn task_path(&self, id: &str) -> Result<PathBuf> {
        let t = self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?;
        Ok(self.root.join(&t.path))
    }

    /// Folder for tasks of a project key, or the inbox for `None`.
    fn tasks_dir(&self, project: Option<&str>) -> Result<PathBuf> {
        match project {
            None => Ok(self.root.join(&self.config.inbox_dir)),
            Some(key) => {
                let p = self.index.project(key).ok_or_else(|| Error::NotFound(format!("project {key} not found")))?;
                Ok(self.root.join(&p.path).join("tasks"))
            }
        }
    }

    /// Set one field (`status`, `agent.active`, ...) of a task.
    pub fn update_task_field(&mut self, id: &str, field: &str, value: &Value, actor: &str) -> Result<()> {
        let path: Vec<&str> = field.split('.').collect();
        self.check_update(field, value, actor)?;
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let from = patch::get_field(&src, &path).map_err(Error::Invalid)?;
        if &from == value {
            return Ok(());
        }
        let out = patch::set_field(&src, &path, value).map_err(Error::Invalid)?;
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.update", id, Some(field), from, value.clone()))?;
        self.rescan();
        Ok(())
    }

    fn check_update(&self, field: &str, value: &Value, actor: &str) -> Result<()> {
        match field {
            "id" => Err(Error::Invalid("the id of a task cannot change".into())),
            "status" => {
                let status = value.as_str().unwrap_or_default();
                if !self.config.task_statuses.iter().any(|s| s.id == status) {
                    return Err(Error::Invalid(format!("unknown status {value}")));
                }
                if status == "done" && actor.starts_with("agent:") {
                    return Err(Error::InvalidTransition("agents never set a task to done".into()));
                }
                Ok(())
            }
            "priority" if !matches!(value.as_u64(), Some(1..=3)) && !value.is_null() => {
                Err(Error::Invalid(format!("priority must be 1, 2 or 3, got {value}")))
            }
            _ => Ok(()),
        }
    }

    /// New task from `_templates/task.md`. `project` is a key; `None` = inbox.
    pub fn create_task(&mut self, title: &str, project: Option<&str>, actor: &str) -> Result<String> {
        let dir = self.tasks_dir(project)?;
        let id = self.next_id(project.unwrap_or(ids::INBOX_KEY));
        let file = dir.join(format!("{id}-{}.md", ids::slug(title)));
        let template = fs::read_to_string(self.root.join("_templates/task.md")).unwrap_or_else(|_| DEFAULT_TASK_TEMPLATE.into());
        let title_yaml = patch::emit(&Value::from(title)).map_err(Error::Invalid)?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let content = template.replace("{{id}}", &id).replace("{{title}}", &title_yaml).replace("{{date}}", &today);
        let rel_path = rel(&self.root, &file);
        match parse_file::<Task>(&rel_path, &content) {
            Ok(t) if t.id == id && t.title == title => {}
            _ => return Err(Error::Invalid("_templates/task.md does not produce a valid task".into())),
        }
        if file.exists() {
            return Err(Error::Invalid(format!("{rel_path} already exists")));
        }
        fs::create_dir_all(&dir)?;
        atomic_write(&file, &content, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.create", &id, None, Value::Null, rel_path.into()))?;
        self.rescan();
        Ok(id)
    }

    /// Move a task to another project (`None` = inbox). Id and file name stay.
    pub fn move_task(&mut self, id: &str, project: Option<&str>, actor: &str) -> Result<()> {
        let from = self.task_path(id)?;
        let to = self.tasks_dir(project)?.join(from.file_name().unwrap());
        if to == from {
            return Ok(());
        }
        if to.exists() {
            return Err(Error::Invalid(format!("{} already exists", rel(&self.root, &to))));
        }
        own_rename(&from, &to, &self.own)?;
        let (a, b) = (rel(&self.root, &from), rel(&self.root, &to));
        log(&self.root, &LogEntry::new(actor, "task.move", id, Some("path"), a.into(), b.into()))?;
        self.rescan();
        Ok(())
    }

    /// Move a task to `.workly/trash/<same relative path>` and stamp `deleted_at`.
    pub fn delete_task(&mut self, id: &str, actor: &str) -> Result<()> {
        let from = self.task_path(id)?;
        let rel_path = rel(&self.root, &from);
        let to = self.root.join(".workly/trash").join(&rel_path);
        if to.exists() {
            return Err(Error::Invalid(format!("trash already has {rel_path}")));
        }
        own_rename(&from, &to, &self.own)?;
        let src = fs::read_to_string(&to)?;
        let now = Value::from(chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ").to_string());
        if let Ok(out) = patch::set_field(&src, &["deleted_at"], &now) {
            atomic_write(&to, &out, &self.own)?;
        }
        let trashed = format!(".workly/trash/{rel_path}");
        log(&self.root, &LogEntry::new(actor, "task.delete", id, Some("path"), rel_path.into(), trashed.into()))?;
        self.rescan();
        Ok(())
    }

    /// Move a trash entry (path relative to the trash) back to where it was.
    pub fn restore(&mut self, trash_path: &str, actor: &str) -> Result<()> {
        if !self.index.trash.iter().any(|p| p == trash_path) {
            return Err(Error::NotFound(format!("{trash_path} is not in the trash")));
        }
        let from = self.root.join(".workly/trash").join(trash_path);
        let to = self.root.join(trash_path);
        if to.exists() {
            return Err(Error::Invalid(format!("{trash_path} already exists")));
        }
        // Unstamp inside the trash first, so the restored file appears once, final.
        let src = fs::read_to_string(&from)?;
        if let Ok(out) = patch::remove_field(&src, &["deleted_at"]) {
            atomic_write(&from, &out, &self.own)?;
        }
        own_rename(&from, &to, &self.own)?;
        let id = parse_file::<Task>(trash_path, &fs::read_to_string(&to)?).map(|t| t.id).unwrap_or_default();
        let trashed = format!(".workly/trash/{trash_path}");
        log(&self.root, &LogEntry::new(actor, "task.restore", &id, Some("path"), trashed.into(), trash_path.into()))?;
        self.rescan();
        Ok(())
    }
}
