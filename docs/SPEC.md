# Workly – Spezifikation v1

Stand: 6. Oktober 2026. Quelle: Dokument „Workly – Spezifikation v1“ (claude.ai). Bei Widerspruch gilt diese Datei im Repo.

## 1. Entscheidungen

Workly ist eine eigenständige, leichte macOS-App. Dateien sind die Quelle der Wahrheit, die App ist ein Werkzeug darauf.

| Thema | Entscheidung | Warum |
| --- | --- | --- |
| Stack | Tauri 2, SvelteKit (adapter-static, SPA, Svelte 5), Rust-Core | Native Hülle mit System-WebView statt gebündeltem Chromium; SvelteKit ist der stärkste Stack des Entwicklers |
| Daten | Markdown mit YAML-Frontmatter, keine Datenbank | Obsidian, VS Code, Git und Agents lesen dieselben Dateien |
| Projekt | Jeder Ordner mit einer `_project.md` ist ein Projekt | Ordner liegen, wo der Nutzer will; die App findet sie |
| Use Case | Ein Projekt mit einem `usecase:`-Block im Frontmatter. Beide Richtungen in der App: „Make use case“ im Projekt-Formular hängt den Block ans Ende des Frontmatters (erster Schritt der ersten Phase); „Remove from cockpit…“ im selben Formular (seltene Aktionen leben im Edit-Formular, nicht auf der Seite) löscht ihn samt `decisions` nach Bestätigung, die Log-Zeile hält den alten Block in `from`. Projekt und Tasks bleiben | Keine zweite Entität; aus einer Idee wird ohne Umzug ein Projekt und zurück |
| Tasks | Liegen im Projektordner unter `tasks/`, Tasks ohne Projekt in `inbox/` | Alles zu einem Projekt an einem Ort, auch für Agents |
| IDs | Projekt-Key + Nummer (`WR-12`); Key wird aus dem Namen vorgeschlagen, beim Anlegen änderbar, danach fest; Inbox-Tasks nutzen den reservierten Key `IN` | Lesbar in Commits und Branches |
| Einstellungen | Pfade pro Gerät in der App, Regeln pro Workspace in `.workly/config.yml` | Standalone-fähig; derselbe Workspace funktioniert auf mehreren Macs |
| Agents | CLI `wly` als einzige Schreibschnittstelle für Agents; Ausführung durch Runner-Adapter (Codex CLI, Copilot in VS Code), erst nach dem Probebetrieb | Workly koordiniert und protokolliert, Agents arbeiten in ihren eigenen Werkzeugen |
| Agent-Funktionen | Abschaltbar per Geräte-Einstellung `agents_enabled` (Standard: aus). Aus = Basisversion ohne Agent-Oberfläche; Felder in den Dateien und die CLI bleiben | Workly bleibt ohne Agents ein schlankes PM-Werkzeug; ob „Start agent“ gebaut wird, entscheidet der Probebetrieb |
| Sprache | UI-Labels Englisch, Deutsch später als zweite Sprache; Datei-IDs immer Englisch | |
| Name | Workly (Bundle-ID z. B. `dev.chaeu.workly`, CLI `wly`, Ordner `.workly/`) | |
| Design | Workly Design System (`docs/design/`) | Steht bereits, inklusive Prototypen |

**Leitplanken:** Keine Firmendaten im Repo, Entwicklung mit dem Beispiel-Workspace unter `fixtures/workspace`. Kein eigener Editor und keine eigene Agent-Umgebung: Bearbeiten in Obsidian oder VS Code, Agents in ihren eigenen CLIs.

## 2. Architektur

Ein Rust-Core (`workly-core`) besitzt alle Regeln für den Zugriff auf die Dateien. App, CLI und später der Runner sind nur verschiedene Türen dazu. So kann ein Agent nichts anderes schreiben als die Oberfläche.

