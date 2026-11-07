# Design-Entwurf: Remote Sessions

**Status:** abgenommen am 2026-11-05. Für alles Sichtbare der Fernsteuerung ist dieser Entwurf verbindlich: Einstellungen, Leiste, Übersicht, Eingabeleiste, Kontingent und Terminal, jeweils aus Sicht von Host und Gast. Alles andere regelt weiter der Entwurf [Vorhaben gruppieren](../2026-11-04_vorhaben-gruppen/README.md) und die älteren Entwürfe. Wo die Umsetzung abweichen muss, wird erst dieser Entwurf geändert, dann der Code. Entscheidungen im Core: [ADR 059](../../decisions/059-remote-sessions.md).

## Ansehen

- Die Datei [mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser (Doppelklick genügt, kein Server nötig).
- Bedienbar: Oben schalten „Rolle“, „Ansicht“, „Zustand“ und „Farbschema“ um.

## Gezeigt wird

| Rolle | Ansicht | Zustände |
|---|---|---|
| Host | Übersicht, Chat | nicht freigegeben, freigegeben, Gast steuert, Gast getrennt; Dialog „Freigabe beenden“; Terminal zum Mitlesen |
| Host | Einstellungen | aus, an samt Geräten und Einladung, Port belegt; Warnung beim Schließen |
| Gast | Chat, Übersicht | übernommen, nur angeboten, Host weg, Freigabe beendet; Terminal auf dem Host |
| Gast | Einstellungen | nicht verbunden, mehrere Hosts verbunden, abgelehnt samt weiteren Fehlermeldungen |

## Entscheidungen des Nutzers (2026-11-05)

1. **Marke:** Übernommene Sessions sehen aus wie eigene und tragen eine Marke, die auf den anderen Host hinweist.
2. **Leiste:** Entfernte Vorhaben stehen in der normalen Liste und lassen sich lokal in Gruppen einsortieren.
3. **Übernehmen:** Kein Knopf in der Zeile, die Zeile behält ihre Höhe. „Übernehmen“ steht im Hauptfenster und im Kontextmenü.
4. **Kontingent:** Das Kontingent verbundener Hosts steht unter dem eigenen, im gewohnten Stil mit Füllstandsbalken.
5. **Mehrere Hosts:** Ein Gast kann mit mehreren Hosts zugleich verbunden sein.
6. **Terminal:** Auch am Gast, der Host liest mit.

## Gestaltung

- Maße, Farben und Schriften folgen `src/styles/theme.css` und den Stilen der Leiste.
- „Chat first“: Die Fernsteuerung bringt ohne verbundenen Host und ohne Freigabe nichts Neues auf den Bildschirm.

## Platzhalter im Entwurf

- Alle Namen, Geräte, Adressen, Zahlen und Zeiten sind Beispieldaten.
- Knöpfe wirken nur auf den gezeigten Zustand des Entwurfs; es gibt keine Verbindung.

## Umsetzung

Stand: Die Einstellungen stehen (Abschnitt „Fernsteuerung“ in der Kategorie „App“, Host und Gast), ebenso Leiste, Übersicht, Chat und Köpfe mit Freigeben, Übernehmen, Zurückgeben, Freigabe beenden und den Offline-Zuständen. Kontingent, Betriebsart des Hosts, Inhalte (Artefakte, Bilder, Anhänge) und Terminal folgen in den nächsten Phasen.

Bilder der laufenden App, zwei Instanzen mit getrennten Datenordnern auf einem Rechner, wirklich gekoppelt. Sie liegen in [`umsetzung/`](umsetzung/):

| Entwurf (Rolle · Zustand) | Bild |
|---|---|
| Host · Einstellungen · Aus (und Gast · Nicht verbunden, derselbe Stand) | `host-aus.png` |
| Host · Einstellungen · An | `host-an.png`, hell `host-an-hell.png` |
| Host · Dialog „Einladung“ | `host-einladung.png` |
| Host · Einstellungen · An, Gerät gekoppelt | `host-geraet-gekoppelt.png` |
| Host · Einstellungen · Port belegt | `host-port-belegt.png` |
| Gast · Einstellungen · Verbunden | `gast-verbunden.png`, hell `gast-verbunden-hell.png` |
| Gast · Einstellungen · Mehrere Hosts | `gast-zwei-hosts.png` |
| Gast · Host nicht erreichbar (Port am Host gewechselt) | `gast-host-nicht-erreichbar.png` |
| Gast · Einstellungen · Abgelehnt (benutzte Einladung) | `gast-abgelehnt.png` |
| Gast · Gerät am Host gesperrt | `gast-gesperrt.png` |
| Gast · Zertifikat passt nicht (hell) | `gast-zertifikat-hell.png` |

Leiste, Übersicht und Chat, zwei Instanzen auf einem Rechner, wirklich gekoppelt, `claude` als Attrappe; dunkel und hell:

| Entwurf (Rolle · Zustand) | Bild |
|---|---|
| Host · Übersicht · Nicht freigegeben (Block „Fernsteuerung“ nur bei eingeschaltetem Server) | `host-nicht-freigegeben.png` |
| Host · Übersicht · Freigegeben, noch kein Gast (Marke und „Für Gäste freigegeben“ in der Leiste) | `host-freigegeben-frei.png` |
| Host · Übersicht · Gast steuert (hell) | `host-gast-steuert-uebersicht-hell.png` |
| Host · Chat · Gast steuert, Sperrzeile (hell) | `host-gast-steuert-chat-hell.png` |
| Host · Dialog „Freigabe beenden“ mit Haken | `host-dialog-freigabe-beenden.png` |
| Host · Chat · Gast getrennt, Band und Restzeit (hell) | `host-gast-getrennt-chat-hell.png` |
| Host · Übersicht · Gast getrennt, „reserviert noch“ (hell) | `host-gast-getrennt-uebersicht-hell.png` |
| Host · Umsetzung · Gast steuert, Knöpfe gesperrt (hell) | `host-umsetzung-gesperrt-hell.png` |
| Gast · Leiste und Hauptfläche · Nur angeboten | `gast-angeboten.png` |
| Gast · Kontextmenü eines freien Vorhabens | `gast-menue-frei.png` |
| Gast · Übersicht · Übernommen (Chip mit dem Host, „Zurückgeben“) | `gast-uebernommen-uebersicht.png` |
| Gast · Chat · Übernommen | `gast-chat-uebernommen.png` |
| Gast · Chat · Host weg, Entwurf erhalten | `gast-host-weg-entwurf.png`, hell `gast-host-weg-hell.png` |
| Gast · Freigabe beendet | `gast-freigabe-beendet.png` |
| Gast · Nach „Zurückgeben“: wieder angeboten | `gast-zurueckgegeben.png` |
| Gast · Vorhaben eines Hosts per Ziehen in eine eigene Gruppe | `gast-gruppe-gezogen.png` |
| Gast · Umsetzung, Rückfrage (hell) | `gast-umsetzung-hell.png`, `gast-rueckfrage-hell.png` |
| Gast · Host neu gestartet: Übernahme verfallen, Angebot statt fremder Session (hell) | `gast-angebot-nach-neustart-hell.png` |

Nicht gesehen: eine offene Rückfrage mit der Zeile „Antworten kann gerade nur …“ (die Attrappe stellt keine), das Terminal am Gast und am Host (Phase 5), Windows, ein zweiter Host, das Neuladen des Verlaufs nach einem Abriss (nur im Quelltext gelesen), die Meldung zum anderen Stand des Protokolls (sie lässt sich nur über die Probe erzeugen), die Warnung beim Schließen der App, „steuert „<Vorhaben>““ in der Geräteliste.

## Abweichungen vom Entwurf

- **Gerätename:** Das Feld ist nicht vorbelegt. Der Core kennt den Namen des Rechners, gibt ihn aber über keinen Command heraus; „Verbinden“ bleibt gesperrt, bis ein Name im Feld steht.
- **Erklärungen:** Der Text unter dem Zeilennamen steht, wie auf der übrigen Einstellungsseite, hinter dem ⓘ ([ADR 057](../../decisions/057-einstellungen-kategorien-und-suche.md)). Unter dem Bedienelement bleiben nur der Hinweis zum Port, die Adressen, der Fingerabdruck und die Fehlersätze.
- **Geräteliste:** Ein Gerät zeigt „Verbunden“ oder „Zuletzt gesehen …“, noch nicht, welches Vorhaben es steuert; die Zeile `RemoteDevice` kennt das nicht.
- **Dialog „Einladung“:** Er nennt „10 Minuten und einmal“ wörtlich und lässt sich nach der Kopplung nicht von selbst schließen.
- **Suche:** Zeilen, die der Stand der Fernsteuerung ausblendet (Port, Status, Geräte ohne laufenden Server; Hosts ohne Kopplung), findet die Suche nicht.
- **Hosts:** Die Liste ist nach Name und Adresse sortiert; „Getrennt“ erscheint für einen Host, den „Trennen“ beendet hat, ohne Fehler.
- **Meta-Zeile am Host:** Sie zeigt den Zustand der Freigabe statt „Läuft · …“ („Für Gäste freigegeben“, „Gesteuert von <Gerät>“, „<Gerät> getrennt · 4:12“). Die Restzeit steht ohne „frei in“, weil die Zeile sonst abgeschnitten wird; der volle Satz steht im Hinweis.
- **Meta-Zeile am Gast:** Der Host steht vor den Sessions („<Host> · 2 Sessions · #2 pausiert“), denn die Zeile ist schmaler als im Entwurf und schneidet das Ende ab.
- **Restzeit der Reservierung:** Der Host kennt den Zeitpunkt des Abrisses nicht als Teil des Vertrags; die Oberfläche liest ihn aus `RemoteDevice.lastSeenAt`, das der Core beim Abriss schreibt. „seit 14:02“ im Block „Fernsteuerung“ fehlt aus demselben Grund.
- **Terminal-Knopf:** Er bleibt am Gast auch bei fehlendem Host stehen; das Terminal am Gast kommt mit Phase 5.
- **Herkunft („von <Gerät>“):** Der Verlauf trägt sie noch nicht; `ChatEntry` hat das Feld im Core noch nicht.
