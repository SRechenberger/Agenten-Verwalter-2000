# 026 — Artefakte: HTML-Seiten des Agenten im Verwalter zeigen

**Status:** angenommen · **Datum:** 2026-10-06 · ergänzt durch [ADR 047](047-artefakte-alle-dateitypen.md) (alle Dateitypen, Betrachter, anlegende Session)

## Kontext

Berichte, Präsentationen, Diagramme und Design-Entwürfe sind als HTML-Seite am besten lesbar. Der Agent soll sie ablegen, der Verwalter sie als echte, bedienbare Seite zeigen und nach jedem Speichern neu laden. Die Seite ist fremdes HTML mit Skripten im Fenster des Verwalters: sie darf weder dessen Befehle aufrufen noch Dateien außerhalb ihres Ordners lesen.

## Optionen

- Ablage: (a) Ordner `.artefakte` im Workspace + Anweisung an den Agenten, (b) eigenes MCP-Werkzeug „Ansicht zeigen“, (c) jede vom Agenten geschriebene `.html`-Datei.
- Darstellung: (a) `srcdoc` im Rahmen — erbt die CSP der App und findet keine relativ eingebundenen Bilder; (b) Asset-Protokoll von Tauri — Scope ist `$HOME/.verwalter/**`, Workspaces liegen woanders, und es setzt keine eigene CSP; (c) eigenes Tauri-Protokoll mit eigener CSP — zuerst gebaut, verworfen: Tauri 2.12 stuft Seiten eines von der App registrierten Protokolls als „lokal“ ein und gibt ihnen die Rechte der App, und unter Windows laufen die Init-Skripte samt `invoke` und Invoke-Key auch im iframe (Sicherheitsprobe); gehalten hat nur, dass WebView2 `postMessage` aus dem Rahmen nicht weiterreicht; (d) eigener HTTP-Server auf `127.0.0.1` — für Tauri entfernte Herkunft.

## Entscheidung

- **Ablageordner `.artefakte` je Vorhaben** (alle Sessions teilen den Workspace). Ein Artefakt ist jede `.html`/`.htm`-Datei direkt darin; Unterordner tragen Bilder, Skripte und Stylesheets.
- **Artefakt-Server auf `127.0.0.1`** (d) mit Zufalls-Port, `std::net`, keine neue Abhängigkeit: `http://127.0.0.1:<port>/<token>/<session-id>/<pfad>`. Jeder Befehl einer Seite von dort scheitert an der Rechteprüfung von Tauri (entfernte Herkunft, keine Remote-Capability) — unabhängig davon, ob WebView2 die Nachricht durchlässt. Der Zufalls-Token je App-Start hält Seiten im Browser draußen, die Prüfung des `Host`-Kopfes DNS-Rebinding. Ausgeliefert werden nur Dateien, deren kanonischer Pfad unter dem kanonischen Ordner liegt (sonst 403), mit eigener CSP je Antwort.
- **Rahmen mit `sandbox="allow-scripts allow-forms allow-modals"`, ohne `allow-same-origin`**: die Seite läuft mit undurchsichtiger Herkunft und kommt nicht an `parent`.
- **Internet über https erlaubt** (Bibliotheken von CDNs): der Agent, der das HTML schreibt, hat ohnehin eine Shell. `ipc:`, `http://ipc.localhost` und die Ordner anderer Sessions auf dem Server stehen nicht in der CSP.
- **Abfrage alle 2 s statt Dateiwächter**: keine neue Abhängigkeit, erfasst auch per Shell geschriebene Seiten.

## Konsequenzen

- Die App-CSP bekommt `frame-src http://127.0.0.1:*` (Port erst zur Laufzeit bekannt); Links nach außen öffnen sich im Rahmen nicht. Nachtrag 2026-11-11: Das Brücken-Skript fängt Klicks auf Links nach außen (`http`, `https`, `mailto`) ab und meldet sie per `postMessage`; `ArtifactFrame` prüft Herkunft und Protokoll und öffnet die Adresse im Standardbrowser (höchstens eine je Sekunde). Verweise auf andere Artefakte desselben Servers bleiben im Rahmen. Damit hängt das Verhalten nicht mehr davon ab, wie WebView2 oder WebKitGTK gesperrte Popups behandeln.
- Adressen der Vorschau gelten nur bis zum nächsten App-Start; die Oberfläche bekommt sie mit jeder Liste neu (`baseUrl`).
- Andere Programme des Benutzers können den Port finden, brauchen aber den Token; Seiten im `.artefakte`-Ordner sind ohnehin für jedes Programm des Benutzers lesbar.
- Formulare schicken nichts ab (`form-action 'none'`), Popups gehen nicht.
- Eine Seite mit `http://`-Quellen (ohne s) lädt diese nicht.
- Karte im Chat nur für `Write` des Hauptagenten; per Shell oder von Subagenten geschriebene Artefakte erscheinen nur im Reiter.
- Die Anweisung an den Agenten steht im selben `--append-system-prompt` wie die Scratchpad-Vorgabe (der autarke Agent liest nur eines).
- Jede HTML-Antwort trägt das Brücken-Skript für Kommentare (ADR 035).
- Nachtrag: Jede HTML-Antwort trägt am Anfang von `<head>` auch das Skript mit dem gespeicherten Zustand der Seite; die Sandbox bleibt unverändert ([ADR 081](081-zustand-von-artefakten.md)).
- Die Abschottung belegt die Sicherheitsprobe im Plan (`artifacts/sicherheitsprobe.html`), kein automatisierter Test.

