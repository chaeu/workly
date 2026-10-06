//! New workspaces, and projects: create from the template, edit, reorder,
//! delete, and read the files inside them.

use crate::model::{Project, parse_file};
use crate::scan::rel;
use crate::settings::expand_home;
use crate::write::{LogEntry, OwnWrites, atomic_write, log};
use crate::{Error, Result, Workspace, ids, patch};
use serde::Deserialize;
use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

macro_rules! fixture {
    ($p:literal) => {
        ($p, include_str!(concat!("../../../fixtures/workspace/", $p)))
    };
}

/// Built-in defaults for a new workspace: the fixture's files.
const WORKSPACE_FILES: [(&str, &str); 7] = [
    fixture!("_templates/project.md"),
    fixture!("_templates/task.md"),
    fixture!("_templates/note.md"),
    fixture!("_templates/decision.md"),
    fixture!(".workly/config.yml"),
    fixture!(".workly/process.yml"),
    fixture!(".workly/agent/AGENTS.md"),
];
const WORKSPACE_DIRS: [&str; 5] = ["inbox", "projects", "knowledge", ".workly/log", ".workly/trash"];

pub const PROJECT_DIRS: [&str; 4] = ["tasks", "docs", "notes", "decisions"];
pub const PROJECT_STATUSES: [&str; 3] = ["active", "paused", "archived"];
pub const COLORS: [&str; 6] = ["proj-1", "proj-2", "proj-3", "proj-4", "proj-5", "proj-6"];
/// Use-case fields only `move_usecase` writes, so every move takes the same path.
const MOVE_FIELDS: [&str; 3] = ["usecase.step", "usecase.step_since", "usecase.decisions"];
/// Folders the project detail shows as a tree.
pub const DOC_DIRS: [&str; 4] = ["docs", "notes", "decisions", "agent"];

const AGENTS_TEMPLATE: &str = "# {{title}} – agent instructions\n\n\
    Project rules for agents, on top of the workspace rules in `.workly/agent/AGENTS.md`.\n\n- \n";

/// True when `path` already is a workspace.
pub fn is_workspace(path: &Path) -> bool {
    path.join(".workly/config.yml").is_file()
}

