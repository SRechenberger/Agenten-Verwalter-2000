# 075 — Prompt-Vorlagen für neue Vorhaben

**Status:** angenommen, Teil „Vorlagen und Neues Vorhaben“ umgesetzt; der Teil „Per MCP“ folgt · **Datum:** 2026-11-23

## Kontext

Wer ein Vorhaben startet, schreibt den Auftrag jedes Mal frei, auch wenn er immer gleich gebaut ist: ein Bugfix mit Fehler und Komponente, eine Taiga-Story mit Nummer und Titel. Die Story (Taiga #58) verlangt Prompt-Vorlagen im Mustache-Stil, aus denen der Verwalter ein Formular erzeugt. In „Neues Vorhaben“ wählt der Nutzer eine Vorlage, füllt das Formular, und der Auftrag entsteht live darunter; nach „OK“ ist er freier Text. Eine Vorlage kann auch den Namen des Vorhabens vorgeben (Nachtrag vom 2026-11-16).

Bisher gab es Platzhalter nur in Befehls-Stationen ([ADR 073](073-befehls-stationen.md)): `{{name}}` und `{{env:NAME}}`, ohne Abschnitte, ein fehlender Wert ist dort ein Fehler. Diese Regeln bleiben unberührt. Vorbild für Liste, Editor, eigene Tabelle und Prüfung im Core sind die Agententypen ([ADR 060](060-agententypen.md)).

## Entscheidungen des Nutzers (E1 bis E8, 2026-11-23)

| | Entscheidung |
|---|---|
| **E1** | Mustache-Teilmenge: Variablen `{{name}}`, Abschnitte `{{#name}}…{{/name}}`, Umkehrungen `{{^name}}…{{/name}}`, Kommentare `{{! … }}`. Keine Listen, keine Partials, keine eigenen Trennzeichen. Kein HTML-Escaping: `{{{name}}}` wirkt wie `{{name}}`, `<`, `&` und `"` bleiben stehen. |
| **E2** | Felder nur aus der Vorlage: Variable → Textfeld, das mitwächst; Abschnitt und Umkehrung → eine Checkbox. Der Name ist die Beschriftung (Umlaute und Leerzeichen erlaubt), Reihenfolge des ersten Auftretens, eine mehrfach genutzte Variable ist ein Feld. Ein Kommentar ist Hilfetext der nächsten Variable. Je Feld ist nichts einstellbar. |
| **E3** | Ein leeres Feld steht als `{{name}}` im Auftrag. „OK“ geht auch mit leeren Feldern. |
| **E4** | Das Formular steht über dem Auftrag, nicht modal. Solange es offen ist, ist der Auftrag schreibgeschützt; Diktat, `/`, „Vorhaben starten“ und Strg+Enter sind gesperrt, mit Hinweis. Anhänge, Repositories, Modell, Modus und Gruppe bleiben frei. Das Menü öffnet der Knopf ▤ in der Leiste des Eingabefelds. |
| **E5** | „OK“ schließt, danach ist der Auftrag normaler Text, und das Formular öffnet nicht wieder. „Verwerfen“ stellt den Text von vor der Wahl her. Eine neue Wahl fragt nach, wenn eigene Änderungen oder vorbelegter Text im Feld stehen. |
| **E6** | Abschnitt „Prompt-Vorlagen“ in der Kategorie Agent der Einstellungen: Name (eindeutig), Vorlagentext, optional eine Namensvorlage; Editor mit Formular-Vorschau. Ein Syntaxfehler blockiert das Speichern und nennt die Stelle. Eigene Tabelle, global, keine mitgelieferten Vorlagen. Der Knopf ▤ erscheint erst, wenn es eine Vorlage gibt. |
| **E7** | Zerlegen und Füllen laufen im Frontend als eigene Teilmenge in TypeScript, ohne neue Abhängigkeit, mit Selbsttest. Die Syntax prüfen Editor *und* Core: Der Core hat eine eigene Prüfung in Rust und prüft bei jedem Speichern. Eine gemeinsame Fallliste hält beide Fassungen gleich. |
| **E8** | Hat die Vorlage eine Namensvorlage, steht unter dem Auftrag ein Namensfeld, das sich live füllt. Ändert der Nutzer den Namen von Hand, bleibt sein Text. Der Name gilt als gesetzt, Haiku ersetzt ihn nicht. Ohne Namensvorlage bleibt alles wie bisher. |

Im Mockup gewählt: Drop-down B (Knopf ▤ neben + und /), Formular A (über dem Auftrag, einspaltig), Namensfeld B (unter dem Auftrag); siehe [docs/design/2026-11-23_prompt-vorlagen](../design/2026-11-23_prompt-vorlagen/README.md).

## Entscheidungen des Architekten (A1 bis A11, M4)

| | Entscheidung |
|---|---|
| **A1** | `project_create` nimmt `name: Option<String>`. Getrimmt und nicht leer gilt er wie bei `project_rename` (auf 60 Zeichen gekürzt), und das Vorhaben hat `name_auto = false`. Session 1 behält ihren Namen aus dem ersten Satz und den Titel von Haiku. |
| **A2** | Ein leeres Namensfeld ist `None`, also wie ohne Namensvorlage. |
| **A3** | Die Felder kommen aus Text und Namensvorlage, zuerst die des Texts. Ein leeres Feld bleibt auch im Namen `{{name}}`. |
| **A4** | Der Inhalt eines Tags wird getrimmt (nur ASCII-Leerraum, in beiden Fassungen gleich), ein Name hat 1 bis 40 Zeichen ohne `{` und `}`. `>`, `=` und `&` am Anfang sind ein Syntaxfehler („nicht unterstützt“). Derselbe Name als Variable und als Abschnitt ist ein Syntaxfehler, auch über Text und Namensvorlage hinweg. |
| **A5** | Abschnitte dürfen geschachtelt sein. Fehler mit Quelle, Zeile und Spalte: nicht geschlossen, falsch geschlossen, `{{/x}}` ohne Öffnung. Steht ein Abschnitts-, Umkehr-, Schluss- oder Kommentar-Tag allein auf seiner Zeile, nimmt er die Zeile mit. |
| **A6** | Checkboxen starten aus. Kommentare stehen nie im Auftrag. Ein Kommentar ohne folgende Variable wird nicht gezeigt. |
| **A7** | Nach „OK“ bleibt das Namensfeld, „Verwerfen“ nimmt es weg. Eine neue Wahl ersetzt Formular, Auftrag und Namensfeld, nach der Nachfrage. |
| **A8** | Grenzen im Core: Name 1 bis 40 Zeichen, eindeutig ohne Beachtung der Schreibweise („Der Name „…“ gehört schon einer anderen Vorlage.“); Vorlagentext 1 bis 16 000 UTF-16-Einheiten, Namensvorlage 0 bis 200 Zeichen; dazu die Syntax (M4). |
| **A9** | Ändert sich die Vorlage, während das Formular offen ist, arbeitet das Formular mit dem Stand bei der Wahl weiter. |
| **A10** | Tooltip „Vorlage wählen“, Hinweis in der Leiste „Mit / startest du mit einem Skill, mit ▤ aus einer Vorlage.“, der Kopf des Formulars nennt die Vorlage, die Textfelder wachsen mit `field-sizing: content`. |
| **A11** | Vorlagen sind nur lokal: Die vier Befehle stehen in Klasse G von `remote/access.rs`. |
| **M4** | `src-tauri/src/prompt_templates/syntax.rs` prüft nur, es füllt nicht; aufgerufen bei Anlegen und Ändern (später auch bei der Antwort auf einen Vorschlag). Die Fallliste `syntax_cases.json` (Eingabe → gültig, oder Quelle, Zeile, Spalte und Satz) lesen der Rust-Test (`include_str!`) und der Selbsttest; beide Fassungen müssen jeden Fall gleich beantworten. |

## Umsetzung

- **Core:** `src-tauri/src/prompt_templates/` (`model.rs` mit den Typen über die Tauri-Grenze, `validate.rs`, `syntax.rs`, `syntax_cases.json`), Tabelle `prompt_templates` (`src-tauri/src/db/prompt_templates.rs`, Migration 035), Befehle `prompt_templates_list`, `prompt_template_create`, `prompt_template_update`, `prompt_template_delete` (`src-tauri/src/commands/prompt_templates.rs`). Prüffehler kommen als `PromptTemplateSaved.errors` (Feld `name`, `text` oder `nameTemplate`), ein Syntaxfehler als „Zeile <z>, Spalte <s>: <Satz>“ mit demselben Satz wie in `mustache.ts`. Das Ereignis `prompt-templates://changed` trägt die ganze Liste und geht nur nach erfolgreichem Speichern oder Löschen hinaus: `prompt_templates::create`, `update` und `delete` rufen den Rückruf des Befehls erst danach.
- **Name:** `project_create` gibt `name` an `create_project` weiter; `project_name` entscheidet nach A1 und A2, `ProjectState::takes_title` lässt `retitle_project` einen gesetzten Namen stehen.
- **Oberfläche:** `src/features/prompttemplates/` mit `mustache.ts` (Zerlegen, Prüfen, Füllen; Selbsttest `mustache.selftest.ts`), der Zeile in den Einstellungen samt Editor und Löschdialog, dem Menü am Knopf ▤, dem Formular und den Texten. „Neues Vorhaben“ (`NewSession.tsx`) hält Formular, Namensfeld und den Text von vor der Wahl als lokalen Zustand.
- **Zeile und Spalte** zählen in beiden Fassungen Codepunkte, 1-basiert; eine Zeile endet an `\n`.

## Verworfen

- **npm `mustache`:** eine neue Abhängigkeit für vier Tag-Arten, mit HTML-Escaping und Listen, die hier stören (E7).
- **Nur in Rust:** jeder Tastendruck ginge über den Core; das Füllen bleibt in TypeScript, nur die Prüfung gibt es zweimal (E7, M4).
- **Einstellungen je Feld** (Beschriftung, mehrzeilig, Auswahlliste, Standardwert): Die Vorlage ist die einzige Quelle der Felder (E2).
- **Drop-down als eigene Zeile über dem Auftrag** (Mockup 1A): kostet eine Zeile auf jeder Seite, auch ohne Wahl.
- **Formular neben dem Auftrag** (Mockup 2B): braucht rund 1060 px statt 720 px.
- **Namensfeld im Formular** (Mockup Namensfeld A): Das Formular verschwindet mit „OK“, der Name bleibt; das Feld müsste wandern.

## Folgen

- Die Prüfung der Syntax gibt es zweimal. Ein Fall, den nur eine Fassung kennt, gehört in die gemeinsame Liste; weicht eine ab, scheitert der Rust-Test oder der Selbsttest.
- Migration 035 hebt die Schemaversion; die drei Tests, die sie festhalten, zählen 35.
- Ein Vorhaben mit Namen aus der Vorlage behält ihn gegen Haiku; umbenennen geht wie bisher.
- Vorlagen sind global und nur lokal; ein Gast sieht sie nicht und legt mit „Neues Vorhaben“ ohnehin ein lokales Vorhaben an.
- Der Teil „Per MCP“ (Werkzeuge `prompt_template_list`, `prompt_template_propose_create`, `prompt_template_propose_update`, Kasten im Chat mit Diff) folgt als eigener Abschnitt dieser ADR.
