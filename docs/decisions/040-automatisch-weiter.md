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

## Nachtrag 2026-10-31: Knopf unter der Zeile

- Der Knopf einer Einstiegszeile steht im Antworttext direkt unter seiner Zeile (`HandoffAnswer`), nicht mehr in einer gemeinsamen Leiste. Die UI teilt den Text hinter jeder Zeile, die sie im Text findet: von hinten zugeordnet über die bereinigte Zeile (wie `clean` im Core), eine Zeile in einem offenen Code-Zaun zählt nicht. Core, Datenbank und Erkennung (letzte fünf nicht leere Zeilen) bleiben unverändert.
- Eine Zeile ohne Stelle im Text behält ihre Reihe in der Leiste unter der Antwort, mit Art und Auszug, wenn es mehrere gibt; so geht nie ein Knopf verloren. Countdown, Haltegrund, Fehler und Schalter stehen einmal je Antwort in der Leiste. Die Regeln „Countdown ersetzt die Weiter- und Zurück-Knöpfe“ und „Zurück an ohne Eltern-Session: keine Reihe“ gelten für beide Orte.
- Verworfen: die Knöpfe per rehype nach dem umgebenden Block einfügen — Zeilen im selben Absatz bekämen ihre Knöpfe gemeinsam darunter.
- Folge: Eine „Weiter:“-Zeile im selben Absatz wird ein eigener Absatz; eine Nadel für „ohne Fundstelle“, die über eine Teilung hinwegreicht, wird nicht mehr markiert.
- Jede zugeordnete Zeile ist ein eigener Absatz: die Teilung schneidet auch vor der Zeile, damit sie nicht mit Text davor oder einer anderen Einstiegszeile verschmilzt. Andere Zeilenumbrüche bleiben, wie sie sind.
- Der Knopf einer „Weiter:“- und einer „Neues Vorhaben“-Zeile wird grau, sobald die neue Session bzw. der Dialog geöffnet ist (`stores/handoffOpened.ts`, Schlüssel Session plus Zeilentext); beim Anlegen und bei einem Fehler bleibt er orange, ein weiterer Klick legt wieder eine Session an. Der Zustand liegt nur im Speicher: nach Neuladen oder Neustart ist der Knopf wieder orange. Verworfen: je Session und Zeile speichern — das braucht eine Spalte und einen Befehl und lässt sich später nachrüsten.
