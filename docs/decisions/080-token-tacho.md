# 080 — Token-Tacho: Tokens je Antwort, Tempo und Reichweite

**Status:** umgesetzt (Kern; Taiga 271, Task 283; Migration 40) · **Datum:** 2026-11-26

## Kontext

Der Nutzer sieht den Verbrauch seines Kontingents nur als Prozent des Fensters, aber nicht, wie schnell er es verbraucht und ob es bis zur Rücksetzung reicht. Story 271 verlangt einen „Token-Tacho“: Tempo je Session und gesamt, dazu die Reichweite je Fenster. Dieser Kern speichert die Zahlen und rechnet; die Oberfläche baut die Lane `tacho-ui` gegen den Vertrag V-T unten.

## Entscheidung

- **Zwei Größen, Hauptzahl ohne Cache-Lesen:** Die Hauptzahl ist `input + cache_write + output`. Cache-Lesen wird getrennt geführt (`cacheReadPerMinute`), weil es die Zahl um Größenordnungen überlagerte und kaum Kontingent kostet.
- **Ablage (T1, T2):** Tabelle `token_usage`, eine Zeile je Antwort des Modells, eindeutig über (`session_id`, `message_id`). Eine Antwort kommt in mehreren Zeilen mit derselben `message.id`; die spätere ersetzt die Zahlen der früheren, die Uhrzeit der ersten bleibt. Ohne `message.id` zählt jede Zeile für sich. Fehlende Werte zählen 0. Die Zeilen fallen mit ihrer Session weg (`ON DELETE CASCADE`). Die Zahlen schreibt der Core über `Outbox` und `persist` wie die übrigen Zeilen einer Session.
- **Betriebsarten:** Claude, Codex und Autark münden in `handle_assistant` (`agents/claude/translate.rs`) und erzeugen dort `AgentEvent::TokensUsed`; `output_tokens` kommt neu aus `usage`. Auch die Antworten eines Subagenten zählen bei der Session. Eine Nebenfrage (ADR 044) läuft in einem eigenen Prozess; `registry/side.rs` schreibt ihre Zahlen mit der Kennung `side-<Frage>` bei ihrer Session.
- **Rechnen in reinen Funktionen (T3):** `tempo/mod.rs` (`minutes`, `rates`, `session_rates`, `snapshot`) und `usage/forecast.rs` nehmen „jetzt“ als Parameter und brauchen keine Datenbank. Das Tempo ist das Mittel der letzten 5 Minuten.
- **Prognose (T4):** Je Fenster hält der `UsageService` im Speicher die Paare (Zeit, Prozent) der letzten 5 Stunden, aus `get_usage` und aus jedem `rate_limit_event`. `percentPerHour` ist der Anstieg über das Fenster der Daten, die Prognose erscheint frühestens nach 10 Minuten Anstieg. `exhaustedAt` steht da, wenn 100 % vor `resets_at` erreicht würden; `percentAtReset` ist der Stand bei `resets_at` und nie kleiner als der jetzige. Fällt der Wert, beginnt der Verlauf neu. Nur bei Claude und Codex.
- **Ereignis gedrosselt (T5):** `tempo://changed` geht höchstens einmal je Sekunde und Session hinaus; ein Ereignis im selben Takt wartet bis zum Ende des Takts und deckt alles Spätere ab, die letzte Änderung geht nie verloren.
- **Fernsteuerung (ADR 059):** `tempo_load` ist in `remote/access.rs` Klasse `Reachable`: Ein Gast sieht die Werte des Hosts. Der Weg über `remote_tempo_load` und die Weiterleitung von `tempo://changed` an den Gast gehört zur Oberfläche (`tacho-ui`).

## Vertrag V-T

- Tabelle `token_usage` wie oben.
- Befehl `tempo_load(scope: TempoScope) -> TempoSnapshot`, `TempoScope = { kind: 'session', sessionId } | { kind: 'all' }`.
- `TempoSnapshot { minutes: TempoMinute[60], perMinute, cacheReadPerMinute, total: TempoTotals | null, top: TempoSessionRate[], active }`; `TempoMinute { atMs, input, cacheWrite, cacheRead, output }`; `total` nur bei einer Session, `top` (höchstens 5, absteigend) und `active` nur bei `all`.
- Ereignis `tempo://changed` mit `{ sessionId }`.
- `UsageStatus.forecast: UsageForecast[]`, `UsageForecast { kind, percentPerHour, exhaustedAt, percentAtReset }`.

## Folgen

- Migration 40 legt `token_usage` mit zwei Indizes an (`session_id, at_ms` und `at_ms` für den Stand „alle“).
- Der Verlauf der Prozentwerte lebt nur im Speicher; nach einem Neustart beginnt die Prognose neu.
