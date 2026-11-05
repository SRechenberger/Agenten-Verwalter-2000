# Phase 4 — Oberfläche: Abschnitt „Mitgeliefert“ im `/`-Menü

## Kontext

Lesen: [README.md](README.md) (Entscheidungen „Typ über die Tauri-Grenze“, „Kein Skill-Abzeichen“; Kontrakt `SkillOrigin`; AK 1–3), [ADR 034](../../decisions/034-mitgelieferte-befehle.md) (aus Phase 3), [docs/conventions/react.md](../../conventions/react.md), [docs/conventions/typescript.md](../../conventions/typescript.md), [docs/conventions/tailwind.md](../../conventions/tailwind.md).

Code: `src/features/chat/commandMenuRows.ts` (`CommandSection`, `buildCommandSections`, `rowKey`, `rowText`), `src/features/chat/CommandMenu.tsx` (`rowOrigin`, `EMPTY_SKILLS_HINT`), `src/features/chat/useCommandMenu.ts`, `src/features/chat/Composer.tsx` und `src/features/sessions/NewSession.tsx` (`pickRow`: `row.kind === 'skill'` → `applySkillToDraft`), `src/lib/bindings/SkillOrigin.ts` (aus Phase 3).

Design: kein eigener Entwurf. Der Abschnitt nutzt die vorhandene Zeile des Menüs unverändert (Bezeichnung `/name`, Beschreibung, Marke rechts) und bekommt eine eigene Überschrift wie die übrigen Abschnitte. Kein neues CSS.

## AK der Phase

1. Das Menü hat in dieser Reihenfolge die Abschnitte „Kontext“, „Modell“, „Skills“, „Mitgeliefert“ und „Session“; leere Abschnitte fallen weg wie bisher. „Skills“ enthält die Einträge mit `origin.kind` `user` oder `repository`, „Mitgeliefert“ die mit `builtin`.
2. Die Marke rechts lautet bei mitgelieferten Befehlen „mitgeliefert“.
3. Wählen (Klick oder Enter) setzt `/name ` an den Anfang des Entwurfs, genau wie bei einem Skill; gesendet wird erst mit Senden.
4. Das Filtern (Knopf-Menü und Tippen nach `/`) trifft auch mitgelieferte Befehle über Name und Beschreibung.
5. In „Neue Session“ (Scope `skills`) erscheinen „Skills“ und „Mitgeliefert“.
6. Ein exakt getippter Session-Befehl (`/rename` usw.) steht weiter ganz oben.

## Checkliste

- [ ] `commandMenuRows.ts`: `CommandSection.title` um `'Mitgeliefert'` erweitern. In `buildCommandSections` die `skills` in `fileSkills` (`origin.kind !== 'builtin'`) und `builtinSkills` (`origin.kind === 'builtin'`) teilen. Der Abschnitt `{ title: 'Skills', rows: fileSkills… }` bleibt, direkt danach kommt `{ title: 'Mitgeliefert', rows: builtinSkills.map(… kind: 'skill' …) }` in jedem Scope.
- [ ] `commandMenuRows.ts` `rowKey`: Herkunft per `switch (row.skill.origin.kind)` → `'user'`, `'builtin'`, `` `repository:${name}` `` (erschöpfend, ohne `default`).
- [ ] `CommandMenu.tsx` `rowOrigin`: `switch` über `row.skill.origin.kind` → `'Benutzer'`, `'mitgeliefert'`, `` `aus ${name}` ``.
- [ ] `CommandMenu.tsx`: prüfen, dass `EMPTY_SKILLS_HINT` nur greift, wenn beide Abschnitte leer sind (Bedingung `sections.length === 0` reicht, keine Änderung erwartet); Ergebnis im Report-Back nennen.
- [ ] Doc: `docs/glossary.md` Eintrag **Skill**: den Satz „Plugin- und eingebaute Skills der Kommandozeile nicht“ ersetzen durch „dazu im Abschnitt „Mitgeliefert“ die mitgelieferten Befehle der Betriebsart (ADR 034)“. Neuer Eintrag **Mitgelieferter Befehl**: „Ein `/name`, das der Agent der Betriebsart selbst versteht, ohne Datei: in „Claude“ die Liste der Kommandozeile, in „Autark“ `/compact`, in „Codex“ `/compact` und `/codex-review`.“
- [ ] Doc: `docs/code-map.md` Zeile „Skills und Befehle“: Oberfläche „Abschnitt „Mitgeliefert“ in `commandMenuRows.ts`“ ergänzen; Zeile „Chat“: beim `/`-Menü „(inkl. `/mcp` und Abschnitt „Mitgeliefert“)“.
- [ ] `pnpm check` grün; Commit `feat(skills): Abschnitt „Mitgeliefert“ im /-Menü`.
- [ ] Planabschluss nach `mode-implementing`: Bottom-Sektionen der README füllen, Plan nach `docs/archive/2026-10/` verschieben, STATE.md aktualisieren, Versions-Tag (Minor) nach [releases.md](../../conventions/releases.md); Smoke-Checkliste der README an den User.

## Report-Back
