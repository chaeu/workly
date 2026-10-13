//! All file logic for Workly workspaces. App and CLI only call into this crate.

pub mod agent;
pub mod backup;
pub mod changes;
pub mod frontmatter;
pub mod history;
pub mod ids;
pub mod model;
pub mod patch;
pub mod project;
pub mod scan;
pub mod settings;
pub mod trash;
pub mod watch;
pub mod write;

use frontmatter::AcceptanceItem;
use model::{Config, ParseError, Process, Task, parse_file};
use scan::{Index, rel};
use serde_json::Value;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use write::{LogEntry, OwnWrites, atomic_write, log, own_rename};

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug)]
pub enum Error {
    /// Bad input: unknown field, invalid value, target exists. CLI exit 1.
    Invalid(String),
    /// Unknown task, project or trash entry. CLI exit 2.
    NotFound(String),
    /// Not allowed for this actor, e.g. an agent setting `done`. CLI exit 3.
    InvalidTransition(String),
    /// The target is not in the state the caller expected (file changed outside,
    /// project folder already exists); nothing written. CLI exit 1.
    Conflict(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Error::Invalid(m) | Error::NotFound(m) | Error::InvalidTransition(m) | Error::Conflict(m) => f.write_str(m),
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

/// Today on this Mac, `YYYY-MM-DD`.
pub fn today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Name in `## Updates` lines and the run log: the agent name, `me` for the app, else the actor (`cli`).
pub(crate) fn who(actor: &str) -> &str {
    match actor {
        "app" => "me",
        a => a.strip_prefix("agent:").unwrap_or(a),
    }
}

/// `text` as one line (whitespace runs collapsed); `empty` is the error when nothing is left.
fn one_line(text: &str, empty: &str) -> Result<String> {
    let text = text.split_whitespace().collect::<Vec<_>>().join(" ");
    if text.is_empty() { Err(Error::Invalid(empty.into())) } else { Ok(text) }
}

/// A new task; only `title` is required. `project` is a key, `None` = inbox.
#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(default)]
pub struct NewTask {
    pub title: String,
    pub project: Option<String>,
    pub priority: Option<u8>,
    pub due: Option<String>,
    pub tags: Vec<String>,
    pub parent: Option<String>,
    pub description: String,
    /// One criterion per entry, written as `- [ ] <text>`.
    pub acceptance: Vec<String>,
    /// `agent.runner`, `agent.model`, `agent.effort`; missing = not written.
    pub runner: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
}

impl NewTask {
    pub fn new(title: &str, project: Option<&str>) -> Self {
        NewTask { title: title.into(), project: project.map(String::from), ..Default::default() }
    }
}

/// Task fields that are absent rather than `null` when unset.
const OPTIONAL_FIELDS: [&str; 2] = ["parent", "waiting_for"];

/// Task fields written as block lists (one path per line, readable diffs).
const BLOCK_LISTS: [&str; 1] = ["agent.changes"];

/// Most tasks on the focus strip.
pub const FOCUS_MAX: usize = 3;

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
        self.index = scan::scan(&self.root, &self.config, self.process.as_ref());
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
            .chain(self.logged_ids())
            .collect();
        format!("{key}-{}", ids::next_number(key, names.iter().map(String::as_str)))
    }

