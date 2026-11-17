# Design-Entwurf: Globale Warteschlange mit Prioritäten

**Status:** freigegeben am 2026-11-16 mit dem Plan „Globale Warteschlange“ (Taiga-Story 103, [ADR 068](../../decisions/068-globale-warteschlange.md)). Gebaut sind der Kern (Phase 103.1: Tor, Durchgang, Priorität je Vorhaben, Einstellung, „Parallel max.“ entfernt) und die Oberfläche (Phase 103.2); die Abnahme in der echten App folgt in 103.A.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Oben wählen „Szenario“ (Frei, Voll · es wartet etwas) und „Farbschema“ (System, hell, dunkel). „Ein Agent endet“, „Du schreibst im Chat“ und „Neue Station wird bereit“ spielen die Abläufe nach; ein Klick auf ein Vorhaben in der Leiste wählt es, die Stufen „hoch“, „normal“, „niedrig“ im Kopf setzen seine Priorität, das Feld in den Einstellungen die Grenze. Sprungmarken: `mockup.html#scen=frei`, `mockup.html#scen=voll&theme=dark`.

## Begriffe

- **Gleichzeitige Agenten max.:** die app-weite Grenze (Einstellung, leer = unbegrenzt, Standard 4); ersetzt „Parallel max.“ je Umsetzung.
- **Platz:** ein Agent mit Zug (startet, läuft, auch beim Warten auf einen Subagenten oder ein Skript), ein Start im Gang oder eine Station, die automatisch fortsetzt.
- **Warteschlange:** was der Verwalter von selbst starten würde und auf einen Platz wartet: Station, Nachricht, Fortsetzen.
- **Priorität des Vorhabens:** hoch, normal oder niedrig; ordnet die Warteschlange vor dem Vorrang der Station und der Wartezeit.

Verbindlich sind die Einträge im [Glossar](../../glossary.md).

## Gezeigt wird

| Teil | Inhalt | Phase |
| :- | :- | :- |
| Leiste der Vorhaben | je Vorhaben „n wartet“, bei hoch ▲, bei niedrig ▼; keine Knöpfe in der Zeile | 103.2 |
| Fuß der Leiste | Marke „n warten“, nur wenn etwas wartet; sonst „Alle Agenten laufen.“ | 103.2 |
| Kopf des Vorhabens | Priorität als drei Stufen | 103.2 (Befehl `project_set_priority` in 103.1) |
| Leiste der Umsetzung | „Plätze n von m (App)“ mit Füllstand, bei Überschreitung „über der Grenze: dein Start“, Link in die Einstellungen; kein Zähler „Parallel max.“ mehr | 103.2 (Zähler entfernt in 103.1) |
| Zeilen | „läuft“, „läuft · von dir gestartet“, „bereit · Warteschlange n“ (global), „Nachricht wartet · Platz n“ | 103.2 (Plätze aus `queuePosition` in 103.1) |
| Liste der Warteschlange | Rang, Was, Vorhaben, Stufe, wartet seit | 103.2 (`agent_queue` in 103.1) |
| Einstellungen · Agent | Zeile „Gleichzeitige Agenten max.“ | 103.2 (Einstellung `max_agents` in 103.1) |

## Entscheidungen des Nutzers

| # | Frage der Story | Antwort (2026-11-16) |
| :- | :- | :- |
| U1 | Was reiht sich ein? | Automatische Starts: Stationen, zugestellte Session-Nachrichten, „Automatisch fortsetzen“, Neustart bei Fehler. Eigene Nachrichten starten sofort, zählen aber mit |
| U2 | Wonach wird begrenzt? | Anzahl Agenten; leer = unbegrenzt. Die Speichergrenze bleibt Story 69 |
| U3 | Wo sitzt die Priorität? | Je Vorhaben, drei Stufen, vom Benutzer in der Oberfläche gesetzt |
| U4 | Bleibt „Parallel max.“ je Umsetzung? | Nein, nur die globale Grenze gilt |

## Annahmen

