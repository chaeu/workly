//! Device settings: `~/Library/Application Support/Workly/settings.json`.
//! Paths per Mac, never inside a workspace.

use crate::write::{OwnWrites, atomic_write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub workspaces: Vec<WorkspaceRef>,
    /// Path of the active workspace, one of `workspaces`.
    pub active: Option<String>,
    pub repos_dir: Option<String>,
    /// system | light | dark
    pub theme: Option<String>,
    /// Keys from newer app versions survive a save.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkspaceRef {
    pub name: String,
    pub path: String,
}

pub fn default_path() -> PathBuf {
    home().join("Library/Application Support/Workly/settings.json")
}

pub fn home() -> PathBuf {
    std::env::var_os("HOME").map(PathBuf::from).unwrap_or_default()
}

/// `~/x` -> `/Users/me/x`; other paths unchanged.
pub fn expand_home(path: &str) -> PathBuf {
    match path.strip_prefix("~/") {
        Some(rest) => home().join(rest),
        None if path == "~" => home(),
        None => PathBuf::from(path),
    }
}

/// `/Users/me/x` -> `~/x`, so paths in shared files work on every Mac.
pub fn tilde(path: &Path) -> String {
    match path.strip_prefix(home()) {
        Ok(rest) if !home().as_os_str().is_empty() => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

impl Settings {
    /// Missing file = defaults. A broken file is an error, so it is never overwritten silently.
    pub fn load(path: &Path) -> io::Result<Settings> {
        match fs::read_to_string(path) {
            Ok(src) => serde_json::from_str(&src).map_err(|e| io::Error::other(format!("{}: {e}", path.display()))),
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Settings::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        atomic_write(path, &(serde_json::to_string_pretty(self)? + "\n"), &OwnWrites::default())
    }

    /// Add the workspace (name = folder name) if new and make it active.
    pub fn activate(&mut self, path: &Path) {
        let p = path.display().to_string();
        if !self.workspaces.iter().any(|w| w.path == p) {
            let name = path.file_name().map_or(p.clone(), |n| n.to_string_lossy().into_owned());
            self.workspaces.push(WorkspaceRef { name, path: p.clone() });
        }
        self.active = Some(p);
    }
}

/// Where Settings → Install CLI puts the `wly` link.
pub fn cli_link_path() -> PathBuf {
    home().join(".local/bin/wly")
}

/// Point `link` at `exe`. Replaces an older link, never a real file.
pub fn link_cli(exe: &Path, link: &Path) -> io::Result<()> {
    if let Ok(meta) = fs::symlink_metadata(link) {
        if !meta.file_type().is_symlink() {
            return Err(io::Error::other(format!("{} exists and is not a link. Remove it first.", tilde(link))));
        }
        fs::remove_file(link)?;
    }
    if let Some(dir) = link.parent() {
        fs::create_dir_all(dir)?;
    }
    std::os::unix::fs::symlink(exe, link)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_keeps_unknown_keys() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a/settings.json");
        assert_eq!(Settings::load(&file).unwrap(), Settings::default());
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, r#"{"theme":"dark","focus_hidden":true}"#).unwrap();
        let mut s = Settings::load(&file).unwrap();
        s.activate(Path::new("/tmp/My Work"));
        s.activate(Path::new("/tmp/My Work"));
        s.save(&file).unwrap();
        let back = Settings::load(&file).unwrap();
        assert_eq!(back.workspaces, [WorkspaceRef { name: "My Work".into(), path: "/tmp/My Work".into() }]);
        assert_eq!(back.active.as_deref(), Some("/tmp/My Work"));
        assert_eq!(back.theme.as_deref(), Some("dark"));
        assert_eq!(back.extra["focus_hidden"], Value::Bool(true));
        fs::write(&file, "{broken").unwrap();
        assert!(Settings::load(&file).is_err());
    }

    #[test]
    fn home_paths() {
        let h = home();
        assert_eq!(expand_home("~/repos/a"), h.join("repos/a"));
        assert_eq!(expand_home("/abs"), PathBuf::from("/abs"));
        assert_eq!(tilde(&h.join("repos/a")), "~/repos/a");
        assert_eq!(tilde(Path::new("/opt/x")), "/opt/x");
    }

    #[test]
    fn cli_link_replaces_links_only() {
        let dir = tempfile::tempdir().unwrap();
        let link = dir.path().join("bin/wly");
        link_cli(Path::new("/a/workly-app"), &link).unwrap();
        link_cli(Path::new("/b/workly-app"), &link).unwrap();
        assert_eq!(fs::read_link(&link).unwrap(), Path::new("/b/workly-app"));
        fs::remove_file(&link).unwrap();
        fs::write(&link, "mine").unwrap();
        assert!(link_cli(Path::new("/b/workly-app"), &link).is_err());
        assert_eq!(fs::read_to_string(&link).unwrap(), "mine");
    }
}
