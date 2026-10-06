use serde_json::Value;
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager, State};
use workly_core::Workspace;
use workly_core::scan::Index;
use workly_core::watch::Watcher;

const ACTOR: &str = "app";

#[derive(Default)]
struct AppState {
    ws: Mutex<Option<Workspace>>,
    watcher: Mutex<Option<Watcher>>,
}

/// Run `f` on the open workspace; errors become strings for the frontend.
fn with_ws<T>(state: &AppState, f: impl FnOnce(&mut Workspace) -> workly_core::Result<T>) -> Result<T, String> {
    let mut guard = state.ws.lock().unwrap();
    let ws = guard.as_mut().ok_or("No workspace. Set WORKLY_WORKSPACE (device settings arrive in M2).")?;
    f(ws).map_err(|e| e.to_string())
}

/// Own writes are not reported by the watcher, so announce them here.
fn changed<T>(app: &AppHandle, result: Result<T, String>) -> Result<T, String> {
    if result.is_ok() {
        let _ = app.emit("workspace-changed", ());
    }
    result
}

#[tauri::command]
fn get_index(state: State<AppState>) -> Result<Index, String> {
    with_ws(&state, |ws| Ok(ws.index.clone()))
}

#[tauri::command]
fn update_task_field(app: AppHandle, state: State<AppState>, id: String, field: String, value: Value) -> Result<(), String> {
    changed(&app, with_ws(&state, |ws| ws.update_task_field(&id, &field, &value, ACTOR)))
}

#[tauri::command]
fn create_task(app: AppHandle, state: State<AppState>, title: String, project: Option<String>) -> Result<String, String> {
    changed(&app, with_ws(&state, |ws| ws.create_task(&title, project.as_deref(), ACTOR)))
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

/// Open the workspace from WORKLY_WORKSPACE and keep the index live.
// ponytail: env var only; device settings with a workspace picker come in M2.
fn open_workspace(app: &AppHandle) -> Result<(), String> {
    let path = std::env::var("WORKLY_WORKSPACE").map_err(|_| "WORKLY_WORKSPACE not set".to_string())?;
    let ws = Workspace::open(&path).map_err(|e| e.to_string())?;
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
    *state.ws.lock().unwrap() = Some(ws);
    *state.watcher.lock().unwrap() = Some(watcher);
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(AppState::default())
        .setup(|app| {
            if let Err(e) = open_workspace(app.handle()) {
                eprintln!("workly: {e}");
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![get_index, update_task_field, create_task, move_task, delete_task, restore])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