    /// Every id the log ever mentions, so an emptied trash frees no numbers.
    // ponytail: reads all log files per new id; index them if logs reach many MB.
    fn logged_ids(&self) -> Vec<String> {
        let dir = self.root.join(".workly/log");
        let lines: Vec<String> = fs::read_dir(dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().extension().is_some_and(|x| x == "jsonl"))
            .flat_map(|e| fs::read_to_string(e.path()).unwrap_or_default().lines().map(String::from).collect::<Vec<_>>())
            .collect();
        lines.iter().filter_map(|l| serde_json::from_str::<Value>(l).ok()?.get("id")?.as_str().map(String::from)).collect()
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

    /// Set one field (`status`, `agent.active`, ...) of a task. `null` for an
    /// optional field (`parent`, `waiting_for`) removes its line.
    pub fn update_task_field(&mut self, id: &str, field: &str, value: &Value, actor: &str) -> Result<()> {
        let path: Vec<&str> = field.split('.').collect();
        self.check_update(field, value, actor)?;
        if field == "parent" {
            self.check_parent(Some(id), value.as_str())?;
        }
        if value.is_null() && OPTIONAL_FIELDS.contains(&field) {
            return self.remove_task_field(id, field, actor);
        }
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let from = patch::get_field(&src, &path).map_err(Error::Invalid)?;
        if &from == value {
            return Ok(());
        }
        let out = patch::set_field(&src, &path, value).map_err(Error::Invalid)?;
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.update", id, Some(field), from.clone(), value.clone()))?;
        self.rescan();
        // `done_at` follows the status: stamped on done, cleared when leaving done.
        if field == "status" && (*value == "done" || from == "done") {
            let done_at = if *value == "done" { Value::from(today()) } else { Value::Null };
            self.update_task_field(id, "done_at", &done_at, actor)?;
        }
        if field == "status" {
            self.left_waiting(id, &from, actor)?;
        }
        Ok(())
    }

    /// `waiting_for` only lives while the status is `waiting`: removed once the
    /// task left it (the log keeps the old value in `from`).
    pub(crate) fn left_waiting(&mut self, id: &str, from: &Value, actor: &str) -> Result<()> {
        let status = self.index.task(id).map(|t| t.task.status.as_str());
        if from == "waiting" && status != Some("waiting") {
            self.remove_task_field(id, "waiting_for", actor)?;
        }
        Ok(())
    }

    /// Set several fields of a task in one write; one log line per changed field.
    /// Callers check the rules; unchanged fields are skipped.
    fn set_task_fields(&mut self, id: &str, changes: &[(&str, Value)], actor: &str) -> Result<()> {
        self.set_task_fields_with_update(id, changes, None, actor)
    }

    /// `set_task_fields` plus, in the same write, `update` appended as one item under `## Updates`.
    fn set_task_fields_with_update(&mut self, id: &str, changes: &[(&str, Value)], update: Option<&str>, actor: &str) -> Result<()> {
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let mut out = src.clone();
        let mut changed = Vec::new();
        for (field, value) in changes {
            let path: Vec<&str> = field.split('.').collect();
            let from = patch::get_field(&out, &path).map_err(Error::Invalid)?;
            if &from != value {
                let set = if BLOCK_LISTS.contains(field) { patch::set_block_list } else { patch::set_field };
                out = set(&out, &path, value).map_err(Error::Invalid)?;
                changed.push((*field, from, value));
            }
        }
        if let Some(text) = update {
            let p = crate::frontmatter::split(&out);
            out = [p.head, p.frontmatter, p.fence, &crate::frontmatter::append_update(p.body, text)].concat();
        }
        if changed.is_empty() && update.is_none() {
            return Ok(());
        }
        atomic_write(&file, &out, &self.own)?;
        for (field, from, to) in changed {
            log(&self.root, &LogEntry::new(actor, "task.update", id, Some(field), from, to.clone()))?;
        }
        if let Some(text) = update {
            log(&self.root, &LogEntry::new(actor, "task.summary", id, Some("updates"), Value::Null, text.into()))?;
        }
        self.rescan();
        Ok(())
    }

    /// Remove a field's line(s) from a task. Missing field = no-op.
    pub fn remove_task_field(&mut self, id: &str, field: &str, actor: &str) -> Result<()> {
        let path: Vec<&str> = field.split('.').collect();
        if field == "id" {
            return Err(Error::Invalid("the id of a task cannot change".into()));
        }
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let from = patch::get_field(&src, &path).map_err(Error::Invalid)?;
        let out = patch::remove_field(&src, &path).map_err(Error::Invalid)?;
        if out == src {
            return Ok(());
        }
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.update", id, Some(field), from, Value::Null))?;
        self.rescan();
        Ok(())
    }

    /// Write `order` 1..n in the given id order. Unchanged orders are not touched.
    pub fn reorder_tasks(&mut self, ids: &[String], actor: &str) -> Result<()> {
        for (i, id) in ids.iter().enumerate() {
            let order = i as i64 + 1;
            if self.index.task(id).ok_or_else(|| Error::NotFound(format!("task {id} not found")))?.task.order != Some(order) {
                self.update_task_field(id, "order", &Value::from(order), actor)?;
            }
        }
        Ok(())
    }

    /// Today's focus strip, in order. Listed tasks get `focus: <today>` and
    /// `focus_order` 1..n; tasks that drop off lose both lines.
    pub fn set_focus(&mut self, ids: &[String], actor: &str) -> Result<()> {
        if ids.len() > FOCUS_MAX {
            return Err(Error::Invalid(format!("the focus strip holds at most {FOCUS_MAX} tasks")));
        }
        for (i, id) in ids.iter().enumerate() {
            if ids[..i].contains(id) {
                return Err(Error::Invalid(format!("{id} is listed twice")));
            }
            self.task_path(id)?;
        }
        let today = today();
        let dropped: Vec<String> = (self.index.tasks.iter())
            .filter(|t| t.task.focus.as_deref() == Some(today.as_str()) && !ids.contains(&t.task.id))
            .map(|t| t.task.id.clone())
            .collect();
        for id in dropped {
            self.remove_task_field(&id, "focus", actor)?;
            self.remove_task_field(&id, "focus_order", actor)?;
        }
        for (i, id) in ids.iter().enumerate() {
            self.update_task_field(id, "focus", &Value::from(today.as_str()), actor)?;
            self.update_task_field(id, "focus_order", &Value::from(i + 1), actor)?;
        }
        Ok(())
    }

    /// Append `- YYYY-MM-DD HH:MM · <who>: <text>` under `## Updates` in the body.
    pub fn add_task_update(&mut self, id: &str, text: &str, actor: &str) -> Result<()> {
        // One list item: line breaks would end it.
        let text = one_line(text, "an update needs text")?;
        let line = format!("{} · {}: {text}", chrono::Local::now().format("%Y-%m-%d %H:%M"), who(actor));
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let p = crate::frontmatter::split(&src);
        let out = [p.head, p.frontmatter, p.fence, &crate::frontmatter::append_update(p.body, &line)].concat();
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.note", id, Some("updates"), Value::Null, line.into()))?;
        self.rescan();
        Ok(())
    }

    /// Replace the description (the `## Description` section, or in the old
    /// layout the text before Acceptance/Updates). `expected` is the description
    /// the caller started from; if the file has another one now, nothing is
    /// written. The old text goes to the log.
    pub fn set_description(&mut self, id: &str, text: &str, expected: &str, actor: &str) -> Result<()> {
        use crate::frontmatter::{description_range, ends_description, normalize_description, replace_description};
        let (text, expected) = (normalize_description(text), normalize_description(expected));
        self.patch_body(id, "description", actor, |body| {
            let (range, sectioned) = description_range(body);
            if let Some(line) = text.lines().find(|l| ends_description(l, sectioned)) {
                return Err(Error::Invalid(format!("the description cannot contain the heading '{}'; use ### for sub-headings", line.trim_end())));
            }
            let from = normalize_description(&body[range]);
            if from != expected {
                return Err(Error::Conflict(format!("the description of {id} changed outside")));
            }
            Ok((replace_description(body, &text, crate::frontmatter::eol_of(body)), from.into(), text.clone().into()))
        })
    }

    /// Check or uncheck acceptance criterion `n` (0-based): one line changes.
    /// `shown` is the item as the app showed it; anything else is a conflict.
    pub fn toggle_acceptance(&mut self, id: &str, n: usize, shown: &AcceptanceItem, actor: &str) -> Result<()> {
        self.patch_body(id, "acceptance", actor, |body| {
            let out = crate::frontmatter::toggle_acceptance(body, n, shown)
                .ok_or_else(|| Error::Conflict(format!("the acceptance criteria of {id} changed outside")))?;
            let to = AcceptanceItem { text: shown.text.clone(), done: !shown.done };
            Ok((out, serde_json::to_value(shown).unwrap_or_default(), serde_json::to_value(to).unwrap_or_default()))
        })
    }

    /// Append one open criterion `- [ ] <text>` to the Acceptance section (created when missing).
    pub fn add_acceptance(&mut self, id: &str, text: &str, actor: &str) -> Result<()> {
        let text = one_line(text, "a criterion needs text")?;
        self.patch_body(id, "acceptance", actor, |body| Ok((crate::frontmatter::append_acceptance(body, &text), Value::Null, text.clone().into())))
    }

    /// Old layout -> `## Description` on top, empty `## Acceptance` above `## Updates`.
    pub fn add_sections(&mut self, id: &str, actor: &str) -> Result<()> {
        self.patch_body(id, "sections", actor, |body| Ok((crate::frontmatter::add_sections(body), Value::Null, "## Description, ## Acceptance".into())))
    }

    /// Rewrite a task's body with `f` -> (body, log from, log to); the
    /// frontmatter stays byte-identical. Unchanged body = no write, no log.
    fn patch_body(&mut self, id: &str, field: &str, actor: &str, f: impl FnOnce(&str) -> Result<(String, Value, Value)>) -> Result<()> {
        let file = self.task_path(id)?;
        let src = fs::read_to_string(&file)?;
        let p = crate::frontmatter::split(&src);
        let (body, from, to) = f(p.body)?;
        if body == p.body {
            return Ok(());
        }
        atomic_write(&file, &[p.head, p.frontmatter, p.fence, &body].concat(), &self.own)?;
        log(&self.root, &LogEntry::new(actor, "task.update", id, Some(field), from, to))?;
        self.rescan();
        Ok(())
    }

    /// One level of parents: `parent` exists, is not `child` itself and has no
    /// parent; `child` (None = a new task) has no sub-tasks.
    fn check_parent(&self, child: Option<&str>, parent: Option<&str>) -> Result<()> {
        let Some(parent) = parent else { return Ok(()) };
        let p = self.index.task(parent).ok_or_else(|| Error::NotFound(format!("parent task {parent} not found")))?;
        if child == Some(parent) {
            return Err(Error::Invalid(format!("{parent} cannot be its own parent")));
        }
        if let Some(grand) = &p.task.parent {
            return Err(Error::Invalid(format!("{parent} is a sub-task of {grand}; sub-tasks cannot have sub-tasks (one level)")));
        }
        if let Some(child) = child
            && self.index.tasks.iter().any(|t| t.task.parent.as_deref() == Some(child))
        {
            return Err(Error::Invalid(format!("{child} has sub-tasks, so it cannot get a parent (one level)")));
        }
        Ok(())
    }

    fn check_update(&self, field: &str, value: &Value, actor: &str) -> Result<()> {
        match field {
            "id" => Err(Error::Invalid("the id of a task cannot change".into())),
            "title" if value.as_str().is_none_or(|s| s.trim().is_empty()) => Err(Error::Invalid("a task needs a title".into())),
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
            "parent" | "waiting_for" if !value.is_null() && value.as_str().is_none_or(|s| s.trim().is_empty()) => {
                Err(Error::Invalid(format!("{field} must be a text or null, got {value}")))
            }
            "priority" if !matches!(value.as_u64(), Some(1..=3)) && !value.is_null() => {
                Err(Error::Invalid(format!("priority must be 1, 2 or 3, got {value}")))
            }
            _ => Ok(()),
        }
    }

    /// New task from `_templates/task.md`, written once. Fields from `new` are
    /// patched into the template; description and acceptance fill its body sections.
    pub fn create_task(&mut self, new: &NewTask, actor: &str) -> Result<String> {
        use crate::frontmatter::{append_acceptance, description_range, ends_description, eol_of, replace_description, split};
        let title = new.title.trim();
        if title.is_empty() {
            return Err(Error::Invalid("a task needs a title".into()));
        }
        self.check_parent(None, new.parent.as_deref())?;
        let project = new.project.as_deref();
        let dir = self.tasks_dir(project)?;
        let id = self.next_id(project.unwrap_or(ids::INBOX_KEY));
        let file = dir.join(format!("{id}-{}.md", ids::slug(title)));
        let template = fs::read_to_string(self.root.join("_templates/task.md")).unwrap_or_else(|_| project::WORKSPACE_FILES[1].1.into());
        let title_yaml = patch::emit(&Value::from(title)).map_err(Error::Invalid)?;
        let mut content = template.replace("{{id}}", &id).replace("{{title}}", &title_yaml).replace("{{date}}", &today());
        let mut fields: Vec<(&str, Value)> = Vec::new();
        fields.extend(new.priority.map(|p| ("priority", Value::from(p))));
        fields.extend(new.due.as_ref().map(|d| ("due", Value::from(d.as_str()))));
        if !new.tags.is_empty() {
            fields.push(("tags", Value::from(new.tags.clone())));
        }
        fields.extend(new.parent.as_ref().map(|p| ("parent", Value::from(p.as_str()))));
        for (field, v) in [("agent.runner", &new.runner), ("agent.model", &new.model), ("agent.effort", &new.effort)] {
            fields.extend(v.as_ref().map(|v| (field, Value::from(v.as_str()))));
        }
        for (field, value) in fields {
            self.check_update(field, &value, actor)?;
            let path: Vec<&str> = field.split('.').collect();
            content = patch::set_field(&content, &path, &value).map_err(Error::Invalid)?;
        }
        let description = new.description.trim_end();
        if !description.is_empty() || !new.acceptance.is_empty() {
            let p = split(&content);
            let mut body = p.body.to_string();
            if !description.is_empty() {
                let sectioned = description_range(&body).1;
                if let Some(line) = description.lines().find(|l| ends_description(l, sectioned)) {
                    return Err(Error::Invalid(format!("the description cannot contain the heading '{}'; use ### for sub-headings", line.trim_end())));
                }
                body = replace_description(&body, description, eol_of(&content));
            }
            for item in &new.acceptance {
                body = append_acceptance(&body, &one_line(item, "a criterion needs text")?);
            }
            content = [p.head, p.frontmatter, p.fence, &body].concat();
        }
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
}
