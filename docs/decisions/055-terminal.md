# 055 — Terminal im Vorhaben

**Status:** angenommen · **Datum:** 2026-11-04

## Kontext

Wer neben dem Agenten arbeitet, braucht eine Kommandozeile in genau dem Ordner, in dem der Agent arbeitet: Dev-Server starten, `git`, `ssh`, ein Blick in den Workspace. Bisher hieß das, ein externes Terminal zu öffnen und den Workspace-Pfad von Hand hineinzukopieren.

## Optionen

- **(a) Externes Terminal starten** — verworfen: Fenster verliert den Bezug zum Vorhaben, die Oberfläche weiß nichts von den Prozessen, nichts wird beim Archivieren aufgeräumt.
- **(b) Eingebettet als Reiter neben dem Chat** — verworfen: Chat und Terminal sollen nebeneinander sichtbar sein.
- **(c) Eingebettet als Dock unten oder rechts** — gewählt: Pseudo-Terminal im Core über `portable-pty` (Windows ConPTY, macOS/Linux Unix-PTY), Anzeige mit xterm.js.
- **(d) `!`-Befehle im Chat** — verworfen: kein dauerhafter Prozess, keine Vollbild-Programme, kein `ssh`.

## Entscheidung

- Terminals gehören dem **Vorhaben**, nicht der Session; alle Sessions teilen den Workspace und damit die Terminals. Höchstens 8 laufende je Vorhaben, Nummer je Vorhaben ab 1, bis App-Ende nie wiederverwendet.
- Die Shell richtet sich nach dem Betriebssystem: Windows `ComSpec` (sonst `cmd.exe`), macOS `SHELL` (sonst `/bin/zsh`) mit `-l`, Linux `SHELL` (sonst `/bin/bash`) mit `-l`; `TERM=xterm-256color`, `COLORTERM=truecolor`.
- **Nur im Speicher:** Terminals enden mit „×“, beim Archivieren des Vorhabens und beim App-Ende. Kein Eintrag in der Datenbank, keine Migration.
- Die Ausgabe landet in einem Puffer von 1 MiB (UTF-8, über Lesestücke hinweg zusammengesetzt). `terminal_attach` hängt den Kanal an und liefert den Puffer unter derselben Sperre, damit zwischen Verlauf und Live-Ausgabe nichts fehlt.
- Das Ende der Shell wird zusammen mit dem Kanal unter der Ausgabe-Sperre festgehalten; wer sich danach anhängt, bekommt `Exit` sofort.

## Konsequenzen

- Der Verlauf wird nur nach einem Neuladen der Oberfläche (`Strg+R`) neu abgespielt; jede xterm-Instanz lebt bis zum Ende ihres Terminals. Das ist eine **bewusste Ausnahme von Critical Rule 3** (Speicher der Oberfläche wächst mit der Zahl offener Terminals statt nur mit dem Sichtbaren), begrenzt durch höchstens 8 laufende je Vorhaben und das Entfernen mit „×“. Nach einem Neuladen können Vollbild-Programme verzerrt aussehen, bis sie neu zeichnen.
- **macOS und Linux sind ungeprüft:** die Shell-Wahl ist nur per Code-Blick auf die `cfg`-Zweige belegt.
- **Enkelprozesse:** ob ein in `cmd` gestartetes Programm (`pnpm dev`) mit „×“ oder App-Ende stirbt, hängt am Schließen der ConPTY unter Windows. Bleibt es stehen, braucht es ein Job-Objekt.
- Die letzten Ausgabezeilen unter Windows hängen daran, dass der Warte-Thread die ConPTY erst nach dem Ende des Prozesses schließt.
