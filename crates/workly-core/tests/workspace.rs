//! Workspace operations against a temp copy of the fixture.

use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;
use workly_core::{Error, Workspace};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");
const WR8: &str = "projects/website-relaunch/tasks/WR-8-content-migration.md";

fn copy_dir(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for e in fs::read_dir(from).unwrap().flatten() {
        let dest = to.join(e.file_name());
        if e.file_type().unwrap().is_dir() {
            copy_dir(&e.path(), &dest);
        } else {
            fs::copy(e.path(), dest).unwrap();
        }
    }
}

/// Fresh fixture copy; never write to `fixtures/` itself.
fn fixture() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    (dir, root)
}

fn log_lines(root: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    for e in fs::read_dir(root.join(".workly/log")).unwrap().flatten() {
        if e.path().extension().is_some_and(|x| x == "jsonl") {
            lines.extend(fs::read_to_string(e.path()).unwrap().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()));
        }
    }
    lines
}

#[test]
fn loads_fixture_with_one_parse_error() {
    let ws = Workspace::open(FIXTURE).unwrap();
    let idx = &ws.index;
    let keys: Vec<&str> = idx.projects.iter().map(|p| p.project.key.as_str()).collect();
    assert_eq!(keys, ["WR", "IE", "OPS", "NA", "ST", "OI"]);
    assert_eq!(idx.tasks.len(), 12);
    assert_eq!(idx.errors.len(), 1, "{:?}", idx.errors);
    let e = &idx.errors[0];
    assert_eq!(e.path, "inbox/IN-3-broken-frontmatter.md");
    assert!(e.line.is_some());
    assert_eq!(idx.trash, ["projects/website-relaunch/tasks/WR-9-old-banner.md"]);
    let oi = idx.task("OI-1").unwrap();
    assert_eq!(oi.project.as_deref(), Some("projects/archive/old-intranet"));
    assert_eq!(idx.task("IN-1").unwrap().project, None);
    assert_eq!(idx.project("NA").unwrap().project.usecase.as_ref().unwrap().decisions.len(), 2);
    assert_eq!(idx.task("WR-3").unwrap().task.agent.as_ref().unwrap().active.as_deref(), Some("codex"));
    let process = ws.process.as_ref().unwrap();
    assert_eq!(process.steps.len(), 17);
    assert!(process.validate().is_empty());
    assert_eq!(ws.config.task_statuses[2].wip_limit, Some(3));
}

#[test]
fn scan_fixture_is_fast() {
    let mut ws = Workspace::open(FIXTURE).unwrap();
    let start = Instant::now();
    ws.rescan();
    let took = start.elapsed();
    let limit = if cfg!(debug_assertions) { 200 } else { 20 };
    assert!(took.as_millis() < limit, "scan took {took:?}");
    eprintln!("scan: {took:?}");
}

#[test]
fn next_ids_include_trash_and_broken_files() {
    let ws = Workspace::open(FIXTURE).unwrap();
    assert_eq!(ws.next_id("WR"), "WR-10");
    assert_eq!(ws.next_id("IN"), "IN-4");
    assert_eq!(ws.next_id("IE"), "IE-5");
    assert_eq!(ws.next_id("ST"), "ST-1");
}

#[test]
fn update_changes_one_line_and_logs() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_task_field("WR-8", "status", &json!("doing"), "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    assert_eq!(after, before.replace("status: backlog\n", "status: doing\n"));
    assert_eq!(ws.index.task("WR-8").unwrap().task.status, "doing");

    let last = log_lines(&root).pop().unwrap();
    assert_eq!(last["actor"], "app");
    assert_eq!(last["kind"], "task.update");
    assert_eq!((&last["id"], &last["field"], &last["from"], &last["to"]), (&json!("WR-8"), &json!("status"), &json!("backlog"), &json!("doing")));
    let keys: Vec<&String> = last.as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 7);

    // Same value: no write, no log line.
    let n = log_lines(&root).len();
    ws.update_task_field("WR-8", "status", &json!("doing"), "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
    // No temp files left behind.
    let dir = root.join("projects/website-relaunch/tasks");
    assert!(fs::read_dir(dir).unwrap().flatten().all(|e| !e.file_name().to_string_lossy().starts_with('.')));
}

