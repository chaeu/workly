# Workly – Spezifikation v1

Stand: 6. Oktober 2026. Quelle: Dokument „Workly – Spezifikation v1“ (claude.ai). Bei Widerspruch gilt diese Datei im Repo.

## 1. Entscheidungen

Workly ist eine eigenständige, leichte macOS-App. Dateien sind die Quelle der Wahrheit, die App ist ein Werkzeug darauf.

| Thema | Entscheidung | Warum |
| --- | --- | --- |
| Stack | Tauri 2, SvelteKit (adapter-static, SPA, Svelte 5), Rust-Core | Native Hülle mit System-WebView statt gebündeltem Chromium; SvelteKit ist der stärkste Stack des Entwicklers |
| Daten | Markdown mit YAML-Frontmatter, keine Datenbank | Obsidian, VS Code, Git und Agents lesen dieselben Dateien |
| Projekt | Jeder Ordner mit einer `_project.md` ist ein Projekt | Ordner liegen, wo der Nutzer will; die App findet sie |
| Use Case | Ein Projekt mit einem `usecase:`-Block im Frontmatter. Das Formular (Anlegen und Edit; seltene Aktionen leben dort, nicht auf der Seite) hat unter „Show as“ zwei Checkboxen, mindestens eine an (keine = archivieren, Hinweis im Formular): **Project** (Sidebar, Projects-Tabelle, Projektauswahl für Tasks) und **Use case** (Cockpit). Use case anhaken hängt den Block ans Ende des Frontmatters (erster Schritt der ersten Phase); abhaken löscht ihn beim Speichern samt `decisions` nach Bestätigung, die Log-Zeile hält den alten Block in `from`, Projekt und Tasks bleiben. Project abhaken schreibt `tracked: false`, anhaken entfernt die Zeile (fehlt = Projekt). „+ New project“ startet mit Project, „+ New use case“ im Cockpit nur mit Use case. Der Core erlaubt `tracked: false` nur bei Use Cases und entfernt den Block nicht, solange `tracked: false` gilt. Ein nicht getrackter Use Case fehlt in Sidebar, Projects-Tabelle und Schnelleingabe (die Task-Detailkarte behält das aktuelle Projekt des Tasks, ⌘N schlägt ihn nicht vor); Cockpit, Projektseite per URL oder „Open page ↗“ und seine Tasks auf dem Tasks-Board bleiben | Keine zweite Entität; aus einer Idee wird ohne Umzug ein Projekt und zurück. Ein explizites Feld statt einer Regel aus dem Schritt: ein laufendes Projekt, das Use Case wird, bleibt sichtbar |
| Tasks | Liegen im Projektordner unter `tasks/`, Tasks ohne Projekt in `inbox/` | Alles zu einem Projekt an einem Ort, auch für Agents |
| IDs | Projekt-Key + Nummer (`WR-12`); Key wird aus dem Namen vorgeschlagen, beim Anlegen änderbar, danach fest; Inbox-Tasks nutzen den reservierten Key `IN` | Lesbar in Commits und Branches |
| Einstellungen | Pfade pro Gerät in der App, Regeln pro Workspace in `.workly/config.yml` | Standalone-fähig; derselbe Workspace funktioniert auf mehreren Macs |
| Agents | CLI `wly` als einzige Schreibschnittstelle für Agents; Ausführung durch Runner-Adapter (Copilot per `code chat` in VS Code, Codex CLI im Terminal), „Start agent“ seit B9 | Workly koordiniert und protokolliert, Agents arbeiten in ihren eigenen Werkzeugen |
| Agent-Funktionen | Abschaltbar per Geräte-Einstellung `agents_enabled` (Standard: aus). Aus = Basisversion ohne Agent-Oberfläche; Felder in den Dateien und die CLI bleiben | Workly bleibt ohne Agents ein schlankes PM-Werkzeug |
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
| `crates/workly-core` | Domänenmodell, Scan, Parser, verlustfreies Schreiben, Index, Watcher, IDs, Prozess, Log, Papierkorb, Backup |
| `src-tauri` (`workly-app`) | Tauri-Befehle und Events, dünn über dem Core |
| `crates/wly` | CLI über dem Core, JSON-Ausgabe für Agents |

## 3. Workspace und Dateiformat

Ein Workspace ist ein Ordner, zugleich Obsidian-Vault und Git-Repo. Menschenlesbares liegt sichtbar, Maschinendaten im versteckten `.workly/`.

### Einstellungen

| Ebene | Ort | Inhalt |
| --- | --- | --- |
| Gerät | `~/Library/Application Support/Workly/settings.json` | Workspaces (Name + Pfad, mehrere, umschaltbar), Repos-Ordner, Theme, UI-Präferenzen, Agent-Funktionen an/aus, Stunden pro FTE und Jahr, Backup je Workspace (`backup: {interval: off | 15m | hourly | daily, push}` im Eintrag des Workspace, Standard hourly und Push an). Fenster (Größe, Position, maximiert) speichert das Plugin `window-state` daneben in `~/Library/Application Support/dev.chaeu.workly/.window-state.json` |
| Workspace | `<workspace>/.workly/config.yml` | Ordner für neue Projekte und Inbox, Scan-Ausschlüsse, Task-Status, WIP-Limits, Agent-Standards (`agent_defaults`: `runner`, `model`, `effort`, `stale_after_minutes`, Standard 30), Agent-Liste (`agents:`, siehe Runner-Adapter) |
| Workspace | `<workspace>/.workly/process.yml` | Use-Case-Prozess (Phasen, Schritte, Gates, Kanten) und Bewertungskriterien (`assessment:` mit Kriterien je Achse samt Ankern, optional `fte`-Schwellen, Bonus-Kriterien mit `bonus: true`; K.-o.-Fragen optional) |

### Ordnerstruktur

```
<workspace>/
  .git/                        # workspace repo, backup (section 4)
  .gitignore                   # Workly adds missing lines: .workly/runs/, .DS_Store, .obsidian/workspace*.json, *.workly-tmp
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
    runs/<ID>/log.jsonl        # agent run log per task (wly task note), not in the backup
    runs/<ID>/prompt.md        # prompt of the last Start agent
    runs/<ID>/start.command    # Codex start script of the last Start agent (mode 700)
    runs/<ID>/baseline.json    # dirty files of the code repo at start (B8)
    runs/<ID>/review.json      # changed files and their hashes at review, summary (B8)
    runs/<ID>/commit-msg.txt   # message of the last Commit
    trash/                     # deleted items, restorable
```

Die App scannt alle Ordner unterhalb des Workspace (ohne `.git`, `node_modules`, `.obsidian`, `.workly`, `_templates`) nach `_project.md`. Der Watcher ignoriert `.git` immer, auch wenn es in `scan_exclude` fehlt: ein Backup-Commit löst keinen Rescan aus.

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
tracked: false                # optional, only for use cases: false = not a project (cockpit only); missing = true
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
    scores: { quality: 2, access: 2, integration: 3, reuse: 3 }   # 1 | 2 | 3, missing = open; volume comes from savings; bonus: 3 = yes, missing = no
    note: Compliance depends on whether output reaches customers
