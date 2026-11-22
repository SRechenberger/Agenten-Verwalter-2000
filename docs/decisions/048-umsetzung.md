# 048 — Umsetzung: Phasen und Stationen eines Plans im Verwalter

**Status:** angenommen; „fertig mit Fertigmeldung“, die Werkzeugliste und die Bindung der Meldung an die Session überholt [ADR 070](070-master-session.md) (Meldung an den Master, Annahme durch den Master) · **Datum:** 2026-11-03

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
- **Ein Präfix je Werkzeugsatz:** `implementation_create`, `implementation_get`, `implementation_set_phases`, `implementation_describe_phase`, `implementation_complete_station` — der Vertrag `ToolSet` kennt genau einen Präfix. Hinzugekommen sind `implementation_agent_types` ([ADR 060](060-agententypen.md)) und `implementation_edit` samt `handoff` an der Fertigmeldung ([ADR 061](061-constraint-ablaeufe.md), Werkzeuge); „Plane Vorhaben“ ([ADR 062](062-plane-vorhaben.md)) prüft, dass ein Plan über dieses Werkzeug ankommt.
- **Bindung an die aufrufende Session:** ein Werkzeug wirkt auf die neueste offene (weder fertige noch abgebrochene) Umsetzung, die die Session als Master führt (`implementation_of_session`, eine Stelle für alle Aufrufer; Issue 177), sonst auf die Umsetzung, deren Station die Session ist, sonst auf die neueste ihres Vorhabens. Eine Nebenfrage darf von den Werkzeugen nur `implementation_get` aufrufen; ihr Schlüssel sperrt die übrigen ([ADR 044](044-nebenfragen.md)).
- **Lagebericht als Text** (`implementation/report.rs`), gleich für Agent und Oberfläche; die Uhrzeit „Stand HH:MM“ ist Ortszeit (über `chrono`, das schon im Abhängigkeitsbaum lag).
- **Reihenfolge:** Seit [ADR 061](061-constraint-ablaeufe.md) plant der Core aus Kanten (`after`) statt aus Position und Welle; ein Plan ohne `after` bekommt seine Kanten aus Position und Welle, bestehende Umsetzungen bei der Migration ebenso.
- **Sidebar:** die Umsetzung ist eine Zeile unter ihrem Vorhaben vor den Sessions; Stationen bekommen keine eigene Session-Zeile (`SessionSummary.station_of`), zählen aber in Aktivität und Ungelesen des Vorhabens. Geändert durch [ADR 065](065-stationen-in-der-sidebar.md): aktive Stationen stehen als Zeilen unter der Umsetzung; geändert durch [ADR 070](070-master-session.md): die Umsetzung steht unter ihrer Master-Session, darunter nur Stationen, die dich brauchen.
- **Von Hand als fertig markieren:** Der Befehl `implementation_mark_station_done(implementationId, stationId, result)` (HostWrite mit Umsetzungs-ID) beendet eine Station des Agenten, die der Ablauf nicht weiterbringt: im Fehler ohne anstehenden Neustart (auch mit Startfehler) oder mit pausierter Session ohne scharfen Zeitgeber „Automatisch fortsetzen“, mit oder ohne Meldung. Nicht erlaubt: Station „durch dich“, angenommene, laufende, abgebrochene Umsetzung, ein anstehender Neustart, ein scharfer Zeitgeber; der Core lehnt dann mit „Als fertig markieren geht nur bei einer Station im Fehler oder in einer Pause ohne Zeitgeber.“ ab. Die Wirkung ist die einer Annahme ohne Meldung und ohne Fix-Runde, auch bei einer Meldung mit `fix_needed`: `completed_at` und `result` (getrimmt, 1 bis 200 Zeichen) werden gesetzt, Nachfolger und Abschluss laufen wie nach einer Annahme (`finish_station`, derselbe Weg wie `complete_station_with_handoff`; `Finish::by_hand` setzt `needs_report = false`, `plan_finish` plant keine Fix-Runde und `Finish::wake_events` wählt „Von Hand fertig“, je als reine Funktion mit Test; kein Aufrufer reicht `by_hand` als Wert durch). Die Session bleibt, wie sie ist; die Station zeigt danach `done` ohne `live`. Der Master bekommt einmal das Weck-Ereignis „Von Hand fertig“ ([ADR 070](070-master-session.md)). Die Regel steht einmal im Core (`implementation::may_be_marked_done(view, station)`, auch „Umsetzung abgebrochen“; jede Ablehnung außer einer unbekannten Station trägt den einen Satz des Vertrags) und einmal als Spiegel für die Knöpfe (`canMarkDone(station, isImplementationCancelled)` in `markDone.ts`, Selbsttest). Prüfen ohne App: `implementation-probe <Datenbank-Kopie> mark-done <ID> <Station-ID> "<Ergebnis>"` geht über `Finish::marking`, `store_finish` (mit einer Registry über der Kopie) und `Finish::wake_events`, also den Weg der App; nicht erreicht sind nur `AppHandle`-Teile (Zeitgeber, Zustellung).

