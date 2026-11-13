# Design-Entwurf: Umsetzung mit Constraints (Zeitleiste mit Kanten-Chips)

**Status:** gewählt am 2026-11-10, Entwurf 1 von dreien (Zeitleiste mit Kanten-Chips); überarbeitet am 2026-11-11 nach dem Review des Nutzers (Lane statt Phase, Konfiguration der Station). Entwurf 2 (Schichten-Graph) und 3 (Board nach Bereitschaft) sind verworfen bzw. zurückgestellt und stehen hier nicht. Gilt für die Ansicht der Umsetzung ([ADR 048](../../decisions/048-umsetzung.md)), sobald Stationen Constraints tragen; bis dahin bleibt die heutige Zeitleiste.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben schalten „Szenario“ (Mitten im Lauf / Frisch angelegt), „Agent“ (schlägt eine Plan-Änderung vor) und „Farbschema“ um. Gestrichelt umrandete Knöpfe sind Simulation und gehören nicht zur App.
- Der Entwurf nimmt an, dass die Agententypen (Taiga Story 55) und „Automatisch fortsetzen“ je Station (Story 56) schon umgesetzt sind.

## Begriffe

- **Lane** ersetzt in der Oberfläche den Begriff „Phase (Umsetzung)“: ein Strang aus Stationen mit Schlüssel und Titel, rein gliedernd. Die Reihenfolge kommt aus den Constraints, nicht aus der Lane. Folgen für Glossar, ADR 048 und die Werkzeuge (`phases`, `key`, `wave`) sind noch nicht gezogen.
- **Stufe** (Zeitleiste): die errechnete Ebene einer Station im Graphen; Stationen derselben Stufe sind „parallel möglich“.
- **Automatisch starten** (Leiste und je Station): die Station startet nach ihren Vorgängern von selbst (Countdown). Heißt in der App heute „Automatisch weiter“ (`auto_start`).
- **Automatisch weiter** (Leiste und je Station): die Station setzt nach leerem Kontingent und nach einem Fehler selbst fort. Entspricht „Automatisch fortsetzen“ aus Story 56, erweitert um den Fehlerfall. Die zwei Schalter heißen so, weil der Nutzer sie im Review so unterschieden hat; das weicht von den Beschriftungen heute ab.

## Gezeigt wird

### Zeile
- Liste in topologischer Reihenfolge, gruppiert nach Stufe. Die durchgehende Linie zwischen den Zeilen entfällt, weil sie eine Kette behauptet.
- Titel „Lane 2 · Review Core #6“, dahinter klein **Modell · Denkaufwand** (Tooltip nennt die Quelle: Station, Typ oder Standard; bei gestarteten Stationen die Werte der Session) und der Schalter **↻ Automatisch weiter** dieser Station. Bei fertigen Stationen ist ↻ gesperrt, die Werte bleiben sichtbar.
- Weicht „Automatisch starten“ einer Station von der Leiste ab, steht eine Marke („⏸ manuell“ / „▶ automatisch“).
- Station **durch dich**: eine **Hand** statt des Punktes, Marke „durch dich“.
- Zweite Zeile: „wartet auf“ und „danach →“ als Chips (grün mit ✓, wenn der Vorgänger fertig ist); eine Station ohne Vorgänger trägt „Startstation“.
- ▶, „Abbrechen“ und „Öffnen“ bleiben klein und verändern die Zeilenhöhe nicht.

### Kartei (Klick auf die Zeile)
Kopf mit **Zweck** (Beschreibung des Agententyps, bei „durch dich“: Aufgabe für dich), darunter vier Abschnitte, deren Kopfzeilen die Werte auch zugeklappt zeigen:
- **Ablauf:** „Wartet auf“ und „Nachfolger“ als bearbeitbare Chips. Der Nachfolger ist abgeleitet: „+ Nachfolger“ lässt die gewählte Station auf diese warten, „×“ löst die Kante. Zyklen sind ausgegraut, mit dem Weg.
- **Auftrag (Prompt):** Text (höchstens 1000 Zeichen), darunter **Änderungen durch Vorgänger** mit Quelle: ergänzt („Entfernen“) oder geändert (Original durchgestrichen, „Zurücksetzen“). **„Vorgänger dürfen: nur ergänzen · ergänzen und ändern · nicht anfassen“** steuert, was ein Vorgänger per `handoff` bzw. Plan-Änderung am Auftrag tun darf. Bei „durch dich“ heißt der Abschnitt **Anweisung** und enthält, sobald die Station an der Reihe ist, ein Ergebnisfeld und „Erledigt“.
- **Agent:** Bearbeiter (Agent / durch dich), **Typ**, **Modell**, **Denkaufwand** (je mit „Standard: …“, das den Wert des Typs bzw. der Einstellungen nennt), Zeile „Läuft mit …“ mit Quelle, **Skill**, **System-Prompt** (Rollentext des Typs, nur lesbar) und **Zusatz** nur für diese Station.
- **Automatik:** „Automatisch starten“ (wie Umsetzung / an / aus), „Automatisch weiter“ (nach leerem Kontingent), „Bei Fehler“ (anhalten / 1× neu starten / bis 3× neu starten).