---
Short description in Markdown, shown as overview in the app.
```

### Task: `tasks/WR-12-<slug>.md`

```yaml
---
id: WR-12
title: Navigation überarbeiten
status: waiting               # backlog | todo | doing | waiting | review | done (task_statuses in config.yml)
priority: 2                   # 1 | 2 | 3
due: 2026-10-10
tags: [frontend]
focus: 2026-10-06             # on the focus strip when equal to today
focus_order: 1                # 1-3
order: 2                      # optional, manual position in its board column
created: 2026-10-06
done_at: null
waiting_for: "IT: read access" # optional, only while status is waiting
parent: WR-10                 # optional, one level: WR-10 has no parent itself
agent:                        # optional; ready: (old files) is still read, never written
  runner: codex               # codex | copilot; missing, null or auto = the default agent
  model: gpt-5                # missing/null/auto = the runner's own default
  effort: high                # low | medium | high | xhigh | max | ultra (Codex, per model); same
  permissions: auto           # ask | auto | full (Codex, B11); missing = ask
  active: null                # set while an agent works, e.g. codex
  since: null                 # ISO timestamp, drives "Codex working · 14 min"
  commit: null                # set by Commit (short sha) or review --commit
  last_run: null
  changes:                    # set at review: repo paths changed since start; what Commit commits
    - src/lib/Nav.svelte
  repo: ~/repos/website       # repo of changes
---
## Description
Burger menu below 720 px, keyboard accessible.

## Acceptance
- [x] Menu opens with Enter and Space
- [ ] Focus returns to the burger on close

## Updates
- 2026-10-06 09:12 · codex: Pipeline green on dev.
  Verify: open /blog on staging
