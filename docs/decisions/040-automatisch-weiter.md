# 040 — Automatisch weiter und mehrere Einstiegszeilen

**Status:** angenommen · **Datum:** 2026-10-26 · ergänzt durch [ADR 050](050-zurueck-zeile.md) (Zeilenart „Zurück an …“)

## Kontext

[ADR 027](027-session-uebergabe.md) kennt genau eine Einstiegszeile je Antwort und einen Knopf, der die Folgesession mit der Zeile im Entwurf anlegt. Im Zielbild „Entwickeln in Sessions statt mit Unteragenten“ wechselt eine Phase ohne Klick in die nächste Session („die Einstiegszeile wird auf Wunsch automatisch gesendet“). Dazu kommt, dass Antworten mehrere Wege anbieten („Weiter: …“ gleich mehrfach) und ein neues Vorhaben vorschlagen.

## Optionen

- **Wo die Automatik läuft:** (a) in der Oberfläche; (b) im Core. Sessions überleben die Oberfläche, zwei Fenster dürfen nichts doppelt auslösen — also (b).
- **Mehrere „Weiter:“-Zeilen:** (a) anhalten; (b) alle parallel starten. Gewählt (a): Welche Zeile gilt, entscheidet der Mensch.
- **Start:** (a) sofort; (b) Countdown von 10 Sekunden mit Abbrechen und Kettenbremse. Gewählt (b): Eine Schleife zweier Sessions verbrennt sonst unbemerkt das Kontingent.
- **Speicher mehrerer Zeilen:** (a) neue Tabelle oder Spalte; (b) die vorhandene Spalte `sessions.handoff_line`, Zeilen durch Zeilenumbruch getrennt. Gewählt (b): eine Einstiegszeile enthält nie einen Zeilenumbruch, Altbestände lesen sich unverändert.

## Entscheidung

- **Zwei Arten von Zeilen**, im Core erkannt: „Weiter:“ (Art Session) und „Neues Vorhaben<Zusatz bis 80 Zeichen ohne Doppelpunkt>: <Text>“ (Art Vorhaben). Gesucht wird von hinten; Einstiegszeilen zählen nicht gegen das Fenster von fünf anderen Zeilen.
- **Das Modell aus der Zeile** (`Modell sonnet` usw.) erkennt der Core und liefert es je Zeile mit; die Oberfläche erkennt nichts mehr selbst.
- **„In neuem Vorhaben starten“ startet nie selbst.** Der Knopf öffnet „Neues Vorhaben“ mit dem Text hinter dem ersten Doppelpunkt als Auftrag, dem Modell aus der Zeile und den bekannten Repositories, deren Name als ganzes Wort in der Zeile steht. Repositories wählt der Mensch.
- **Schalter „Automatisch weiter“ je Vorhaben** (Standard aus) und die Automatik selbst folgen in den nächsten Phasen des Plans: Haltegründe (🔴, Frage, „Merge“, mehrere „Weiter:“-Zeilen, schon eine neue Session im Vorhaben, Kettenbremse 6, Abbruch), Countdown 10 Sekunden, nur im Speicher.

## Konsequenzen

- Der Haltegrund „Merge“ greift auch bei „Merge-Gate“ in der TL;DR-Zeile; gewollt vorsichtig.
- Nach einem Neustart der App läuft kein Countdown weiter; der Knopf steht wieder da, die Kette beginnt bei 0.
- Die Spalte heißt weiter `handoff_line`, obwohl sie mehrere Zeilen trägt; eine Umbenennung wäre eine Migration ohne Nutzen.
