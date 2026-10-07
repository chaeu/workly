# Workly

Lokale macOS-App für Tasks, Projekte und Use Cases auf Markdown-Dateien, plus CLI `wly` für Coding-Agents.

## Installieren

Voraussetzungen: Xcode Command Line Tools (`xcode-select --install`), Rust (`rustup`), Node 22+, pnpm.

```
git clone <repo-url> ~/Projects/personal/workly
cd ~/Projects/personal/workly
scripts/install.sh
```

Das Skript baut die App (ad-hoc signiert, ca. 2 min), kopiert sie nach `/Applications/Workly.app` und verlinkt `~/.local/bin/wly` auf die App. `~/.local/bin` muss im `PATH` sein; sonst sagt das Skript, was zu tun ist. Aktualisieren: `git pull`, Workly beenden, `scripts/install.sh` erneut.

## Auf einen anderen Mac umziehen

Die App hält keine Daten. Alles Wichtige liegt im Workspace-Ordner; pro Mac gibt es nur Einstellungen.

1. **Workspace verfügbar machen.** Den Workspace-Ordner auf den neuen Mac bringen, wie er ohnehin geteilt wird (Git-Clone, OneDrive, Kopie). `.workly/` muss mitkommen: Konfiguration, Prozess, Agent-Regeln, Log und Papierkorb liegen dort.
2. **App installieren** wie oben (Repo klonen, `scripts/install.sh`).
3. **Workly starten → Choose folder…** und den Workspace-Ordner wählen. Danach in Settings den Repos-Ordner setzen und Theme, Detailkarten und Agent-Schalter nach Geschmack.
4. **Repos klonen.** Projekte verweisen mit `~/…`-Pfaden auf ihre Repos. Was auf diesem Mac fehlt, zeigt Workly unter **Problems**; dort klonen oder im Projekt den Pfad ändern. Die `<key>.code-workspace`-Dateien schreibt „Open in VS Code“ mit den Pfaden dieses Macs neu.

Pro Mac und nicht im Workspace: `~/Library/Application Support/Workly/settings.json` (Workspace-Liste, Repos-Ordner, Theme, UI-Einstellungen, Agent-Schalter) und der `wly`-Link. Beides entsteht neu durch Schritt 2 und 3.

## Für die Entwicklung

| Datei | Inhalt |
| --- | --- |
| `CLAUDE.md` | Regeln und Befehle für Claude Code (Englisch) |
| `docs/SPEC.md` | Spezifikation v1 – Quelle der Wahrheit |
| `docs/CLI.md` | `wly`-Befehle, Exit-Codes, JSON-Felder |
| `docs/PROMPTS.md` | Setup, Prompts M0–M7, Abnahme-Checklisten |
| `docs/design/` | Design System: Tokens, Komponenten, Prinzipien; `icon/` mit App-Icon und Marke |
| `docs/reference/` | Lauffähige HTML-Prototypen (Task-Board, Use-Case-Cockpit) |
| `docs/screenshots/` | Zielbilder hell/dunkel |
| `fixtures/workspace/` | Beispiel-Workspace mit neutralen Daten und Testfällen |
| `scripts/` | `install.sh` (Installation), `perf.sh` (Start- und Scan-Messung mit 500 Tasks) |
