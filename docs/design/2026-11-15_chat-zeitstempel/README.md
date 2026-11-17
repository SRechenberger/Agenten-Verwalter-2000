# Design-Entwurf: Zeitpunkt an Einträgen im Chat

**Status:** gewählt am 2026-11-15 (Taiga-Story #106), umgesetzt; die Abnahme in der echten App steht aus. Entscheidung: [ADR 068](../../decisions/068-zeitpunkt-im-chat.md).

## Ansehen

- [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. `?theme=dark` zeigt dunkel, `?hover=1` alle beim Überfahren versteckten Zeiten.
- [umsetzung/](umsetzung/) hat die Bilder der Bildprüfung gegen den Mock (Fixture-Verlauf über drei Tage, alte Einträge ohne Zeit, kleine Seitengröße zum Nachladen).

## Entscheidungen (Benutzer, 2026-11-15)

- **U1 Alle Einträge** tragen einen Zeitpunkt: Nutzer, Session, Agent, Werkzeug-Gruppen, Gedanken, Aufgaben, Artefakte, Rückfragen, Fehler, Hinweise.
- **U2 Nachrichten immer, der Rest beim Überfahren.** Nachrichten sind Nutzer, Session und Agententext.
- **U3 Am Eintrag nur die Uhrzeit**, das Datum blockweise als Tagestrenner („Heute, Sonntag 15.11.“); der Tooltip nennt den vollen Zeitpunkt mit Sekunden.
- **U4 Alte Einträge** ohne Zeitpunkt zeigen nichts, auch nicht beim Überfahren.
- **U5 Links über dem Eintrag**, nicht im rechten Rand.

## Lage und Format

- Uhrzeit `HH:MM` (Ortszeit, `de-DE`), `--font-size-2xs`, `--color-fg-muted`, bündig mit dem Inhalt (32 px Rand), nicht markierbar.
- Nachrichten: eigene Zeile über dem Eintrag (14 px Zeile, 4 px Abstand); ein Block mit Agententext wird 18 px höher, ein Nutzerblock 14 px (sein Außenabstand fällt mit dem Abstand zusammen). Alles andere: schwebt beim Überfahren und bei Fokus im Block in der Lücke über dem Block (`top: -11px`) und ändert keine Höhe.
- Tagestrenner oben im Block-Rahmen, wenn der Tag gegenüber dem letzten Block mit Zeit wechselt; der erste Block mit Zeit bekommt einen nur, wenn kein älterer Verlauf mehr nachzuladen ist (sonst fiele er beim Nachladen weg); Großbuchstaben, nicht markierbar. In einem anderen Jahr steht das Jahr dabei („Dienstag 30.12.2025“).
- Eine Werkzeug-Gruppe zeigt die Zeit ihres ersten Aufrufs, eine Hintergrund-Zeile die ihres Aufrufs; der Block „Arbeitet …“ keine.

## Abweichungen der Umsetzung vom Mock

- Keine in Lage, Größe, Farbe und Format (Bild für Bild verglichen).
- Das Wachsen eines Blocks hängt von der Art ab: Der Außenabstand des Nutzerkastens (6 px) fällt mit dem Abstand unter der Zeile (4 px) zusammen, ein Nutzerblock wird deshalb um 14 px höher, ein Block mit Agententext um 18 px (der Mock verhält sich genauso).

## Abnahme in der echten App

- [ ] 1. Neue Nutzernachricht: Uhrzeit links über der Nachricht, sofort sichtbar; über der ersten Nachricht des Tages steht der Tagestrenner „Heute, …“.
- [ ] 2. Antwort des Agenten: Uhrzeit links über dem Text.
- [ ] 3. Werkzeug-Gruppe, Gedanken, Aufgabenliste, Artefakt, Rückfrage, Fehler, Hinweis: Uhrzeit erst beim Überfahren, links über dem Eintrag; auf der Uhrzeit nennt der Tooltip Datum und Sekunden.
- [ ] 4. Nachrichten von gestern oder früher: Tagestrenner „Gestern, Samstag 14.11.“ bzw. „Donnerstag 12.11.“; am Eintrag selbst steht nur die Uhrzeit.
- [ ] 5. Eine Session von vor der Änderung lädt ohne Fehler: Nutzernachrichten zeigen ihre Zeit (mit Tagestrenner), alles andere keine, auch nicht beim Überfahren.
- [ ] 6. Hell und dunkel: nichts abgeschnitten, die Uhrzeit bündig mit dem Inhalt; der Verlauf springt beim Überfahren nicht.
- [ ] 7. Über mehrere Blöcke markieren und kopieren: weder Uhrzeiten noch Tagestrenner im Text.
- [ ] 8. Rückfrage beantworten: Die Frage behält die Zeit der Frage.
- [ ] 9. Beim Hochscrollen werden ältere Einträge nachgeladen: Die Stelle im Verlauf bleibt, die Tagestrenner stimmen.
- [ ] 10. Wenn möglich: ein Gast auf einem zweiten Rechner zeigt die Zeiten des Hosts in seiner Ortszeit.
