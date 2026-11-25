# 077 — Karten per Werkzeug: `verwalter_notify_user` statt Einordnung durch ein Modell

**Status:** umgesetzt (Taiga 235, Task 239; keine Migration; Messung zur Werkzeugtreue unten) · **Datum:** 2026-11-25

## Kontext

Das Dashboard ([ADR 071](071-dashboard.md)) zeigte am Zugende einer Session ohne Station nur „Antwort liegt vor“. Ein kleines Modell (Haiku, Einstellung „Modell der Einordnung“) las die letzte Textantwort und machte daraus `asks`, `requests` oder `done` mit einem Einzeiler. Das kostete einen Aufruf je Zug, scheiterte still (Zeitlimit, Modell „Aus“) und riet, was der Agent selbst weiß: ob er etwas fragt, bittet, zeigt oder nicht weiterkommt. Es gab keine Arten für „Blockiert“, „Zum Ansehen“ und „Fertig mit Angebot“. Eine Rückfrage mit Auswahl ging über `AskUserQuestion`, das die Kommandozeile abfängt ([ADR 045](045-rueckfrage-kasten.md)).

## Entscheidung

Der Agent meldet die Karte selbst. Das Session-Werkzeug `verwalter_notify_user` (Satz `SessionTools`, ohne Freigabe, nur Betriebsart Claude) ersetzt die Einordnung. Die Einordnung per Modell, die Einstellung „Modell der Einordnung“ und die Probe `answer-probe` mit ihren 14 Fällen entfallen; `ask_model` bleibt für TL;DR und Retro.

- **K1, Eingabe:** `kind` ∈ `asks | requests | done | review | blocked`; `line` (Pflicht, höchstens 120 Zeichen, gekürzt mit `trim_line` statt abgelehnt); `questions` (nur `asks`, Form wie `AskUserQuestion`, 1 bis 4 Fragen mit je 2 bis 4 Optionen); `offer` (nur `done`, höchstens 120 Zeichen); `link` (nur und zwingend bei `review`: Name eines Artefakts in `.artefakte` oder `https://…`). Jede Regel hat ihren Satz; ein Feld bei der falschen Art wird abgelehnt, nicht stillschweigend verworfen. Die Prüfung steht rein in `dashboard/notify.rs` (`parse`, `parse_for`).
- **K2, Ohne Frage:** Ein Aufruf ohne `questions` antwortet sofort „Vorgemerkt: <Art>. Die Karte erscheint, wenn dein Zug endet.“ und merkt sich die Karte im Speicher der Session (`SessionState::notified`). **Der letzte Aufruf im Zug gilt.** Erst `TurnEnd::Completed` macht daraus `answer_kind` und `answer_line` (`note_answer`); Pause, Kontingent, Fehler, Abbruch, ein Zug des Verwalters (`/compact`) und jede neue Eingabe verwerfen ihn. Eine Station meldet nichts ohne Rückfrage (siehe K5).
- **K3, Mit `questions`:** Der Aufruf blockiert wie die heutige Rückfrage und gibt die Antworten zurück (`{"answers": {"<Frage>": "<Antwort>"}}`). Die Karte ist eine gewöhnliche Rückfrage der Art `AskUser` mit allem aus ADR 045 (Ziehharmonika, Notiz, eigene Antwort, Freitext im Eingabefeld); `ToolRequest::AskUser` (`#[ts(skip)]`) sagt dem Kern, dass die Antwort über den Kanal des Aufrufs statt an den Agenten geht, und bleibt aus dem Eintrag heraus. Pause, Abbruch und Ende des Prozesses lassen den Kanal fallen, der Aufruf endet mit „Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.“ und nichts passiert. Eine Antwort mit falscher Zahl von Auswahlen weist der Kern ab und lässt die Rückfrage offen.
- **K4, Rang:** `blocked` direkt nach den blockierenden Anfragen, vor `asks`; `review` direkt nach `requests`. Die Reihenfolge in `level`: blockierend (`question`, `permission`, `sessionTool`), `blocked`, `asks`, `requests`, `review`, `answer`, `planChange`, `cancelRequest`, `byYou`, `returnsExhausted`, `masterMissing`, `error`, `paused`, `ready`, `done`.
- **K5, Stationen:** Nur `asks` mit `questions`. Jede andere Art bekommt „Eine Station meldet nur Rückfragen mit questions. Dein Ergebnis meldest du mit implementation_complete_station.“ Stationen bekommen weiter keine Karten aus Antworten (ADR 071 B7).
- **K6, Rückfall:** Ruft der Agent das Werkzeug nicht, bleibt die Karte `answer` („Antwort liegt vor“); `DashboardTask.reported` ist dann falsch, im Popup steht „nicht gemeldet“. `reported` ersetzt die Herkunft aus `answer_kind`.
- **Aktionen (Oberfläche, heutige Karten und Popups):** „Ja, mach das“ an einer fertigen Karte mit Angebot sendet „Ja, bitte: <offer>“ als Nachricht des Benutzers; die neue Eingabe räumt die Karte ab. „Zum Ansehen“ öffnet einen Artefaktnamen im Reiter „Artefakte“ der Session (`Artefakt öffnen`; steht `.artefakte/` im Pfad, gilt der erste Name dahinter) und eine `http(s)`-Adresse im Standardbrowser (`Im Browser öffnen ↗`). Der Kopf des Popups und die Karte nennen „✓ vom Agenten gemeldet“. Texte und Anordnung stammen aus dem Prototyp `prototyp-postfach-runde-2.html`; die Oberfläche des Postfachs baut Taiga 187; die Änderung hier betrifft nur die heutigen Karten und Popups.
- **Neue Felder an `DashboardTask`:** `offer`, `link`, `reported`, alle mit `#[serde(default)]`: Ein Gast liest einen älteren Host. `DashboardTaskKind` bekommt `blocked` und `review`, `AnswerKind` dieselben zwei Werte.