```
 Workly App (SvelteKit in Tauri 2)     wly CLI  <── Agents (Codex, Claude Code)
              │ Tauri commands            │                    ▲
              ▼                           ▼                    │ starts (v1.5)
 ┌──────────────────── workly-core (Rust) ─────────┐ ──>  Runner (v1.5)
 │ scan · in-memory index · file watcher · IDs     │          │ works in
 │ lossless write · process · log · trash          │          ▼
 └─────────────────────────────────────────────────┘     Repos (git worktree per task)
              │ read, write, watch
              ▼
 Workspace (Markdown files)  <── same files ──>  Obsidian · VS Code · Git
```

Keine Datenbank, kein Server. Beim Start baut der Core einen Index im Speicher und hält ihn per Datei-Watcher aktuell; externe Änderungen erscheinen ohne Neustart. Ein Cache kommt erst, wenn Messungen ihn rechtfertigen, und wäre jederzeit neu aufbaubar.

**Cargo-Workspace:**

| Crate | Zweck |
| --- | --- |
| `crates/workly-core` | Domänenmodell, Scan, Parser, verlustfreies Schreiben, Index, Watcher, IDs, Prozess, Log, Papierkorb |
| `src-tauri` (`workly-app`) | Tauri-Befehle und Events, dünn über dem Core |
| `crates/wly` | CLI über dem Core, JSON-Ausgabe für Agents |

## 3. Workspace und Dateiformat

Ein Workspace ist ein Ordner, zugleich Obsidian-Vault und Git-Repo. Menschenlesbares liegt sichtbar, Maschinendaten im versteckten `.workly/`.

### Einstellungen

| Ebene | Ort | Inhalt |
| --- | --- | --- |
| Gerät | `~/Library/Application Support/Workly/settings.json` | Workspaces (Name + Pfad, mehrere, umschaltbar), Repos-Ordner, Theme, Fenster, UI-Präferenzen, Agent-Funktionen an/aus, Stunden pro FTE und Jahr |
| Workspace | `<workspace>/.workly/config.yml` | Ordner für neue Projekte und Inbox, Scan-Ausschlüsse, Task-Status, WIP-Limits, Agent-Standards |
| Workspace | `<workspace>/.workly/process.yml` | Use-Case-Prozess (Phasen, Schritte, Gates, Kanten) und Bewertungskriterien (`assessment:` mit K.-o.-Fragen und Kriterien je Achse samt Ankern) |

### Ordnerstruktur

```
<workspace>/
  _templates/                  # project, task, note, decision
  inbox/                       # tasks without a project (IN-*)
  knowledge/                   # general docs, not a project
  projects/
    website-relaunch/
      _project.md              # metadata + overview
      tasks/WR-12-navigation.md
      docs/
      notes/2026-10-06-kickoff.md
      decisions/0001-hosting.md
      agent/AGENTS.md
      agent/skills/<name>/SKILL.md
  .workly/
    config.yml
    process.yml
    agent/AGENTS.md            # global agent rules
    log/2026-10.jsonl          # every change, append-only
    runs/<run-id>/             # agent runs (v1.5)
    trash/                     # deleted items, restorable
```

Die App scannt alle Ordner unterhalb des Workspace (ohne `.git`, `node_modules`, `.obsidian`, `.workly`, `_templates`) nach `_project.md`.

### Projekt: `_project.md`

```yaml
---
key: WR                       # 2-5 uppercase letters, unique, fixed after creation
title: Website Relaunch
status: active                # active | paused | archived
color: proj-1                 # design token proj-1 … proj-6
order: 1                      # manual sort
repos: [~/repos/website]
links: []
created: 2026-10-06
usecase:                      # optional, only for use cases
  type: ai                    # ai | hybrid | rule
  area: Finance               # free text; suggestions = process.yml areas ∪ areas in use
  step: pilot                 # step id from process.yml
  step_since: 2026-10-01
  status: active              # active | waiting | blocked | on_hold | stable
  blocked_by: null
  next_step: Review pilot results
  current_state: Pilot with 20 invoices running
  decisions:                  # gate decisions, newest last
    - { date: 2026-09-10, gate: G1, text: "Yes, pursue" }
  savings:                    # optional, activities the use case takes over
    - { what: Capture invoice, count: 1200, per: month, minutes: 6 }   # per: year | month | week | day
  savings_note: "Volume from the department, sample 09/2026"
  assessment:                 # optional, current assessment (criteria from process.yml)
    date: 2026-10-08
    ko: { owner: pass, risk: open, data_use: pass }      # pass | fail | open
    scores: { volume: 3, quality: 2, data: 2, path: 3 }  # 1 | 2 | 3, missing = open
    note: Risk depends on whether output reaches customers
---
Short description in Markdown, shown as overview in the app.
```

