# 007 — Anhänge, Skills und Hintergrund: gemischter Transport, Skills aus dem Dateisystem, Claudes eigener Scratchpad

**Status:** angenommen · **Datum:** 2026-09-29

## Kontext

Meilenstein 2b ergänzt die Eingabeleiste um Anhänge (Knopf `+`, Hineinziehen, Ctrl+V) und ein `/`-Menü für Skills und Befehle, dasselbe im Aufgabenfeld unter „Neue Session“. Die Session-Kopfzeile bekommt ein Hintergrund-Panel mit laufenden und ausgeführten Befehlen, Subagenten und dem Scratchpad-Ordner ([PROJECT.md](../PROJECT.md), [Entwurf](../design/2026-09-28_hauptansichten/README.md), Tafeln `Attach`, `Cmd`, `Slash`, `NewSession`, `BgProc`, `BgAgents`, `BgScratch`). Alle Protokoll-Fakten sind in [claude-stream-json.md](../knowledge/claude-stream-json.md) belegt (Abschnitte „Anhänge“ bis „Scratchpad“). Randbedingungen: Der Agent sieht nur seinen Workspace (AGENTS.md, Regel 5); was die App anzeigt, stammt aus den Ereignissen der Kommandozeile und den Dateien, die sie schreibt, nie aus dem Text des Agenten (Regel 2); „Neue Session“ hat noch keinen Workspace.

## Optionen

- **Transport der Anhänge:** (a) alles als Pfad im Text; (b) alles als Inhaltsblock; (c) gemischt — Bilder und PDFs als Block, der Rest als Pfad.
- **Ablage vor dem Senden:** (a) direkt in den Workspace; (b) Zwischenordner im Benutzerordner der App, beim Senden in den Workspace verschieben.
- **Quelle der Skill-Liste:** (a) `system/init.skills` der Kommandozeile; (b) das Dateisystem (`.claude\skills`, `.claude\commands`).
- **Scratchpad:** (a) eigener Ordner im Workspace, dem Agenten per Systemprompt-Zusatz genannt; (b) Claudes eigener Ordner aus `system/init.scratchpad_path`.
- **Aktualisieren des Hintergrund-Panels:** (a) Dateisystem-Beobachter; (b) Ereignis `background://changed` plus Nachladen im Takt, solange sichtbar.

## Entscheidung

### Anhänge

- **(b) Zwischenordner.** Jeder Anhang wird sofort nach `<Benutzerordner>\.verwalter\attachments\<id>\<name>` kopiert (`<id>` = UUID v4). So braucht „Neue Session“ keinen Workspace, und die Oberfläche zeigt sofort ein Vorschaubild. Der App-Start leert den Zwischenordner, denn ungesendete Entwürfe leben nur im flüchtigen Zustand der Oberfläche.
- **Beim Senden** verschiebt der Core jeden Anhang nach `<Workspace>\.anhaenge\<id>\<name>`: innerhalb der Sicherheitsgrenze, lesbar für den Agenten, nach einem Neustart weiter im Verlauf sichtbar. `rename` scheitert unter Windows über Laufwerksgrenzen; dann kopiert der Core und löscht die Quelle.
- **(c) Gemischter Transport.** Bilder (`png`, `jpg`, `jpeg`, `gif`, `webp`) bis 3 932 160 Bytes gehen als Base64-Bildblock — Base64 wächst um 4/3, die API nimmt je Bild höchstens 5 MB. PDFs bis 10 485 760 Bytes gehen als Base64-Dokumentblock; das hält die stdin-Zeile unter rund 14 MB, längere Zeilen sind ungeprüft. Alles andere und alles darüber nennt der Text unter `Angehängte Dateien (im Workspace):` als absoluten Pfad. Ohne Anhänge bleibt die Nachricht ein String. Der Textblock steht immer zuerst, weil ein `/skill` nur im ersten Block wirkt.
- **Anhang-IDs aus der Oberfläche** werden als UUID geparst, bevor sie in einen Pfad eingehen.
- **Während einer offenen Rückfrage keine Anhänge:** Die Nachricht beantwortet dann die Rückfrage als Text. Die Oberfläche sperrt `+`, Hineinziehen und Einfügen; der Core lehnt zusätzlich mit `attachmentsWhileWaiting` ab.
- **Asset-Protokoll** für Vorschaubilder: Scope `$HOME/.verwalter/**`, CSP `img-src` mit `http://asset.localhost`. Scratchpad-Ordner werden einzeln zur Laufzeit freigegeben, nie ganz `%TEMP%`.