```

### Regeln

- **IDs:** Nächste Nummer = höchste vorhandene Nummer dieses Keys im Workspace + 1 (inklusive Papierkorb und aller IDs im Log, damit auch ein geleerter Papierkorb keine Nummer freigibt), kein gespeicherter Zähler. Eine ID bleibt beim Verschieben in ein anderes Projekt unverändert.
- **Keys:** Vorschlag aus dem Titel: ist das erste Wort ein Kürzel in Großbuchstaben, wird es übernommen; sonst Anfangsbuchstaben der ersten bis zu drei Wörter; bei einem Wort die ersten drei Buchstaben. Bei Kollision Ziffer anhängen. `IN` ist reserviert.
- **Verlustfreies Schreiben:** Nur betroffene Felder ändern. Unbekannte Felder, Reihenfolge, Kommentare, Leerzeilen und Body bleiben byte-gleich. Wichtigster Test im Projekt.
- **Neue Tasks:** Status und Body kommen aus `_templates/task.md` (eingebaut: `status: backlog`, Body `## Description` / `## Acceptance` / `## Updates`). Kein Config-Key dafür; bestehende Workspaces behalten ihre eigene Vorlage. Schnelleingabe und volles Formular schreiben die Datei einmal (Vorlage + Felder + Abschnitte).
- **Waiting:** Status `waiting` steht in neuen Workspaces zwischen doing und review. Fehlt er in `config.yml`, fügt Settings → „Add Waiting column“ genau eine Zeile `- { id: waiting, label: Waiting }` in `task_statuses` ein (vor `review`, sonst am Ende; Einrückung und Klammerstil vom Nachbarn; atomar, eine Logzeile `config.update`). Hat die Liste eine andere Form als ein Flow-Map pro Zeile, schreibt Workly nichts und zeigt die Zeile mit „Open config.yml“. `waiting_for` (Text, optional) gilt nur in Waiting: wer Waiting verlässt (App, `wly task start`), verliert das Feld, das Log hält den alten Wert in `from`.
- **Body-Layout:** `## Description`, `## Acceptance` (alt: `## Acceptance criteria`, wird gelesen), `## Updates`; ein Abschnitt endet an der nächsten Überschrift der Ebene 1 oder 2. Ohne `## Description` gilt das alte Layout: Beschreibung = Text vor der ersten Acceptance- oder Updates-Überschrift. Die Beschreibung schreibt nur ihren Bereich (Konfliktprüfung gegen `expected`); Überschriften, die den Bereich beenden würden, lehnt der Core ab (`###` geht). „Add sections“ setzt `## Description` über den alten Text und ein leeres `## Acceptance` über `## Updates`, sonst ändert sich nichts.
- **Acceptance:** Kriterien sind die Listenpunkte `- [ ] …`, `- [x] …` oder schlicht `- …` (offen) direkt im Acceptance-Abschnitt; leere Punkte zählen nicht. Abhaken ändert genau eine Zeile (`- [ ]` ↔ `- [x]`, `- a` wird `- [x] a`) und wird abgelehnt, wenn Punkt n nicht mehr Text und Zustand hat, die die App gezeigt hat. „Add criterion“ hängt eine Zeile `- [ ] …` an (Abschnitt bei Bedarf über `## Updates` angelegt). Löschen und Umformulieren nur in der Datei. Listenpunkte außerhalb des Abschnitts (z. B. in der Beschreibung) sind keine Kriterien.
- **Parent:** `parent: <ID>`, eine Ebene: Ein Task mit Parent kann kein Parent sein, ein Parent bekommt keinen Parent, kein Task ist sein eigener. Der Core prüft beim Anlegen und beim Ändern von `parent` (App und CLI, lesbare Fehler); Projektgrenzen spielen keine Rolle, Verschieben ändert nichts. `null` entfernt die Zeile. Löschen eines Parents kaskadiert nicht, die Kinder zeigen „↑ WR-12 (deleted)“. Faustregel: Schritte unter einem Tag sind Acceptance-Kriterien, echte Arbeitspakete unter einem Ziel sind Kind-Tasks.
- **Updates** werden als Zeile unter `## Updates` im Body angehängt (Abschnitt wird bei Bedarf angelegt). Format `- JJJJ-MM-TT HH:MM · <wer>: <Text>`; `wer` = Agent-Name, `me` für die App. Eine Review-Zusammenfassung (`wly task review --summary`) ist ein Eintrag mit Folgezeilen, je zwei Leerzeichen eingerückt; die App zeigt sie als einen Eintrag mit Zeilenumbrüchen. Agent-Notizen landen nicht hier, sondern im Run-Log (Abschnitt 5).
- **Board-Reihenfolge:** In einer Spalte sortiert nach `order`, dann Priorität, dann ID; Tasks ohne `order` stehen hinten. Ziehen in eine andere Spalte ändert nur `status`; erst Umsortieren innerhalb einer Spalte schreibt `order` 1..n für die Tasks dieser Spalte (bzw. Zelle in der Projekt-Gruppierung).
- **`done_at`** folgt dem Status: wird beim Wechsel auf `done` auf das heutige Datum gesetzt, beim Verlassen von `done` auf `null`.
- **Fokus:** höchstens drei Tasks. Hinzufügen setzt `focus: <heute>` und `focus_order`; Entfernen löscht beide Zeilen. Werte von anderen Tagen bleiben liegen und werden ignoriert.
- **Projekt-Board** (Tab Tasks): ohne Projektfilter, Gruppierung und Fokus-Streifen; Spalten zeigen die Anzahl im Projekt ohne WIP-Limit (das gilt für den ganzen Workspace). An der Stelle des Fokus-Streifens steht eine gleich hohe Leiste mit den Tabs. Fokus wird auf dem Tasks-Board oder in der Detailkarte gesetzt.
- **Projekt-Tabs:** Overview · Use case · Tasks · Files in derselben Leiste; `/projects/<KEY>` ohne Parameter ist Tasks, `?tab=overview`, `?tab=usecase` bzw. `?tab=files` die anderen. Use case gibt es nur, solange das Projekt einen `usecase:`-Block hat; sonst (auch direkt nach „Remove from cockpit“) zeigt die Seite Overview. Die Projektliste öffnet Overview.
- **Projektwechsel in der Sidebar** öffnet den zuletzt benutzten Tab (Overview, Use case, Tasks oder Files, Standard Tasks; Use case bei einem Projekt ohne Use Case wird Overview); vom Tasks-Board aus immer Tasks. Gemerkt wird nur im Speicher.
- **Tasks-Board:** Tasks archivierter Projekte erscheinen nicht. Ziehen nach Waiting öffnet die Detailkarte mit dem Feld „Waiting for“ im Fokus (kein Pflichtfeld). Der Chip „Hide parents“ (Geräte-Einstellung `hide_parents`, erscheint nur, wenn es Parents gibt) blendet Tasks mit Kindern aus. In der Gruppierung nach Projekt hat die Inbox eine eigene Bahn; Ziehen in eine andere Bahn verschiebt die Datei (ID bleibt). Filter „Diese Woche“ = fällig bis einschließlich Sonntag, „Überfällig“ = fällig vor heute und nicht `done`.
- **Löschen** verschiebt nach `.workly/trash/` (Pfad erhalten), wiederherstellbar. Ein gelöschtes Projekt ist ein Eintrag; seine vorher einzeln gelöschten Tasks bleiben beim Wiederherstellen im Papierkorb. **Papierkorb leeren** verschiebt alles in den macOS-Papierkorb (dort noch wiederherstellbar) und schreibt jede ID ins Log.
- **Problems:** Je Problem Pfad, Meldung und, wo bekannt, ein Hinweis zur Behebung. Eindeutige Fälle (Zahl als Text, leerer Zahlwert, `process.yml` ohne `assessment:`) behebt ein Fix-Knopf: der Core berechnet den Fix beim Klick neu aus der Datei, ändert genau ein Feld oder hängt einen Block an und loggt. Alles andere: Hinweis, „Open in VS Code“ und Papierkorb (kaputte `_project.md` = ganzer Ordner; nie unter `.workly/`). Kein Ignorieren. Details: `docs/PROBLEMS.md`.
- **Log:** Jede Änderung als JSON-Zeile in `.workly/log/JJJJ-MM.jsonl`: `{ts, actor, kind, id, field, from, to}`. `actor` = `app`, `cli`, `agent:<name>`. Notizen im Run-Log stehen nur dort (Abschnitt 5). Ein Backup-Commit: `kind: backup`, `id` leer, `to: {sha, files, pushed, error}`.
- **Use-Case-Verlauf:** Der Core liest das Log (nur lesen) und baut daraus pro Phase Zeitraum, Status-Episoden (mit dem letzten „Blocked by“) und die Entscheidungen aus `usecase.decisions` (Gate-Entscheidungen in der Phase des Gates, sonst nach Datum). `usecase.remove` setzt den Verlauf zurück. Der Verlauf kennt nur Änderungen über Workly; weicht die Datei vom Log ab, gilt die Datei (`step`, `step_since`).
- **Ersparnis (`usecase.savings`):** Stunden und FTE werden berechnet, nie gespeichert. Stunden/Jahr = Σ `count` × Faktor(`per`) × `minutes` / 60 mit Faktor year 1, month 12, week 46, day 220 (Arbeitswochen/-tage, Konstanten in der App); FTE = Stunden / `fte_hours_per_year` (Geräte-Einstellung, Standard 1720). Die App schreibt immer die ganze Liste und prüft dabei: `what` nicht leer, `count` und `minutes` Zahlen ≥ 0, `per` aus der Liste. Ungültige Einträge von Hand stehen unter Problems, der Use Case lädt trotzdem. Anzeige: auf der Use-Case-Seite eine Karte unter der Beschreibung (Summe, eine Zeile pro Tätigkeit, Notiz), bearbeitet im Popup „Edit…“ (Tabelle, Notiz, Umrechnung); Detailkarte (Popup, Seitenleiste) zeigt in der Seitenspalte nur die Summe „≈ FTE · h/yr“, der Tooltip listet jede Tätigkeit und die Umrechnung (ohne Einträge: „No effort recorded“ mit Link zur Seite); sortierbare Spalte Effort (FTE-Wert) in der Liste (leer = hinten), Pill auf der Board-Karte bei > 0. Keine Summe im Cockpit, kein Automatisierungsgrad, keine Geldbeträge. UI-Label *Manual effort*, der Feldname bleibt `savings`.
- **Bewertung (`usecase.assessment`):** Vor G0 „Pilot?“ und nach dem Pilot an G1 erneut. Die Methode steht in `process.yml` unter `assessment:` (`criteria:` mit `id`, `axis` value | feasibility, `label`, genau 3 `anchors` für 1, 2, 3, optional `fte: [a, b]` mit zwei aufsteigenden Zahlen > 0; ein Bonus-Kriterium hat `bonus: true`, steht auf der Achse value, hat genau einen Anker (was „Ja“ heißt) und kein `fte`; jede Achse braucht mindestens ein Kriterium, das kein Bonus ist; optional `ko:` mit `id`, `label` – die Vorlage hat keine K.-o.-Fragen, ohne sie blendet die App jede K.-o.-Oberfläche aus); fehlt der Block, blendet die App jede Bewertung aus und zeigt unter Problems „Add default assessment“ (hängt den Block aus der eingebauten Vorlage an, siehe `docs/PROBLEMS.md`). Pro Use Case gibt es eine aktuelle Bewertung; eine Neubewertung überschreibt sie, der alte Stand steht im Log (`from`). K.-o.-Werte sind `pass` / `fail` / `open` (nicht yes/no, YAML 1.1), Punkte 1–3; ein fehlender Schlüssel heißt offen. Ein Bonus ist Ja/Nein: gespeichert wird nur Ja als 3, ein fehlender Schlüssel heißt Nein; Nein zählt nicht und ist nie offen, ein Bonus kann den Value also nur heben (Vorlage: `customer`, `money`, `reuse`). Hat ein Kriterium `fte` und der Use Case Einträge in Manual effort, rechnet die App seine Punkte stufenlos aus den FTE: 1 + 2 · (FTE − a) / (b − a), begrenzt auf 1–3, auf eine Nachkommastelle gerundet (Vorlage: `volume` mit `[0.5, 2]`, 1,9 FTE → 2,9); ein von Hand gesetzter Wert (1, 2 oder 3) gilt nur ohne Effort-Einträge. Value, Feasibility (Durchschnitt der bewerteten Kriterien je Achse samt abgeleiteter, ohne Gewichte), Quadrant (Grenze 2,0: Quick win, Big bet, Fill-in, Drop; ein `fail` = K.O.) und offene Punkte werden berechnet, nie gespeichert. Die App schreibt immer den ganzen Block (ein Write, eine Logzeile, `ko` und `scores` als Flow-Maps, `null` entfernt ihn) und prüft dabei Werte, Datum und dass jede ID in `process.yml` steht. Ungültige Werte von Hand stehen unter Problems und gelten als offen, der Use Case lädt trotzdem. **Anzeige:** bewertet wird im Popup „Assess…“ auf dem Tab Use case (K.-o.-Fragen pass / fail / open, je Kriterium 1 · 2 · 3 · ? mit den drei Ankern sichtbar, ein abgeleitetes Kriterium fix mit „From Manual effort: 1.9 FTE → 2.9“, Bonus-Kriterien unter den Value-Kriterien in einer ruhigen Gruppe „Bonus“ als Häkchen mit Label und Anker (an = 3, aus = Schlüssel entfernt), Notiz, Ergebnis live im Fuß samt Rechenweg je Achse, z. B. „Value (2.9 + 2 + 3) / 3 = 2.6“); „Save“ schreibt den Block mit heutigem Datum, ohne abgeleitete Kriterien, ohne IDs, die nicht mehr in `process.yml` stehen, und ohne Bonus-Werte außer 3 (so räumt „Re-assess…“ alte Bewertungen auf), „Clear…“ nach Rückfrage `null`. Die Pill zeigt den Quadranten (Quick win `--w-ok`, Big bet `--w-info`, Fill-in `--w-warn`, Drop `--w-hold`), K.O. (`--w-danger`) sticht den Quadranten, „Incomplete“, solange eine Achse keine Punkte hat. Ein offenes K.-o. zeigt weiter den Quadranten und steht unter den offenen Punkten. Die Bewertungskarte zeigt neben den Werten „n open“, damit man sieht, wie belastbar das Ergebnis ist; eine Mindestabdeckung gibt es nicht.
- **Dateinamen:** `<ID>-<slug>.md`; Titeländerungen benennen die Datei nicht um (stabil für Links).
- **Projekt-Keys:** 2–6 Zeichen, Großbuchstaben, Ziffern erst nach dem ersten Buchstaben (Kollisionsziffer, z. B. `WR2`).
- **Neue Projekte** liegen in `<new_projects_dir>/<slug>/`. Existiert der Ordner schon ohne `_project.md`, bietet das Formular „Use existing folder“ an: die App schreibt `_project.md` und legt nur Fehlendes an (Unterordner, `agent/AGENTS.md`, `<key>.code-workspace`); vorhandene Dateien werden nie überschrieben oder verschoben, Titel und Key kommen aus dem Formular. Hat der Ordner schon eine `_project.md`, ist er bereits ein Projekt und das Anlegen schlägt fehl. Projekt löschen verschiebt den ganzen Ordner in den Papierkorb.
- **`<key>.code-workspace`** ist generiert: beim Anlegen, bei jeder Änderung von `repos` und bei „Open in VS Code“ neu geschrieben (absolute Pfade, pro Mac). Repos stehen im Frontmatter als `~/…`.
- **Workspace anlegen:** Ein Ordner ohne `.workly/config.yml` bekommt die fehlenden Teile aus den eingebauten Vorlagen; vorhandene Dateien werden nie überschrieben. Er wird zugleich Git-Repo für das Backup (Abschnitt 4); unter OneDrive oder iCloud nur mit Warnung, der Workspace entsteht trotzdem.

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
- **Git:** Workly committet nur das **Workspace-Repo**, als Backup. Code-Repos nur auf einen ausdrücklichen Klick (B8) und dort ohne Push. Agents committen nie und legen keine Repos an.
  - **Repo:** Nur ein Workspace, der selbst `.git` hat; ein Workspace in einem fremden Repo wird nie committet. Neue Workspaces bekommen `git init` und `.gitignore`; bestehende per „Enable backup“ in Settings. Unter `~/Library/CloudStorage/` (OneDrive) und `~/Library/Mobile Documents/` (iCloud) lehnt Workly das ab, weil der Sync `.git` zerlegt. Ein vorhandenes Repo samt Branches und Remotes bleibt, wie es ist.
  - **Backup:** beim App-Start und Workspace-Wechsel (fängt Änderungen aus Obsidian ab), im Intervall, mit „Back up now“ und beim Beenden (höchstens 3 s, Beenden wartet nie länger). Nie im UI-Thread, nie zwei Läufe gleichzeitig. Übersprungen mit Fehler bei detached HEAD, laufendem Merge, Rebase, Cherry-Pick oder Revert und bei Konflikten im Index. Sonst `git add -A` und Commit `backup: JJJJ-MM-TT HH:MM (N files)`; nichts geändert (oder nur die eigene Logzeile unter `.workly/log/`, die fährt beim nächsten Backup mit) = kein Commit.
  - **Push** auf den Upstream des aktuellen Branches (`git push <remote> HEAD:<merge-ref>`), wenn Push an ist, auch nachgeholt, solange der Branch voraus ist. Nie Force, nie andere Branches, nie History umschreiben, kein Pull. Workly richtet keine Remotes ein und ändert keine; ohne Upstream bleiben die Commits lokal („Backed up locally“, kein Fehler).
  - **Ohne Rückfrage:** System-`git` mit Argumentliste (keine Shell), `GIT_TERMINAL_PROMPT=0`, SSH mit `BatchMode`, Git Credential Manager nicht interaktiv, 30 s Timeout je Aufruf. Fehlende Zugangsdaten sind ein Fehler, kein hängendes Fenster. Zugangsdaten in URLs werden in Anzeige und Log durch `***` ersetzt.
  - **Code-Repo (B8):** Commit nur auf Klick („Commit“, „Commit & Done“ in der Task-Detailkarte), nur die Pfade aus `agent.changes`, nie Push. Abgelehnt mit lesbarer Meldung, wenn der Index schon fremde gestagte Änderungen hat; die werden nie mitcommittet. Jeder Pfad wird vor `git add` geprüft (relativ, im Repo, kein `..`), Pfade sind literal (`GIT_LITERAL_PATHSPECS`), die Nachricht kommt aus einer Datei (`git commit -F`). Hooks des Repos laufen, bis 120 s. Derselbe Git-Aufruf wie beim Backup.
  - **Anzeige:** Sidebar-Fuß „Backed up 12 min ago“ (muted), „Backed up locally …“, „Backup failed – details“ (`--w-warn`, führt zu Settings → Backup mit den letzten 20 Zeilen stderr) oder „Backup off“ (kein Repo oder Intervall aus). Settings → Backup: Intervall, Push, „Back up now“, Upstream nur lesend. Der Status lebt nur im Speicher; die Commits sind der Nachweis.