## Konsequenzen

- Ohne Verwalter gibt es die Werkzeuge nicht: im Terminal arbeiten die Skills weiter mit der Einstiegszeile.
- Stationen sind Sessions ohne eigene Zeile — wer sie öffnen will, geht über die Ansicht der Umsetzung.
- Prüfen ohne App: `cargo run --example implementation-probe -- <Datenbank-Kopie> create <Session-ID> <JSON-Datei>` bzw. `… show <Umsetzungs-ID>`.

### Nachtrag: Art der Station ([ADR 073](073-befehls-stationen.md))

Eine Station hat eine Art (`kind`): `agent`, `user` („durch dich“) oder `command` (Befehls-Station: Der Verwalter führt Schritte ohne Session und ohne Modell aus). Die Spalte `kind` (Migration 33) ist die Quelle; `byUser` in der Ansicht folgt aus ihr, die Eingabe heißt `by: "agent" | "you" | "command"`. Eine Befehls-Station hat keine Session, ihr Zustand folgt aus dem gespeicherten Lauf (`running`, `error`, `done`).

### Nachtrag 2026-11-21: Marke „Plan ändern?“ in der Leiste (Taiga #132)

Hat eine Umsetzung einen offenen Vorschlag (`pendingChange`), trägt ihre Zeile in der Leiste die Marke „✎ Plan?“, ein Knopf mit `aria-label` „Plan ändern? Vorschlag ansehen“ (Variante A des Entwurfs `docs/design/2026-11-21_stationen-bedienen/`, nur an der Zeile der Umsetzung). Ein Klick öffnet die Umsetzung mit der Karte „Plan ändern?“ vorn (`PENDING_FOCUS`). Ist der Vorschlag entschieden, ist `pendingChange` leer, und die Marke verschwindet. Die Karte selbst hebt sich stärker ab: getönter Grund, kräftiger Rahmen mit Fokusring, im Kopf ✎ und „wartet auf dich“. Es ist nur Darstellung; Zustand und Core bleiben.

### Nachtrag 2026-11-22: Einstiegszeilen entfallen (Taiga #41)

Zwei Sätze oben stimmen nicht mehr. Im Kontext trägt nicht mehr die Einstiegszeile den Übergang zwischen Sessions ([ADR 027](027-session-uebergabe.md), [ADR 040](040-automatisch-weiter.md)), und die Konsequenz „im Terminal arbeiten die Skills weiter mit der Einstiegszeile“ gilt nicht mehr. [ADR 052](052-session-werkzeuge.md) hat die Zeilen „Weiter: …“ und „Zurück an …“ in allen Betriebsarten gestrichen; solche Zeilen sind normaler Text. Es gilt: Sessions geben einander Arbeit über die Session-Werkzeuge (`verwalter_send_message_to` und die übrigen), nur in der Betriebsart Claude. Stationen einer Umsetzung melden mit `implementation_complete_station` an ihren Master ([ADR 070](070-master-session.md)). Außerhalb von Claude gibt es keine Übergabe durch die App. Eine Einstiegszeile „Neues Vorhaben …“ (Auftrag für ein neues Vorhaben) gibt es weiter, sie ist nicht Teil der Umsetzung (Glossar: Einstiegszeile).