#[test]
fn update_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    assert!(matches!(ws.update_task_field("WR-5", "status", &json!("done"), "agent:codex"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.update_task_field("WR-5", "status", &json!("nope"), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("WR-5", "priority", &json!(7), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("WR-5", "id", &json!("WR-50"), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("WR-5", "title", &json!(" "), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.update_task_field("XX-1", "status", &json!("todo"), "app"), Err(Error::NotFound(_))));
    ws.update_task_field("WR-5", "status", &json!("done"), "app").unwrap();
    ws.update_task_field("WR-5", "agent.active", &json!("codex"), "cli").unwrap();
    assert_eq!(ws.index.task("WR-5").unwrap().task.agent.as_ref().unwrap().active.as_deref(), Some("codex"));
}

#[test]
fn create_tasks() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // Project without a tasks/ folder.
    let id = ws.create_task("Collect volume: numbers", Some("ST"), None, "app").unwrap();
    assert_eq!(id, "ST-1");
    let src = fs::read_to_string(root.join("projects/support-triage/tasks/ST-1-collect-volume-numbers.md")).unwrap();
    assert!(src.starts_with("---\nid: ST-1\ntitle: \"Collect volume: numbers\"\nstatus: todo\n"), "{src}");
    assert_eq!(ws.create_task("Inbox thing", None, None, "app").unwrap(), "IN-4");
    assert_eq!(ws.create_task("Next", Some("WR"), None, "app").unwrap(), "WR-10");
    assert!(matches!(ws.create_task("x", Some("NOPE"), None, "app"), Err(Error::NotFound(_))));
    assert_eq!(log_lines(&root).iter().filter(|l| l["kind"] == "task.create").count(), 3);
}

#[test]
fn move_keeps_id_and_file_name() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    ws.move_task("WR-8", Some("ST"), "app").unwrap();
    let moved = root.join("projects/support-triage/tasks/WR-8-content-migration.md");
    assert_eq!(fs::read_to_string(&moved).unwrap(), before);
    let t = ws.index.task("WR-8").unwrap();
    assert_eq!(t.project.as_deref(), Some("projects/support-triage"));
    ws.move_task("WR-8", None, "app").unwrap();
    assert!(root.join("inbox/WR-8-content-migration.md").exists());
    assert_eq!(ws.next_id("WR"), "WR-10");
}

#[test]
fn delete_and_restore_round_trip() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    ws.delete_task("WR-8", "app").unwrap();
    assert!(!root.join(WR8).exists());
    assert!(ws.index.task("WR-8").is_none());
    let trashed = fs::read_to_string(root.join(".workly/trash").join(WR8)).unwrap();
    assert!(trashed.contains("\ndeleted_at: "));
    assert!(ws.index.trash.contains(&WR8.to_string()));

    ws.restore(WR8, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert!(ws.index.task("WR-8").is_some());

    // Restore refuses to overwrite.
    ws.delete_task("WR-8", "app").unwrap();
    fs::write(root.join(WR8), "someone else's file").unwrap();
    ws.rescan();
    assert!(matches!(ws.restore(WR8, "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.restore("nope.md", "app"), Err(Error::NotFound(_))));
}

#[test]
fn broken_config_and_process_are_reported() {
    let (_tmp, root) = fixture();
    fs::write(root.join(".workly/config.yml"), "scan_exclude: [unclosed\n").unwrap();
    let p = root.join(".workly/process.yml");
    let src = fs::read_to_string(&p).unwrap().replace("{ from: run,     to: end,", "{ from: run,     to: nowhere,");
    fs::write(&p, src).unwrap();
    let ws = Workspace::open(&root).unwrap();
    let msgs: Vec<String> = ws.index.errors.iter().map(|e| format!("{}: {}", e.path, e.message)).collect();
    assert!(msgs[0].starts_with(".workly/config.yml: "), "{msgs:?}");
    assert_eq!(msgs[1], ".workly/process.yml: edge 20 (run -> nowhere) references unknown step 'nowhere'");
    // Defaults keep the app usable.
    assert_eq!(ws.index.projects.len(), 6);
}

// ---------------------------------------------------------------- projects (M2)

use workly_core::project::{NewProject, init_workspace, is_workspace};

#[test]
fn empty_folder_becomes_workspace() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().join("new ws");
    assert!(!is_workspace(&root));
    init_workspace(&root).unwrap();
    assert!(is_workspace(&root));
    for p in ["_templates/project.md", "_templates/task.md", "inbox", "projects", "knowledge", ".workly/process.yml", ".workly/agent/AGENTS.md", ".workly/trash"] {
        assert!(root.join(p).exists(), "{p}");
    }
    assert_eq!(fs::read_to_string(root.join(".workly/config.yml")).unwrap(), fs::read_to_string(Path::new(FIXTURE).join(".workly/config.yml")).unwrap());
    // Existing files are never overwritten.
    fs::write(root.join(".workly/agent/AGENTS.md"), "mine").unwrap();
    init_workspace(&root).unwrap();
    assert_eq!(fs::read_to_string(root.join(".workly/agent/AGENTS.md")).unwrap(), "mine");

    let mut ws = Workspace::open(&root).unwrap();
    assert!(ws.index.errors.is_empty() && ws.index.projects.is_empty(), "{:?}", ws.index.errors);
    assert_eq!(ws.suggest_key("Test Alpha"), "TA");
    let new = NewProject { title: "Test Alpha".into(), key: "TA".into(), color: "proj-1".into(), repos: vec![], usecase: None, adopt: false };
    assert_eq!(ws.create_project(&new, "app").unwrap(), "projects/test-alpha");
    assert_eq!(ws.create_task("First", Some("TA"), None, "app").unwrap(), "TA-1");
}

#[test]
fn create_project_builds_structure() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let repo = workly_core::settings::home().join("repos/alpha");
    let new = NewProject { title: "Über: Alpha".into(), key: "UA".into(), color: "proj-3".into(), repos: vec!["~/repos/alpha".into()], usecase: None, adopt: false };
    let dir = ws.create_project(&new, "app").unwrap();
    assert_eq!(dir, "projects/ueber-alpha");
    let d = root.join(&dir);
    for sub in ["tasks", "docs", "notes", "decisions", "agent/AGENTS.md"] {
        assert!(d.join(sub).exists(), "{sub}");
    }
    let p = &ws.index.project("UA").unwrap().project;
    assert_eq!((p.title.as_str(), p.status.as_str(), p.color.as_deref(), p.order), ("Über: Alpha", "active", Some("proj-3"), Some(7)));
    assert_eq!(p.repos, ["~/repos/alpha"]);
    let src = fs::read_to_string(d.join("_project.md")).unwrap();
    assert!(src.contains("title: \"Über: Alpha\"\n") && src.contains("repos: [~/repos/alpha]\n") && src.contains("## Goal"), "{src}");
    let cw: Value = serde_json::from_str(&fs::read_to_string(d.join("UA.code-workspace")).unwrap()).unwrap();
    assert_eq!(cw["folders"], json!([{ "path": "." }, { "path": repo }]));
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.create");

    let mut bad = |key: &str, title: &str, color: &str| {
        let n = NewProject { title: title.into(), key: key.into(), color: color.into(), repos: vec![], usecase: None, adopt: false };
        matches!(ws.create_project(&n, "app"), Err(Error::Invalid(_)))
    };
    assert!(bad("WR", "Other", "proj-1"), "key taken");
    assert!(bad("IN", "Other", "proj-1"), "reserved");
    assert!(bad("ab", "Other", "proj-1"), "lowercase");
    assert!(bad("ZZ", "Über: Alpha", "proj-1"), "folder exists");
    assert!(bad("ZZ", "  ", "proj-1"), "no title");
    assert!(bad("ZZ", "Other", "#fff"), "colour");
}

/// Every file under `dir` with its bytes.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    for e in fs::read_dir(dir).unwrap().flatten() {
        if e.file_type().unwrap().is_dir() {
            out.extend(snapshot(&e.path()));
        } else {
            out.push((e.path(), fs::read(e.path()).unwrap()));
        }
    }
    out.sort();
    out
}

#[test]
fn adopt_existing_folder_keeps_every_file() {
    let (_tmp, root) = fixture();
    let wr8 = fs::read(root.join(WR8)).unwrap();
    let dir = root.join("projects/foo");
    fs::create_dir_all(dir.join("notes")).unwrap();
    fs::create_dir_all(dir.join("agent")).unwrap();
    fs::write(dir.join("notes/idea.md"), "# Idea\n\nLoose note.\n").unwrap();
    fs::write(dir.join("README.md"), "readme\r\nno newline").unwrap();
    fs::write(dir.join("agent/AGENTS.md"), "my rules\n").unwrap();
    fs::write(dir.join("FOO.code-workspace"), "{ \"folders\": [] }").unwrap();
    let before = snapshot(&dir);
    let mut ws = Workspace::open(&root).unwrap();
    let mut new = NewProject { title: "Foo".into(), key: "FOO".into(), color: "proj-2".into(), repos: vec![], usecase: None, adopt: false };

    // Without adopt: a distinguishable refusal, nothing written.
    assert!(matches!(ws.create_project(&new, "app"), Err(Error::Conflict(_))));
    assert_eq!(snapshot(&dir), before);

    new.adopt = true;
    assert_eq!(ws.create_project(&new, "app").unwrap(), "projects/foo");
    let after = snapshot(&dir);
    for file in &before {
        assert!(after.contains(file), "{} changed", file.0.display());
    }
    let added: Vec<String> = after.iter().filter(|f| !before.contains(f)).map(|f| f.0.strip_prefix(&dir).unwrap().display().to_string()).collect();
    assert_eq!(added, ["_project.md"]);
    for sub in ["tasks", "docs", "decisions"] {
        assert!(dir.join(sub).is_dir(), "{sub}");
    }
    assert_eq!(ws.index.project("FOO").unwrap().project.title, "Foo");
    assert_eq!((&log_lines(&root).pop().unwrap()["kind"]).as_str(), Some("project.adopt"));
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);

    // A folder with _project.md already is a project, adopt or not.
    let wr = root.join("projects/website-relaunch/_project.md");
    let wr_before = fs::read(&wr).unwrap();
    let n = NewProject { title: "Website Relaunch".into(), key: "WEB".into(), color: "proj-1".into(), repos: vec![], usecase: None, adopt: true };
    assert!(matches!(ws.create_project(&n, "app"), Err(Error::Invalid(m)) if m.contains("already a project")));
    assert_eq!(fs::read(&wr).unwrap(), wr_before);
}

