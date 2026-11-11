# 067 — Werkzeug-Retro

**Status:** angenommen · **Datum:** 2026-11-11

## Kontext

Die Vorhaben-Retro (ADR 028) kennt Werkzeug-Aufrufe nur als Zählung und als Fehlschläge. Damit lässt sich nicht beantworten, was ein Entwickler nach einer Aufgabe fragt: Was war zu tun, wie kam der Agent ans Ziel, welche Befehle und Skripte haben geholfen oder hätten geholfen, was lässt sich künftig automatisieren.

Probelauf vom 11.11.2026, von Hand mit einem Python-Skript auf den Transkripten eines Vorhabens mit 36 Sessions (80 Nutzer-Aufträge), Haiku mit denselben Optionen wie `print_command`:

- Haiku je Session: 0,6 / 1,5 / 2,1 Cent bei 10 / 25 / 36 Tausend Zeichen Spur, Dauer 36 / 100 / 130 s.
- Haiku fand in drei Sessions übereinstimmend dieselben festen Handgriffe (Kontext zum Arbeitsstart einsammeln, rote Tests gegen einen Basisstand vergleichen, Windows-Ersatz für ein fehlendes Python-Modul bei jedem Testlauf, Abschluss-Ritual im Tracker) und meldete keinen einzelnen Read-, Grep-, Edit- oder Write-Aufruf als Handgriff.
- Ohne Fehlertext in der Spur riet Haiku eine falsche Ursache: 20 von 25 fehlgeschlagenen `git status`-Aufrufen waren Hook-Sperren, Haiku schrieb „kein Git-Repo“. Deshalb trägt die Spur Fehlertext und Sperrgrund.
- Reine Abfolgen-Zählung über Werkzeug-Familien war voller Rauschen (`Grep → Grep → …`).
- Wortgleich doppelte Inline-Skripte über Sessions hinweg: keine von 117. Wiederverwendung zeigt sich am Zweck, den Haiku je Skript benennt, nicht am Text.
- Haiku schlug ein Testlauf-Skript vor, das im Projekt schon lag. Den Abgleich mit vorhandenen Skripten macht der Skill des Nutzers, nicht der Verwalter.

## Optionen

- **(a) Nur zählen** — verworfen: Zahlen allein sagen nicht, welches Rezept hinter einer Wiederholung steckt.
- **(b) Die Sonnet-Mini-Retro erweitern** — verworfen: Sie liest den Verlauf ohne erfolgreiche Aufrufe; die Werkzeug-Retro liest nur die Spur. Eine gemeinsame Anweisung würde beides verwässern und teurer machen.
- **(c) Eigener Haiku-Aufruf auf einer ohne Modell verdichteten Spur** — gewählt.

## Entscheidung

- Der Verwalter verdichtet die Aufrufe jeder Session ohne Modell zu einer Werkzeug-Spur (`spur-<N>.md`: Aufrufe je Nutzer-Auftrag, normalisiert, mit Status, Fehlertext, Sperrgrund und Ausgabegröße) und zu einem Querschnitt über alle Sessions (`werkzeuge.md` Teil A); erkannte Inline-Skripte liegen einmal je Session und Hash in `skripte\`.
- Ein Einmal-Aufruf mit Haiku je Session (`ask_haiku`, direkt nach der Mini-Retro im selben Thread) ordnet die Spur nach Ziel, Weg, festen Handgriffen, Reibung und Zweck der Inline-Skripte (`werkzeuge.md` Teil B). In den Betriebsarten mit LM Studio antwortet wie bei der Mini-Retro das lokale Modell, in Codex der Übersetzer.
- Das Urteil (Skript beim Nutzer, Projekt-Skript, Werkzeug im Verwalter, Hook anpassen) fällt wie bisher im Skill des Nutzers, der den Retro-Ordner liest.
- Kein Schalter in den Einstellungen; die Werkzeug-Retro läuft bei jedem Retro-Lauf mit. Ein Fehlschlag bricht den Lauf nicht ab, zählt nicht in `RetroExport.failed_count` und steht als Zeile in `werkzeuge.md`. `RetroExport` und das Frontend bleiben unverändert.
- Schwellen: wiederkehrend heißt mindestens 3-mal in mindestens 2 Sessions; ein Inline-Skript hat mindestens 10 Zeilen. Grundwerkzeuge (`Read`, `Grep`, `Glob`, `Edit`, `Write`, `MultiEdit`, `NotebookEdit`, `Skill`, `Agent`) zählen nie für sich als Wiederholung.
- Ausgaben werden nur als Länge und Kopf geladen (`tool_details::load_summaries`: `length(output)`, `substr(output, 1, 400)`), nie ganz.
- Keine neuen Tests (Projektkonvention); Beleg je Phase ist die Ausgabe von `retro-probe` gegen eine Kopie der Datenbank.

## Konsequenzen

- Zwei Einmal-Aufrufe je Session statt einem, rund 1–2 Cent mehr.
- Sessions vor ADR 042 haben keine Eingaben in `tool_details`: ihre Spur zeigt nur das Ziel (`target`) des Aufrufs, Skripte fehlen dort.
- Ändert Claude Code das Präfix der Hook-Ausgabe (`PreToolUse:… hook error: …`, ADR 053), erscheinen Sperren in der Spur als Fehler statt als `SPERRE`.
- Die Normalisierung macht Pfade, IDs, Hashes und Zahlen unkenntlich; gleiche Aufrufe verschiedener Sessions werden dadurch zählbar, einzelne Zahlen in Fehlertexten gehen verloren.
- Laufzeit je Session steigt um den Haiku-Aufruf (Probelauf 36–130 s).
