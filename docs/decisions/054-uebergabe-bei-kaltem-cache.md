# 054 — Übergabe bei kaltem Cache

**Status:** angenommen · **Datum:** 2026-11-04

## Kontext

Der Hinweis über dem Eingabefeld ([ADR 033](033-warnhinweise.md)) warnt, wenn der Prompt-Cache einer Session abgelaufen ist, und bietet „In neuer Session weiter“ an. Der Knopf legte bisher nur eine leere Session an, ohne Entwurf; die neue Session startete ohne jeden Stand. Der Cache einer langen Session besteht vor allem aus Werkzeug-Ausgaben, eine Nachricht nach Ablauf schreibt ihn ganz neu (im Hinweis z. B. 384k Token auf Opus). Wer nach Feierabend, Wochenende, leerem Kontingent, Netzausfall oder zugeklapptem Laptop weiterarbeiten will, soll den Verlauf nicht neu in den Cache schreiben müssen.

## Optionen

- **(a) Nebenfrage auf einer Kopie der Session** ([ADR 044](044-nebenfragen.md)) — verworfen: günstig nur bei warmem Cache; bei kaltem schreibt sie den ganzen Verlauf, also genau die Kosten, die der Knopf vermeiden soll.
- **(b) Haiku über den Gesprächstext, wie beim TL;DR** ([ADR 011](011-vorhaben-und-sessions.md)) — gewählt: unabhängig vom Cache der alten Session, das Transkript enthält weder Werkzeug-Ausgaben noch Gedankengang.
- **(c) Nur Auszüge ohne Modell** — als Rückfall gewählt, nicht als Regelfall: erste Nachricht und letzte Beiträge sagen weniger über Entscheidungen und Fundstellen als eine Zusammenfassung.

## Entscheidung

- Der Command `tldr_handoff_brief` baut die Eingabe unter der Session-Sperre (`handoff_brief_input`) und ruft Haiku danach ohne Sperre auf. Antwort ist `HandoffBrief` (Text, ob Haiku zusammengefasst hat, Grund des Rückfalls).
- **Nichts wird gespeichert**, die Übergabe lebt nur im Entwurf der neuen Session. **Entwurf statt Senden**, wie bei der Einstiegszeile ([ADR 027](027-session-uebergabe.md)): der Nutzer liest, bevor er abschickt.
- **Rückfall statt Fehler:** scheitert Haiku (Kontingent leer, kein Netz, Zeitlimit, unlesbare Antwort), entsteht trotzdem eine Session, ihr Entwurf ist der Auszug mit dem Grund im Kopf. Nur eine unbekannte oder verlaufslose Session ist ein Fehler.
- Die Einstiegszeilen der Art Session sind mit [ADR 052](052-session-werkzeuge.md) entfallen, die Übergabe enthält deshalb keine.

## Konsequenzen

- Haiku kennt keine Werkzeug-Ausgaben; die neue Session liest Dateien und Repository-Stand selbst nach, der Schlusssatz der Übergabe sagt es ihr.
- Jeder Klick ist ein Haiku-Aufruf (Eingabe bis rund 107 000 Token an der Obergrenze des Transkripts).
- Bei leerem Kontingent wartet der Klick bis zu `TLDR_TIMEOUT` (90 s) auf den Rückfall, wenn die Kommandozeile nicht sofort ablehnt.
