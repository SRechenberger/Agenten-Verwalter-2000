# Phase 2 — Oberfläche (standard, Wave 2)

Ergebnis: Fenster über der Eingabeleiste, Reiter „Nebenfragen“, die Einstiege `/btw`, `+`-Menü und `Strg+;`. Kontrakt, Entscheidungen, Grenzen: [README](README.md). Voraussetzung: Phase 1 committet, Bindings (`SideQuestion`, `SideState`, `SideUsage`) vorhanden, [ADR 044](../../decisions/044-nebenfragen.md) nennt die gewählte Variante und ob der Cache trägt.

## Kontext (lesen)

- `src/features/chat/Composer.tsx` (`runSessionCommand`, `pickRow`, `pickFromPlusMenu`, `handleKeyDown`, `setOpenMenu`, `setErrorMessage`, `LOCAL_MODEL_PICK_HINT`, das `+`-Menü mit `AttachMenu`, `CacheColdHint` oben in `composer__box`) und `Composer.css`.
- `src/features/chat/commandMenuRows.ts` (`SESSION_COMMANDS`, `SESSION_COMMAND_TEXT`, `sessionRows`, `CommandRow`) und `CommandMenu.tsx`, `useCommandMenu.ts`.
- `src/features/attachments/AttachMenu.tsx` und `AttachMenu.css` (das `+`-Menü: eine Zeile + Fuß).
- `src/features/background/BackgroundPanel.tsx` (Reiter-Liste, `TAB_LABEL`, `countFor`, `renderTab`), `ProcessesTab.tsx` und `SubagentsTab.tsx` (Muster einer Liste mit Auswahl), `PanelLayout`/`BackgroundRow`/`VirtualPanelList`, `src/stores/background.ts` (`BACKGROUND_TABS`, `BackgroundSelection`).
- `src/lib/tldr.ts` (Muster: Wrapper + Abo auf ein Ereignis), `src/features/tldr/useTldrView.ts` bzw. `useSessionTldr.ts` (Muster: Laden + Abo), `src/stores/tldr.ts`.
- `src/features/promptcache/` (`promptCacheState`: warm/kalt/neu; `CacheColdHint`), `src/components/Markdown.tsx`, `src/components/Popover.tsx` (Fokus, Esc), `src/lib/labels.ts` (`isLocalOperatingMode`), `src/stores/settings.ts` (Betriebsart steht in den Einstellungen, nicht an der Session), `src/app/App.tsx` um Zeile 118 (globaler `keydown` für `Strg+N`).
- [docs/conventions/react.md](../../conventions/react.md), [typescript.md](../../conventions/typescript.md), [tailwind.md](../../conventions/tailwind.md), Entwurf [artifacts/entwurf-nebenfragen.html](artifacts/entwurf-nebenfragen.html) (Abschnitt A und F: Aufbau, Beschriftungen, Farben — die Farbe „Neben“ ist `--color-cache-cold`).

## Neue und geänderte Dateien

