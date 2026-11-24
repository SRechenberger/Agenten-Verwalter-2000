# 063 — Agententypen per MCP vorschlagen

**Status:** umgesetzt (Story Taiga #109: begrenztes Lesen in Phase 1, Werkzeuge, Kasten-Mechanik und Befehl in Phase 2, Oberfläche des Kastens samt Dialog in Phase 3) · **Datum:** 2026-11-15, Oberfläche 2026-11-16

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
- **Fenster zwischen Speichern und Antwort** (D8, `ponytail:` im Befehl): Speichern und Antwort liegen nicht unter einer Sperre, sonst läge die Datenbank unter der Session-Sperre. Bricht der Agent genau dazwischen ab, bleibt der Typ gespeichert, der Kasten steht auf „Abgebrochen“, und der Befehl liefert trotzdem den gespeicherten Typ. Dasselbe Fenster öffnet sich, wenn Host und Gast fast gleichzeitig „Übernehmen“ und „Verwerfen“ klicken: Das Verwerfen kann den Kasten zwischen Speichern und Antwort schließen; dann ist der Typ gespeichert, der Agent bekommt `rejected`, und `finish_agent_type_proposal` scheitert danach still.

### Oberfläche

Entwurf und Bilder der gebauten App: [`docs/design/2026-11-15_agententypen-mcp/`](../design/2026-11-15_agententypen-mcp/). `SessionToolBlock` gibt einen Vorschlag an `AgentTypeProposalBlock` (`src/features/agenttypes/`) weiter; alles Übrige bleibt beim Kasten der Session-Werkzeuge.

- **Offen:** Kopf „Claude schlägt einen neuen Agententyp vor“ oder „… eine Änderung an „<Name>“ vor“ mit der Marke „wartet auf dich“. Ein neuer Typ zeigt alle Felder; eine Änderung nur die geänderten, alt und neu (Listen: „+“ für Neues, durchgestrichen für Wegfallendes), darunter „Unverändert: …“. Dateien stehen hervorgehoben mit dem Satz, dass der Start sie liest; der Systemprompt ist eingeklappt, mit „Ganz zeigen“ und Zähler. Knöpfe „Übernehmen“ (`Accept` mit dem Vorschlag), „Bearbeiten …“ und „Verwerfen“; Verwerfen öffnet „Begründung für Claude (optional)“ mit „Verwerfen“ und „Zurück“. Meldungen aus `AgentTypeSaved.errors` (auch „veraltet“) und ein `CommandError` stehen als Liste über den Knöpfen; der Kasten bleibt offen. Das offene Feld, die Begründung und die Meldungen leben wie die halben Eingaben der übrigen Kästen in `ChatTimeline` (`ToolDraft`), denn der virtualisierte Verlauf hängt den Kasten beim Scrollen aus. Darunter bleibt der Satz „Text im Eingabefeld unten verwirft den Vorschlag mit dieser Begründung.“ (siehe *Kein Weg daneben*).
- **Bearbeiten** öffnet `AgentTypeDialog` mit `proposal` (`initial`, Nummer der Session, `onSave`): Titel „Vorschlag bearbeiten“, Knopf „Übernehmen“, sonst derselbe Dialog wie in den Einstellungen mit allen Bedienelementen und den Meldungen am Feld; nur „Datei wählen …“ fehlt bei einem entfernten Vorhaben, denn der Pfad gilt auf dem Host. `onSave` schickt `Accept` mit dem bearbeiteten Stand; ohne Meldungen schließt der Dialog, und der Kasten ist beantwortet. Wird der Kasten anders beantwortet (Stopp, freier Text), schließt der Dialog mit ihm.
- **Beantwortet:** eine Zeile „Agententyp „<Name>““ (der Name des Vorschlags, auch wenn der Dialog ihn geändert hat; die Antwortzeile des Core trägt den gespeicherten Namen nicht) mit der Marke aus der Antwortzeile des Core, nicht aus eigenem Zustand: „Übernommen“ und „Übernommen mit Änderungen“ (dazu „In den Einstellungen öffnen“, nicht bei einem entfernten Vorhaben), „Verworfen“ mit der Begründung und „Vorschlag zeigen“, „Abgebrochen“, „Pausiert“ und „Nicht beantwortet“ als graue Marke mit „Der Agent wurde gestoppt; nichts gespeichert.“. Meldungen stehen als „Feld: Meldung“, „veraltet“ ohne Feld.
- **Gesperrt** sind die Knöpfe während einer Antwort und solange das Vorhaben unter fremder Kontrolle steht (dieselbe Sperre wie die übrigen Kästen im Verlauf).
- **Keine Ziffern-Tasten:** „1“, „2“ und Enter des Verlaufs übergehen einen offenen Vorschlag (`handleDigit` in `ChatTimeline`), denn sie antworten über `chat_answer`, das jeden Vorschlag abweist.
- **Abweichungen vom Entwurf** nennt die README des Entwurfs.

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
- Tests in `agent_types/tests.rs` über freie Funktionen (Schema, lesend, Nebenfrage, Eingabe, Prüfung, Ergebnis, Antwortzeilen) und eine Datenbank im Speicher (veraltet, nichts gespeichert bis Übernehmen), dazu `chat_answer` lehnt ab und die Klasse des Befehls; freier Text verwirft einen offenen Vorschlag (`sessions/registry/session_tools/agent_types.rs`, ohne `AppHandle`, läuft unter dem Filter `agent_types::`), und `remote::` prüft, dass der Server `origin` des Befehls überschreibt; kein `Box<dyn ToolSet>` der echten Sätze, damit kein Tauri-Code ins Testprogramm kommt (Windows).
- Gleichzeitiges „Übernehmen“ und „Verwerfen“ von Host und Gast kann einen gespeicherten Typ hinterlassen, den der Agent für verworfen hält (Fenster D8, siehe *Kasten und Antwort*). Der Benutzer sieht ihn in den Einstellungen und kann ihn löschen.
- Die Oberfläche hat keine automatischen Tests; belegt ist sie durch die Bilder in `docs/design/2026-11-15_agententypen-mcp/umsetzung/` (Linux).
- **Offen:** der Probelauf mit einer echten Claude-Session; unter Windows nur, was die CI dort prüft.
- Prompt-Vorlagen folgen diesem Muster mit eigenem Werkzeug-Satz, Kasten und Befehl: [ADR 075](075-prompt-vorlagen.md), Abschnitt „Per MCP“.
