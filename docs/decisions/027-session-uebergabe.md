# 027 — Session-Übergabe über die Einstiegszeile

**Status:** angenommen · **Datum:** 2026-10-06 · teilweise abgelöst durch [ADR 040](040-automatisch-weiter.md) (mehrere Zeilen, Modell im Core, automatisches Senden); dritte Zeilenart „Zurück an …“ in [ADR 050](050-zurueck-zeile.md)

## Kontext

Die Umsetzungs-Skills beenden eine Phase mit einer Einstiegszeile („Weiter: …“), mit der die nächste Session starten soll. Die Sidebar zeigte eine solche Session als „abgeschlossen“, und der Nutzer kopierte die Zeile von Hand in eine neue Session. Dabei ging auch die Modell-Empfehlung der Zeile (z. B. Sonnet für die Umsetzung) leicht unter.

## Optionen

- **Wo die Zeile erkannt wird:** (a) nur in der Oberfläche; (b) im Core, gemerkt je Session.
- **Was der Knopf tut:** (a) neue Session anlegen und die Zeile sofort senden; (b) anlegen und die Zeile nur in den Entwurf setzen.
- **Wie die Zeile erkannt wird:** (a) eigenes Werkzeug oder Steuerzeichen des Agenten; (b) Textzeile mit „Weiter:“ am Ende der Antwort.

## Entscheidung

**(b), (b) und (b).**

- **Erkennung im Core.** Die Sidebar kennt keine Chat-Einträge; zwei Erkennungen (Rust + TS) liefen auseinander. Der Core prüft jede neue Textantwort und speichert das Ergebnis in `sessions.handoff_line` (Migration 9), gemeldet in `SessionSummary.handoffLine`.
- **Regel:** „Weiter:“ in den letzten fünf nicht leeren Zeilen (Code-Zäune zählen nicht, eine TL;DR-Zeile danach ist erlaubt); seit [ADR 040](040-automatisch-weiter.md), Nachtrag „Einstiegszeilen in der ganzen Antwort“, wird auch weiter oben außerhalb von Zäunen erkannt, die Automatik zählt aber nur das Fenster. Jede neue Textantwort überschreibt den Wert, jede Nutzernachricht löscht ihn. Eigenes Werkzeug oder Steuerzeichen wäre eine Änderung in jedem Skill ohne Mehrwert.
- **Anzeige-Status „Wiedereinstieg“ nur in der Oberfläche,** kein neuer `SessionStatus` im Core: abgeschlossen, Einstiegszeile vorhanden und keine Folgesession im Vorhaben gestartet. Sobald die Folgesession läuft, zeigt die alte wieder den Haken, ohne dass der Core etwas zurücksetzt.
- **Knopf setzt einen Entwurf, sendet nicht.** Vor der ersten Nachricht lassen sich Modell und Denkaufwand ändern — das ist der Sparhebel. Ein vorhandener Entwurf der Ziel-Session bleibt hinter der Zeile erhalten.
- **Modell aus der Zeile:** Nennt sie `Modell <fable|opus|sonnet|haiku>`, stellt der Knopf die neue Session darauf um; Denkaufwand und Modus bleiben übernommen.
- **Knopf nur unter der letzten Textantwort,** solange keine Nutzernachricht danach steht und der Agent nicht arbeitet.

## Konsequenzen

- Gibt ein Skill die Zeile anders aus, bleibt der Haken und kein Knopf erscheint; schlimmstenfalls bleibt es beim Kopieren von Hand.
- Sessions, deren letzte Antwort vor Migration 9 kam, zeigen erst nach ihrer nächsten Antwort das Symbol.
- Die neue Session übernimmt die Werte der Session mit der höchsten Nummer; steht schon eine leere Session im Vorhaben, wird diese wiederverwendet.
