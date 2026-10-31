# 050 — Einstiegszeile „Zurück an …“

**Status:** angenommen · **Datum:** 2026-10-31

## Kontext

Eine Session, die ein Entwickler- oder Prüfauftrag aus einer anderen Session gestartet hat, meldet ihr Ergebnis am Ende der Antwort („<Lane> reported“). Der Nutzer kopierte diesen Satz bisher von Hand in die Eltern-Session. Die Einstiegszeile „Weiter: …“ ([ADR 027](027-session-uebergabe.md), [ADR 040](040-automatisch-weiter.md)) passt nicht: Sie legt eine neue Session an, statt in die bestehende zurückzukehren.

## Optionen

- **Ziel:** (a) die Session per Titel in der Zeile suchen — verworfen: Titel ändern sich, sind nicht eindeutig, und das Modell müsste sie kennen; (b) der Core merkt die Eltern-Session beim Anlegen — gewählt.
- **Wirkung des Knopfs:** (a) Entwurf der Eltern-Session setzen und dorthin wechseln, wie bei „Weiter:“ — gewählt; (b) direkt senden — verworfen: Die Antwort geht ungeprüft an eine andere Session.
- **Inhalt:** (a) die ganze Zeile — verworfen: „Zurück an Architekt:“ ist Anweisung an die App, nicht an die Session; (b) nur der Text hinter dem ersten Doppelpunkt — gewählt.

## Entscheidung

- **Dritte Art der Einstiegszeile:** „Zurück an <Name>: <Text>“ (Groß-/Kleinschreibung beachten; Name nicht leer, höchstens 80 Zeichen, ohne Doppelpunkt; Text nicht leer). Prüfreihenfolge: Weiter, Neues Vorhaben, Zurück. Die Zeile zählt nicht gegen das Fenster der fünf Zeilen, der Text wird nie als Modellwahl gelesen. `HandoffKind::Back` (TS `'back'`).
- **Eltern-Session im Core:** `sessions.parent_session_id` (Migration 016, ohne Fremdschlüssel), `SessionSummary.parent_session_id`. `session_create_in_project(project_id, parent_session_id)` merkt sie für die neue (oder wiederverwendete „Neu“-)Session, wenn sie im selben Vorhaben liegt und nicht die Session selbst ist. Löscht der Nutzer die Eltern-Session, setzt der Core die Angabe bei den Kindern auf `None`; beim Laden räumt `forget_missing_parents` Reste auf.
- **Oberfläche:** „In neuer Session weiter“ übergibt die aktuelle Session als Eltern-Session. Unter einer Antwort mit „Zurück an …“-Zeile steht — nur mit gemerkter Eltern-Session — der Knopf „Zurück an <Name>“ (Name der Eltern-Session, sonst der aus der Zeile). Klick: Text in den Entwurf der Eltern-Session, dorthin wechseln, Eingabefeld fokussieren; gesendet wird nichts. Ohne Eltern-Session gibt es keine Reihe und keine Leiste.
- **„Automatisch weiter“:** Nur wenn die Antwort keine „Weiter:“-Zeile und genau eine „Zurück an“-Zeile (mehr: `SeveralLines`) hat und eine Eltern-Session existiert. Nach 10 s Countdown („Sendet automatisch in N s an <Name>“) sendet der Core den Text an die Eltern-Session, wenn sie im Status `Completed` ist, ohne offene Rückfrage und ohne laufenden Subagent oder Skript; sonst Halt `ParentBusy`. Dazu gelten dieselben Haltegründe wie bei „Weiter:“ (🔴, Frage, „Merge“, Kettenbremse). Das Senden läuft ohne Sperre ([ADR 040](040-automatisch-weiter.md)); danach `set_auto_chain(parent, chain + 1)` und `session://handoff-started` von Kind- zu Eltern-Session.
- **Sidebar:** Eine Antwort, die nur „Zurück an“-Zeilen trägt, ist nicht „wartet auf Wiedereinstieg“.

## Konsequenzen

- Nur per Knopf gestartete Sessions kennen ihre Eltern-Session; eine von Hand angelegte hat keine, ihre „Zurück an“-Zeile bleibt ohne Knopf.
- Löscht der Nutzer die Eltern-Session, entfällt der Knopf bei den Kindern.
- Der Name wird aus der Zeile nur für die Anzeige gelesen, der Text beginnt aber nach dem ersten Doppelpunkt der Zeile — der Name darf also keinen enthalten.
- Migration 016 ergänzt eine Spalte; ältere Datenbanken laufen unverändert weiter.
- Die Skills (Entwickler, Prüfer, Session-Briefs) müssen die Zeile noch ausgeben; das ist eine eigene Lane im Vault.
- Code: `src-tauri/src/sessions/` (`handoff.rs`, `auto_handoff.rs` mit `fire_back`, `registry.rs` mit `parent_rest` und `forget_parent`), `src/features/chat/` (`BackButton`, `HandoffBar`).