- `src/lib/side.ts` — Wrapper nach Kontrakt (`askSide`, `loadSide`, `dismissSide`, `subscribeSide`), JSDoc mit `@throws` wie die übrigen Wrapper.
- `src/stores/side.ts` — Store nach Kontrakt (nur flüchtig).
- `src/features/side/useSideQuestions.ts` — Hook: lädt `loadSide(sessionId)`, abonniert `side://changed` (nur für diese Session), liefert `{ questions, error }`; ein Lauf `Running` hält ein Intervall von 2 s **nicht** nötig, das Ereignis genügt.
- `src/features/side/SideWindow.tsx` + `SideWindow.css` — das Fenster. Zustände: **Eingabe** (`questionId === null`): Titelzeile „Nebenfrage · kommt nicht in den Verlauf“, Zitat als Streifen (falls `quote`), Textfeld (Platzhalter „Frage an diese Session …“, fokussiert), Enter sendet (`Umschalt+Enter` Zeilenumbruch, nicht bei IME wie in `Composer.handleKeyDown`), nach dem Senden wechselt das Fenster mit der ID aus `askSide` in den Zustand Antwort; **läuft** (`state: Running`): Satz „Claude liest den Verlauf …“ mit demselben Lade-Symbol wie `WorkingIndicator`; **fertig**: Frage fett, Antwort über `Markdown`, darunter „Kopieren“ (Zwischenablage, wie `copy` an anderer Stelle im Projekt nachschlagen) und die Pille; **Fehler**: Fehlertext in `--color-status-error`, Knopf „Nochmal“ (stellt dieselbe Frage neu). `Esc` und ✕ schließen (`close`). Platz: über `composer__box`, `position: absolute; bottom: 100%` im `.composer`-Container, volle Breite der Leiste, `max-height: 50vh` mit Scrollen im Fenster; gerendert von `Composer.tsx` neben `CacheColdHint`.
- Pille (`SideCostPill`, in `SideWindow.tsx`): ist laut ADR 044 der Cache nutzbar, zeigt sie vor und während der Antwort den Zustand aus `promptCacheState(session…)` — „Cache warm · kostet kaum etwas“ bzw. „Cache kalt · volle Kosten“ (Tooltip erklärt: „Eine Nebenfrage liest den bisherigen Verlauf. Bei warmem Cache ist das billig.“) — und nach der Antwort zusätzlich `<n> Tokens aus dem Cache` aus `usage.cacheReadTokens`. Trägt der Cache laut ADR nicht, zeigt sie fest „liest den ganzen Verlauf neu“.
- `src/features/side/SideTab.tsx` + `SideTab.css` — Reiter-Inhalt: Kopf „Diese Sitzung“, je Nebenfrage eine Zeile (Frage fett, Uhrzeit, eine Zeile Antwortvorschau mit Ellipse; Zustand `Running` mit Lade-Symbol, `Failed` mit Fehlerfarbe), Klick öffnet das Fenster (`open(sessionId, { questionId })`) und wählt die Zeile (`select(sessionId, 'side', id)`), je Zeile ein Knopf „Entfernen“ (`dismissSide`). Leerzustand: „Noch keine Nebenfragen. Mit /btw oder Strg+; fragst du, ohne den Chat zu verlängern.“ Fuß: „Nur im Speicher: nach dem Beenden der App weg.“
- `src/stores/background.ts` — `BACKGROUND_TABS` um `'side'`, `BackgroundSelection` und `NO_SELECTION` um `side: string | null`.
- `src/features/background/BackgroundPanel.tsx` — `TAB_LABEL.side = 'Nebenfragen'`, `countFor('side')` = Zahl der Nebenfragen der Session (über `useSideQuestions`), `renderTab` Fall `'side'` → `<SideTab session={session} />`.
- `src/features/chat/commandMenuRows.ts` — `SESSION_COMMANDS` um `'btw'`; `SESSION_COMMAND_TEXT.btw = { label: '/btw', description: 'Nebenfrage stellen, ohne den Chat zu verlängern' }`; `sessionRows` fügt die Zeile **immer** ein (auch in Autark und Codex; dort sagt die Auswahl den Hinweis).
- `src/features/chat/Composer.tsx` — `runSessionCommand('btw')`: ist die Betriebsart Autark oder Codex (aus `useSettingsStore`) → `setErrorMessage(SIDE_UNSUPPORTED_HINT)`, sonst `useSideStore.getState().open(session.id)`; bei `/`-Menü wie die anderen Befehle den Entwurf leeren. `pickFromPlusMenu` bekommt eine Schwester `pickSideFromPlusMenu`. Neue Konstante `SIDE_UNSUPPORTED_HINT = 'Nebenfragen gibt es in dieser Betriebsart noch nicht.'` neben `LOCAL_MODEL_PICK_HINT`.
- `src/features/attachments/AttachMenu.tsx` — zweite Zeile `<button className="attach-menu__item">` „Nebenfrage stellen …“ mit `<kbd>Ctrl ;</kbd>`, Prop `onSide: () => void`, `title`: „Frage zur Session, die nicht im Chat landet. Auch mit /btw.“
- `src/app/App.tsx` — im bestehenden `keydown`-Handler (neben `Strg+N`): `event.ctrlKey && !event.altKey && event.key === ';'` → `preventDefault`, bei sichtbarer Chat-Session wie `runSessionCommand('btw')` (gleiche Betriebsart-Prüfung; dafür die Prüfung als Funktion `openSide(sessionId, mode)` in `src/features/side/openSide.ts` ablegen und von `Composer` und `App` benutzen). **`shiftKey` nicht ausschließen** (deutsche Tastatur, siehe README).
- Doku: `docs/code-map.md` Zeile „Nebenfragen“ (Oberfläche-Spalte füllen), Features-Liste unverändert; `docs/glossary.md` Eintrag **Nebenfrage** um „Fenster“ und „Reiter“ erweitern.

## AK

