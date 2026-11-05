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

## Abweichungen vom Entwurf

Noch keine: Bisher steht nur das Steuerrecht im Core, nichts davon ist sichtbar.
