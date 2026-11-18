# Design-Entwurf: Dashboard

**Status:** freigegeben am 2026-11-17 (Taiga-Story 102, [ADR 071](../../decisions/071-dashboard.md)). Gebaut sind der Kern (Phase 102.1: Migration 31, Karten, Abläufe und Sessions als reine Funktion, `dashboard_get` und `dashboard_dismiss`, Antwort am Zugende, Sperre leerer Nachrichten) und die Oberfläche (Phase 102.3); die Einordnung (102.2) folgt.

## Zweck

Das Dashboard ist eine **Arbeitsliste**: Es zeigt, was auf dich wartet, und sonst nichts. Der Mock zeigt die Ansicht „Aufgaben“ mit den Karten, ihren Popups, die Spalte der Abläufe und der aktiven Sessions und die Wahl des Modells der Einordnung. Er ist die Vorlage der Oberfläche (102.3); die Regeln der Karten lebten im Mock nur als Beispieldaten und leben jetzt im Kern.

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig). Die gestrichelte Leiste links ist Simulation und gehört nicht zur App; alle Namen und Zahlen sind Beispieldaten.
- Sprungmarken: `#theme=light|dark&model=off|haiku&popup=<Nr>`, etwa `#theme=light&model=off&popup=4` (helles Schema, Modell der Einordnung aus, Popup der vierten Karte offen).

## Entscheidungen B1 bis B14 in Kürze

