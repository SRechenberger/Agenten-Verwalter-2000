# 081 — Zustand von Artefakten bleibt erhalten

**Status:** umgesetzt (Taiga 248, Task 280; keine Migration) · **Datum:** 2026-11-26

## Kontext

Eine interaktive HTML-Seite im Reiter „Artefakte“ läuft im abgeschotteten Rahmen ohne `allow-same-origin` ([ADR 026](026-artefakte.md)). `localStorage` wirft dort einen `SecurityError`, und ein Formular schickt nichts ab. Was der Nutzer anklickt, ging beim Wechsel in den Chat, beim Neuladen nach einer Änderung der Datei und beim Neustart verloren. Ein Agent sah nur die HTML-Datei, nicht die Häkchen, und konnte einen Testplan nicht auswerten.

## Entscheidung

- **Schnittstelle: `localStorage`.** Der Artefakt-Server setzt in jede HTML-Antwort ein Skript (`artifacts/state.js`) an den Anfang von `<head>`, vor jedes Skript der Seite (`with_state` in `server.rs`; die Brücke hängt weiter am Ende von `body`). Es überschreibt `window.localStorage` per `Object.defineProperty` mit einem Speicher, der schon gefüllt ist und synchron arbeitet (`getItem`, `setItem`, `removeItem`, `clear`, `key`, `length`). Die Probe `docs/design/2026-11-26_artefakt-zustand/probe.html` zeigte unter WebKitGTK, dass das im Rahmen ohne `allow-same-origin` greift; Windows (WebView2) prüft der Nutzer bei der Abnahme. Wirft `defineProperty`, meldet das Skript es in der Konsole, und die Seite läuft wie bisher ohne dauerhaften Zustand. Zeigt die Probe unter WebView2 FAIL, folgt ein Nachpatch mit `window.verwalter.storage` und derselben Schnittstelle; die Anweisung nennt dann diesen Namen.
- **`sessionStorage`** wird ersetzt und lebt nur im Speicher des Rahmens: Seiten damit brechen nicht mehr ab, dauerhaft ist nur `localStorage`. IndexedDB und Cookies bleiben unverändert gesperrt.
- **Ablage:** `.artefakte/.zustand/<artefakt>.json`, bei einer Website `<ordner>.json` (eine Datei für alle Seiten, wie eine Herkunft im Browser), mit dem vollen Dateinamen des Artefakts (`plan.html.json`). Der Inhalt ist ein JSON-Objekt Schlüssel → Zeichenkette. Der Ordner beginnt mit `.` und erscheint nicht in der Liste ([ADR 047](047-artefakte-alle-dateitypen.md)), liegt aber im Workspace und ist für jeden Agenten lesbar. Schreibt ein Agent Zahlen oder Objekte, gelten sie als ihr JSON-Text.
- **Die App wählt die Datei, nie die Seite (P6).** `setItem`, `removeItem` und `clear` melden den ganzen Stand sofort per `postMessage` `{ verwalter: 1, type: 'state', data }`. `ArtifactFrame` prüft die Quelle (`stateFromFrame`: nur das Fenster des eigenen Rahmens) und ruft `artifact_state_save` mit dem Artefakt aus seiner eigenen Auswahl, nicht aus der Nachricht. Der Core (`artifacts/state.rs`) prüft den Namen (ein Name direkt im Ordner, das Artefakt existiert, `.zustand` liegt im Ordner), nur Zeichenketten und höchstens 1 MiB; darüber wirft `setItem` einen `QuotaExceededError` (die Seite rechnet vor), und der Core lehnt es ab, die Datei bleibt. Geschrieben wird atomar: temporäre Datei im selben Ordner, `sync_all`, umbenennen. Während ein Speichern läuft, wartet nur der neueste Stand.
- **Maskierung:** Der Zustand steht als JSON im Skript, jedes `<` als `<`, damit weder `</script>` noch `<!--` das Element beenden oder verändern.
- **Neu laden (P5):** `artifacts_list` liefert je Artefakt `stateModifiedAt`, den Änderungszeitpunkt der Zustandsdatei. Ändert er sich und stammt er nicht vom eigenen Speichern dieses Rahmens (`artifact_state_save` gibt den Zeitpunkt zurück), lädt der Rahmen neu. Andere offene Ansichten (zweite Session, Gast, Vollbild) laden so mit dem neuen Stand neu. Ändert ein Agent die Datei, zeigt die Seite das nach höchstens 2 s.
- **Zurücksetzen:** Der Knopf „Zustand zurücksetzen“ mit Bestätigung steht in der Vorschau, solange ein Zustand existiert (`artifact_state_reset`).
- **Fernsteuerung (P8):** Ein Gast liest den Zustand über `remote_artifact` (Pfad `.zustand/<artefakt>.json`, dieselbe Prüfung wie jede Datei des Ordners) und speichert mit `artifact_state_save` beim Host. Beide Befehle sind Klasse C in `remote/access.rs` ([ADR 059](059-remote-sessions.md)): ein Gast nur als Inhaber, die Oberfläche des Hosts gesperrt, solange ein Gast steuert.
- **Anweisung:** `artifacts_prompt` nennt in allen Betriebsarten `localStorage` mit sprechenden Schlüsseln und JSON als Wert und den Pfad der Zustandsdatei zum Auswerten.
- **Kein MCP-Werkzeug:** Die Datei reicht, jeder Agent kann Dateien lesen.

## Verworfen

- Eine Tabelle in SQLite: Agenten könnten sie nur über ein neues MCP-Werkzeug lesen.
- Ein Schalter oder Typ je Artefakt: Seiten bleiben normale HTML-Dateien mit derselben Sandbox und CSP.
- `allow-same-origin` für den Rahmen: bräche die Abschottung ([ADR 026](026-artefakte.md)).

## Konsequenzen

- Umbenennen oder Löschen eines Artefakts lässt seine Zustandsdatei liegen; ein neues Artefakt gleichen Namens übernimmt sie. Aufräumen gehört nicht hierher.
- Artefakte aus Ordnern außerhalb des Vorhabens (Taiga 139) haben keinen dauerhaften Zustand.
- Betrachter-Seiten (Markdown, PDF, Bild …) bekommen keinen Zustand: sie sind nicht interaktiv.
- Eine Seite einer Website, die auf ein anderes Artefakt verlinkt, schreibt seinen Zustand in die Datei des angezeigten Artefakts, denn die Datei bestimmt der Rahmen aus seiner Auswahl.
- Eine Seite kann den Zustand anderer Artefakte per `fetch` lesen (`.zustand/` liegt im Ordner, den die CSP freigibt), nicht schreiben.
