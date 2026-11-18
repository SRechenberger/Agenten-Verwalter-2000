# 071 — Dashboard: eine Arbeitsliste aus den Zuständen des Kerns

**Status:** angenommen, Kern umgesetzt (Migration 31, Karten, Abläufe und Sessions als reine Funktion `dashboard::build`, Befehle `dashboard_get` und `dashboard_dismiss`, Antwort am Zugende, Sperre leerer Nachrichten, Bindings; Story Taiga #102, Phase 102.1); die Einordnung (102.2) und die Oberfläche (102.3) folgen.

## Kontext

Wer mehrere Agenten parallel laufen lässt, sucht heute in der Leiste, was auf ihn wartet: eine Rückfrage hier, eine Station „durch dich“ dort, eine fertige Antwort irgendwo. Ctrl J springt nur zu Anfragen, `asking` und `waitsForYou`. Es fehlt eine Liste dessen, was auf den Benutzer wartet, in einer Reihenfolge, die der Verwalter kennt und nicht die Oberfläche erraten muss. Der Entwurf liegt unter `docs/design/2026-11-17_dashboard/`.

## Entscheidung

Vom Benutzer entschieden (B1 bis B14, Nachtrag in der Story vom 2026-11-17):

- **B1/B2:** Alles, was auf dich wartet, wird eine **Karte**; der Kern erzeugt sie aus seinen Zuständen, nicht die Oberfläche. Eine Aufforderung per Werkzeug gibt es nicht.
- **B3/B4:** Die Arten und die Reihenfolge stehen unten; blockierende Karten (ein Agent hängt an der Antwort) kommen zuerst, „Fertig“ zuletzt, innerhalb einer Stufe die am längsten wartende vorn.
- **B5:** Eine Karte verschwindet, wenn der Zustand dahinter endet. Karten aus Textantworten, Fehlern und Pausen einer Session gehen außerdem mit „Gelesen“; die Session zu öffnen genügt nicht.
- **B6:** Ein Modell ordnet die letzte Antwort ein (`asks`, `requests`, `done` samt Einzeiler); ohne Einordnung bleibt die Karte `answer`. Folgt in Phase 102.2.
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
| `paused` (Station) | Station wartet auf dich, ohne Meldung, Session pausiert ohne scharfen Zeitgeber | wie bei der Session | sie läuft weiter |
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

Endet ein Zug einer Session ohne Station mit `TurnEnd::Completed`, setzt `end_turn` `answer_at` auf die Zeit, die das Zugende als `last_activity_at` schreibt, `answer_excerpt` auf die erste nicht leere Zeile der letzten `Text`-Antwort des Zugs (getrimmt, höchstens 160 Zeichen, mit „…“, wenn gekürzt) und löscht `answer_kind` und `answer_line`. Pause, Kontingent, Fehler und Abbruch setzen `answer_at` nicht. Ein Zugende mit wartendem Skript oder Subagent zeigt erst danach eine Karte, weil die Regel am Status der Momentaufnahme hängt.

`SessionRegistry::send` lehnt Text ab, der leer ist oder nur aus Leerzeichen besteht, wenn weder Anhänge noch Kommentare dabei sind („Leere Nachricht: Text, Anhang oder Kommentar fehlt.“). Der Satz kommt als `CommandError::Implementation`, wie die übrigen Sätze über Eingaben („Plane Vorhaben“ braucht einen Auftrag); eine neue Variante hätte jede Behandlung der Fehler in der Oberfläche berührt.

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

Folgt (Phase 102.2).

## Oberfläche

Folgt (Phase 102.3).

## Was das in früheren ADRs überholt

Nichts. [ADR 070](070-master-session.md) bleibt in Kraft: Das Dashboard liest `StationState`, `report`, `returnsLeft`, `released` und `masterMissing` der Ansicht, ändert sie aber nicht.

## Tests

`cargo test --manifest-path src-tauri/Cargo.toml --lib -- implementation:: agent_types:: dashboard::` führt die Regeln unter `dashboard::` mit (Karten je Art mit Gegenfall, Reihenfolge, Kennungen, Abläufe, Sessions, Auszug, Migration 31); die Sperre leerer Nachrichten und das Festhalten der Antwort am Zugende laufen unter `sessions::registry::tests::`.
