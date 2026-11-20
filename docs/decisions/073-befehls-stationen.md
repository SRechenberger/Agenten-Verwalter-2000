# 073 — Befehls-Stationen: Schritte ohne Modell, aus einer Vorlage, nur Programme der Freigabeliste

**Status:** angenommen und umgesetzt im Kern (Migration 33, Art der Station, Vorlage, Werte der Vorgänger, Freigabeliste, Lauf und Stopp; Story Taiga #134, Phase 134.1). Die Oberfläche steht seit Phase 134.2.

## Kontext

Ein Merge, ein `gh pr create` oder ein Aufruf von `glab` braucht kein Modell und kein Kontingent. Bisher war so eine Station „durch dich“: Der Benutzer tippte den Befehl selbst, oder ein Agent lief nur, um ihn auszuführen.

## Entscheidung

Vom Benutzer entschieden (N1, N2, N7, D9 bis D13):

- **Art der Station (N2).** `kind: 'agent' | 'user' | 'command'`. Quelle ist die Spalte `implementation_stations.kind`; Migration 33 setzt sie aus `by_user` (`user` bei 1, sonst `agent`). `by_user` bleibt als Spalte stehen, wird aber nicht mehr gelesen oder geschrieben; in der Ansicht ist `byUser` gleich `kind == 'user'`. Eingabe der Werkzeuge: `by: "agent" | "you" | "command"`.
- **Vorlage (N1).** Eine Befehls-Station hat `command` (1 bis 5 Schritte, je höchstens 1000 Zeichen) und `cwd`. Ein Schritt sind Wörter, getrennt durch Leerraum; `"…"` hält Leerraum zusammen, `\"` ist ein Anführungszeichen, jeder andere Backslash bleibt (Windows-Pfade). Das erste Wort ist das Programm. `|`, `>`, `&&`, `$` und `*` sind gewöhnliche Zeichen.
- **Werte der Vorgänger (N1).** `{{name}}` setzt den Wert ein, den ein direkter Vorgänger mit `vars` seiner Meldung hinterlassen hat (`implementation_complete_station`: höchstens 20 Werte, Name `^[a-z][a-z0-9_]{0,31}$`, Wert 1 bis 500 Zeichen in einer Zeile). Die Werte stehen in der Spalte `vars` der Station und werden mit der Meldung ersetzt. `{{env:NAME}}` kommt aus der Umgebung des Verwalters. `{{.x}}` einer Go-Vorlage und ein einzelnes `{{` bleiben Text; ein Platzhalter mit ungültigem Namen lehnt schon das Anlegen ab.
- **Eingesetzt wird nach dem Zerlegen**, aus Stücken (`command::Piece`), nie aus dem Text eines Werts: Ein Wert wird nie zu zwei Wörtern, und ein Wert, der wie `{{env:X}}` aussieht, bleibt Text. Fehlt ein Wert oder nennen zwei Vorgänger denselben Namen mit verschiedenen Werten, ist das ein Startfehler der Station; der Satz nennt den Namen.
- **Keine Shell (N7, D9).** Der Verwalter startet das Programm über `processes::command` mit den Wörtern als Argumenten; es gibt kein `sh -c` und kein `cmd /c`. Das Programm ist ein bloßer Name ohne `/` und `\` und ohne Platzhalter und steht in der Freigabeliste.
- **Freigabeliste.** Einstellung `commandPrograms` (Schlüssel `command_programs`, Standard `["gh", "glab"]`), global, `settings_update` hat die Klasse G. Geprüft wird beim Anlegen, Ändern und Vorschlagen (`implementation_create`, `implementation_set_phases`, `implementation_edit`) und beim Start noch einmal: „Programm „<name>“ ist für Befehls-Stationen nicht freigegeben.“
- **`cwd`.** Ein absoluter Pfad: der Ordner des Vorhabens, ein Repository des Vorhabens oder ein Ordner `<repo>-wt-*` daneben, samt Unterordnern, nie mit `..`. Ohne Angabe gilt der Ordner des Vorhabens. Die Prüfung ist lexikalisch; ein Symlink im Ordner wird nicht aufgelöst.
- **Lauf (D11).** Die Station startet wie jede Station mit ▶ oder durch die Automatik, aber ohne Session, ohne Modell und ohne Platz am Tor. `Countdowns` kennt sie nicht, `due_for_countdown` und `station_waiters` lassen sie aus. Die Automatik hat keinen Countdown: Ein Durchgang der Warteschlange startet jede bereite Befehls-Station gleich (`command_run::start_due`). Die Schritte laufen nacheinander im eigenen Thread, stdout und stderr zusammen; die letzten 200 Zeilen stehen in `output`, die Ansicht folgt höchstens alle 2 s. Zeitlimit 60 Minuten je Lauf (`command::run` nimmt es als Parameter); mit „Bei Fehler“ und n Neustarts läuft eine Station also höchstens (n+1) × 60 Minuten.
- **Wo der Lauf lebt.** `step`, `output` und der Grund des Fehlers stehen in der Datenbank (Spalten `step`, `output`, `command_error`), nicht im Speicher: `build_view` bleibt eine reine Funktion der Zeilen, und der Grund übersteht den Neustart. Der Preis ist ein Schreibzugriff alle 2 Sekunden je laufender Station. Nur das Zeichen zum Stoppen (`AtomicBool` je Station) steht im Speicher.
- **Zustand.** `running`, solange `step` gesetzt ist; `error` mit Grund; `done`; sonst `pending` oder `ready` wie andere Stationen. Der Grund steht in `startError` der Ansicht (kein eigenes Feld).
- **Ende.** Erfolg: alle Schritte mit Exit 0, die Station ist `done`, `result` ist die letzte nicht leere Zeile (höchstens 200 Zeichen) oder „Befehl erfolgreich“. Der Abschluss ist `complete_station_with_handoff` ohne Meldung, ohne Fix-Runde und als Entscheidung des Masters (`Decider::Master`): Nachfolger wie nach einer Annahme, kein Wecken, auch nicht bei der letzten Station. Fehler: Exit ≠ 0 („Schritt <n>: Exit <k>“), „Zeitlimit“ oder „abgebrochen“. „Bei Fehler“ beginnt die Schritte von vorn, im selben Thread (`command::run_restarting`), außer nach „abgebrochen“; sonst weckt der Fehler den Master mit dem vorhandenen Text „Fehler“, nach „Abbrechen“ mit „gestoppt“ (`command_run::failure_event`, die Marke ist der Beginn des Laufs). ▶ startet die Schritte nach einem Fehler erneut.
- **Neustart der App.** Beim Start setzt `interrupt_commands` jede laufende Befehls-Station auf `error` „unterbrochen“. Sie startet nicht von selbst (Critical Rule 4); „Bei Fehler“ greift dort nicht, weil der Neustart im Thread geschieht, der mit der App endet.
- **Stopp.** `implementation_stop_command(id, stationId)`, Klasse HostWrite mit Umsetzungs-ID (`id` wie bei den anderen Befehlen der Umsetzung, damit `ids_of` die Umsetzung findet). Er setzt das Zeichen; der Läufer beendet den laufenden Schritt. Eine abgebrochene Umsetzung setzt es beim nächsten Fortschritt selbst.
- **Anlegen (D12).** Nur über die Werkzeuge des Planers; die Oberfläche (134.2) legt nichts an. `implementation_edit` ändert mit `change` `command` und `cwd` einer nicht gestarteten Befehls-Station; als begonnen gilt eine Station mit Session, Beginn oder Ende.

## Sicherheit

- Keine Shell an keiner Stelle; ein Pfad als Programm ist verboten; die Freigabeliste gilt beim Anlegen und beim Start.
- `{{env:…}}` erscheint nie in der Ansicht (`resolved` lässt den Platzhalter stehen), nie in einem Fehlertext (er nennt nur den Namen ohne Klammern) und nie im Log.
- **Die Ausgabe zeigt die App, wie sie ist.** Gibt ein freigegebenes Programm einen Wert aus der Umgebung aus, steht er in `output`. Wer ein Programm freigibt, das Werte ausgibt, vertraut ihm.
- Freigegeben ist ein Programm mit allen Argumenten: `gh api` kann viel. Die Liste begrenzt, was startet, nicht was es tut.

## Grenzen

- Der Vertrag nennt „▶ oder Countdown“; die Automatik startet ohne Countdown, weil ein Countdown einen Platz am Tor bände. Der Benutzer, der vor einem Merge Zeit braucht, schaltet „Automatisch starten“ der Station aus.
- Ein Prozess, der beim Abbrechen Kindprozesse hat, lässt sie laufen (`Child::kill` beendet nur den Prozess).
- Der Master wird nicht geweckt, wenn der Erfolg die letzte Station abschließt.
- `StationView.startError` trägt bei einer Befehls-Station den Grund ihres Fehlers.

## Oberfläche

- **Plan-Karte und Zeitleiste:** Eine Befehls-Station heißt `<Stufe> · Befehl` und zeigt ihre Schritte gekürzt (`commandStation.ts`), alle im Tooltip. In „Plan ändern?“ gibt es für sie nichts zu ändern; die Schritte stehen nur da.
- **Kartei:** Der Abschnitt „Befehl“ zeigt die Schritte mit den eingesetzten Werten (`resolved`, `{{env:…}}` bleibt stehen), den laufenden Schritt, die Ausgabe in Monospace (folgt der Ansicht), „Abbrechen“ während `running` (`implementation_stop_command`) und ▶ nach `error` (der vorhandene Start). Auftrag und Agent entfallen; „Als fertig markieren …“ gilt wie bei jeder Station im Fehler. Bei einer Befehls-Station im Fehler trägt `startError` den Grund des Laufs und heißt „Fehler“, nicht „Startet nicht“.
- **Einstellungen:** „Programme für Befehls-Stationen“ mit Hinzufügen und Entfernen; die Oberfläche prüft nur Name, Pfad und Doppelte, den Satz bei Ungültigem liefert der Kern.
- **Nichts angelegt:** Die Oberfläche legt keine Befehls-Station an und rechnet keine Regel des Kerns nach.

## Prüfung

`cargo test --manifest-path src-tauri/Cargo.toml --lib -- implementation::` (Zerlegen, Einsetzen, Prüfung, Lauf mit `git`, Zustände, Tor, Neustart), `… --lib remote::` (Klasse des Befehls), `… --example implementation-probe -- --selftest` (eine Befehls-Station mit `git --version` wird fertig, und ihr Nachfolger startet).
