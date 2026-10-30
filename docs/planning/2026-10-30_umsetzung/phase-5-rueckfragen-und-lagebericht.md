# Phase 5 — Rückfragen mit Lagebericht als Anhang, Knopf „Lagebericht“

Rating **standard** · Wave 5 · Aufwand M · Status: pending

## Kontext

- [README.md](README.md): Entscheidungen „Rückfragen“, „Lagebericht“ (vollständige Zeilen inkl. „Ziel“ und „Geänderte Dateien“), Kontrakt `implementation_report`, `implementation_new_inquiry`, `SessionSummary.inquiry_of`.
- Entwurf [artifacts/entwurf-umsetzung.html](artifacts/entwurf-umsetzung.html), Abschnitt **5** (Rückfrage als reguläre Session, Lagebericht als Anhang, Nachtrag beim Senden, Lagebericht ohne Session).
- `src-tauri/src/attachments/mod.rs` (`add_bytes` legt einen Anhang im Zwischenordner an; `take_for_workspace` beim Senden), `src/stores/attachments.ts` (ungesendete Anhänge je Session), `src/features/attachments/useAttachmentInput.ts` (wie ein Anhang in den Entwurf kommt), `src/features/chat/composerDraft.ts` (Entwurf setzen, Muster Retro), `src-tauri/src/sessions/registry.rs` `send` (Stelle `carried`/`with_project_tldr` — der Nachtrag des Lageberichts hängt sich dort an: Anhang anlegen **vor** `update`, ID an `attachment_ids` anhängen), `notices/model.rs` (`Notice`-Arten, Muster `note_compaction` in `registry.rs`), `src/features/notices/` (`NoticeBlock`, `noticeTexts`), `src-tauri/src/changes/mod.rs` + `SessionRegistry::changes_input` (Dateizahl „Alle“ in Reichweite Vorhaben; `plain`-Aufruf ohne Diff), `src-tauri/src/sessions/registry/retro.rs` (Muster: Session im Vorhaben anlegen und Entwurf vorbelegen), `src/app/Sidebar.tsx` + `SidebarProject.tsx` (Kontextmenü eines Vorhabens — Muster für das Menü der Umsetzungs-Zeile), `src/app/SidebarImplementation.tsx`.
- [docs/conventions/rust.md](../../conventions/rust.md), [react.md](../../conventions/react.md). Fehlerklassen (Vault): keine einschlägig.

## Abnahmekriterien

1. „Neue Rückfrage“ (Knopf in der Kopfzeile der Umsetzung, `title` „Frage zur Umsetzung in einer frischen Session — der Lagebericht hängt dran“, und Zeile im Kontextmenü der Umsetzungs-Zeile in der Sidebar) legt eine Session „Neu“ im Vorhaben an (`create_in_project`), setzt `inquiry_of`, legt den Lagebericht als Datei `lagebericht-<HHMM>.md` im Zwischenordner an, hängt ihn als ungesendeten Anhang an diese Session, öffnet sie und setzt den Fokus ins Eingabefeld. Der Name der Session entsteht wie bei jeder Session aus der ersten Nachricht.
2. Der Lagebericht enthält alle Zeilen nach Kontrakt, in einer Rückfrage mit der Zeile „Diese Session ist eine Rückfrage: …“, „Ziel“ aus der README der Quelle (nur wenn `source` ein Ordner mit `README.md` im Workspace oder in einem Repository-Ordner des Vorhabens ist; Pfadgrenze wie `file_links::resolve`), „Geänderte Dateien“ aus den Changes der Reichweite Vorhaben.
3. Beim Senden aus einer Rückfrage vergleicht der Core den aktuellen Bericht (ohne „Stand“-Zeile) mit dem zuletzt angehängten (gemerkt im Speicher je Session: `SessionState.last_report: Option<String>`; nach Neustart leer → beim nächsten Senden wird angehängt); bei Unterschied hängt er `lagebericht-<HHMM>.md` als weiteren Anhang an und schreibt den Hinweis „Lagebericht aktualisiert angehängt · Stand <HH:MM> · <kürzeste Änderung: „Phase 2 fertig“ / „Plan geändert“ / „Stand geändert“>“ in den Verlauf (`Notice::ReportUpdated { at, summary }`).
4. Knopf „Lagebericht“ in der Kopfzeile zeigt den aktuellen Text im `ReportPanel` (Phase 1) als Fenster über der Ansicht, mit „Kopieren“ (Zwischenablage) und „Schließen“.
5. Rückfragen stehen als normale Session-Zeilen in der Sidebar; `SessionSummary.inquiry_of` ist gesetzt; die Meta-Zeile zeigt „Rückfrage zur Umsetzung“ solange Status „Neu“.
6. `implementation-probe … report <Umsetzungs-ID>` druckt den Lagebericht (ohne Zustände der Sessions, wie `show`, aber vollständig).