### Was sich wann ändern lässt
| Zustand | Auftrag, Typ, Modell, Denkaufwand, System-Prompt, Skill, Kanten der Station, „Automatisch starten“ | „Automatisch weiter“, „Bei Fehler“ |
|---|---|---|
| wartet / bereit | änderbar | änderbar |
| läuft / Frage / Fehler | festgeschrieben 🔒 (Momentaufnahme beim Start; ein Modellwechsel in der Eingabeleiste der Session bleibt möglich) | änderbar |
| fertig | nur Ansicht, mit den Werten, mit denen die Station lief; Ergebnis sichtbar | nur Ansicht |

Ein Agent kann den **Auftrag** einer wartenden Station ergänzen oder (wenn die Station es erlaubt) ändern; **Typ, Modell, Denkaufwand und System-Prompt** ändert nie ein Agent, nur der Benutzer.

### Plan ändern?
Der Vorschlag eines Agenten (neue Station samt Kanten) zeigt **Typ, Modell, Denkaufwand und Prompt** zum Ändern vor dem Übernehmen („Vorschlag war: …“ bei jeder Abweichung). Alles Weitere steht nach dem Übernehmen in der Kartei.

### Neue Lane („+ Lane“ in der Leiste)
- Titel; je Station **Typ** (tester, developer, reviewer, integrator oder „durch dich“) mit dem **Zweck** darunter, **Name**, **Prompt** (mit der Vorgabe des Typs), **Modell** und **Denkaufwand**. Ein Wechsel des Typs übernimmt Name und Prompt-Vorgabe, solange sie nicht geändert wurden.
- „nacheinander“, „Wartet auf“ (leer = Startstation) und „Danach“ (noch nicht gestartete Stationen, die auf die neue Lane warten sollen).
- Die Vorschau nennt die Stufe der neuen Stationen und die Stationen, die nach hinten rücken; Zyklus, doppelter Name und fehlender Titel sperren „Anlegen“. Nach dem Anlegen steht die Lane an ihrer Stufe, mit der Marke „neu“ und im Fokus. Der Benutzer ändert den Plan direkt.

### Ablauf und Leiste
- Countdown 10 s, ▶ und „Abbrechen“; **Parallel max.** als Zähler (− n +) statt Auswahlliste; ist die Grenze erreicht, steht die Zeile in der **Warteschlange** („bereit · Warteschlange 1“, Zähler in der Leiste) und rückt selbst nach.
- „Automatisch starten“ und „Automatisch weiter“ in der Leiste; der zweite schaltet alle noch nicht fertigen Stationen, ↻ in der Zeile oder die Kartei nur eine (letzte Aktion gewinnt).
- **Fix-Runde:** „Mit Befund melden“ hängt Fix-Runde und zweites Review ein; die Nachfolger warten danach auf das zweite Review.
- Meldungen unten („Änderungen am Plan“); die Ersteller-Session wird bei „Umsetzung fertig“ und bei einem Befund benachrichtigt.
- Zustände: fertig, läuft, bereit, Countdown, wartet, durch dich, Frage offen, Fehler. Dunkles und helles Farbschema.

## Entscheidungen des Nutzers
**2026-11-10**
1. Der Nachfolger ist abgeleitet, gespeichert werden Kanten (`after`); „Nachfolger ändern“ ist ein Kanten-Edit.
2. Eine Station ohne `after` darf parallel zu anderen starten; den Anstoß gibt der Benutzer (▶) oder die Automatik.
3. Entwurf 1, als Click-Through-Mock.

**2026-11-11 (Review)**
4. „Lane“ statt „Phase“. Der Chip „+ Vorgänger“ entfällt an gestarteten Stationen.
5. Jede Station bekommt schon beim Anlegen Prompt, Modell und Denkaufwand; der Zweck (Typ) ist von Anfang an sichtbar, ebenso, was danach noch änderbar ist. Der Prompt lässt sich durch Vorgänger ergänzen oder ändern.
6. Die Kartei zeigt und ändert alle wichtigen Werte: Prompt, System-Prompt, Modell, Denkaufwand, Automatisch starten, Automatisch weiter (nach Fehler, leerem Kontingent). Die Zeile zeigt Modell, Denkaufwand und den Schalter „Automatisch weiter“.
7. Die Plan-Karte lässt Typ, Modell, Denkaufwand und Prompt des Vorschlags ändern. Station durch dich: Hand als Symbol. Kein weißer Text auf grauem Grund bei der Auswahl von „Parallel max.“.