### Task: `tasks/WR-12-<slug>.md`

```yaml
---
id: WR-12
title: Navigation überarbeiten
status: todo                  # backlog | todo | doing | review | done
priority: 2                   # 1 | 2 | 3
due: 2026-10-10
tags: [frontend]
focus: 2026-10-06             # on the focus strip when equal to today
focus_order: 1                # 1-3
order: 2                      # optional, manual position in its board column
created: 2026-10-06
done_at: null
agent:                        # optional
  ready: false
  runner: auto                # auto | codex | copilot | claude-code
  model: auto
  effort: auto                # auto | low | medium | high
  active: null                # set while an agent works, e.g. codex
  since: null                 # ISO timestamp, drives "Codex working · 14 min"
  commit: null
  last_run: null
---
Description and acceptance criteria.

## Updates
- 2026-10-06 09:12 · codex: Pipeline green on dev.
```

### Regeln

- **IDs:** Nächste Nummer = höchste vorhandene Nummer dieses Keys im Workspace + 1 (inklusive Papierkorb und aller IDs im Log, damit auch ein geleerter Papierkorb keine Nummer freigibt), kein gespeicherter Zähler. Eine ID bleibt beim Verschieben in ein anderes Projekt unverändert.
- **Keys:** Vorschlag aus dem Titel: ist das erste Wort ein Kürzel in Großbuchstaben, wird es übernommen; sonst Anfangsbuchstaben der ersten bis zu drei Wörter; bei einem Wort die ersten drei Buchstaben. Bei Kollision Ziffer anhängen. `IN` ist reserviert.
- **Verlustfreies Schreiben:** Nur betroffene Felder ändern. Unbekannte Felder, Reihenfolge, Kommentare, Leerzeilen und Body bleiben byte-gleich. Wichtigster Test im Projekt.
- **Updates** werden als Zeile unter `## Updates` im Body angehängt (Abschnitt wird bei Bedarf angelegt). Format `- JJJJ-MM-TT HH:MM · <wer>: <Text>`; `wer` = Agent-Name, `me` für die App.
- **Board-Reihenfolge:** In einer Spalte sortiert nach `order`, dann Priorität, dann ID; Tasks ohne `order` stehen hinten. Ziehen in eine andere Spalte ändert nur `status`; erst Umsortieren innerhalb einer Spalte schreibt `order` 1..n für die Tasks dieser Spalte (bzw. Zelle in der Projekt-Gruppierung).
- **`done_at`** folgt dem Status: wird beim Wechsel auf `done` auf das heutige Datum gesetzt, beim Verlassen von `done` auf `null`.
- **Fokus:** höchstens drei Tasks. Hinzufügen setzt `focus: <heute>` und `focus_order`; Entfernen löscht beide Zeilen. Werte von anderen Tagen bleiben liegen und werden ignoriert.
- **Projekt-Board** (Tab Tasks): ohne Projektfilter, Gruppierung und Fokus-Streifen; Spalten zeigen die Anzahl im Projekt ohne WIP-Limit (das gilt für den ganzen Workspace). An der Stelle des Fokus-Streifens steht eine gleich hohe Leiste mit den Tabs. Fokus wird auf dem Tasks-Board oder in der Detailkarte gesetzt.
- **Projekt-Tabs:** Overview · Use case · Tasks · Files in derselben Leiste; `/projects/<KEY>` ohne Parameter ist Tasks, `?tab=overview`, `?tab=usecase` bzw. `?tab=files` die anderen. Use case gibt es nur, solange das Projekt einen `usecase:`-Block hat; sonst (auch direkt nach „Remove from cockpit“) zeigt die Seite Overview. Die Projektliste öffnet Overview.
- **Projektwechsel in der Sidebar** öffnet den zuletzt benutzten Tab (Overview, Use case, Tasks oder Files, Standard Tasks; Use case bei einem Projekt ohne Use Case wird Overview); vom Tasks-Board aus immer Tasks. Gemerkt wird nur im Speicher.
- **Tasks-Board:** Tasks archivierter Projekte erscheinen nicht. In der Gruppierung nach Projekt hat die Inbox eine eigene Bahn; Ziehen in eine andere Bahn verschiebt die Datei (ID bleibt). Filter „Diese Woche“ = fällig bis einschließlich Sonntag, „Überfällig“ = fällig vor heute und nicht `done`.
- **Löschen** verschiebt nach `.workly/trash/` (Pfad erhalten), wiederherstellbar. Ein gelöschtes Projekt ist ein Eintrag; seine vorher einzeln gelöschten Tasks bleiben beim Wiederherstellen im Papierkorb. **Papierkorb leeren** verschiebt alles in den macOS-Papierkorb (dort noch wiederherstellbar) und schreibt jede ID ins Log.
- **Log:** Jede Änderung als JSON-Zeile in `.workly/log/JJJJ-MM.jsonl`: `{ts, actor, kind, id, field, from, to}`. `actor` = `app`, `cli`, `agent:<name>`.
- **Use-Case-Verlauf:** Der Core liest das Log (nur lesen) und baut daraus pro Phase Zeitraum, Status-Episoden (mit dem letzten „Blocked by“) und die Entscheidungen aus `usecase.decisions` (Gate-Entscheidungen in der Phase des Gates, sonst nach Datum). `usecase.remove` setzt den Verlauf zurück. Der Verlauf kennt nur Änderungen über Workly; weicht die Datei vom Log ab, gilt die Datei (`step`, `step_since`).
- **Ersparnis (`usecase.savings`):** Stunden und FTE werden berechnet, nie gespeichert. Stunden/Jahr = Σ `count` × Faktor(`per`) × `minutes` / 60 mit Faktor year 1, month 12, week 46, day 220 (Arbeitswochen/-tage, Konstanten in der App); FTE = Stunden / `fte_hours_per_year` (Geräte-Einstellung, Standard 1720). Die App schreibt immer die ganze Liste und prüft dabei: `what` nicht leer, `count` und `minutes` Zahlen ≥ 0, `per` aus der Liste. Ungültige Einträge von Hand stehen unter Problems, der Use Case lädt trotzdem. Anzeige: auf der Use-Case-Seite eine Karte unter der Beschreibung (Summe, eine Zeile pro Tätigkeit, Notiz), bearbeitet im Popup „Edit…“ (Tabelle, Notiz, Umrechnung); Detailkarte (Popup, Seitenleiste) zeigt in der Seitenspalte nur die Summe „≈ FTE · h/yr“, der Tooltip listet jede Tätigkeit und die Umrechnung (ohne Einträge: „No effort recorded“ mit Link zur Seite); sortierbare Spalte Effort (FTE-Wert) in der Liste (leer = hinten), Pill auf der Board-Karte bei > 0. Keine Summe im Cockpit, kein Automatisierungsgrad, keine Geldbeträge. UI-Label *Manual effort*, der Feldname bleibt `savings`.
- **Bewertung (`usecase.assessment`):** Vor G0 „Pilot?“ und nach dem Pilot an G1 erneut. Die Methode steht in `process.yml` unter `assessment:` (`ko:` mit `id`, `label`; `criteria:` mit `id`, `axis` value | feasibility, `label`, genau 3 `anchors` für 1, 2, 3); eingebaut ist nichts, fehlt der Block, blendet die App jede Bewertung aus. Pro Use Case gibt es eine aktuelle Bewertung; eine Neubewertung überschreibt sie, der alte Stand steht im Log (`from`). K.-o.-Werte sind `pass` / `fail` / `open` (nicht yes/no, YAML 1.1), Punkte 1–3; ein fehlender Schlüssel heißt offen. Value, Feasibility (Durchschnitt der bewerteten Kriterien je Achse, ohne Gewichte), Quadrant (Grenze 2,0: Quick win, Big bet, Fill-in, Drop; ein `fail` = K.O.) und offene Punkte werden berechnet, nie gespeichert. Die App schreibt immer den ganzen Block (ein Write, eine Logzeile, `ko` und `scores` als Flow-Maps, `null` entfernt ihn) und prüft dabei Werte, Datum und dass jede ID in `process.yml` steht. Ungültige Werte von Hand stehen unter Problems und gelten als offen, der Use Case lädt trotzdem. **Anzeige:** bewertet wird im Popup „Assess…“ auf dem Tab Use case (K.-o.-Fragen pass / fail / open, je Kriterium 1 · 2 · 3 · ? mit den drei Ankern sichtbar, Notiz, Ergebnis live im Fuß samt Rechenweg je Achse, z. B. „Value (3 + 2 + 2) / 3 = 2.3“); „Save“ schreibt den Block mit heutigem Datum, „Clear…“ nach Rückfrage `null`. Die Pill zeigt den Quadranten (Quick win `--w-ok`, Big bet `--w-info`, Fill-in `--w-warn`, Drop `--w-hold`), K.O. (`--w-danger`) sticht den Quadranten, „Incomplete“, solange eine Achse keine Punkte hat. Ein offenes K.-o. zeigt weiter den Quadranten und steht unter den offenen Punkten.
- **Dateinamen:** `<ID>-<slug>.md`; Titeländerungen benennen die Datei nicht um (stabil für Links).
- **Projekt-Keys:** 2–6 Zeichen, Großbuchstaben, Ziffern erst nach dem ersten Buchstaben (Kollisionsziffer, z. B. `WR2`).
- **Neue Projekte** liegen in `<new_projects_dir>/<slug>/`. Existiert der Ordner schon ohne `_project.md`, bietet das Formular „Use existing folder“ an: die App schreibt `_project.md` und legt nur Fehlendes an (Unterordner, `agent/AGENTS.md`, `<key>.code-workspace`); vorhandene Dateien werden nie überschrieben oder verschoben, Titel und Key kommen aus dem Formular. Hat der Ordner schon eine `_project.md`, ist er bereits ein Projekt und das Anlegen schlägt fehl. Projekt löschen verschiebt den ganzen Ordner in den Papierkorb.
- **`<key>.code-workspace`** ist generiert: beim Anlegen, bei jeder Änderung von `repos` und bei „Open in VS Code“ neu geschrieben (absolute Pfade, pro Mac). Repos stehen im Frontmatter als `~/…`.
- **Workspace anlegen:** Ein Ordner ohne `.workly/config.yml` bekommt die fehlenden Teile aus den eingebauten Vorlagen; vorhandene Dateien werden nie überschrieben.