#[test]
fn update_project_is_lossless() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/website-relaunch/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_project_field("WR", "title", &json!("Site Relaunch"), "app").unwrap();
    ws.update_project_field("WR", "status", &json!("paused"), "app").unwrap();
    let after = fs::read_to_string(&file).unwrap();
    assert_eq!(after, before.replace("title: Website Relaunch\n", "title: Site Relaunch\n").replace("status: active\n", "status: paused\n"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["id"], &last["field"]), (&json!("project.update"), &json!("WR"), &json!("status")));

    ws.update_project_field("WR", "repos", &json!(["~/repos/website", "/opt/api"]), "app").unwrap();
    let cw = fs::read_to_string(root.join("projects/website-relaunch/WR.code-workspace")).unwrap();
    assert!(cw.contains("/opt/api"), "{cw}");

    for (field, value) in [("key", json!("XX")), ("status", json!("done")), ("color", json!("red")), ("title", json!(""))] {
        assert!(matches!(ws.update_project_field("WR", field, &value, "app"), Err(Error::Invalid(_))), "{field}");
    }
}

#[test]
fn reorder_writes_order() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let keys: Vec<String> = ["IE", "WR", "OPS", "NA", "ST", "OI"].map(String::from).into();
    let n = log_lines(&root).len();
    ws.reorder_projects(&keys, "app").unwrap();
    assert_eq!(log_lines(&root).len(), n + 2, "only IE and WR changed");
    let order: Vec<&str> = ws.index.projects.iter().map(|p| p.project.key.as_str()).collect();
    assert_eq!(order, ["IE", "WR", "OPS", "NA", "ST", "OI"]);
}

