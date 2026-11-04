# Entwurf zur Umsetzung

| Datei | Zeigt | Phase |
|---|---|---|
| `entwurf-umsetzung.html` (+ `entwurf-umsetzung/app.css`) | Klickbarer Entwurf mit neun Abschnitten: 1 Zeitleiste und Kartei (Phase 2), 2 parallele Phasen (Phase 4), 3 Plan ändert sich (Phase 4), 4 Werkzeuge des Verwalters (Phase 1, 3, 4), 5 Rückfragen und Lagebericht (Phase 5), 6 Punkte und Phasen (Phase 2), 7 Regeln (Phase 2, 3), 8 Skills (Phase 7), 9 Entscheidungen und offene Punkte (README dieses Plans) | 1–7 |

**Ansehen:** Datei im Browser öffnen (offline, keine externen Bibliotheken); im Agenten-Verwalter liegt dieselbe Datei als Artefakt des Vorhabens, in dem der Plan entstand (`.artefakte/entwurf-umsetzung.html`).

**Werte:** Farben und Maße kommen aus `entwurf-umsetzung/app.css` (Tokens des Verwalters nachgebildet); in der App gelten die Tokens aus `src/styles/theme.css`, nicht die Werte der CSS-Datei — der Entwurf ist Kontrakt für Struktur, Sektionen und Schlüssel-Elemente (Phase 2 AK), nicht für Hex-Werte.

**Platzhalter:** Namen („Kontext-Leiste“, „Sprint 44“), Nummern und Zeiten sind erfunden; die Fragen auf den Karten sind Beispiele. Die Antwort auf eine Frage direkt auf der Karte (Abschnitt 2) wird nicht gebaut — Phase 2 leitet in den Chat (bewusste Abweichung, siehe Phase 2).

**Abweichende Werkzeugnamen:** Der Entwurf nennt `phase_describe` und `station_complete`; gebaut werden `implementation_describe_phase` und `implementation_complete_station` (ein Präfix je Werkzeugsatz, README „Festgelegte Entscheidungen“).

Phasen-Screenshots der Umsetzung kommen als `phase-N-<thema>.png` hierher (Report-Backs).
