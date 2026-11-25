# Design-Entwurf: Dashboard als Postfach (Taiga #187)

**Status:** gewählt am 2026-11-24 (Runde 1, Variante A) und 2026-11-25 (Runde 2), umgesetzt in [ADR 079](../../decisions/079-dashboard-als-postfach.md). `mockup.html` ist der Prototyp der Runde 2 (eigenständig, Hell und Dunkel umschaltbar, die gestrichelte Leiste oben ist Simulation); `runde-1.html` ist der Prototyp der Runde 1 aus dem Anhang der Story. Er zeigt Ordner, Gruppen, Abläufe mit Master, Stufen, zwei Zeilen je Eintrag, Gleiche zusammenfassen, beide Reihenfolgen, Mehrfachauswahl, Sammelaktionen, Tastatur, Lesebereich, Auf- und Zuklappen mit allen Aktionen, den flachen Stil und Rückgängig.

## Entscheidungen des Nutzers (Story, 1 bis 8 vom 2026-11-24)

1. **Variante A:** Das Dashboard wird ein Postfach mit Ordnern, Liste und Lesebereich. Variante B (Postfach statt der Liste links) ist verworfen.
2. **Die Karten entfallen**, es gibt keinen Schalter zurück.
3. **„Noch nicht angesehen“** (fett, Punkt vorn) speichert der Kern je Karte, damit Host und Gast gleich zählen. Ansehen räumt nichts ab; das macht weiter nur „Gelesen“.
4. **Rückgängig nach „Gelesen“:** Ein neuer Befehl setzt `card_dismissed_at` zurück.
5. **Gleiche zusammenfassen:** Einträge mit gleicher Art und gleichem Titel werden eine aufklappbare Zeile; standardmäßig an.
6. **Reihenfolge:** „Wichtigste zuerst“ (die des Kerns, Standard) und „Neueste zuerst“; die Stufen bleiben in beiden.
7. **Abläufe als Ordner** in der linken Spalte statt rechts, „Sessions ohne Ablauf“ darunter.
8. **Sammelaktionen** „Gelesen“, „Fortsetzen“ und „Starten“ für mehrere markierte Einträge.

Weitere Festlegungen des Prototyps: zwei Zeilen je Eintrag (Art, Absender, Vorhaben, ⇄ bei Host-Vorhaben, Alter; darunter der Titel); Stufen als Überschriften (hält einen Agenten an, wartet auf dich, Fehler und Pausen, bereit, fertig); Lesebereich statt Popup, nach „Gelesen“, „Senden“ oder einer Aktion rückt die Auswahl zum nächsten Eintrag; Ordner Alles, Hält Agenten an, Fragt und bittet, Abläufe, Fehler und Pausen, Fertig, darunter jede Umsetzung und jedes Vorhaben; Tastatur (j/k oder Pfeile, x, e, r oder Enter, 1–9, /, Strg+Enter); „Durch dich“ trägt den Namen der Station als Titel statt „Du bist an der Reihe“; die Leiste der App bleibt. Aus Runde 2 (Nutzer, 2026-11-24): Der Inhalt jedes Eintrags lässt sich in der Liste aufklappen, mehrere zugleich, mit denselben Möglichkeiten wie der Lesebereich; flacher Stil (#236, immer).

## Entscheidungen des Plans (P1–P7)

| | Entscheidung |
|---|---|
| P1 | Variante A; `seen_at` je Karte im Kern, Rückgängig, Gleiche zusammenfassen, zwei Reihenfolgen, Abläufe als Ordner, Sammelaktionen, Tastatur. |
| P2 | „Gruppiert nach“ (`DashboardChoice`) entfällt; die Gruppen der Leiste ordnen die Vorhaben im Rand (aufklappbar); „Nur, was einen Agenten anhält“ ist der Ordner „Hält Agenten an“. |
| P3 | Jeder Ablauf nennt seinen Master (`◆ Session 41`); mehrere Abläufe eines Vorhabens stehen nebeneinander (#217). |
| P4 | Die Popups im Chat bleiben; im Dashboard ersetzt der Lesebereich das Popup und nutzt dieselben Inhalte. |
| P5 | Tabelle `dashboard_seen(card_id, seen_at)`; ungelesen heißt `seen_at < since`; `dashboard_get` räumt ab; Befehle `dashboard_seen` und `dashboard_undismiss`, beide Klasse C. |
| P6 | Sammelaktionen rufen die vorhandenen Befehle der Reihe nach auf. |
| P7 | Aufklappen: ▸/▾ vorn, `o`, →, ←, „Alle aufklappen“/„Alle zuklappen“; mehrere zugleich; eine aufgeklappte Zeile ist der Lesebereich ohne Kopf; der Entwurf einer Antwort ist je Karte einer; offene Zeilen sind flüchtiger Zustand der Oberfläche; Aufklappen zählt als angesehen. |

## Abweichungen vom Prototyp

- Die Texte stammen aus der App: ohne Aufruf des Werkzeugs „Nicht gemeldet: Der Agent hat das Werkzeug verwalter_notify_user nicht gerufen.“; „Blockiert“ trägt den Untertitel im Kopf des Lesebereichs.
- In der Zeile steht kein Zusatztext nach dem Titel (der Prototyp hat Beispielwerte); „· Angebot“ steht an der Art.
- „Gleiche zusammenfassen“: „Alle aufklappen“ öffnet auch die zusammengefassten Zeilen, damit sichtbar wird, was aufgeklappt ist.
- Die Inhalte im Lesebereich sind die Bausteine der Popups (dieselbe Komponente), nur flach gestellt; ihre Anordnung weicht in Einzelheiten vom Prototyp ab (Knopfzeile unten, „Session öffnen“ links).