#[test]
fn delete_project_moves_folder_to_trash() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    ws.delete_project("OI", "app").unwrap();
    assert!(!root.join("projects/archive/old-intranet").exists());
    assert!(root.join(".workly/trash/projects/archive/old-intranet/_project.md").is_file());
    assert!(ws.index.project("OI").is_none() && ws.index.task("OI-1").is_none());
    // Ids keep counting through the trash.
    assert_eq!(ws.next_id("OI"), "OI-2");
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.delete");
}

#[test]
fn project_files_and_preview() {
    let (_tmp, root) = fixture();
    let ws = Workspace::open(&root).unwrap();
    assert_eq!(
        ws.project_files("WR").unwrap(),
        [
            "projects/website-relaunch/agent/AGENTS.md",
            "projects/website-relaunch/agent/skills/release-check/SKILL.md",
            "projects/website-relaunch/decisions/0001-hosting.md",
            "projects/website-relaunch/docs/architecture.md",
            "projects/website-relaunch/notes/2026-09-15-kickoff.md",
        ]
    );
    assert!(ws.project_files("ST").unwrap().is_empty());
    let body = ws.read_markdown("projects/website-relaunch/_project.md").unwrap();
    assert!(body.starts_with("Relaunch of the portfolio site"), "{body}");
    assert!(matches!(ws.read_markdown("../outside.md"), Err(Error::NotFound(_))));
    fs::write(root.parent().unwrap().join("outside.md"), "x").unwrap();
    assert!(matches!(ws.read_markdown("../outside.md"), Err(Error::Invalid(_))));
    assert!(matches!(ws.read_markdown("projects/../../outside.md"), Err(Error::Invalid(_))));
}

#[test]
fn done_stamps_and_clears_done_at() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.update_task_field("WR-8", "status", &json!("done"), "app").unwrap();
    let today = workly_core::today();
    let done = before.replace("status: backlog\n", "status: done\n").replace("done_at: null\n", &format!("done_at: {today}\n"));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), done);
    ws.update_task_field("WR-8", "status", &json!("backlog"), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
}