/// Turn `path` (created if missing) into a workspace. Only adds what is
/// missing; existing files are never overwritten.
pub fn init_workspace(path: &Path) -> Result<()> {
    for dir in WORKSPACE_DIRS {
        fs::create_dir_all(path.join(dir))?;
    }
    for (file, content) in WORKSPACE_FILES {
        let target = path.join(file);
        if !target.exists() {
            fs::create_dir_all(target.parent().unwrap())?;
            atomic_write(&target, content, &OwnWrites::default())?;
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Deserialize)]
pub struct NewProject {
    pub title: String,
    pub key: String,
    pub color: String,
    #[serde(default)]
    pub repos: Vec<String>,
}

/// 2-6 characters, uppercase letters, digits only after the first letter (`WR`, `WR2`).
pub fn check_key(key: &str) -> Result<()> {
    let ok = (2..=6).contains(&key.len())
        && key.starts_with(|c: char| c.is_ascii_uppercase())
        && key.chars().all(|c| c.is_ascii_uppercase() || c.is_ascii_digit());
    if !ok {
        return Err(Error::Invalid(format!("key '{key}': 2-6 uppercase letters or digits, starting with a letter")));
    }
    if key == ids::INBOX_KEY {
        return Err(Error::Invalid(format!("key '{key}' is reserved for the inbox")));
    }
    Ok(())
}

impl Workspace {
    fn project_dir(&self, key: &str) -> Result<PathBuf> {
        let p = self.index.project(key).ok_or_else(|| Error::NotFound(format!("project {key} not found")))?;
        Ok(self.root.join(&p.path))
    }

    /// Key suggestion for a title, avoiding keys already in use.
    pub fn suggest_key(&self, title: &str) -> String {
        let taken: Vec<&str> = self.index.projects.iter().map(|p| p.project.key.as_str()).collect();
        ids::suggest_key(title, &taken)
    }

    /// New project from `_templates/project.md` in `<new_projects_dir>/<slug>/`,
    /// with tasks/, docs/, notes/, decisions/, agent/AGENTS.md and `<key>.code-workspace`.
    /// Returns the project folder relative to the workspace.
    pub fn create_project(&mut self, new: &NewProject, actor: &str) -> Result<String> {
        let title = new.title.trim();
        if title.is_empty() {
            return Err(Error::Invalid("a project needs a title".into()));
        }
        check_key(&new.key)?;
        if self.index.project(&new.key).is_some() {
            return Err(Error::Invalid(format!("key {} is already used", new.key)));
        }
        if !COLORS.contains(&new.color.as_str()) {
            return Err(Error::Invalid(format!("unknown colour {}", new.color)));
        }
        let dir = self.root.join(&self.config.new_projects_dir).join(ids::slug(title));
        let rel_dir = rel(&self.root, &dir);
        if dir.exists() {
            return Err(Error::Invalid(format!("folder {rel_dir} already exists")));
        }
        let order = self.index.projects.iter().filter_map(|p| p.project.order).max().unwrap_or(0) + 1;
        let template = fs::read_to_string(self.root.join("_templates/project.md")).unwrap_or_else(|_| WORKSPACE_FILES[0].1.into());
        let title_yaml = patch::emit(&Value::from(title)).map_err(Error::Invalid)?;
        let today = chrono::Local::now().format("%Y-%m-%d").to_string();
        let mut content = template
            .replace("{{key}}", &new.key)
            .replace("{{title}}", &title_yaml)
            .replace("{{color}}", &new.color)
            .replace("{{order}}", &order.to_string())
            .replace("{{date}}", &today);
        if !new.repos.is_empty() {
            content = patch::set_field(&content, &["repos"], &json!(new.repos)).map_err(Error::Invalid)?;
        }
        match parse_file::<Project>("_templates/project.md", &content) {
            Ok(p) if p.key == new.key && p.title == title => {}
            _ => return Err(Error::Invalid("_templates/project.md does not produce a valid project".into())),
        }
        for sub in PROJECT_DIRS {
            fs::create_dir_all(dir.join(sub))?;
        }
        fs::create_dir_all(dir.join("agent"))?;
        atomic_write(&dir.join("agent/AGENTS.md"), &AGENTS_TEMPLATE.replace("{{title}}", title), &self.own)?;
        write_code_workspace(&dir, &new.key, &new.repos, &self.own)?;
        atomic_write(&dir.join("_project.md"), &content, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "project.create", &new.key, None, Value::Null, rel_dir.clone().into()))?;
        self.rescan();
        Ok(rel_dir)
    }

    /// Set one field of `_project.md` (`title`, `repos`, `usecase.step`, ...). The key is fixed.
    pub fn update_project_field(&mut self, key: &str, field: &str, value: &Value, actor: &str) -> Result<()> {
        match field {
            "key" => return Err(Error::Invalid("the key of a project cannot change".into())),
            "title" if value.as_str().is_none_or(|s| s.trim().is_empty()) => {
                return Err(Error::Invalid("a project needs a title".into()));
            }
            "status" if !value.as_str().is_some_and(|s| PROJECT_STATUSES.contains(&s)) => {
                return Err(Error::Invalid(format!("status must be one of {}, got {value}", PROJECT_STATUSES.join(", "))));
            }
            "color" if !value.as_str().is_some_and(|s| COLORS.contains(&s)) => {
                return Err(Error::Invalid(format!("unknown colour {value}")));
            }
            f if MOVE_FIELDS.contains(&f) => return Err(Error::Invalid(format!("{f} only changes by moving the use case"))),
            "usecase.status" | "usecase.type" => {
                if let Some(p) = &self.process {
                    let list = if field == "usecase.status" { &p.statuses } else { &p.types };
                    if !list.iter().any(|x| value.as_str() == Some(x.id.as_str())) {
                        return Err(Error::Invalid(format!("{field}: {value} is not listed in process.yml")));
                    }
                }
            }
            _ => {}
        }
        let dir = self.project_dir(key)?;
        let file = dir.join("_project.md");
        let path: Vec<&str> = field.split('.').collect();
        let src = fs::read_to_string(&file)?;
        let from = patch::get_field(&src, &path).map_err(Error::Invalid)?;
        if &from == value {
            return Ok(());
        }
        let out = patch::set_field(&src, &path, value).map_err(Error::Invalid)?;
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "project.update", key, Some(field), from, value.clone()))?;
        self.rescan();
        if field == "repos" {
            self.code_workspace(key, true)?;
        }
        Ok(())
    }

    /// The one way a use case changes its step (board drag, map drag, decision button):
    /// sets `usecase.step` and `usecase.step_since` (today), and when it leaves a gate,
    /// appends `{ date, gate, text }` to `usecase.decisions`. The text is the label of
    /// the edge taken, or "Moved to <step>" when no edge leads there. One write, one log line.
    pub fn move_usecase(&mut self, key: &str, step: &str, actor: &str) -> Result<()> {
        let process = self.process.as_ref().ok_or_else(|| Error::Invalid("no valid .workly/process.yml".into()))?;
        let step_of = |id: &str| process.steps.iter().find(|s| s.id == id);
        let to = step_of(step).ok_or_else(|| Error::Invalid(format!("unknown step '{step}'")))?;
        if to.kind == "term" {
            return Err(Error::Invalid(format!("'{}' is a start or end point, not a position", to.label)));
        }
        let entry = self.index.project(key).ok_or_else(|| Error::NotFound(format!("project {key} not found")))?;
        let from = entry.project.usecase.as_ref().ok_or_else(|| Error::Invalid(format!("{key} is not a use case")))?.step.clone();
        if from.as_deref() == Some(step) {
            return Ok(());
        }
        let file = self.root.join(&entry.path).join("_project.md");
        let today = crate::today();
        let src = fs::read_to_string(&file)?;
        let mut out = patch::set_field(&src, &["usecase", "step"], &Value::from(step)).map_err(Error::Invalid)?;
        out = patch::set_field(&out, &["usecase", "step_since"], &Value::from(today.as_str())).map_err(Error::Invalid)?;
        if let Some(gate) = from.as_deref().and_then(step_of).filter(|s| s.kind == "gate") {
            let edge = process.edges.iter().find(|e| e.from == gate.id && e.to == step);
            let text = edge.and_then(|e| e.label.clone()).unwrap_or_else(|| format!("Moved to {}", to.label));
            let mut decisions = match patch::get_field(&out, &["usecase", "decisions"]).map_err(Error::Invalid)? {
                Value::Array(list) => list,
                Value::Null => Vec::new(),
                _ => return Err(Error::Invalid(format!("{key}: usecase.decisions is not a list"))),
            };
            decisions.push(json!({ "date": today, "gate": gate.code.as_deref().unwrap_or(&gate.id), "text": text }));
            out = patch::set_field(&out, &["usecase", "decisions"], &Value::Array(decisions)).map_err(Error::Invalid)?;
        }
        atomic_write(&file, &out, &self.own)?;
        log(&self.root, &LogEntry::new(actor, "usecase.move", key, Some("usecase.step"), from.into(), step.into()))?;
        self.rescan();
        Ok(())
    }

    /// Write `order` 1..n in the given key order. Unchanged orders are not touched.
    pub fn reorder_projects(&mut self, keys: &[String], actor: &str) -> Result<()> {
        for (i, key) in keys.iter().enumerate() {
            let order = i as i64 + 1;
            if self.index.project(key).ok_or_else(|| Error::NotFound(format!("project {key} not found")))?.project.order != Some(order) {
                self.update_project_field(key, "order", &Value::from(order), actor)?;
            }
        }
        Ok(())
    }

    /// Move the whole project folder to `.workly/trash/<same relative path>`.
    pub fn delete_project(&mut self, key: &str, actor: &str) -> Result<()> {
        let from = self.project_dir(key)?;
        let rel_dir = rel(&self.root, &from);
        let to = self.root.join(".workly/trash").join(&rel_dir);
        if to.exists() {
            return Err(Error::Invalid(format!("trash already has {rel_dir}")));
        }
        fs::create_dir_all(to.parent().unwrap())?;
        self.own.expect(&from, None);
        fs::rename(&from, &to)?;
        let trashed = format!(".workly/trash/{rel_dir}");
        log(&self.root, &LogEntry::new(actor, "project.delete", key, Some("path"), rel_dir.into(), trashed.into()))?;
        self.rescan();
        Ok(())
    }

    /// Absolute path of `<project>/<key>.code-workspace`; written when missing or `rewrite`.
    pub fn code_workspace(&self, key: &str, rewrite: bool) -> Result<PathBuf> {
        let dir = self.project_dir(key)?;
        let file = dir.join(format!("{key}.code-workspace"));
        if rewrite || !file.exists() {
            let repos = &self.index.project(key).unwrap().project.repos;
            write_code_workspace(&dir, key, repos, &self.own)?;
        }
        Ok(file)
    }

    /// Files under docs/, notes/, decisions/ and agent/, relative to the workspace.
    pub fn project_files(&self, key: &str) -> Result<Vec<String>> {
        let dir = self.project_dir(key)?;
        let mut files = Vec::new();
        let mut stack: Vec<PathBuf> = DOC_DIRS.iter().map(|d| dir.join(d)).collect();
        while let Some(d) = stack.pop() {
            for e in fs::read_dir(&d).into_iter().flatten().flatten() {
                if e.file_name().to_string_lossy().starts_with('.') {
                    continue;
                }
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    stack.push(e.path());
                } else {
                    files.push(rel(&self.root, &e.path()));
                }
            }
        }
        files.sort();
        Ok(files)
    }

    /// Absolute path for a workspace-relative path; refuses anything outside the workspace.
    pub fn resolve(&self, rel_path: &str) -> Result<PathBuf> {
        let path = self.root.join(rel_path);
        let real = path.canonicalize().map_err(|e| Error::NotFound(format!("{rel_path}: {e}")))?;
        if !real.starts_with(&self.root) {
            return Err(Error::Invalid(format!("{rel_path} is outside the workspace")));
        }
        Ok(real)
    }

    /// Body of a Markdown file without its frontmatter, for the preview.
    pub fn read_markdown(&self, rel_path: &str) -> Result<String> {
        let src = fs::read_to_string(self.resolve(rel_path)?)?;
        Ok(crate::frontmatter::split(&src).body.to_string())
    }
}

/// VS Code workspace: the project folder plus its repos (absolute, per Mac).
// ponytail: regenerated whole when repos change, so hand edits to this file are lost; merge `folders` if anyone customises it.
fn write_code_workspace(dir: &Path, key: &str, repos: &[String], own: &OwnWrites) -> Result<()> {
    let mut folders = vec![json!({ "path": "." })];
    folders.extend(repos.iter().map(|r| json!({ "path": expand_home(r) })));
    let text = serde_json::to_string_pretty(&json!({ "folders": folders })).map_err(std::io::Error::other)? + "\n";
    atomic_write(&dir.join(format!("{key}.code-workspace")), &text, own)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys() {
        for ok in ["WR", "OPS", "CRMXY", "WR2"] {
            assert!(check_key(ok).is_ok(), "{ok}");
        }
        for bad in ["W", "wr", "2W", "W-R", "IN", "TOOLONG7"] {
            assert!(check_key(bad).is_err(), "{bad}");
        }
    }
}
