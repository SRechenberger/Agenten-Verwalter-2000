# 048 — Umsetzung: Phasen und Stationen eines Plans im Verwalter

**Status:** angenommen · **Datum:** 2026-11-03

## Kontext

Ein freigegebener Plan wird heute Phase für Phase in frischen Sessions umgesetzt; den Übergang trägt die Einstiegszeile am Ende jeder Antwort ([ADR 027](027-session-uebergabe.md), [ADR 040](040-automatisch-weiter.md)). Der Verwalter sieht dabei nur einzelne Sessions, nicht den Ablauf: welche Phase dran ist, was fertig ist, was als Nächstes startet. Seit [ADR 046](046-mcp-zugang.md) kann ein Agent Werkzeuge des Verwalters aufrufen.

## Optionen

- **(a) Der Verwalter liest die Plan-Dateien selbst** — verworfen: bindet die App an das Tabellenformat der Plan-README; Pläne, die in einem Tracker leben, blieben außen vor.
- **(b) Ein Textblock in der Antwort des Agenten, den der Verwalter erkennt** — verworfen: keine Prüfung der Eingabe, keine Rückmeldung an den Agenten, kein Lesezugriff auf den Stand.
- **(c) Werkzeuge über den MCP-Zugang des Verwalters** — gewählt: die Planungs-Session legt nach der Freigabe die Umsetzung mit `implementation_create` an; der Agent bekommt geprüfte Fehler (`isError` mit dem Feld im Satz) und liest den Stand mit `implementation_get`.

## Entscheidung

- **Speicher:** eigene Tabellen `implementations`, `implementation_phases`, `implementation_stations`, `implementation_changes` (Migration 19) statt einer JSON-Spalte am Vorhaben — eine Station wird über ihre Session-ID gefunden (`implementation_stations_session`), das geht nur mit einer Zeile je Station.
- **Der Zustand einer Station wird nicht gespeichert**, sondern aus ihrer Session abgeleitet (ohne Session wartend bzw. bereit, mit Session nach deren Status, mit Fertigmeldung fertig) — eine Wahrheit, wie beim Git-Zustand (Critical Rule 2). Gespeichert sind nur Aufbau, Texte, Bindung an die Session und die Fertigmeldung.
- **Eine Umsetzung gehört einem Vorhaben**, je Vorhaben höchstens eine unfertige. Prüfung und Anlegen laufen unter derselben Datenbank-Sperre.
- **Ein Präfix je Werkzeugsatz:** `implementation_create`, `implementation_get`, `implementation_set_phases`, `implementation_describe_phase`, `implementation_complete_station` — der Vertrag `ToolSet` kennt genau einen Präfix.
- **Bindung an die aufrufende Session:** ein Werkzeug wirkt auf die Umsetzung, deren Station die Session ist, sonst auf die neueste ihres Vorhabens. Eine Nebenfrage darf von den Werkzeugen nur `implementation_get` aufrufen; ihr Schlüssel sperrt die übrigen ([ADR 044](044-nebenfragen.md)).
- **Lagebericht als Text** (`implementation/report.rs`), gleich für Agent und Oberfläche; die Uhrzeit „Stand HH:MM“ ist Ortszeit (über `chrono`, das schon im Abhängigkeitsbaum lag).
- **Reihenfolge:** Seit [ADR 061](061-constraint-ablaeufe.md) plant der Core aus Kanten (`after`) statt aus Position und Welle; ein Plan ohne `after` bekommt seine Kanten aus Position und Welle, bestehende Umsetzungen bei der Migration ebenso.
- **Sidebar:** die Umsetzung ist eine Zeile unter ihrem Vorhaben vor den Sessions; Stationen bekommen keine eigene Session-Zeile (`SessionSummary.station_of`), zählen aber in Aktivität und Ungelesen des Vorhabens.

## Konsequenzen

- Ohne Verwalter gibt es die Werkzeuge nicht: im Terminal arbeiten die Skills weiter mit der Einstiegszeile.
- Stationen sind Sessions ohne eigene Zeile — wer sie öffnen will, geht über die Ansicht der Umsetzung.
- Prüfen ohne App: `cargo run --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON-Datei>` bzw. `… show <Umsetzungs-ID>`.