## 5. Agentic Workflow

### CLI `wly`

Läuft auf demselben Core wie die App. Agents (und der Nutzer) lesen und ändern Tasks darüber.

```
wly task list [--project WR] [--status todo] [--json]
wly task show WR-12 [--json]          # task + project + agent files as context
wly task start WR-12 --agent codex    # status doing, agent.active + agent.since set; from doing only the agent fields; repo baseline
wly task note WR-12 "text"            # appends to the run log .workly/runs/WR-12/log.jsonl
wly task note WR-12 "text" --to-body  # appends to ## Updates instead (escape hatch)
wly task review WR-12 [--summary "…"] [--commit a3f9c1]   # status review, agent.active cleared, agent.changes; never done
wly task add "title" [--project WR | --inbox] [--priority 2]
wly project list [--json]
```

Exit-Codes: 0 ok, 1 Nutzungsfehler, 2 nicht gefunden, 3 ungültiger Übergang. `--workspace <path>` oder `WORKLY_WORKSPACE` wählt den Workspace, sonst der in den Geräte-Einstellungen aktive.

### Agent-Dateien

- `<projekt>/agent/AGENTS.md`: Projekt-Anweisungen.
- `<projekt>/agent/skills/<name>/SKILL.md`: wiederkehrende Abläufe.
- `<workspace>/.workly/agent/AGENTS.md`: globale Regeln. Die eingebaute Vorlage (= Fixture):
  1. Task-Status nur über `wly`; Start `wly task start <ID> --agent <name>`.
  2. Nie `git init`, nie Repos anlegen, nie Remotes ändern.
  3. Nie committen, pushen, stashen oder resetten; Änderungen bleiben im Working Tree, der Nutzer committet aus Workly.
  4. Kein Repo oder das Repo nicht gefunden: `wly task note <ID> "BLOCKED: <Grund>"` und aufhören.
  5. `wly task note` sparsam, nur für Meilensteine, die einen Menschen interessieren.
  6. Abschluss mit genau einem `wly task review <ID> --summary "<3–6 Zeilen: was geändert, wie prüfen, was offen>"`; nie `done`.
  7. Nur im Repo arbeiten, in dem der Agent gestartet wurde; außerhalb nichts löschen.

  `init` überschreibt eine vorhandene `AGENTS.md` nie. Deshalb trägt „Copy prompt“ dieselben Regeln; bestehende Workspaces passt der Nutzer von Hand an.