## Keine Migration

`answer_kind` und `answer_line` bleiben die Spalten der Karte. `answer_kind` trägt die fünf Texte; `answer_line` trägt die Zeile als einfachen Text und, nur wenn ein Angebot oder ein Verweis dazukommt, ein JSON-Objekt `{"line", "offer", "link"}` (`notify::stored_line` und `from_stored`; ein Text, der kein solches Objekt ist, ist die Zeile). Ältere Zeilen aus der Einordnung lesen sich unverändert. Die Zeile `answer_model` einer bestehenden Datenbank bleibt liegen und wirkt nicht mehr.

## Der Satz in `session_tools_prompt` (V1)

Satz 1 nennt `mcp__verwalter__verwalter_notify_user` neben den anderen Werkzeugen; danach steht der neue Satz: „Endet dein Zug mit einer Frage, einer Bitte, etwas zum Ansehen, einem Hindernis oder einem Ergebnis für den Nutzer, sag es ihm vorher mit mcp__verwalter__verwalter_notify_user. Rückfragen mit Auswahl stellst du nur über dieses Werkzeug (Feld questions).“ Die übrigen Sätze bleiben.

## Messung (F4): Werkzeugtreue und der Satz mit Haiku

Aufbau: `notify-probe` (`cargo run --manifest-path src-tauri/Cargo.toml --example notify-probe -- <Ausgabeordner> --model <Modell>`), die echte Kommandozeile `claude` 2.1.296 im Modus Auto mit der Zeile der App, `session_tools_prompt` und einem MCP-Stand-in, das die Werkzeuge des Verwalters mit ihren echten Schemas anbietet; eine Rückfrage mit `questions` beantwortet es mit der ersten Option. Wegwerf-`HOME` mit verlinktem `.claude`, die `CLAUDE*`-Variablen der eigenen Session entfernt. Sechs Aufträge, je drei Läufe, also 18 Läufe je Modell, ein Zug je Lauf: zwei Rückfragen mit Auswahl (Speicherformat, Name), zwei Bitten (eine Datei, die nur der Nutzer hat; ein Testserver, den nur er startet), zwei Ergebnisse (17 mal 23; Hauptstadt von Australien). **Treu** heißt: Der Agent ruft `verwalter_notify_user` und nie `AskUserQuestion`. Der Wortlaut ist der Satz oben, unverändert.

| Modell | treu | Art passt | Fragen | Bitten | Ergebnisse | `AskUserQuestion` | Kosten |
|---|---|---|---|---|---|---|---|
| Sonnet (`claude-sonnet-5-5`) | **9 von 18** | 10 von 18 | 5 von 6 | 4 von 6 | 0 von 6 | 1 | 1,03 USD |
| Opus (`claude-opus-5-5`) | **18 von 18** | 18 von 18 | 6 von 6 | 6 von 6 | 6 von 6 | 0 | 2,33 USD |
| Haiku (`claude-haiku-5-5`) | 10 von 18 | 9 von 18 | 6 von 6 | 4 von 6 | 0 von 6 | 0 | 0,07 USD |

