# 045 — Rückfrage-Kasten als Ziehharmonika mit eigenen Ergänzungen

**Status:** angenommen · **Datum:** 2026-10-30

## Kontext

Stellt Claude per AskUserQuestion mehrere Fragen, stehen sie im Kasten „Claude wartet auf deine Entscheidung“ als Wand untereinander; wie viele schon beantwortet sind, sieht man nicht. Freitext im Eingabefeld gilt nur für alle Fragen gemeinsam, eine Ergänzung zu einer einzelnen Option oder Frage gibt es nicht. Rechte-Abfragen (Erlauben/Ablehnen) nutzen denselben Kasten und funktionieren gut.

## Optionen

Darstellung:

- **(a) Alle Fragen offen als Liste** — verworfen: bleibt bei vier Fragen lang.
- **(b) Ziehharmonika** — gewählt (Entwurf vom 2026-10-30): genau eine Frage ist aufgeklappt, jede andere ist eine Zeile mit Kurzname und Antwort bzw. Frage.
- **(c) Reiter** — verworfen: versteckt die Antworten der anderen Fragen.

Transport der Ergänzungen (Notiz je Option, eigene Antwort je Frage):

- **(d) Im Antworttext je Frage** — gewählt: erreicht den Agenten sicher und steht im Verlauf, ohne Umbau im Core.
- **(e) Eigenes Feld und Weitergabe als `annotations` an Claude Code** — verworfen: ob Claude Code diese Notizen dem Modell zeigt, ist nicht belegt, und der gespeicherte Antworttext verlöre sie.

## Konsequenzen

- `Question` trägt `header`, Claudes Kurzname der Frage, mit `serde(default)`: ältere Verläufe in `chat_entries` lesen sich ohne Migration, die Oberfläche zeigt dann „Frage <n>“. Rechte-Abfragen haben keinen Kurznamen.
- Ab zwei Fragen oder mit einer Mehrfachauswahl sendet nur „Antworten“ oder Enter; eine einzelne Frage in Einfachauswahl sendet weiter beim Klick.
- Ein Options-Label mit „(Recommended)“ oder „(Empfohlen)“ am Ende erscheint ohne Zusatz mit Plakette „Empfohlen“; gewählt und gesendet wird der Name ohne Zusatz.
- Der Antworttext ist für den Agenten Freitext mit festem Muster: Antworten je Frage mit ` · ` getrennt, gewählte Optionen einer Frage mit `, `.
- Rechte-Abfragen behalten Aufbau und Verhalten.
