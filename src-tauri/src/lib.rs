use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;
use std::sync::{Mutex, OnceLock, mpsc};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Emitter, LogicalSize, Manager, RunEvent, State, WebviewWindow};
use tauri_plugin_window_state::StateFlags;
use workly_core::agent::{AgentContext, RunEntry};
use workly_core::backup::{self, BackupInfo};
use workly_core::changes::FileChange;
use workly_core::feed::FeedEvent;
use workly_core::frontmatter::AcceptanceItem;
use workly_core::history::PhaseSpan;
use workly_core::model::{Config, Process};
use workly_core::project::{MissingRepo, NewProject, NewUseCase, init_workspace, is_workspace};
use workly_core::scan::Index;
use workly_core::settings::{self, Settings, expand_home};
use workly_core::trash::TrashItem;
use workly_core::watch::Watcher;
use workly_core::{NewTask, Workspace};

const ACTOR: &str = "app";

/// Process start, for the one "ready" line on stderr.
static STARTED: OnceLock<Instant> = OnceLock::new();

#[derive(Default)]
struct AppState {
    ws: Mutex<Option<Workspace>>,
    watcher: Mutex<Option<Watcher>>,
    /// Why the workspace from the settings did not open at startup.
    open_error: Mutex<Option<String>>,
    backup: Mutex<BackupStatus>,
    /// Held while a backup runs, so runs never overlap.
    backup_running: Mutex<()>,
    /// Last finished run; the timer counts the interval from here.
    backup_last: Mutex<Option<Instant>>,
}

/// What the sidebar shows. Runtime only: the commits are the record.
#[derive(Clone, Default, Serialize)]
struct BackupStatus {
    /// off | running | ok | local | failed
    state: &'static str,
    /// End of the last run, ms since the epoch.
    at: Option<u64>,
    upstream: Option<String>,
    /// Message and git's stderr tail.
    error: Option<String>,
}

/// Run `f` on the open workspace; errors become strings for the frontend.
fn with_ws<T>(state: &AppState, f: impl FnOnce(&mut Workspace) -> workly_core::Result<T>) -> Result<T, String> {
    let mut guard = state.ws.lock().unwrap();
    let Some(ws) = guard.as_mut() else {
        return Err(state.open_error.lock().unwrap().clone().unwrap_or_else(|| "No workspace open.".into()));
    };
    f(ws).map_err(|e| e.to_string())
}

/// Own writes are not reported by the watcher, so announce them here.
fn changed<T>(app: &AppHandle, result: Result<T, String>) -> Result<T, String> {
    if result.is_ok() {
        let _ = app.emit("workspace-changed", ());
    }
    result
}

/// Everything the views need from the open workspace.
#[derive(Serialize)]
struct Snapshot {
    root: String,
    config: Config,
    process: Option<Process>,
    missing_repos: Vec<MissingRepo>,
    /// Last activity per task with an active agent (run log, else agent.since).
    agent_activity: BTreeMap<String, String>,
    /// Tasks whose active agent stopped with a BLOCKED note: the note, by id.
    agent_blocked: BTreeMap<String, String>,
    #[serde(flatten)]
    index: Index,
}

#[tauri::command]
fn get_index(state: State<AppState>) -> Result<Snapshot, String> {
    with_ws(&state, |ws| {
        let root = ws.root().display().to_string();
        let missing_repos = ws.missing_repos();
        let agent_activity = ws.agent_activity();
        let agent_blocked = ws.agent_blocked();
        Ok(Snapshot { root, config: ws.config.clone(), process: ws.process.clone(), missing_repos, agent_activity, agent_blocked, index: ws.index.clone() })
    })
}

/// Called once by the frontend after its first render with data.
#[tauri::command]
fn app_ready(state: State<AppState>) {
    static ONCE: OnceLock<()> = OnceLock::new();
    if ONCE.set(()).is_ok() {
        let tasks = state.ws.lock().unwrap().as_ref().map_or(0, |ws| ws.index.tasks.len());
        let ms = STARTED.get().map_or(0, |t| t.elapsed().as_millis());
        eprintln!("workly: ready after {ms} ms ({tasks} tasks)");
    }
}

