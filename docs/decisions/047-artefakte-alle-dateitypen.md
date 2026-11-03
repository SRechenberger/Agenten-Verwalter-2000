# 047 — Artefakte aller Dateitypen: Typ-Erkennung, Betrachter, Gruppierung nach Session

**Status:** angenommen · **Datum:** 2026-10-30 · ergänzt [ADR 026](026-artefakte.md)

## Kontext

Der Reiter „Artefakte“ zeigte nur HTML-Seiten direkt im Ordner `.artefakte`; Unterordner galten stillschweigend als Zubehör, die Session je Datei war die zuletzt schreibende. Agenten legen aber auch Bilder, Markdown, Daten, PDFs, Videos und Ton zum Ansehen ab, und mehrere Sessions eines Vorhabens bearbeiten dieselben Dateien.

**Artefakt (Definition):** eine Datei oder ein Ordner, den ein Agent für den Menschen im Vorhaben ablegt, um einen Sachverhalt zu veranschaulichen oder zum Ausprobieren anzubieten — Bericht, Dokument, Grafik, Diagramm, Bild, Video, Ton, strukturierte Daten. Ein Artefakt ist Anschauungsmaterial, kein Arbeitsergebnis im Repository. Prüfstein: Ist die Datei zum Anschauen gemacht oder für das Programm?

## Optionen

- Darstellung: (a) Inhalt in der App rendern (React) — verworfen: Dateiinhalt des Agenten liefe im Fenster der App mit deren Rechten; (b) Betrachter-Seite im selben abgeschotteten Rahmen wie HTML, ausgeliefert vom Artefakt-Server — gewählt.
- Bibliotheken der Betrachter (Markdown, Mermaid, pdf.js): (a) in die exe einbauen — bläht jede Version um mehrere MB auf; (b) neben die exe legen — das App-Update tauscht nur die exe ([ADR 041](041-app-update.md)); (c) über https von `cdn.jsdelivr.net` mit fester Version und `integrity`-Prüfsumme — gewählt; ohne Internet zeigt der Betrachter den Rohtext.
- Session je Artefakt: (a) zuletzt schreibende (bisher); (b) anlegende — gewählt, die zuletzt schreibende steht zusätzlich in der Meta-Zeile, wenn sie eine andere ist.

## Entscheidung

- **Typ:** `ArtifactKind` (Core, an die Oberfläche als kebab-case). Die einzige Zuordnungstabelle Endung → Typ ist `artifacts::kind_of`; ein Ordner mit `index.html` ist `Site`, sonst `Folder`. Die Oberfläche hält keine eigene Liste: `Artifact.viewable` sagt, ob ein Betrachter existiert.
- **Was ist ein Artefakt:** jede Datei direkt in `.artefakte` außer versteckten (Name mit `.` vorn; `.env*` zählt als Konfiguration und bleibt sichtbar), und jeder Unterordner, der kein Zubehör ist. **Zubehör** ist ein Unterordner, dessen Name mit `_` beginnt, der so heißt wie eine HTML-Datei daneben ohne Endung, oder auf den eine HTML-Datei direkt im Ordner mit `"<name>/`, `'<name>/` oder `"./<name>/` verweist. Die `_`-Regel steht in der Anweisung an den Agenten; die beiden anderen fangen Altbestand.
- **Anlegende Session:** `session_files.first_touched_at` (Migration 17) setzt nur der erste Eintrag einer Session für einen Pfad. Einträge von davor tragen dort ihre damals letzte Uhrzeit, die wahre erste kennt niemand mehr. Anlegend ist die Session mit der kleinsten ersten Uhrzeit. Bei Ordnern zählt jede Datei darin.
- **Betrachter:** `<baseUrl><datei>?view=1` liefert eine Betrachter-Seite (`artifacts/viewers/shell.html` + `<typ>.js`, per `include_str!`) mit Kommentar-Brücke und derselben CSP wie jede Artefakt-Seite; die Seite holt die Rohdatei per `fetch` aus demselben Ordner. Der Dateiname geht als JSON-String (`</` maskiert) und nur aus dem geprüften Ziel in die Seite. Die Weiche ist `viewers::page(kind)`; ein neuer Betrachter ist eine `.js` und eine Zeile. Ohne `?view=1` liefert der Server die Datei selbst, ein Ordner-Ziel `<ordner>/` dessen `index.html`. Ein Ordner ohne `index.html` zeigt `<ordner>/?view=1` als Ordner-Betrachter; seine Dateiliste kommt von `<ordner>/?list=1` (JSON, höchstens 3 Ebenen und 2000 Einträge).
- **Rest-Karte:** Typen ohne Betrachter zeigt die App als Karte mit Typ, Name, Größe, Änderung und den Knöpfen „Mit Standardprogramm öffnen“ und „Im Explorer zeigen“. Sie zeigt nur Angaben des Core, nie Dateiinhalt — keine Agenten-Datei wird im Fenster der App gerendert.
- **Liste:** gruppiert nach anlegender Session (`groupArtifacts`), eigene Session zuerst, dann nach neuestem Artefakt, „Ohne Zuordnung“ zuletzt; Gruppen aufklappbar, Klappzustand flüchtig je Vorhaben.
- **Karte im Chat:** jede Schreibaktion in `.artefakte` oder in einen Nicht-Zubehör-Ordner darin ergibt eine Karte (bei Ordnern für den Ordner) mit Typ-Abzeichen; ältere Karten ohne Typ gelten als HTML.

## Konsequenzen

- Ordnergröße und Dateizahl werden bis zwei Ebenen tief und höchstens 1000 Einträge weit gezählt; ein riesiger Ordner zeigt dann zu kleine Zahlen statt die Liste aufzuhalten.
- Betrachter mit Bibliothek brauchen Internet; ohne zeigen sie Rohtext. Bild, Video und Ton laufen ohne Bibliothek.
- `Artifact.session_*` bedeutet jetzt **anlegende** Session; Leser, die „zuletzt bearbeitet“ meinten, nehmen `last_session_*`.
- Kommentare auf einer Betrachter-Seite tragen Selektoren der Betrachter-Seite (z. B. `img`), nicht des Dateiinhalts.
- PDF rechnet im Hauptthread: ein Worker von fremder Herkunft startet unter der CSP nicht und meldet keinen Fehler. Der Betrachter lädt `pdf.worker.min.mjs` als Modul mit Prüfsumme; sehr große PDFs können die Seite kurz anhalten.
- Kommentare auf Inhalte in der Vorschau des Ordner-Betrachters (zweiter Rahmen) erreichen die App nicht; Ordner-Kommentare gehen nur auf Baumzeilen und Raster-Kacheln.
- „Mit Standardprogramm öffnen“ öffnet bei einer Website `index.html`, bei einem Ordner ohne `index.html` den Ordner im Explorer. Das gilt nur für Dokumente und Medien (Liste in `artifacts/mod.rs`); Programme, Skripte und unbekannte Typen öffnet der Knopf nicht und nennt stattdessen „Im Explorer zeigen“, denn ein Agent darf nichts ablegen, das ein Klick des Benutzers ausführt.