#[test]
fn reorder_tasks_adds_one_line() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.reorder_tasks(&["WR-8".into(), "NA-1".into()], "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before.replace("done_at: null\n---", "done_at: null\norder: 1\n---"));
    assert_eq!(ws.index.task("NA-1").unwrap().task.order, Some(2));
    // Unchanged orders are not rewritten.
    let n = log_lines(&root).len();
    ws.reorder_tasks(&["WR-8".into()], "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
    assert!(matches!(ws.reorder_tasks(&["XX-1".into()], "app"), Err(Error::NotFound(_))));
}

#[test]
fn focus_sets_and_removes_lines() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();
    let ids = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    ws.set_focus(&ids(&["IN-1", "WR-8", "IN-2"]), "app").unwrap();
    let focused = before.replace("done_at: null\n---", &format!("done_at: null\nfocus: {today}\nfocus_order: 2\n---"));
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), focused);
    // Fixture tasks focused on another day are not today's strip.
    let strip = |ws: &Workspace| {
        let mut t: Vec<_> = ws.index.tasks.iter().filter(|t| t.task.focus.as_deref() == Some(today.as_str())).collect();
        t.sort_by_key(|t| t.task.focus_order);
        t.iter().map(|t| t.task.id.clone()).collect::<Vec<_>>()
    };
    let fixture_today: Vec<String> = Workspace::open(FIXTURE).unwrap().index.tasks.iter()
        .filter(|t| t.task.focus.as_deref() == Some(today.as_str())).map(|t| t.task.id.clone()).collect();
    if fixture_today.is_empty() {
        assert_eq!(strip(&ws), ["IN-1", "WR-8", "IN-2"]);
    }
    // Reorder, then drop WR-8: its lines go, the file is as before.
    ws.set_focus(&ids(&["WR-8", "IN-1"]), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), focused.replace("focus_order: 2", "focus_order: 1"));
    ws.set_focus(&ids(&["IN-1"]), "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert_eq!(strip(&ws), ["IN-1"]);
    assert!(!fs::read_to_string(root.join("inbox/IN-2-read-article.md")).unwrap().contains("focus"));
    assert!(matches!(ws.set_focus(&ids(&["IN-1", "IN-2", "WR-8", "WR-5"]), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.set_focus(&ids(&["IN-1", "IN-1"]), "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.set_focus(&ids(&["XX-1"]), "app"), Err(Error::NotFound(_))));
}

#[test]
fn update_note_appends_to_body_only() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let notes = |root: &Path| log_lines(root).iter().filter(|l| l["kind"] == "task.note").count();
    let n = notes(&root);
    ws.add_task_update("WR-8", "Asked the client\nabout URLs", "app").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    let added = after.strip_prefix(before.as_str()).expect("original bytes kept");
    assert!(added.starts_with("\n\n## Updates\n- 20") && added.ends_with(" · me: Asked the client about URLs\n"), "{added:?}");
    ws.add_task_update("WR-7", "Checked", "agent:codex").unwrap();
    assert!(fs::read_to_string(root.join("projects/website-relaunch/tasks/WR-7-lighthouse-audit.md")).unwrap().trim_end().ends_with(" · codex: Checked"));
    assert!(matches!(ws.add_task_update("WR-8", "  ", "app"), Err(Error::Invalid(_))));
    assert_eq!(notes(&root), n + 2);
}

#[test]
fn set_description_replaces_only_the_description() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let old = "Body with trailing spaces   \nand **markdown**.\n\n- [ ] list a\n- [x] list b";
    let (fm, desc) = before.split_at(before.find("Body with").unwrap());
    assert_eq!(desc, old);
    let mut ws = Workspace::open(&root).unwrap();

    // No Updates section, no final newline: stays without one.
    ws.set_description("WR-8", "New plan\n\n- step one\n", old, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), format!("{fm}New plan\n\n- step one"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"]), (&json!("task.update"), &json!("description"), &json!(old.trim_end())));

    // With an Updates section: frontmatter and Updates stay byte-identical.
    ws.add_task_update("WR-8", "Asked", "app").unwrap();
    let with_updates = fs::read_to_string(root.join(WR8)).unwrap();
    let updates = &with_updates[with_updates.find("## Updates").unwrap()..];
    ws.set_description("WR-8", "Shorter", "New plan\n\n- step one", "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), format!("{fm}Shorter\n\n{updates}"));

    // Unchanged text: no write, no log line.
    let n = log_lines(&root).len();
    ws.set_description("WR-8", "Shorter\n", "Shorter", "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn set_description_on_empty_body_and_back() {
    let (_tmp, root) = fixture();
    let file = root.join("inbox/IN-1-workshop-date.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    ws.set_description("IN-1", "Ask for Tuesday", "", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{before}Ask for Tuesday\n"));
    ws.set_description("IN-1", "  \n", "Ask for Tuesday", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
}

#[test]
fn set_description_conflict_writes_nothing() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let n = log_lines(&root).len();
    let r = ws.set_description("WR-8", "Mine", "What the card showed before", "app");
    assert!(matches!(r, Err(Error::Conflict(_))), "{r:?}");
    let r = ws.set_description("WR-8", "a\n## Updates\n- fake", &before[before.find("Body").unwrap()..], "app");
    assert!(matches!(r, Err(Error::Invalid(_))), "{r:?}");
    assert_eq!(fs::read_to_string(root.join(WR8)).unwrap(), before);
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn set_description_keeps_crlf() {
    let (_tmp, root) = fixture();
    let file = root.join("inbox/IN-7-crlf.md");
    let fm = "---\r\nid: IN-7\r\ntitle: Windows file\r\nstatus: todo\r\n---\r\n";
    let updates = "## Updates\r\n- 2026-10-01 09:00 · me: x\r\n";
    fs::write(&file, format!("{fm}Old\r\ntext\r\n\r\n{updates}")).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    // The textarea hands back LF; `expected` may still carry the file's CRLF.
    ws.set_description("IN-7", "New\nlines", "Old\r\ntext", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), format!("{fm}New\r\nlines\r\n\r\n{updates}"));
}

#[test]
fn create_task_with_priority() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let id = ws.create_task("  Urgent thing ", Some("WR"), Some(1), "app").unwrap();
    let t = ws.index.task(&id).unwrap();
    assert_eq!((t.task.title.as_str(), t.task.priority), ("Urgent thing", Some(1)));
    assert!(t.path.ends_with("WR-10-urgent-thing.md"));
    assert!(matches!(ws.create_task(" ", None, None, "app"), Err(Error::Invalid(_))));
    assert!(matches!(ws.create_task("x", None, Some(5), "app"), Err(Error::Invalid(_))));
}

// --------------------------------------------------------------- use cases (M4)

#[test]
fn move_usecase_writes_step_since_and_decision() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();

    // Into a gate: no decision yet.
    ws.move_usecase("IE", "g1", "app").unwrap();
    let at_gate = before.replace("  step: pilot\n  step_since: 2026-09-24\n", &format!("  step: g1\n  step_since: {today}\n"));
    assert_eq!(fs::read_to_string(&file).unwrap(), at_gate);

    // Out of a gate along an edge: the edge label becomes the decision, old items stay verbatim.
    ws.move_usecase("IE", "wd", "app").unwrap();
    let decided = at_gate.replace("  step: g1\n", "  step: wd\n").replace(
        "    - { date: 2026-09-10, gate: null, text: \"Start pilot, timebox 3 weeks\" }\n",
        &format!("    - {{ date: 2026-09-10, gate: null, text: \"Start pilot, timebox 3 weeks\" }}\n    - {{ date: {today}, gate: G1, text: \"Yes\" }}\n"),
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), decided);
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.step.as_deref(), uc.decisions.len()), (Some("wd"), 2));

    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["id"], &last["field"], &last["from"], &last["to"]), (&json!("usecase.move"), &json!("IE"), &json!("usecase.step"), &json!("g1"), &json!("wd")));

    // Same step: nothing written.
    ws.move_usecase("IE", "wd", "app").unwrap();
    assert_eq!(fs::read_to_string(&file).unwrap(), decided);
}

