# 062 — „Plane Vorhaben“: der Einstieg mit festem Planungs-Prompt

**Status:** angenommen, Phase 1 (Einstieg und Merker), Phase 2 (Wächter) und Phase 3 (Messung an Haiku) der Story (Taiga #81) umgesetzt · **Datum:** 2026-11-11

## Kontext

Ein Plan soll im Umsetzungstool des Verwalters landen ([ADR 048](048-umsetzung.md)), nicht als Fließtext im Chat. Heute zwingt nichts dazu: Der Hinweis steht in der Beschreibung von `implementation_create` und in Skills außerhalb des Verwalters, und das Modell darf den Aufruf auslassen. Der umgekehrte Weg, Plantext zu erkennen und nachzutippen, ist mit ADR 048 (b) verworfen: keine Prüfung, keine Rückmeldung.

Die Story (Taiga #81) verlangt eine ausdrückliche Aktion „Plane Vorhaben“: Der Verwalter setzt selbst die erste Nachricht des Planungszugs und merkt sich, dass der Zug ein Planungszug ist. Darauf setzt ein Wächter auf (Phase 2): Er prüft am Ende des Zugs, ob `implementation_create` oder `implementation_set_phases` aufgerufen wurde.

## Entscheidung

- **Einstieg:** Ein Knopf „Plane Vorhaben“ in der Eingabeleiste. Der Text im Feld wird der Auftrag; bei leerem Feld ist der Knopf gesperrt. Einen Befehl `/plane` gibt es nicht: Die Story sagt „und/oder“, und die Eingabeleiste kennt zwar Zeilen im `/`-Menü, aber keinen Befehl, der Text aus dem Feld mitnimmt. Der Knopf ist auch gesperrt, wo die Session die Werkzeuge des Verwalters nicht hat (alle Betriebsarten außer „Claude“, bei einem Vorhaben eines Hosts gilt seine), und solange eine Rückfrage des Agenten offen ist; der Hinweis steht im Tooltip.
- **Befehl:** `session_plan_vorhaben(session_id, task, origin)` (Klasse C in `remote/access.rs`, die Herkunft trägt wie bei `chat_send` der Server des Hosts ein) ruft `SessionRegistry::plan_vorhaben`. Es prüft Auftrag und Betriebsart und geht über denselben Sendeweg wie jede Nachricht (`send_turn`); nur der Text für den Agenten ist `plan_prompt(task)`.
- **Prompt fest im Verwalter:** `implementation::plan::plan_prompt(task)` ist eine reine Funktion mit festem Text in Rust, versioniert mit dem Schema (Empfehlung der Story). Er sagt: Das Ergebnis kommt über `implementation_create`, nie als Fließtext; die Begriffe **Lane** und **Station**; das Muster Test, Umsetzung, Review, PR; Reihenfolge aus Bedingungen statt aus Wellen; je Station Typ, Prompt, Modell und Denkaufwand, wobei der Planer die Typen vorher mit `implementation_agent_types` liest ([ADR 060](060-agententypen.md)). Der Auftrag steht unverändert am Ende.
- **Keine Feldnamen:** Der Prompt nennt kein Feld des Schemas. Die Beschreibung von `implementation_create` ist ihre einzige Quelle; Phase 76.2 der Story zu den Constraint-Abläufen ändert das Schema, ohne den Prompt anzufassen. Ein Test hält das fest (`after`, `wave`, `phases`, `max_parallel` kommen nicht vor).
- **Wortlaut:** Der feste Teil steht in der ersten Person („Ich möchte ein Vorhaben planen …“), denn der Benutzer hat den Knopf gedrückt. Ein Satz, der wie eine fremde Anweisung klingt, lehnt das kleinste Modell ab ([ADR 052](052-session-werkzeuge.md), Messung an Haiku). Der Wortlaut ist in Phase 1 **nicht** an Haiku gemessen; das hat Phase 3 nachgeholt (Nachtrag „Wächter, Messung“).
- **Darstellung:** Der Verlauf zeigt nur den Auftrag des Benutzers, nicht den festen Teil (Chat zuerst); der Agent bekommt den ganzen Prompt. Der Name der Session entsteht aus dem Auftrag.
- **Merker „Planungszug“:** `SessionState::plan_guard` (bis Phase 2 `planning_turn`), nur im Speicher (keine Spalte, keine Migration). Jeder neue Zug setzt ihn neu (`start_turn`): gesetzt nur durch „Plane Vorhaben“. Die Antwort auf eine Rückfrage lässt ihn stehen, denn der Zug läuft weiter. `SessionRegistry::is_planning_turn(session_id)` liest ihn; das ist die Schnittstelle zu Phase 2. Nach einem Neustart der App ist er gelöscht (Critical Rule 4: nichts startet von selbst).

## Nachtrag: der Wächter (Phase 2)

- **Erkennen:** `apply_event` sieht jeden Werkzeugaufruf des Hauptagenten als `ToolStarted`. Heißt er `mcp__verwalter__implementation_create` oder `mcp__verwalter__implementation_set_phases` (`plan::is_plan_tool`), gilt der Planungszug als erfüllt. Nur der Name zählt, nicht das Ergebnis: Ein abgelehnter Plan (etwa ein Zyklus) kommt als Fehler beim Agenten an, der ihn dann verbessert; so hängt der Wächter an keinem Schema. Aufrufe von Subagenten zählen nicht, ebenso wenig gleichnamige Werkzeuge eines anderen MCP-Servers.
- **Entscheidung:** `plan::guard_step` ist rein: kein Wächter → nichts; Aufruf gesehen → nichts; weniger als `MAX_PLAN_REMINDERS` (2) Erinnerungen → erinnern; sonst der Hinweis. Sie läuft in `check_plan` am Ende eines **abgeschlossenen** Zugs, und nur, wenn die Session ruht (kein laufender Subagent, kein erwartetes Skript): Sonst setzt der Agent von selbst fort. Ein Zug, der mit Fehler, Abbruch, Pause oder am Kontingent endet, löst nichts aus.
- **Erinnern:** Die feste Erinnerung `PLAN_REMINDER_TEXT` (erste Person, nennt das Werkzeug) geht wie `RESUME_TEXT` von „Automatisch fortsetzen“ als Nachricht des Benutzers in den Verlauf und an den Agenten, über `start_turn` mit `PlanTurn::Reminder`: Der Wächter bleibt mit seinem Zähler stehen. Sie geht in einem eigenen Thread hinaus (`schedule_plan_reminder`, wie die Warteschlange der Session-Nachrichten); unter der Sperre prüft `send_plan_reminder` noch einmal, dass die Session ruht und der Wächter noch wartet. Wer dazwischen sendet, beendet die Prüfung, und die Erinnerung entfällt. `auto_resume.rs` bleibt unberührt: Ein gemeinsamer Baustein wäre nicht kleiner, denn der Zeitgeber dort wartet auf eine Uhr.
- **Hinweis:** Nach der zweiten Erinnerung steht im Verlauf `Notice::PlanMissing` mit dem Auftrag: „Kein Plan im Umsetzungstool.“ mit **Erneut** (ein neuer Planungszug mit demselben Auftrag, wie „Plane Vorhaben“) und **Trotzdem weiter** (erledigt den Hinweis; die Prüfung endete schon mit ihm). Beide über `session_plan_settle(session_id, seq, retry, origin)` (Klasse C, Herkunft wie `session_plan_vorhaben`); der Hinweis bekommt unter der Sperre `settled` und verliert seine Knöpfe, bevor ein neuer Zug beginnt, sodass ein zweiter Klick (etwa beim Gast) nur `PLAN_NOTICE_GONE` erhält; kommt der Zug bei „Erneut“ nicht zustande, bekommt er sie zurück. Der Wächter endet schon mit dem Hinweis (`advance_guard` gibt ihn heraus); ein Zug, den die Kommandozeile danach von selbst beginnt, bringt keinen zweiten, und „Trotzdem weiter“ berührt den Wächter eines neueren Planungszugs nicht.
- **Neustart:** Der Wächter liegt nur im Speicher. Nach einem Neustart prüft nichts, und nichts startet von selbst (Critical Rule 4). Der gespeicherte Hinweis behält „Erneut“, denn er trägt den Auftrag; „Trotzdem weiter“ nimmt dann nur die Knöpfe weg.
- **Grenzen:** Eine Frage des Agenten als Fließtext beendet den Zug und löst eine Erinnerung aus; die Erinnerung bittet deshalb, Fragen mit `AskUserQuestion` zu stellen (die lässt den Zug offen). „Automatisch fortsetzen“ nach leerem Kontingent sendet `RESUME_TEXT` als gewöhnlichen Zug und beendet die Prüfung. Die Wirkung hat Phase 3 an Haiku gemessen (Nachtrag „Wächter, Messung“).

## Nachtrag: Wächter, Messung (Phase 3)

Gemessen am 2026-11-13 mit `src-tauri/examples/plan-guard-probe.rs` gegen die echte Kommandozeile.

- **Aufbau:** Je Lauf ein Planungszug mit der Zeile `claude` der App (`spawn`: Strom-JSON, `--permission-mode auto` wie die Voreinstellung für neue Vorhaben, `--effort high`, Systemprompt mit Scratchpad, Artefakten und den Session-Werkzeugen, `--allowedTools mcp__verwalter`), dem festen Prompt `plan_prompt` mit einem Auftrag und den Werkzeugen des Verwalters mit ihren echten Schemas. „Mit Wächter“ heißt: nach jedem abgeschlossenen Zug entscheidet `advance_guard` wie in `check_plan`, eine Erinnerung (`PLAN_REMINDER_TEXT`) geht als nächste Nachricht in denselben Prozess, nach der zweiten folgt der Hinweis. „Ohne Wächter“: der Lauf endet nach dem ersten Zug. Die Läufe wechselten sich ab (mit/ohne zuerst im Wechsel). Der MCP-Zugang ist ein Stand-in im Prozess der Probe (keine App, keine Datenbank der App): Es bietet alle Werkzeuge der Claude-Betriebsart mit ihren Beschreibungen und Schemas an, liefert `implementation_agent_types` aus einer frischen Wegwerf-Datenbank (die Migrationen legen keine Typen an, siehe ADR 060: Das Modell bekam in allen 20 Läufen `{"agentTypes":[]}`) und nimmt `implementation_create` mit der Prüfung der App an (Felder, Graph in `new_rows`, Agententypen, nur eine Umsetzung je Vorhaben), ohne zu speichern; die übrigen Werkzeuge antworten mit einem Fehler.
- **Auftrag (für alle Läufe derselbe):** „Eine kleine Kommandozeilen-Anwendung `notiz` in Rust, die Notizen in einer JSON-Datei speichert: `notiz add <Text>`, `notiz list` und `notiz done <Nummer>`. Es gibt noch kein Repository. Sie soll Tests bekommen und vor dem Zusammenführen geprüft werden.“
- **Rückfragen:** `AskUserQuestion` beantwortet die Probe mit einer Ablehnung („Der Benutzer hat die Frage nicht beantwortet.“); in der App bliebe der Zug dagegen offen, bis der Benutzer antwortet.
- **Kommandozeile:** 2.1.296, Modell `claude-haiku-5-5`, mit Wegwerf-`HOME` (verlinktes `.claude` für die Anmeldung), ohne die Umgebung der aufrufenden Session und mit `--setting-sources project,local --strict-mcp-config --no-session-persistence` (ohne Hooks, Plugins und MCP-Server des Benutzers, ohne Transkript, wie die Messung in ADR 060).
- **Läufe:** 20 gewertet, 10 mit und 10 ohne Wächter. Dazu zwei Probeläufe mit einer gewöhnlichen Nachricht (Aufbau, 0,01 USD) und sechs weitere Läufe des ersten Durchgangs, die am Sitzungslimit des Kontos scheiterten (Antwort 429 `usage_limit_reached`, nach 0 bis 35 s); sie zählen nicht, denn kein Zug endete regulär, und wurden nach dem Zurücksetzen des Kontingents wiederholt (die Läufe 15 bis 20 unten). Bei einem davon (ohne Wächter) hatte Haiku den Plan vor dem Limit schon angelegt und die Prüfung ihn angenommen; die Wiederholung zählt trotzdem neu, auch das ist keine Auswahl nach Ergebnis, sondern gilt für alle sechs. Ein Fehler der Probe zählte bei einem Zug, der nach dem Aufruf mit Fehler endete, den Aufruf nicht mit (betraf nur diesen Lauf); er ist behoben.

| | mit Wächter | ohne Wächter |
| :-- | ---: | ---: |
| Läufe | 10 | 10 |
| Plan angelegt (`implementation_create` gerufen) | 10 | 9 |
| davon im ersten Zug | 9 | 9 |
| Plan von der Prüfung des Verwalters angenommen | 10 | 9 |
| Erinnerungen insgesamt | 1 | — |
| Hinweis „Kein Plan im Umsetzungstool“ | 0 | — |
| Läufe mit Rückfrage (`AskUserQuestion`) | 2 | 3 |
| Züge insgesamt | 11 | 10 |
| Dauer, Median (s) | 48 | 60 |
| Kosten (Listenpreis, USD) | 0,118 | 0,120 |

| Lauf | Wächter | Plan angelegt | im ersten Zug | Erinnerungen | Hinweis | Züge | Rückfragen | Dauer (s) |
| ---: | :-- | :-- | :-- | ---: | :-- | ---: | ---: | ---: |
| 1 | mit | ja | ja | 0 | nein | 1 | 1 | 35 |
| 2 | ohne | ja | ja | 0 | nein | 1 | 0 | 59 |
| 3 | ohne | ja | ja | 0 | nein | 1 | 0 | 118 |
| 4 | mit | ja | ja | 0 | nein | 1 | 0 | 57 |
| 5 | mit | ja | ja | 0 | nein | 1 | 0 | 59 |
| 6 | ohne | ja | ja | 0 | nein | 1 | 1 | 61 |
| 7 | ohne | ja | ja | 0 | nein | 1 | 0 | 45 |
| 8 | mit | ja | ja | 0 | nein | 1 | 0 | 41 |
| 9 | mit | ja | ja | 0 | nein | 1 | 0 | 63 |
| 10 | ohne | nein | nein | 0 | nein | 1 | 1 | 32 |
| 11 | ohne | ja | ja | 0 | nein | 1 | 0 | 61 |
| 12 | mit | ja | ja | 0 | nein | 1 | 1 | 63 |
| 13 | mit | ja | nein | 1 | nein | 2 | 0 | 49 |
| 14 | ohne | ja | ja | 0 | nein | 1 | 0 | 35 |
| 15 | mit | ja | ja | 0 | nein | 1 | 0 | 41 |
| 16 | ohne | ja | ja | 0 | nein | 1 | 0 | 67 |
| 17 | ohne | ja | ja | 0 | nein | 1 | 0 | 41 |
| 18 | mit | ja | ja | 0 | nein | 1 | 0 | 47 |
| 19 | mit | ja | ja | 0 | nein | 1 | 0 | 47 |
| 20 | ohne | ja | ja | 0 | nein | 1 | 1 | 72 |

- **Befund:** Haiku rief `implementation_create` in 18 von 20 Läufen schon im ersten Zug (9 und 9), meist nach `ToolSearch` und `implementation_agent_types`. Je Arm endete ein Zug ohne Aufruf. Mit Wächter war es Lauf 13: Haiku stellte seine Fragen als Fließtext; die Erinnerung ließ ihn den Plan anlegen. Ohne Wächter war es Lauf 10: Haiku stellte eine Frage mit `AskUserQuestion`, bekam keine Antwort und beendete den Zug ohne Plan; dieser Lauf blieb ohne Plan. Der Wächter löste in keinem der 9 Läufe mit Aufruf im ersten Zug etwas aus. Nach einer Rückfrage legte Haiku in 4 von 5 Läufen den Plan trotzdem an. Eine Verweigerung wie in ADR 052 gab es in keinem Lauf: Der Wortlaut des festen Prompts (erste Person) funktioniert mit Haiku.
- **Wie groß der Unterschied ist:** Eins von zehn (Plan angelegt 10 gegen 9). Das ist kein Beleg: Bei 10 Läufen je Arm und einem Fehlzug je Arm liegt der Unterschied in der Streuung (zweiseitiger exakter Test nach Fisher p = 1,0). Belegt ist nur, dass der Wächter nichts auslöst, wenn der Aufruf kam, und dass seine Erinnerung in dem einen Fall, in dem sie nötig war, wirkte. Der Hinweis (zwei Erinnerungen ohne Wirkung) kam nie vor und ist damit **nicht** an einem echten Modell gemessen.
- **Grenzen der Stichprobe:** **keine eingerichteten Agententypen** (das Modell sah eine leere Liste: Der Teil von `plan_prompt`, der je Station einen Typ verlangt, und die Prüfung der Typen in `implementation_create` kamen nie zum Tragen; ein Vorhaben mit Typen ist nicht gemessen); ein Auftrag, ein Modell (Haiku; größere Modelle sind nicht gemessen), 10 Läufe je Arm, ein Messtag, ein Stand der Kommandozeile; ein Stand-in statt der App (kein Speichern, kein Lagebericht als Antwort, `implementation_set_phases` ohne Umsetzung nicht aufrufbar); die Rückfrage wird abgelehnt statt beantwortet; die Voreinstellung „Auto“, nicht der Modus Plan (dort sperrt die Kommandozeile Werkzeuge ohne `readOnlyHint`, siehe ADR 060; ob `implementation_create` dort ankommt, ist nicht gemessen); ohne Hooks, Plugins und `CLAUDE.md` des Benutzers; Dauer und Kosten schwanken je Lauf stärker als der Unterschied der Arme.

## Verworfen

- **Plantext erkennen** (ADR 048 (b)): keine Prüfung, keine Rückmeldung an den Agenten.
- **Prompt aus einem Skill:** Er läge außerhalb des Verwalters, wäre je Rechner verschieden und ließe sich nicht mit dem Schema versionieren; ohne den Skill gäbe es keinen Planungszug.
- **Befehl `/plane` zusätzlich zum Knopf:** Die kleinere Lösung gilt, siehe oben.

## Folgen

- Phase 2: Der Wächter liest seinen Merker am Ende des Zugs und die Werkzeugaufrufe im Strom; seine Erinnerungen gehen über `start_turn` mit `PlanTurn::Reminder`, das den Merker erhält (siehe Nachtrag).
- Wer das Schema von `implementation_create` ändert, ändert nur dessen Beschreibung; der Prompt verweist darauf.
- Die Werkzeuge des Verwalters hat nur die Betriebsart Claude (`session_tools` in `start_process`); „Plane Vorhaben“ gibt es deshalb nur dort.
