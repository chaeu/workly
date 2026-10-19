//! Device settings: `~/Library/Application Support/Workly/settings.json`.
//! Paths per Mac, never inside a workspace.

use crate::write::{OwnWrites, atomic_write};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct Settings {
    pub workspaces: Vec<WorkspaceRef>,
    /// Path of the active workspace, one of `workspaces`.
    pub active: Option<String>,
    pub repos_dir: Option<String>,
    /// system | light | dark
    pub theme: Option<String>,
    /// macOS app names for the file buttons, in button order. "Obsidian" opens via obsidian://.
    pub open_with: Vec<String>,
    /// Keys from newer app versions survive a save.
    #[serde(flatten)]
    pub extra: BTreeMap<String, Value>,
}

// By hand: `#[serde(default)]` fills a missing `open_with` from here too.
impl Default for Settings {
    fn default() -> Self {
        Settings {
            workspaces: Vec::new(),
            active: None,
            repos_dir: None,
            theme: None,
            open_with: vec!["Obsidian".into(), "Visual Studio Code".into()],
            extra: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct WorkspaceRef {
    pub name: String,
    pub path: String,
    #[serde(default)]
    pub backup: BackupSettings,
}

/// Per workspace and Mac: how often Workly commits the workspace repo.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default)]
pub struct BackupSettings {
    /// off | 15m | hourly | daily; anything else counts as hourly.
    pub interval: String,
    /// Push to the current branch's upstream after a commit.
    pub push: bool,
}

impl Default for BackupSettings {
    fn default() -> Self {
        BackupSettings { interval: "hourly".into(), push: true }
    }
}

impl BackupSettings {
    /// None = off.
    pub fn every(&self) -> Option<Duration> {
        match self.interval.as_str() {
            "off" => None,
            "15m" => Some(Duration::from_secs(15 * 60)),
            "daily" => Some(Duration::from_secs(24 * 3600)),
            _ => Some(Duration::from_secs(3600)),
        }
    }
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
            self.workspaces.push(WorkspaceRef { name, path: p.clone(), backup: BackupSettings::default() });
        }
        self.active = Some(p);
    }

    /// Backup settings of the workspace at `root`; defaults for one not in the list (WORKLY_WORKSPACE).
    pub fn backup_for(&self, root: &Path) -> BackupSettings {
        let same = |p: &str| Path::new(p).canonicalize().is_ok_and(|p| p == root) || Path::new(p) == root;
        self.workspaces.iter().find(|w| same(&w.path)).map(|w| w.backup.clone()).unwrap_or_default()
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
        assert_eq!(back.workspaces, [WorkspaceRef { name: "My Work".into(), path: "/tmp/My Work".into(), backup: BackupSettings::default() }]);
        assert_eq!(back.active.as_deref(), Some("/tmp/My Work"));
        assert_eq!(back.theme.as_deref(), Some("dark"));
        assert_eq!(back.extra["focus_hidden"], Value::Bool(true));
        fs::write(&file, "{broken").unwrap();
        assert!(Settings::load(&file).is_err());
    }

    #[test]
    fn open_with_default_and_custom() {
        let old: Settings = serde_json::from_str(r#"{"theme":"dark"}"#).unwrap();
        assert_eq!(old.open_with, ["Obsidian", "Visual Studio Code"]);
        let custom: Settings = serde_json::from_str(r#"{"open_with":["Zed","Obsidian"]}"#).unwrap();
        assert_eq!(custom.open_with, ["Zed", "Obsidian"]);
        let back: Settings = serde_json::from_str(&serde_json::to_string(&custom).unwrap()).unwrap();
        assert_eq!(back.open_with, ["Zed", "Obsidian"]);
        let none: Settings = serde_json::from_str(r#"{"open_with":[]}"#).unwrap();
        assert!(none.open_with.is_empty());
    }

    #[test]
    fn backup_settings_default_and_per_workspace() {
        let s: Settings = serde_json::from_str(
            r#"{"workspaces":[{"name":"a","path":"/a"},{"name":"b","path":"/b","backup":{"interval":"off"}},{"name":"c","path":"/c","backup":{"interval":"15m","push":false}}]}"#,
        )
        .unwrap();
        let a = s.backup_for(Path::new("/a"));
        assert_eq!((a.every(), a.push), (Some(Duration::from_secs(3600)), true));
        assert_eq!(s.backup_for(Path::new("/b")).every(), None);
        assert!(s.backup_for(Path::new("/b")).push);
        let c = s.backup_for(Path::new("/c"));
        assert_eq!((c.every(), c.push), (Some(Duration::from_secs(900)), false));
        assert_eq!(s.backup_for(Path::new("/elsewhere")), BackupSettings::default());
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