// ------------------------------------------------------------------ tasks

#[tauri::command]
fn update_task_field(app: AppHandle, state: State<AppState>, id: String, field: String, value: Value) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.update_task_field(&id, &field, &value, ACTOR)))
}

#[tauri::command]
fn create_task(app: AppHandle, state: State<AppState>, task: NewTask) -> Result<String, String> {
    changed(&app, with_ws(&state, |ws| ws.create_task(&task, ACTOR)))
}

#[tauri::command]
fn add_waiting_status(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.add_waiting_status(ACTOR)))
}

#[tauri::command]
fn reorder_tasks(app: AppHandle, state: State<AppState>, ids: Vec<String>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.reorder_tasks(&ids, ACTOR)))
}

#[tauri::command]
fn set_focus(app: AppHandle, state: State<AppState>, ids: Vec<String>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.set_focus(&ids, ACTOR)))
}

#[tauri::command]
fn add_task_update(app: AppHandle, state: State<AppState>, id: String, text: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.add_task_update(&id, &text, ACTOR)))
}

/// True when saved, false when the description changed outside (nothing written).
#[tauri::command]
fn set_description(app: AppHandle, state: State<AppState>, id: String, text: String, expected: String) -> Result<bool, String> {
    changed(
        &app,
        with_ws(&state, |ws| match ws.set_description(&id, &text, &expected, ACTOR) {
            Err(workly_core::Error::Conflict(_)) => Ok(false),
            r => r.map(|()| true),
        }),
    )
}

#[tauri::command]
fn toggle_acceptance(app: AppHandle, state: State<AppState>, id: String, n: usize, shown: AcceptanceItem) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.toggle_acceptance(&id, n, &shown, ACTOR)))
}

#[tauri::command]
fn add_acceptance(app: AppHandle, state: State<AppState>, id: String, text: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.add_acceptance(&id, &text, ACTOR)))
}

#[tauri::command]
fn add_sections(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.add_sections(&id, ACTOR)))
}

#[tauri::command]
fn move_task(app: AppHandle, state: State<AppState>, id: String, project: Option<String>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.move_task(&id, project.as_deref(), ACTOR)))
}

#[tauri::command]
fn delete_task(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.delete_task(&id, ACTOR)))
}

#[tauri::command]
fn restore(app: AppHandle, state: State<AppState>, path: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.restore(&path, ACTOR)))
}

#[tauri::command]
fn trash_items(state: State<AppState>) -> Result<Vec<TrashItem>, String> {
    with_ws(&state, |ws| Ok(ws.trash_items()))
}

/// Moves the trash contents to the macOS Trash; returns how many items.
#[tauri::command]
fn empty_trash(app: AppHandle, state: State<AppState>) -> Result<usize, String> {
    changed(&app, with_ws(&state, |ws| ws.empty_trash(ACTOR)))
}

// --------------------------------------------------------------- projects

#[tauri::command]
fn suggest_key(state: State<AppState>, title: String) -> Result<String, String> {
    with_ws(&state, |ws| Ok(ws.suggest_key(&title)))
}

/// The new folder, or None when a folder without `_project.md` is in the way (nothing written; retry with `adopt`).
#[tauri::command]
fn create_project(app: AppHandle, state: State<AppState>, project: NewProject) -> Result<Option<String>, String> {
    changed(
        &app,
        with_ws(&state, |ws| match ws.create_project(&project, ACTOR) {
            Err(workly_core::Error::Conflict(_)) => Ok(None),
            r => r.map(Some),
        }),
    )
}

#[tauri::command]
fn update_project_field(app: AppHandle, state: State<AppState>, key: String, field: String, value: Value) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.update_project_field(&key, &field, &value, ACTOR)))
}

#[tauri::command]
fn make_usecase(app: AppHandle, state: State<AppState>, key: String, usecase: NewUseCase) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.make_usecase(&key, &usecase, ACTOR)))
}

#[tauri::command]
fn remove_usecase(app: AppHandle, state: State<AppState>, key: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.remove_usecase(&key, ACTOR)))
}

