# Design-Entwurf: Karten per Werkzeug (Taiga #235)

**Status:** gewählt am 2026-11-25, umgesetzt in [ADR 077](../../decisions/077-karten-per-werkzeug.md). Der Prototyp ist `prototyp-postfach-runde-2.html` (Runde 2 über Variante A des Postfachs, Taiga #187); aus ihm stammen die Texte der neuen Arten, „vom Agenten gemeldet“ und „nicht gemeldet“, „Ja, mach das“ und „Artefakt öffnen“. Das Mockup des Postfachs legt die Story des Postfachs ab; dieser Entwurf hält nur fest, was die Karten per Werkzeug festlegen.

## Arten

Der Agent meldet am Ende seines Zugs mit `verwalter_notify_user`, was der Nutzer jetzt tun kann oder muss. Es gibt fünf Arten, dazu den Rückfall.

| `kind` | Karte | Felder | Ton |
|---|---|---|---|
| `blocked` | Blockiert | `line`: das Hindernis | Fehler (rot) |
| `asks` | Fragt | `line`: die Frage; mit `questions` eine Rückfrage, die blockiert | wartet |
| `requests` | Bittet | `line`: worum der Agent bittet | wartet |
| `review` | Zum Ansehen | `line`: was zu sehen ist; `link` (Pflicht): Artefakt oder Adresse | Akzent |
| `done` | Fertig, mit `offer`: Fertig mit Angebot | `line`: das Ergebnis; `offer`: was der Agent anbietet | ok |
| (kein Aufruf) | Antwort liegt vor | Auszug der letzten Antwort | gedämpft |

`line` und `offer` haben höchstens 120 Zeichen und werden an einer Wortgrenze mit „…“ gekürzt, nicht abgelehnt. `offer` gibt es nur bei `done`, `link` nur bei `review`, `questions` nur bei `asks` (1 bis 4 Fragen, je 2 bis 4 Optionen, Form wie `AskUserQuestion`).

## Felder der Karte

`DashboardTask` bekommt `offer`, `link` und `reported` (alle `#[serde(default)]`: Ein Gast liest einen älteren Host). `reported` ist wahr, wenn die Art vom Agenten kommt, und ersetzt die Herkunft aus `answer_kind`. `DashboardTaskKind` bekommt `blocked` und `review`. Gespeichert wird ohne Migration in `answer_kind` (die Art) und `answer_line` (die Zeile; mit Angebot oder Verweis ein JSON-Objekt).

## Rang

blockierende Anfragen (`question`, `permission`, `sessionTool`), **Blockiert**, Fragt, Bittet, **Zum Ansehen**, Antwort liegt vor, Plan ändern?, Abbrechen?, Durch dich, Zurückgeben ausgeschöpft, Master ausgefallen, Fehler, Unterbrochen, Bereit, Fertig. Im Postfach stehen „Blockiert“ und „Zum Ansehen“ beide in der Stufe „Wartet auf dich“.

## Texte und Aktionen

- **Marke:** „✓ vom Agenten gemeldet“ im Kopf des Popups und „✓ gemeldet“ an der Karte; ohne Aufruf im Popup „Nicht gemeldet: Der Agent hat das Werkzeug verwalter_notify_user nicht gerufen.“
- **„Ja, mach das“** (Karte „Fertig“ mit Angebot) sendet „Ja, bitte: <offer>“ als Nachricht des Nutzers; die neue Eingabe räumt die Karte ab.
- **„Artefakt öffnen“** (Karte „Zum Ansehen“ mit Artefakt) wählt das Artefakt im Reiter „Artefakte“ der Session aus und springt dorthin. Ein Pfad mit `.artefakte/` gilt ab dem ersten Namen dahinter. **„Im Browser öffnen ↗“** bei einer `http(s)`-Adresse.
- **Rückfrage mit `questions`:** wie jede Rückfrage mit Auswahl ([ADR 045](../../decisions/045-rueckfrage-kasten.md)); die Antworten gehen als Ergebnis des Aufrufs zurück.
- **Stationen** melden nur `asks` mit `questions`; jede andere Art bekommt den Satz mit Verweis auf `implementation_complete_station`.
- **Flach** (Story „flacher Chat-Stil“): Das Angebot und der Verweis im Popup sind Zeilen mit 2-px-Linie links in der Farbe der Art, der Knopf hat 2 px Rundung.

## Messung

Werkzeugtreue mit der echten Kommandozeile (`notify-probe`, 18 Läufe je Modell): Opus 18 von 18, Sonnet 9 von 18, Haiku 10 von 18 ohne Verweigerung; Sonnet und Haiku melden kurze Ergebnisse (0 von 6) nicht, Opus schon (6 von 6). Die Sperre von `AskUserQuestion` (mindestens 17 von 18 je Modell) ist damit nicht erreicht; beide Wege bleiben offen.
