# Design-Entwurf: Umsetzungen je Session

**Status:** gewählt am 2026-11-25 (Taiga #216, #217; [ADR 076](../../decisions/076-umsetzungen-je-session.md)).

[mockup.html](mockup.html) ist eigenständig und läuft direkt im Browser, hell und dunkel. Es zeigt die Leiste mit zwei Umsetzungen eines Vorhabens unter zwei Master-Sessions, die Auswahl des Masters mit einer gesperrten Session und die Übersicht ohne „Selbstfreigabe für neue Sessions“.

## Entscheidungen

- **S1 (Ticket #216):** Keine Session öffnet Sessions per Werkzeug. Die Wege der Oberfläche und `verwalter_send_message_to` bleiben.
- **S2:** `verwalter_open_session` verschwindet aus dem Werkzeugsatz, statt mit einem Satz abzulehnen: Ein Werkzeug, das immer ablehnt, kostet jeden Aufruf einen Zug, und Sichtbarkeit gibt es nur je Satz. Der Hinweis auf die zwei Wege steht in `session_tools_prompt`. `ToolRequest::OpenSession` und die Spalte `self_release.open_session` bleiben für alte Einträge; die Zeile „Selbstfreigabe für neue Sessions“ entfällt in Übersicht und Einstellungen.
- **S3:** Höchstens eine unfertige Umsetzung je Master-Session. Nach dem Ende oder dem Abbruch darf dieselbe Session eine neue anlegen. Ein partieller UNIQUE-Index (Migration 036) sichert das im Kern; die Grenze „eine je Vorhaben“ entfällt.
- **S4:** Ein Masterwechsel auf eine Session, die schon eine unfertige Umsetzung führt, wird abgelehnt. Im Picker ist sie gesperrt, mit dem Tooltip „führt schon „…““.
- **S5:** Gelöschte oder archivierte Master bleiben „Master ausgefallen“ mit Auswahl, unverändert.
- **S6:** Eine Station legt keine Umsetzung an.
- **S7:** `implementation_of_session` sucht der Reihe nach die unfertige Umsetzung, die die Session führt; die ihrer Station; die ihrer Rückfrage; die zuletzt geführte, auch eine fertige. Kein Rückfall auf das Vorhaben.
- **S8:** Der Reiter „Changes“ der Umsetzung zeigt weiter das Vorhaben und heißt „Changes im Vorhaben“. Eine Reichweite nur über die Sessions der Umsetzung wird ein Folge-Ticket.
- **S9:** Der Vault-Skill `architect-workflow` wird nicht von der Lane geändert; sie legt einen Patch-Vorschlag ab.
