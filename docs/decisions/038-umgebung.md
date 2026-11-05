# 038 — Umgebung in den Einstellungen

**Status:** angenommen · **Datum:** 2026-11-04

## Kontext

Skills, Befehle, Ausgabestile und die Regeldatei `CLAUDE.md` unter `~\.claude` wirken in jeder Session. Liegen sie als Kopien neben einem Original (ein synchronisierter Ordner, ein Repository), veralten sie unbemerkt; zeigt eine Verknüpfung (Junction, symbolischer Link) ins Leere, fehlt etwas, ohne dass jemand es sagt. Der Verwalter kennt weder Profile noch Accounts noch persönliche Pfade und soll das auch nicht lernen.

## Optionen

- **(a) Die eigene Sync-Einrichtung des Benutzers prüfen** (Profile, feste Pfade) — verworfen: gehört nicht in committeten Code.
- **(b) Allgemein: Verknüpfungen unter `~\.claude` prüfen und mit einem frei wählbaren Master-Ordner abgleichen** — gewählt. Der Master-Ordner ist eine Einstellung in der lokalen Datenbank.
- **(c) Spiegeln mit Löschen** (wie `robocopy /MIR`) — verworfen: ein falsch gewählter Master-Ordner würde lokale Skills löschen.
- **(d) Vergleich allein nach Änderungszeit** — verworfen: ein synchronisiertes Laufwerk setzt eigene Zeiten und meldete gleiche Dateien als abweichend. Gewählt: der Inhalt entscheidet, ob etwas abweicht, die Zeit nur die Richtung.
- **(e) Umfang nur Skills und Befehle** — verworfen: Ausgabestile und `CLAUDE.md` wirken in jeder Session und blieben sonst bei grünem „Gleich“ veraltet. `settings.json` bleibt draußen, sie trägt Maschinen-Schlüssel.

## Entscheidung

- **Verknüpfung** = Eintrag direkt unter `~\.claude` oder direkt in einem echten Unterordner davon (zwei Ebenen), den `fs::symlink_metadata` als Link meldet. In einen Link wird nicht hineingegangen. **Kaputt**, wenn `fs::metadata` (folgt dem Link) scheitert; das Ziel steht, soweit `fs::read_link` es liefert.
- **Einträge des Abgleichs:** Skill = Unterordner von `skills` mit `SKILL.md`; Befehl = `*.md` direkt in `commands`; Ausgabestil = `*.md` direkt in `output-styles`; Regeldatei = `CLAUDE.md` gegen `~\.claude\CLAUDE.md`.
- **Abgleich** vergleicht Datei für Datei nach Inhalt (erst Größe, dann Bytes). Zustände: `onlyMaster`, `masterNewer`, `localNewer` (bleibt, zählt nicht als Abweichung), `onlyLocal` (bleibt). Dateien, deren Name auf eine Konfliktkopie des Laufwerks deutet (` (<Zahl>)`, `(Kopie)`, `konflikt`, `conflict`, ohne Beachtung der Schreibweise), zählen nicht mit.
- **Spiegeln** kopiert per `fs::copy`, was lokal fehlt oder abweicht und im Master neuer ist; es legt Ordner an, löscht nichts, kopiert nie rückwärts, folgt im Master keinen Links und schreibt lokal nie durch einen Link hindurch (eine lokale `CLAUDE.md`, die selbst ein Link ist, gilt als gleich). Tiefe höchstens 8. Vor jedem Schreiben wird der Vergleich der Datei noch einmal gemacht.
- **Geprüft wird** beim App-Start und beim Öffnen der Einstellungen, nie im Hintergrund. „Zuletzt gespiegelt“ lebt nur im Speicher.
- **Master-Ordner gültig**, wenn er existiert, nicht `~\.claude` selbst ist und `skills`, `commands`, `output-styles` oder `CLAUDE.md` enthält; sonst steht ein Satz in `master_error` und es gibt keinen Abgleich.

## Konsequenzen

- Kein Hintergrund-Prüfen: eine Verknüpfung, die zwischen zwei Öffnungen der Einstellungen bricht, fällt erst beim nächsten Öffnen auf.
- Lokal neuere Dateien werden nur angezeigt („lokal neuer · bleibt“), nie überschrieben. Wer im Master etwas zurücknehmen will, löscht es dort und lokal von Hand.
- Die Namensmuster für Konfliktkopien raten, wie ein Laufwerk Kopien benennt. Sie treffen auch echte Dateien, deren Name `conflict` oder `konflikt` enthält (etwa `conflict-resolution.md`): die werden nie abgeglichen. Weicht ein Laufwerk von den Mustern ab, rutscht eine Kopie in die Liste und beim Spiegeln lokal hinein.
- Ein Skill, der im Master und lokal nur in der Schreibweise des Namens abweicht, gilt als derselbe Eintrag (Windows vergleicht Namen so).
