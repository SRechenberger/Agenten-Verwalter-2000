# Release Conventions — verwalter

> Ein PR `release/v<X.Y.Z>` → `main` baut den Kandidaten `v<X.Y.Z>-candidate`, ein Vorab-Release, und sein Merge veröffentlicht `v<X.Y.Z>` mit genau dessen Dateien; beides macht [.github/workflows/release.yml](../../.github/workflows/release.yml). Gebaut wird wie bisher: Windows baut den NSIS-Installer und die lose `verwalter.exe`, macOS (Apple Silicon, ab macOS 13.4) ein `.dmg`, Ubuntu 24.04 ein `.AppImage` und ein `.deb`. Schlägt nur macOS oder Linux fehl, erscheinen Kandidat und Release ohne deren Dateien; schlägt Windows fehl, erscheint kein Kandidat. „Run workflow" auf der Actions-Seite baut alle Plattformen als Probe, ohne Kandidat und ohne Release, auch auf einem Branch. Projektentscheidungen in dieser Datei haben Vorrang.

## Branches und PRs

- Feature-Branches zweigen von `dev` ab ([commits.md](commits.md)).
- Ein PR nach `dev` wird per **Squash** gemergt. Sein Titel trägt `[MAJOR]`, `[MINOR]` oder `[PATCH]`, sonst ist der Check `annotation` rot; der Squash nimmt den Titel als Betreff.
- `release/v<X.Y.Z>` → `main` wird mit **Merge-Commit** gemergt, nie per Squash: Nach einem Squash wäre der Release-Stand kein Vorfahr von `dev`, und der nächste Release-PR rechnete vom alten Abzweig aus.
- Danach geht `release/v<X.Y.Z>` **immer** per PR mit Merge-Commit zurück nach `dev`, auch ohne Fix. Der Versions-Commit liegt sonst nur auf dem Release-Branch, und beim nächsten Release kollidieren die vier Versionszeilen.
- Den Release-Branch erst nach der Rückführung löschen.

## Versionsnummer

**Die Version steigt nur auf einem Release-Branch.** Ein PR nach `dev` ändert sie nie. Welche Nummer das Release trägt, folgt aus den Annotationen der Betreffzeilen seit dem letzten Release: Basis ist die Version in `package.json` auf `main`, gezählt werden die Commits auf dem Release-Branch, die `main` noch nicht hat.

Eine Betreffzeile trägt eine Annotation, wenn sie `[MAJOR]`, `[MINOR]` oder `[PATCH]` enthält, genau so geschrieben, großgeschrieben und in eckigen Klammern, an beliebiger Stelle (GitHub hängt ` (#N)` an den Squash-Betreff). Ein nacktes „PATCH“ zählt nicht, denn Commit-Texte sprechen von HTTP-`PATCH`. Merge-Commits und der Versions-Commit tragen keine Annotation.

| Annotationen seit dem letzten Release | Nächste Version |
|---|---|
| mindestens ein `MAJOR`, Basis `0.y.z` | `0.(y+1).0` |
| mindestens ein `MAJOR`, sonst | `(x+1).0.0` |
| kein `MAJOR`, mindestens ein `MINOR` | `x.(y+1).0` |
| nur `PATCH` | `x.y.(z+1)` |
| keine | keine: es gibt nichts zu releasen |

`node scripts/version.ts next origin/main` nennt die Nummer, wenn man auf dem Stand steht, der ausgeliefert werden soll (`dev` oder der Release-Branch); es liest `origin/main..HEAD`. `node scripts/version.ts selftest` prüft die Regel an festen Fällen, `pnpm version:check` fährt ihn mit, also auch `pnpm check` und [.github/workflows/check.yml](../../.github/workflows/check.yml).

Die Version steht an **vier Stellen**, `pnpm bump <X.Y.Z>` setzt sie, und zwar nur auf dem Release-Branch; `pnpm check` und die CI prüfen, dass alle vier gleich sind.

- `package.json` (`version`)
- `src-tauri/tauri.conf.json` (`version`) — daraus bekommt der Installer seine Nummer
- `src-tauri/Cargo.toml` (`[package]` → `version`)
- `src-tauri/Cargo.lock` (`version` direkt hinter `name = "verwalter"`)

