# Workly Design System

Richtung D („Synthese“): die Struktur und Typografie des Leitstands, die Weichheit einer nativen Mac-App. Ruhige, getönte Flächen tragen den Inhalt; Farbe bedeutet immer etwas.

Dateien: `tokens.css` (alle Werte, hell und dunkel), `components.css` (Referenz-Umsetzung der Komponenten), `reference/task-board.html` und `reference/use-case-cockpit.html` (lauffähige Referenzen).

## 1. Prinzipien

1. **Eine Akzentfarbe.** Petrol steht nur für aktiv, ausgewählt oder primär. Nie als Dekoration.
2. **Farbe ist Bedeutung.** Rot = blockiert oder P1, Orange = wartet, P2 oder überfällig, Blau = in Arbeit, Grün = erledigt oder stabil, Grau = on hold. Projektfarben gibt es nur als kleines Quadrat.
3. **Flächen statt Rahmen.** Gruppen sind getönte Ablagen (`--w-tray`), Inhalte sind weiße Karten mit Schatten. Rahmen nur, wenn ein Zustand sie braucht (blockiert, on hold, Drop-Ziel).
4. **Feste Anatomie.** Gleiche Elemente stehen immer an derselben Stelle. Das Auge lernt die Karte einmal.
5. **Mono für alles, was man vergleicht.** IDs, Zählungen, Daten, Commits, Tage.
6. **Leere Bereiche bleiben ruhig.** Keine Platzhalter-Grafiken, keine „No tasks“-Texte in jeder Spalte. Eine leere Ablage ist Information genug.

## 2. Tokens

Alle Werte stehen in `tokens.css`. Präfix `--w-`. Die App übernimmt diese Datei 1:1 (oder überträgt sie in ihr Theme-System) und verwendet im Code **nur Tokens, keine Hex-Werte**.

| Gruppe | Tokens | Verwendung |
| --- | --- | --- |
| Flächen | `bg`, `tray`, `surface`, `sunk`, `line`, `edge` | App-Grund, Ablagen und Sidebar, Karten, Einsätze in Karten, Haarlinien, Diagramm-Kanten |
| Text | `ink`, `muted` | Muted erfüllt 4,5:1 auf `surface` und `tray` |
| Akzent | `accent`, `accent-hover`, `accent-soft`, `on-accent` | Primär-Button, aktive Navigation, Agent-Aktivität, Fokus-Nummern |
| Semantik | `info`, `warn`, `danger`, `danger-soft`, `hold`, `ok` | Status, Priorität, Alter |
| Projekte | `proj-1` … `proj-6` | Reihum vergeben, Farbe im Projekt-Frontmatter speichern |
| Bahnen | `lane-fb`, `lane-me`, `lane-gov`, `lane-it` | nur Prozesslandkarte |
| Höhe | `shadow-card`, `shadow-raised`, `shadow-panel`, `shadow-pop`, `ring-active` | Karte, Pille/Segment, Panel, Modal/Drag, aktive Karte |
| Form | `r-sm` 5, `r-md` 9, `r-lg` 12, `r-xl` 14, `r-pill` | Tag, Karte/Button, Ablage/Panel, Modal |
| Abstand | `s-1` … `s-8` (4 bis 32 px) | Abstände per `gap`, nicht per Margin |

### Typografie

| Rolle | Schrift | Größe / Gewicht |
| --- | --- | --- |
| Seitentitel | IBM Plex Sans | 28 / 600 |
| Spalten- und Abschnittsköpfe | IBM Plex Sans | 15 / 600 |
| Caps-Label („FOKUS HEUTE“, „PROJEKTE“) | IBM Plex Sans | 12 / 600, Versalien, +0,06em |
| Fließtext, Kartentitel | IBM Plex Sans | 14 / 400, Kartentitel 500 |
| Bedienelemente, Untertitel | IBM Plex Sans | 13 |
| Meta | IBM Plex Sans | 12 |
| IDs, Zahlen, Daten | IBM Plex Mono | 11, `tabular-nums` |

Schriften lokal bündeln (z. B. `@fontsource/ibm-plex-sans` und `@fontsource/ibm-plex-mono`), damit die App offline gleich aussieht.

### Marke

App-Icon und Marke liegen in `icon/`: `workly-icon.svg` und `workly-icon-1024.png` (macOS-Raster: Fläche 824 px auf 1024, Rand und Schatten sind enthalten) sowie `workly-mark.svg` (nur die Raute mit W, für Sidebar und About). Die Raute mit dem W ist das einzige Logo; nicht nachzeichnen, nicht umfärben.

## 3. Layout

- **App-Rahmen:** Sidebar 236 px (`tray`, keine Trennlinie) + Inhalt (`bg`, Innenabstand 26/30 px).
- **Sidebar:** Marke, Hauptnavigation (Heute, Tasks, Use Cases, Wochenreview), Projektliste mit Farbquadrat und offener Anzahl, unten der Workspace-Pfad in Mono. Aktiver Eintrag = weiße Pille mit `shadow-raised`.
- **Seitenkopf:** Titel + Unterzeile mit Zählung links, Toolbar rechts (Suche, Segment-Umschalter, sekundäre Buttons, Primär-Button ganz rechts).
- **Board:** Spalten als Ablagen, `gap` 12, Mindestbreite 216 px je Spalte, horizontal scrollbar im eigenen Container.