#[test]
fn move_usecase_without_decisions_and_off_edge() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/support-triage/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let today = workly_core::today();
    ws.move_usecase("ST", "g3", "app").unwrap();
    // No edge g3 -> parked: the decision says where it went.
    ws.move_usecase("ST", "parked", "app").unwrap();
    let want = before.replace("  step: need\n  step_since: 2026-10-02\n", &format!("  step: parked\n  step_since: {today}\n")).replace(
        "  current_state: Need reported, no pilot yet\n",
        &format!("  current_state: Need reported, no pilot yet\n  decisions: [{{ date: {today}, gate: G3, text: Moved to Parked }}]\n"),
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
}

#[test]
fn move_usecase_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    for (key, step) in [("IE", "start"), ("IE", "nowhere"), ("WR", "pilot")] {
        assert!(matches!(ws.move_usecase(key, step, "app"), Err(Error::Invalid(_))), "{key} {step}");
    }
    assert!(matches!(ws.move_usecase("XX", "pilot", "app"), Err(Error::NotFound(_))));
    // Moves take one path; status and type follow process.yml.
    for (field, value) in [("usecase.step", json!("g1")), ("usecase.decisions", json!([])), ("usecase.status", json!("done")), ("usecase.type", json!("ml"))] {
        assert!(matches!(ws.update_project_field("IE", field, &value, "app"), Err(Error::Invalid(_))), "{field}");
    }
    // Status is a property: it never moves the use case.
    ws.update_project_field("IE", "usecase.status", &json!("blocked"), "app").unwrap();
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.status.as_deref(), uc.step.as_deref()), (Some("blocked"), Some("pilot")));
    // Areas are free text: one that no list has is accepted as typed.
    ws.update_project_field("IE", "usecase.area", &json!("Underwriting"), "app").unwrap();
    assert_eq!(ws.index.project("IE").unwrap().project.usecase.clone().unwrap().area.as_deref(), Some("Underwriting"));
}

#[test]
fn create_usecase_starts_at_first_phase() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let uc = Some(workly_core::project::NewUseCase { kind: "ai".into(), area: Some("Finance".into()) });
    let new = NewProject { title: "Mail Sorting".into(), key: "MS".into(), color: "proj-1".into(), repos: vec![], usecase: uc, adopt: false };
    let dir = ws.create_project(&new, "app").unwrap();
    let src = fs::read_to_string(root.join(dir).join("_project.md")).unwrap();
    let today = workly_core::today();
    let block = format!(
        "created: {today}\nusecase:\n  type: ai\n  area: Finance\n  step: need\n  step_since: {today}\n  status: active\n  blocked_by: null\n  next_step: null\n  current_state: null\n---\n"
    );
    assert!(src.contains(&block), "{src}");
    assert_eq!(ws.index.project("MS").unwrap().project.usecase.as_ref().unwrap().step.as_deref(), Some("need"));

    let bad = Some(workly_core::project::NewUseCase { kind: "ml".into(), area: None });
    let n = NewProject { title: "Other".into(), key: "OT".into(), color: "proj-2".into(), repos: vec![], usecase: bad, adopt: false };
    assert!(matches!(ws.create_project(&n, "app"), Err(Error::Invalid(_))));
    assert!(!root.join("projects/other").exists());
}

// ----------------------------------------------------------- savings (P4)

#[test]
fn savings_write_is_lossless_and_logged() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let old = json!([
        { "what": "Capture invoice header", "count": 1200, "per": "month", "minutes": 6 },
        { "what": "Clarify queries with suppliers", "count": 80, "per": "month", "minutes": 15 },
    ]);
    // Unchanged row stays verbatim, the edited one is rewritten, the new one appended. Key order as sent.
    let new = json!([
        old[0],
        { "what": "Clarify queries with suppliers", "count": 80, "per": "month", "minutes": 20 },
        { "what": "Post, archive", "count": 2.5, "per": "day", "minutes": 30 },
    ]);
    ws.update_project_field("IE", "usecase.savings", &new, "app").unwrap();
    let want = before.replace(
        "    - { what: Clarify queries with suppliers, count: 80, per: month, minutes: 15 }\n",
        "    - { what: Clarify queries with suppliers, count: 80, per: month, minutes: 20 }\n    - { what: \"Post, archive\", count: 2.5, per: day, minutes: 30 }\n",
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.savings.len(), uc.savings[2].count, uc.savings[2].per.as_deref()), (3, Some(2.5), Some("day")));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((&last["kind"], &last["field"], &last["from"], &last["to"]), (&json!("project.update"), &json!("usecase.savings"), &old, &new));

    // A use case without savings gets the list appended to its block.
    let file = root.join("projects/support-triage/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    ws.update_project_field("ST", "usecase.savings", &json!([{ "what": "Route ticket", "count": 300, "per": "week", "minutes": 2 }]), "app").unwrap();
    let want = before.replace(
        "  current_state: Need reported, no pilot yet\n",
        "  current_state: Need reported, no pilot yet\n  savings: [{ what: Route ticket, count: 300, per: week, minutes: 2 }]\n",
    );
    assert_eq!(fs::read_to_string(&file).unwrap(), want);
}

