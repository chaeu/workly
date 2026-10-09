# Workly – Probleme direkt beheben

Stand: 9. Oktober 2026. Neue Funktion: Die Seite Problems sagt nicht nur, was Workly nicht lesen kann, sondern auch, wie man es behebt, und behebt eindeutige Fälle mit einem Klick.

Eine Session, Commit-Präfix `PB1:`.

## Warum

Bestehende Dateien veralten auf zwei Wegen, und beide kommen wieder:

1. **Workspace-Upgrades.** Ein neues Feature erweitert `process.yml` oder `config.yml`. `init_workspace` legt nur fehlende Dateien an und überschreibt nie. Ein älterer Workspace bekommt das Feature also nie. Beispiel: `process.yml` ohne `assessment:` = keine Bewertung, auch nicht bei neuen Use Cases.
2. **Handarbeit.** Edits in Obsidian oder von Agents: `priority: "1"` statt `1`, `priority: ""`, Dateien ohne `key`. Ein falsches Feld lässt die ganze Datei ausfallen.

**Ablauf für den Nutzer:** Problems öffnen → lesen, was falsch ist und wie man es behebt → Fix-Knopf oder Papierkorb.

## Entscheidungen

| # | Entscheidung |
|---|---|
| D1 | Der Core berechnet Hinweis und Fix beim Scannen. Das Frontend zeigt sie nur an. |
| D2 | Ein Fix wird nicht aus dem serde-Fehlertext abgeleitet, sondern aus den rohen Feldern (`patch::get_field`). Der Hinweis darf am Fehlertext hängen, er ist nur Anzeige. |
| D3 | Der Fix-Command bekommt nur den Pfad. Der Core liest die Datei neu, berechnet den Fix neu und wendet genau den an. Das Frontend kann kein Feld und keinen Wert vorgeben. |
| D4 | Ein Fix ändert ein Feld oder hängt einen Block an, mehr nicht. Er schreibt atomar und legt eine Logzeile an (`actor: app`). Keine Rückfrage. |
| D5 | **`process.yml` ohne `assessment:` ist ein Problem** mit dem Fix „Add default assessment“. Der Block kommt aus der eingebauten Vorlage (Fixture) und wird ans Dateiende angehängt. Das löst ASSESSMENT.md D1 ab: Absichtlich ausblenden geht nicht mehr ohne diese Meldung. |
| D6 | Der Papierkorb ist der Notausgang für alles ohne eindeutigen Fix. Bei einer kaputten `_project.md` geht der ganze Ordner, sonst nur die Datei. Rückfrage ja, wiederherstellbar über den bestehenden Trash. Nie für Dateien unter `.workly/`. |
| D7 | Nicht gebaut: Ignorieren (ein ignoriertes Problem = Task fehlt still), Reset einer Datei auf die Vorlage (widerspricht verlustfreiem Schreiben), Fix für fehlenden `key` (die Wahl trifft der Nutzer), Fix für doppelte IDs (Umbenennen betrifft Verweise), „Fix all“. |

## Datenmodell

`ParseError` (`crates/workly-core/src/model.rs`) bekommt drei optionale Felder. Alle bestehenden Stellen setzen sie auf `None`.

```rust
pub struct ParseError {
    pub path: String,
    pub line: Option<usize>,
    pub message: String,       // unchanged: raw message
    pub hint: Option<String>,  // what is wrong and how to fix it by hand
    pub fix: Option<String>,   // button label; None = no safe fix
    pub trash: Option<String>, // what "Move to trash" moves: the file or the project folder
}
```

## Fixes

| Fall | Erkennung | `fix` | Wirkung |
|---|---|---|---|
| Zahl als Text | Task: `priority`, `focus_order`, `order`; Projekt: `order`. Roher Wert ist ein String, der getrimmt als ganze Zahl in den Feldtyp passt (`u8` bzw. `i64`) | `Set priority to 1` | `patch::set_field` mit der Zahl |
| Leerer Wert | dieselben Felder, String leer oder nur Leerzeichen | `Remove empty priority` | `patch::remove_field` |
| Kein Bewertungsblock | `process.yml` lädt, `assessment` ist `None` | `Add default assessment` | Block `assessment:` aus `fixtures/workspace/.workly/process.yml` (inkl. Kommentar darüber) ans Dateiende anhängen, mit genau einer Leerzeile davor. Der Rest der Datei bleibt byte-identisch. |

Hat eine Datei mehrere solche Felder, repariert ein Klick das erste. Der nächste Scan zeigt das nächste.

## Hinweise

Nur Anzeige, auf Englisch wie alle UI-Texte:

