# 060 — Agententypen für Stationen der Umsetzung

**Status:** in Umsetzung, Phasen 1 bis 6 von 7 (Stand siehe „Stand der Umsetzung“) · **Datum:** 2026-11-08

## Kontext

Eine Umsetzung ([ADR 048](048-umsetzung.md)) startet jede Station als eigene Session. Modell, Modus und Denkaufwand erbt die neue Session von der Session mit der höchsten Nummer (`create_fresh_in_project`); nur ein ausdrückliches Modell der Station gewinnt. Welche Rolle eine Station spielt, steht allein in ihrem Auftrag und ihrem Skill.

Die Story (Taiga #55) verlangt eigene Agententypen: In den Einstellungen lassen sich Typen wie developer, tester oder reviewer anlegen, je mit Modell und Denkaufwand, Systemprompt, Dateien und Skills, „praktisch Agenten mit ihren Briefs“. Ihre Beschreibungen helfen dem Planer, gute Empfehlungen für den Ablauf zu machen. Aus Issue #47 gilt: Ein ausdrücklich angegebenes Modell einer Station gewinnt. Die Vorgabe U1 des Nutzers vom 2026-11-08 erweitert das: Jede Einstellung eines Typs lässt sich für eine einzelne Station anpassen, etwa der maximale Denkaufwand für ein besonders kritisches Review oder ein eigener Zusatztext zum Prompt. Was ein Typ festlegt, gilt also als Vorgabe, von der die Station abweichen darf.

Der Name ist doppelt belegt: „Agenten-Typ“ heißt im Glossar der Provider, und `standalone/agents.rs` kennt `AgentDefinition` und `DEFAULT_AGENT_TYPE` für Subagenten. Das Glossar grenzt „Agententyp“ dagegen ab; der Core-Ordner heißt `agent_types`, wie die Story es festlegt.

## Entscheidungen

### Vorschläge der Story (V1 bis V14)

Die Story nannte sie unbestätigt; die Freigabe des Plans am 2026-11-08 bestätigt sie, soweit N1 bis N15 nichts anderes sagen.

| | Entscheidung | Phase |
|---|---|---|
| **V1** | Ein Plan ohne Typ nimmt Standardmodell und Standard-Modus der Einstellungen, wie die erste Session eines neuen Vorhabens heute. | unverändert |
| **V2** | Die Felder eines Typs (Abschnitt „Typ und Befehle“), ein Abschnitt „Agententypen“ in der Kategorie Agent der Einstellungen, eine eigene Tabelle, keine mitgelieferten Typen. | 1 (Tabelle, Befehle), 3 (Oberfläche) |
| **V3** | Zuordnung über das Feld `agent` der Station, sonst über den Stationsnamen; ein Stationsname gehört höchstens einem Typ. | 1 (Regel, Prüfung), 2 (Feld) |
| **V4** | Reihenfolge je Wert: Station, Typ, allgemeine Regel. Durch U1 gilt das auch für den Denkaufwand. | 1 (Funktion), 2 (Start) |
| **V5** | Der Systemprompt des Typs geht in `--append-system-prompt`, hinter die Vorgaben des Verwalters, in allen vier Betriebsarten. | 4 |
| **V6** | Die Dateien liest der Core: erst der Text, dann die Dateien; Pfade absolut, `~/…` oder relativ; Frontmatter fällt weg; Obergrenze 16 000. | 4 |
| **V7** | Der Systemprompt wird beim Start als Momentaufnahme an der Session gespeichert. | 4 |
| **V8** | Fehler stoppen die Station: geprüft vor `create_fresh_in_project`, die Karte nennt den Grund. | 2 (Mechanismus, gelöschter Typ), 4 (Dateien), 6 (Anzeige) |
| **V9** | Skills: erst `skill` der Station, dann die des Typs. | 4 |
| **V10** | Fix-Runde und zweites Review übernehmen `agent`, mit U1 auch `effort` und `prompt`: Ein kritisches Review bleibt kritisch. Ein Typ, der „Fix-Runde“ führt, schlägt den Rückfall. | 2 |
| **V11** | Die Stationskarte nennt Typ, Modell und Denkaufwand. | 2 (Daten), 6 (Karte) |
| **V12** | Modell und Denkaufwand **des Typs** gelten nur in der Betriebsart Claude, Systemprompt und Skills in allen. Ausdrückliche Werte der Station gelten wie das Modell der Station heute in jeder Betriebsart. | 2, 4 |
| **V13** | Lesewerkzeug `implementation_agent_types`; kein Werkzeug schreibt Typen oder nimmt Pfade an. | 5 |
| **V14** | Der Planer liest die Typen, bevor er den Plan vorlegt; dafür ein fester Satz in `session_tools_prompt`. | 5 |

### Neu im Plan (N1 bis N15)

| | Entscheidung | Verworfen |
|---|---|---|
| **N1** | Sieben Phasen, je ein PR und ein Taiga-Task; Phase 2 und 3 laufen parallel, ebenso Phase 4 und 5. Kern und Oberfläche sind getrennte Phasen, weil die Fix-Runde Modell und Skill der *letzten* Station „Umsetzung“ einer Phase nimmt. | Strikt nacheinander. |
| **N2** | Getrennt von #47 (Standardwerte neuer Sessions), das zurückgestellt ist: `create_fresh_in_project` bleibt unberührt, Modell und Denkaufwand werden auf der neuen, noch nicht gestarteten Session gesetzt. Die „allgemeine Regel“ ist heute das Erben von der Session mit der höchsten Nummer, nach #47 sind es die Einstellungen. | #47 vorneweg. |
| **N3** | Die Prüfung beim Speichern liefert Feldfehler im Ergebnis (`AgentTypeSaved`), keine neue Variante von `CommandError`; die Meldungen stehen im Core. Grenzen: Name 1 bis 40 Zeichen, Beschreibung bis 500, höchstens 20 Stationsnamen je bis 40, 10 Skills je 1 bis 40 ohne „/“, 10 Dateien je bis 500, Text bis 16 000. | Eine Variante, die Folgeänderungen in `errors.ts` und in jedem erschöpfenden `Record` bräuchte. |
| **N4** | Umbenennen führt `agent` in noch nicht gestarteten Stationen nach, mit einer `UPDATE` in derselben Transaktion. | Nicht nachführen: Diese Stationen scheiterten danach beim Start wie nach dem Löschen. |
| **N5** | Neue Spalte `start_error`: Die Station bleibt `ready`, der Fehler steht auf der Karte, der automatische Start versucht sie nicht erneut, ▶ versucht es. | Ohne Spalte: Ein Fehler vor der Session hätte keine Heimat, denn ADR 048 leitet den Zustand aus der Session ab. |
| **N6** | Der Typname steht klein hinter dem Stationsnamen in der Zeitleiste. | |
| **N7** | `check.yml` fährt die Tests von `implementation::` und `agent_types::` in einem eigenen Schritt, der Wert in Anführungszeichen. | Die Tests nur von Hand, wie bisher die der Fix-Runde. |
| **N8** | Je Phase ein Taiga-Task. Jede Station der Phase setzt ihn beim Start auf „In progress“, falls er davor steht, die PR-Station am Ende auf „Ready for test“; geschlossen wird er von der ersten Station der nächsten Welle, sobald der PR gemergt ist. Den Status der Story setzt der Nutzer. | |
| **N9** | Der Entwurf der Oberfläche kommt nach `docs/design/2026-11-07_agententypen/`. | |
| **N10** | ADR 060, Migrationen ab 22, Version je PR die nächste freie Minor; alles zu Beginn jeder Phase gegen den gemergten Stand und die offenen PRs neu gelesen. | Die Nummern der Story (058, 21), beide inzwischen vergeben. |
| **N11** | Nicht im Umfang, auf Zuruf: Typ und Abweichungen im Lagebericht und in `implementation_get`, `deny_unknown_fields` für `StationInput`, Modus je Typ, Typen je Vorhaben. | |
| **N12** | Eine Station überschreibt jede Einstellung des Typs für sich: `agent`, `model`, `effort` (neu), `prompt` (neu: Zusatztext zum Systemprompt) und `skill`. Modell und Denkaufwand lösen sich unabhängig auf; nur die Definition eines Typs verlangt sie als Paar. Dateien bleiben am Typ. | Dateien je Station: eine Spalte mehr und ein Weg, der Pfade annimmt (V13 fiele); sie stünden nicht in `StationInput` und gingen bei jeder Plan-Änderung verloren. |
| **N13** | Der Zusatztext der Station ergänzt den Text des Typs und ersetzt ihn nicht: erst der Text des Typs, dann dessen Dateien, dann der Zusatztext. Er wirkt auch an einer Station ohne Typ und in allen vier Betriebsarten. Bis 4 000 Zeichen; die Gesamtgrenze von 16 000 bleibt. | Ersetzen, das einen Schalter je Station bräuchte; wer die Rolle ganz anders will, legt einen Typ an und nennt ihn im Feld `agent`. |
| **N14** | Die Abweichungen setzt der Planer im Plan (`effort`, `prompt` in `implementation_create` und `implementation_set_phases`) und der Nutzer auf der Karte (Befehl `implementation_set_station_agent`, Klasse C), beides nur bei einer Station ohne Session. Der Dialog setzt immer den ganzen Satz der fünf Werte. | Nur der Planer; nur die Oberfläche. |
| **N15** | Eine auf der Karte angepasste Station trägt die Marke `edited`. Ersetzt eine Plan-Änderung die kommenden Phasen, übernimmt eine neue Station mit gleichem Phasenschlüssel, gleicher Stufe (wörtlich) und gleicher laufender Nummer unter den Stationen dieser Stufe die fünf Werte der alten, soweit die Eingabe des Planers sie nicht nennt. Was der Planer nennt, gilt. Die Spalte kommt mit den übrigen Stationsspalten in die Migration der Phase 2; geschrieben wird sie erst in Phase 5. | Ohne Marke ersetzte jede Plan-Änderung die Einstellungen des Nutzers still. |

### Festlegungen der Phase 1

Was die Verträge offenlassen, legt der Core so fest:

- **Trimmen:** Name, Beschreibung, jeder Stationsname, jeder Pfad und jeder Skill werden getrimmt geprüft und gespeichert; der Text (`prompt`) bleibt, wie er kommt. Doppelte Stationsnamen eines Typs (ohne Beachtung der Schreibweise) fallen ohne Meldung weg, der erste bleibt.
- **Zählen:** Längen zählen Zeichen (Unicode-Skalarwerte, wie `check_length` der Umsetzung). Nur der Text zählt UTF-16-Einheiten, denn er geht als Argument auf die Kommandozeile, deren Grenze Windows in diesen Einheiten zählt; ein Zeichen außerhalb der BMP zählt dort zwei. Die Oberfläche der Phase 3 zählt ebenso: die Beschreibung in Codepunkten (`[...text].length`), den Text mit `.length`; ein Zähler mit `.length` wiche bei der Beschreibung für Zeichen außerhalb der BMP vom Core ab.
- **Skill mit „/“:** Ein Skill mit führendem „/“ ist ungültig, der Core kürzt nicht. Die Oberfläche der Phase 3 schneidet ein getipptes führendes „/“ vor dem Senden ab.
- **Reihenfolge der Anlage:** Die Spalte `position` (größte vorhandene plus eins) hält sie, auch über das Löschen hinweg.
- **Transaktion:** Anlegen und Ändern lesen die gespeicherten Typen, prüfen und schreiben in einer Transaktion, die sofort schreibend beginnt (`IMMEDIATE`); dazwischen ändert niemand die Tabelle.
- **Unbekannte ID:** Ändern und Löschen eines Typs, den es nicht gibt, sind ein `CommandError`, keine Meldung der Prüfung.
- **Ereignis:** `agent-types://changed` geht nach jeder Änderung, also nicht, wenn die Prüfung etwas meldet.

### Festlegungen der Phase 2

Was die Verträge für den Ablauf offenlassen, legt der Core so fest:

- **Startfehler und ▶:** `schedule::may_start` liefert `false`, solange `start_error` gesetzt ist; der automatische Start (`schedule_ready` über `schedule::due_for_countdown`, der Zeitgeber über `may_start`) versucht die Station deshalb nicht erneut. Die Ansicht zeigt sie trotzdem als `ready` (N5), denn ihr Zustand folgt aus `schedule::is_due` (an der Reihe, ohne Blick auf den Fehler); ▶ prüft in Schritt 1 ebenfalls `is_due`, sonst könnte niemand den Start erneut versuchen. Scheitert ▶ wieder, steht der neue Text an der Station und kommt als Fehler zurück; gelingt der Start, räumt Schritt 6 den Fehler aus.
- **Entscheidung des Starts:** `start::start_plan(types, station, mode)` ist rein und liefert Typ, Modell und Denkaufwand oder den Text des Startfehlers; `start_claimed` ruft sie vor `create_fresh_in_project` und vergleicht danach mit Modell und Denkaufwand der neuen Session. Die Betriebsart kommt aus den Einstellungen, denn mit ihr startet der erste `send` den Prozess.
- **Rückfall der Fix-Runde:** Hat die Phase weder Umsetzung noch Abschluss und führt ein Typ die Fix-Runde (Stationsname „Fix-Runde“ wie bei der Auflösung), schreibt der Rückfall weder Modell noch Skill: Beides kommt beim Start vom Typ, und `implement` stünde sonst vor dessen Skills. Kopiert wird das Feld `agent` der Vorlage, nicht der aufgelöste Typ; lief die Umsetzung nur über ihren Stationsnamen als Typ, nimmt die Fix-Runde den Typ, der „Fix-Runde“ führt, oder keinen.
- **Ansicht:** Vor dem Start löst `build_view` Typ, Modell und Denkaufwand unabhängig von der Betriebsart auf; nennt die Station einen unbekannten Typ, stehen nur ihre eigenen Werte da. Nach dem Start (eine Session ist gebunden) gilt der gespeicherte Name `agent_used`, Modell und Denkaufwand bleiben leer, denn dann gelten die der Session. Eine Station des Benutzers läuft nie als Typ.
- **Speichern:** `agent` wird getrimmt gespeichert, `prompt` getrimmt und leer als kein Zusatztext; `effort` liegt wie `model` als serde-Text in der Spalte.
- **Umbenennen und Ändern:** Ein neuer Name, auch nur in anderer Schreibweise, führt `agent` aller Stationen ohne Session und ohne Fertigmeldung nach; verglichen wird in Rust mit `name_key`, weil `lower()` in SQLite nur ASCII kennt. Nach jeder Änderung an den Typen, auch nach dem Anlegen, meldet der Befehl jede Umsetzung mit einer noch nicht gestarteten Station neu (`implementation://changed`), denn deren Auflösung hängt an den Typen.

### Festlegungen der Phase 4

Was die Verträge für Systemprompt und Skills offenlassen, legt der Core so fest:

- **Zusammensetzen:** `agent_types::prompt::compose` ist rein, das Lesen der Dateien kommt als Funktion herein. Jeder Teil verliert den Leerraum am Rand, auch der Text des Typs, der gespeichert unverändert bleibt; so steht zwischen zwei Teilen genau eine Leerzeile. Ein Teil ohne Text fällt weg. Bleibt nichts übrig, bekommt die Session keinen Schnappschuss (`NULL`), und ihre Argumente sind wie vor dieser ADR. Von jeder Datei fällt das Frontmatter nach `skills/frontmatter.rs` weg; eine Datei ohne Frontmatter bleibt ganz.
- **Pfade:** `~/…` gilt im Benutzerordner, ein absoluter Pfad dort, wohin er zeigt. Jeder andere gilt der Reihe nach in den Arbeitsordnern der Repositories, die die neue Session bekommt (die der Session mit der höchsten Nummer, wie bei `create_fresh_in_project`): Es zählt das erste, in dem die Datei liegt. Liegt sie dort, ist aber nicht lesbar, zählt kein späteres. „Nicht gefunden“ heißt: in keinem dieser Ordner. Nicht lesbar ist auch ein Ordner und eine Datei, die kein UTF-8 ist. Die Meldung nennt den Pfad so, wie er im Typ steht. `~\…` kennt der Core nicht.
- **Grenze:** Gezählt wird das Ganze samt Leerzeilen in UTF-16-Einheiten, ohne die Vorgaben des Verwalters davor. Die Meldung schreibt die Zahl wie die Grenze mit Leerzeichen als Tausendertrenner („hat 16 001 Zeichen“). Eine Datei wird ganz gelesen.
- **Schnappschuss:** `start_claimed` setzt den Text vor `create_fresh_in_project` zusammen, aus dem Typ, den `start_plan` nennt; ein Fehler geht denselben Weg wie ein unbekannter Typ (Startfehler, keine Session). Nach `agent_used` hält `store_agent_prompt` den Text an der Session fest, danach sendet `send` die erste Nachricht und startet damit den ersten Prozess. Gespeichert wird er über `upsert`, das die Spalte auch im `UPDATE` schreibt, denn die Zeile der Session legt `create_fresh_in_project` ohne ihn an. `start_process` und `ask_side` lesen ihn aus dem Zustand der Session, nach einem Neustart der App und nach `restore_project` kommt er aus der Zeile.
- **Skills:** Die übrigen Skills des Typs stehen in der zweiten Zeile der ersten Nachricht: „Lade außerdem diese Skills: a, b.“ Ein Skill, der schon in Zeile 1 steht, fehlt dort. Ohne Skills bleibt die erste Nachricht, wie sie war.
- **Aufzeichnung der Kommandozeile:** Die Kommandozeile (2.1.296) zeichnet mit `--system-prompt-snapshot on`, ihrem Standard, den gerenderten Systemprompt samt `--append-system-prompt` beim ersten Request auf und sendet ihn bei jedem weiteren Request und Resume unverändert, auch wenn ein späterer Start anderen Text übergibt, bis die Unterhaltung verdichtet wird. Der Schnappschuss des Verwalters bleibt trotzdem nötig: Nach dem Verdichten gilt, was der Verwalter übergibt; Autark und Codex kennen keine Aufzeichnung und lesen bei jedem Start nur das Argument; und ein Modell, das nach einem Neustart den alten Text befolgt, beweist nicht, dass der Verwalter ihn übergeben hat. Belegt werden deshalb die Argumente der Prozesse, und einmal ein Lauf mit `--system-prompt-snapshot off`.

### Festlegungen der Phase 6

Was Plan und Entwurf für Karte und Dialog offenlassen, legt die Oberfläche so fest:

- **Karte vor dem Start:** Die Zeile zeigt, was der Core in `StationView` auflöst (`agentType`, `agentModel` und `agentEffort` mit ihrer Herkunft Station oder Typ). Fehlt ein Wert, gilt die allgemeine Regel; ihr Wert ist der der Session mit der höchsten Nummer im Vorhaben, den die Oberfläche aus den Sessions liest (`latestSessionOf`, dieselbe Regel wie `create_fresh_in_project`; ändert sich die Regel, ändert sich nur diese Funktion). Der Core meldet die allgemeine Regel nicht als Wert.
- **Karte nach dem Start:** Modell und Denkaufwand der Session ohne Herkunft, denn der Nutzer kann sie in der Eingabeleiste geändert haben. Der Typ ist der gespeicherte (`agentType` nach dem Start).
- **Andere Betriebsarten:** Die Karte zeigt nur ausdrückliche Werte der Station („Station“); sonst steht „Modell: wie die Session“ und „Denkaufwand: wie die Session“, auch nach dem Start. Die Betriebsart ist die des Hosts, wenn das Vorhaben einem anderen Gerät gehört (`useOperatingMode`).
- **Typ ohne Treffer:** Nennt `agent` einen Typ, den es nicht gibt, steht er gestrichelt als „X · gelöscht“; ohne `agent` und ohne Treffer über den Stationsnamen steht „kein Typ“ gestrichelt.
- **Startfehler:** Der Ton `error` gehört der Station, nicht ihrem Zustand (`stationTone`): Punkt, Linie der Zeitleiste und Rand der Karte sind rot, der Zustand bleibt „bereit“ und ▶ versucht es erneut. Der Text im Block ist der des Core. Ein manueller Start gibt ihn zusätzlich als Fehler zurück; ihn zeigt das vorhandene Banner über der Zeitleiste. „Typ bearbeiten“ steht nur, wenn die Auflösung einen Typ fand; ein gelöschter Typ wird in „Einstellungen“ ersetzt.
- **Dialog:** Er schickt immer den ganzen Satz der fünf Werte; leer wird `null`. Der Core meldet einen Satz je Aufruf; der Dialog setzt ihn an das Feld, mit dem er beginnt („prompt: …“, „skill: …“, „Der Agententyp …“), alles andere in den Fuß. Die Vorschau „So startet die Station“ bildet die Auflösung aus 4.3 für den Entwurf nach, denn der Core kennt ihn nicht; es ist die einzige Stelle der Oberfläche, die diese Regel rechnet. Esc und Klick auf den Hintergrund fragen bei Ungespeichertem wie im Dialog der Typen; ein offenes Modellmenü schließt zuerst.
- **Gäste:** Die Typen im Dialog und für „Typ bearbeiten“ kommen aus den Einstellungen dieses Rechners (Klasse G). Gehört das Vorhaben einem anderen Gerät, kann die Liste von der des Hosts abweichen; geprüft wird am Host.

### Festlegungen der Phase 5

Was die Verträge für Werkzeuge, Befehl und Plan-Änderung offenlassen, legt der Core so fest:

- **Befehl:** `implementation::set_station_agent` liest die Ansicht, prüft mit der reinen Funktion `checked_settings` und schreibt mit `set_station_settings`. Geprüft wird in dieser Reihenfolge: begonnen (Session oder Fertigmeldung), Station des Benutzers, Typ (wie die Auflösung getrimmt und ohne Beachtung der Schreibweise), Zusatztext, Skill. Gespeichert wird wie aus dem Plan: `agent` getrimmt in der eingegebenen Schreibweise, `prompt` getrimmt und leer als kein Zusatztext, `skill` getrimmt und ohne führenden „/“. Die Marke `edited` setzt auch ein Satz, der alles löscht.
- **Begonnen zwischen Prüfen und Schreiben:** Die `UPDATE`-Anweisung schreibt nur, solange die Station weder Session noch Fertigmeldung hat; sonst meldet der Befehl „Die Station hat schon begonnen.“. Offen bleibt der umgekehrte Fall: Liest ein Start die Station, bevor der Befehl schreibt, und bindet er seine Session erst danach, startet die Station mit den alten Werten, und die neuen stehen an ihr (eine Fix-Runde übernähme sie). Das Fenster reicht vom Lesen der Ansicht im Start bis `bind_station`.
- **Plan-Änderung:** Die reine Funktion `changes::replacement_rows` rechnet die neuen Zeilen: `phase_rows`, dann `keep_known_ids`, dann `keep_settings`; `apply` und die Probe rufen sie. Weil `edited` nicht in der Ansicht steht, liest `apply` beim Übernehmen die gespeicherten Zeilen (`station_rows`), also auch, was der Nutzer nach dem Vorschlag gesetzt hat. Gleicher Phasenschlüssel heißt in `keep_settings` gleiche `phase_id`, denn `keep_known_ids` gibt einer Phase mit bekanntem Schlüssel vorher die alte `id`. „Die Eingabe nennt den Wert nicht“ heißt: Die neue Zeile hat ihn nicht; ein leerer Zusatztext des Planers zählt deshalb als nicht genannt. Eine neue Station, die einer angepassten entspricht, trägt `edited`, auch wenn der Planer alle fünf Werte nennt.
- **Lesewerkzeug:** Der Text ist das JSON von `structuredContent`. Eine Nebenfrage darf `implementation_get` und `implementation_agent_types` rufen. `tools/list` nennt das Werkzeug mit `"annotations": { "readOnlyHint": true }` (`ToolSet::is_read_only`): Ohne die Kennzeichnung lehnt die Kommandozeile 2.1.296 ein MCP-Werkzeug im Modus Plan ab („Cannot call mcp__verwalter__implementation_agent_types while in plan mode.“), auch mit `--allowedTools mcp__verwalter`. Nur dieses Werkzeug trägt sie; die übrigen lesenden Werkzeuge (`implementation_get`, `artifact_list`, die Session-Werkzeuge) bleiben im Modus Plan gesperrt wie bisher.
- **Satz für den Planer:** `session_tools_prompt` endet mit „Bevor du einen Plan mit Stationen vorlegst, lies mit mcp__verwalter__implementation_agent_types die Agententypen aus den Einstellungen und empfiehl je Station einen Typ.“ Gemessen an Haiku (`claude-haiku-5-5`, Kommandozeile 2.1.296, Modus Plan, die Argumente der App, ein Planungsauftrag ohne Hinweis auf das Werkzeug): In drei von drei Läufen lud und rief Haiku das Werkzeug als Erstes, vor jedem Plan. Im ersten Lauf lehnte die Kommandozeile den Aufruf noch ab (ohne `readOnlyHint`), in den beiden folgenden kam die Liste an, und beide Pläne nennen je Station einen Typ. Die Stichprobe ist klein.

## Aufbau

```text
Einstellungen ──► agent_types (SQLite) ──► Auflösung (rein) ◄── Station: stage, agent, model, effort, prompt, skill
                         │                       │                        ▲
        implementation_agent_types (MCP)         │                        ├─ Planer: implementation_create / _set_phases   (Phase 5)
        Planer liest, empfiehlt                  │                        └─ Nutzer: Karte „Einstellungen“ ─► implementation_set_station_agent
                                                 ▼                             (Dialog Phase 6, Befehl Phase 5; nur ohne Session)
                                          start_claimed (Phase 2)
                                          1. auflösen      → Fehler? start_error, keine Session
                                          2. Text + Dateien + Zusatztext → Fehler? start_error, keine Session   (Phase 4)
                                          3. create_fresh_in_project, bind, rename
                                          4. set_model, set_effort  (Station: jede Betriebsart; Typ: nur Claude)
                                          5. Schnappschuss an der Session                          (Phase 4)
                                          6. send(first_message)  → ein Prozess, richtige Werte
```

Schritt 4 reicht: Auf einer Session im Zustand `New` schreiben `set_model` und `set_effort` nur in den Zustand. Es gibt keinen Prozess und keine Meldung über einen Modellwechsel. Der erste `send` startet genau einen Prozess aus `state.model` und `state.effort` (`sessions/registry.rs`, `start_turn` und `start_process`). `create_fresh_in_project` bleibt unberührt, daher kein Konflikt mit #47.

## Verträge

Sie gelten wörtlich für alle sieben Phasen. Was davon gebaut ist, steht unter „Stand der Umsetzung“.

### Typ und Befehle

```rust
// src-tauri/src/agent_types/model.rs; derive(Serialize, Deserialize, TS), serde camelCase
pub struct AgentType {
    pub id: String,                // UUID
    pub name: String,              // getrimmt, 1..40, ohne Beachtung der Schreibweise eindeutig
    pub description: String,       // Pflicht, bis 500, für den Planer
    pub stages: Vec<String>,       // Stationsnamen, null bis 20
    pub model: Option<ModelId>,    // Some und effort Some, oder beide None („Standard“)
    pub effort: Option<Effort>,
    pub prompt: String,            // bis 16 000 Zeichen; leer erlaubt
    pub files: Vec<String>,        // null bis 10; absolut, `~/…` oder relativ zu den Repositories des Vorhabens
    pub skills: Vec<String>,       // null bis 10
}
pub struct AgentTypeInput { /* wie AgentType ohne id */ }
pub enum AgentTypeField { Name, Description, Stages, Model, Prompt, Files, Skills }
pub struct FieldError { pub field: AgentTypeField, pub message: String }
pub struct AgentTypeSaved { pub agent_type: Option<AgentType>, pub errors: Vec<FieldError> }
```

```text
agent_types_list()                  -> AgentType[]           (Reihenfolge der Anlage)
agent_type_create(input)            -> AgentTypeSaved
agent_type_update(id, input)        -> AgentTypeSaved        (Umbenennen = anderer name; ab Phase 2 mit Nachführen von `agent`)
agent_type_delete(id)               -> ()
Ereignis  agent-types://changed     Nutzlast AgentType[] (die ganze Liste)
Klasse in remote/access.rs: alle vier in „nur lokal“ (G)
```

Ist `errors` nicht leer, ist `agentType` `null` und nichts gespeichert. Echte Fehler (Datenbank) bleiben `CommandError`.

### Prüfung und Meldungen

| Feld | Regel | Meldung |
| :- | :- | :- |
| `name` | getrimmt nicht leer | „Der Name fehlt.“ |
| `name` | höchstens 40 | „Der Name ist länger als 40 Zeichen.“ |
| `name` | nicht schon bei einem anderen Typ (getrimmt, ohne Beachtung der Schreibweise) | „Der Name „X“ gehört schon einem anderen Typ.“ |
| `description` | getrimmt nicht leer | „Die Beschreibung fehlt. Der Planer braucht sie für seine Empfehlung.“ |
| `description` | höchstens 500 | „Die Beschreibung ist länger als 500 Zeichen.“ |
| `stages` | jeder Name getrimmt 1 bis 40 | „Der Stationsname „X“ ist leer oder länger als 40 Zeichen.“ |
| `stages` | ein Name gehört höchstens einem Typ (getrimmt, ohne Beachtung der Schreibweise); innerhalb des Typs doppelte fallen weg | „„X“ gehört schon zum Typ „Y“.“ |
| `stages` | höchstens 20, gezählt nach dem Wegfall doppelter Namen | „Ein Typ führt höchstens 20 Stationsnamen.“ |
| `model` | nur zusammen mit `effort` | „Modell und Denkaufwand gelten nur zusammen.“ |
| `prompt` | höchstens 16 000 Zeichen (UTF-16-Einheiten, wie Windows zählt) | „Der Text ist länger als 16 000 Zeichen.“ |
| `files` | höchstens 10 | „Ein Typ hat höchstens 10 Dateien.“ |
| `files` | jeder Pfad 1 bis 500 Zeichen, ohne Zeilenumbruch und NUL | „Der Pfad „X“ ist ungültig.“ |
| `skills` | höchstens 10 | „Ein Typ hat höchstens 10 Skills.“ |
| `skills` | jeder 1 bis 40 Zeichen, ohne „/“ | „Der Skill „X“ ist ungültig (1 bis 40 Zeichen, ohne „/“).“ |

Die Oberfläche zeigt die Meldungen an ihrem Feld und rechnet nur die Zähler (500, 16 000) selbst. Der Core zählt Längen in Zeichen (Unicode-Skalarwerten, wie `check_length` der Umsetzung), nur den Text des Typs in UTF-16-Einheiten; die Oberfläche zählt ebenso: die Beschreibung in Codepunkten (`[...text].length`), den Text mit `.length`. Name, Beschreibung, Stationsnamen, Pfade und Skills werden getrimmt geprüft und gespeichert, der Text nicht. Alle Meldungen aller Felder kommen zusammen, in der Reihenfolge der Felder. Ein Skill mit führendem „/“ ist ungültig (der Core kürzt nicht); die Oberfläche schneidet ein getipptes führendes „/“ vor dem Senden ab.

### Auflösung

```text
resolve(types, station: StationValues) -> Result<Resolved, UnknownAgent>          // StationValues { stage, agent, model, effort }
  type   = station.agent gesetzt  -> der Typ mit diesem Namen (getrimmt, ohne Beachtung der Schreibweise); keiner: Err(UnknownAgent { name, known })
           sonst                  -> der Typ, der station.stage führt (getrimmt, ohne Beachtung der Schreibweise); keiner: kein Typ
  model  = station.model  >  type.model  >  allgemeine Regel
  effort = station.effort >  type.effort >  allgemeine Regel
  „allgemeine Regel“ = nichts setzen (heute erbt die Session von der mit der höchsten Nummer; nach #47 gelten die Einstellungen)
Resolved { agent_type: Option<&AgentType>,
           model: Option<ModelId>,   model_from: Option<AgentValueSource>,     // Station | Type
           effort: Option<Effort>,   effort_from: Option<AgentValueSource> }   // Station | Type
```

`AgentValueSource` steht in `agent_types/model.rs` (Phase 1). `StationValues { stage, agent, model, effort }` ist die Eingabe der Auflösung; Phase 2 baut sie aus den Feldern der Station. `UnknownAgent { name, known }` trägt den getrimmten Namen und die Namen der Typen in der Reihenfolge der Anlage; sein Text ist „Der Agententyp „X“ ist unbekannt. Bekannt: a, b“ (ohne Typen „Bekannt: keine“). Modell und Denkaufwand lösen sich **unabhängig** auf: Eine Station mit `effort: max` und ohne `model` nimmt das Modell des Typs; das Paar verlangt nur die Definition eines Typs. `station.model` und `station.effort` gelten in jeder Betriebsart, wie das Modell der Station heute; `type.model` und `type.effort` wendet der Start nur in der Betriebsart Claude an. Ob die Kommandozeile eine Kombination (etwa Haiku mit `max`) annimmt, prüft der Verwalter nicht; die Eingabeleiste lässt dieselben Kombinationen zu. „Gesamt-Review“ trifft den Typ nicht, der „Review“ führt; `review` und ` Review ` treffen ihn. Der Vergleich der Stationsnamen für Fix-Runde, Umsetzung und Abschluss in `implementation/mod.rs` bleibt, wie er ist (wörtlich, mit Beachtung der Schreibweise).

### Station: Eingabe, Ansicht, Datenbank (Phase 2)

```rust
// implementation/model.rs
StationInput { ...,                                  // alle neuen Felder: serde default, skip_serializing_if none
  agent: Option<String>,            // getrimmt, 1..40; unbekannter Name: Fehler (Werkzeuge)
  effort: Option<String>,           // 'low' | 'medium' | 'high' | 'xhigh' | 'max' (wie `model` als Text, damit `validate` das Feld nennt)
  prompt: Option<String>,           // Zusatztext zum Systemprompt, bis 4 000 Zeichen
}
StationView  { ...,
  agent: Option<String>,            // Feld der Station
  effort: Option<Effort>,           // ausdrücklicher Denkaufwand der Station (wie `model`)
  prompt: Option<String>,           // Zusatztext der Station
  agent_type: Option<String>,       // Typ, als der die Station läuft oder laufen wird; nach dem Start der gespeicherte Name
  agent_model: Option<ModelId>,     // vor dem Start aufgelöst; None = allgemeine Regel
  agent_model_from: Option<AgentValueSource>,   // Station | Type
  agent_effort: Option<Effort>,     // vor dem Start aufgelöst; None = allgemeine Regel
  agent_effort_from: Option<AgentValueSource>,  // Station | Type
  start_error: Option<String>,
}
```

```sql
ALTER TABLE implementation_stations ADD COLUMN agent TEXT;
ALTER TABLE implementation_stations ADD COLUMN agent_used TEXT;   -- Typname beim Start
ALTER TABLE implementation_stations ADD COLUMN effort TEXT;       -- ausdrücklicher Denkaufwand der Station
ALTER TABLE implementation_stations ADD COLUMN prompt TEXT;       -- Zusatztext der Station
ALTER TABLE implementation_stations ADD COLUMN start_error TEXT;
ALTER TABLE implementation_stations ADD COLUMN edited INTEGER NOT NULL DEFAULT 0;   -- 1 = auf der Karte angepasst (N15)
```

Nach dem Start zeigt die Karte Modell und Denkaufwand der Session (`SessionSummary`), nicht die aufgelösten Werte. Der Zusatztext im Feld `prompt` gilt an einer Station ohne Typ ebenso wie an einer mit.

### Start einer Station (Phase 2 und 4)

1. Die vorhandenen Prüfungen (keine Session, nicht fertig, nicht „durch dich“, `schedule::may_start`; Letzteres liefert `false`, solange `start_error` gesetzt ist).
2. Auflösen. `UnknownAgent { name, known }`: Fehler mit der Meldung aus 3 (`name`). Ab Phase 4: Text zusammensetzen (Text des Typs, seine Dateien, Zusatztext der Station); Fehler bei fehlender oder nicht lesbarer Datei und bei Überschreitung der Grenze.
3. Bei einem Fehler in 2: `start_error` schreiben, ankündigen, **keine** Session anlegen. Meldungstexte: „Den Typ „X“ gibt es nicht mehr.“, „Die Datei „P“ aus dem Typ „X“ wurde nicht gefunden.“, „Die Datei „P“ aus dem Typ „X“ ist nicht lesbar.“, „Der Systemprompt dieser Station hat N Zeichen; erlaubt sind 16 000.“ Ein manueller Start gibt denselben Text als Fehler zurück.
4. Erst danach `create_fresh_in_project`, `bind_station`, `rename`.
5. `set_model` mit dem ausdrücklichen Modell der Station, wie bisher in jeder Betriebsart; fehlt es und ist die Betriebsart Claude, mit dem Modell des Typs. `set_effort` ebenso: mit dem ausdrücklichen Denkaufwand der Station in jeder Betriebsart; fehlt er und ist die Betriebsart Claude, mit dem des Typs. Beide nur, wenn der Wert von dem der neuen Session abweicht. In den anderen Betriebsarten ändert der Typ weder Modell noch Denkaufwand; ob ein ausdrücklicher Wert der Station dort wirkt, entscheidet die Betriebsart wie heute beim Modell, der Start ruft `set_model` und `set_effort` unabhängig davon auf.
6. `agent_used` schreiben, `start_error` leeren; ab Phase 4 den Schnappschuss an der Session speichern.
7. `send(first_message)`.

Alles Fehlbare liegt vor Schritt 4. Scheitert ein Schritt danach, bleibt die Station gebunden wie heute.

### Fix-Runde (Phase 2)

`plan_completion` behält Signatur und Verhalten (die neun Tests in `implementation/tests.rs` bleiben der Beleg). Neu ist `plan_completion_with_types(…, types)`, die der Betrieb aufruft.

- Zeile „Fix-Runde“: `agent`, `effort` und `prompt` der letzten Station „Umsetzung“ der Phase, sonst der letzten „Abschluss“, sonst keine. Modell und Skill wie bisher von dort. Der Rückfall (`FALLBACK_FIX_MODEL`, Skill `implement`) gilt nur, wenn weder ein Modell kopiert wird noch ein Typ die Station führt: Führt ein Typ den Namen „Fix-Runde“, schreibt der Rückfall kein Modell.
- Zeile „zweites Review“: `agent`, `effort` und `prompt` der meldenden Station, dazu Stufe, Modell, Skill und Auftrag wie bisher.

### Werkzeuge (Phase 5)

- Neu, lesend: `implementation_agent_types`, Eingabe `{}`, Ergebnis `structuredContent` und Text gleich: `{ "agentTypes": [ { "name", "description", "stages", "model", "effort", "skills" } ] }`; `model` und `effort` sind `null` bei „Standard“; nie `prompt` oder `files`. Ohne Typen eine leere Liste. Erreichbar in einem Vorhaben ohne Umsetzung, im Modus Plan ohne Rückfrage und in Nebenfragen.
- `implementation_create` und `implementation_set_phases`: je Station die optionalen Felder `agent` (1 bis 40), `effort` (`low`, `medium`, `high`, `xhigh`, `max`) und `prompt` (Zusatztext bis 4 000 Zeichen). Ein unbekannter Wert lässt den Aufruf scheitern, mit Feld und Wert, etwa `phases[0].stations[1].agent: unbekannter Agententyp „reviewr“; bekannt: developer, reviewer, tester`.
- Kein Werkzeug schreibt Typen, keines nimmt einen Dateipfad an; `prompt` ist Text, kein Pfad.
- `session_tools_prompt` bekommt einen festen Satz, der das Werkzeug beim Namen nennt; er hängt nicht von den Typen ab und geht nur im Claude-Betrieb mit.

### Schnappschuss des Systemprompts (Phase 4)

- Aufbau: `[Text des Typs, Datei 1, Datei 2, …, Zusatztext der Station]` ohne leere Teile, mit einer Leerzeile dazwischen. Frontmatter jeder Datei fällt weg. Obergrenze für das Ganze: 16 000 UTF-16-Einheiten, der Zusatztext eingerechnet.
- Pfade: absolut, `~/…` oder relativ; relativ gilt gegen die Repositories des Vorhabens in deren Reihenfolge, erster Treffer.
- Weg: Der Text ist der letzte Teil des einen `--append-system-prompt`, hinter Scratchpad, Artefakten und Session-Werkzeugen.
- Speicher: Spalte `sessions.agent_prompt`; jeder spätere Prozessstart und jede Nebenfrage dieser Session nutzt dieselben Bytes (Prompt-Cache, [ADR 044](044-nebenfragen.md)). Rückfragen und spätere Sessions bekommen den Text nicht.

### Skills in der ersten Nachricht (Phase 4)

Erster Skill ist `skill` der Station, sonst der erste des Typs; er steht allein als `/name` in der ersten Zeile. Die übrigen Skills des Typs nennt die Nachricht in einem Satz als Anweisung, sie zu laden.

### Einstellungen einer Station ändern (Phase 5)

```rust
// implementation/model.rs; derive(Deserialize, TS), serde camelCase; Eingabe des Befehls
pub struct StationSettings {
    pub agent: Option<String>,    // None = nach Stationsname; sonst der Name eines Typs
    pub model: Option<ModelId>,   // None = vom Typ
    pub effort: Option<Effort>,   // None = vom Typ
    pub prompt: Option<String>,   // None oder leer = kein Zusatztext; bis 4 000 Zeichen
    pub skill: Option<String>,    // None = keiner; 1 bis 40 Zeichen, ein führender „/“ fällt weg
}
```

```text
implementation_set_station_agent(id, stationId, settings: StationSettings) -> ()
Klasse in remote/access.rs: C (HostWrite, IdKind::Implementation), neben implementation_start_station
```

Der Befehl ersetzt alle fünf Werte der Station auf einmal; `None` löscht eine Abweichung. Er setzt die Marke `edited` und meldet die Ansicht. Fehler (`CommandError::Implementation`): „Die Station hat schon begonnen.“, „Diese Station erledigst du selbst.“, „Der Agententyp „X“ ist unbekannt. Bekannt: …“, „prompt: höchstens 4000 Zeichen, nicht N“ und „skill: …“. Er wirkt auf keine Session und startet nichts.

### Plan-Änderung (Phase 5)

`changes::apply` ruft nach `phase_rows` die reine Funktion `keep_settings(removed, &mut stations)`: Eine neue Station nimmt von der alten mit gleichem Phasenschlüssel, gleicher Stufe (wörtlich) und gleicher laufender Nummer `agent`, `model`, `effort`, `prompt` und `skill`, wenn die alte `edited` trägt und die Eingabe den jeweiligen Wert nicht nennt; sie trägt dann selbst `edited`. Stationen ohne `edited` ersetzt die Plan-Änderung wie heute vollständig.

## Stand der Umsetzung

**Gebaut in Phase 1 (Typen im Core):** Tabelle `agent_types` (Migration 22), die vier Befehle mit dem Ereignis `agent-types://changed`, alle in Klasse G; die Prüfung mit den Meldungen der Tabelle und den Festlegungen oben, Prüfen und Schreiben in einer Transaktion; die Auflösung als reine Funktion (`agent_types::resolve`); die Bindings `AgentType`, `AgentTypeInput`, `AgentTypeField`, `FieldError`, `AgentTypeSaved` und `AgentValueSource`.

**Belegt (unter Linux):** die Tests in `src-tauri/src/agent_types/tests.rs` (Auflösung und jede Zeile der Prüfung, mit Mutationsprobe); die Probe `agent-type-probe --selftest` (Anlegen, Ändern, Löschen, doppelter Name, doppelter Stationsname, Neustart); die Migration auf einer leeren Datenbank und auf Kopien einer echten (von Version 20 und von 21 auf 22, ohne verletzte Fremdschlüssel). Die Tests fährt künftig auch der Schritt `cargo test --manifest-path src-tauri/Cargo.toml --lib -- implementation:: agent_types::` in `check.yml`.

**Gebaut in Phase 2 (Typen im Ablauf):** die Felder `agent`, `effort` und `prompt` an der Station (Migration 23 mit `agent_used`, `start_error` und `edited`, das erst Phase 5 schreibt), ihre Prüfung (`effort` und `prompt` in `validate_station`, `agent` gegen die Typen in `implementation_create` und `implementation_set_phases`), die Auflösung in der Ansicht, der Start nach der Reihenfolge oben samt Startfehler, die Fix-Runde mit `plan_completion_with_types` und das Nachführen beim Umbenennen. Belegt durch die Tests in `src-tauri/src/implementation/tests.rs` (mit Mutationsprobe), die Probe `implementation-probe` und einen Lauf der App mit einer Attrappe der Kommandozeile unter Linux.

**Gebaut in Phase 3 (Einstellungen):** Liste und Dialog der Agententypen in den Einstellungen (Kategorie Agent); siehe Glossar und Code-Map.

**Gebaut in Phase 4 (Systemprompt, Dateien, Skills):** der Systemprompt einer Station aus Text und Dateien ihres Typs und ihrem Zusatztext (`agent_types::prompt`), als Schnappschuss an der Session (Spalte `sessions.agent_prompt`, Migration 24) und als letzter Teil des einen `--append-system-prompt` jedes Prozesses und jeder Nebenfrage dieser Session; die Startfehler für fehlende, nicht lesbare und zu große Dateien; die Skills in der ersten Nachricht. Belegt (unter Linux) durch die Tests in `src-tauri/src/agent_types/tests.rs` und `src-tauri/src/implementation/tests.rs` (mit Mutationsprobe; einer baut die Argumente aller vier Betriebsarten und liest sie wie Autark und Codex), die Probe `agent-prompt-probe --selftest`, die Migration auf einer leeren Datenbank und auf einer Kopie einer echten (von 21 auf 24) und einen Lauf der App mit Attrappen: In allen vier Betriebsarten stand der Text zuletzt im einen `--append-system-prompt` (`/proc/<pid>/cmdline` und Protokoll der Attrappe), Autark gab ihn als Ende seines Systemprompts an das Modell; Neustart im Leerlauf, Nebenfrage, Neustart der App und Wiederherstellen des Vorhabens nahmen dieselben Bytes; Rückfrage und spätere Session bekamen ihn nicht; eine fehlende, eine nicht lesbare und eine zu große Datei hielten die Station ohne Session an, die zu große auch beim automatischen Start, ▶ startete nach der Korrektur. Zwei echte Läufe mit Haiku (Kontingent): Nach dem Neustart im Leerlauf las der zweite Zug den ganzen Präfix aus dem Cache, mit Aufzeichnung 34 222 von 34 539 Eingabe-Token und mit `--system-prompt-snapshot off` 41 575 von 42 404. Die Nebenfrage traf den Cache mit Aufzeichnung (34 537 von 40 704), ohne Aufzeichnung nicht (0 von 42 861), obwohl ihr `--append-system-prompt` byte-gleich war; vermutlich rendert die Kommandozeile den Systemprompt im Modus `dontAsk` der Nebenfrage anders als im Modus der Session.

**Gebaut in Phase 5 (Werkzeuge und Stationsbefehl):** das Lesewerkzeug `implementation_agent_types` (als lesend gekennzeichnet, in Nebenfragen erlaubt), `agent`, `effort` und `prompt` im Schema von `implementation_create` und `implementation_set_phases`, der Satz in `session_tools_prompt`, der Befehl `implementation_set_station_agent` (Klasse C, Binding `StationSettings`) und die Plan-Änderung, die Einstellungen mit `edited` behält. Belegt durch die Tests in `src-tauri/src/implementation/tests.rs` (mit Mutationsprobe), die Probe `implementation-probe` (`set-station`, `set-phases --apply`), einen Lauf der App mit einer Attrappe der Kommandozeile und `mcp-probe` (Hauptprozess und Nebenfrage) und drei Läufe mit Haiku, alles unter Linux.

**Gebaut in Phase 6 (Karte und Stationseinstellungen):** die Zeile mit Typ, Modell und Denkaufwand auf der Karte einer Station samt Herkunft, der Chip „Zusatztext · N Zeichen“, der Startfehler im Fehlerton mit „Typ bearbeiten“, der Typname in Zeitleiste und Kartei, „startet nicht“ als Meta, Nachladen bei `agent-types://changed` und der Dialog „Einstellungen der Station“, der den Befehl aus Phase 5 aufruft. Belegt (unter Linux) durch einen Lauf der App mit Wegwerf-`HOME`, einer Attrappe der Kommandozeile und Bildschirmfotos in Hell und Dunkel (Entwurfsordner, README), ohne Test-Läufer für die Oberfläche.

**Noch nicht gebaut:** die Phase 7 (Abschluss). Ohne Typen und ohne Zusatztext läuft eine Umsetzung wie vor dieser ADR.

**Windows:** Unter Windows ist nichts davon gelaufen, auch nicht der Start einer Station mit Typ, das Lesen der Dateien, Pfade wie `~/…` und relative Pfade, die Grenze der Kommandozeile (32 767 Zeichen), der Befehl oder die Werkzeuge. Die CI fährt dort Lint, Typecheck, Build, Clippy und die Tests von `remote::`, `processes::`, `implementation::` und `agent_types::`; die Probe läuft in keiner CI.

## Konsequenzen

- Die vier Befehle stehen in Klasse G: Gäste erreichen sie nicht, und das Ereignis geht nicht über das Netz.
- Namen mit Logik: `fix_rounds` und `last_of` vergleichen Stationsnamen wörtlich und mit Beachtung der Schreibweise, die Zuordnung der Typen nicht. `umsetzung` trifft den Typ, der „Umsetzung“ führt, zählt für die Fix-Runde aber nicht als „Umsetzung“. Das bleibt so.
- `StationInput` verwirft unbekannte Felder still (kein `deny_unknown_fields` im Crate): Ein Tippfehler wie `agnet` verliert den Typ ohne Meldung (N11).
- Der Zusatztext eines Planers landet im Systemprompt einer anderen Session und wiegt dort mehr als der Auftrag. Dagegen: höchstens 4 000 Zeichen, kein Pfad, vor dem Start auf der Karte sichtbar, und die erste Station startet der Nutzer selbst.
- Planer und Karte schreiben dieselben fünf Werte einer Station; die letzte Änderung gewinnt, ohne Schutz vor verlorenen Änderungen.
- Eine Plan-Änderung kann eine Einstellung des Nutzers nicht löschen, nur überschreiben. Die Einstellungen hängen an Phasenschlüssel, Stufe und laufender Nummer: Wer Stufen umbenennt oder Phasen neu nummeriert, verliert sie.
- Die Marke `edited` gilt für die ganze Station, nicht je Wert. Die neue Station einer Plan-Änderung trägt sie auch für die Werte, die der Planer genannt hat, und eine spätere Plan-Änderung, die so einen Wert weglässt, behält ihn wie eine Einstellung des Nutzers. An einer angepassten Station kann der Planer deshalb keinen der fünf Werte löschen, auch keinen eigenen: Ein Zusatztext, den er einmal gesetzt hat, bleibt an der Station und geht beim Start in ihren Systemprompt, bis er ihn überschreibt oder der Nutzer ihn mit `implementation_set_station_agent` löscht.
- `check.yml` fährt die Tests von `implementation::` und `agent_types::`, also auch unter Windows.