## Checkliste

- [ ] `report.rs`: Zeilen „Ziel“ (`fn goal_of(source, roots) -> Option<String>`), „Geänderte Dateien“ (Zahl über `changes_input` Reichweite Vorhaben, Blickwinkel „Alle“; Fehler → Zeile weglassen), „Rückfrage“-Zeile per Parameter; `pub fn stripped(text) -> String` (ohne „Stand“-Zeile, für den Vergleich).
- [ ] `registry/implementation.rs`: `pub fn new_inquiry(&self, app, implementation_id) -> Result<SessionSummary, CommandError>` (`create_in_project`, `inquiry_of` in Zustand und Datenbank, Bericht als Anhang über `attachments::add_bytes(app, "lagebericht-<HHMM>.md", base64)`, Rückgabe des `Attachment` an die Oberfläche: Command liefert `(SessionSummary, Attachment)` als Struct `InquiryCreated { session, attachment }`, `derive(TS)`, in `gen-bindings.rs`).
- [ ] `registry.rs` `send`: vor `update`, wenn `state.inquiry_of` gesetzt: Bericht bauen, mit `last_report` vergleichen, ggf. Anhang anlegen und `attachment_ids` erweitern, nach `update` `last_report` setzen und `Notice::ReportUpdated` als Eintrag schreiben (Muster `note_compaction`).
- [ ] `notices/model.rs`: `Notice::ReportUpdated { at: f64, summary: String }`; Oberfläche `NoticeBlock`/`noticeTexts`: Zeile „Lagebericht aktualisiert angehängt · Stand <HH:MM> · <summary>“.
- [ ] `commands/implementation.rs`: `implementation_report`, `implementation_new_inquiry`; Wrapper `loadImplementationReport`, `createInquiry`.
- [ ] Oberfläche: `ImplementationHeader.tsx` Knöpfe „Neue Rückfrage“ und „Lagebericht“; `src/features/implementation/useInquiry.ts` (ruft `createInquiry`, legt den Anhang in `src/stores/attachments.ts` an die neue Session, `selectSession`, Fokus ins Eingabefeld über den bestehenden Weg des `Composer` — nachlesen, wie `NewSessionIntro`/Retro den Fokus setzen); Kontextmenü an `SidebarImplementation.tsx` (Zeile „Neue Rückfrage“; Muster `SidebarProject`); `ReportPanel` als Fenster (`Dialog` aus `src/components`), „Kopieren“ über `navigator.clipboard.writeText`.
- [ ] `src/features/sessions/sessionStatus.ts` `metaLine`: bei `new` und `inquiryOf !== null` „Rückfrage zur Umsetzung“.
- [ ] `implementation-probe.rs`: Aufruf `report`.
- [ ] `pnpm check` grün, Commit `feat(implementation): Rückfragen mit Lagebericht als Anhang`.

## Definition of Done

- AK 1–6 erfüllt; **Reviewable Artifact:** eine erzeugte `lagebericht-<HHMM>.md` aus einer Rückfrage (Inhalt im Report-Back) und ein Screenshot des Verlaufs mit dem Hinweis „Lagebericht aktualisiert angehängt“ (`artifacts/phase-5-rueckfrage.png`); **User-visible outcome:** Klick auf „Neue Rückfrage“ öffnet eine Session mit dem Bericht als Anhang, der Cursor steht im Eingabefeld.
- Commands run: `pnpm check`, Probe-Aufruf.
- Seven-Goals-Check (je ein Satz); Carry-Overs höchstens 2.

## Report-Back
