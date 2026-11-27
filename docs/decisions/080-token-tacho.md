# 080 — Token-Tacho: Tokens je Antwort, Tempo und Reichweite

**Status:** umgesetzt (Kern; Taiga 271, Task 283; Migration 40) · **Datum:** 2026-11-26

## Kontext

Der Nutzer sieht den Verbrauch seines Kontingents nur als Prozent des Fensters, aber nicht, wie schnell er es verbraucht und ob es bis zur Rücksetzung reicht. Story 271 verlangt einen „Token-Tacho“: Tempo je Session und gesamt, dazu die Reichweite je Fenster. Dieser Kern speichert die Zahlen und rechnet; die Oberfläche baut die Lane `tacho-ui` gegen den Vertrag V-T unten.

## Entscheidung

- **Zwei Größen, Hauptzahl ohne Cache-Lesen:** Die Hauptzahl ist `input + cache_write + output`. Cache-Lesen wird getrennt geführt (`cacheReadPerMinute`), weil es die Zahl um Größenordnungen überlagerte und kaum Kontingent kostet.
- **Ablage (T1, T2):** Tabelle `token_usage`, eine Zeile je Antwort des Modells, eindeutig über (`session_id`, `message_id`). Eine Antwort kommt in mehreren Zeilen mit derselben `message.id`; die spätere ersetzt die Zahlen der früheren, die Uhrzeit der ersten bleibt. Ohne `message.id` zählt jede Zeile für sich. Fehlende Werte zählen 0. Die Zeilen fallen mit ihrer Session weg (`ON DELETE CASCADE`). Die Zahlen schreibt der Core über `Outbox` und `persist` wie die übrigen Zeilen einer Session.
- **Betriebsarten:** Claude, Codex und Autark münden in `handle_assistant` (`agents/claude/translate.rs`) und erzeugen dort `AgentEvent::TokensUsed`; `output_tokens` kommt neu aus `usage`. Codex setzt die letzte Zählung in jede `assistant`-Zeile bis zur nächsten (Denken, Werkzeug, Text, auch die ersten Zeilen des nächsten Zugs); damit sie einmal zählt, trägt jede neue Zählung eine eigene `message.id` `codex-<UUID>` (`LastCount` in `codex/tokens.rs`). Meldet Codex dieselbe Zählung noch einmal, bleibt die id. Eine UUID statt eines Zählers, weil die Zeilen einen Neustart des Übersetzers überdauern. Autark schreibt eine Zeile je Anfrage und bleibt ohne `message.id`. Auch die Antworten eines Subagenten zählen bei der Session. Eine Nebenfrage (ADR 044) läuft in einem eigenen Prozess; `registry/side.rs` schreibt ihre Zahlen mit der Kennung `side-<Frage>` bei ihrer Session.
- **Rechnen in reinen Funktionen (T3):** `tempo/mod.rs` (`minutes`, `rates`, `session_rates`, `snapshot`) und `usage/forecast.rs` nehmen „jetzt“ als Parameter und brauchen keine Datenbank. Das Tempo ist das Mittel der letzten 5 Minuten.
- **Prognose (T4):** Je Fenster hält der `UsageService` im Speicher die Paare (Zeit, Prozent) der letzten 5 Stunden, aus `get_usage` und aus jedem `rate_limit_event`. `percentPerHour` ist der Anstieg über das Fenster der Daten, die Prognose erscheint frühestens nach 10 Minuten Anstieg. `exhaustedAt` steht da, wenn 100 % vor `resets_at` erreicht würden; `percentAtReset` ist der Stand bei `resets_at` und nie kleiner als der jetzige. Fällt der Wert, beginnt der Verlauf neu. Nur bei Claude und Codex.
- **Ereignis gedrosselt (T5):** `tempo://changed` geht höchstens einmal je Sekunde und Session hinaus; ein Ereignis im selben Takt wartet bis zum Ende des Takts und deckt alles Spätere ab, die letzte Änderung geht nie verloren.
- **Fernsteuerung (ADR 059):** `tempo_load` ist in `remote/access.rs` Klasse `Reachable`; der Gast sieht die Werte des Hosts, wie beim Kontingent (`hostUsage`).
  - **G1:** Am Gast fragt `remote_tempo_load(hostId, scope)` (Klasse `LocalOnly`) den Host; der Host beantwortet `tempo_load` in `remote/dispatch.rs` (`tempo_for`) mit `tempo::load`.
  - **G2:** Bei `all` bekommt der Gast die Werte des ganzen Hosts (`minutes`, `perMinute`, `cacheReadPerMinute`, `active`); `top` enthält nur Sessions der Vorhaben, die das Gerät hält. Eine `session` eines anderen Vorhabens lehnt der Host ab (`ForeignId`).
  - **G3:** `tempo://changed` geht in `remote/forward.rs` als `Owner::Session` nur an das Gerät, das das Vorhaben der Session hält, und kommt am Gast als `remote://tempo-changed` mit `{ hostId, sessionId }` an.
  - **G4:** `src/lib/tempo.ts` hat `TempoSource` mit `OWN_TEMPO` und `hostTempo(hostId)` wie `UsageSource`.