| # | Entscheidung |
|---|---|
| B1 | Alles, was auf dich wartet, wird eine Karte; der Verwalter erzeugt sie programmatisch, per Werkzeug fordert niemand eine an. |
| B2 | Die Liste entsteht im Kern aus den Zuständen, nicht in der Oberfläche; der Autopilot (#146) liest später dieselbe Liste. |
| B3 | Karten: Rückfrage, Rechte-Abfrage, Kasten (halten einen Agenten an); Plan ändern?, durch dich, Fehler, Master ausgefallen, Zurückgeben ausgeschöpft, Antworten aus Text; „Bereit“ nur ohne „Automatisch starten“; „Fertig“ ganz unten. |
| B4 | Reihenfolge: erst, was einen Agenten anhält, dann, was sonst auf dich wartet, dann bereite Stationen, zuletzt fertige Antworten; innerhalb davon die am längsten wartende vorn. |
| B5 | Eine Karte verschwindet, sobald der Zustand dahinter erledigt ist; Karten aus Textantworten bei einer Antwort von dir oder mit „Gelesen“, nicht beim Öffnen der Session. |
| B6 | Das Modell ordnet nur ein, wenn eine Antwort ohne offene Anfrage endet: „fragt“, „bittet“ oder „fertig“ mit Einzeiler. Standard Haiku, abschaltbar; ohne Modell erscheint „Antwort liegt vor“. |
| B7 | Stationen bekommen keine eingeordneten Karten; eine Antwort ohne Meldung geht an ihren Master („Wartet auf Master“). |
| B8 | Ein Fehler einer Station bleibt eine Karte für dich. |
| B9 | Rechts: Abläufe (eckiges Symbol, Zustand, Balken, „N offen“) und aktive Sessions ohne Ablauf. |
| B10 | Einstieg: Eintrag „Dashboard“ oben in der Leiste mit der Zahl offener Karten; die App startet darauf. |
| B11 | Gruppiert nach Vorhaben (Standard), nach Art oder nach Gruppe; dazu „Nur, was einen Agenten anhält“. |
| B12 | Das Popup einer Station „durch dich“ zeigt „vom Master freigegeben“. |
| B13 | Keine leeren Nachrichten: „Senden“ und im Kasten „Erlauben“ bleiben gesperrt, solange das Feld leer ist oder nur Leerzeichen enthält; `SessionRegistry::send` lehnt leeren Text ohne Anhänge und Kommentare ab. |
| B14 | Der Mock liegt mit der Umsetzung hier. |

## Abweichungen vom Mock

Gegenüber dem Mock **entfallen** „Als fertig markieren“ bei Fehlern (es gibt keinen Befehl; der Neustart geht wie im Chat) und „Merge“ (#134). Es **kommen dazu** die Karten „Unterbrochen“ (Session oder Station pausiert ohne scharfen Zeitgeber „Automatisch fortsetzen“, mit oder ohne Meldung) und „Abbrechen?“ (Anfrage eines Agenten, die Umsetzung abzubrechen) sowie „Fehler“ auch für Sessions ohne Station (D13).

## Abweichungen

Die gebaute Oberfläche (`src/features/dashboard/`) weicht so vom Mock ab:

- **Popups mit den Bausteinen der App.** Rückfrage, Rechte-Abfrage und Kasten zeigen `QuestionBlock` und `SessionToolBlock`, Plan-Änderung `PlanChangeCard`, „Abbrechen?“ `CancelRequestCard`, „Zurückgeben ausgeschöpft“ `StationReportSection`, „Master ausgefallen“ `MasterWarning` und `MasterPicker`. Grund: Eine zweite Fassung derselben Eingaben liefe auseinander (Ziffern, Entwürfe, Rechte). Folge: Die Knöpfe „Antworten“, „Übernehmen“, „Erlauben“ stehen im Baustein statt in der Fußzeile des Popups, der Baustein hat einen eigenen Rahmen im Popup, und der Master wird über „ändern“ gewählt statt über eine Liste im Popup. Der Kasten bietet „Erlauben“/„Ablehnen“ als Optionsknöpfe, ein Vorschlag für einen Agententyp zeigt nur eine Zeile.
- **„Durch dich“:** Das Ergebnis ist eine Zeile (wie in der Kartei) statt eines Textfelds; der Auftrag steht als Text, ohne Haken je Schritt (die Station kennt keine Schritte).
- **Alter** der Karte in der Form des Mocks („4 min“, „1 h 10 min“), darüber hinaus „2 d 3 h“.
- **Zeile im Popup-Kopf:** Vorhaben · Session oder Station · „seit …“; der Kopf trägt die Farbe der Art als linken Streifen.
- **Eintrag in der Leiste:** zwei mal zwei Kästchen statt „▦“; die Zahl steht in der Farbe „wartet“.
- **Antwort-Popup:** zeigt zusätzlich die Nachricht vor der letzten Antwort (aus `chat_history`).
- **Fehler** zeigt den Text der Karte; „Als fertig markieren“ entfällt (s. o.).
- Das Popup nimmt den Fokus auf das erste bedienbare Element (der Schließen-Knopf, wenn der Baustein keins hat).

## Stand der Sichtprüfung

Gesehen, Hell und Dunkel (Bilder im Scratchpad der Umsetzung): alle Arten von Karten, Gruppierung nach Vorhaben, Art und Gruppe, der Filter, die Abläufe in allen fünf Zuständen, die Sessions rechts und jedes Popup. Die Oberfläche lief dafür mit festen Beispieldaten in einem Browser (Befehle des Kerns als Attrappe). Einzeln gegen den Mock verglichen hat die Umsetzung vier Popups (Rückfrage, Antwort, Plan, Master); alle 17 Popups hat die Review einzeln in Hell und Dunkel mit dem Mock verglichen, die Abweichungen stehen oben. Die Gruppierung nach Vorhaben mit mehr als einer Gruppe ist nicht gesehen: Alle festen Daten gehören einem Vorhaben; die Gruppierung ist im Selbsttest belegt, der Probelauf in der echten App prüft sie. Die echte App lief mit Wegwerf-`HOME`, einer Kopie der Datenbank und der Attrappe der Kommandozeile, aber nicht mit dem letzten Stand dieses Zweigs: Gesehen ist dort der frühere Stand (Start auf dem Dashboard, Eintrag mit Zahl 3, Karten „Unterbrochen“); den Start mit dem endgültigen Stand sieht erst der Probelauf. Nicht gesehen: Klicks in den Popups der echten App (Antworten, Übernehmen, Erledigen), das Schließen eines Popups, wenn der Kern die Karte entzieht, und die Fehlermeldung nach „Übernehmen mit Änderungen“ oder „Abbrechen?“ mit nicht gelöschtem Ordner; das ist nur gelesen.
