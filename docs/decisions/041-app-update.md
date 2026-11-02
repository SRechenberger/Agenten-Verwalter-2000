# 041 — Update des Verwalters aus den GitHub-Releases

**Status:** angenommen · **Datum:** 2026-10-27

## Kontext

Neue Versionen erscheinen als GitHub-Releases (Workflow `release.yml`, der Tag `v<Version>` entsteht mit dem Release): eine portable exe, ein NSIS-Installer, Pakete für macOS und Linux. Bisher holt man sie von Hand. Die Releases sind nicht signiert, das Repository ist öffentlich.

## Optionen

- **(a) Tauri-Updater-Plugin** — verworfen: verlangt einen Signaturschlüssel und ein Manifest je Release, und die portable exe kennt es nicht.
- **(b) Eigener Ablauf mit dem `digest` der GitHub-API** — gewählt: GitHub nennt je Release-Datei die SHA-256; keine Schlüsselverwaltung, kein Eingriff in `release.yml`.
- **(c) Tausch über ein Hilfsprogramm oder Skript** — verworfen: Windows erlaubt, die laufende exe umzubenennen; das genügt.

## Entscheidung

- Abfrage `GET /repos/SRechenberger/Agenten-Verwalter-2000/releases?per_page=10` ohne Token, beim App-Start und auf „Erneut prüfen“; Entwürfe und Vorabversionen zählen nicht.
- Geladen wird nur eine Datei unter `https://github.com/SRechenberger/Agenten-Verwalter-2000/releases/download/`, deren Größe, SHA-256 und Programmkopf (`MZ`) stimmen. Fehlt der `digest`, wird nicht installiert.
- **Portable** (keine `uninstall.exe` neben der exe): Download nach `<exe>.neu`, laufende exe nach `<exe>.alt`, `.neu` an ihren Platz, Neustart über `cmd /c "ping … & start "" "<exe>""` nach zwei Sekunden, App beendet sich. Der nächste Start löscht `.alt` und `.neu`.
- **Installer** (`uninstall.exe` neben der exe): Download in den Temp-Ordner, Start des Installers auf demselben Weg, App beendet sich.
- **Manual** (nicht Windows): nur Link auf die Release-Seite.
- `VERWALTER_APP_VERSION` ersetzt die laufende Version nur in Debug-Läufen.

**2026-11-02:** Die App holt Updates vom Fork statt vom Original, denn sonst ersetzt ein Release des Originals den Stand des Forks. Der Tag löst `release.yml` nicht mehr aus: Merges laufen über die GitHub-Oberfläche, es ist also keine Session da, die danach taggt, und ein über `GITHUB_TOKEN` gesetzter Tag startet keinen weiteren Workflow. Deshalb legt der Workflow den Tag selbst an.

## Konsequenzen

- Die Prüfsumme schützt vor beschädigten Downloads, nicht vor einem gekaperten GitHub-Konto.
- Die Dateinamen `*-setup.exe` und `*_x64-portable.exe` sind ein Vertrag zwischen `release.yml` und `app_update/releases.rs`.
- Der Dateiname der portable exe bleibt nach dem Update der alte, auch wenn er eine Version trägt; die echte Version steht in den Einstellungen.
- Laufende Agenten werden beim Neustart unterbrochen. Ein Agent, der von der eigenen exe läuft (Autark, Codex), kann `.alt` sperren; dann bleibt sie bis zum nächsten Start liegen, und ein weiteres Update vor dem Ende dieses Agenten scheitert mit einer Meldung.
- Ohne `digest` in künftigen API-Antworten bricht die Installation ab; das zu lockern braucht einen neuen ADR.
