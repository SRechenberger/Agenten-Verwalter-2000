# Entwurf Rückfrage-Kasten

| Datei | Zeigt | Phase |
|---|---|---|
| [entwurf-rueckfrage-kasten.html](entwurf-rueckfrage-kasten.html) | Den Kasten einer Rückfrage im Chat, umschaltbar zwischen „Heute“, „Liste“, „Ziehharmonika“ und „Reiter“, jeweils offen und beantwortet | Ziehharmonika: Phase 1 (Aufbau, Marker, aktive Frage, Fuß, kompakt beantwortet), Phase 2 (✎-Notiz, „Eigene Antwort …“) |

**Verbindlich** ist die Variante „Ziehharmonika“, vom User am 2026-10-30 freigegeben. „Liste“ und „Reiter“ sind verworfene Varianten (ADR 045), „Heute“ ist der Nachbau des Stands vor dem Plan.

**Ansehen:** Datei im Browser öffnen, keine Abhängigkeiten. Hell und dunkel folgen der Systemeinstellung. Ziffern 1–3, ✎, „Eigene Antwort …“, Enter und „Antworten“ funktionieren.

**Werte:** Farben, Schriftgrößen und Radien sind die semantischen Tokens aus `src/styles/theme.css` (hell und dunkel), im Entwurf als Hex-Werte ausgeschrieben. Die Umsetzung nimmt die Tokens, nicht die Hex-Werte.

**Platzhalter:** Inhalt ist eine echte Rückfrage aus einer Session; die Optionen von Frage 4 („Einstieg“) sind erfunden, um die Mehrfachauswahl zu zeigen. Die Bausteine-Tabelle im Entwurf führt „Hinweis am Eingabefeld“ und „Vorschau je Option“; beide sind nicht Teil des Plans.

**Neu erzeugen:** Die Datei ist die Quelle; sie wird von Hand gepflegt.
