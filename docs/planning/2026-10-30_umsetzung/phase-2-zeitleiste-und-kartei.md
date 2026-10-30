# Phase 2 — Ansicht: Zeitleiste mit gekoppelter Kartei

Rating **standard** · Wave 2 · Aufwand L · Status: pending

## Kontext

- [README.md](README.md): Entscheidungen „Sidebar“, „Ansicht“, Kontrakt „Oberfläche“; Typen `ImplementationView`, `StationState`.
- Entwurf [artifacts/entwurf-umsetzung.html](artifacts/entwurf-umsetzung.html), Abschnitte **1** (Zeitleiste + Kartei, klickbar), **6** (Punkte und Linie), **7** (Regeln); Abschnitt 2 (Wellen) und 3 (Plan geändert) sind Phase 4. Die Maße und Farben stehen im `app.css` daneben (`artifacts/entwurf-umsetzung/app.css`).
- Phase-1-Dateien: `src/features/implementation/*`, `src/stores/implementation.ts`, `src/lib/implementation.ts`.
- Muster: `src/features/chat/ChatTimeline.tsx` (Tasten-Handling, `isTextTarget`), `src/features/projects/ProjectSessionCard.tsx` (Karte je Session), `src/features/chat/HandoffBar.tsx` + `useSecondsUntil.ts` (Countdown-Anzeige; der Countdown selbst kommt in Phase 3, die Karte zeigt ihn ab dort), `src/components/StatusIcon.tsx`, `src/styles/theme.css` (Farb-Tokens `--color-success`, `--color-waiting`, `--color-running`, `--color-muted` — die tatsächlichen Namen in der Datei nachlesen), `src/features/changes/ChangesView.tsx` (Reiter „Changes“).
- [docs/conventions/react.md](../../conventions/react.md), [tailwind.md](../../conventions/tailwind.md) (BEM je Komponente), `mode-web-frontend` → „Idiotensicherheit“ (Erklärung an jeder Aktion als `title`).
- Fehlerklassen (Vault): keine einschlägig.

## Abnahmekriterien (prüfbare Struktur nach Entwurf)

1. **Zeitleiste** (`Timeline`, links, `role="list"`): je Phase eine Überschrift (`<key> <Titel>` + Zustand „fertig · 42 min“ / „läuft“ / „fragt dich“ / „geplant“ / „Fehler“), darunter je Station eine Zeile mit Punkt (`StationDot`, alle gleich groß: 12 px), Stufe, `#<n>` und Dauer (`running_ms` als „8 min“) bzw. „läuft“/„fragt“. Mechanische Phasen (nur eine Station) zeigen die Station an der Phase. Punkte: `done` gefüllt grün mit Haken, `running`/`starting` gefüllt blau, `asking`/`waitsForYou` gelb mit „?“, `error` rot mit „!“, `pending`/`ready`/`countdown` grau leer. Die Linie ist ein durchgehender Strich: grün bis zur letzten fertigen Station, dann in der Farbe der aktuellen Station bis zu ihr, danach grau (`Timeline.css`, Segmente je Station).
2. **Kartei** (`CardStack`, rechts, `aria-live="polite"`): große Karte = Station im Fokus; darüber bis zu drei vorige, darunter bis zu drei kommende als kleine Karten (eine Zeile: Phase · Stufe · Zustand). Klick auf Station oder kleine Karte → Fokus; `ArrowUp`/`ArrowDown` blättern in Plan-Reihenfolge (nicht in Textfeldern, Muster `isTextTarget`); die Station im Fokus ist links hervorgehoben (`timeline__station--focused`).
3. **Fokus-Automatik:** Ohne gesetzten Fokus (Store) liegt vorn die Station mit dem dringendsten Zustand (Reihenfolge `urgency`), bei Gleichstand die erste in Plan-Reihenfolge; ist alles fertig, die letzte. Knopf „Zur aktuellen“ (nur sichtbar, wenn ein Fokus gesetzt ist und von der Automatik abweicht) löscht den Fokus.
4. **Große Karte** (`StationCard`): Kopf „Phase <key> · <Stufe>“ + Zustandstext („läuft seit 12 min“, „fragt seit 2 min“, „fertig · 24 min“, „wartet auf dich“, „wartet auf Start“, „Fehler“), `#<n>`; bei `asking` der Fragetext (`question`) in einem Kasten mit Knopf „Chat öffnen“ (Antworten im Chat, nicht auf der Karte — Abweichung vom Entwurf, siehe unten); bei `done` die Ergebniszeile; bei `by_user` und Zustand `waitsForYou` Knopf „Erledigt“ (Phase 3 verdrahtet); Knöpfe „Chat öffnen“ (`selectSession`) und „Changes · <n> Dateien“ (wechselt in den Reiter Changes); ohne Session kein „Chat öffnen“. Jede Karte trägt `title`-Erklärungen an den Knöpfen.
5. **Kopfzeile** (`ImplementationHeader`): Pfad, Reiter, Zeile „Phase <k> von <n> · <fertig> von <gesamt> Sessions fertig“, Schalter „Automatisch weiter“ (`Switch`, `title` „Startet die nächste Station von selbst, 10 Sekunden nach der Fertigmeldung“) und Knopf „Anhalten nach dieser Session“ (`title` „Die laufende Station läuft zu Ende, danach startet nichts mehr“) — beide in Phase 2 nur angezeigt, in Phase 3 verdrahtet (bis dahin `disabled`).
6. Doppelklick auf eine Station öffnet den Chat; Kartei und Zeitleiste scrollen unabhängig; unter 900 px Breite der Hauptfläche (Container-Query wie in `SessionHeader.css`) steht die Kartei unter der Zeitleiste.
7. Reiter „Changes“ zeigt die Changes der Reichweite Vorhaben (wie die Übersicht); der Reiter trägt die Dateizahl.

