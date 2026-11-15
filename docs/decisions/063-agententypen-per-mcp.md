# 063 — Agententypen per MCP vorschlagen

**Status:** angenommen; Werkzeuge, Kasten-Mechanik und Befehl umgesetzt (Phase 2 der Story Taiga #109), die Oberfläche des Kastens samt Dialog folgt (Phase 3) · **Datum:** 2026-11-15

## Kontext

Agententypen legt bisher nur der Benutzer in den Einstellungen an ([ADR 060](060-agententypen.md)); V13 dort sagt: Kein Werkzeug schreibt Typen oder nimmt Pfade an, der Planer liest sie nur über `implementation_agent_types` (ohne Systemprompt und Dateien). Die Story (Taiga #109) verlangt, dass ein Agent einen Typ vollständig lesen und einen neuen Typ oder eine Änderung vorschlagen kann. Wirksam wird ein Vorschlag erst, wenn der Benutzer ihn übernimmt.

Vorgaben des Benutzers vom 2026-11-15:

| | Frage | Antwort |
|---|---|---|
| U1 | Auftrag | Story und Plan, Umsetzung in drei Phasen |
| U2 | Operationen per MCP | Anlegen und Ändern, dazu ein Lesewerkzeug mit Systemprompt und Dateien; **kein Löschen** |
| U3 | Wie wird ein Vorschlag wirksam | Vorschlag und „Übernehmen“ in der Oberfläche, nie direkt gespeichert |
| U4 | Ablauf | Kasten im Chat wie die Freigabe von `verwalter_send_message_to`: Das Werkzeug wartet, die Entscheidung ist sein Ergebnis; nur Betriebsart Claude, nur im Speicher |
| U5 | Dateipfade im Vorschlag | Erlaubt, mit begrenztem Lesen beim Start (Phase 1, ADR 060 V6) |
| U6 | Aktionen am Vorschlag | Übernehmen, Bearbeiten (Dialog vorbefüllt), Verwerfen mit optionaler Begründung |

## Entscheidung

### Werkzeuge

Ein neuer Werkzeug-Satz `AgentTypeTools` (`src-tauri/src/agent_types/tools.rs`, Präfix `agent_type_`, in `mcp_access::tool_sets()`): `requires_claude` (nur im Claude-Betrieb sichtbar, auch das Lesewerkzeug, D5), `blocks` (Antwort als Ereignis-Strom mit Fortschritt alle 30 s), lesend (`readOnlyHint`) nur `agent_type_get`. Eine Nebenfrage darf nur `agent_type_get` rufen, sonst „Eine Nebenfrage darf Agententypen nur lesen.“. Kein Satz in `session_tools_prompt`; die Beschreibungen der Werkzeuge reichen (D6).

- **`agent_type_get`** `{ name }` (1 bis 40 Zeichen, ohne Beachtung der Schreibweise): `{ "agentType": AgentType }` mit allen Feldern, auch `prompt` und `files`; unbekannt: „Kein Agententyp „<name>“.“.
- **`agent_type_propose_create`**: `name` und `description` Pflicht, dazu `stages`, `model`, `effort`, `prompt`, `files`, `skills`; Fehlendes ist leer bzw. „Standard“.
- **`agent_type_propose_update`**: `target` (heutiger Name, ohne Beachtung der Schreibweise) Pflicht, dazu dieselben acht Felder, alle optional. Ein genanntes Feld ersetzt den gespeicherten Wert, ein fehlendes behält ihn; `"model": null` und `"effort": null` setzen „Standard“ (fehlend und `null` sind verschieden). Zwei Werkzeuge statt eines mit optionalem Ziel, denn die Pflichtfelder hängen vom Ziel ab (D1).
- Alle Schemas mit `additionalProperties: false`; der Core liest die Argumente mit `deny_unknown_fields`, ein Tippfehler im Feldnamen scheitert also auch, wenn die Kommandozeile das Schema nicht prüft.

### Ablauf eines Vorschlags

1. Aus den Argumenten entsteht die vollständige `AgentTypeInput` (bei einer Änderung: gespeicherter Typ plus Änderungen).
2. Geprüft wird mit derselben Funktion wie im Dialog (`agent_types::validate`), gegen die gespeicherten Typen; bei einer Änderung kollidiert der Typ nicht mit sich selbst. Meldungen gehen als `isError` zurück, eine Zeile je Fehler `<feld>: <Meldung>` (Feld wie `AgentTypeField` in camelCase), **ohne Kasten**.
3. Sonst öffnet `SessionRegistry::tool_propose_agent_type` immer einen Kasten (`QuestionKind::SessionTool`, `ToolRequest::AgentType { base, proposal }`) und wartet. Nie Selbstfreigabe, keine Kettenbremse (D4): Jeder Vorschlag braucht einen Klick. `proposal` ist die normalisierte Eingabe aus `validate`.
4. Ergebnis als `structuredContent` und gleicher Text: übernommen `{ "decision": "accepted", "edited": bool, "agentType": AgentType }` (`edited`: der gespeicherte Typ weicht vom Vorschlag ab, der Benutzer hat ihn also im Dialog geändert), verworfen `{ "decision": "rejected", "reason": string }`. Agent gestoppt, Aufruf abgebrochen und eine wiederholte `tool_use_id` enden mit denselben Texten wie bei den Session-Werkzeugen ([ADR 052](052-session-werkzeuge.md)).

### Kasten und Antwort

- **Kanal:** Der offene Aufruf hängt wie bei den Session-Werkzeugen als `PendingTool` an der Rückfrage. Er wartet aber nicht auf eine `QuestionAnswer`, sondern auf sein fertiges Ergebnis (`Waiter::Result`); senden kann es allein `SessionRegistry::finish_agent_type_proposal`. Abbruch (`cancel_tool_call`) und Stopp (`abandon_pending`) lassen den Sender fallen: Antwortzeile „Abgebrochen“, nichts gespeichert.
- **Befehl** `agent_type_proposal_answer(session_id, request_id, decision, origin)` mit `ProposalDecision` = `Accept { input }` (Übernehmen mit dem Vorschlag, oder Speichern im Dialog mit dem bearbeiteten Stand) oder `Reject { reason }`; Klasse C mit Session-ID wie `chat_answer`, die Herkunft trägt bei einem Gast der Server ein. Unbekannte Session, ein Kasten, der nicht offen ist, und ein Kasten anderer Art sind ein `CommandError`; gespeichert wird dann nichts.
- **Gespeichert wird im Befehl**, nicht im wartenden Werkzeug (D2): So sieht der Benutzer Prüfmeldungen im Kasten, der offen bleibt, und der Agent wartet weiter. Ein neuer Typ geht über `agent_types::create`, eine Änderung über `agent_types::update_if_unchanged`. Danach `emit_changed` wie bei den übrigen Befehlen, dann das Ergebnis an das Werkzeug und die Antwortzeile „Übernommen“ oder „Übernommen mit Änderungen“.
- **Veraltet** (D3): Eine Änderung wird nur übernommen, solange der gespeicherte Typ mit der ID von `base` noch genau `base` ist. Geprüft wird in derselben Transaktion wie das Schreiben (`db::agent_types::update_if_unchanged`). Sonst kommt `AgentTypeSaved { agentType: null, errors: [{ field: "name", message: "Der Typ „<Name>“ wurde seit dem Vorschlag geändert oder gelöscht. Verwirf den Vorschlag." }] }`, und der Kasten bleibt offen. Eine eigene Variante von `AgentTypeField` oder `CommandError` hätte jedes erschöpfende `Record` der Oberfläche berührt.
- **Verwerfen** antwortet mit der getrimmten Begründung; Antwortzeile „Verworfen“ oder „Verworfen: <Begründung>“.
- **Kein Weg daneben:** `chat_answer` auf einen Vorschlag lehnt jede Antwort ab („Einen Vorschlag für einen Agententyp beantwortet nur agent_type_proposal_answer.“), denn „Erlauben“ würde am Speichern vorbei übernehmen. Freier Text im Eingabefeld, der sonst die älteste offene Rückfrage ablehnt, **verwirft** einen Vorschlag mit dem Text als Begründung; gespeichert wird so nie etwas.
- **Fenster zwischen Speichern und Antwort** (D8, `ponytail:` im Befehl): Speichern und Antwort liegen nicht unter einer Sperre, sonst läge die Datenbank unter der Session-Sperre. Bricht der Agent genau dazwischen ab, bleibt der Typ gespeichert, der Kasten steht auf „Abgebrochen“, und der Befehl liefert trotzdem den gespeicherten Typ.

### Oberfläche in Phase 2

Nur das Nötigste (D7), damit zwischen den Merges kein Agent an einem Kasten hängen bleibt: `SessionToolBlock` gibt einen Vorschlag an `AgentTypeProposalBlock` weiter, der den Namen des Typs nennt und nur „Verwerfen“ anbietet (`Reject { reason: "" }` über den Wrapper `answerAgentTypeProposal` in `src/lib/agentTypes.ts`), ohne Schalter der Selbstfreigabe. Den Kasten mit allen Feldern, alt und neu, Fehlerliste, Bearbeiten im Dialog und Begründung baut Phase 3 und ergänzt diesen ADR.

## Verworfen

- **Ein Werkzeug mit optionalem Ziel** (D1): Pflichtfelder hingen vom Ziel ab, für kleine Modelle schwerer.
- **Speichern im wartenden Werkzeug** (D2): „Übernommen“ stünde im Chat, auch wenn das Speichern danach scheitert.
- **Schalter in „Selbstfreigabe“** (D4): eine neue Einstellung ohne Bedarf; ein Agent schreibt hier Systemprompts anderer Agenten.
- **LM Studio und Autark** (D5): Autark bricht wartende Aufrufe nach 300 s ab.
- **Satz in `session_tools_prompt`** (D6): erst nachrüsten, wenn der Probelauf zeigt, dass Agenten die Werkzeuge nicht finden.
- **Speichern und Antwort unter einer Sperre** (D8): Datenbank unter der Session-Sperre.
- **Löschen per MCP** (U2): nicht gewünscht.

## Folgen

- ADR 060 V13 ist teilweise abgelöst: Ein Werkzeug nimmt Typen und Pfade an, aber nur als Vorschlag, den der Benutzer übernimmt. Das begrenzte Lesen der Dateien beim Start (Phase 1) gilt für alle Typen.
- Offene Kästen leben nur im Speicher; nach einem Neustart der App ist ein Vorschlag verloren, der Agent bekommt den Fehler „Der Kasten wurde geschlossen, ohne dass der Nutzer entschieden hat.“.
- Tests in `agent_types/tests.rs` über freie Funktionen (Schema, lesend, Nebenfrage, Eingabe, Prüfung, Ergebnis, Antwortzeilen) und eine Datenbank im Speicher (veraltet, nichts gespeichert bis Übernehmen), dazu `chat_answer` lehnt ab und die Klasse des Befehls; kein `Box<dyn ToolSet>` der echten Sätze, damit kein Tauri-Code ins Testprogramm kommt (Windows).
- **Offen:** der Kasten mit allen Feldern und der Dialog mit eigenem Speichern (Phase 3), der Probelauf mit einer echten Claude-Session.
