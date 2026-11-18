# 071 — Dashboard: eine Arbeitsliste aus den Zuständen des Kerns

**Status:** angenommen, Kern umgesetzt (Migration 31, Karten, Abläufe und Sessions als reine Funktion `dashboard::build`, Befehle `dashboard_get` und `dashboard_dismiss`, Antwort am Zugende, Sperre leerer Nachrichten, Bindings; Story Taiga #102, Phase 102.1); die Einordnung ist seit Phase 102.2 umgesetzt, die Oberfläche (102.3) folgt.

## Kontext

Wer mehrere Agenten parallel laufen lässt, sucht heute in der Leiste, was auf ihn wartet: eine Rückfrage hier, eine Station „durch dich“ dort, eine fertige Antwort irgendwo. Ctrl J springt nur zu Anfragen, `asking` und `waitsForYou`. Es fehlt eine Liste dessen, was auf den Benutzer wartet, in einer Reihenfolge, die der Verwalter kennt und nicht die Oberfläche erraten muss. Der Entwurf liegt unter `docs/design/2026-11-17_dashboard/`.

## Entscheidung

Vom Benutzer entschieden (B1 bis B14, Nachtrag in der Story vom 2026-11-17):

- **B1/B2:** Alles, was auf dich wartet, wird eine **Karte**; der Kern erzeugt sie aus seinen Zuständen, nicht die Oberfläche. Eine Aufforderung per Werkzeug gibt es nicht.
- **B3/B4:** Die Arten und die Reihenfolge stehen unten; blockierende Karten (ein Agent hängt an der Antwort) kommen zuerst, „Fertig“ zuletzt, innerhalb einer Stufe die am längsten wartende vorn.
- **B5:** Eine Karte verschwindet, wenn der Zustand dahinter endet. Karten aus Textantworten, Fehlern und Pausen einer Session gehen außerdem mit „Gelesen“; die Session zu öffnen genügt nicht.
- **B6:** Ein Modell ordnet die letzte Antwort ein (`asks`, `requests`, `done` samt Einzeiler); ohne Einordnung bleibt die Karte `answer` (Phase 102.2, Abschnitt „Einordnung“).
- **B7/B8:** Stationen bekommen keine eingeordneten Karten; eine Antwort ohne Meldung geht an den Master („wartet auf Master“). Ein Fehler einer Station bleibt eine Karte für dich.
- **B9:** Rechts stehen die Abläufe und die aktiven Sessions ohne Station.
- **B13:** Keine leeren Nachrichten.

Vom Architekten (D1 bis D18, hier die Teile des Kerns):

- **D1:** Ein Feature `dashboard` in jeder Schicht (`src-tauri/src/dashboard/`, `commands/dashboard.rs`, `src/lib/dashboard.ts`, `src/features/dashboard/`). Die Regeln sind eine reine Funktion über einer Momentaufnahme aus einfachen Daten, ohne Tauri; der CI-Filter nimmt `dashboard::` dazu.
- **D3/D4:** Der Kern hält beim Zugende einer Session ohne Station Zeitpunkt und Auszug der letzten Antwort fest (`answer_at`, `answer_excerpt`); das Dashboard lädt keinen Verlauf.
- **D5:** Migration 31 gibt `answer_at` nur Sessions ohne Station, die abgegeben und ungelesen sind: Nach dem Update erscheinen keine Karten für längst gelesene Antworten.
- **D10/D11:** `dashboard_get` nur lokal, kein neues Ereignis.
- **D12:** Abläufe: nicht abgebrochene Umsetzungen, nicht fertige und die der letzten 24 Stunden fertig gewordenen.
- **D13:** Neu gegenüber dem Mock: Karten „Unterbrochen“ und „Abbrechen?“, „Fehler“ auch für Sessions ohne Station; entfallen: „Als fertig markieren“, „Merge“.

## Vertrag in Kürze

`DashboardView { tasks, flows, sessions }` (ts-rs, `src/lib/bindings/Dashboard*.ts`). Eine Karte trägt eine stabile Kennung `<Art>:<Session|Umsetzung>[:<Station|Anfrage>]`, ihre Art, `blocking` (genau bei `question`, `permission`, `sessionTool`), `since`, den Titel, Vorhaben und Gruppe, Session, Umsetzung und Station mit ihrem Namen, die Anfrage mit ihrem Chat-Eintrag und bei „durch dich“ die Marke `released`. Die Zeiten zählen in ms seit der Epoche wie `last_activity_at`.

## Die Karten-Regeln

`WaitsForYou` einer Station ist überladen (durch dich, Antwort ohne Meldung, Pause ohne Zeitgeber, gemeldet ohne Rückgaben übrig). Die Karten lesen deshalb die rohen Zustände der Session und der Station (`dashboard::station_card_of`), nicht `StationState` allein; `StationState` und `urgency` bleiben, wie [ADR 070](070-master-session.md) sie baut.

| Art | Bedingung | `since` | verschwindet, wenn |
|---|---|---|---|
| `question`, `permission`, `sessionTool` | offene Anfrage einer Session, auch einer Station (`SessionState.pending`, Eintrag `entries[seq]`) | Zeitpunkt ihres Chat-Eintrags, sonst letzte Aktivität | beantwortet (Popup, Chat, Pause) |
| `asks`, `requests`, `done` | Session ohne Station, `Completed`, kein wartendes Skript, kein wartender Subagent, `answer_at` und `answer_kind` gesetzt | `answer_at` | du schreibst oder „Gelesen“ |
| `answer` | wie oben, `answer_kind` leer; Titel ist der Auszug | `answer_at` | wie oben |
| `error` (Session) | Status `Error` | letzte Aktivität (`Error` schreibt sie) | der Fehler endet, „Gelesen“ |
| `error` (Station) | Station im Fehler, kein Neustart („Bei Fehler“) ansteht | letzte Aktivität der Session | der Fehler endet |
| `paused` (Session) | Status `Paused`, kein scharfer Zeitgeber „Automatisch fortsetzen“ | Beginn der Ruhe (`idle_since`, nach App-Neustart letzte Aktivität) | sie läuft weiter, „Gelesen“ |
| `paused` (Station) | Station wartet auf dich, mit oder ohne Meldung, Session pausiert ohne scharfen Zeitgeber | wie bei der Session | sie läuft weiter |
| `planChange`, `cancelRequest` | `pendingChange`, `pendingCancel` einer Umsetzung | Zeitpunkt des Vorschlags oder der Anfrage | übernommen, bestätigt oder verworfen |
| `byYou` | Station „durch dich“ an der Reihe | letzte Aktivität der Umsetzung (Näherung: die jüngste Annahme eines Vorgängers) | erledigt |
| `returnsExhausted` | gemeldete Station, keine Rückgabe übrig | Zeitpunkt der Meldung | angenommen oder zurückgegeben |
| `masterMissing` | Umsetzung nicht fertig, nicht abgebrochen, ohne Master | letzte Aktivität der Umsetzung | neuer Master |
| `ready` | bereite Station, die nicht automatisch startet | letzte Aktivität der Umsetzung | gestartet |

- Karten kommen nur aus eigenen, nicht archivierten Vorhaben; Vorhaben eines Hosts liegen nie in der Registry.
- Eine Session-Karte ist sichtbar, solange `card_dismissed_at` fehlt oder vor `since` liegt.
- Eine beendete oder abgebrochene Umsetzung hat keine Karten außer den Anfragen ihrer Sessions.
- Reihenfolge: blockierende; dann `asks`, `requests`, `answer`, `planChange`, `cancelRequest`, `byYou`, `returnsExhausted`, `masterMissing`, `error`, `paused`; dann `ready`; zuletzt `done`. Je Stufe `since` aufsteigend, bei Gleichstand die Kennung.

## Abläufe, Sessions, Befehle

- **Abläufe:** jede nicht abgebrochene Umsetzung eigener Vorhaben, die nicht fertig ist oder in den letzten 24 Stunden fertig wurde. `state` nach der ersten zutreffenden Regel: `error` (eine Station im Fehler), `waitsForYou` (eine Karte der Umsetzung), `waitsForMaster` (eine Station gemeldet oder ohne Meldung abgegeben), `done`, sonst `running`; `open` zählt die Karten mit ihrer Kennung. Nicht fertige zuerst nach letzter Aktivität absteigend, fertige danach.
- **Sessions:** Sessions ohne Station aus eigenen Vorhaben, nur aktive: `running` (startet, läuft, auch mit wartendem Skript oder Subagent), `asking`, `waitsForYou` (offene Session-Karte außer `error`), `error`. Nach letzter Aktivität absteigend.
- **Befehle:** `dashboard_get() -> DashboardView` (Klasse G, nur lokal), `dashboard_dismiss(sessionId)` (Klasse C mit Session-ID): setzt `card_dismissed_at` auf jetzt und sendet `session://changed`.
- **Momentaufnahme** (`SessionRegistry::dashboard_snapshot`): erst die Datenbank (Gruppen, Umsetzungen über den Weg der Ansicht), dann jede Session einmal und kurz unter ihrer Sperre, nie zwei zugleich; der Verlauf wird nicht geladen (die Anfragen kommen aus `pending`, ihr Eintrag aus `entries[seq]`, soweit er im Speicher liegt).

## Antwort am Zugende und leere Nachrichten

Endet ein Zug einer Session ohne Station mit `TurnEnd::Completed`, setzt `end_turn` `answer_at` auf die Zeit, die das Zugende als `last_activity_at` schreibt, `answer_excerpt` auf die erste nicht leere Zeile der letzten `Text`-Antwort des Zugs (getrimmt, höchstens 160 Zeichen, mit „…“, wenn gekürzt) und löscht `answer_kind` und `answer_line`. Pause, Kontingent, Fehler und Abbruch setzen `answer_at` nicht. Ein Zugende mit wartendem Skript oder Subagent zeigt erst danach eine Karte, weil die Regel am Status der Momentaufnahme hängt. Beginnt ein neuer Zug (Nachricht des Benutzers oder einer Session, Fortsetzen), leert `note_input` alle vier Felder der Antwort und speichert; so zeigt eine Session nach Archivieren und Zurückholen keine Karte mit dem Auszug eines früheren Zugs.

`SessionRegistry::send` lehnt Text ab, der leer ist oder nur aus Leerzeichen besteht, wenn weder Anhänge noch Kommentare dabei sind („Leere Nachricht: Text, Anhang oder Kommentar fehlt.“). Der Satz kommt als `CommandError::Implementation`, wie die übrigen Sätze über Eingaben („Plane Vorhaben“ braucht einen Auftrag); eine neue Variante hätte jede Behandlung der Fehler in der Oberfläche berührt. Dieselbe Prüfung mit demselben Satz steht am Anfang von `create_project` (Vorhaben anlegen; der Auftrag ist die erste Nachricht, ein Anhang genügt wie bei `send`); einziger Aufrufer ist der Befehl `project_create`, den Oberfläche und Fernsteuerung gemeinsam nutzen.

## Grenzen

- **Gäste sehen nur Lokales:** `dashboard_get` ist Klasse G; Karten aus Vorhaben eines Hosts erscheinen nicht, eine Zusammenführung über Hosts wäre eigene Arbeit.
- **Ctrl J behält seine Liste** (`waitingQueue.ts`); das Dashboard ersetzt sie nicht.
- **Wieder laufende fertige Station:** Taiga #152 bleibt offen und gehört nicht hierher.
- **Nach einem App-Neustart** steht jede vorher aktive Session auf `Paused` und bekommt eine Karte „Unterbrochen“; „Gelesen“ räumt sie ab.
- **`since` von `byYou`, `ready` und `masterMissing`** ist die letzte Aktivität der Umsetzung, nicht der Zeitpunkt, an dem die Station an der Reihe wurde; der Kern speichert diesen Zeitpunkt nicht.
- **Die Bestandsregel der Migration** kennt nur `completed` und `seen_at`; eine Session, die vor dem Update abgegeben hat und gelesen wurde, bekommt keine Karte, auch wenn sie nie beantwortet wurde.

## Warum das kein KPI-Dashboard ist

[`docs/PROJECT.md`](../PROJECT.md) nennt „keine KPI-Dashboards“ als Nicht-Ziel: keine Kennzahlen, keine Verläufe, keine Auslastung. Das Dashboard hier zeigt keine Zahl über die Arbeit, sondern **die Arbeit, die auf dich wartet**: Karten, die verschwinden, wenn sie erledigt sind. Es ist eine Arbeitsliste, kein Messinstrument; „Chat first“ bleibt, denn eine Karte öffnet das Popup oder den Chat, sie ersetzt ihn nicht.

## Einordnung

Eine Karte `answer` („Antwort liegt vor“) zeigt nur den Anfang der Antwort. Ein kleines Modell ordnet sie ein: `asks` (der Agent fragt oder verlangt eine Entscheidung), `requests` (der Agent bittet um eine Handlung außerhalb der Session) oder `done` (Bericht, nichts erwartet), dazu ein deutscher Einzeiler für die Karte. Aus `answer` wird dann `asks`, `requests` oder `done` mit dem Einzeiler als `title` (B6).

- **Auslöser:** `update` startet die Einordnung, wenn ein Zug mit `TurnEnd::Completed` endet, die Session keine Station ist, keine Anfrage offen ist und die Einstellung `answer_model` nicht aus ist. Pause, Kontingent, Fehler und Abbruch lösen nichts aus. Die Eingabe sammelt `end_turn` unter der Session-Sperre aus `entries` im Speicher; danach läuft alles ohne Sperre.
- **Eingabe** (`dashboard::classify::collect`): die Nachricht, die den Zug begann (der letzte `User`- oder `SessionMessage`-Eintrag vor den `Text`-Einträgen des Zugs, höchstens 2000 Zeichen vom Anfang), und die letzte `Text`-Antwort des Zugs (höchstens 6000 Zeichen vom Ende). Die Probe `answer-probe --texts` schreibt dieselbe Eingabe durch dieselbe Funktion.
- **Prompt und Schema** stehen in `src-tauri/src/dashboard/classify.rs` (deutscher Systemprompt; Schema `{"kind": "asks" | "requests" | "done", "line": string}`). „Fertig. Soll ich noch X?“ ist `asks`.
- **Aufruf:** `print_program` und `ask_model` (wie das TL;DR) in einem eigenen Thread je Aufruf, Zeitlimit 60 s, nie unter einer Sperre. Je Session läuft höchstens ein Aufruf (`classify_running`); endet ein weiterer Zug währenddessen, wartet nur der jüngste in `classify_next` und läuft danach. Der Ort ist `sessions/registry/classify.rs`.
- **Speichern:** unter der Sperre nur, wenn `answer_at` noch dasselbe Zugende nennt (`same_turn`): Ein neuer Zug ändert `answer_at` und macht das Ergebnis wertlos; „Gelesen“ ändert es nicht. Dann setzt `update` `answer_kind` und `answer_line` (getrimmt, höchstens 120 Zeichen an einer Zeichengrenze), schreibt sie und sendet `session://changed`.
- **Rückfall:** Fehler, Zeitlimit, eine unlesbare Antwort, eine unbekannte Art oder ein leerer Einzeiler schreiben nichts; eine Zeile im Log der Session, die Karte bleibt `answer`.
- **Einstellung:** `Settings.answerModel: ModelId | null` (Schlüssel `answer_model`, Standard Haiku, leerer Wert = aus), global. Ohne Schlüssel gilt Haiku, auch in einer bestehenden Datenbank; eine Migration gibt es nicht. In den Einstellungen steht die Zeile „Modell der Einordnung“ im Abschnitt „Dashboard“ gleich hinter „Modell des TL;DR“, mit der Wahl „Aus“.
- **Kosten:** ein Aufruf je abgegebenem Zug einer Session ohne Station, mit Haiku unter 0,01 $ (gemessen 0,0001 bis 0,0013 $, im Mittel 0,0003 $).
- **LM Studio, Autark, Codex:** Dort antwortet das lokale Modell, gleich welches Modell die Einstellung nennt (`ask_model`); „Aus“ gilt auch dort.
- **Kein Nachholen:** Beim App-Start ordnet nichts nach. Eine Antwort, die zum Zeitpunkt des Zugendes nicht eingeordnet wurde (Modell aus, Aufruf gescheitert, App beendet), bleibt `answer`; auch die Karten, die Migration 31 für abgegebene Bestandssessions anlegt.
- **Warum automatisch trotz ADR 011:** [ADR 011](011-vorhaben-und-sessions.md) legt das TL;DR auf den Knopf, weil es bei jeder Antwort veralten würde und jeden Aufruf kostet. Die Einordnung gilt genau einem Zugende, veraltet nie (ein neuer Zug löscht sie), kostet wenige Zehntel Cent und ist der Zweck der Karte: Ohne sie müsste der Benutzer jede Antwort öffnen, um zu sehen, ob sie auf ihn wartet (B6). Wer das nicht will, schaltet sie aus.

### Messung

Wortlaut und Eingabe wie die App (`answer-probe --texts`, 12 Beispiele: je vier `asks`, `requests`, `done`, Deutsch und Englisch; Grenzfälle: Frage am Ende eines langen Berichts, „Fertig. Soll ich noch …?“, eine Bitte um Datei oder Programmstart außerhalb, ein Bericht ohne Frage). Je Beispiel drei Läufe mit `claude -p --model haiku --tools "" --setting-sources "" --no-session-persistence --output-format json`, `--system-prompt` aus `prompt.txt`, `--json-schema` aus `schema.json`, Eingabe aus der Datei auf der Standardeingabe, Wegwerf-`HOME` mit nur der verlinkten `.credentials.json`, ohne `CLAUDE*`-Variablen. Claude Code 2.1.296, Modell `claude-haiku-5-5`, 2026-11-18, keine Läufe mit 429.

**Ergebnis: 36 von 36 richtig**, Kosten zusammen 0,0119 $. Auffällig nur: Lauf 10-done-3 erfindet im Einzeiler „Git-Commits ungeprüft (kein Repo)“, was nicht in der Antwort steht; die Art stimmt. Der Einzeiler ist Beiwerk, das die Karte zeigt, und wird nicht als Entscheidung gelesen.

| Lauf | Soll | Ist | Einzeiler | Kosten ($) |
|---|---|---|---|---|
| 01-asks-1 | asks | asks | Soll ich die Zeilen in 500er-Transaktionen bündeln und die Ländertabelle cachen, oder den Import auf eine Staging-Tabelle umbauen? | 0.0003 |
| 01-asks-2 | asks | asks | Soll ich die Transaktionen bündeln und die Ländertabelle cachen, oder den Import auf eine Staging-Tabelle umbauen? | 0.0013 |
| 01-asks-3 | asks | asks | Soll ich nur Transaktionen bündeln und die Ländertabelle cachen (3,2 s statt 41 s) oder den Import auf eine Staging-Tabelle umbauen? | 0.0001 |
| 02-asks-1 | asks | asks | Soll ich zusätzlich den Migrationsguide für die umbenannten Config-Keys aktualisieren? | 0.0003 |
| 02-asks-2 | asks | asks | Soll ich auch den Migrationsleitfaden für die umbenannten Config-Keys aktualisieren? | 0.0005 |
| 02-asks-3 | asks | asks | Soll ich auch den Migrationsguide für die umbenannten Config-Keys aktualisieren? | 0.0001 |
| 03-asks-1 | asks | asks | Soll die Suche in der Projektliste zusätzlich mit Tastenkürzeln (Strg+F zum Öffnen, Esc zum Schließen) ausgestattet werden? | 0.0002 |
| 03-asks-2 | asks | asks | Soll die Suche in der Projektliste noch Tastenkürzel (Strg+F zum Öffnen, Esc zum Schließen) bekommen? | 0.0001 |
| 03-asks-3 | asks | asks | Soll ich die Tastenkürzel Strg+F und Esc für die Suche in der Projektliste noch ergänzen? | 0.0001 |
| 04-asks-1 | asks | asks | Retry-Strategie: ganzer Upload neu mit Backoff (a) oder Fortsetzen ab letztem Chunk (b), wofür Staging noch keinen Offset liefert? | 0.0003 |
| 04-asks-2 | asks | asks | Ganzer Upload neu mit Backoff, oder ab letztem Chunk fortsetzen? Staging liefert den Offset noch nicht. | 0.0002 |
| 04-asks-3 | asks | asks | Vollständiger Neuversuch mit Backoff oder Resume ab letztem Chunk? Staging-Server liefert den Offset noch nicht. | 0.0006 |
| 05-requests-1 | requests | requests | Starte pnpm dev, öffne http://localhost:5173 und sag, ob die Startseite ohne Fehler in der Konsole lädt. | 0.0005 |
| 05-requests-2 | requests | requests | Dev-Server mit pnpm dev starten, http://localhost:5173 im Browser öffnen und melden, ob die Startseite ohne Konsolenfehler lädt. | 0.0002 |
| 05-requests-3 | requests | requests | Bitte `pnpm dev` starten, http://localhost:5173 öffnen und melden, ob die Startseite ohne Konsolenfehler lädt. | 0.0003 |
| 06-requests-1 | requests | requests | Bitte service-account.json in den Ordner secrets/ des Repos legen, damit die Integrationstests gegen den echten Endpoint laufen können | 0.0005 |
| 06-requests-2 | requests | requests | Bitte service-account.json in den Ordner secrets/ des Repositories legen, damit die Integrationstests gegen den echten Endpoint laufen können. | 0.0003 |
| 06-requests-3 | requests | requests | Bitte service-account.json in den Ordner secrets/ des Repositories legen, damit die Integrationstests laufen können. | 0.0002 |
| 07-requests-1 | requests | requests | Im Dashboard des Zahlungsanbieters Live-Schlüssel erzeugen und als PAYMENT_KEY in .env.production eintragen. | 0.0005 |
| 07-requests-2 | requests | requests | Im Dashboard des Zahlungsanbieters Live-Schlüssel erzeugen und als PAYMENT_KEY in .env.production eintragen | 0.0003 |
| 07-requests-3 | requests | requests | Live-Schlüssel im Zahlungsanbieter-Dashboard erzeugen und als PAYMENT_KEY in .env.production eintragen | 0.0001 |
| 08-requests-1 | requests | requests | Bitte App mit leerer Settings-Datei starten, prüfen, ob sie startet, und ob die Warnung im Log erscheint. | 0.0005 |
| 08-requests-2 | requests | requests | Bitte App mit leerer Settings-Datei starten, prüfen ob sie startet und die Warnung im Log erscheint | 0.0002 |
| 08-requests-3 | requests | requests | Bitte App mit leerer Settings-Datei starten, prüfen ob sie startet und die Warnung im Log erscheint | 0.0001 |
| 09-done-1 | done | done | Vier ungenutzte Pakete (lodash, moment, classnames, uuid) entfernt; Build und 212 Tests grün, Bundle 74 KB kleiner. | 0.0003 |
| 09-done-2 | done | done | Vier ungenutzte Pakete (lodash, moment, classnames, uuid) entfernt; Build und 212 Tests grün, Bundle 74 KB kleiner. | 0.0006 |
| 09-done-3 | done | done | Vier ungenutzte Pakete (lodash, moment, classnames, uuid) entfernt; Build und 212 Tests grün, Bundle 74 KB kleiner. | 0.0001 |
| 10-done-1 | done | done | Auth-Modul: Session-Token laufen nach 12 statt 24 Stunden ab, Reset-Mail neu mit Request-IP, verifyToken gibt null statt zu werfen. | 0.0003 |
| 10-done-2 | done | done | Auth seit letzter Woche: Sessions 12 h statt 24 h, neues Reset-Mail-Template mit IP, verifyToken gibt null zurück. | 0.0007 |
| 10-done-3 | done | done | Auth-Modul: Session 12h statt 24h, Reset-Mail neu mit IP, verifyToken gibt null; Git-Commits ungeprüft (kein Repo) | 0.0004 |
| 11-done-1 | done | done | Alle 42 Tests sind grün (Dauer 8,3 s). | 0.0005 |
| 11-done-2 | done | done | Alle 42 Tests sind grün (Dauer 8,3 s). | 0.0001 |
| 11-done-3 | done | done | Alle 42 Tests laufen grün, Dauer 8,3 s. | 0.0001 |
| 12-done-1 | done | done | Umbenennung fmtDate zu formatDate abgeschlossen: 23 Stellen in 9 Dateien, Type-Check und Lint laufen durch. | 0.0005 |
| 12-done-2 | done | done | Helper fmtDate in 9 Dateien zu formatDate umbenannt (23 Stellen), Typcheck und Lint bestehen; Changelog unberührt. | 0.0002 |
| 12-done-3 | done | done | Helper fmtDate in 9 Dateien (23 Stellen) zu formatDate umbenannt; Typecheck und Lint laufen durch, Changelog unverändert. | 0.0001 |

## Oberfläche

Folgt (Phase 102.3).

## Was das in früheren ADRs überholt

Nichts. [ADR 070](070-master-session.md) bleibt in Kraft: Das Dashboard liest `StationState`, `report`, `returnsLeft`, `released` und `masterMissing` der Ansicht, ändert sie aber nicht.

## Tests

`cargo test --manifest-path src-tauri/Cargo.toml --lib -- implementation:: agent_types:: dashboard::` führt die Regeln unter `dashboard::` mit (Karten je Art mit Gegenfall, Reihenfolge, Kennungen, Abläufe, Sessions, Auszug, Migration 31); die Einordnung (Auslöser, Eingabe, veraltetes Ergebnis, Einzeiler, Einstellung) unter `dashboard::classify::`; die Sperre leerer Nachrichten und das Festhalten der Antwort am Zugende laufen unter `sessions::registry::tests::`.