## 4. Dokumentation

Jedes Projekt hat genau ein Zuhause: seinen Projektordner. Das Repo enthält nur, was mit dem Code versioniert werden muss.

| Art | Ort |
| --- | --- |
| Überblick, Ziel, Stakeholder | `_project.md` (Body) |
| Lebende Doku | `docs/` |
| Datierte Notizen | `notes/JJJJ-MM-TT-thema.md` |
| Entscheidungen | `decisions/0001-thema.md` |
| Agent-Anweisungen | `agent/AGENTS.md`, `agent/skills/` |
| Code-nahe Doku | im Repo |
| Allgemeines Wissen | `knowledge/` |

- **Obsidian** liest und schreibt Doku; `.workly/` bleibt dort unsichtbar.
- **VS Code:** Workly erzeugt pro Projekt `<projekt>/<key>.code-workspace` mit Projektordner und Repos; „Open in VS Code“ öffnet diese Datei.
- **Workly** zeigt Struktur und Status, Markdown-Vorschau, öffnet Dateien in Obsidian (`obsidian://open?path=…`), VS Code oder Finder. Kein Markdown-Editor; die Task-Beschreibung ist ein einfaches Textfeld.
- **Vorlagen** in `_templates/` werden von App und Obsidian genutzt.
- **Git** macht der Nutzer; Workly committet nicht.