#[tauri::command]
fn move_usecase(app: AppHandle, state: State<AppState>, key: String, step: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.move_usecase(&key, &step, ACTOR)))
}

#[tauri::command]
fn reorder_projects(app: AppHandle, state: State<AppState>, keys: Vec<String>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.reorder_projects(&keys, ACTOR)))
}

#[tauri::command]
fn delete_project(app: AppHandle, state: State<AppState>, key: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.delete_project(&key, ACTOR)))
}

#[tauri::command]
fn project_files(state: State<AppState>, key: String) -> Result<Vec<String>, String> {
    with_ws(&state, |ws| ws.project_files(&key))
}

#[tauri::command]
fn read_markdown(state: State<AppState>, path: String) -> Result<String, String> {
    with_ws(&state, |ws| ws.read_markdown(&path))
}

#[tauri::command]
fn usecase_history(state: State<AppState>, key: String) -> Result<Vec<PhaseSpan>, String> {
    with_ws(&state, |ws| ws.usecase_history(&key))
}

// ----------------------------------------------------------------- agents

#[tauri::command]
fn agent_context(state: State<AppState>, key: Option<String>) -> Result<AgentContext, String> {
    with_ws(&state, |ws| ws.agent_context(key.as_deref()))
}

#[tauri::command]
fn run_log(state: State<AppState>, id: String) -> Result<Vec<RunEntry>, String> {
    with_ws(&state, |ws| ws.run_log(&id))
}

#[tauri::command]
fn agent_feed(state: State<AppState>) -> Result<Vec<FeedEvent>, String> {
    with_ws(&state, |ws| Ok(ws.agent_feed()))
}

#[derive(Serialize)]
struct Tool {
    path: String,
    version: Option<String>,
}

/// Agents → Setup: what a start needs on this Mac, and the global rules next to the built-in ones.
#[derive(Serialize)]
struct AgentChecks {
    codex: Option<Tool>,
    vscode: Option<Tool>,
    /// missing | builtin | custom
    rules: &'static str,
    builtin_rules: &'static str,
}

/// Off the UI thread: Spotlight and `codex --version` take a moment.
#[tauri::command]
async fn agent_checks(state: State<'_, AppState>) -> Result<AgentChecks, String> {
    let rules = with_ws(&state, |ws| Ok(ws.global_rules()))?;
    let job = move || AgentChecks {
        codex: codex_bin().map(|p| Tool { version: version_line(&p), path: settings::tilde(&p) }),
        vscode: vscode_app().ok().map(|(v, app)| Tool {
            path: settings::tilde(&app),
            version: Some(v.iter().map(u32::to_string).collect::<Vec<_>>().join(".")),
        }),
        rules,
        builtin_rules: workly_core::agent::builtin_rules(),
    };
    tauri::async_runtime::spawn_blocking(job).await.map_err(|e| e.to_string())
}

/// Where start.command will most likely find `codex`. The script itself asks the login shell.
// ponytail: fixed places, no shell (an interactive shell can hang); add a place when a setup misses.
fn codex_bin() -> Option<std::path::PathBuf> {
    let home = expand_home("~");
    let fixed = [".local/bin/codex", ".cargo/bin/codex", ".npm-global/bin/codex", ".bun/bin/codex"].map(|p| home.join(p));
    let system = ["/opt/homebrew/bin/codex", "/usr/local/bin/codex"].map(std::path::PathBuf::from);
    let newest = |dir: std::path::PathBuf, tail: &str| {
        let mut found: Vec<_> = std::fs::read_dir(dir).into_iter().flatten().flatten().map(|e| e.path().join(tail)).filter(|p| p.is_file()).collect();
        found.sort_by_key(|p| std::fs::metadata(p).and_then(|m| m.modified()).ok());
        found.pop()
    };
    (fixed.into_iter().chain(system))
        .find(|p| p.is_file())
        .or_else(|| newest(home.join(".nvm/versions/node"), "bin/codex"))
        .or_else(|| newest(home.join(".vscode/extensions"), "bin/macos-aarch64/codex").filter(|p| p.to_string_lossy().contains("openai.chatgpt-")))
}

