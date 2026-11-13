# 062 — „Plane Vorhaben“: der Einstieg mit festem Planungs-Prompt

**Status:** angenommen, Phase 1 (Einstieg und Merker) und Phase 2 (Wächter) der Story (Taiga #81) umgesetzt · **Datum:** 2026-11-11

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
- **Merker „Planungszug“:** `SessionState::plan_guard` (bis Phase 2 `planning_turn`), nur im Speicher (keine Spalte, keine Migration). Jeder neue Zug setzt ihn neu (`start_turn`): gesetzt nur durch „Plane Vorhaben“. Die Antwort auf eine Rückfrage lässt ihn stehen, denn der Zug läuft weiter. `SessionRegistry::is_planning_turn(session_id)` liest ihn; das ist die Schnittstelle zu Phase 2. Nach einem Neustart der App ist er gelöscht (Critical Rule 4: nichts startet von selbst).

## Nachtrag: der Wächter (Phase 2)

- **Erkennen:** `apply_event` sieht jeden Werkzeugaufruf des Hauptagenten als `ToolStarted`. Heißt er `mcp__verwalter__implementation_create` oder `mcp__verwalter__implementation_set_phases` (`plan::is_plan_tool`), gilt der Planungszug als erfüllt. Nur der Name zählt, nicht das Ergebnis: Ein abgelehnter Plan (etwa ein Zyklus) kommt als Fehler beim Agenten an, der ihn dann verbessert; so hängt der Wächter an keinem Schema. Aufrufe von Subagenten zählen nicht, ebenso wenig gleichnamige Werkzeuge eines anderen MCP-Servers.
- **Entscheidung:** `plan::guard_step` ist rein: kein Wächter → nichts; Aufruf gesehen → nichts; weniger als `MAX_PLAN_REMINDERS` (2) Erinnerungen → erinnern; sonst der Hinweis. Sie läuft in `check_plan` am Ende eines **abgeschlossenen** Zugs, und nur, wenn die Session ruht (kein laufender Subagent, kein erwartetes Skript): Sonst setzt der Agent von selbst fort. Ein Zug, der mit Fehler, Abbruch, Pause oder am Kontingent endet, löst nichts aus.
- **Erinnern:** Die feste Erinnerung `PLAN_REMINDER_TEXT` (erste Person, nennt das Werkzeug) geht wie `RESUME_TEXT` von „Automatisch fortsetzen“ als Nachricht des Benutzers in den Verlauf und an den Agenten, über `start_turn` mit `PlanTurn::Reminder`: Der Wächter bleibt mit seinem Zähler stehen. Sie geht in einem eigenen Thread hinaus (`schedule_plan_reminder`, wie die Warteschlange der Session-Nachrichten); unter der Sperre prüft `send_plan_reminder` noch einmal, dass die Session ruht und der Wächter noch wartet. Wer dazwischen sendet, beendet die Prüfung, und die Erinnerung entfällt. `auto_resume.rs` bleibt unberührt: Ein gemeinsamer Baustein wäre nicht kleiner, denn der Zeitgeber dort wartet auf eine Uhr.
- **Hinweis:** Nach der zweiten Erinnerung steht im Verlauf `Notice::PlanMissing` mit dem Auftrag: „Kein Plan im Umsetzungstool.“ mit **Erneut** (ein neuer Planungszug mit demselben Auftrag, wie „Plane Vorhaben“) und **Trotzdem weiter** (erledigt den Hinweis; die Prüfung endete schon mit ihm). Beide über `session_plan_settle(session_id, seq, retry, origin)` (Klasse C, Herkunft wie `session_plan_vorhaben`); der Hinweis bekommt unter der Sperre `settled` und verliert seine Knöpfe, bevor ein neuer Zug beginnt, sodass ein zweiter Klick (etwa beim Gast) nur `PLAN_NOTICE_GONE` erhält; kommt der Zug bei „Erneut“ nicht zustande, bekommt er sie zurück. Der Wächter endet schon mit dem Hinweis (`advance_guard` gibt ihn heraus); ein Zug, den die Kommandozeile danach von selbst beginnt, bringt keinen zweiten, und „Trotzdem weiter“ berührt den Wächter eines neueren Planungszugs nicht.
- **Neustart:** Der Wächter liegt nur im Speicher. Nach einem Neustart prüft nichts, und nichts startet von selbst (Critical Rule 4). Der gespeicherte Hinweis behält „Erneut“, denn er trägt den Auftrag; „Trotzdem weiter“ nimmt dann nur die Knöpfe weg.
- **Grenzen:** Eine Frage des Agenten als Fließtext beendet den Zug und löst eine Erinnerung aus; die Erinnerung bittet deshalb, Fragen mit `AskUserQuestion` zu stellen (die lässt den Zug offen). „Automatisch fortsetzen“ nach leerem Kontingent sendet `RESUME_TEXT` als gewöhnlichen Zug und beendet die Prüfung. Die Wirkung misst Phase 3 der Story an Haiku.

## Verworfen

- **Plantext erkennen** (ADR 048 (b)): keine Prüfung, keine Rückmeldung an den Agenten.
- **Prompt aus einem Skill:** Er läge außerhalb des Verwalters, wäre je Rechner verschieden und ließe sich nicht mit dem Schema versionieren; ohne den Skill gäbe es keinen Planungszug.
- **Befehl `/plane` zusätzlich zum Knopf:** Die kleinere Lösung gilt, siehe oben.

## Folgen

- Phase 2: Der Wächter liest seinen Merker am Ende des Zugs und die Werkzeugaufrufe im Strom; seine Erinnerungen gehen über `start_turn` mit `PlanTurn::Reminder`, das den Merker erhält (siehe Nachtrag).
- Wer das Schema von `implementation_create` ändert, ändert nur dessen Beschreibung; der Prompt verweist darauf.
- Die Werkzeuge des Verwalters hat nur die Betriebsart Claude (`session_tools` in `start_process`); „Plane Vorhaben“ gibt es deshalb nur dort.