#[test]
fn savings_are_validated_on_write() {
    let (_tmp, root) = fixture();
    let file = root.join("projects/invoice-extraction/_project.md");
    let before = fs::read_to_string(&file).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let row = |what: Value, count: Value, per: Value, minutes: Value| json!([{ "what": what, "count": count, "per": per, "minutes": minutes }]);
    for bad in [
        row(json!(" "), json!(1), json!("day"), json!(1)),
        row(json!("X"), json!(-1), json!("day"), json!(1)),
        row(json!("X"), json!("12"), json!("day"), json!(1)),
        row(json!("X"), json!(1), json!("quarter"), json!(1)),
        row(json!("X"), json!(1), json!("day"), Value::Null),
        json!("about 0.5 FTE"),
    ] {
        assert!(matches!(ws.update_project_field("IE", "usecase.savings", &bad, "app"), Err(Error::Invalid(_))), "{bad}");
    }
    assert_eq!(fs::read_to_string(&file).unwrap(), before);
    // Empty is fine; so is a zero.
    ws.update_project_field("IE", "usecase.savings", &row(json!("X"), json!(0), json!("year"), json!(0)), "app").unwrap();
    ws.update_project_field("IE", "usecase.savings", &json!([]), "app").unwrap();
    assert!(fs::read_to_string(&file).unwrap().contains("  savings: []\n"));
}

#[test]
fn hand_written_bad_savings_are_problems_not_breakage() {
    let (_tmp, root) = fixture();
    let ie = root.join("projects/invoice-extraction/_project.md");
    let src = fs::read_to_string(&ie).unwrap()
        .replace("count: 1200, per: month, minutes: 6 }", "count: 1200, per: month, minutes: 6, source: SAP }")
        .replace("count: 80, per: month", "count: lots, per: month");
    fs::write(&ie, src).unwrap();
    let st = root.join("projects/support-triage/_project.md");
    let src = fs::read_to_string(&st).unwrap().replace("  current_state: Need reported, no pilot yet\n", "  current_state: Need reported, no pilot yet\n  savings: about 0.5 FTE\n");
    fs::write(&st, src).unwrap();

    let ws = Workspace::open(&root).unwrap();
    let uc = ws.index.project("IE").unwrap().project.usecase.clone().unwrap();
    assert_eq!((uc.savings.len(), uc.savings[1].count, uc.savings[1].minutes), (2, None, Some(15.0)));
    // Unknown keys ride along, so rewriting the list keeps them.
    assert_eq!(serde_json::to_value(&uc.savings[0]).unwrap()["source"], json!("SAP"));
    assert!(ws.index.project("ST").unwrap().project.usecase.as_ref().unwrap().savings.is_empty());
    let msgs: Vec<(&str, &str)> = ws.index.errors.iter().map(|e| (e.path.as_str(), e.message.as_str())).filter(|(_, m)| m.contains("savings")).collect();
    assert_eq!(
        msgs,
        [
            ("projects/invoice-extraction/_project.md", "usecase.savings 2: count must be a number >= 0"),
            ("projects/support-triage/_project.md", "usecase.savings must be a list"),
        ]
    );
}

// ------------------------------------------------------------------ agents (M5)

#[test]
fn start_and_review_are_lossless_and_logged() {
    let (_tmp, root) = fixture();
    let before = fs::read_to_string(root.join(WR8)).unwrap();
    let mut ws = Workspace::open(&root).unwrap();
    let n = log_lines(&root).len();
    ws.start_task("WR-8", Some("codex"), "agent:codex").unwrap();
    let after = fs::read_to_string(root.join(WR8)).unwrap();
    // Only status changes in place; the new agent block is appended to the frontmatter.
    let (head, tail) = after.split_once("agent:\n").unwrap();
    assert_eq!(head, before.split_once("---\nBody").unwrap().0.replace("status: backlog\n", "status: doing\n"));
    assert!(tail.starts_with("  active: codex\n  since: 20") && tail.contains("---\nBody with trailing spaces   \n"), "{tail}");
    let a = ws.index.task("WR-8").unwrap().task.agent.clone().unwrap();
    assert_eq!(a.active.as_deref(), Some("codex"));
    let lines = log_lines(&root);
    assert_eq!(lines.len(), n + 3);
    assert!(lines[n..].iter().all(|l| l["actor"] == "agent:codex" && l["id"] == "WR-8"));

    ws.review_task("WR-8", Some("A3F9C1E"), "agent:codex").unwrap();
    let t = ws.index.task("WR-8").unwrap().task.clone();
    let a = t.agent.unwrap();
    assert_eq!((t.status.as_str(), a.active, a.since, a.commit.as_deref()), ("review", None, None, Some("a3f9c1e")));
    assert!(a.last_run.is_some());
    let reviewed = fs::read_to_string(root.join(WR8)).unwrap();
    assert!(reviewed.ends_with("- [x] list b") && reviewed.contains("# Kept by hand, do not reorder\n"));
}