- **K6 der Planung ist nicht erfüllt.** Die Grenze für die Sperre von `AskUserQuestion` (`BLOCKED_TOOLS`) lag bei mindestens 17 von 18 Läufen je Modell, Sonnet erreicht 9. `AskUserQuestion` bleibt deshalb **offen**. Die Erinnerung des Wächters von „Plane Vorhaben“ (`PLAN_REMINDER_TEXT`) nennt beide Wege (`AskUserQuestion` und `verwalter_notify_user` mit `questions`); sie wurde mit dem neuen Text nicht neu gemessen ([ADR 062](062-plane-vorhaben.md) maß den alten).
- **Wo Sonnet ausfällt:** Bei Ergebnissen ruft es das Werkzeug nie (0 von 6; die Antworten sind eine Zeile lang, „17 × 23 = 391“), bei der Bitte um eine Datei in zwei von drei Läufen nicht (es sucht im Arbeitsverzeichnis und antwortet im Text), und in einem Lauf fragt es mit dem Werkzeug und danach zusätzlich mit `AskUserQuestion`. Wo es das Werkzeug ruft, ist es richtig: Jeder der zehn Aufrufe wurde von der Prüfung angenommen, und die Art passte jedes Mal.
- **Opus** nimmt in allen 18 Läufen das Werkzeug, mit der richtigen Art, und nie `AskUserQuestion`. Zweimal meldete es nach einer Rückfrage zusätzlich `done`; der letzte Aufruf gilt.
- **Der Satz mit Haiku, 18 Läufe, ohne Verweigerung:** In keinem Lauf lehnte Haiku den Satz oder das Werkzeug ab. Die Probe sucht Verweigerungswörter (keines gefunden); die acht Läufe ohne Aufruf las ein Mensch nach: Haiku antwortet im Text, ohne den Satz zu erwähnen oder abzulehnen. Es ruft das Werkzeug in zehn Läufen und nie `AskUserQuestion`; wie Sonnet meldet es kein Ergebnis (0 von 6) und bei der Bitte um eine Datei zweimal nicht. In einem Lauf folgte auf `blocked` noch `asks` (die Art passt dann nicht zur Bitte). Der Satz ist damit nach ADR 052 und 070 (kleinstes Modell, keine Verweigerung) gemessen.
- **Grenzen der Stichprobe:** 18 Läufe je Modell, kurze Aufträge ohne Repository, ein Zug je Lauf, die Verweigerung erkennt die Probe an Wörtern und ein Mensch liest die Läufe nach; ein Lauf mit ToolSearch-Umweg zählt wie jeder andere. Die Zahlen gelten für diesen Satz und diese Beschreibung des Werkzeugs; ein anderer Wortlaut müsste neu gemessen werden. Aussicht: Ein Satz, der auch kurze Ergebnisse und Bitten ausdrücklich nennt („auch bei einem Ergebnis in einer Zeile“), könnte Sonnet näher an die Grenze bringen; er ist nicht gemessen.

## Was das in früheren ADRs überholt

- [ADR 071](071-dashboard.md): B6 und der Abschnitt „Einordnung“ samt Messung, Einstellung und Probe (Nachtrag dort).
- [ADR 045](045-rueckfrage-kasten.md): Eine Rückfrage mit Auswahl hat einen zweiten Weg, `verwalter_notify_user` mit `questions` (Nachtrag dort). `AskUserQuestion` bleibt offen.
- [ADR 052](052-session-werkzeuge.md) und [ADR 070](070-master-session.md): `session_tools_prompt` bekommt den Satz oben; sein Wortlaut ist mit Haiku gemessen (siehe oben).

## Konsequenzen

- Codex und LM Studio kennen die Session-Werkzeuge nicht; dort entsteht nur die Karte „Antwort liegt vor“, wie schon vor der Änderung bei „Aus“.
- Ein Agent, der das Werkzeug nicht ruft, hat keine Karte „Fragt“ mehr, auch wenn seine Antwort mit einer Frage endet; sie erscheint als „Antwort liegt vor“ („nicht gemeldet“). Das ist die Kehrseite der Messung oben: Sonnet meldet kurze Ergebnisse nicht.
- Der Satz „Vorgemerkt“ kommt auch, wenn der Zug später mit Fehler endet; die Karte entsteht dann nicht. Der Agent erfährt das nicht, der Nutzer sieht den Fehler.

## Tests

`cargo test --manifest-path src-tauri/Cargo.toml --lib -- dashboard:: sessions::registry::tests:: notify_user:: settings::` (je Art eine Karte, `line` und `offer` auf 120 Zeichen, der letzte Aufruf im Zug, Pause, Fehler, Abbruch und `/compact` verwerfen, die Station und der Satz, die Rückfrage blockiert und gibt die Antwort zurück, ohne Aufruf `answer`, Rang der neuen Arten, kein Modellaufruf am Zugende, die Einstellung ist weg); in der Oberfläche `pnpm selftest` (`dashboardTexts`, `dashboardNest`, `chatTasks`); die Probe `notify-probe --selftest` ohne Kontingent. Geändert wurden die Tests der Einordnung: Sie entfielen mit dem Code (`dashboard/classify.rs`, vier Tests in `sessions/registry.rs`) oder sind auf die neue Regel umgestellt (Liste im Bericht der Lane).
