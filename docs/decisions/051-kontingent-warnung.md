# 051 — Aufgebrauchtes Kontingent als Warnung, Session pausiert mit Begründung

**Status:** angenommen · **Datum:** 2026-11-03

## Kontext

Endet eine Antwort am aufgebrauchten Kontingent, kam das bisher als `TurnEnd::Failed` an: Fehlerkarte im Verlauf, Status `error`, `send` antwortet mit `AgentStopped`, die Eingabeleiste sperrt. Das ist kein Fehler des Agenten. Die Session ist in Ordnung und könnte weiterlaufen, sobald das Kontingent zurückgesetzt ist.

Das Format ist nur halb bekannt. Belegt sind lokale Transkripte der Kommandozeile (Version 2.1.310): eine synthetische `assistant`-Zeile mit `error: "rate_limit"`, `apiError: "usage_limit_reached"`, `apiErrorParams.rate_limit_info.resetsAt`, `quotaLimits.resetsAt` und dem Text `You've hit your session limit · resets 6:30pm (Europe/Berlin)`. Im Stream der App (`--output-format stream-json`) wurde eine Sperre nie gemessen, auch nicht `rate_limit_event` mit `rejected`.

## Optionen

- **Zustand:** (a) neuer Status „wartet auf Kontingent“; (b) `paused` mit einer Begründung. Gewählt (b): Pausiert sagt schon, was gilt (der Agent arbeitet nicht, die nächste Nachricht setzt fort), ein neuer Status müsste durch jede Status-Tabelle. Die Begründung lässt sich an weitere Wartezustände heften.
- **Lebensdauer:** (a) in der Datenbank, übersteht den App-Neustart; (b) nur im Speicher. Gewählt (b): keine Migration, und nach einem Neustart ist die Session ohnehin `paused`, nur ohne Begründung.
- **Erkennung:** (a) ein Signal wählen; (b) alle drei möglichen nutzen. Gewählt (b), weil der Stream nicht gemessen ist.
- **Rücksetzzeit:** (a) nur strukturiert; (b) strukturiert vor Text. Gewählt (b): fehlt das Feld, liest `usage::reset_text` die Uhrzeit aus der Meldung.

## Entscheidung

- **Die Erkennung liegt im Übersetzer** (`agents/claude/translate.rs`), der Zustand je Antwort kennt. Er merkt sich (1) `rate_limit_event` mit `status: "rejected"`, (2) eine `assistant`-Zeile des Hauptagenten (ohne `parent_tool_use_id`) mit `error: "rate_limit"` oder `apiError: "usage_limit_reached"`, (3) den `result`-Text, wenn er ohne Beachtung der Groß-/Kleinschreibung mit `You've hit your` beginnt und `limit` enthält.
- **Die Regel:** Kontingent-Fall, wenn (2) oder (3) zutrifft, gleich ob `is_error` gesetzt ist (die synthetische Meldung kann eine erfolgreiche Antwort beenden), oder wenn (1) zutrifft und `is_error` gesetzt ist (ein `rejected` bei Erfolg, etwa mit Überziehung, ist keiner). Ein Abbruch hat immer Vorrang. Alle Signale gelten für genau eine Antwort: die `result`-Zeile räumt sie ab, auch beim Abbruch.
- **Die synthetische Zeile wird nicht weiter übersetzt:** ihr Text stünde sonst doppelt im Verlauf, ihre leere `usage` setzte den Kontext auf null.
- **`TurnEnd::QuotaExhausted { text, resets_at }`** trägt nur den strukturierten Wert in Sekunden. Die Registry löst die Zeit über `reset_text::resolve` auf, dieselbe Stelle, die `quota-probe` prüft. Der Text ist der aus (2), sonst `result`, sonst „Das Kontingent ist aufgebraucht.“.
- **Die Registry** (`pause_for_quota`) schließt laufende Werkzeuge und offene Rückfragen ab, schreibt `Notice::QuotaExhausted` statt `ChatEntry::Error`, setzt `paused` und `PauseReason::Quota { resets_at, auto_resume: false }` (Millisekunden). Das geht der angeforderten Pause vor und gilt nur im Claude-Prozess; anderswo bleibt es ein Fehler.
- **`pause_reason` verschwindet** mit jeder neuen Nachricht, jedem Status außer `paused` und jedem Start des Prozesses. Beendet der Reaper den ruhenden Prozess, bleibt sie stehen: die Rücksetzzeit liegt oft Stunden entfernt, und die nächste Nachricht startet ihn ohnehin neu.
- **Endet der Prozess von selbst, solange die Begründung steht,** bleibt die Session `paused` statt `error`; `send` startet ihn neu.

## Konsequenzen

