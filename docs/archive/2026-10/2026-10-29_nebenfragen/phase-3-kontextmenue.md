# Phase 3 — Markieren → Rechtsklick (standard, Wave 3)

Ergebnis: Wer im Chat Text einer Antwort markiert und rechtsklickt, bekommt ein kleines Menü mit „Kopieren“, „Nebenfrage zur Auswahl …“ und „Als Zitat in die Nachricht“. Die zweite Zeile öffnet das Fenster aus Phase 2 mit dem Auswahltext als Zitat. Voraussetzung: Phase 2 committet (`useSideStore.open`, `openSide`). Entscheidungen und Grenzen: [README](README.md).

## Kontext (lesen)

- `src/features/chat/ChatTimeline.tsx` (virtualisierter Verlauf; `keydown`-Handler um Zeile 209–228; welches Element den Text trägt), `ChatTimeline.css`, `src/components/Markdown.tsx` (Textblöcke der Antwort), `src/features/chat/TextBlock.tsx` bzw. der Block, der Agententext rendert (per Glob in `src/features/chat/` finden), `src/features/chat/UserMessage.tsx`.
- `src/features/side/openSide.ts`, `src/stores/side.ts` (Phase 2), `src/stores/chat.ts` (`setDraft` je Session) und `src/features/chat/composerDraft.ts` (Entwurf setzen), `src/components/Popover.tsx` (Fokus, Esc, Klick daneben) als Muster für ein schwebendes Menü, `src/features/chat/CommandMenu.css` für Optik von Menüzeilen.
- [docs/conventions/react.md](../../conventions/react.md), [tailwind.md](../../conventions/tailwind.md), Entwurf [artifacts/entwurf-nebenfragen.html](artifacts/entwurf-nebenfragen.html) (Abschnitt F, „Einstieg 1“).

## Neue und geänderte Dateien

- `src/features/side/selectionText.ts` — reine Funktion `selectedChatText(root: HTMLElement): string | null`: liest `window.getSelection()`, liefert den getrimmten Text, wenn die Auswahl nicht leer ist und **beide** Enden (`anchorNode`, `focusNode`) in `root` liegen; sonst `null`. Kürzen übernimmt der Core (`clip_quote`), hier nicht.
- `src/features/side/SelectionMenu.tsx` + `SelectionMenu.css` — Menü an der Mausposition (`position: fixed`, am Fensterrand nach innen geschoben): drei Zeilen „Kopieren“ (`Strg C`), „Nebenfrage zur Auswahl …“ (`Strg ;`), „Als Zitat in die Nachricht“; Schließen bei `Esc`, Klick daneben, Scrollen und Fensterwechsel; Fokus auf der ersten Zeile, Pfeiltasten wechseln die Zeile, Enter wählt (Muster `CommandMenu`).
- `src/features/chat/ChatTimeline.tsx` — `onContextMenu` am Scroll-Container: `const text = selectedChatText(container)`; ist sie `null`, **nichts tun** (das Standardmenü bleibt unverändert); sonst `event.preventDefault()` und Menü mit Auswahltext und Mausposition öffnen. Zustand des Menüs lokal in der Komponente, nicht im Store.
- Zeilen im Menü: „Kopieren“ → `navigator.clipboard.writeText(text)`; „Nebenfrage zur Auswahl …“ → `openSide(session.id, mode, { quote: text })` (Betriebsart-Prüfung wie Phase 2; in Autark und Codex stattdessen `SIDE_UNSUPPORTED_HINT` in die Fehlerzeile); „Als Zitat in die Nachricht“ → Entwurf der Session um `> <Text, jede Zeile mit "> " eingerückt>\n\n` **am Anfang** ergänzen (bestehender Entwurf bleibt dahinter) und die Eingabeleiste fokussieren (Muster `composerDraft.ts`).
- Doku: `docs/code-map.md` Zeile „Nebenfragen“ um `SelectionMenu`, `selectedChatText`, Anschluss in `ChatTimeline` ergänzen; `docs/glossary.md` unverändert.

## AK

1. Text in einer Agenten-Antwort markieren, rechtsklicken → das Menü mit drei Zeilen erscheint an der Maus, nicht das Standardmenü.
2. „Nebenfrage zur Auswahl …“ öffnet das Fenster im Zustand Eingabe, der Streifen darüber zeigt den Auswahltext als Zitat; die gesendete Frage trägt das Zitat (sichtbar im Fenster und im Reiter nach dem Senden).
3. „Als Zitat in die Nachricht“ setzt `> …` an den Anfang des Entwurfs, der vorherige Entwurf bleibt erhalten, die Leiste hat den Fokus.
4. „Kopieren“ legt den markierten Text in die Zwischenablage.
5. Rechtsklick **ohne** Auswahl, auf das Eingabefeld oder auf einen Link verhält sich wie vorher (kein neues Menü).
6. Eine Auswahl, die über mehrere Zeilen bzw. Blöcke der Antwort reicht, wird vollständig übernommen; eine Auswahl außerhalb des Verlaufs (z. B. in der Sidebar) öffnet das Menü nicht.
7. `Esc`, Klick daneben und Scrollen schließen das Menü.
8. `pnpm check` grün.

## Checkliste

- [x] `selectionText.ts`, `SelectionMenu` anlegen; in `ChatTimeline` einhängen; die virtualisierte Liste darf die Auswahl nicht verlieren (nach `preventDefault` bleibt die Auswahl bestehen; Scrollen innerhalb des Menüs gibt es nicht).
- [x] CSS nach BEM mit Tokens (keine Rohfarben).
- [x] `pnpm check` grün.
- [ ] Selbstprüfung in der laufenden App: AK 1–7, je Punkt Screenshot oder Zeile im Report-Back.
- [x] Doku wie oben.

Abweichung: Die Zeile „Nebenfrage zur Auswahl …“ trägt kein `Strg ;`, weil `Strg+;` (`App.tsx`) das Fenster ohne Zitat öffnet — die Anzeige hätte etwas anderes versprochen.
- [ ] Ein Commit, Scope `side`. Danach Planabschluss: archivieren nach `docs/archive/2026-10/2026-10-29_nebenfragen/`, `STATE.md` nachführen, Versions-Tag (Minor-Version) nach `mode-committing`.

## Report-Back

Screenshots zu AK 1, 2, 3, 5, 6, `pnpm check`-Ergebniszeile, Commit-Hash.
