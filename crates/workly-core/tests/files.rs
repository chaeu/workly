//! Project files (B16): tree, body editor, create, delete, path checks.
//! Always against a temp copy of the fixture.

use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};
use workly_core::files::FileNode;
use workly_core::{Error, Workspace};

const FIXTURE: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../fixtures/workspace");
const WR: &str = "projects/website-relaunch";
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

fn fixture() -> (tempfile::TempDir, PathBuf, Workspace) {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("ws");
    copy_dir(Path::new(FIXTURE), &root);
    let ws = Workspace::open(&root).unwrap();
    let root = ws.root().to_path_buf();
    (dir, root, ws)
}

fn log_lines(root: &Path) -> Vec<Value> {
    let mut lines = Vec::new();
    for e in fs::read_dir(root.join(".workly/log")).into_iter().flatten().flatten() {
        lines.extend(fs::read_to_string(e.path()).unwrap().lines().map(|l| serde_json::from_str::<Value>(l).unwrap()));
    }
    lines
}

/// "name" for a file, "name/" for a folder, children indented.
fn outline(nodes: &[FileNode], depth: usize, out: &mut Vec<String>) {
    for n in nodes {
        let pad = "  ".repeat(depth);
        match &n.children {
            None => out.push(format!("{pad}{}", n.name)),
            Some(c) => {
                out.push(format!("{pad}{}/", n.name));
                outline(c, depth + 1, out);
            }
        }
    }
}

fn tree(ws: &Workspace, key: &str) -> Vec<String> {
    let mut out = Vec::new();
    outline(&ws.project_files(key).unwrap(), 0, &mut out);
    out
}

#[test]
fn tree_lists_project_files_and_empty_folders() {
    let (_tmp, root, mut ws) = fixture();
    let wr = root.join(WR);
    fs::write(wr.join(".hidden.md"), "x").unwrap();
    fs::write(wr.join("WR.code-workspace"), "{}").unwrap();
    fs::write(wr.join("brief.pdf"), "%PDF").unwrap();
    ws.create_folder("WR", &format!("{WR}/notes"), "kunde-x", "app").unwrap();
    assert_eq!(
        tree(&ws, "WR"),
        ["decisions/", "  0001-hosting.md", "docs/", "  architecture.md", "notes/", "  kunde-x/", "  2026-09-15-kickoff.md", "brief.pdf"]
    );
    let nodes = ws.project_files("WR").unwrap();
    assert_eq!(nodes[0].path, format!("{WR}/decisions"));
    assert_eq!(nodes[0].children.as_ref().unwrap()[0].path, format!("{WR}/decisions/0001-hosting.md"));
    assert!(ws.project_files("ST").unwrap().is_empty());
}