Beim Start setzt Workly den Kontext zusammen: globale Regeln, Projekt-Anweisungen, Skills, Task.

### Runner-Adapter

Jeder Agent ist ein Adapter: Kontext zusammensetzen, starten, Status über `wly`. Workly übergibt an die Werkzeuge, in denen der Nutzer ohnehin arbeitet, und sieht danach keinen Prozess mehr (siehe Stale). v1, Knopf **„Start agent“** im Agent-Bereich der Task-Detailkarte (Primärknopf, hinter `agents_enabled`):

1. **Ein Agent pro Repo:** `start_task` im Core (also auch `wly task start`, Exit 3) verweigert mit der ID des anderen Tasks, solange ein anderer Task, dessen Projekt dasselbe erste Repo hat, `agent.active` gesetzt hat. Ohne Agent-Namen gibt es keine Prüfung.
2. **Start** über dieselbe Core-Funktion als App: Status `doing`, `agent.active` = Runner, `agent.since`. Aus `doing` setzt sie nur die Agent-Felder.
3. Der Prompt (derselbe wie „Copy prompt“, mit „bereits gestartet“) landet in `.workly/runs/<ID>/prompt.md`; im argv steht nie der Prompt, nur sein Pfad.
4. **Copilot:** `code` aus dem App-Bundle der neuesten VS-Code-Installation ab 1.98 (`/Applications`, sonst Spotlight nach `com.microsoft.VSCode`; nie PATH, eine aus dem Finder gestartete App hat den Shell-PATH nicht). Erst `code <key>.code-workspace` (vorher neu geschrieben), nach einer kurzen Pause `code chat -m agent -r -a <prompt.md> "<Zeile>"`: Chat im Agent-Modus im zuletzt aktiven Fenster, mit dem Prompt als angehängter Datei, sofort abgeschickt. Ein nicht vertrauter Workspace fragt vorher nach Workspace Trust.
5. **Codex:** `.workly/runs/<ID>/start.command` (Modus 700), geöffnet mit `open -a Terminal` (kein AppleScript, keine Bedienungshilfen-Freigabe). Terminal führt es in der Login-Shell des Nutzers aus, `codex` kommt also aus dessen PATH, sonst aus der neuesten VS-Code-Extension (`~/.vscode/extensions/openai.chatgpt-*/bin/macos-aarch64/codex`), sonst eine lesbare Meldung. Das Skript wechselt ins erste Repo, setzt `WORKLY_WORKSPACE` und `WORKLY_AGENT=codex` und startet `codex [-m <model>] [-c model_reasoning_effort=<effort>] <permissions> --add-dir <Projektordner> --add-dir <workspace>/.workly "$(cat prompt.md)"`; Permissions (B11): ask = `-s workspace-write` (Codex fragt nach), auto = `-s workspace-write -a never` (fragt nie, bleibt in der Sandbox, kein Netzwerk), full = `--dangerously-bypass-approvals-and-sandbox` (keine Sandbox). Die beiden `--add-dir` gelten in allen Stufen: `wly` schreibt nach `.workly/` und in die Task-Datei, Projektnotizen gehören in den Projektordner; Modell und Effort aus der Agent-Wahl (unten), ungesetzt = kein Flag, Codex nimmt dann `~/.codex/config.toml`. Jeder Pfad im Skript ist in einfachen Anführungszeichen (ein Helfer), der Prompt steht nie im Skript.
6. **Rückfall** (VS Code oder `code` fehlt, Startfehler): Prompt in die Zwischenablage, Workspace in VS Code öffnen, Banner mit Grund und „Prompt copied – paste into the chat“. Der Task bleibt gestartet. Nie im UI-Thread.

Deaktiviert mit Grund: Projekt ohne Repo, Repo fehlt auf diesem Mac, Task `done`, anderer Agent im Repo, Agent schon aktiv an diesem Task, Runner weder Copilot noch Codex. „Copy prompt“ bleibt immer verfügbar. Claude Code folgt als weiterer Adapter.

**Agent-Liste (B10).** `agents:` in `.workly/config.yml` ist das Menü, aus dem man wählt; der erste Eintrag ist der Standard:

```yaml
agents:
  - { name: Codex Luna, runner: codex, model: gpt-6-luna, effort: medium }
  - { name: Codex Sol, runner: codex, model: gpt-6-sol, effort: high }
  - { name: Copilot, runner: copilot }
```

`name` ist Pflicht und eindeutig, `runner` ist `codex` oder `copilot`, `model` und `effort` sind optional (fehlt = Standard des Runners), `permissions` ist `ask` (Standard), `auto` oder `full` (B11, nur Codex). Copilot ignoriert alle drei: `code chat` hat weder Modell- noch Freigabe-Parameter, beides stellt man in VS Code ein. **Agent-Wahl** (Core `agent_choice`, im Frontend gespiegelt): Nennt der Task einen Runner (`agent.runner` gesetzt, nicht `auto`), gelten seine Felder `model`, `effort`, `permissions` so, wie sie sind (bei `permissions` ist `auto` ein Wert, nicht „ungesetzt“); sonst der erste Eintrag der Liste. Ohne `agents:` füllt `agent_defaults` fehlende Felder (Workspaces vor B10).

- **Task-Detailkarte:** ein Menü „Agent“ statt Runner/Model/Effort: „Default · <erster Name>“, die Einträge, und – wenn die Felder des Tasks zu keinem Eintrag passen – deren Werte als eigene Zeile („Codex · gpt-x · low“), damit nichts verloren geht. Darunter Modell und Effort, die tatsächlich starten („gpt-5 · high“, „default model · medium“, bei Copilot „Model picked in VS Code“). Die Wahl schreibt `agent.runner/model/effort/permissions` in einem Write (fehlend = `null`), „Default“ setzt alle auf `null`. In der Zeile darunter steht „no prompts“ bzw. „full access“, wenn nicht ask. Der Knopf heißt „Start <Name>“. QuickAdd hat dasselbe Menü.
- **Agents → Setup** (hinter `agents_enabled`; bis B11 Settings → Agents): Tabelle Name, Runner, Model, Effort (—, low, medium, high, xhigh, max, ultra), Permissions (Ask, Auto, Full; Full in `--w-warn`, darunter eine Erklärung der drei Stufen), je Zeile ↑ ↓ ✕, die erste Zeile mit „Default“, darunter „+ Add agent“. Jede vollständige Änderung schreibt die ganze Liste (eine neue Zeile erst mit Namen). Bei Copilot sind Model, Effort und Permissions gesperrt und werden geleert. Geschrieben wird verlustfrei: nur der Block `agents:` ändert sich (eine Zeile je Eintrag als Flow-Map, unveränderte Zeilen bleiben byte-gleich), fehlt er, wird er angehängt, leer = `agents: []`; eine Logzeile `config.update` mit Feld `agents`. Ungültige Einträge oder eine Form, die der Patcher nicht sicher ändern kann, werden abgelehnt, nichts wird geschrieben, die Meldung steht mit „Open config.yml“ im Panel.
- „auto“ gibt es in der UI nicht mehr; alte Werte `auto` gelten als ungesetzt. Eine automatische Wahl (Orchestrator) ist v2 und käme als weiterer Eintrag dazu.