/// First line of `<bin> --version`, None when it does not run (e.g. a node script without node on the app's PATH).
fn version_line(bin: &Path) -> Option<String> {
    let out = Command::new(bin).arg("--version").stdin(std::process::Stdio::null()).output().ok()?;
    let line = String::from_utf8_lossy(&out.stdout).lines().next()?.trim().to_string();
    (out.status.success() && !line.is_empty()).then_some(line)
}

#[tauri::command]
fn set_agents(app: AppHandle, state: State<AppState>, agents: Vec<workly_core::model::AgentPreset>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.set_agents(&agents, ACTOR)))
}

#[tauri::command]
fn set_task_agent(app: AppHandle, state: State<AppState>, id: String, agent: Option<workly_core::model::AgentPreset>) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.set_task_agent(&id, agent.as_ref(), ACTOR)))
}

#[tauri::command]
fn clear_agent(app: AppHandle, state: State<AppState>, id: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.clear_agent(&id, ACTOR)))
}

#[tauri::command]
fn create_agents_md(app: AppHandle, state: State<AppState>, key: String) -> Result<String, String> {
    changed(&app, with_ws(&state, |ws| ws.create_agents_md(&key, ACTOR)))
}

#[derive(Serialize)]
struct CliLink {
    target: String,
    /// False when the target is gone, e.g. a removed dev build.
    ok: bool,
}

/// Where `~/.local/bin/wly` points, if it is a link.
#[tauri::command]
fn cli_link() -> Option<CliLink> {
    let target = std::fs::read_link(settings::cli_link_path()).ok()?;
    Some(CliLink { ok: target.is_file(), target: target.display().to_string() })
}

/// Link `~/.local/bin/wly` to this binary, which runs the CLI when started as `wly`.
#[tauri::command]
fn install_cli() -> Result<String, String> {
    let exe = std::env::current_exe().and_then(|p| p.canonicalize()).map_err(|e| e.to_string())?;
    let link = settings::cli_link_path();
    settings::link_cli(&exe, &link).map_err(|e| e.to_string())?;
    Ok(settings::tilde(&link))
}

// ------------------------------------------------------- open elsewhere

/// macOS `open` with argv, never through a shell.
fn open(args: &[&str]) -> Result<(), String> {
    let out = Command::new("open").args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
    }
}

const VSCODE: &str = "com.microsoft.VSCode";

/// Obsidian, browser and mail links. Other schemes (file:, javascript:, custom
/// app handlers) are refused, since URLs can come from Markdown content.
#[tauri::command]
fn open_url(url: String) -> Result<(), String> {
    let scheme = url.split_once(':').map(|(s, _)| s.to_ascii_lowercase()).unwrap_or_default();
    if !["obsidian", "http", "https", "mailto"].contains(&scheme.as_str()) {
        return Err(format!("Refusing to open {url}"));
    }
    open(&[&url])
}

/// Workspace-relative path in VS Code, at `line` if given.
#[tauri::command]
fn open_in_vscode(state: State<AppState>, path: String, line: Option<u32>) -> Result<(), String> {
    let abs = with_ws(&state, |ws| ws.resolve(&path))?;
    match line {
        None => open(&["-b", VSCODE, &abs.to_string_lossy()]),
        Some(line) => {
            let enc: String = (abs.to_string_lossy().bytes())
                .map(|b| if b.is_ascii_alphanumeric() || b"-._~/".contains(&b) { (b as char).to_string() } else { format!("%{b:02X}") })
                .collect();
            open(&[&format!("vscode://file{enc}:{line}")])
        }
    }
}

/// The project's `<key>.code-workspace` in VS Code, rewritten first: its
/// absolute paths belong to this Mac, the synced file may come from another.
#[tauri::command]
fn open_project_in_vscode(state: State<AppState>, key: String) -> Result<(), String> {
    let file = with_ws(&state, |ws| ws.code_workspace(&key, true))?;
    open(&["-b", VSCODE, &file.to_string_lossy()])
}