## Vertrag V-T

- Tabelle `token_usage` wie oben.
- Befehl `tempo_load(scope: TempoScope) -> TempoSnapshot`, `TempoScope = { kind: 'session', sessionId } | { kind: 'all' }`.
- `TempoSnapshot { minutes: TempoMinute[60], perMinute, cacheReadPerMinute, total: TempoTotals | null, top: TempoSessionRate[], active }`; `TempoMinute { atMs, input, cacheWrite, cacheRead, output }`; `total` nur bei einer Session, `top` (höchstens 5, absteigend) und `active` nur bei `all`.
- Ereignis `tempo://changed` mit `{ sessionId }`.
- `UsageStatus.forecast: UsageForecast[]`, `UsageForecast { kind, percentPerHour, exhaustedAt, percentAtReset }`.

## Folgen

- Migration 40 legt `token_usage` mit zwei Indizes an (`session_id, at_ms` und `at_ms` für den Stand „alle“).
- Der Verlauf der Prozentwerte lebt nur im Speicher; nach einem Neustart beginnt die Prognose neu.

## Nachtrag: Oberfläche (Taiga 271, Lane `tacho-ui`)

Die Oberfläche liegt in `src/features/tempo/`, ohne neue Migration und ohne neuen Befehl; sie folgt dem Prototyp `docs/design/2026-11-26_kontingent-und-tacho/mockup.html`.

- **U1 Quelle:** Jede Anzeige holt ihr Tempo über `TempoSource` (`OWN_TEMPO`, `hostTempo(hostId)`), die Quelle einer Session aus dem Vorhaben (`useTempoSource`). Beim Gast zeigt die Zeile „Tempo“ wie die Zeilen unter „Kontingent“ eine Zeile je Host.
- **U2 Zahlen:** Das Tempo steht als „18 k/min“, unter 10 k mit einer Nachkommastelle („1,5 k/min“, „0,4 k/min“), ab 1 000 k als „1,2 M/min“, in Ruhe als „0 k/min“. Die Reichweite steht unter 10 mit einer Nachkommastelle („1,2 %/h“), sonst ganz („9 %/h“). Die Formate und die Wahl des knappsten Fensters stehen in `tempoTexts.ts`, geprüft von `tempoTexts.selftest.ts`.
- **U3 Linie:** 60 Punkte aus `minutes` (Summe ohne Cache-Lesen), ohne Achse, im Grau des Textes; 56 × 14 px im Kopf und an Stationen, 44 × 14 px in der Zeile „Tempo“.
- **U4 Kopf:** Ein eigener Knopf neben dem Knopf für Kontext, Cache und Laufzeit, immer zu sehen, bei Ruhe mit „0 k/min“. Er öffnet das Fenster „Tempo“ der Session, getrennt vom Fenster für Kontext und Cache.
- **U5 Stationen:** Linie und Zahl stehen in der Zeile der Zeitleiste und im Kopf der Stationskarte; ein Klick öffnet das Fenster der Session der Station. Eine Station ohne Session zeigt „—“. Die Leiste der Sessions bleibt ohne Tempo.
- **U6 Zeile „Kontingent“:** Hinter der Zahl steht die Reichweite des knappsten Fensters: das mit dem frühesten `exhaustedAt`, sonst das mit dem höchsten `percentPerHour`. Ohne `forecast` steht dort nichts; der Tooltip sagt dann „Noch keine Prognose“.
- **U7 Zeile „Tempo“:** Unter „Kontingent“ mit Symbol, Linie, Summe und „· n“ (`active`), immer zu sehen. Ein Klick öffnet das Fenster „Tempo“ gesamt.
- **U8 Fenster:** Hauptzahl, gestapelter Verlauf je Minute (Ausgabe, Eingabe neu, Cache geschrieben von unten nach oben, 1 px Abstand, Legende immer da, Tooltip je Minute), „Cache gelesen jetzt“ und bei einer Session die Summen seit dem Start. Gesamt kommen die Reichweite je Fenster (Balken mit gestrichelter Verlängerung bis `percentAtReset`, rot über 100 %) und die teuersten Sessions hinzu; ein Klick öffnet die Session.
- **U9 Laden:** Nur eine eingehängte Anzeige lädt und abonniert. Ein Ereignis lädt nur die Anzeige der betroffenen Session neu, höchstens einmal je Sekunde; dazu lädt jede sichtbare Anzeige einmal je Minute, weil das 5-Minuten-Mittel auch ohne neue Antwort sinkt und die Linie weiterrückt (`useTempo`).