### Übergabe per Prompt

„Copy prompt“ im Agent-Bereich der Task-Detailkarte (Prompt: Task-ID, zuerst `wly task show <ID>`, dann die globalen Regeln mit `--agent <runner>`; bei `doing` ohne `start`; danach Beschreibung und Acceptance-Punkte des Tasks, wenn vorhanden) und „Open repo“ (erstes Repo des Projekts in VS Code). Der Prompt wird in Copilot Chat oder Codex eingefügt; „Start agent“ nimmt denselben Text. Runner = der der Agent-Wahl, sonst `agent`. `agent.ready` ist veraltet: alte Dateien laden weiter, Workly schreibt es nie und zeigt es nicht.

**Wohin der Agent schreibt (B11):** Er arbeitet im Repo. Notizen über das Projekt – Entscheidungen, Doku des Projekts, Besprechungsnotizen – schreibt er in den Projektordner im Workspace (`docs/`, `notes/`, `decisions/`); der Prompt nennt dessen absoluten Pfad (Regel 8), entschieden wird aus dem Tasktext. Sonst nirgends. „Changes“ und „Commit“ (B8) sehen nur das Repo: Dateien im Projektordner werden nie von Workly committet, sie sichert das Workspace-Backup.

### Run-Log

`wly task note` hängt eine JSON-Zeile an `.workly/runs/<ID>/log.jsonl`: `{ts, agent, run, text}`. `run` = `agent.since` des laufenden Laufs (solange `agent.active` gesetzt ist), sonst `null`. Die Task-Datei bleibt unverändert, `.workly/log/` bekommt keine Zeile, das Backup ignoriert `.workly/runs/`. `--to-body` schreibt wie früher unter `## Updates`; Notizen aus der App gehen immer dorthin. Workly baut den Pfad nur aus einer ID der Form `KEY-N`.

Der Watcher meldet Änderungen unter `.workly/runs/` ohne Rescan des Index; die Detailkarte liest den Run-Log neu. Abschnitt **Agent activity** (hinter `agents_enabled`, nur bei Einträgen oder stale): eingeklappt der jüngste Eintrag und „N more“, ausgeklappt alle, neueste zuerst, je Zeit und eine Zeile; lange Zeilen gekürzt, ein Klick zeigt die ganze.

### Stale

Letzte Aktivität = jüngster Run-Log-Eintrag des laufenden Laufs, sonst `agent.since`. Ist sie bei gesetztem `agent.active` älter als `agent_defaults.stale_after_minutes` (Standard 30), zeigt die Karte „Agent inactive?“ (`--w-warn`, ohne Ring) statt „Codex working · n min“, und die Detailkarte bietet „Clear agent“: löscht `agent.active` und `agent.since` in einem Write mit Log, der Status bleibt. Kein PID-Watching, kein `agent.status`, kein `last_seen`: Copilot und Codex laufen als VS-Code-Extensions, es gibt keinen Prozess, den Workly sehen könnte.

### Ansicht Agents (B12)

Der Ort, an den man zurückkommt: Agents arbeiten in VS Code und im Terminal, Workly zeigt, was davon den Nutzer braucht. Drei Tabs in der Leiste unter dem Seitenkopf (`?tab=activity|setup`, ohne = Overview), Unterzeile „N need you · M running“.

- **Needs you** (Zahl am Menüpunkt „Agents“ und am Tab Overview) = blockiert + in Review mit Agent-Arbeit (`last_run`, `changes` oder `commit`) + „Agent inactive?“.
- **Blocked:** Agent aktiv und der letzte Run-Log-Eintrag des laufenden Laufs beginnt mit `BLOCKED` (Prompt-Regel 5). Kein neues Feld; der Snapshot trägt `agent_blocked` (ID → Notiz). „Clear agent“ räumt auf.
- **Overview:** oben „Running“, eine Kachel je aktivem, nicht blockiertem Agent (Badge wie auf der Karte, Repo, Task, letzte Notiz des Laufs; aktiv mit Akzent-Ring, inaktiv in `--w-warn` mit „Clear agent“), darunter „Free: …“ (erste Repos lebender Projekte, die auf diesem Mac liegen und frei sind). Dann „Needs you“ als eine Liste: Blocked zuerst (mit Grund), dann Reviews, älteste zuerst. Eine Review-Zeile hat „Commit“ und „Commit & Done“ (nur mit `agent.changes` und ohne `agent.commit`, sonst „Done“) und klappt auf: Summary (letzter Eintrag unter `## Updates`) und Dateien wie in Changes. Hat sich eine Datei seit dem Review geändert, öffnet Commit die Task-Karte (dort Diff und Rückfrage, B8) statt zu committen.
- **Activity:** Zeitleiste der letzten 30 Tage, höchstens 300 Einträge, nach Tagen gruppiert (Core `agent_feed`, nur lesend): Run-Log-Notizen (`note`, `blocked`) plus aus `.workly/log` der Start (`agent.active` gesetzt), das Review eines Agents (Status doing → review, Actor nicht `app`) und der Commit. Filter je Agent und „Reviews & blocks“.
- **Setup:** die Agent-Liste (oben, unverändert wie bisher in Settings), **Checks** (`wly`-Link mit Install/Reinstall, Codex an den üblichen Orten – `~/.local/bin`, Homebrew, npm, nvm, VS-Code-Extension – samt `--version`, VS Code ≥ 1.98, Repos auf diesem Mac, globale Regeln gleich der eingebauten Vorlage; alles, was passt, steht in einer Zeile „N checks OK“, der Rest einzeln), **Rules & skills** (globale `AGENTS.md`, je Projekt `agent/AGENTS.md` mit „Open“ oder „Create“, Skills als Chips). Eigene globale Regeln sind ein Hinweis, keine Warnung („Copy built-in rules“ zum Vergleichen). Settings behält nur den Schalter und verweist hierher.

### Commit aus Review

Workly committet die Änderungen eines Agents im Code-Repo auf einen ausdrücklichen Klick; Agents committen nie (Regel 3).