| Meldung | `hint` |
|---|---|
| `missing frontmatter` | No frontmatter (`---` block at the top). If this is not a Workly file, move it to trash or add its folder to `scan_exclude` in `.workly/config.yml`. |
| `missing field \`key\`` (Projekt) | Every project needs `key:` with 2–6 capital letters or digits, e.g. `key: OPS`. |
| `missing field \`<f>\`` (sonst) | Add `<f>:` to the frontmatter. |
| falscher Typ bei einem Fix-Feld | `<f>` must be a whole number or left out. |
| `duplicate …` | Two files use the same ID. Change it in one of them, or move one to trash. |
| `process.yml` ohne `assessment:` | Use cases cannot be assessed without an assessment method. |

## Core-API

In einer neuen Datei `crates/workly-core/src/fix.rs`:

- `suggest_fix(rel_path, src) -> Option<(label, Edit)>`: wird von `scan` beim Fehlschlag eines Tasks/Projekts aufgerufen und von `rescan` für `process.yml`.
- `Workspace::fix_problem(path, actor) -> Result<()>`: Pfad über `resolve` prüfen (bleibt im Workspace), Datei lesen, `suggest_fix` neu rechnen. `None` → `Error::Invalid("nothing to fix")`. Sonst schreiben, loggen (`kind: "problem.fix"`, `field` = Feldname bzw. `assessment`), `rescan`.
- `Workspace::trash_problem(path, actor) -> Result<()>`: Pfad unter `.workly/` → Fehler. `…/_project.md` → Ordner, sonst die Datei, nach `.workly/trash/<gleicher Pfad>`. Gleiche Logik wie `delete_project` / `delete_task` (inkl. `merge_into`), aber pfadbasiert, weil kaputte Dateien keinen Key/keine ID haben. `delete_project` und `delete_task` rufen danach dieselbe Hilfsfunktion. Log `kind: "problem.trash"`.

`src-tauri`: zwei Commands `fix_problem(path)` und `trash_problem(path)`, jeweils über `changed(...)` wie die anderen schreibenden Commands. Die CLI bekommt nichts.

## UI (`src/routes/problems/+page.svelte`)

```
pm/projects/sonstiges/tasks/task-1791…000.md:6           [Remove empty priority] [Open in VS Code] [Finder] [Trash]
priority: invalid type: string "", expected u8 at line 5 column 11
priority must be a whole number or left out.
```

- Zeile 1 Pfad, Zeile 2 Meldung (gedämpft wie heute), Zeile 3 `hint` in `--w-ink`.
- Fix-Knopf nur mit `fix`, primär, ganz vorn. Klick schreibt sofort.
- Trash-Knopf nur mit `trash`, leise. Rückfrage mit `confirm`-Dialog wie beim Löschen eines Tasks: „Move <trash> to trash?“, bei einem Ordner mit „with n tasks“.
- Fehler eines Commands rot in der Zeile (wie heute `cliError`).
- Untertitel: „Workly skips what it cannot read and keeps going. Fix it here or in the file; this list updates on its own.“
- Nur Tokens, keine neue Komponente.

## Tests (Core, nur auf Tempfile-Kopien der Fixture)

1. `priority: "1"` → Problem mit `fix = "Set priority to 1"`; `fix_problem` → Task lädt, Diff ist genau eine Zeile. Dasselbe gegen WR-8 (Regel 2): dort `priority` kaputt machen, reparieren, Datei byte-identisch zum Original.
2. `priority: ""` → `Remove empty priority`; danach fehlt nur diese Zeile.
3. Projekt mit `order: "2"` → `Set order to 2` (ein Projektfall reicht).
4. `process.yml` ohne `assessment:` → Problem; `fix_problem` → Präfix der Datei byte-identisch, `process.assessment` gesetzt, Problem weg.
5. `fix_problem` auf einer gesunden Datei → Fehler, Datei unverändert, keine Logzeile.
6. `trash_problem`: kaputte `_project.md` → Ordner im Trash; Task → nur Datei; `.workly/process.yml` → Fehler.
7. Jeder Fix und jeder Trash schreibt genau eine Logzeile.
8. `delete_task` / `delete_project` laufen nach dem Umbau weiter (bestehende Tests).

## Checkliste (manuell)

- `pnpm dev:fixture`, in `/tmp/workly-dev` einen Task auf `priority: "2"` setzen → Problem mit Knopf, Klick → Task wieder auf dem Board, `git diff --no-index` zeigt eine Zeile.
- `assessment:`-Block aus `/tmp/workly-dev/.workly/process.yml` löschen → Problem, Klick → Matrix und Assess wieder da.
- Ordner mit kaputter `_project.md` anlegen → Trash mit Rückfrage → im Trash sichtbar, wiederherstellbar.
- Hell und dunkel, keine Konsolenfehler.
