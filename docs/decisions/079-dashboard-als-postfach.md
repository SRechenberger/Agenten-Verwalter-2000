# 079 — Dashboard als Postfach: Ordner, Liste, Lesebereich

**Status:** umgesetzt (Taiga 187, Task 243; Migration 38) · **Datum:** 2026-11-25

## Kontext

Das Dashboard ([ADR 071](071-dashboard.md)) zeigte die Karten als Kacheln, geschachtelt über die Einstellung „Gruppiert nach“ (`DashboardChoice`, Taiga 186), mit einem Popup je Karte und den Abläufen in einer rechten Spalte. Bei vielen Karten wurde das unübersichtlich: Ein Klick öffnete ein Popup, ohne dass die Liste sagte, was schon gesehen war, der Inhalt einer Karte ließ sich nicht neben der Liste lesen, und die Karte einer Session aus „Gelesen“ konnte man nicht zurückholen. Der Nutzer wählte am 2026-11-24 Variante A: ein Postfach wie ein Mail-Programm. Prototyp: `docs/design/2026-11-25_dashboard-postfach/mockup.html` (Runde 2; Runde 1 als `runde-1.html`), Entscheidungen im dortigen `README.md`.

## Entscheidung

- **P1, Postfach:** Drei Spalten. Links die **Ordner**, in der Mitte die Liste, rechts der **Lesebereich**. Die Kacheln entfallen ohne Schalter zurück. Die Liste hat Stufenköpfe, die Reihenfolgen „Wichtigste zuerst“ (die des Kerns, [ADR 071](071-dashboard.md) und [ADR 077](077-karten-per-werkzeug.md)) und „Neueste zuerst“ (die Stufen bleiben), „Gleiche zusammenfassen“ (Standard an: gleiche Art und gleicher Titel in einer Zeile), Kästchen mit Sammelaktionen und eine Tastatur. Reine Logik: `src/features/dashboard/postfach.ts`.
- **P2, Gruppen statt „Gruppiert nach“:** `DashboardChoice` und ihre Einstellung (Schlüssel `dashboard` in `settings`) entfallen; ein alter gespeicherter Wert wird ignoriert. Die Gruppen der Leiste ordnen die Vorhaben im Rand, aufklappbar (der Zustand ist flüchtig). „Nur, was einen Agenten anhält“ ist der Ordner „Hält Agenten an“ (blockierende und pausierte Karten, wie der alte Filter).
- **P3, Abläufe:** Jeder Ablauf ist ein Ordner und nennt seinen Master (`◆ #41 Architekt`, aus `masterOf`; ohne Master „Master ausgefallen“). Mehrere Abläufe eines Vorhabens stehen nebeneinander ([ADR 076](076-umsetzungen-je-session.md)). Die Sessions ohne Ablauf stehen darunter im Rand.
- **P4, Lesebereich statt Popup:** Im Dashboard zeigt der Lesebereich die gewählte Karte mit denselben Bausteinen wie das Popup (`PopupBodies.tsx` über `TaskContent.tsx`). Die Karten im Chat behalten ihr `TaskPopup`. Das Popup bleibt nach „Senden“ und „Ja, mach das“ offen; im Postfach verlässt der Eintrag die Liste, und eine Meldung unten sagt „Gesendet: …“.
- **P5, „angesehen“ je Karte im Kern:** Tabelle `dashboard_seen(card_id PRIMARY KEY, seen_at)` (Migration 38). Ungelesen heißt `seen_at < since` (`dashboard::is_unread`, in der Oberfläche `isUnread`); eine Karte ohne Zeile ist ungelesen, und eine neue Art mit derselben Kennung (`session:<id>`) ist es wieder, weil `since` später liegt. `DashboardTask.seen_at` (`#[serde(default)]`) trägt den Wert: `build` setzt ihn aus der Momentaufnahme, `dashboard_snapshot` liest die Tabelle vor den Session-Sperren. Weil der Gast die Karte mit `seenAt` vom Host bekommt (`dashboard_for` baut aus derselben Momentaufnahme), ist „ungelesen“ bei Host und Gast gleich. `dashboard_get` räumt die Zeilen verschwundener Karten ab (nur für die eigene Ansicht, vor den Karten der Hosts); ein Fehler dabei lässt es nicht scheitern. Neuer Befehl `dashboard_seen(projectId, cardIds)` (Klasse C mit dem Vorhaben; `projectId` führt nur die Fernsteuerung zum Host, gespeichert werden die Karten-IDs). Wählen und Aufklappen setzen „angesehen“, in der Ansicht sofort (`markSeen` im Store) und danach im Kern.
- **Rückgängig nach „Gelesen“:** Neuer Befehl `dashboard_undismiss(sessionId)` (Klasse C mit der Session). `dismiss_card` merkt im Speicher der Session den Vorwert von `card_dismissed_at` (`card_dismissed_prev`); `undismiss_card` stellt ihn wieder her, ohne Vorwert oder ohne gemerkten Vorwert (nach einem Neustart) auf `NULL`. Bei zweimal „Gelesen“ und einmal Rückgängig gilt der Wert vor dem zweiten.
- **P6, Sammelaktionen:** „Gelesen“, „Fortsetzen“ und „Starten“ rufen die vorhandenen Befehle (`dashboard_dismiss`, `session_resume`, `implementation_start_station`) der Reihe nach auf; es gibt keinen Sammelbefehl. Jeder Knopf nennt die Zahl der Karten, die er trifft, und ruht bei 0.
- **P7, Aufklappen:** ▸/▾ vorn in jeder Zeile; `o`, → und ← schalten, dazu „Alle aufklappen“ und „Alle zuklappen“. Mehrere Zeilen dürfen zugleich offen sein. Eine aufgeklappte Zeile rendert dieselbe Komponente (`TaskPanel`) wie der Lesebereich, ohne Kopf; der Entwurf einer Antwort liegt je Karte im Store (`drafts`) und ist an beiden Stellen und im Popup derselbe. Nur der Lesebereich beantwortet die Ziffern einer Rückfrage (`PopupProps.keys`), sonst sendete eine Ziffer doppelt. Welche Zeilen offen sind, ist flüchtiger Zustand im Store (Schlüssel = Karten-ID); verschwindet eine Karte, fällt sie aus der Menge, aus der Auswahl und aus den Entwürfen.
- **Flach (Story 236):** keine runden Kästen, die Farbe der Art als Linie links, Zeilen mit Haarlinien, eckige Felder, Knöpfe und Marken mit 2 px.