/// A repo listed in a project's `repos`, in VS Code.
#[tauri::command]
fn open_repo(state: State<AppState>, repo: String) -> Result<(), String> {
    let known = with_ws(&state, |ws| Ok(ws.index.projects.iter().any(|p| p.project.repos.contains(&repo))))?;
    if !known {
        return Err(format!("{repo} is not a project repo"));
    }
    let path = expand_home(&repo);
    if !path.is_dir() {
        return Err(format!("Repo folder not found: {repo}"));
    }
    open(&["-b", VSCODE, &path.to_string_lossy()])
}

/// Start a task for an agent from the app: status doing (from doing only the agent
/// fields), refused while another agent works in the same repo. Async: the
/// baseline runs git.
#[tauri::command]
async fn start_agent_task(app: AppHandle, state: State<'_, AppState>, id: String, agent: String) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.start_task(&id, Some(&agent), ACTOR)))
}

/// Copy prompt on a task already in doing: a baseline unless it has one.
#[tauri::command]
async fn ensure_baseline(state: State<'_, AppState>, id: String) -> Result<bool, String> {
    with_ws(&state, |ws| ws.ensure_baseline(&id))
}

#[tauri::command]
async fn task_changes(state: State<'_, AppState>, id: String) -> Result<Vec<FileChange>, String> {
    with_ws(&state, |ws| ws.task_changes(&id))
}

#[tauri::command]
async fn change_diff(state: State<'_, AppState>, id: String, path: String) -> Result<String, String> {
    with_ws(&state, |ws| ws.change_diff(&id, &path))
}

/// Commit the task's changes in its repo; returns the short sha. git add/commit run
/// without the workspace lock (a pre-commit hook may take up to two minutes).
#[tauri::command]
async fn commit_task(app: AppHandle, state: State<'_, AppState>, id: String, accept_drift: bool) -> Result<String, String> {
    let plan = with_ws(&state, |ws| ws.prepare_commit(&id, accept_drift))?;
    let run = plan.clone();
    let sha = tauri::async_runtime::spawn_blocking(move || run.run()).await.map_err(|e| e.to_string())?.map_err(|e| e.to_string())?;
    changed(&app, with_ws(&state, |ws| ws.record_commit(&plan, (&sha.0, &sha.1), ACTOR)))?;
    Ok(sha.0)
}

/// Hand a started task to its runner, off the UI thread. Copilot: the project's
/// `<key>.code-workspace` in VS Code, then an agent-mode chat with prompt.md attached.
/// Codex: `.workly/runs/<ID>/start.command` in a new Terminal window. Errors carry
/// the stderr tail; the frontend then falls back to the clipboard.
#[tauri::command]
async fn launch_agent(state: State<'_, AppState>, id: String, runner: String, prompt: String) -> Result<(), String> {
    let job: Box<dyn FnOnce() -> Result<(), String> + Send> = match runner.as_str() {
        "codex" => {
            let script = with_ws(&state, |ws| ws.write_codex_start(&id, &prompt))?;
            Box::new(move || open(&["-a", "Terminal", &script.to_string_lossy()]))
        }
        "copilot" => {
            let (workspace, prompt) = with_ws(&state, |ws| {
                ws.repo_dir(&id)?;
                let folder = ws.index.task(&id).and_then(|t| t.project.clone());
                let key = (ws.index.projects.iter()).find(|p| Some(&p.path) == folder.as_ref()).map(|p| p.project.key.clone());
                let key = key.ok_or_else(|| workly_core::Error::Invalid(format!("{id} has no project")))?;
                Ok((ws.code_workspace(&key, true)?, ws.write_run_prompt(&id, &prompt)?))
            })?;
            Box::new(move || {
                let code = vscode_cli()?;
                exec(&code, &[&workspace.to_string_lossy()])?;
                // ponytail: `code` returns before VS Code has the window; `chat -r` goes to the
                // last active one. Fixed pause; raise it if chats land in the wrong window.
                thread::sleep(Duration::from_millis(1500));
                let line = format!("Work on Workly task {id}. Follow the attached prompt.md exactly.");
                exec(&code, &["chat", "-m", "agent", "-r", "-a", &prompt.to_string_lossy(), &line])
            })
        }
        _ => return Err(format!("No adapter for runner {runner}: pick Copilot or Codex")),
    };
    tauri::async_runtime::spawn_blocking(job).await.map_err(|e| e.to_string())?
}

