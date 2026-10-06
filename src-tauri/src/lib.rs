use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::process::Command;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use workly_core::model::{Config, Process};
use workly_core::project::{NewProject, init_workspace, is_workspace};
use workly_core::scan::Index;
use workly_core::settings::{self, Settings, expand_home};
use workly_core::watch::Watcher;
use workly_core::Workspace;

const ACTOR: &str = "app";

#[derive(Default)]
struct AppState {
    ws: Mutex<Option<Workspace>>,
    watcher: Mutex<Option<Watcher>>,
}

/// Run `f` on the open workspace; errors become strings for the frontend.
fn with_ws<T>(state: &AppState, f: impl FnOnce(&mut Workspace) -> workly_core::Result<T>) -> Result<T, String> {
    let mut guard = state.ws.lock().unwrap();
    let ws = guard.as_mut().ok_or("No workspace open.")?;
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
    #[serde(flatten)]
    index: Index,
}

#[tauri::command]
fn get_index(state: State<AppState>) -> Result<Snapshot, String> {
    with_ws(&state, |ws| {
        let root = ws.root().display().to_string();
        Ok(Snapshot { root, config: ws.config.clone(), process: ws.process.clone(), index: ws.index.clone() })
    })
}

// ------------------------------------------------------------------ tasks

#[tauri::command]
fn update_task_field(app: AppHandle, state: State<AppState>, id: String, field: String, value: Value) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.update_task_field(&id, &field, &value, ACTOR)))
}

#[tauri::command]
fn create_task(app: AppHandle, state: State<AppState>, title: String, project: Option<String>, priority: Option<u8>) -> Result<String, String> {
    changed(&app, with_ws(&state, |ws| ws.create_task(&title, project.as_deref(), priority, ACTOR)))
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

// --------------------------------------------------------------- projects

#[tauri::command]
fn suggest_key(state: State<AppState>, title: String) -> Result<String, String> {
    with_ws(&state, |ws| Ok(ws.suggest_key(&title)))
}

#[tauri::command]
fn create_project(app: AppHandle, state: State<AppState>, project: NewProject) -> Result<String, String> {
    changed(&app, with_ws(&state, |ws| ws.create_project(&project, ACTOR)))
}

#[tauri::command]
fn update_project_field(app: AppHandle, state: State<AppState>, key: String, field: String, value: Value) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.update_project_field(&key, &field, &value, ACTOR)))
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

/// Workspace-relative path in VS Code.
#[tauri::command]
fn open_in_vscode(state: State<AppState>, path: String) -> Result<(), String> {
    let abs = with_ws(&state, |ws| ws.resolve(&path))?;
    open(&["-b", VSCODE, &abs.to_string_lossy()])
}

/// The project's `<key>.code-workspace` in VS Code (written first if missing).
#[tauri::command]
fn open_project_in_vscode(state: State<AppState>, key: String) -> Result<(), String> {
    let file = with_ws(&state, |ws| ws.code_workspace(&key, false))?;
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

/// Workspace-relative path selected in Finder.
#[tauri::command]
fn reveal(state: State<AppState>, path: String) -> Result<(), String> {
    let abs = with_ws(&state, |ws| ws.resolve(&path))?;
    open(&["-R", &abs.to_string_lossy()])
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
#[tauri::command]
fn open_workspace(app: AppHandle, path: String) -> Result<(), String> {
    let path = Path::new(&path);
    if !is_workspace(path) {
        init_workspace(path).map_err(|e| e.to_string())?;
    }
    load_workspace(&app, path)?;
    let file = settings::default_path();
    let mut s = Settings::load(&file).map_err(|e| e.to_string())?;
    s.activate(path);
    s.save(&file).map_err(|e| e.to_string())?;
    let _ = app.emit("workspace-changed", ());
    Ok(())
}

/// Replace the open workspace and its watcher.
fn load_workspace(app: &AppHandle, path: &Path) -> Result<(), String> {
    let ws = Workspace::open(path).map_err(|e| e.to_string())?;
    let handle = app.clone();
    let watcher = ws
        .watch(move |_changes| {
            let state = handle.state::<AppState>();
            if let Some(ws) = state.ws.lock().unwrap().as_mut() {
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
    Ok(())
}

/// WORKLY_WORKSPACE (dev, not saved) wins over the active workspace from the settings.
fn startup_workspace() -> Option<String> {
    std::env::var("WORKLY_WORKSPACE").ok().or_else(|| Settings::load(&settings::default_path()).ok()?.active)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default())
        .setup(|app| {
            if let Some(path) = startup_workspace()
                && let Err(e) = load_workspace(app.handle(), Path::new(&path))
            {
                eprintln!("workly: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_index,
            update_task_field,
            create_task,
            reorder_tasks,
            set_focus,
            add_task_update,
            move_task,
            delete_task,
            restore,
            suggest_key,
            create_project,
            update_project_field,
            reorder_projects,
            delete_project,
            project_files,
            read_markdown,
            open_url,
            open_in_vscode,
            open_project_in_vscode,
            open_repo,
            reveal,
            get_settings,
            save_settings,
            open_workspace,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