1. **Baseline:** `start_task` im Core (also `wly task start`, „Start agent“ und der Start durch den Agent nach „Copy prompt“) hält den Zustand des Repos fest: Repo = erstes Repo des Projekts, im Toplevel des Git-Repos; `HEAD` und `git hash-object` jeder geänderten oder ungetrackten Datei (`git status --porcelain -z --untracked-files=all --no-renames`) → `.workly/runs/<ID>/baseline.json`. Ein Start aus backlog, todo oder waiting schreibt sie neu; ein Start aus doing oder review (Nachbessern) behält die vorhandene, damit nicht committete Änderungen des ersten Laufs in der Liste bleiben. „Copy prompt“ bei `doing` legt eine an, wenn keine da ist. Kein Repo, kein Git-Repo oder Git-Fehler: keine Baseline, der Start klappt trotzdem.
2. **Review:** `wly task review` vergleicht mit der Baseline. Geändert = jetzt dirty und entweder in der Baseline sauber oder mit anderem Inhalt (neu, geändert, gelöscht; ein Rename zählt als gelöscht + neu). Im selben Write: `agent.changes` (Block-Liste), `agent.repo` (`~/…`), `agent.commit: null` (außer mit `--commit`). `.workly/runs/<ID>/review.json` hält die Hashes zur Review-Zeit und die Summary. Ohne Baseline bleibt alles wie vorher.
3. **Anzeige** (hinter `agents_enabled`): Review-Karte ohne Commit „N files changed“. Detailkarte, Abschnitt **Changes**: je Pfad A/M/D, ein Klick zeigt den Diff (nur lesen; `git diff HEAD` bzw. der Inhalt einer neuen Datei; ab 400 Zeilen gekürzt, Binärdateien als Hinweis). „edited“ markiert Dateien, die sich seit dem Review geändert haben.
4. **Commit** (nur aus review): `git add -- <geänderte Pfade aus agent.changes>`, `git commit -F .workly/runs/<ID>/commit-msg.txt` mit `<ID>: <Titel>` und der Summary als Body. Danach `agent.commit` = kurzer SHA, Log `{kind: commit, id, to: {sha, repo, files}}`. Der Task bleibt in review; „Commit & Done“ setzt danach `done`. Hat sich eine Datei seit dem Review geändert, zeigt die App ihren Diff und fragt vor dem Commit. Git läuft nie im UI-Thread, der Commit ohne Workspace-Sperre.

Nicht in v1: „Commit all reviewed“ pro Repo (eine Commit-Folge über mehrere Tasks), Erkennung eines Agents, der trotz Regel selbst committet hat (dann ist die Liste leer), Änderungen beim Ziehen einer Karte nach Review (nur `wly task review` berechnet sie).

### Schalter Agent-Funktionen

`agents_enabled` in den Geräte-Einstellungen (Standard: aus). Aus blendet jede Agent-Oberfläche aus: Ansicht und Menüpunkt „Agents“, Agent-Bereich, Changes und Agent activity der Task-Detailkarte samt „Clear agent“ und „Commit“, Agent-Badge (auch „Agent inactive?“), -Ring, Commit und „N files changed“ auf Karten, Agent-Dateien in der Projektansicht und „Start agent“. Die Ansicht „Agents“ trägt Agent-Liste, „Install CLI“ und Checks (Tab Setup). Nichts wird gelöscht: `agent:`-Felder bleiben in den Dateien, `wly` funktioniert weiter. Jede neue Agent-Oberfläche hängt an diesem einen Schalter.

### Stufen

| Stufe | Inhalt |
| --- | --- |
| v1 | Agent-Dateien, CLI, Schalter, Live-Status auf der Karte; „Start agent“ mit zwei Adaptern (Copilot per `code chat`, Codex im Terminal), ein Agent pro Repo (M7, gebaut als B9) |
| v1.5 | Queue: Läufe ohne Fenster nacheinander, Git-Worktree je Task auf Branch `wly/WR-12`, Agent-Ansicht (Queue, Running, Review, Failed) mit Live-Log |
| v2 | Parallele Läufe, automatische Modellwahl, Budgets, Folge-Tasks, MCP-Server |

**Grenzen:** Agents setzen nie `done`, löschen nichts, arbeiten nur im Repo bzw. Worktree.

## 6. Umfang v1

