# Wartende Nachrichten sichtbar, eigene verwerfen (Taiga-Story 218)

`mockup.html` ist der eigenständige Prototyp (hell und dunkel, Verwerfen-Variante A schaltbar zu B). Entscheidungen in [ADR 078](../../decisions/078-wartende-nachrichten.md).

## Entscheidungen

- **N1 Verwerfen.** Es steht an jeder eigenen wartenden Blase (einzeln) und im Reiter „Nachrichten“. Die Leiste über dem Eingabefeld behält nur „Jetzt senden“.
- **N2 Nach dem Verwerfen** bleibt der Eintrag als „verworfen“ stehen (Einträge lassen sich nicht löschen) und bietet „Text übernehmen“: Der Text geht ins leere Eingabefeld. Der Text geht nie an den Agenten.
- **N3 Anhänge** bleiben in `.anhaenge/`; der verworfene Eintrag nennt sie.
- **N4 Wettlauf.** Wer zuerst die Zeile in `session_inbox` löscht, gewinnt. Verwerfen setzt `queued` unter der Session-Sperre zurück und löscht dann die Zeile. Wer verliert, bekommt „Schon gesendet“ (`alreadySent`).
- **N5 Nur eigene** Nachrichten lassen sich verwerfen; fremde zeigt der Chat nur an.
- **N6 Fremde wartende Nachrichten** stehen im Chat gestrichelt wie eine `SessionMessage` (Absender, Betreff) mit der Marke des Grundes; darüber der Hinweis „n Nachrichten anderer Sessions warten auf diese Session“.
- **N7 Grund** (`waitsFor`): `slot` „Platz n“, `turnEnd` „bis der Zug endet“, `answer` „bis die Rückfrage beantwortet ist“, `paused` „pausiert“, `start` „bis die Session startet“; später `compact` „bis der Compact fertig ist“ (#215).
- **N8 Popover** der Warteschlange: Reiter „Plätze“ (wie bisher) und „Nachrichten“ (Von → An, Betreff, Grund, seit). Je Host ein Abschnitt.
- **N9 Gast.** Er verwirft in übernommenen Vorhaben (`chat_discard_queued`, Klasse C); `inbox_list` ist Klasse F und gefiltert wie `agent_queue`.

## Abweichung vom Prototyp

`WaitingMessage` (Vertrag V3) trägt keinen Text. Die fremde wartende Nachricht zeigt deshalb Absender, Betreff und Marke, aber nicht den Text, den der Prototyp zeigt.