### Skills und Befehle

- **(b) Dateisystem.** `init.skills` enthält nur Namen ohne Herkunft und Beschreibung und ist erst bekannt, nachdem der Agent einmal lief; „Neue Session“ braucht die Liste vorher.
- **Vier Orte, in dieser Reihenfolge:** `~\.claude\skills\<ordner>\SKILL.md`, `~\.claude\commands\<datei>.md` (nur oberste Ebene), dann je Repository der Session `<wurzel>\.claude\skills\…` und `<wurzel>\.claude\commands\…`. `<wurzel>` ist der Worktree, unter „Neue Session“ der Haupt-Checkout.
- **Name und Beschreibung** aus dem Frontmatter (`name`, `description`); ersatzweise Ordner- bzw. Dateiname und bei Befehlen die erste Textzeile. Beschreibungen werden auf 160 Zeichen gekürzt. Doppelte Namen bleiben beide stehen, die Herkunft unterscheidet sie.
- **Plugin- und eingebaute Skills stehen nicht im Menü** — abgelöst durch ADR 034. Tippen funktioniert trotzdem, die Kommandozeile löst jedes `/name` selbst auf.
- **Skill-Marke im Verlauf:** Beginnt eine gesendete Nachricht mit `/<name>` aus der Liste der Session, speichert der Core Name und Herkunft am Chat-Eintrag. Die Kommandozeile meldet das Laden nicht selbst.
- **Session-Befehle** (`/rename`, `/changes`, `/pause`) führt die App selbst aus; sie gehen nie an den Agenten.

### Hintergrund

- **Drei Arten:** `command` (Bash im Vordergrund), `process` (Bash mit `run_in_background`), `subagent` (Werkzeug `Agent`). Befehle von Subagenten sind Schritte ihres Subagenten, keine eigenen Einträge.
- **Zustände:** `running`, `completed`, `failed`, `stopped` (von der App angehalten), `interrupted` (der Agent-Prozess endete, während der Eintrag lief — er kann trotzdem weitergelaufen sein).
- **Ausgabe:** `command` aus dem `tool_result` (höchstens 64 KiB, in SQLite); `process` aus Claudes Ausgabedatei (höchstens die letzten 256 KiB). Die Liste lädt nie Ausgaben mit.
- **Anhalten** über die Steueranfrage `stop_task`. „Neu starten“ und „Erneut ausführen“ entfallen: Die Kommandozeile bietet nichts dafür, und die App führt keine Befehle selbst aus.
- **(b) Claudes eigener Scratchpad** — abgelöst durch [ADR 022](022-scratchpad-im-workspace.md). `system/init.scratchpad_path` ist belegt und bleibt über `--resume` gleich; ein eigener Ordner bräuchte einen Systemprompt-Zusatz, auf den sich der Agent nicht verlassen muss. Der Core speichert den Pfad an der Session und gibt ihn für das Asset-Protokoll frei.
- **Selbstständiges Aufwachen:** Kommt Ausgabe des Hauptagenten, während die Session `completed` oder `paused` ist und weder Pause noch Abbruch angefordert ist, wechselt sie auf `running`.
- **Ruhe-Timer:** Kein Agent wird beendet, solange einer seiner Einträge läuft.
- **(b) Ereignis plus Nachladen.** Begründung wie [ADR 006](006-changes-und-diff.md): ein Beobachter kostet auch, wenn niemand hinsieht.

## Konsequenzen

- Ein Bild über 3,75 MiB oder ein PDF über 10 MiB liest der Agent über den Pfad selbst, mit seinen Werkzeugen.
- Ungesendete Anhänge überleben keinen Neustart.
- Der Workspace jeder Session mit Anhängen bekommt einen Ordner `.anhaenge`.
- Ob Bild- und PDF-Blöcke nach `--resume` im Verlauf des Agenten erhalten bleiben, ist ungeprüft; die Smoke-Abnahme prüft es.
- Plugin- und eingebaute Skills fehlen im `/`-Menü.
- Ein Wechsel des Denkaufwands startet den Agenten neu und unterbricht laufende Hintergrundprozesse; M2b schützt nicht davor.
- Ob ein angehaltener oder unterbrochener Dev-Server als verwaister Prozess weiterläuft, prüft die Smoke-Abnahme.