## 4. Komponenten

### Task-Karte

```
┌───────────────────────────┐
│ T-031                 P1  │  Meta: ID (Mono) · Priorität rechts (Mono, farbig)
│ Azure Launch vorbereiten  │  Titel 14/500, umbricht, nie abgeschnitten
│ #pilot  #brainstorming    │  optional: Tags (Mono)
│ ● Codex arbeitet · 14 min │  optional: Agent-Aktivität (Akzent)
│ commit a3f9c1 · Codex     │  optional: Commit (Mono auf sunk)
│ ■ Operations Lighthouse 10.10. │  Fuß: Projekt (Quadrat + Name) · Fälligkeit rechts
└───────────────────────────┘
```

| Zustand | Darstellung |
| --- | --- |
| normal | `surface`, `shadow-card` |
| Agent arbeitet daran | `ring-active` (Akzent-Ring) |
| erledigt | halbtransparent, kein Schatten, Titel durchgestrichen, Datum in der Meta-Zeile |
| blockiert | `danger-soft` + 1 px `danger` innen |
| on hold | transparent + 1 px `line` innen, Titel muted |
| gefiltert | Deckkraft 0,2 (nicht ausblenden, damit die Position sichtbar bleibt) |
| beim Ziehen | Original 0,3, Geist mit `shadow-pop`, 1,5° gedreht |

In der Gruppierung nach Projekt entfällt die Projektzeile im Fuß (steht schon im Gruppenkopf).

### Spalte / Ablage

Kopf: Name (15/600) und Zahl rechts (Mono). WIP-Limit als `2 / 3`. „Erledigt“ mit muted Kopf. Drop-Ziel: 2 px Akzent-Innenring.

### Gruppierung nach Projekt

Spaltenköpfe einmal oben. Je Projekt eine Ablage mit Kopf (Farbquadrat, Name, „n offen“) und fünf Zellen. Umschaltung über Segment „Nach Status / Nach Projekt“.

### Fokus-Streifen

Weißes Panel über dem Board, eine Zeile: Caps-Label, drei nummerierte Einträge (Reihenfolge = Priorität, Nummer in Mono und Akzent, Projektquadrat, Titel einzeilig mit Auslassung; ID, Priorität, Fälligkeit und Projekt im Tooltip), „Ausblenden“ rechts; ausgeblendet erscheint in der Toolbar „Fokus einblenden · 3“. Zustand als UI-Präferenz speichern. Befüllung: Tasks mit `focus: <heutiges Datum>` im Frontmatter, Reihenfolge über `focus_order`.

Der Streifen hat eine feste Höhe (`--w-bar-h`), leer wie voll. In der Projektansicht steht an seiner Stelle eine gleich hohe Leiste (`.w-bar`) mit den Tabs „Tasks | Files“, damit Board und Inhalt in jeder Ansicht auf derselben Höhe beginnen.

### Steuerelemente

- **Primär-Button:** Akzent, 9 px Radius, eine pro Ansicht.
- **Sekundär-Button:** `surface` + `shadow-raised`.
- **Leiser Button:** ohne Fläche, muted, Hover `tray`.
- **Segment:** Ablage `tray`, gewähltes Segment `surface` + `shadow-raised`.
- **Suche:** Feld auf `tray`, Lupe in muted, Fokus als `accent-soft`-Ring.
- **Filter-Chips:** `surface`-Pillen, gewählt = 1,5 px `ink`-Innenring. Status-Chips mit Punkt in Statusfarbe.

### Detailkarte (Modal)

Radius 14, `shadow-pop`, Hintergrund-Abdunklung `scrim`. Aufbau wie im Cockpit: Kopf (ID, Typ, Bereich, Schließen), Titel, Position/Fortschritt, Status-Pillen, Inhalt zweispaltig, Aktionsleiste unten auf `sunk`. Die Task-Karte ist als Popup `--w-detail-w` breit und mindestens `--w-detail-min-h` hoch, damit Beschreibung und Updates auch bei kurzen Tasks Platz haben; als Seitenleiste `--w-panel-w` breit (auch Use Cases). Fokus setzt ein Stern im Kopf (gefüllt = im Fokus, Tooltip „Focus #n“).

### Gate-Marker

Raute (10 px, 45°, Akzent-Rand, `accent-soft` Fläche) + Code in Mono + Name 12/600. Auf dem Board an der rechten Spaltenkante, in der Prozesslandkarte als große Raute.

## 5. Dark Mode

Alle Tokens haben einen dunklen Wert. Die App folgt dem System, `data-theme="light|dark"` am Root überschreibt. Komponenten verwenden nur Tokens; nichts wird nur für einen Modus gestylt.

## 6. Bewegung

160 ms, `cubic-bezier(.2,.7,.2,1)` für Hover und Zustände. Modal skaliert von 98 % ein. Bei `prefers-reduced-motion` keine Animation.

## 7. Barrierefreiheit

- Fokusrahmen: 2 px Akzent, 2 px Abstand.
- Karten sind per Tastatur erreichbar; Enter öffnet die Details.
- Farbe nie als einziges Signal: Status hat immer Punkt und Text, Priorität immer `P1`/`P2`.
- Mindest-Klickfläche 28 px in der Desktop-App.