- Die Marke „n warten“ und die Liste erscheinen nur, wenn etwas wartet (Critical Rule 1); ohne Wartende bleibt der Fuß der Leiste leer bis auf den Satz.
- Die Priorität sitzt im Kopf des Vorhabens; die Zeile in der Leiste bleibt so hoch wie bisher und zeigt nur ▲ oder ▼.
- „Wartet seit“ ist eine relative Zeit; der Entwurf rechnet sie aus einer Folgenummer.
- Ein Start des Benutzers über der Grenze wird angezeigt, aber nicht verhindert.

## Platzhalter im Entwurf

- Namen der Vorhaben und Stationen sind ausgedacht.
- Die Simulation startet Wartende sofort, ohne die zehn Sekunden Countdown einer Station.
- Die Wartezeiten („vor n min“) folgen aus einer Zählung, nicht aus der Uhr.

## Abweichungen vom Entwurf

Stand nach Phase 103.2, verglichen Zustand für Zustand mit dem Entwurf (`#scen=frei|voll`, hell und dunkel):

- **Fuß der Leiste ohne Satz:** Ohne Wartende steht dort nichts, auch nicht „Alle Agenten laufen.“ Grund: Critical Rule 1 und der Auftrag der Phase (kein Platzhalter bei leerer Warteschlange).
- **Keine Zahl „n wartet“ je Vorhaben:** Die Zeile eines Vorhabens zeigt nur ▲ (hoch) oder ▼ (niedrig). Grund: Der Auftrag der Phase nennt nur die Marke; die Zeilenhöhe bleibt, und die Liste hinter „n warten“ nennt das Vorhaben je Eintrag.
- **Marke „1 wartet“:** Bei einem Eintrag steht die Einzahl, sonst „n warten“.
- **Marke geschlossen umrandet:** Der Entwurf zeigt sie gefüllt, weil seine Liste immer offen ist; in der App ist sie gefüllt, solange das Popover offen ist (`aria-expanded`).
- **Priorität im Kopf der Übersicht:** Die Schaltergruppe steht im Kopf des Vorhabens (Übersicht), nicht über der Leiste der Umsetzung; die Ansicht einer Umsetzung hat ihren eigenen Kopf mit der Umsetzung als Titel.
- **„läuft · von dir gestartet“ fehlt:** Der Core sagt nicht, wer einen Zug gestartet hat; die Zeile zeigt wie bisher „läuft · Dauer“.
- **„über der Grenze: dein Start“ auch nach dem Senken der Grenze:** Der Text erscheint, wann immer belegt > Grenze ist; die Oberfläche kennt den Grund nicht.
- **„Nachricht wartet · Platz n“ in der Sidebar kurz:** In der schmalen Session- und Stationszeile „✉ Platz n“ (sonst verschwände der Name); der volle Satz steht in Tooltip und Screenreader-Namen. In der Zeitleiste der volle Satz.
- **„wartet“ als echte Zeit:** „gerade eben“, „vor n min“, „vor n h“ aus `sinceMs`.
- **Einstellungszeile:** Die Beschreibung steht im ⓘ wie bei den Nachbarn, nicht unter dem Namen; eine ungültige Eingabe bleibt mit dem Satz „Eine ganze Zahl von 1 bis 9999; leer = unbegrenzt.“ stehen, Esc nimmt sie zurück. „Grenze in den Einstellungen“ öffnet die Kategorie Agent, ohne das Feld zu fokussieren.

### Stand der Sichtprüfung (2026-11-17)

Gesehen in der gebauten App (Wegwerf-`HOME`, eigenes Xvfb, Attrappe der Kommandozeile, Grenze 2, fünf Nachrichten im Posteingang, eine Umsetzung mit zwei Startstationen), je hell und dunkel: Marke und Liste, Priorität hoch und niedrig samt ▲/▼, „bereit · Warteschlange n“, „Nachricht wartet“, Leiste voll, über der Grenze und frei, Einstellungszeile samt Fehlerfall und Suche, frei ohne Marke. Nicht gesehen: ein Eintrag „Fortsetzen“ (nur im Selbsttest) und ein Start des Benutzers über der Grenze (gesehen nur über eine gesenkte Grenze).