/// `code` of the newest VS Code ≥ 1.98 (for `code chat`).
fn vscode_cli() -> Result<std::path::PathBuf, String> {
    vscode_app().map(|(_, app)| app.join("Contents/Resources/app/bin/code"))
}

/// Newest VS Code ≥ 1.98 with its `code` CLI, as (version, app): /Applications first, then
/// Spotlight by bundle id. Never PATH: an app started from Finder does not get the user's.
fn vscode_app() -> Result<(Vec<u32>, std::path::PathBuf), String> {
    let mut apps = vec![std::path::PathBuf::from("/Applications/Visual Studio Code.app")];
    if let Ok(out) = Command::new("/usr/bin/mdfind").arg(format!("kMDItemCFBundleIdentifier == '{VSCODE}'")).output() {
        apps.extend(String::from_utf8_lossy(&out.stdout).lines().map(std::path::PathBuf::from));
    }
    let version = |app: &Path| -> Option<Vec<u32>> {
        let plist = app.join("Contents/Info.plist");
        let out = Command::new("/usr/bin/plutil").args(["-extract", "CFBundleShortVersionString", "raw", "-o", "-"]).arg(plist).output().ok()?;
        String::from_utf8_lossy(&out.stdout).trim().split('.').map(|n| n.parse().ok()).collect()
    };
    (apps.iter())
        .filter(|app| app.join("Contents/Resources/app/bin/code").is_file())
        .filter_map(|app| Some((version(app).filter(|v| v[..] >= [1, 98][..])?, app.clone())))
        .max_by(|a, b| a.0.cmp(&b.0))
        .ok_or_else(|| "VS Code 1.98 or newer not found".into())
}

/// Run a program with argv (no shell); a failure returns the stderr tail.
fn exec(program: &Path, args: &[&str]) -> Result<(), String> {
    let out = Command::new(program).args(args).output().map_err(|e| format!("{}: {e}", program.display()))?;
    if out.status.success() {
        return Ok(());
    }
    let err = String::from_utf8_lossy(&out.stderr);
    let tail: Vec<&str> = err.lines().rev().take(5).collect();
    Err(tail.into_iter().rev().collect::<Vec<_>>().join("\n"))
}

/// Text to the macOS clipboard (`pbcopy`, no shell): the agent prompt.
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    use std::io::Write;
    use std::process::Stdio;
    let mut child = Command::new("pbcopy").stdin(Stdio::piped()).spawn().map_err(|e| e.to_string())?;
    child.stdin.take().expect("piped").write_all(text.as_bytes()).map_err(|e| e.to_string())?;
    let status = child.wait().map_err(|e| e.to_string())?;
    if status.success() {
        Ok(())
    } else {
        Err("pbcopy failed".into())
    }
}

/// Workspace-relative path selected in Finder.
#[tauri::command]
fn reveal(state: State<AppState>, path: String) -> Result<(), String> {
    let abs = with_ws(&state, |ws| ws.resolve(&path))?;
    open(&["-R", &abs.to_string_lossy()])
}

// ----------------------------------------------------------------- backup

#[derive(Clone, Copy, PartialEq)]
enum Trigger {
    /// Timer: only when the interval has passed since the last run.
    Tick,
    /// Workspace opened, app quits: whenever backup is on.
    Event,
    /// "Back up now": also when the interval is off.
    Manual,
}

fn set_backup(app: &AppHandle, status: BackupStatus) {
    *app.state::<AppState>().backup.lock().unwrap() = status.clone();
    let _ = app.emit("backup-status", status);
}

