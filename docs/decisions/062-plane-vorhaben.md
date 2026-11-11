# 062 — „Plane Vorhaben“: der Einstieg mit festem Planungs-Prompt

**Status:** angenommen, Phase 1 umgesetzt (Einstieg und Merker); der Wächter folgt in Phase 2 der Story (Taiga #81) · **Datum:** 2026-11-11

## Kontext

Ein Plan soll im Umsetzungstool des Verwalters landen ([ADR 048](048-umsetzung.md)), nicht als Fließtext im Chat. Heute zwingt nichts dazu: Der Hinweis steht in der Beschreibung von `implementation_create` und in Skills außerhalb des Verwalters, und das Modell darf den Aufruf auslassen. Der umgekehrte Weg, Plantext zu erkennen und nachzutippen, ist mit ADR 048 (b) verworfen: keine Prüfung, keine Rückmeldung.

Die Story (Taiga #81) verlangt eine ausdrückliche Aktion „Plane Vorhaben“: Der Verwalter setzt selbst die erste Nachricht des Planungszugs und merkt sich, dass der Zug ein Planungszug ist. Darauf setzt ein Wächter auf (Phase 2): Er prüft am Ende des Zugs, ob `implementation_create` oder `implementation_set_phases` aufgerufen wurde.

## Entscheidung

- **Einstieg:** Ein Knopf „Plane Vorhaben“ in der Eingabeleiste. Der Text im Feld wird der Auftrag; bei leerem Feld ist der Knopf gesperrt. Einen Befehl `/plane` gibt es nicht: Die Story sagt „und/oder“, und die Eingabeleiste kennt zwar Zeilen im `/`-Menü, aber keinen Befehl, der Text aus dem Feld mitnimmt. Der Knopf ist auch gesperrt, wo die Session die Werkzeuge des Verwalters nicht hat (alle Betriebsarten außer „Claude“, bei einem Vorhaben eines Hosts gilt seine), und solange eine Rückfrage des Agenten offen ist; der Hinweis steht im Tooltip.
- **Befehl:** `session_plan_vorhaben(session_id, task, origin)` (Klasse C in `remote/access.rs`, die Herkunft trägt wie bei `chat_send` der Server des Hosts ein) ruft `SessionRegistry::plan_vorhaben`. Es prüft Auftrag und Betriebsart und geht über denselben Sendeweg wie jede Nachricht (`send_turn`); nur der Text für den Agenten ist `plan_prompt(task)`.
- **Prompt fest im Verwalter:** `implementation::plan::plan_prompt(task)` ist eine reine Funktion mit festem Text in Rust, versioniert mit dem Schema (Empfehlung der Story). Er sagt: Das Ergebnis kommt über `implementation_create`, nie als Fließtext; die Begriffe **Lane** und **Station**; das Muster Test, Umsetzung, Review, PR; Reihenfolge aus Bedingungen statt aus Wellen; je Station Typ, Prompt, Modell und Denkaufwand, wobei der Planer die Typen vorher mit `implementation_agent_types` liest ([ADR 060](060-agententypen.md)). Der Auftrag steht unverändert am Ende.
- **Keine Feldnamen:** Der Prompt nennt kein Feld des Schemas. Die Beschreibung von `implementation_create` ist ihre einzige Quelle; Phase 76.2 der Story zu den Constraint-Abläufen ändert das Schema, ohne den Prompt anzufassen. Ein Test hält das fest (`after`, `wave`, `phases`, `max_parallel` kommen nicht vor).
- **Wortlaut:** Der feste Teil steht in der ersten Person („Ich möchte ein Vorhaben planen …“), denn der Benutzer hat den Knopf gedrückt. Ein Satz, der wie eine fremde Anweisung klingt, lehnt das kleinste Modell ab ([ADR 052](052-session-werkzeuge.md), Messung an Haiku). Der Wortlaut ist hier **nicht** an Haiku gemessen; das tut Phase 3 der Story.
- **Darstellung:** Der Verlauf zeigt nur den Auftrag des Benutzers, nicht den festen Teil (Chat zuerst); der Agent bekommt den ganzen Prompt. Der Name der Session entsteht aus dem Auftrag.
- **Merker „Planungszug“:** `SessionState::planning_turn`, nur im Speicher (keine Spalte, keine Migration). Jeder neue Zug setzt ihn neu (`start_turn`): `true` nur durch „Plane Vorhaben“. Die Antwort auf eine Rückfrage lässt ihn stehen, denn der Zug läuft weiter. `SessionRegistry::is_planning_turn(session_id)` liest ihn; das ist die Schnittstelle zu Phase 2. Nach einem Neustart der App ist er gelöscht (Critical Rule 4: nichts startet von selbst).

## Verworfen

- **Plantext erkennen** (ADR 048 (b)): keine Prüfung, keine Rückmeldung an den Agenten.
- **Prompt aus einem Skill:** Er läge außerhalb des Verwalters, wäre je Rechner verschieden und ließe sich nicht mit dem Schema versionieren; ohne den Skill gäbe es keinen Planungszug.
- **Befehl `/plane` zusätzlich zum Knopf:** Die kleinere Lösung gilt, siehe oben.

## Folgen

- Phase 2: Der Wächter liest `is_planning_turn` am Ende des Zugs und die Werkzeugaufrufe im Strom; für seine Erinnerungen braucht er einen Sendeweg, der den Merker erhält (`send_turn` mit `planning`).
- Wer das Schema von `implementation_create` ändert, ändert nur dessen Beschreibung; der Prompt verweist darauf.
- Die Werkzeuge des Verwalters hat nur die Betriebsart Claude (`session_tools` in `start_process`); „Plane Vorhaben“ gibt es deshalb nur dort.