1. **Einstiege:** `/btw` im `/`-Menü, die Zeile „Nebenfrage stellen …“ im `+`-Menü und `Strg+;` öffnen dasselbe Fenster im Zustand Eingabe, Fokus im Textfeld. `Strg+;` funktioniert auf deutscher Tastatur.
2. **Erklärung:** Die Zeile im `+`-Menü, die Zeile im `/`-Menü und das Fenster haben jeweils einen Satz, was eine Nebenfrage ist (Titelzeile bzw. `title`); der Reiter hat einen Leerzustand mit Anleitung.
3. **Ablauf:** Frage senden → Zustand „läuft“ → Antwort als Markdown, Pille, „Kopieren“ funktioniert, `Esc` schließt. Die Frage taucht nicht im Chat-Verlauf auf, auch nicht nach dem Wechsel der Session und zurück.
4. **Reiter:** „Nebenfragen“ ist der vierte Reiter des Hintergrund-Panels mit Zähler wie „Prozesse 2“; die Zeile zeigt Frage, Uhrzeit und Antwortvorschau; Klick öffnet das Fenster mit dieser Antwort; „Entfernen“ löscht sie (Zähler sinkt).
5. **Fehler:** Scheitert der Aufruf (Prozess nicht startbar, Zeitlimit), zeigt das Fenster den Fehlertext und „Nochmal“; die Zeile im Reiter ist rot markiert.
6. **Betriebsarten:** In Autark und Codex setzt jeder Einstieg den Hinweis `SIDE_UNSUPPORTED_HINT` in die Fehlerzeile der Leiste und öffnet kein Fenster.
7. **Läuft nebenher:** Während ein Zug läuft (Zustand „arbeitet“), lässt sich eine Nebenfrage stellen und beantworten, ohne den Zug zu unterbrechen oder die Eingabeleiste zu sperren.
8. **Pille:** entspricht ADR 044 (siehe „Neue und geänderte Dateien“); bei kaltem Cache zeigt sie „volle Kosten“, bevor die Frage gesendet wird.
9. `pnpm check` grün (Lint, Typecheck, Format, Build).

## Checkliste

- [x] Wrapper, Store, Hook, `SideWindow`, `SideTab`, `openSide` anlegen; Store-Typ `BackgroundTab` um `side` erweitern und alle Stellen nachziehen, die `Record<BackgroundTab, …>` brauchen (`TAB_LABEL`, `NO_SELECTION`) — der Typecheck zeigt sie.
- [x] `Composer`, `commandMenuRows`, `AttachMenu`, `App` verdrahten.
- [x] CSS nach BEM mit Tokens aus `theme.css` (keine Rohfarben), Muster `Composer.css`, `BackgroundPanel.css`.
- [x] `pnpm check` grün.
- [ ] **Selbstprüfung in der laufenden App** (`pnpm tauri dev`, Prüfkette: [STATE.md](../../../STATE.md)): AK 1–8, je Punkt ein Screenshot oder eine Zeile im Report-Back; AK 7 mit einem langen Auftrag; AK 6 durch Umschalten der Betriebsart in den Einstellungen.
- [x] Doku: `docs/code-map.md`, `docs/glossary.md` wie oben.
- [ ] Ein Commit, Scope `side`.

## Report-Back

- Stand: umgesetzt, `pnpm check` grün (Lint, Typecheck, Format, Build, rustfmt, Clippy), committet als `bc68a37`.
- **Selbstprüfung in der laufenden App (AK 1–8) nicht gelaufen** — kein Weg, die Oberfläche aus der Session heraus zu bedienen; offen für den Betrieb.
- Abweichungen: `SIDE_UNSUPPORTED_HINT` liegt in `openSide.ts` statt in `Composer.tsx`. Der Hinweis für Autark/Codex läuft über `useSideStore.notices`, damit `Strg+;` (in `App`) und später der Rechtsklick ihn ohne Zugriff auf die Fehlerzeile der Leiste setzen können. Die Zeile im `+`-Menü erscheint nur in der Eingabeleiste (Prop `onSide` optional, „Neue Session“ hat keinen Verlauf).
- Pille: `promptCacheState` warm/kalt vor und während der Antwort, danach zusätzlich `<n>k Tokens aus dem Cache`; bei `fresh`/`unknown` zeigt sie vor dem Senden nichts.

Screenshots zu AK 1, 3, 4, 5, 6, 7, `pnpm check`-Ergebniszeile, Commit-Hash.