/// One backup of the open workspace. Blocks: call it off the UI thread.
fn backup_once(app: &AppHandle, trigger: Trigger) {
    let state = app.state::<AppState>();
    let Ok(_running) = state.backup_running.try_lock() else { return };
    let Some(root) = state.ws.lock().unwrap().as_ref().map(|ws| ws.root().to_path_buf()) else { return };
    let cfg = Settings::load(&settings::default_path()).unwrap_or_default().backup_for(&root);
    if !backup::is_repo(&root) || (cfg.every().is_none() && trigger != Trigger::Manual) {
        return set_backup(app, BackupStatus { state: "off", ..Default::default() });
    }
    let last = *state.backup_last.lock().unwrap();
    if trigger == Trigger::Tick && last.zip(cfg.every()).is_some_and(|(t, every)| t.elapsed() < every) {
        return;
    }
    let prev = state.backup.lock().unwrap().clone();
    set_backup(app, BackupStatus { state: "running", ..prev });
    let b = backup::backup(&root, cfg.push);
    *state.backup_last.lock().unwrap() = Some(Instant::now());
    let at = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).ok();
    let label = match (&b.error, b.pushed) {
        (Some(_), _) => "failed",
        (None, true) => "ok",
        (None, false) => "local",
    };
    set_backup(app, BackupStatus { state: label, at, upstream: b.upstream, error: b.error });
}

fn spawn_backup(app: &AppHandle, trigger: Trigger) -> thread::JoinHandle<()> {
    let app = app.clone();
    thread::spawn(move || backup_once(&app, trigger))
}

#[tauri::command]
fn backup_status(state: State<AppState>) -> BackupStatus {
    state.backup.lock().unwrap().clone()
}

/// Starts a run and returns; the result arrives as `backup-status`.
#[tauri::command]
fn backup_now(app: AppHandle) {
    spawn_backup(&app, Trigger::Manual);
}

#[tauri::command(async)]
fn backup_info(state: State<AppState>) -> Result<BackupInfo, String> {
    let root = with_ws(&state, |ws| Ok(ws.root().to_path_buf()))?;
    Ok(backup::info(&root))
}

/// `git init` and `.gitignore` for an open workspace that is not a repo yet, then a first backup.
#[tauri::command(async)]
fn enable_backup(app: AppHandle, state: State<AppState>) -> Result<(), String> {
    let root = with_ws(&state, |ws| Ok(ws.root().to_path_buf()))?;
    backup::init_backup(&root).map_err(|e| e.to_string())?;
    spawn_backup(&app, Trigger::Event);
    Ok(())
}

// --------------------------------------------------------------- settings

#[tauri::command]
fn get_settings() -> Result<Settings, String> {
    Settings::load(&settings::default_path()).map_err(|e| e.to_string())
}

#[tauri::command]
fn save_settings(settings: Settings) -> Result<(), String> {
    settings.save(&settings::default_path()).map_err(|e| e.to_string())
}

/// Open (and if needed initialise) a workspace, make it active and rebuild the index.
/// A new workspace also becomes a Git repo for backups; when that is refused
/// (OneDrive, iCloud) the workspace still opens and the reason comes back as a warning.
#[tauri::command]
fn open_workspace(app: AppHandle, path: String) -> Result<Option<String>, String> {
    let path = Path::new(&path);
    let mut warning = None;
    if !is_workspace(path) {
        init_workspace(path).map_err(|e| e.to_string())?;
        warning = backup::init_backup(path).err().map(|e| e.to_string());
    }
    load_workspace(&app, path)?;
    *app.state::<AppState>().open_error.lock().unwrap() = None;
    let file = settings::default_path();
    let mut s = Settings::load(&file).map_err(|e| e.to_string())?;
    s.activate(path);
    s.save(&file).map_err(|e| e.to_string())?;
    let _ = app.emit("workspace-changed", ());
    Ok(warning)
}