| Bereich | Funktionen | Abnahme |
| --- | --- | --- |
| Settings | Workspace wählen oder anlegen (mehrere, umschaltbar), Repos-Ordner, Theme, Agent-Funktionen an/aus, Backup (Intervall, Push, Back up now) | Leerer Ordner wird mit `_templates/` und `.workly/` zum Workspace |
| Projects | Tabelle im Stil der Cockpit-Liste mit Spalten ID (Farbe + Key), Title, Tasks (Balken je Task-Status + „n open“), Next up, Due, Use case (Schritt mit Status-Punkt, Tage ab `stale_after_days`), Repo (erster Repo-Ordnername, „+n“ bei mehreren). **Next up** = ein offener Task: Backlog und Waiting zählen nur, wenn sonst nichts offen ist; Reihenfolge überfällig (ältestes `due` zuerst), heute im Fokus (`focus_order`), Priorität, `due`, ID. Klick auf den Titel öffnet die Detailkarte, ohne offenen Task „+ Add task“. **Due** = `due` dieses Tasks, dazu „+n overdue“ für weitere überfällige Tasks. Keine weitere Use-Case-Info außerhalb der Spalte Use case (kein Blocker, keine rote Kante). Chips über der Tabelle zählen Projekte und filtern (ODER): Overdue, Due today, In focus, No open tasks; dazu Suche nach Key oder Titel. Spalten außer Next up sortierbar, dritter Klick zurück zur manuellen Reihenfolge; Ziehen nur ohne sortierte Spalte (schreibt `order`). Title, Tasks und Next up haben feste Breiten und schrumpfen anteilig mit dem Fenster, freie Breite liegt zwischen Due und Use case. Klick auf eine Zeile öffnet Overview. Pausierte mit Badge in der Tabelle, archivierte eingeklappt darunter. Use Cases mit `tracked: false` fehlen (siehe Abschnitt 1); darunter eine ruhige Zeile „n use cases not tracked as projects → Use cases“, nur bei n > 0. Ziehen ordnet sie hinter den sichtbaren Zeilen ein. Anlegen aus Vorlage, bearbeiten, Farbe, archivieren, löschen (Papierkorb) | Neues Projekt erzeugt Ordnerstruktur; Änderungen in Obsidian erscheinen ohne Neustart |
| Project detail | Die Sidebar listet die Projekte im Alltag (nicht archiviert, nicht `tracked: false`). Klick in der Sidebar öffnet das Projekt; ein Projekt außerhalb der Liste (per URL oder „Open page ↗“) markiert keinen Eintrag, die Projektliste öffnet Overview. Tab **Overview**: Beschreibung (Body von `_project.md`, live), Eigenschaften (Key, Status, Farbe, Pfad, erstellt, Repos, Links, offene Tasks), Use-Case-Kurzinfo mit Link zum Tab Use case oder „Make use case…“; bearbeitet wird im Edit-Formular. Tab **Use case** (nur bei Use Cases), eigenes Layout: links zwei Karten: oben Status, Blocked by, Current state, Next step, Gate-/Next-Buttons (klein und zurückhaltend; verschieben sofort, ohne Toast) und Tasks mit Häkchen und „Add task“; darunter der Prozess als eigene, intern scrollende Karte – senkrechte Phasenliste mit Verlauf: vergangene Phasen mit Zeitraum, Entscheidungen und Status-Abweichungen, aktuelle Phase mit Schritt, Owner und Tagen, nächste Phase, der Rest in einer Zeile. Ein wachsender Verlauf verschiebt nichts darüber. Rechts Classification (Step, Area, Type) und die Beschreibung (scrollt), darunter immer an derselben Stelle die Bewertungskarte (nur mit `assessment:` in `process.yml`: Quadrant- oder K.-o.-Pill, „Value 2.3 · Feasibility 1.7 · 2 open“ (die Anzahl nur mit offenen Punkten), Datum, durchgefallene K.-o.-Fragen (nur mit `ko:`), „Open for pilot“ mit höchstens 5 Zeilen und „+n“, Notiz, „Assess…“ / „Re-assess…“; sonst „Not assessed“) und die Aufwandskarte mit „Edit…“. Seltene Aktionen (Use case abhaken, Project an/aus) im Edit-Formular. Tab **Tasks**: Board nur mit den Tasks des Projekts, Layout wie das Tasks-Board, neue Tasks landen im Projekt. Tab **Files**: Übersicht, Ordnerbaum, Markdown-Vorschau, öffnen in Obsidian/VS Code/Finder, Repos verknüpfen, Bearbeiten | Alle Links öffnen das richtige Ziel; Wechsel zwischen Tasks-Board und Projekt-Tab verschiebt nichts im Layout |
| Tasks | Board, Gruppierung nach Projekt, Filter, Suche, Drag & Drop, Schnelleingabe, bearbeiten, löschen. ⌘N öffnet immer das volle Formular: Titel, Projekt, Priorität, Due, Parent, Beschreibung, Tags, Acceptance-Liste (Enter fügt eine Zeile an, Backspace in leerer Zeile löscht sie), Agent-Felder eingeklappt (nur mit Agent-Funktionen); alle Felder in einer Label-Spalte, gleich hoch; Enter im Titel oder ⌘↵ legt an. Karte: „↑ WR-12“ bei Kindern, `waiting_for` in `--w-warn`, „☑ 2/5“ bei Kriterien | Jede Änderung landet verlustfrei in der Datei |
| Detail card | Popup oder Seitenleiste (schließt mit Esc, ✕ oder einem Klick außerhalb; beide liegen über einem grauen Overlay, ein Klick daneben schließt nur und öffnet nichts), Status, Priorität, Fälligkeit, Tags, Beschreibung, Acceptance als Checkboxen mit „2/5“ und „+ Add criterion“ (alte Tasks ohne Abschnitte: „Add sections“), „Waiting for“ (in Waiting), Parent (Eingabe mit Vorschlägen, fehlt bei Tasks mit Kindern; Link zum Parent), bei Tasks ohne Parent „Sub-tasks · 3/7 done“ mit Status je Kind und „+ Add sub-task“ (Schnelleingabe mit Parent und Projekt), Updates | Wie im Design System |
| Focus | Streifen mit bis zu drei Tasks, ein- und ausblendbar | `focus`, `focus_order` gesetzt |
| Use-case cockpit | Liste (sortierbare Tabelle, Standard; Spalte „Assessment“ sortiert Quick win, Big bet, Fill-in, Drop, K.O., Incomplete, unbewertet immer zuletzt), Board, Prozesslandkarte und Matrix (nur mit `assessment:` in `process.yml`, sonst kein Knopf: 2×2-Feld, x = Feasibility 1–3 nach rechts leichter, y = Value 1–3 nach oben mehr, Mittellinien bei 2,0, Quadrantnamen in den Ecken; ein Punkt pro bewertetem Use Case in seiner Projektfarbe, daneben ein Label mit Key und Titel – kollidierende Labels weichen aus und behalten eine Linie zum Punkt; Tooltip mit Titel, FTE, Rechenweg je Achse und offenen Punkten, Klick öffnet die Detailkarte; rechts eine Liste nach Quadranten (Anzahl, Σ FTE; je Use Case V/F, Schritt, offene Punkte), so hoch wie das Feld und intern scrollend, Hover verbindet Liste und Punkt; ein Stift an jeder Listenzeile und jedem Chip öffnet „Assess…“ direkt über der Matrix, nach dem Speichern bleibt die Matrix offen und der Punkt springt; „How is this calculated?“ unter dem Feld zeigt beim Hover die Methode aus `process.yml` (Kriterien je Achse, Durchschnitt ohne Gewichte, Grenze 2,0 mit Quadrant-Tabelle, FTE-Schwellen abgeleiteter Kriterien, K.-o.-Fragen nur mit `ko:`); darunter die Zeilen „K.O. (n)“ (nur mit `ko:`) und „Not assessed (n)“ mit Chips, Incomplete steht bei Not assessed; Schalter Standard | Advanced links neben „Details“: Advanced zeigt Blasengröße nach FTE, Farbe nach Quadrant, gestrichelt = noch in der ersten Phase; dieselben Filter wie die Liste; kein Drag, Punkte gibt es nur im Formular), Detailkarte als leichte Version der Use-Case-Seite (Popup `--w-uc-detail-w` breit, mindestens `--w-uc-detail-min-h` hoch, oder Seitenleiste; Hauptspalte: Blocked by, Current state, Next step; Seitenspalte: Classification, Assessment (nur die Pill, Tooltip mit Value/Feasibility und offenen Punkten), Manual effort (nur Summe), Tasks, Entscheidungen; bearbeitbar sind Titel, Status, Texte, Classification, Task-Häkchen und Gate-Buttons; keine Beschreibung; „Open page ↗“ im Kopf neben ✕ oder ⌘↩ führt zum Tab Use case), Filter, Knopf „Edit process“ (öffnet `process.yml` in VS Code) | Wie im Prototyp, Prozess aus `process.yml`. Areas sind Freitext mit Vorschlägen (`process.yml`-Areas ∪ von Use Cases benutzte Areas), der Core prüft sie nicht gegen die Liste. Board nach Area zeigt nur Areas mit Use Cases (Reihenfolge aus `process.yml`) |
| Agents | CLI `wly`, Agent-Dateien, Schalter, Live-Status, Ansicht „Agents“ (Overview, Activity, Setup; B12), „Start agent“ (Copilot per `code chat` in VS Code, Codex im Terminal; ein Agent pro Repo) mit Agent-Menü aus `agents:` (B10, Editor seit B12 in Agents → Setup) und „Copy prompt“ in der Task-Karte | Ein Agent setzt einen Task über `wly` auf Review; Schalter aus zeigt keine Agent-Oberfläche |
| Log | Jede Änderung in `.workly/log/` | |

**Nicht in v1:** Queue-Runner, Wochenreview, Jira/SharePoint, Markdown-Editor, Geräte-Sync, Mobile.

**Nicht-funktional:** Start < 1 s bei einigen hundert Dateien, keine CPU-Last im Leerlauf, externe Änderungen < 1 s sichtbar, hell und dunkel, Fenster beim ersten Start ca. 80 % der Arbeitsfläche des Monitors in logischen Punkten (mindestens 1100×700, höchstens die Arbeitsfläche, zentriert), danach gemerkt (Größe, Position, maximiert); Mindestgröße 960×600; Scrollbalken nur bei echtem Überlauf; Tabellen, Boards und Prozesskarte scrollen nur waagrecht, senkrecht scrollt dort die Seite; Kürzel ⌘N (Schnelleingabe), ⌘K (Suche), ⌘1–4 (Ansichten).

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
| – | Probebetrieb, 1–2 Wochen echter Einsatz | Entschieden 2026-10: M7 wird gebaut, neu zugeschnitten als B9 (`docs/BACKLOG-2026-10.md`) |
| M7 = B9 | „Start agent“: Copilot per `code chat` in VS Code, Codex im Terminal, Live-Status | Ein Klick führt vom Task zum laufenden Agent, Status kommt zurück |

**Reihenfolge:** Der Agent-Start (M7) kam erst nach dem Probebetrieb. Dort war der Wunsch da, er wurde im Oktober 2026 als B9 gebaut: Übergabe an VS Code und Terminal statt eigener Prozessüberwachung.
