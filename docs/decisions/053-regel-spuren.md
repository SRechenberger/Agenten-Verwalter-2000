# 053 — Regel-Spuren in der Vorhaben-Retro

**Status:** angenommen · **Datum:** 2026-11-04

## Kontext

Die Vorhaben-Retro ([ADR 028](028-vorhaben-retro.md)) zeigte bisher nur, was schiefging. Eine Hook-Sperre erschien im Verlauf als „Werkzeug fehlgeschlagen: Bash …“ ohne Grund, eine funktionierende Regel sah also aus wie ein Fehlversuch. Erfolgreiche Werkzeug-Aufrufe fehlten im Verlauf, damit auch alle Skill-Aufrufe durch den Agenten. Die Mini-Retro sammelte nur Korrekturen, Fehlschläge, unbelegte Behauptungen, Fakten und Offenes, nie die Gegenrichtung „hat gewirkt“.

## Optionen

- **(a) Hook-Sperren am Präfix der gespeicherten Werkzeug-Ausgabe erkennen** ([ADR 042](042-werkzeug-details.md)) — gewählt.
- **(b) Hooks schreiben Session-ID und Namen in eine eigene Datei, der Verwalter liest sie** — verworfen: koppelt den Verwalter an Skripte des Nutzers außerhalb seines Datenbestands.
- **(c) Das Modell der Mini-Retro erkennt Sperren selbst** — verworfen: eine Zählung gehört in deterministischen Code (ADR 028: der Verwalter sammelt, der Skill urteilt).

## Entscheidung

- Eine Sperre ist ein fehlgeschlagener Werkzeug-Aufruf, dessen gespeicherte Ausgabe mit `PreToolUse:` beginnt und ` hook error: ` enthält; der Sperrgrund ist der Text danach, eine Zeile, höchstens 200 Zeichen. Hook-Namen stehen nicht im Text und werden nicht geraten.
- Skill-Aufrufe zählen aus dem Menü-Eintrag (`/name`) und aus dem Skill-Werkzeug des Agenten, getrennt nach Herkunft.
- `session-<N>.md` und `befunde.md` tragen `Hook-Sperren:` und `Skills:`; im Verlauf steht `Hook-Sperre: <Werkzeug> <Ziel> — <Grund>` statt „Werkzeug fehlgeschlagen“.
- Die Mini-Retro bekommt die Kategorie `effects` („Regel hat gewirkt“).
- Code: `src-tauri/src/retro/traces.rs`, Probe `examples/retro-probe.rs`.

## Konsequenzen

- Aufrufe vor [ADR 042](042-werkzeug-details.md) haben keine Ausgabe; ihre Sperren bleiben unerkannt.
- Ändert Claude Code das Präfix, fällt die Erkennung still auf „Werkzeug fehlgeschlagen“ zurück; `retro-probe` zeigt es. Die Erkennung steht in genau einer Funktion (`parse_hook_block`).
- Einwürfe von Hooks ohne Sperre (`additionalContext`) bleiben unsichtbar.
- Codex liefert keine Spuren. Autark schreibt Hook-Sperren im selben Wortlaut (siehe Nachtrag).

## Nachtrag 2026-11-22 (Taiga 54)

- Autark gibt bei einer Hook-Sperre (`HookVerdict::Deny`) als Ausgabe `PreToolUse:<Werkzeug> hook error: <Grund>` aus, im Wortlaut von Claude Code. Das Modell sieht denselben Text wie dort, die Retro zählt die Sperre.
- Den Text baut `hook_block_output` in `retro/traces.rs`, neben `parse_hook_block`: Das Format steht weiter nur dort. Andere Ablehnungen (Abbruch, Argumentfehler, Rechte) bleiben ohne Präfix und sind keine Sperren.
- `parse_hook_block` und `hook_block_output` haben Tests, auch dafür, dass die eine liest, was die andere schreibt.