/// Replace the open workspace and its watcher.
fn load_workspace(app: &AppHandle, path: &Path) -> Result<(), String> {
    let ws = Workspace::open(path).map_err(|e| e.to_string())?;
    let handle = app.clone();
    let watcher = ws
        .watch(move |changes| {
            let state = handle.state::<AppState>();
            if let Some(ws) = state.ws.lock().unwrap().as_mut()
                && !changes.iter().all(|c| c.is_run_log())
            {
                ws.rescan();
            }
            let _ = handle.emit("workspace-changed", ());
        })
        .map_err(|e| e.to_string())?;
    let state = app.state::<AppState>();
    // Old watcher first, so it cannot rescan the new workspace with stale paths.
    *state.watcher.lock().unwrap() = None;
    *state.ws.lock().unwrap() = Some(ws);
    *state.watcher.lock().unwrap() = Some(watcher);
    // App start or switch: catches edits made elsewhere (Obsidian) while it was not open.
    set_backup(app, BackupStatus { state: "off", ..Default::default() });
    spawn_backup(app, Trigger::Event);
    Ok(())
}

/// WORKLY_WORKSPACE (dev, not saved) wins over the active workspace from the settings.
fn startup_workspace() -> Option<String> {
    std::env::var("WORKLY_WORKSPACE").ok().or_else(|| Settings::load(&settings::default_path()).ok()?.active)
}

/// No saved window state yet: ~80 % of the monitor's work area in logical points, at least 1100x700, centred.
fn first_launch_size(win: &WebviewWindow) {
    let Some(m) = win.current_monitor().ok().flatten().or_else(|| win.primary_monitor().ok().flatten()) else { return };
    let area = m.work_area().size.to_logical::<f64>(m.scale_factor());
    let fit = |v: f64, min: f64, max: f64| (v * 0.8).max(min).min(max);
    let _ = win.set_size(LogicalSize::new(fit(area.width, 1100.0, area.width), fit(area.height, 700.0, area.height)));
    let _ = win.center();
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    STARTED.get_or_init(Instant::now);
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        // Size, position and maximised state; never visibility, setup shows the window.
        .plugin(tauri_plugin_window_state::Builder::new().with_state_flags(StateFlags::all() - StateFlags::VISIBLE).build())
        .manage(AppState::default())
        .setup(|app| {
            let win = app.get_webview_window("main").expect("main window");
            let saved = app.path().app_config_dir().is_ok_and(|d| d.join(tauri_plugin_window_state::DEFAULT_FILENAME).exists());
            if !saved {
                first_launch_size(&win);
            }
            win.show()?;
            if let Some(path) = startup_workspace()
                && let Err(e) = load_workspace(app.handle(), Path::new(&path))
            {
                eprintln!("workly: {e}");
                *app.state::<AppState>().open_error.lock().unwrap() = Some(e);
            }
            // ponytail: wakes once a minute and checks whether a backup is due; nothing else runs.
            let handle = app.handle().clone();
            thread::spawn(move || {
                loop {
                    thread::sleep(Duration::from_secs(60));
                    backup_once(&handle, Trigger::Tick);
                }
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_index,
            update_task_field,
            create_task,
            add_waiting_status,
            reorder_tasks,
            set_focus,
            add_task_update,
            set_description,
            toggle_acceptance,
            add_acceptance,
            add_sections,
            move_task,
            delete_task,
            restore,
            trash_items,
            empty_trash,
            app_ready,
            suggest_key,
            create_project,
            update_project_field,
            make_usecase,
            remove_usecase,
            move_usecase,
            reorder_projects,
            delete_project,
            project_files,
            read_markdown,
            usecase_history,
            agent_context,
            run_log,
            agent_feed,
            agent_checks,
            clear_agent,
            set_agents,
            set_task_agent,
            start_agent_task,
            launch_agent,
            ensure_baseline,
            task_changes,
            change_diff,
            commit_task,
            create_agents_md,
            cli_link,
            install_cli,
            open_url,
            open_in_vscode,
            open_project_in_vscode,
            open_repo,
            copy_text,
            reveal,
            get_settings,
            save_settings,
            open_workspace,
            backup_status,
            backup_now,
            backup_info,
            enable_backup,
        ])
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|app, event| {
            // Last backup on quit: a few seconds at most, quitting never waits longer.
            if let RunEvent::Exit = event {
                let (tx, rx) = mpsc::channel();
                let app = app.clone();
                thread::spawn(move || {
                    backup_once(&app, Trigger::Event);
                    let _ = tx.send(());
                });
                let _ = rx.recv_timeout(Duration::from_secs(3));
            }
        });
}