## 5. Agentic Workflow

### CLI `wly`

Läuft auf demselben Core wie die App. Agents (und der Nutzer) lesen und ändern Tasks darüber.

```
wly task list [--project WR] [--status todo] [--json]
wly task show WR-12 [--json]          # task + project + agent files as context
wly task start WR-12 --agent codex    # status doing, agent.active + agent.since set
wly task note WR-12 "text"            # appends to ## Updates
wly task review WR-12 [--commit a3f9c1]   # status review, agent.active cleared; never done
wly task add "title" [--project WR | --inbox] [--priority 2]
wly project list [--json]
```

Exit-Codes: 0 ok, 1 Nutzungsfehler, 2 nicht gefunden, 3 ungültiger Übergang. `--workspace <path>` oder `WORKLY_WORKSPACE` wählt den Workspace, sonst der in den Geräte-Einstellungen aktive.

### Agent-Dateien

- `<projekt>/agent/AGENTS.md`: Projekt-Anweisungen.
- `<projekt>/agent/skills/<name>/SKILL.md`: wiederkehrende Abläufe.
- `<workspace>/.workly/agent/AGENTS.md`: globale Regeln (Status nur über `wly`, nie `done`, nichts löschen).

Beim Start setzt Workly den Kontext zusammen: globale Regeln, Projekt-Anweisungen, Skills, Task.

