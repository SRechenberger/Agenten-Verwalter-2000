# 012 — Einstellungen, Farbschema und virtuelle Listen

**Status:** angenommen · **Datum:** 2026-10-01

## Kontext

Meilenstein 6 bringt die App auf das MVP-Zielbild: Die Einstellungsseite nach Tafel `Settings` bekommt ein Farbschema (Dunkel · Hell · System) und Standardwerte für neue Vorhaben. Bis hierher waren diese Werte fest eingebaut: Modell Sonnet, Denkaufwand Hoch, Modus Auto, Farbschema nach Windows. Außerdem rendern die Sidebar, die Listen im Hintergrund-Panel und der Scratchpad-Baum noch jede Zeile, obwohl AGENTS.md Regel 3 verlangt, dass der Speicherbedarf der UI mit dem Sichtbaren wächst. Lade- und Aktionsfehler landen an mehreren Stellen nur in der Entwicklerkonsole.

Der Entwurf zeigt zusätzlich die Zeilen „Branch-Präfix“, „Basis für Changes“ und „Ordner für Worktrees“ (änderbar). Seit [ADR 010](010-worktrees-durch-den-agenten.md) legt die App bei neuen Vorhaben keine Branches und Worktrees mehr an.

## Optionen

- **Speicherort:** (a) Tabelle `settings` mit Schlüssel und Text in der App-Datenbank; (b) eigene JSON-Datei neben der Datenbank; (c) nur `localStorage` der Oberfläche.
- **Farbschema beim Start:** (1) Klasse erst setzen, wenn die Einstellungen aus dem Core geladen sind; (2) zusätzlich ein Spiegel in `localStorage`, den `main.tsx` vor dem ersten Rendern liest.
- **Branch-Präfix:** (i) einstellbar, gilt für neue Vorhaben; (ii) weglassen.
- **Typ-Erzeugung raus aus dem Installer:** (α) `required-features` an der Bin-Definition; (β) `src-tauri/examples/gen-bindings.rs`.
- **Virtuelle Listen:** (A) seitenweises Nachladen der Daten aus dem Core; (B) Daten wie bisher vollständig im Speicher, gerendert wird nur das Sichtbare.
- **Sichtbare Fehler:** Toast, Dialog oder ein Satz an der Stelle, an der das Ergebnis fehlt.

## Entscheidung