- **Stream nie gemessen.** Die Probe `quota-probe` arbeitet mit Zeilen, die aus Transkripten abgeleitet sind. Belegt ist nur, dass `rate_limit_event` mit `allowed` vor `result` kommt. Käme ein `rejected` erst danach, zählte es zur nächsten Antwort; endet die erfolgreich, bleibt es folgenlos, scheitert sie an etwas anderem, gilt sie fälschlich als Kontingent-Fall.
- **Endet der Prozess vor der `result`-Zeile,** greift die Regel nicht: die Session geht wie bisher auf `error`.
- **Wochen-Fenster:** Das Textformat ist nicht belegt. Ohne strukturiertes `resetsAt` bleibt die Rücksetzzeit dann `None`; die Warnung und die Pause gelten trotzdem.
- **Fehlalarme:** Eine Antwort des Modells, die selbst mit „You've hit your … limit“ beginnt, gilt als Kontingent-Fall; der `result`-Text ist von der Meldung nicht zu unterscheiden. Die nächste Nachricht setzt die Session normal fort. Auch ein vorübergehender Fehler mit `error: "rate_limit"` zählt, falls die Kommandozeile ihn so meldet (nicht gemessen).
- **Außerhalb des Claude-Prozesses** (LM Studio, Autark, Codex) wird ein Text, der die Regel (3) trifft, zur Fehlerkarte, auch bei einer erfolgreichen Antwort; `TurnEnd::QuotaExhausted` sagt nicht, ob `is_error` gesetzt war.
- **Warnkarte folgt:** bis dahin zeigt `NoticeBlock` die Meldung wie einen `Info`-Hinweis, und nichts in der Oberfläche ruft `session_set_auto_resume`.
- Die Archiv-Suche findet den Text der Meldung; TL;DR und Retro lassen ihn weg wie jeden anderen Hinweis.

## Nachtrag: Automatisch fortsetzen

- **Befehl:** `session_set_auto_resume(session_id, enabled)` legt `auto_resume` in `PauseReason::Quota` um. Ohne diese Begründung, bei `resets_at = None` und bei einem Status außer `paused` antwortet er mit `CommandError::AutoResumeUnavailable`. Doppelt `true` ändert nichts und startet keinen zweiten Zeitgeber.
- **Marke:** `SessionState.auto_resume_token`, wie in ADR 040. Jedes Umlegen erhöht sie, nur im Speicher.
- **Zeitgeber:** ein Thread `auto-resume` je Einschalten (`sessions/auto_resume.rs`), Ziel `resets_at` + 30 s. Er wartet in Schritten von höchstens 15 s auf die Wanduhr und übersteht so einen Ruhezustand des Rechners; danach trifft er höchstens einen Schritt zu spät. Er endet, sobald die Prüfung nicht mehr gilt oder die Session verschwunden ist.
- **Prüfung vor `send`:** Marke gleich, Status `paused`, dieselbe `resets_at`, `auto_resume` an. Sie muss vorher liegen: der Reaper beendet den ruhenden Prozess nach 30 Minuten, `send` startet ihn dann neu, und `start_process` löscht die Begründung vor dem Senden des Textes. Der Text geht über `registry.send`, es gibt keine eigene Sende-Logik.
- **Sendefehler:** `ChatEntry::Error` „Automatisches Fortsetzen fehlgeschlagen“ mit dem Fehlertext, `auto_resume` aus, Eintrag im Log, kein Wiederholen. Die Session bleibt pausiert.

### Verworfen

- **Persistenz:** Entscheidung des Users; keine Migration, keine Spalte.
- **Warten über die ganze Dauer in einem `sleep`:** die Monotonic-Uhr steht im Ruhezustand je nach Plattform still, das Fortsetzen käme um dessen Länge zu spät, und ein Abschalten bliebe bis zum Ablauf als schlafender Thread liegen.

### Obergrenzen

- Ein Neustart der App verliert Schalter und Zeitgeber; die Session ist danach `paused` ohne Begründung.
- Prüfung und Senden sind getrennt wie bei `auto_handoff::fire_back`: eine Nachricht des Users genau dazwischen ist akzeptiert, der Agent bekommt den Fortsetz-Text dann zusätzlich.
- Scheitert erst das Schreiben an den neu gestarteten Prozess, ist die Begründung schon gelöscht und der Status `running`, wie bei jeder Nachricht in diesem Fall; das Ende des Prozesses setzt `error`. Die Karte steht trotzdem im Verlauf.
- Eine zweite Kontingent-Antwort nach dem Fortsetzen beginnt wieder mit `auto_resume: false`: neue Karte, kein zweiter Zeitgeber, keine Schleife.

**Nachtrag 2026-11-03:** „Nachricht“ meint in diesem ADR die des Nutzers. Die Nachricht einer anderen Session ([ADR 052](052-session-werkzeuge.md)) setzt eine pausierte Session nicht fort und wartet bis zur nächsten vollständigen Antwort.