#[test]
fn transition_rules() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    // WR-3 is doing, WR-1 done: no start. WR-5 is todo: no review.
    assert!(matches!(ws.start_task("WR-3", Some("codex"), "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.start_task("WR-1", None, "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.review_task("WR-5", None, "cli"), Err(Error::InvalidTransition(_))));
    assert!(matches!(ws.start_task("XX-1", None, "cli"), Err(Error::NotFound(_))));
    assert!(matches!(ws.start_task("WR-5", Some("a b"), "cli"), Err(Error::Invalid(_))));
    assert!(matches!(ws.review_task("WR-3", Some("not-a-sha"), "cli"), Err(Error::Invalid(_))));
    // Without an agent name only the status changes; review from review restarts.
    ws.start_task("WR-7", None, "cli").unwrap();
    let t = &ws.index.task("WR-7").unwrap().task;
    assert_eq!((t.status.as_str(), t.agent.as_ref().unwrap().active.as_deref()), ("doing", None));
    ws.review_task("WR-3", None, "agent:codex").unwrap();
    assert_eq!(ws.index.task("WR-3").unwrap().task.agent.as_ref().unwrap().active, None);
}

#[test]
fn agent_context_and_agents_md() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let ctx = ws.agent_context(Some("WR")).unwrap();
    assert!(ctx.global.unwrap().content.starts_with("# Global agent rules"));
    assert_eq!(ctx.project.unwrap().path, "projects/website-relaunch/agent/AGENTS.md");
    assert_eq!(ctx.skills.len(), 1);
    let s = &ctx.skills[0];
    assert_eq!((s.name.as_str(), s.path.as_str()), ("release-check", "projects/website-relaunch/agent/skills/release-check/SKILL.md"));
    assert!(s.description.starts_with("Checks a build"));
    // Inbox task: global rules only.
    let inbox = ws.agent_context(None).unwrap();
    assert!(inbox.global.is_some() && inbox.project.is_none() && inbox.skills.is_empty());

    let ie = ws.agent_context(Some("IE")).unwrap();
    assert!(ie.project.is_none() && ie.skills.is_empty());
    let path = ws.create_agents_md("IE", "app").unwrap();
    assert_eq!(path, "projects/invoice-extraction/agent/AGENTS.md");
    assert!(fs::read_to_string(root.join(&path)).unwrap().starts_with("# Invoice Extraction"));
    assert!(matches!(ws.create_agents_md("IE", "app"), Err(Error::Invalid(_))));
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.agents_md");
}

#[test]
fn trash_lists_tasks_and_project_folders() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let items = ws.trash_items();
    assert_eq!(items.len(), 1);
    assert_eq!((items[0].kind, items[0].id.as_str()), ("task", "WR-9"));

    ws.delete_project("OI", "app").unwrap();
    ws.delete_task("WR-8", "app").unwrap();
    let items = ws.trash_items();
    let kinds: Vec<(&str, &str)> = items.iter().map(|i| (i.kind, i.id.as_str())).collect();
    assert_eq!(kinds.len(), 3, "{kinds:?}");
    assert_eq!(kinds[0], ("task", "WR-8"), "newest deletion first");
    assert!(kinds.contains(&("project", "OI")), "a project folder is one item, not its files");
    assert!(ws.index.trash.iter().any(|p| p.ends_with("OI-1-legacy-export.md") || p.contains("old-intranet/tasks/")));
}

#[test]
fn restore_project_folder_and_refuse_orphan_task() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let task = ws.index.task("OI-1").unwrap().path.clone();
    ws.delete_task("OI-1", "app").unwrap();
    ws.delete_project("OI", "app").unwrap();
    // Its project folder is gone, so the task cannot go back on its own.
    assert!(matches!(ws.restore(&task, "app"), Err(Error::Invalid(_))));

    ws.restore("projects/archive/old-intranet", "app").unwrap();
    assert!(ws.index.project("OI").is_some());
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "project.restore");
    ws.restore(&task, "app").unwrap();
    assert!(ws.index.task("OI-1").is_some());
    assert_eq!(ws.trash_items().len(), 1, "only WR-9 left");
}

#[test]
fn ids_stay_taken_after_the_trash_is_gone() {
    let (_tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let id = ws.create_task("Short lived", Some("WR"), None, "app").unwrap();
    assert_eq!(id, "WR-10");
    ws.delete_task(&id, "app").unwrap();
    // As after emptying the trash: the file is gone, the log remembers.
    fs::remove_dir_all(root.join(".workly/trash/projects")).unwrap();
    ws.rescan();
    assert_eq!(ws.next_id("WR"), "WR-11");
}

#[test]
fn missing_repos_skip_archived_and_existing() {
    let (tmp, root) = fixture();
    let mut ws = Workspace::open(&root).unwrap();
    let repo = tmp.path().join("repo");
    fs::create_dir(&repo).unwrap();
    ws.update_project_field("WR", "repos", &json!([repo.display().to_string(), "~/no/such/repo"]), "app").unwrap();
    let missing: Vec<(String, String)> = ws.missing_repos().into_iter().map(|m| (m.key, m.repo)).collect();
    assert!(missing.contains(&("WR".into(), "~/no/such/repo".into())));
    assert!(!missing.iter().any(|(_, r)| r == &repo.display().to_string()));
    assert!(!missing.iter().any(|(k, _)| k == "OI"));
}
