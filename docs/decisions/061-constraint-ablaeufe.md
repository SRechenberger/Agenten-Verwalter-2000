# 061 — Constraint-Abläufe: Planung aus Kanten statt aus Position und Welle

**Status:** angenommen; der Kern ist umgesetzt, Werkzeuge, Oberfläche und Konfiguration je Station folgen · **Datum:** 2026-11-11

## Kontext

Eine Umsetzung ([ADR 048](048-umsetzung.md)) ordnet ihre Stationen heute aus Listenposition und Wellennummer: Stationen einer Phase nacheinander, Phasen derselben Welle gleichzeitig, die nächste Gruppe erst nach allen davor (`schedule::groups`, `is_due`). Einfügen, Verschieben und Parallelisieren heißt, die Liste neu zu schreiben, und die Fix-Runde ist ein fest verdrahteter Zweig.

Die Story (Taiga #76) verlangt, dass ein Plan nur Stationen und ihre Bedingungen enthält und der Verwalter den Ablauf daraus berechnet: nach jeder Fertigmeldung, jeder Plan-Änderung und jeder Bearbeitung. Erste Ausbaustufe sind `after` (harte Abhängigkeit), `priority` und `max_parallel` je Umsetzung samt Warteschlange. Diese ADR hält die Entscheidungen des Kerns fest; die Werkzeuge, die Oberfläche (Entwurf [`docs/design/2026-11-10_constraint-ablaeufe/`](../design/2026-11-10_constraint-ablaeufe/README.md)) und die Konfiguration je Station bauen auf den Namen hier auf.

## Entscheidungen

### Modell

- **Kanten gespeichert, Nachfolger abgeleitet.** Jede Station trägt ihre Plan-Kennung (`plan_id`) und ihre Vorgänger (`after`, eine Liste von Plan-Kennungen). Der Nachfolger (`next`) ist die Umkehrung von `after` und wird nie gespeichert: ein gespeichertes `next` hätte zwei Wahrheiten und bräche beim Einfügen.
- **Eingabe** (`StationInput`, alle neu mit `serde(default)`, damit gespeicherte Vorschläge lesbar bleiben): `id` (Plan-Kennung, getrimmt 1 bis 40 Zeichen, im Plan eindeutig; fehlt sie, gilt `<Phasenschlüssel>-<Nummer>`, die Nummer 1-basiert in der Phase), `after` (Plan-Kennungen; Verweise dürfen auf jede Station der Umsetzung zielen, auch gestartete und fertige; `[]` heißt Startstation), `priority` (höher zuerst, Standard 0, nur Tie-Break unter bereiten Stationen). Am Plan `max_parallel` (mindestens 1, leer = unbegrenzt); `implementation_create` nimmt es an, das Schema nennt es mit den Werkzeugen.
- **Speicher** (Migration 25): `implementation_stations.plan_id`, `after_ids` (JSON-Liste), `priority`; `implementations.max_parallel`.
- **Ansicht:** `StationView` trägt `plan_id`, `after` (Plan-Kennungen wie gespeichert), `next` (als `id` der Stationen der Ansicht, damit die Oberfläche sie nachschlagen kann), `priority`, `level` und `queue_position`; `ImplementationView` trägt `max_parallel` und `queued` (Zahl der Stationen in der Warteschlange).

### Regel R1: altes Schema → Kanten

- Nennt irgendeine Station des Plans `after`, gilt der Plan als neues Schema, und Stationen ohne Angabe sind Startstationen. Nennt keine `after`, gelten Position und Welle: Stationen einer Phase in Reihe; die erste Station jeder Phase einer Gruppe hängt an der letzten Station jeder Phase der Gruppe davor; aufeinanderfolgende Phasen derselben `wave` bilden eine Gruppe, eine Phase ohne `wave` ist eine eigene.
- Die Ableitung steht an einer Stelle als reine Funktion (`schedule::legacy_after`) und gilt für `implementation_create`, für Plan-Änderungen (dann hinter den bleibenden, begonnenen Phasen) und für die Migration der Bestandsdaten (`db::implementations::backfill_edges`, in der Transaktion von Migration 25). Bestehende Umsetzungen laufen danach unverändert weiter; ein Test vergleicht an jedem Zwischenstand aller Reihenfolgen, welche Stationen an der Reihe sind, mit der alten Regel.
- Die Werkzeug- und Codefelder heißen weiter `phases`, `key`, `wave`; Wellen sind nur noch Eingabe des alten Schemas, keine Quelle der Planung.

### Planung

- **Bereit:** nicht fertig gemeldet, und alle Vorgänger sind fertig gemeldet. Ein Vorgänger, den es nicht gibt, ist nie fertig. Reihenfolge unter bereiten Stationen: `priority` absteigend, dann Planreihenfolge (Phasen, dann Stationen nach Position).
- **Stufe errechnet:** die längste Kette bis zur Station; Wurzeln haben Stufe 1; innerhalb einer Stufe gilt die Planreihenfolge.
- **Prüfung** beim Anlegen und beim Vorschlagen und Übernehmen einer Plan-Änderung über den ganzen Plan (bleibende und neue Stationen; gestartete und fertige Stationen und ihre Kanten ändern sich nie). Ein Satz auf Deutsch, der die Stationen nennt: „Zyklus: a → b → a“ (in Ablaufrichtung), „Unbekannte Kennung: Station „a“ nennt den Vorgänger „x“, den es im Plan nicht gibt.“, „Selbstverweis: Station „a“ nennt sich selbst als Vorgänger.“, „Doppelte Kennung: „a“ steht an mehr als einer Station.“
- **Erste Nachricht:** „Vorgänger:“ nennt nur die Ergebnisse der Vorgänger-Kanten, nicht mehr aller früheren Stationen.
- **Fix-Runde auf Kanten:** Fix-Runde nach dem meldenden Review, zweites Review nach der Fix-Runde, beide mit dem Vorrang des Reviews und Kennungen `<Phasenschlüssel>-<n>` mit dem nächsten freien `n`; wer das Review in `after` führt, wartet danach auf das zweite Review. Die Auswahl (höchstens zwei Fix-Runden, Rückfall auf Modell und Skill) bleibt wie in [ADR 060](060-agententypen.md).
- **Auslöser bleibt Variante B:** Der Agent meldet fertig, der Verwalter plant den bereiten Satz und startet nach dem Countdown. Der Agent schickt dem Nachfolger nichts.

### Warteschlange hinter `max_parallel`

- **Aktiv** (zählt gegen die Grenze): `Countdown`, `Starting`, `Running`, `WaitsForScript`, `Asking`; nicht `Pending`, `Ready`, `WaitsForYou`, `Error`, `Done`.
- `schedule_ready` plant nur so viele Countdowns, wie Plätze frei sind, nach Vorrang und Planreihenfolge; die übrigen bereiten Stationen stehen in der Warteschlange und rücken selbst nach. Das gilt nur mit Automatik (`auto_start`, in der App „Automatisch weiter“, im Entwurf „Automatisch starten“); ohne Automatik gibt es keine Warteschlange, und ▶ ignoriert die Grenze.
- **Zählen und Beginnen unter einer Sperre:** `Countdowns::begin_within` zählt unter der Sperre der Countdowns die aktiven Stationen (eine mit Session nach ihrem Zustand aus der Ansicht, eine ohne nach den laufenden Countdowns und Starts dort) und beginnt die Countdowns. So überschreiten zwei gleichzeitige Fertigmeldungen die Grenze nicht. Countdown-Ende und Startanspruch sind ein Schritt (`take_for_start`), damit eine Station dazwischen nicht als frei zählt. Endet zwischen dem Lesen der Ansicht und dem Zählen ein Start (`release_start`), kennt die Ansicht die neue Session noch nicht und die Countdowns den Start nicht mehr; ein Zähler der Freigaben erkennt das, und `schedule_ready` liest die Ansicht neu (`begin_if_current`). Keine weitere Sperre: die Countdowns bleiben eine eigene, nie zusammen mit einer anderen gehaltene Sperre.
- **Neuer Haken:** Wird die Session einer Station fertig, pausiert, fehlerhaft oder abgebrochen (`SessionState::set_status`), plant ein eigener Thread nach dem Freigeben der Session-Sperre neu (`implementation::station_session_ended`). Er plant nur mit Grenze, Automatik, ohne Halt und solange die Umsetzung läuft (`resumes_queue`); ohne `max_parallel` ändert sich gegenüber früher nichts. Kein Zeitgeber, kein Polling. Mit Grenze plant so auch jedes Zugende einer Station-Session neu; ein abgebrochener Countdown ist nicht gespeichert und beginnt dann wieder, wie ohne Grenze nach jeder Fertigmeldung.
- **Startfehler nach dem Countdown:** Ohne Session endet keine, der Haken greift nicht; `fire` plant selbst neu (`resumes_queue_after_start_error`). Nur mit gespeichertem Startfehler, den `due_for_countdown` auslässt: Die Station wird nicht erneut versucht.
- Neu geplant wird außerdem nach einer Fertigmeldung, nach dem Übernehmen einer Plan-Änderung (wie bisher), beim Einschalten der Automatik, nach ▶ (nur mit Grenze: ▶ setzt die Warteschlange fort) und beim Ändern von `max_parallel` (`implementation_set_max_parallel`, Klasse wie die übrigen Schalter der Umsetzung). Senken der Grenze beendet weder laufende Countdowns noch Sessions; sie gilt ab dem nächsten Planen.
- **Nach einem Neustart** startet die Warteschlange nicht von selbst (Critical Rule 4): Countdowns liegen nur im Speicher, beim Laden plant nichts. ▶ oder Umlegen der Automatik setzt sie fort.
- Die Grenze gilt je Umsetzung, nicht über Vorhaben hinweg.

### Verworfen

- **Variante A** (der Agent sendet dem Nachfolger selbst eine Nachricht): zwei Wege für denselben Übergang, und der Verwalter sähe den Ablauf nicht mehr.
- **Gespeichertes `next`:** zwei Wahrheiten; ein Einfügen müsste den Vorgänger umschreiben.
- **Wellen als Quelle der Planung:** Sie bleiben nur als Eingabe des alten Schemas, aus der R1 Kanten macht.
- **Eine eigene Sperre für die Planung** (um Ansicht und Countdowns zusammen): Sie läge über Datenbank- und Session-Sperren; das Zählen unter der Sperre der Countdowns genügt, weil eine veraltete Ansicht nur zu wenig plant; den einen Fall, in dem sie zu viel plante (ein Start endet dazwischen), fängt der Zähler der Freigaben.

## Konsequenzen

- Eine Umsetzung ohne `after` und ohne `max_parallel` verhält sich wie vorher, bis auf die erste Nachricht, die nur noch die Ergebnisse der Vorgänger-Kanten nennt.
- Wer „Erledigt“ an einer Station des Benutzers drückt, bevor ihre Vorgänger fertig sind, macht ihre Nachfolger bereit, ohne dass die früheren Stationen der Kette fertig sein müssen; vorher hielt die Regel alle früheren Stationen fest. Der Befehl prüft das weiterhin nicht.
- Prüfen ohne App: `cargo run --example implementation-probe -- --selftest` (Sortierung, bereiter Satz, Zyklus, Einfügen, Verzweigung und Zusammenlauf, Migration, Warteschlange).