## Gestaltung
- Maße, Farben und Schriften folgen `src/styles/theme.css` und den Stilen der bestehenden Zeitleiste; Farbe trägt nur der Zustand.
- **Keine nativen Auswahllisten:** Typ, Modell, Denkaufwand, Automatik und „Parallel max.“ sind Schaltergruppen bzw. ein Zähler. Das umgeht die Theme-Abweichung nativer Listen (Text auf Grund im falschen Schema) und zeigt alle Optionen auf einmal. `color-scheme` ist je Farbschema gesetzt.
- Der Chat bleibt die Hauptfläche; die Zeile bleibt einzeilig im Titel, die Konfiguration steht in der Kartei.

## Annahmen im Entwurf
- Story 55 (Agententypen mit Modell, Denkaufwand, Systemprompt, Skills; Prompt „brief“ höchstens 1000 Zeichen; Werte beim Start aufgelöst und gespeichert; „ohne Typ“ möglich) und Story 56 (Schalter je Umsetzung, einzeln überschreibbar, „letzte Aktion gewinnt“) sind umgesetzt. Die Typen im Mock (tester, developer, reviewer, integrator) und ihre Werte sind Beispiele; Standard laut Einstellungen: Opus, Max.
- **Neu gegenüber den Stories** (noch in keiner Story): „Automatisch starten“ je Station (Story 56 kennt nur den Schalter der Umsetzung); „Bei Fehler: neu starten“; „Vorgänger dürfen …“ mit Ergänzung und Änderung des Auftrags; „Zusatz“ zum System-Prompt je Station; der Umbau der Leistenbeschriftung.
- Ein von Hand gestartetes ▶ ignoriert „Parallel max.“. Ob das so bleibt, ist offen.
- Die Stufe einer Station ist die längste Kette bis zu ihr (Wurzeln = Stufe 1); die Reihenfolge innerhalb einer Stufe ist die Planreihenfolge.
- „Plan ändern?“ gibt es nur für Agenten; der Benutzer ändert Kanten und Werte direkt.
- **Die Warteschlange ist im Core nicht vorhanden** und hier nur simuliert (ein Sekunden-Tick im Browser). Was der Core dafür braucht, steht in der Story „Constraintbasierte Umsetzungsabläufe“: `max_parallel` am Plan; `schedule_ready` zählt aktive Stationen und startet nur die freien Plätze, in Planreihenfolge; Zählen und `begin_countdown` unter einer Sperre; neuer Haken, wenn eine Station-Session endet, ohne fertig gemeldet zu sein. Nach einem Neustart pausiert sie (Critical Rule 4). Die Grenze gilt je Umsetzung, das Kontingent ist kontoweit (ADR 051).
- Eingaben in der Kartei und in der Plan-Karte werden beim Tippen nicht neu gezeichnet; der Mock zeichnet erst nach dem Verlassen des Felds nach.

## Platzhalter im Entwurf
- Alle Namen, Nummern, Zeiten, Typen und Zähler sind Beispieldaten; die Laufzeit „12:00 min“ ist erfunden.
- „Lagebericht“ ist ohne Funktion (unverändert wie heute); „Anhalten nach dieser Session“, die Rückfrage der Umsetzung und die Einstellungsseite für die Typen sind nicht gezeigt.
- Der Entwurf rechnet Bereitschaft, Stufen und Zyklen im Browser selbst; in der App kommt beides aus dem Core (`implementation/schedule.rs`).

## Abweichungen vom Entwurf

- **„Automatisch weiter“ nach leerem Kontingent fehlt** (Core, 2026-11-11): weder als Schalter der Umsetzung noch je Station noch als Zustand. Es hängt an Story 56, die noch nicht umgesetzt ist. „Bei Fehler“ (anhalten, 1×, bis 3× neu starten) und „Automatisch starten“ je Station stehen im Core.
- **„Neu starten“ heißt: dieselbe Session.** Der Verwalter startet den Agenten der Session neu und schickt eine Nachricht zum Fortsetzen; die Station bleibt an ihre Session gebunden, die Momentaufnahme des Starts bleibt. Nach einem App-Neustart startet nichts neu.
- **Eine Plan-Änderung, die eine wartende Station ersetzt, verwirft deren Änderungen am Auftrag:** Den neuen Auftrag gibt der Planer.