## Texte

Ohne Aufruf von `verwalter_notify_user` steht im Lesebereich weiter der Satz der App, „Nicht gemeldet: Der Agent hat das Werkzeug verwalter_notify_user nicht gerufen.“ (nicht der des Prototyps). „Blockiert“ trägt den Untertitel „der Agent kommt ohne dich nicht weiter“, eine fertige Karte mit Angebot in der Liste den Zusatz „· Angebot“.

## Tastatur

j/k und die Pfeile wählen, `x` markiert, `e` liest, `r` und Enter springen ins Antwortfeld der aufgeklappten Zeile (sonst des Lesebereichs; bei einer zusammengefassten Zeile schalten sie deren Einträge), `o`, → und ← klappen, 1–9 wählen die Option einer Rückfrage (`useQuestionKeys`), `/` sucht, Strg+Enter sendet, Esc verlässt ein Feld. Die Tasten ruhen, solange ein Feld, ein Knopf (für Enter und Leertaste) oder ein Dialog den Fokus hat.

## Folgen

- Entfallen: `DashboardChoice`, `SettingsChange::Dashboard`, `DashboardFlows.tsx`, `dashboardNest.selftest.ts` und in `dashboardTexts.ts` die Gruppierung (`groupTasks`, `nestTasks`, `levelsOf` …) und `filterTasks`; die Fälle der Marke „entfernt“ gingen in `postfach.selftest.ts`.
- Ein Rennen bleibt: Markiert `dashboard_seen` eine Karte, die erst nach dem Bau der Ansicht entstanden ist, räumt ein gleichzeitiges `dashboard_get` die Zeile ab; die Karte gilt dann wieder als ungelesen. Nach „Rückgängig“ ist eine zurückgeholte Karte ebenfalls ungelesen, weil „Gelesen“ sie aus der Ansicht und damit aus der Tabelle nahm.
- **Migration:** Der Plan sah 38 vor (037 gehört der Lane `gruppen`). Die Nummer ist die Position in `MIGRATIONS`; gemergt in der Reihenfolge der Lanes trägt die Datei die nächste freie Nummer, die PR-Station benennt sie bei Bedarf um.

## Tests

`dashboard::tests::` (`seen_at` je Karte, ungelesen, neue Art, älterer Host, `mark_seen`, `prune_seen`, der Weg von `dashboard_get`, Migration 37), `sessions::registry::tests::` (Rückgängig), `remote::` (gleiches `seenAt` bei Gast und Host, Gast markiert nur gehaltene Vorhaben, Klassen), `settings::` (alter Schlüssel ignoriert), Selbsttests `postfach.selftest.ts` und `dashboardTexts.selftest.ts`.