- **(a) Tabelle `settings`** (`key TEXT PRIMARY KEY`, `value TEXT NOT NULL`, Migration 006). Die Datenbank ist schon die eine Quelle für alles Persistente (AGENTS.md Regel 3); (b) wäre eine zweite Datei mit eigener Fehlerbehandlung, (c) ginge mit dem WebView-Profil verloren und wäre für den Core unsichtbar. Enums liegen als ihr serde-Text (`enum_to_text`/`enum_from_text`). Schlüssel und Standardwerte: `color_scheme` = `system`, `default_model` = `sonnet`, `default_effort` = `high`, `default_mode` = `auto` — die bisher fest eingebauten Werte. Fehlt ein Schlüssel oder ist sein Text unlesbar, gilt der Standardwert; der unlesbare Text bleibt stehen, bis der Benutzer den Wert ändert. Modus und Denkaufwand werden gemeinsam in einer Transaktion gespeichert, weil sie im selben Menü stehen.
- **Commands:** `settings_load` liefert die Werte samt der Ordner, die die Seite nur anzeigt (`<Benutzerordner>\.claude\skills` mit der Zahl der Benutzer-Skills ohne Befehle, `<Benutzerordner>\.verwalter\workspaces`); `settings_update` speichert eine Änderung und gibt den ganzen neuen Stand zurück.
- **Standardwerte gelten für die erste Session eines neuen Vorhabens.** Weitere Sessions eines Vorhabens übernehmen den Stand der letzten Session ([ADR 011](011-vorhaben-und-sessions.md)); bestehende Sessions ändern sich nie. Seit Taiga 47 nehmen auch neue Sessions in einem bestehenden Vorhaben Modell und Denkaufwand aus den Einstellungen, nur der Modus bleibt geerbt ([ADR 052](052-session-werkzeuge.md), Nachtrag 2026-11-20).
- **(2) Spiegel in `localStorage`** unter `verwalter.colorScheme`. Mit (1) stünde bei jedem Start ein Frame im falschen Schema. Nach dem Laden gewinnt der Wert aus der Datenbank und überschreibt den Spiegel. `theme.css` kennt drei Zustände (ohne Klasse = System, `:root.dark`, `:root.light`); die App setzt nur die Klasse an `<html>` und über `getCurrentWindow().setTheme(…)` das Schema der Windows-Titelleiste (`null` = System).
- **(ii) kein Branch-Präfix.** Seit ADR 010 legt die App bei neuen Vorhaben keine Branches an; Branch-Namen bestimmt das Regelwerk des Agenten. Ein Präfix-Feld hätte keine Wirkung. Ebenfalls weggelassen: „Basis für Changes“ — Changes vergleicht gegen den Base ref je Repository ([ADR 006](006-changes-und-diff.md)), ein globales Feld widerspräche dem. Der Ordner der Workspaces wird nur angezeigt, nicht geändert: die Asset-Freigabe `$HOME/.verwalter/**` und die Sicherheitsgrenze bleiben unberührt. „Im Explorer zeigen“ nutzt `revealItemInDir`, das `opener:default` schon erlaubt; `openPath` bräuchte eine eigene Freigabe mit Pfad-Scope.
- **(β) `examples/gen-bindings.rs`**, Aufruf `cargo run --example gen-bindings`. Tauri bündelt nur Programme mit Ziel-Art `bin`; `cargo clippy --all-targets` prüft Beispiele weiter mit. Gegen (α): Clippy mit `--all-targets` überspringt die Bin dann still, solange nicht auch `--all-features` gesetzt ist.
- **(B) virtualisiert gerendert, Daten im Speicher.** Virtualisiert werden der Baum der Sidebar, die Listen „Prozesse“ und „Subagenten“ und der Scratchpad-Baum (bis 2000 Einträge), mit `@tanstack/react-virtual` nach dem Muster von `FileTree` + `buildFileRows` ([ADR 002](002-typgenerierung-und-listen.md)): Gruppen werden zu einer flachen Zeilenliste, die Zeilenhöhe misst `measureElement`. Die gehaltenen Objekte sind klein (Zusammenfassungen, Hintergrund-Einträge ohne Ausgaben); (A) wäre eine eigene Baustelle mit Cursor-Logik in jeder Liste. Nicht virtualisiert: `/`-Menü, Repository-Auswahl, Aufgabenliste, Anhänge-Zeile, Werkzeug-Gruppen (liegen im virtualisierten Verlauf).
- **Fehler als Satz an der Stelle des fehlenden Ergebnisses**, in `--color-status-error`, statt der Liste, statt der Ausgabe oder als Zeile unter der Session-Kopfzeile. Kein Toast (verschwindet, bevor man ihn liest), kein Dialog (blockiert). Die Konsole bekommt den Fehler zusätzlich. Aktionsfehler einer Session sammelt der Zustand-Slice `sessionErrors`; die Zeile zeigt den letzten, bis die nächste Aktion der Session gelingt oder der Benutzer sie schließt.

## Konsequenzen

- Ein neuer Einstellungswert ist ein neuer Schlüssel mit Standardwert in `src-tauri/src/settings/`, keine Migration.
- Eine Datenbank von einer älteren App-Version bekommt Migration 006 beim Start; ohne gespeicherte Werte verhält sich die App wie vorher.
- Löscht der Benutzer den WebView-Speicher, blitzt beim nächsten Start einmal das System-Schema auf, bis die Einstellungen geladen sind.
- Die Einstellungsseite weicht vom Entwurf ab: keine Zeilen „Branch-Präfix“ und „Basis für Changes“; der Denkaufwand steht in der Zeile „Standard-Modus“, weil er in der App seit M2a zum Modus-Menü gehört.
- Sehr viele Vorhaben kosten weiter Speicher für ihre Zusammenfassungen, aber keine DOM-Knoten mehr. Seitenweises Laden bleibt offen, falls das je spürbar wird.
