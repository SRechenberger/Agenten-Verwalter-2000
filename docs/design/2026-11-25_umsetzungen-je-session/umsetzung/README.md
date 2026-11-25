# Probelauf: Umsetzungen je Session (Taiga 217, 216)

Echte App aus dem Branch der Lane: eigenes Xvfb-Display, Port 1451 mit `--strictPort`, Wegwerf-`HOME`, frische Datenbank (Schema 36). Der Agent ist eine Attrappe ohne Kontingent. Werkzeugaufrufe laufen über `mcp-probe` mit dem Schlüssel der laufenden Attrappe. Vorhaben „Probelauf 217“ mit den Sessions #1 Planer A, #2 Planer B und #3 Fremd C; #4 entsteht über „Neue Rückfrage“.

| # | Kriterium | Weg | Beleg | Ergebnis |
|---|---|---|---|---|
| 1 | Zwei Sessions legen je eine Umsetzung an, beide in der Leiste unter ihrem Master | `implementation_create` aus #1 und #2 | `k1-create-a.log`, `k1-create-b.log`, `k1-db.log`, `leiste-hell.png`, `leiste-dunkel.png` | beide angelegt, beide unfertig im selben Vorhaben; „Umsetzung A“ unter #1, „Umsetzung B“ unter #2 |
| 2 | Zweite `implementation_create` aus derselben Session abgelehnt (V4) | zweiter Aufruf aus #1 | `k2-create-a2.log` | „Du führst schon die Umsetzung „Umsetzung A“. Ändere ihren Plan mit implementation_edit, oder leg eine neue an, wenn sie fertig ist.“ |
| 3 | Masterwechsel auf eine führende Session im Picker gesperrt, mit Grund | Umsetzung B, „ändern“; Klick auf #1; zusätzlich Befehl `implementation_set_master` | `picker-hell.png`, `picker-dunkel.png`, `k3-picker-dom.log`, `k3-db-nach-klick.log`, `k3-set-master-core.log` | #1 `disabled`, `title` „führt schon „Umsetzung A““; der Klick ändert nichts; der Kern lehnt mit „Die Session führt schon die Umsetzung „Umsetzung A“.“ ab |
| 4 | `tools/list` ohne `verwalter_open_session` | `mcp-probe … list` gegen die laufende App | `k4-tools-list.log`, `k4-tools-names.txt` | 21 Werkzeuge, kein `verwalter_open_session` |
| 5 | Rückfrage sieht ihre Umsetzung; fremde Session bekommt den Satz aus S7 | „Neue Rückfrage“ in Umsetzung A, erste Nachricht; `implementation_get` aus #4 und #3 | `k5-db.log`, `k5-get-rueckfrage.log`, `k5-get-fremd.log` | #4 bekommt den Lagebericht von „Umsetzung A“; #3 bekommt „Du führst keine Umsetzung. Leg eine mit implementation_create an.“ |
| 6 | Übersicht ohne „Selbstfreigabe für neue Sessions“, hell und dunkel neben dem Prototyp | Übersicht des Vorhabens, Einstellungen › Agent › Selbstfreigabe | `uebersicht-*.png`, `einstellungen-*.png`, `k6-dom-*.json`, `prototyp-*.png` | Zeile fehlt in Übersicht und Einstellungen, hell und dunkel |

Die Bilder `prototyp-*` zeigen `../mockup.html` in denselben Ansichten.
