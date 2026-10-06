# Workly

Lokale macOS-App für Tasks, Projekte und Use Cases auf Markdown-Dateien, plus CLI `wly` für Coding-Agents.

Dieses Repo startet als Bauplan; der Code entsteht Meilenstein für Meilenstein mit Claude Code.

| Datei | Inhalt |
| --- | --- |
| `CLAUDE.md` | Regeln für Claude Code (Englisch) |
| `docs/SPEC.md` | Spezifikation v1 – Quelle der Wahrheit |
| `docs/PROMPTS.md` | Setup, Prompts M0–M7, Abnahme-Checklisten |
| `docs/design/` | Design System: Tokens, Komponenten, Prinzipien; `icon/` mit App-Icon und Marke |
| `docs/reference/` | Lauffähige HTML-Prototypen (Task-Board, Use-Case-Cockpit) |
| `docs/screenshots/` | Zielbilder hell/dunkel |
| `fixtures/workspace/` | Beispiel-Workspace mit neutralen Daten und Testfällen |

## Start

```
cd ~/dev/workly
git init && git add . && git commit -m "Docs, design, fixtures"
claude
```

Dann `docs/PROMPTS.md` öffnen und mit M0 beginnen.