#[test]
fn set_body_keeps_frontmatter_bytes() {
    let (_tmp, root, mut ws) = fixture();
    // Tasks keep their own write paths; WR-8 is refused and untouched.
    let wr8 = fs::read(root.join(WR8)).unwrap();
    assert!(matches!(ws.set_body(WR8, "x", "", "app"), Err(Error::Invalid(_))));
    assert_eq!(fs::read(root.join(WR8)).unwrap(), wr8);

    // The same tricky frontmatter as a note: only the body changes.
    let note = format!("{WR}/notes/wr8-copy.md");
    fs::copy(root.join(WR8), root.join(&note)).unwrap();
    let src = String::from_utf8(wr8).unwrap();
    let (head, body) = src.split_at(src.find("Body with").unwrap());
    ws.set_body(&note, "New body\n", body, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(&note)).unwrap(), format!("{head}New body\n"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((last["kind"].as_str(), last["id"].as_str(), last["to"].as_str()), (Some("file.body"), Some("WR"), Some(note.as_str())));

    // Unchanged body: no write, no log line.
    let n = log_lines(&root).len();
    ws.set_body(&note, "New body\n", "New body\n", "app").unwrap();
    assert_eq!(log_lines(&root).len(), n);
}

#[test]
fn set_body_conflict_writes_nothing() {
    let (_tmp, root, mut ws) = fixture();
    let note = format!("{WR}/notes/2026-09-15-kickoff.md");
    let before = fs::read(root.join(&note)).unwrap();
    assert!(matches!(ws.set_body(&note, "mine", "what I saw earlier", "app"), Err(Error::Conflict(_))));
    assert_eq!(fs::read(root.join(&note)).unwrap(), before);
}

#[test]
fn set_body_without_frontmatter_and_with_crlf() {
    let (_tmp, root, mut ws) = fixture();
    let doc = format!("{WR}/docs/architecture.md");
    let old = fs::read_to_string(root.join(&doc)).unwrap();
    ws.set_body(&doc, "# Architecture\n\nNew.\n", &old, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(&doc)).unwrap(), "# Architecture\n\nNew.\n");

    let crlf = format!("{WR}/notes/crlf.md");
    fs::write(root.join(&crlf), "---\r\ntags: [a]\r\n---\r\nold\r\n").unwrap();
    ws.set_body(&crlf, "new\r\nline\r\n", "old\r\n", "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(&crlf)).unwrap(), "---\r\ntags: [a]\r\n---\r\nnew\r\nline\r\n");

    // Closing fence at the end of the file without a line break.
    let bare = format!("{WR}/notes/bare.md");
    fs::write(root.join(&bare), "---\ntags: [a]\n---").unwrap();
    ws.set_body(&bare, "text\n", "", "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(&bare)).unwrap(), "---\ntags: [a]\n---\ntext\n");
}

#[test]
fn create_file_and_folder() {
    let (_tmp, root, mut ws) = fixture();
    let notes = format!("{WR}/notes");
    let dir = ws.create_folder("WR", &notes, "kunde-x", "app").unwrap();
    assert_eq!(dir, format!("{notes}/kunde-x"));
    let file = ws.create_file("WR", &dir, " kickoff ", "app").unwrap();
    assert_eq!(file, format!("{notes}/kunde-x/kickoff.md"));
    assert_eq!(fs::read_to_string(root.join(&file)).unwrap(), "");
    assert_eq!(ws.create_file("WR", WR, "plan.md", "app").unwrap(), format!("{WR}/plan.md"));
    let last = log_lines(&root).pop().unwrap();
    assert_eq!((last["kind"].as_str(), last["to"].as_str()), (Some("file.create"), Some(format!("{WR}/plan.md").as_str())));

    // Existing names, bad names, locked or foreign folders: refused.
    for (dir, name) in [
        (dir.as_str(), "kickoff.md"),
        (notes.as_str(), ""),
        (notes.as_str(), "  "),
        (notes.as_str(), ".hidden"),
        (notes.as_str(), ".."),
        (notes.as_str(), "../x"),
        (notes.as_str(), "a/b"),
        (&format!("{WR}/tasks"), "x"),
        (&format!("{WR}/agent"), "x"),
        (".workly", "x"),
        ("projects", "x"),
        ("", "x"),
    ] {
        assert!(matches!(ws.create_file("WR", dir, name, "app"), Err(Error::Invalid(_))), "file {dir} / {name}");
        assert!(matches!(ws.create_folder("WR", dir, name, "app"), Err(Error::Invalid(_))), "folder {dir} / {name}");
    }
    assert!(matches!(ws.create_folder("WR", &notes, "kunde-x", "app"), Err(Error::Invalid(_))));
    // tasks/ and agent/ at the top are reserved (a file "tasks.md" is fine).
    fs::remove_dir_all(root.join(WR).join("agent")).unwrap();
    assert!(matches!(ws.create_folder("WR", WR, "agent", "app"), Err(Error::Invalid(_))));
    // A folder of another project.
    assert!(matches!(ws.create_file("ST", &notes, "x", "app"), Err(Error::Invalid(_))));
    assert!(!root.join(WR).join("tasks/x.md").exists());
}

#[test]
fn delete_file_and_folder_to_trash_and_restore() {
    let (_tmp, root, mut ws) = fixture();
    let note = format!("{WR}/notes/2026-09-15-kickoff.md");
    let bytes = fs::read(root.join(&note)).unwrap();
    ws.delete_path(&note, "app").unwrap();
    assert!(!root.join(&note).exists());
    assert_eq!(fs::read(root.join(".workly/trash").join(&note)).unwrap(), bytes, "no stamp in a note");
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "file.delete");
    let item = ws.trash_items().into_iter().find(|i| i.path == note).unwrap();
    assert_eq!(item.kind, "file");

    // A folder with a sub-folder, a note and a PDF is one item.
    let dir = ws.create_folder("WR", &format!("{WR}/docs"), "specs", "app").unwrap();
    ws.create_folder("WR", &dir, "old", "app").unwrap();
    let f = ws.create_file("WR", &format!("{dir}/old"), "a", "app").unwrap();
    ws.set_body(&f, "text\n", "", "app").unwrap();
    fs::write(root.join(&dir).join("brief.pdf"), "%PDF").unwrap();
    ws.delete_path(&dir, "app").unwrap();
    assert!(!root.join(&dir).exists());
    let items = ws.trash_items();
    let mine: Vec<(&str, &str)> = items.iter().filter(|i| i.kind != "task").map(|i| (i.kind, i.path.as_str())).collect();
    assert_eq!(mine.len(), 2, "{mine:?}");
    assert!(mine.contains(&("folder", dir.as_str())));
    assert!(mine.contains(&("file", note.as_str())));
    assert!(items.iter().any(|i| i.kind == "task" && i.id == "WR-9"), "older task stays its own item");

    ws.restore(&dir, "app").unwrap();
    assert_eq!(fs::read_to_string(root.join(&f)).unwrap(), "text\n");
    assert!(root.join(&dir).join("brief.pdf").is_file());
    assert_eq!(log_lines(&root).pop().unwrap()["kind"], "file.restore");
    ws.restore(&note, "app").unwrap();
    assert_eq!(fs::read(root.join(&note)).unwrap(), bytes);
    assert_eq!(ws.trash_items().len(), 1, "only WR-9 left");

    // A PDF on its own.
    let pdf = format!("{WR}/brief.pdf");
    fs::write(root.join(&pdf), "%PDF").unwrap();
    ws.delete_path(&pdf, "app").unwrap();
    assert!(ws.trash_items().iter().any(|i| i.kind == "file" && i.path == pdf));
    ws.restore(&pdf, "app").unwrap();
    assert!(root.join(&pdf).is_file());
}

#[test]
fn delete_refuses_project_tasks_and_agent() {
    let (_tmp, root, mut ws) = fixture();
    for path in [WR.to_string(), format!("{WR}/_project.md"), WR8.to_string(), format!("{WR}/tasks"), format!("{WR}/agent/AGENTS.md"), ".workly/config.yml".into()] {
        assert!(matches!(ws.delete_path(&path, "app"), Err(Error::Invalid(_))), "{path}");
        assert!(root.join(&path).exists(), "{path}");
    }
}

#[test]
fn paths_cannot_escape_the_project() {
    let (tmp, root, mut ws) = fixture();
    let outside = tmp.path().join("outside.md");
    fs::write(&outside, "secret").unwrap();
    std::os::unix::fs::symlink(&outside, root.join(WR).join("notes/link.md")).unwrap();
    std::os::unix::fs::symlink(tmp.path(), root.join(WR).join("docs/out")).unwrap();
    for path in [
        "../outside.md".to_string(),
        outside.to_string_lossy().into_owned(),
        format!("{WR}/../../../outside.md"),
        format!("{WR}/notes/link.md"),
        format!("{WR}/docs/out/outside.md"),
        "_templates/note.md".into(),
    ] {
        assert!(matches!(ws.set_body(&path, "x", "secret", "app"), Err(Error::Invalid(_))), "{path}");
        assert!(ws.delete_path(&path, "app").is_err(), "{path}");
    }
    assert!(matches!(ws.create_file("WR", &format!("{WR}/docs/out"), "x", "app"), Err(Error::Invalid(_))));
    assert_eq!(fs::read_to_string(&outside).unwrap(), "secret");
    assert!(!tmp.path().join("x.md").exists());
}

#[test]
fn project_template_falls_back_to_built_in() {
    let (_tmp, root, ws) = fixture();
    assert!(ws.project_template().starts_with("## Summary\n"), "{}", ws.project_template());
    fs::write(root.join("_templates/project.md"), "---\nkey: x\n---\nMine\n").unwrap();
    assert_eq!(ws.project_template(), "Mine\n");
    fs::remove_file(root.join("_templates/project.md")).unwrap();
    assert!(ws.project_template().starts_with("## Summary\n"));
}