### Runner-Adapter

Jeder Agent ist ein Adapter: Kontext zusammensetzen, starten, Status über `wly`. v1:

- **Codex CLI:** öffnet Terminal im Repo und startet Codex mit dem zusammengesetzten Prompt.
- **Copilot in VS Code:** öffnet die `.code-workspace`-Datei und legt den Prompt in die Zwischenablage.

Claude Code folgt als weiterer Adapter.

### Schalter Agent-Funktionen

`agents_enabled` in den Geräte-Einstellungen (Standard: aus). Aus blendet jede Agent-Oberfläche aus: Ansicht und Menüpunkt „Agents“, Agent-Bereich der Task-Detailkarte, Agent-Badge, -Ring und Commit auf Karten, Agent-Dateien in der Projektansicht, „Install CLI“ und später „Start agent“. Nichts wird gelöscht: `agent:`-Felder bleiben in den Dateien, `wly` funktioniert weiter. Jede neue Agent-Oberfläche hängt an diesem einen Schalter.

### Stufen

| Stufe | Inhalt |
| --- | --- |
| v1 | Agent-Dateien, CLI, Schalter, Live-Status auf der Karte; Button „Start agent“ mit zwei Adaptern nur, wenn der Probebetrieb dafür spricht (M7) |
| v1.5 | Queue: Läufe ohne Fenster nacheinander, Git-Worktree je Task auf Branch `wly/WR-12`, Agent-Ansicht (Queue, Running, Review, Failed) mit Live-Log |
| v2 | Parallele Läufe, automatische Modellwahl, Budgets, Folge-Tasks, MCP-Server |

**Grenzen:** Agents setzen nie `done`, löschen nichts, arbeiten nur im Repo bzw. Worktree.

## 6. Umfang v1