**1.0.0 entsteht nie von selbst:** `MAJOR` bei `0.y.z` ist ein Minor-Schritt. Den Sprung entscheidet der User ausdrücklich; das Tor in `release.yml` wird dafür einmal angepasst.

## Release anlegen

```bash
git fetch origin
v=$(node scripts/version.ts next origin/main)
git switch -c release/v$v origin/dev
pnpm bump $v
git commit -am "chore(release): Version $v"
git push -u origin release/v$v
```

Dann den PR `release/v<X.Y.Z>` → `main` öffnen. `next` liest `origin/main..HEAD`, `HEAD` soll beim ersten Aufruf also auf `origin/dev` stehen; weicht die Nummer ab, nennt das Tor des PRs die richtige.

## Was die CI tut

- **PR → `main`** (geöffnet, neuer Push, wieder geöffnet): Das Tor (`gate`) prüft, dass der Branch `release/v<X.Y.Z>` heißt, dass `<X.Y.Z>` gleich `next origin/main` ist und dass die vier Stellen `<X.Y.Z>` tragen. Dann baut die Matrix, und der Job `candidate` legt das Vorab-Release `v<X.Y.Z>-candidate` an, mit dem Tag auf dem Kopf des Branches (nicht auf dem Test-Merge des PRs). Jeder weitere Push ersetzt den Kandidaten: Es gibt immer höchstens einen, und ein laufender Bau desselben PRs wird abgebrochen.
- **Der Verwalter bietet den Kandidaten nicht an:** Die Update-Funktion ignoriert Vorab-Releases (`src-tauri/src/app_update/releases.rs`, [ADR 041](../decisions/041-app-update.md)).
- **Merge nach `main`:** Der Lauf auf `main` prüft die Versionsstellen und dass `v<X.Y.Z>` noch nicht auf einem anderen Stand liegt. Der Job `release` vergleicht den Baum von `main` mit dem des Kandidaten. Sind sie gleich, übernimmt er die **Dateien des Kandidaten** als Release `v<X.Y.Z>` (nichts wird neu gebaut), mit Notizen seit dem letzten veröffentlichten Release, und entfernt danach den Kandidaten samt Tag. Sind sie verschieden oder fehlt der Kandidat, bleibt das Release aus, und der Lauf wird rot.
- **PR ohne Merge geschlossen:** Kandidat und Tag werden entfernt.
- **Probelauf** („Run workflow"): baut alle Plattformen wie der Kandidat, legt aber nichts an.

## Fixes im Release

- Ein Fix geht per PR (Squash, Titel mit Annotation) in `release/v<X.Y.Z>`. Der Push auf den Release-Branch baut einen neuen Kandidaten.
- Ändert ein Fix die errechnete Nummer, etwa durch ein `[MINOR]`, wird das Tor rot, und das Release braucht einen neuen Branch.
- **Hotfix** für ein Release, während auf `dev` Unfertiges liegt: `release/v<X.Y.Z>` von `main` statt von `dev` abzweigen. Das Tor zählt dann nur die Fix-Commits, also meist `PATCH`. Danach geht der Branch wie jeder andere zurück nach `dev`.

## Upstream-Abgleich

Der Abgleich mit `Aalphones/main` läuft als PR nach `dev` mit Annotation. Dort hebt jeder PR die Version weiter an; die vier Versionszeilen werden beim Abgleich immer auf den Stand von `dev` aufgelöst.

## Wenn etwas schiefgeht

- **Tor rot wegen Name oder Version:** Die Meldung nennt `release/v<errechnet>` oder `pnpm bump <X.Y.Z>`. Den Branch unter dem richtigen Namen neu anlegen bzw. bumpen, committen und pushen. Meldet das Tor „nichts zu releasen“, fehlt seit dem letzten Release jede Annotation.
- **`main` weicht vom Kandidaten ab:** Der Lauf `release` wird rot, es entsteht kein Release, der Kandidat bleibt stehen. Ursache ist ein Merge vor dem fertigen Kandidaten oder ein weiterer Commit auf `main` dazwischen. Weiter geht es nur nach Rücksprache mit dem User: etwa mit einem neuen Release-Branch von `main` (Hotfix-Weg), der den Stand ausliefert.
- **Kandidat fehlt:** Der Lauf `release` meldet „kein Kandidat“ und wird rot. Ursache meist ein Merge, bevor der Kandidat gebaut war; weiter wie bei „`main` weicht ab“.
- **Lauf wiederholen:** `candidate` und `release` lassen sich über „Re-run“ wiederholen und kommen zum selben Ergebnis: Gibt es das Release schon, entfernt der Lauf nur den Kandidaten; ein Tag ohne Release räumt `candidate` vor dem Anlegen ab.
- **Eine macOS-Datei fehlt im Kandidaten**, weil der Job abgebrochen wurde (rund 15 Minuten, 0 Schritte): Den Kandidaten-Lauf wiederholen, **bevor** gemergt wird. Das Release übernimmt die Dateien des Kandidaten und damit auch das Fehlen.

## Regeln

- Ein Release entsteht nur durch den Merge eines Release-PRs: nicht über „Draft a new release" auf GitHub und nicht durch einen Tag-Push.
- Ein über `GITHUB_TOKEN` gesetzter Tag oder angelegter PR startet keinen Lauf. Deshalb entsteht das Release im selben Lauf, der den Merge sieht.
- Ein veröffentlichter Tag `v<X.Y.Z>` wird **nie verschoben oder gelöscht** ohne ausdrückliche Anweisung des Users. Ausnahme sind die Kandidaten-Tags `v<X.Y.Z>-candidate`, die die CI verwaltet.
- Ohne eingeschaltete Actions im Repository entsteht kein Kandidat und kein Release.
- Das Release ist ohne Code-Signatur: Windows SmartScreen warnt beim Start der heruntergeladenen Datei. macOS meldet die App als „beschädigt"; nach dem Kopieren nach Programme hilft `xattr -dr com.apple.quarantine "/Applications/Agenten Verwalter 2000.app"`. Das ist bekannt und kein Fehler des Laufs.
- macOS und Linux sind nicht vollständig getestet: Git-, Changes- und Artefaktpfade behalten dort ihre `/`, aber es gab noch keinen Lauf auf diesen Systemen. Die Betriebsart „Autark“ ruft weiter Windows-Programme (`taskkill`, `powershell.exe`, `cmd`) auf.
- **Icons:** Installer, lose exe, `.dmg`, `.AppImage` und `.deb` bringen das App-Icon aus `src-tauri/icons/` mit. Der Installer bekommt es über `bundle.windows.nsis.installerIcon`, ohne den Schlüssel trägt er das NSIS-Standardsymbol. Eine AppImage-Datei kann ihr Icon nicht selbst im Dateimanager zeigen: Sie enthält es als `.DirIcon`, und ein Vorschau-Plugin liest es dort. Dolphin unter KDE braucht dafür das Paket `libappimage`.
- Die Dateinamen `*-setup.exe` und `*_x64-portable.exe` sind ein **Vertrag mit der Update-Funktion** des Verwalters (`src-tauri/src/app_update/releases.rs`, [ADR 041](../decisions/041-app-update.md)): wer sie in `release.yml` ändert, passt die Auswahl der Datei dort an, sonst findet kein laufender Verwalter mehr sein Update.
- **Ältere Pläne:** Nennt ein Plan unter `docs/planning/` noch eine Anhebung in jedem PR oder einen Tag-Push, gilt diese Datei.

## Critical Rules

1. **Die Version steigt nur auf einem Release-Branch** — ein PR nach `dev` ändert sie nicht; die Nummer folgt aus den Annotationen.
2. **Nach `main` gehen nur `release/v<X.Y.Z>`-Branches** — per Merge-Commit; das Tor lässt sonst nichts durch.
3. **Die vier Stellen tragen dieselbe Version** — sonst trägt der Installer eine andere Nummer als das Release; gesetzt wird sie nur mit `pnpm bump <X.Y.Z>`.
4. **Nach jedem Release geht es zurück nach `dev`** — per PR mit Merge-Commit, auch ohne Fix.
