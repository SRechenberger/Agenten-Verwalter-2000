# Probelauf „Vorhaben von Hand ordnen“ (Taiga 101)

Echte App aus dem Worktree mit `pnpm tauri dev`, Vite auf Port 1475 mit `--strictPort`, Xvfb `:75`. Wegwerf-`HOME` mit leerem `.claude`, der Agent ist eine Attrappe. Die Datenbank legt `seed.py` im Stand vor dem Update an (Migrationen bis 039, also ohne 040). Für die Gast-Prüfung läuft eine zweite Instanz als Host auf `:76` mit Fernsteuerung auf Port 1476. Klicks und Ziehen gehen per XTEST. Ein Vite-Treiber liest die Zeilen der Leiste und schaltet das Farbschema.

Jede Zeile unten nennt die Leiste von oben nach unten, `|` trennt die Bereiche „ohne Gruppe“, „Gruppe Eins“ und „Gruppe Zwei“.

## K7: Stand vor dem Update, erster Start
- Alt (Formel der Leiste vor dem Update, `k7-alte-reihenfolge.json`): Beta, Gamma, Delta, Alpha | Golf, Echo, Foxtrot | India, Hotel
- Erster Start (`k7-zeilen-erster-start.json`, `k7-erster-start-*.png`): Beta, Gamma, Delta, Alpha | Golf, Echo, Foxtrot | India, Hotel
- In Delta hat die archivierte Session #7 die jüngste Aktivität. Sie hebt Delta nicht an, wie vorher.

## K6: 6 Sessions
- Aufgeklappt: #6, #5, #4 und „3 weitere anzeigen“ (`k6-drei-*.png`). Die jüngste Aktivität hat #2.
- „mehr“: #6, #5, #4, #3, #2, #1, „Weniger anzeigen“ (`k6-zeilen-alle.json`, `k6-alle-*.png`). Die archivierte #7 fehlt.

## K2: mit Gruppen ziehen
- K2a: Foxtrot wird über die obere Hälfte von Golf gezogen. Die Linie liegt über Golf. Danach Gruppe Eins: Foxtrot, Golf, Echo (`k2a-in-gruppe-*`).
- K2b: Beta wird über die untere Hälfte von India gezogen. Die Linie liegt unter India. Danach Gruppe Zwei: India, Beta, Hotel (`k2b-andere-gruppe-*`).
- K2c: Alpha wird auf den Kopf „Gruppe Eins“ gezogen, der Kopf ist markiert. Danach Gruppe Eins: Alpha, Foxtrot, Golf (`k2c-gruppenkopf-*`).
- K2d: Echo wird auf „Ohne Gruppe — hier ablegen“ gezogen. Danach ohne Gruppe: Gamma, Delta, Echo, also ans Ende (`k2d-ohne-gruppe-*`).

## K3: Menü
- Foxtrot „Nach oben“: Gruppe Eins wird Foxtrot, Alpha, Golf.
- Beim ersten Vorhaben der Gruppe ist „Nach oben“ aus, beim letzten „Nach unten“, in der Mitte sind beide an.
- Alpha „Nach unten“: Foxtrot, Golf, Alpha (`k3-menue-*`, `k3-nach-unten-hell.png`).

## K4: neues Vorhaben
- Neu angelegt über „Neues Vorhaben“ in „Gruppe Zwei“ (`k4-dialog-ausgefuellt.png`).
- „Kilo neu“ steht oben in Gruppe Zwei, `position` −1 (`k4-neu-oben-*`).

## Wiederhergestellt
- „Juliett“ wird über Archiv → Wiederherstellen zurückgeholt (`entarchiv-dialog.png`). Es steht ganz oben, `position` −2 (`entarchiviert-oben-*`).

## K5: Nachricht und Fertigwerden
- Hotel steht als letztes Vorhaben der Leiste.
- Nachricht „langsam bitte“ an Hotel #1. Die Attrappe antwortet nach 12 s. In der Zwischenzeit ist Juliett #1 gewählt.
- Während des Laufs zeigt Hotel den Laufstatus und bleibt an seiner Stelle (`k5-laeuft-*`).
- Danach ist die Session `completed`, Hotel und #1 tragen den Ungelesen-Punkt. Hotel steht weiter als letztes (`k5-fertig-*`).

## K1: ohne Gruppen, Neustart
- Beide Gruppen werden gelöscht. Hotel wird über die obere Hälfte von India gezogen, die Linie liegt zwischen Gamma und India (`k1-ohne-gruppen-*`).
- Danach: Juliett, Kilo neu, Foxtrot, Golf, Alpha, Gamma, Hotel, India, Delta, Echo, Beta (`k1-nach-ziehen.txt`).
- Nach dem Neustart der App ist die Folge gleich (`k1-nach-neustart.txt`, `k1-nach-neustart-*`).

## Gast: Vorhaben eines Hosts
- Gekoppelt mit dem Host. „Host-Zwei“ und „Host-Vorhaben“ stehen hinter den eigenen Vorhaben. Auf dem Gast gibt es „Gast-Gruppe“ mit Gamma und Delta.
- G1: Host-Vorhaben über die obere Hälfte von Gamma ziehen. Es gibt keine Linie, kein Ziel ist markiert, nichts ändert sich (`gast-g1-vor-gamma-*`).
- G2: Host-Vorhaben über die untere Hälfte von Echo ziehen. Ebenso keine Linie, keine Änderung (`gast-g2-hinter-echo-*`).
- G3: Host-Vorhaben auf den Kopf „Gast-Gruppe“ ziehen. Der Kopf ist markiert, das Vorhaben landet hinter Gamma und Delta (`gast-g3-gruppenkopf-*`).
- G4: Host-Vorhaben auf „Ohne Gruppe“ ziehen. Es steht wieder ohne Gruppe hinter den eigenen (`gast-g4-ohne-gruppe-*`).
- Steht ein Host-Vorhaben in der Gruppe, ist bei Delta, dem letzten eigenen Vorhaben, „Nach unten“ aus (`gast-menue-delta-hell.png`).
- Das Menü des Host-Vorhabens hat kein „Nach oben“ und kein „Nach unten“ (`gast-menue-host-*`).