Abweichung vom Entwurf, bewusst: Fragen werden **im Chat** beantwortet (Knopf auf der Karte), nicht auf der Karte selbst — die Rückfrage-Bausteine (`questionDraft`, Notizen je Option, Rechte-Abfragen) leben in `ChatTimeline`, ein zweiter Antwortweg wäre eine Dopplung. Falls Sascha die Antwort auf der Karte will: Follow-up.

## Checkliste

- [ ] `implementationOrder.ts`: `flatStations(view): StationRef[]` (Plan-Reihenfolge mit Phase), `autoFocus(view): string | null`, `neighbors(view, focusedId, count = 3)`; reine Funktionen, ohne React.
- [ ] `Timeline.tsx` + `Timeline.css` (BEM `timeline__phase`, `timeline__station`, `timeline__dot`, `timeline__line`), `StationDot.tsx` (Zustand → Klasse + Zeichen, `role="img"`, `aria-label` = Zustandstext).
- [ ] `CardStack.tsx` + `.css` (`card-stack__previous`, `card-stack__current`, `card-stack__next`), `StationCard.tsx` + `.css` (`station-card__*`), Texte in `implementationTexts.ts` (`stateText(station, now)`, `durationText`).
- [ ] `ImplementationView.tsx`: Layout zwei Spalten (`implementation-view__timeline`, `implementation-view__cards`), Tasten-Handler, Reiter-Weiche (Ablauf/Changes); `ReportPanel` aus Phase 1 bleibt als Panel hinter dem Knopf „Lagebericht“ in der Kopfzeile (Phase 5 füllt den Knopf; hier den Knopf anlegen, `disabled`).
- [ ] `ImplementationHeader.tsx`: Schalter und Knopf nach AK 5 (noch `disabled`), Dateizahl am Reiter Changes aus `useSessionChanges` mit Reichweite Vorhaben (wie `App.tsx` es für die Übersicht macht).
- [ ] `useNow` (aus `features/background/useNow`) für laufende Dauern, nur wenn eine Station läuft.
- [ ] Doc: noch nichts (Phase 6).
- [ ] `pnpm check` grün, Commit `feat(implementation): Zeitleiste und Kartei der Umsetzung`.

## Definition of Done

- AK 1–7 erfüllt; **Reviewable Artifact:** Screenshot der Ansicht mit einer Umsetzung aus dem Prüfprogramm (`artifacts/phase-2-ansicht.png`), geprüft gegen Entwurf Abschnitt 1 (Spalten, Punkte, Linie, Kartei mit vorigen/kommenden Karten); **User-visible outcome:** Klick auf die Sidebar-Zeile zeigt Zeitleiste und Kartei, Pfeiltasten blättern.
- Commands run: `pnpm check`.
- Seven-Goals-Check (je ein Satz).

## Report-Back