| Bereich | Funktionen | Abnahme |
| --- | --- | --- |
| Settings | Workspace wählen oder anlegen (mehrere, umschaltbar), Repos-Ordner, Theme, Agent-Funktionen an/aus | Leerer Ordner wird mit `_templates/` und `.workly/` zum Workspace |
| Projects | Liste, anlegen aus Vorlage, bearbeiten, Farbe, Sortierung per Drag, archivieren, löschen (Papierkorb) | Neues Projekt erzeugt Ordnerstruktur; Änderungen in Obsidian erscheinen ohne Neustart |
| Project detail | Klick in der Sidebar öffnet das Projekt, die Projektliste öffnet Overview. Tab **Overview**: Beschreibung (Body von `_project.md`, live), Eigenschaften (Key, Status, Farbe, Pfad, erstellt, Repos, Links, offene Tasks), Use-Case-Kurzinfo mit Link zum Tab Use case oder „Make use case…“; bearbeitet wird im Edit-Formular. Tab **Use case** (nur bei Use Cases), eigenes Layout: links zwei Karten: oben Status, Blocked by, Current state, Next step, Gate-/Next-Buttons (klein und zurückhaltend; verschieben sofort, ohne Toast) und Tasks mit Häkchen und „Add task“; darunter der Prozess als eigene, intern scrollende Karte – senkrechte Phasenliste mit Verlauf: vergangene Phasen mit Zeitraum, Entscheidungen und Status-Abweichungen, aktuelle Phase mit Schritt, Owner und Tagen, nächste Phase, der Rest in einer Zeile. Ein wachsender Verlauf verschiebt nichts darüber. Rechts Classification (Step, Area, Type) und die Beschreibung (scrollt), darunter immer an derselben Stelle die Bewertungskarte (nur mit `assessment:` in `process.yml`: Quadrant- oder K.-o.-Pill, „Value 2.3 · Feasibility 1.7“, Datum, durchgefallene K.-o.-Fragen, „Open for pilot“ mit höchstens 5 Zeilen und „+n“, Notiz, „Assess…“ / „Re-assess…“; sonst „Not assessed“) und die Aufwandskarte mit „Edit…“. Seltene Aktionen („Remove from cockpit…“) im Edit-Formular. Tab **Tasks**: Board nur mit den Tasks des Projekts, Layout wie das Tasks-Board, neue Tasks landen im Projekt. Tab **Files**: Übersicht, Ordnerbaum, Markdown-Vorschau, öffnen in Obsidian/VS Code/Finder, Repos verknüpfen, Bearbeiten | Alle Links öffnen das richtige Ziel; Wechsel zwischen Tasks-Board und Projekt-Tab verschiebt nichts im Layout |
| Tasks | Board, Gruppierung nach Projekt, Filter, Suche, Drag & Drop, Schnelleingabe, bearbeiten, löschen | Jede Änderung landet verlustfrei in der Datei |
| Detail card | Popup oder Seitenleiste (schließt mit Esc, ✕ oder einem Klick außerhalb; beide liegen über einem grauen Overlay, ein Klick daneben schließt nur und öffnet nichts), Status, Priorität, Fälligkeit, Tags, Beschreibung, Updates | Wie im Design System |
| Focus | Streifen mit bis zu drei Tasks, ein- und ausblendbar | `focus`, `focus_order` gesetzt |
| Use-case cockpit | Liste (sortierbare Tabelle, Standard; Spalte „Assessment“ sortiert Quick win, Big bet, Fill-in, Drop, K.O., Incomplete, unbewertet immer zuletzt), Board, Prozesslandkarte und Matrix (nur mit `assessment:` in `process.yml`, sonst kein Knopf: 2×2-Feld, x = Feasibility 1–3 nach rechts leichter, y = Value 1–3 nach oben mehr, Mittellinien bei 2,0, Quadrantnamen in den Ecken; ein Punkt pro bewertetem Use Case in seiner Projektfarbe, daneben ein Label mit Key und Titel – kollidierende Labels weichen aus und behalten eine Linie zum Punkt; Tooltip mit Titel, FTE, Rechenweg je Achse und offenen Punkten, Klick öffnet die Detailkarte; rechts eine Liste nach Quadranten (Anzahl, Σ FTE; je Use Case V/F, Schritt, offene Punkte), so hoch wie das Feld und intern scrollend, Hover verbindet Liste und Punkt; „How is this calculated?“ unter dem Feld zeigt beim Hover die Methode aus `process.yml` (Kriterien je Achse, Durchschnitt ohne Gewichte, Grenze 2,0 mit Quadrant-Tabelle, K.-o.-Fragen); darunter die Zeilen „K.O. (n)“ und „Not assessed (n)“ mit Chips, Incomplete steht bei Not assessed; Schalter Standard | Advanced links neben „Details“: Advanced zeigt Blasengröße nach FTE, Farbe nach Quadrant, gestrichelt = noch in der ersten Phase; dieselben Filter wie die Liste; kein Drag, Punkte gibt es nur im Formular), Detailkarte als leichte Version der Use-Case-Seite (Popup `--w-uc-detail-w` breit, mindestens `--w-uc-detail-min-h` hoch, oder Seitenleiste; Hauptspalte: Blocked by, Current state, Next step; Seitenspalte: Classification, Assessment (nur die Pill, Tooltip mit Value/Feasibility und offenen Punkten), Manual effort (nur Summe), Tasks, Entscheidungen; bearbeitbar sind Titel, Status, Texte, Classification, Task-Häkchen und Gate-Buttons; keine Beschreibung; „Open page ↗“ im Kopf neben ✕ oder ⌘↩ führt zum Tab Use case), Filter, Knopf „Edit process“ (öffnet `process.yml` in VS Code) | Wie im Prototyp, Prozess aus `process.yml`. Areas sind Freitext mit Vorschlägen (`process.yml`-Areas ∪ von Use Cases benutzte Areas), der Core prüft sie nicht gegen die Liste. Board nach Area zeigt nur Areas mit Use Cases (Reihenfolge aus `process.yml`) |
| Agents | CLI `wly`, Agent-Dateien, Schalter, Live-Status; „Start agent“ (Codex, Copilot) nur nach Entscheidung im Probebetrieb | Ein Agent setzt einen Task über `wly` auf Review; Schalter aus zeigt keine Agent-Oberfläche |
| Log | Jede Änderung in `.workly/log/` | |