## Nachtrag: Eingetragene Ordner außerhalb des Vorhabens (Taiga 139)

Ein Agent zeigt mit `artifact_show` auch eine Datei außerhalb von `.artefakte`, etwa im Vault. Die Sicherheitsgrenze bleibt eng: Nichts kommt über einen rohen Pfad aus einer Adresse oder einer Seite.

- **X1 Einstellung:** Die app-weite Liste „Ordner für Artefakte“ (`Settings.artifact_folders`, Vorbild `commandPrograms`) nimmt nur absolute, vorhandene Ordner und speichert den kanonischen Pfad. Unter Windows ohne `\\?\` (`\\?\C:\…` → `C:\…`, `\\?\UNC\server\…` → `\\server\…`, `filesystem::display_path`): In dieser Form stehen Ordner und Pfade in der Einstellung, im Tooltip, in der Anweisung, in Fehlertexten, in `.extern.json` und in `Artifact.externalDir`/`externalPath`. Eintragen kann nur der Host; ein Gast trägt keine Ordner ein.
- **X2 Zeigen:** `artifact_show` nimmt zusätzlich einen absoluten Pfad oder `~/…`. Der Kern löst ihn kanonisch auf, mit Symlinks, und nimmt ihn nur, wenn das Ziel eine Datei in einem eingetragenen Ordner ist. `..` und ein Symlink nach draußen scheitern daran. Verglichen wird mit dem gespeicherten Ordner, nicht mit einem neu aufgelösten: Wird der Ordner oder ein Elternordner später zum Symlink, wandert die Grenze nicht mit. Sonst lehnt er ab, mit Text und der Liste der Ordner. Eine Nebenfrage darf weiter nur auflisten.
- **X3 Liste:** Gezeigte Pfade stehen in der versteckten Datei `.artefakte/.extern.json` (ADR 047, ohne Migration): Schlüssel, Pfad, Session, Zeitpunkt, höchstens 200. Geschrieben wird unter einer Sperre im Prozess und atomar (temporäre Datei, umbenennen), damit gleichzeitige `artifact_show` keinen Eintrag verlieren. Die Datei selbst liefert der Server nie aus, weder einer Seite noch einem Gast: Sie nennt Pfade außerhalb des Vorhabens. Die Liste zeigt sie in der Gruppe „Extern“ unter dem Dateinamen, mit dem Ordner im Tooltip (`Artifact.externalDir`, `file` = `.extern/<schlüssel>/<name>`). Liegt ein Pfad nicht mehr in einem eingetragenen Ordner, fehlt er in der Liste und wird nicht ausgeliefert.
- **X4 Ausliefern:** Der Server bekommt nie einen Pfad. Er löst `.extern/<schlüssel>/<name>` nur über den Schlüssel aus `.extern.json` auf und prüft X2 bei jeder Anfrage neu. `<name>` muss genau der Dateiname des Eintrags sein, ein Unterpfad ist nie einer; sonst 404 (`extern_files::resolve`, für Host und Gast). Die Oberfläche kodiert die Adresse je Bestandteil, so holen der Betrachter und relative Anfragen einer Seite unter `.extern/<schlüssel>/`, nie aus `.artefakte`. Es gibt nur GET und dieselbe CSP; Dateien neben der gezeigten Datei liefert er nicht. Markdown geht über den Betrachter, HTML bekommt die Brücke für Kommentare, aber keinen dauerhaften Zustand: `localStorage` lebt nur im Speicher der Seite (Zustandsskript mit leerem Stand, am Host wie beim Gast, ohne Frage nach Liste oder Zustand beim Host; die Oberfläche speichert für diese Dateien nichts, [ADR 081](081-zustand-von-artefakten.md)).
- **X5 Rechte:** Ansehen und Kommentieren (ADR 035), nicht ändern; ein Kommentar geht wie bei anderen Artefakten an die Session und nennt den echten Pfad der Datei. Auch „Im Chat besprechen“ nennt ihn; den vollen Pfad baut der Kern mit dem Trenner des Systems (`Artifact.externalPath`). „Mit Standardprogramm öffnen“ und „Im Explorer zeigen“ gibt es für diese Dateien nicht.
- **X6 Gast:** Ein Gast hat dieselben Rechte wie der Host (Nutzer, 2026-11-26): Er sieht die Gruppe „Extern“ in der `artifacts_list` des Hosts und holt die Datei über `remote_artifact` (`content::artifact_in`), das X2 und X4 am Host mit den Ordnern des Hosts prüft. Es gibt keinen neuen Befehl und damit keine neue Klasse in `remote/access.rs`.
- **X7 Anweisung:** Sind Ordner eingetragen und hat der Agent die Werkzeuge des Verwalters, nennt die Anweisung sie in einem Satz (`extern_folders_prompt`): Dateien daraus zeigst du mit `artifact_show` und dem absoluten Pfad.

Grenzen: Eine HTML-Datei aus einem Ordner lädt relativ eingebundene Nachbardateien nicht. Eine Datei, die ein Agent mit Schreibrechten außerhalb ändert, lädt der Reiter wie jede andere beim Abfragen neu.
