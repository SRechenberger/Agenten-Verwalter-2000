# Release Conventions — verwalter

> Ein Versions-Tag `vX.Y.Z` löst [.github/workflows/release.yml](../../.github/workflows/release.yml) aus: Windows baut den NSIS-Installer und die lose `verwalter.exe`, macOS (Apple Silicon, ab macOS 13.4) ein `.dmg`, Ubuntu 24.04 ein `.AppImage` und ein `.deb`; ein letzter Job legt das GitHub-Release an und hängt alle Dateien an. Schlägt nur macOS oder Linux fehl, erscheint das Release ohne deren Dateien; schlägt Windows fehl, erscheint keins. „Run workflow" auf der Actions-Seite baut alle Plattformen als Probe, ohne Release. Projektentscheidungen in dieser Datei haben Vorrang.

## Wann getaggt wird

Genau dann, wenn ein Plan **vollständig** abgeschlossen ist und archiviert wird — alle Phasen `complete`, Doc-Abgleich erledigt, Prüfkette grün. Getestet wird danach im laufenden Betrieb, nicht vorher per Checkliste. Nicht nach einzelnen Phasen, nicht bei roter Prüfkette.

## Versionsnummer

Solange die App unter 1.0 ist: **jeder abgeschlossene Plan hebt die Minor-Nummer** (`0.2.0` → `0.3.0`). Ein Plan, der nur Fehler behebt und nichts Neues bringt, hebt die Patch-Nummer (`0.2.0` → `0.2.1`). Den Sprung auf `1.0.0` entscheidet der User ausdrücklich.

Die Version steht an **drei Stellen**, die immer übereinstimmen müssen — das Release bricht sonst ab:

- `package.json` (`version`)
- `src-tauri/tauri.conf.json` (`version`) — daraus bekommt der Installer seine Nummer
- `src-tauri/Cargo.toml` (`[package]` → `version`)

Dazu gehört der nachgezogene Eintrag in `src-tauri/Cargo.lock`.

## Ablauf beim Archivieren

1. Plan archivieren wie gewohnt und committen.
2. Neue Version festlegen (Regel oben) und in den drei Dateien setzen.
3. `cargo check --manifest-path src-tauri/Cargo.toml` laufen lassen, damit `Cargo.lock` die neue Version übernimmt; dann `pnpm check` — grün, sonst kein Tag.
4. Ein eigener Commit: `chore(release): v<X.Y.Z>` mit den vier Dateien.
5. Annotierten Tag auf diesen Commit setzen: `git tag -a v<X.Y.Z> -m "v<X.Y.Z>"`.
6. Commits und Tag zusammen pushen: `git push origin main v<X.Y.Z>`. Der Tag-Push startet den Release-Lauf.
7. Im Bericht an den User nennen: Version, Tag, dass der Release-Lauf angestoßen ist, und den Link auf die Actions-Seite des Repositories.

## Regeln

- Ein gepushter Tag wird **nie verschoben oder gelöscht** ohne ausdrückliche Anweisung des Users. Schlägt der Release-Lauf wegen abweichender Versionen fehl, ist das eine Rückfrage, keine eigenmächtige Reparatur.
- Kein Tag ohne den `chore(release)`-Commit darunter — der Tag zeigt immer auf den Stand, dessen Dateien die Version tragen.
- Das Release ist ohne Code-Signatur: Windows SmartScreen warnt beim Start der heruntergeladenen Datei. macOS meldet die App als „beschädigt"; nach dem Kopieren nach Programme hilft `xattr -dr com.apple.quarantine "/Applications/Agenten Verwalter 2000.app"`. Das ist bekannt und kein Fehler des Laufs.
- macOS und Linux sind nicht vollständig getestet: Git-, Changes- und Artefaktpfade behalten dort ihre `/`, aber es gab noch keinen Lauf auf diesen Systemen. Die Betriebsart „Autark“ ruft weiter Windows-Programme (`taskkill`, `powershell.exe`, `cmd`) auf.
- Die Dateinamen `*-setup.exe` und `*_x64-portable.exe` sind ein **Vertrag mit der Update-Funktion** des Verwalters (`src-tauri/src/app_update/releases.rs`, [ADR 041](../decisions/041-app-update.md)): wer sie in `release.yml` ändert, passt die Auswahl der Datei dort an, sonst findet kein laufender Verwalter mehr sein Update.

## Critical Rules

1. **Getaggt wird nur ein abgeschlossener Plan mit grüner Prüfkette** — ein Release ist für Dritte sichtbar und gilt als fertiger Stand.
2. **Tag, `package.json`, `tauri.conf.json` und `Cargo.toml` tragen dieselbe Version** — sonst trägt der Installer eine andere Nummer als das Release.