**Nicht in v1:** Queue-Runner, Wochenreview, Jira/SharePoint, Markdown-Editor, Geräte-Sync, Mobile.

**Nicht-funktional:** Start < 1 s bei einigen hundert Dateien, keine CPU-Last im Leerlauf, externe Änderungen < 1 s sichtbar, hell und dunkel, Kürzel ⌘N (Schnelleingabe), ⌘K (Suche), ⌘1–4 (Ansichten).

## 7. Bauplan

| # | Meilenstein | Fertig, wenn |
| --- | --- | --- |
| M0 | Gerüst: Tauri 2 + SvelteKit + Cargo-Workspace, Tokens, Fonts, App-Rahmen | `.app` startet, leerer Rahmen hell und dunkel |
| M1 | Core: Scan, verlustfreies Lesen/Schreiben, Index, Watcher, IDs, Papierkorb, Log | Tests grün inkl. byte-gleicher Roundtrip; externe Änderung erscheint live |
| M2 | Projects + Settings | Abnahme laut Umfang |
| M3 | Tasks: Board, Gruppierung, Detailkarte, Fokus | Abnahme laut Umfang, Screenshots gegen Referenz |
| M4 | Use-Case-Cockpit | Wie im Prototyp |
| M5 | Schalter Agent-Funktionen, CLI `wly` + Agent-Dateien | Ein Agent setzt einen Fixture-Task über `wly` auf Review; Schalter aus blendet alle Agent-Oberfläche aus |
| M6 | Kürzel, leere Zustände, Fehler, Build, Installation | Ein Tag echter Einsatz |
| – | Probebetrieb, 1–2 Wochen echter Einsatz | Entscheidung über M7 ist hier nachgetragen |
| M7 (optional) | „Start agent“ mit Codex- und Copilot-Adapter, Live-Status | Ein Klick führt vom Task zum laufenden Agent, Status kommt zurück |

**Reihenfolge:** Der Agent-Start (M7) ist der teuerste und unsicherste Teil. Er kommt erst nach dem Probebetrieb und nur, wenn dort der Wunsch entsteht, einen Agent aus der App zu starten. Sonst endet v1 nach M6.
