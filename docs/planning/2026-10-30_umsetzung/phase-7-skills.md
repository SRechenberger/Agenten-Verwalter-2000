# Phase 7 — Skills: Planung und Umsetzung auf die Werkzeuge des Verwalters umstellen

Rating **standard** · Wave 7 · Aufwand M · Status: pending

**Ort:** nicht das Verwalter-Repo. Geschrieben wird im Master der eigenen Konfiguration, `G:\Meine Ablage\U104_Agents\` (`skills/`, `commands/`, `CLAUDE.md`, `skill-evolution-log.md`), danach gespiegelt nach `~\.claude\skills` und `~\.claude\commands` (`robocopy … /E`). Kein Commit im Verwalter-Repo; der Report-Back dieser Phase ist der Decision-Log-Eintrag. Läuft die Phase als Station im Verwalter, muss der Master-Ordner als „Ordner ohne Git“ am Vorhaben hängen (ADR 018), sonst darf der Agent dort nicht schreiben.

## Kontext

- [README.md](README.md): Kontrakt „Werkzeuge“ (Namen, Eingaben), Entscheidungen „Erste Nachricht“, „Fertig heißt gemeldet“, „Fix-Runde“, „Plan-Änderung“.
- Entwurf [artifacts/entwurf-umsetzung.html](artifacts/entwurf-umsetzung.html), Abschnitt **8** (was sich an den Skills ändert) — die Vorgabe dieser Phase.
- `~\.claude\knowledge\topics\skill-evolution.md` (Decision-Log, Spiegeln, Zweistufigkeit), `~\.claude\knowledge\topics\profile-private.md`, `~\.claude\CLAUDE.md` Abschnitt „Autonomie & Agenten“ (Absatz „Unteragenten sind Schlucker, nie Kopf“), „Regeldateien enthalten nur Anweisungen“ (keine Vorfall-Erzählungen in die Skills).
- Zu ändernde Dateien im Master: `skills/mode-planning/SKILL.md` (Abschnitt „Nach dem Plan-Vorlegen“), `skills/mode-planning/WORK.md` („Nach dem Plan-Vorlegen“, „Parallelisierung ausweisen (Wave)“, „Recommend next actions“), `skills/mode-implementing/SKILL.md` („Einstieg“, „Umsetzungs-Loop pro Phase“ Schritt 8, „Pre-Clear Checklist“, „Plan fertig (private)“), `skills/mode-implementing/WORK.md` (Einleitung Orchestrator, „Rollen & Briefings“, „Modellwahl pro Phase“, „Spawn-Schema“, „Single-Writer-Regel“, „Ablauf pro Phase“, „Parallel-Waves“, „Context-Hygiene“, „Pre-Clear Checklist“, „Abschluss“, „Recommend next actions“), `skills/mode-implementing/agents/tester.md`, `developer.md`, `reviewer.md`, `commands/implement.md`, `commands/plan.md`, `CLAUDE.md` (Zeile „Unteragenten sind Schlucker, nie Kopf“).
- Fehlerklassen (Vault): keine einschlägig (Regeltexte).

## Abnahmekriterien (jede Regel ein prüfbarer Satz in der Zieldatei)

1. **Erkennung des Verwalters:** In `mode-planning/SKILL.md` und `mode-implementing/SKILL.md` steht je ein Absatz „Im Agenten-Verwalter“: Sind die Werkzeuge `mcp__verwalter__implementation_*` vorhanden (Werkzeugliste prüfen, ggf. per ToolSearch), gelten die Verwalter-Regeln dieser Phase; fehlen sie (Terminal), gilt alles Übrige unverändert.
2. **Planung, beide Profile** (`mode-planning/SKILL.md` + `WORK.md`, „Nach dem Plan-Vorlegen“): Nach Freigabe und Commit ruft die Session `implementation_create` mit `name` (Plan-Titel), `source` (Plan-Ordner relativ zum Repository bzw. Manager-Schlüssel), je Phase `key` (Phasennummer aus der Overview), `title`, `description` (ein, zwei Sätze), `wave` (aus der Spalte „Wave“, nur work), `stations` nach Tabelle: *private* — eine Station `Umsetzung` (`skill: "implement"`, Modell `sonnet` bei mechanisch/standard, `opusplan` bei heikel); *work* — mechanisch: `Umsetzung` (`sonnet`); standard: `Test` (`sonnet`), `Umsetzung` (`sonnet`), `Review` (`sonnet`); heikel: dieselben drei auf `opus`; alle mit `skill: "implement"` und `brief` = Rolle + Phase („Tester für Phase 3: AK-Tests schreiben, rot belegen“); am Ende eine Phase `Abschluss` mit *private* `Abnahme` (`by: "you"`), *work* `Gesamt-Review` (`fable`, `skill: "review"`) und `Merge` (`by: "you"`). Keine Einstiegszeile mehr; die Antwort endet mit „Umsetzung angelegt: <n> Phasen, <m> Stationen“. Scheitert der Aufruf, steht der Fehlersatz in der Antwort und die Einstiegszeile wie bisher.
3. **Plan ändert sich** (`mode-planning` beide Dateien, `mode-implementing/WORK.md` „Findings & Deviations-Relay“): Zwischenphase oder Umstellung → `implementation_set_phases` mit der vollständigen Liste der kommenden Phasen, **danach** Plan-Dateien bzw. Manager nachziehen; die Freigabe holt der Verwalter über die Karte — kein 🔴 im Chat dafür.
4. **Umsetzung, private** (`mode-implementing/SKILL.md`): Phasengrenze im Verwalter = Stand persistieren, `implementation_complete_station` mit einer Ergebniszeile, Antwort beenden — kein `/clear`-Aufruf, keine Modell-Empfehlung, keine Einstiegszeile; die nächste Station startet der Verwalter. Letzte Phase gibt die Prüfliste aus; sie gehört zur Station „Abnahme“. Unverändert: keine Tests, keine Reviews, keine Subagenten.
5. **Umsetzung, work** (`mode-implementing/WORK.md`): Der Orchestrator-Modus mit Tester-/Developer-/Reviewer-Agenten entfällt im Verwalter; Test, Umsetzung und Review sind Stationen, die Briefings `agents/tester.md`, `developer.md`, `reviewer.md` gelten für die jeweilige Station (die Station liest ihr Briefing selbst, Hinweis im `brief`); Fertigmelden: Test und Umsetzung mit `implementation_complete_station`; das Review meldet grün **erst nach dem Commit** (gebündelter Review-Modus: Commit auf den Feature-Branch durch die Review-Station; Review-Modus „pro Phase“: nach Freigabe des Users im Chat, bis dahin keine Meldung); bei Mängeln `fix_needed: true` (Fix-Runde + zweites Review hängt der Verwalter an); nach zwei Fix-Runden ohne grün keine Meldung, sondern 🔴 an den User; Report-Back in die Phasen-Datei, die Ergebniszeile geht mit der Fertigmeldung; Schreiber von STATE.md und Commits ist bei einer Welle die erste Station nach dem Zusammenlauf, jede Spur schreibt nur ihre Phasen-Datei; Rot-Gate: die Test-Station belegt Rot selbst gegen den Stand vor der Phase, die Bestands-Suite läuft einmal in der ersten Test-Station des Plans; Manager-Fall: `start_work` macht die erste Station einer Phase, `complete_work` das Review nach dem Commit; Subagenten nur für Mengenarbeit.
6. **Briefings** (`agents/*.md`): Zusatz je Datei „Als Station im Agenten-Verwalter: Report-Back in die Phasen-Datei, Ergebniszeile (eine Zeile, höchstens 200 Zeichen) per `implementation_complete_station`; keine Einstiegszeile; bei 🔴 keine Meldung — der Verwalter wartet dann auf den Benutzer.“
7. **Befehle** (`commands/implement.md`, `commands/plan.md`): `implement` — beginnt die erste Nachricht mit `Station:`, entfällt die Plan-Auswahl; Plan und Phase stehen in der Nachricht; `plan` — Schritt 5 nennt `implementation_create` nach AK 2.
8. **CLAUDE.md:** In Zeile „Unteragenten sind Schlucker, nie Kopf“ wird „gestartet per Einstiegszeile („Weiter: … Modell …, nächster Schritt: …“)“ zu „gestartet per Einstiegszeile („Weiter: … Modell …, nächster Schritt: …“) oder, im Agenten-Verwalter, über dessen Umsetzung“. Sonst nichts.
9. **Decision-Log** `skill-evolution-log.md`: ein Eintrag nach Vorlage des Topics (Datum, Auslöser „Umsetzung im Verwalter, ADR 048“, geänderte Dateien, Kern der Regel), danach `robocopy` beider Ordner; `Get-ChildItem ~\.claude\skills\mode-implementing` zeigt die neue Änderungszeit.
10. Jede neue Zeile ist eine Anweisung mit Prüfstein, keine Erzählung („Regeldateien enthalten nur Anweisungen“); keine Datumsangaben in den Skills; Modellnamen wie in den Tabellen der Skills.

## Checkliste

- [ ] `mode-planning/SKILL.md` + `WORK.md` nach AK 1–3 (eine Tabelle „Stationen je Rating“ in `WORK.md`, private-Zeile in `SKILL.md`).
- [ ] `mode-implementing/SKILL.md` nach AK 1, 4; `WORK.md` nach AK 5 (Abschnitt „Im Agenten-Verwalter: Stationen statt Orchestrator“, der die betroffenen Abschnitte je mit einem Satz umlenkt, statt sie zu löschen — im Terminal gelten sie weiter).
- [ ] `agents/tester.md`, `developer.md`, `reviewer.md` nach AK 6.
- [ ] `commands/implement.md`, `commands/plan.md` nach AK 7.
- [ ] `CLAUDE.md` nach AK 8.
- [ ] Decision-Log und Spiegeln nach AK 9; Selbst-Test nach AK 10 (jede geänderte Zeile gegen den Prüfstein lesen).

## Definition of Done

- AK 1–10 erfüllt; **Reviewable Artifact:** der Decision-Log-Eintrag (Text im Report-Back) und die `robocopy`-Zusammenfassungszeilen; **User-visible outcome:** Eine Planungs-Session im Verwalter endet mit „Umsetzung angelegt“, die Sidebar zeigt die Umsetzung; im Terminal läuft alles wie heute.
- Seven-Goals-Check (je ein Satz; Token-Effizienz: die Skills werden nicht länger als nötig — jede neue Zeile ersetzt eine Einstiegszeilen-Regel oder lenkt um).

## Report-Back
