//! Walk a workspace and build the in-memory index.

use crate::model::{Config, ParseError, Process, Project, Task, assessment_problems, parse_file, saving_problems};
use serde::Serialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, Serialize)]
pub struct Index {
    pub projects: Vec<ProjectEntry>,
    pub tasks: Vec<TaskEntry>,
    /// Paths inside `.workly/trash/`, relative to it.
    pub trash: Vec<String>,
    /// Broken files and config problems. Never abort the scan.
    pub errors: Vec<ParseError>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectEntry {
    /// Project folder, relative to the workspace.
    pub path: String,
    #[serde(flatten)]
    pub project: Project,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskEntry {
    /// Task file, relative to the workspace.
    pub path: String,
    /// Folder of the owning project; `None` for inbox tasks.
    pub project: Option<String>,
    #[serde(flatten)]
    pub task: Task,
}

impl Index {
    pub fn task(&self, id: &str) -> Option<&TaskEntry> {
        self.tasks.iter().find(|t| t.task.id == id)
    }

    pub fn project(&self, key: &str) -> Option<&ProjectEntry> {
        self.projects.iter().find(|p| p.project.key == key)
    }
}

pub fn rel(root: &Path, path: &Path) -> String {
    path.strip_prefix(root).unwrap_or(path).to_string_lossy().into_owned()
}

pub fn scan(root: &Path, config: &Config, process: Option<&Process>) -> Index {
    let mut idx = Index::default();
    walk(root, root, config, process, &mut idx);
    for file in md_files(&root.join(&config.inbox_dir)) {
        load_task(root, &file, None, &mut idx);
    }
    let trash = root.join(".workly/trash");
    let mut stack = vec![trash.clone()];
    while let Some(dir) = stack.pop() {
        for entry in fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if entry.file_type().is_ok_and(|t| t.is_dir()) {
                stack.push(path);
            } else if path.extension().is_some_and(|e| e == "md") {
                idx.trash.push(rel(&trash, &path));
            }
        }
    }
    idx.trash.sort();
    idx.projects.sort_by(|a, b| (a.project.order, &a.project.key).cmp(&(b.project.order, &b.project.key)));
    idx.tasks.sort_by(|a, b| a.path.cmp(&b.path));
    report_duplicates(&mut idx);
    idx
}

fn walk(root: &Path, dir: &Path, config: &Config, process: Option<&Process>, idx: &mut Index) {
    let project_file = dir.join("_project.md");
    if project_file.is_file() {
        let path = rel(root, dir);
        let file = rel(root, &project_file);
        match read(root, &project_file).and_then(|src| parse_file::<Project>(&file, &src).map(|p| (src, p))) {
            Ok((src, project)) => {
                // Bad savings and assessment entries load as empty fields; say what is wrong.
                if project.usecase.is_some() {
                    let raw = |f| crate::patch::get_field(&src, &["usecase", f]).unwrap_or_default();
                    let method = process.and_then(|p| p.assessment.as_ref());
                    let problems = saving_problems(&raw("savings")).into_iter().chain(assessment_problems(&raw("assessment"), method));
                    idx.errors.extend(problems.map(|message| ParseError { path: file.clone(), line: None, message }));
                }
                idx.projects.push(ProjectEntry { path: path.clone(), project });
            }
            Err(e) => idx.errors.push(e),
        }
        for file in md_files(&dir.join("tasks")) {
            load_task(root, &file, Some(path.clone()), idx);
        }
    }
    let mut dirs: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        // file_type does not follow symlinks, so linked folders cannot loop.
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| !config.scan_exclude.iter().any(|x| e.file_name() == x.as_str()))
        .map(|e| e.path())
        .collect();
    dirs.sort();
    for d in dirs {
        walk(root, &d, config, process, idx);
    }
}

fn md_files(dir: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.extension().is_some_and(|e| e == "md") && p.is_file())
        .collect();
    files.sort();
    files
}

fn read(root: &Path, path: &Path) -> Result<String, ParseError> {
    fs::read_to_string(path).map_err(|e| ParseError { path: rel(root, path), line: None, message: e.to_string() })
}

fn load_task(root: &Path, file: &Path, project: Option<String>, idx: &mut Index) {
    let path = rel(root, file);
    match read(root, file).and_then(|src| parse_file::<Task>(&path, &src)) {
        Ok(task) => idx.tasks.push(TaskEntry { path, project, task }),
        Err(e) => idx.errors.push(e),
    }
}

/// Duplicate ids or keys would make writes ambiguous; surface them.
fn report_duplicates(idx: &mut Index) {
    let mut seen: HashMap<&str, &str> = HashMap::new();
    let mut errors = Vec::new();
    let ids = idx.tasks.iter().map(|t| (t.task.id.as_str(), t.path.as_str(), "id"));
    let keys = idx.projects.iter().map(|p| (p.project.key.as_str(), p.path.as_str(), "project key"));
    for (id, path, what) in ids.chain(keys) {
        if let Some(first) = seen.insert(id, path) {
            errors.push(ParseError { path: path.into(), line: None, message: format!("duplicate {what} {id} (also in {first})") });
        }
    }
    idx.errors.extend(errors);
}
