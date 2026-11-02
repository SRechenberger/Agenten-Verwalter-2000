# Release Conventions — verwalter

> Ein Push auf `main` löst [.github/workflows/release.yml](../../.github/workflows/release.yml) aus. Gibt es den Tag `v<Version>` noch nicht, wird gebaut, und Release samt Tag entstehen zusammen: Windows baut den NSIS-Installer und die lose `verwalter.exe`, macOS (Apple Silicon, ab macOS 13.4) ein `.dmg`, Ubuntu 24.04 ein `.AppImage` und ein `.deb`; ein letzter Job legt das GitHub-Release an und hängt alle Dateien an. Schlägt nur macOS oder Linux fehl, erscheint das Release ohne deren Dateien; schlägt Windows fehl, erscheint keins. „Run workflow" auf der Actions-Seite baut alle Plattformen als Probe, ohne Release, auch auf einem Branch. Projektentscheidungen in dieser Datei haben Vorrang.

## Versionsnummer

**Jeder PR hebt die Version in seinem eigenen Diff.** Solange die App unter 1.0 ist, ist eine Fehlerbehebung ein Patch (`0.2.0` → `0.2.1`), alles andere ein Minor (`0.2.0` → `0.3.0`). Den Sprung auf `1.0.0` entscheidet der User ausdrücklich.

**Parallele PRs:** Haben zwei offene PRs angehoben, setzt der später gemergte auf `main` auf und hebt von der Version an, die `main` dann trägt. Dieselbe Nummer gibt es nie zweimal. Heben zwei offene PRs auf dieselbe neue Nummer, mergen sie ohne Konflikt, denn beide Seiten ändern die Zeilen gleich, und der zweite bleibt grün, weil `pull_request` bei einer Änderung am Basis-Branch nicht neu läuft. Vor dem Merge also prüfen, ob die Nummer noch frei ist.

Die Version steht an **vier Stellen**, `pnpm bump <X.Y.Z>` setzt sie, `pnpm check` und [.github/workflows/check.yml](../../.github/workflows/check.yml) prüfen sie. Ein eigener `chore(release)`-Commit entfällt.

- `package.json` (`version`)
- `src-tauri/tauri.conf.json` (`version`) — daraus bekommt der Installer seine Nummer
- `src-tauri/Cargo.toml` (`[package]` → `version`)
- `src-tauri/Cargo.lock` (`version` direkt hinter `name = "verwalter"`)

## Wenn etwas schiefgeht

- **Merge ohne Anhebung:** `check.yml` lässt den PR scheitern. Wird er trotzdem gemergt, entsteht kein Release, und der Lauf von `release.yml` auf `main` wird rot, weil der Tag auf einen anderen Commit zeigt. Der nächste PR hebt an und nimmt den Stand mit.
- **Fehlgeschlagener Lauf:** Der Tag entsteht erst mit dem Release, also lässt sich der Lauf wiederholen.

## Regeln

- Ein Tag entsteht nur durch den Workflow: Ein von Hand gesetzter Tag `v<Version>` unterdrückt das Release dieser Version.
- Ein gepushter Tag wird **nie verschoben oder gelöscht** ohne ausdrückliche Anweisung des Users.
- Ohne eingeschaltete Actions im Repository entsteht kein Release.
- Das Release ist ohne Code-Signatur: Windows SmartScreen warnt beim Start der heruntergeladenen Datei. macOS meldet die App als „beschädigt"; nach dem Kopieren nach Programme hilft `xattr -dr com.apple.quarantine "/Applications/Agenten Verwalter 2000.app"`. Das ist bekannt und kein Fehler des Laufs.
- macOS und Linux sind nicht vollständig getestet: Git-, Changes- und Artefaktpfade behalten dort ihre `/`, aber es gab noch keinen Lauf auf diesen Systemen. Die Betriebsart „Autark“ ruft weiter Windows-Programme (`taskkill`, `powershell.exe`, `cmd`) auf.
- Die Dateinamen `*-setup.exe` und `*_x64-portable.exe` sind ein **Vertrag mit der Update-Funktion** des Verwalters (`src-tauri/src/app_update/releases.rs`, [ADR 041](../decisions/041-app-update.md)): wer sie in `release.yml` ändert, passt die Auswahl der Datei dort an, sonst findet kein laufender Verwalter mehr sein Update.
- **Ältere Pläne:** Nennt ein Plan unter `docs/planning/` noch einen Versions-Tag oder einen `chore(release)`-Commit, gilt diese Datei.

## Critical Rules

1. **Jeder Merge auf `main` trägt eine neue Version** — ohne sie entsteht kein Release, und ein Release ist für Dritte sichtbar und gilt als fertiger Stand.
2. **Die vier Stellen tragen dieselbe Version** — sonst trägt der Installer eine andere Nummer als das Release; gesetzt wird sie nur mit `pnpm bump <X.Y.Z>`.
