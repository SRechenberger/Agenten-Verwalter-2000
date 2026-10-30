# FINDINGS

Erkenntnisse während der Umsetzung, je eine Zeile: `- [ ] → Phase N: <Erkenntnis>`. Erledigte Einträge werden abgehakt, nicht gelöscht.

- [x] → Phase 2: ADR 044 hat entschieden, dass der Cache nutzbar ist (gleiche Werkzeugliste); der feste Hinweis „liest den ganzen Verlauf neu“ entfällt, die Pille nimmt immer `promptCacheState`. `SideUsage.inputTokens` ist die Summe aller Eingabe-Token samt Cache, Anteil aus dem Cache = `cacheReadTokens / inputTokens`.
- [x] → Phase 2: `side_ask` lehnt eine Session ohne Transkript der Kommandozeile ab („Diese Session hat noch keinen Verlauf.“); der Einstieg kann das als Hinweis zeigen wie den Satz für Autark/Codex. `side_dismiss` meldet kein `side://changed`, nach dem Entfernen selbst neu laden.
- [x] → Phase 3: Der Einstieg läuft über `openSide(sessionId, operatingMode, quote?)` in `src/features/side/openSide.ts`; in Autark und Codex setzt er einen Hinweis in den Store (`notices`), den die Eingabeleiste in ihrer Fehlerzeile zeigt. Die Rechtsklick-Zeile ruft nur `openSide` auf, ohne eigene Prüfung. Das Fenster zeigt `quote` aus dem Store im Eingabezustand.
