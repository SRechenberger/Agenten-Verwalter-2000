# Design-Entwurf: Sprung in die Ziel-Session, Zeit und Name am Fragekasten

**Status:** Entwurf vom 2026-11-21, wartet auf die Wahl des Nutzers (Taiga Story 107, Issue 185; [ADR 052](../../decisions/052-session-werkzeuge.md), [ADR 069](../../decisions/069-zeitpunkt-im-chat.md)). Aufbau und Stil wie [2026-11-20_dashboard-zettel](../2026-11-20_dashboard-zettel/README.md).

## Ansehen

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser. Hell und Dunkel schaltet der Schalter oben um, oder die Sprungmarke `#theme=light` bzw. `#theme=dark`. Alle Namen, Nummern und Uhrzeiten sind Beispieldaten.

## Bilder

| Bild | Ticket | Zeigt |
|---|---|---|
| 1 links | #107 | Kasten „Claude möchte eine Nachricht senden an …“: Empfänger als Links (einer im Zustand Hover, einer mit Fokus), ein gelöschter Empfänger als Text |
| 1 rechts | #107 | Derselbe Kasten für „TL;DR lesen von …“, schmal wie im Popup des Dashboards |
| 2 | #107 | Selbst freigegebene Sendung: Werkzeugzeile `verwalter_send_message_to` mit den Empfängern als Links und ihrem Zustand (zugestellt, wartet, nicht zustellbar); daneben die Zeile heute und die Zeile bei Ablehnung oder unlesbarer Ausgabe |
| 3 | #107 | `verwalter_open_session`: Link zur neuen Session; abgelehnt ohne Link |
| 4 | #185 | Gedankengang und Fragekasten: heute (Zeit schwebt im Abstand), Variante A (eigene Zeile über dem Kasten), Variante B (rechts im Kopf) |
| 5 | #185 | Kopf „… hat gefragt“ für Claude, Claude (LM Studio), Autark und Codex; Tabelle der Namen |

## Offen: Zeit des Fragekastens (A oder B)

- **A:** eigene Zeile wie bei Nachrichten ([ADR 069](../../decisions/069-zeitpunkt-im-chat.md): `entry-time--message`). Einheitlich mit den Nachrichten, kostet eine Zeile Höhe.
- **B:** rechts im Kopf des Kastens, vor der Zahl „N Fragen“, wenn es mehrere gibt. Die Zeit gehört sichtbar zum Kasten und braucht keine Zeile; es ist eine zweite Art, Zeit zu zeigen.

**Es gilt:** noch offen (Wahl des Nutzers).

## Offen: kurze Namen der Betriebsarten

Vorschlag: Claude → „Claude“, Claude Code + LM Studio → „Claude (LM Studio)“, Autark → „Autark“, Codex → „Codex“.

## Festlegungen des Entwurfs

- Links sind Knöpfe mit `aria-label` („Zu Session 12 „Titel““), Akzentfarbe, Unterstreichung bei Hover, Fokusring wie der Kopf einer `SessionMessage`.
- Ohne Ziel (Session gelöscht, Ausgabe abgelehnt oder nicht lesbar) kein Link und keine Fehlermeldung.
- In der Werkzeugzeile stehen die Links neben dem Knopf zum Aufklappen, nicht in ihm.
