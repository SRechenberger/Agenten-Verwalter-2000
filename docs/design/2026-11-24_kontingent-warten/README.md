# Warten auf Kontingent: eine Anzeige, kein Platz (Taiga #227)

Entwurf: [`mockup.html`](mockup.html), links der Stand vom 2026-11-24, rechts die Entscheidung des Benutzers.

## Anlass

Am 2026-11-24 war das 5-Stunden-Kontingent von 10:42 bis 13:30 leer. Der Balken zeigte bis dahin „4/4“, ohne dass ein Agent lief: Drei Stationen, die nach 12 bis 18 Sekunden am Kontingent pausiert hatten, und eine gemeldete Station mit pausierter Session hielten die Plätze. Die gemeldete zeigte „gemeldet · pausiert“ mit dem bernsteinfarbenen „Kontingent bis 13:30“ und sah aus wie eine Pause, die auf den Benutzer wartet; die drei anderen zeigten „setzt automatisch fort“ mit „ab 13:30“. Alle vier setzten um 13:30:30 von selbst fort.

## Regel

- **Kein Platz:** Eine am Kontingent pausierte Session belegt keinen Platz. Neue Stationen hält die Warteschlange bis zur Rücksetzzeit der Kontingent-Meldung; ein fälliges Fortsetzen geht vor neue Stationen und höchstens bis zur Grenze durchs Tor.
- **Eine Anzeige**, nach dem gezeigten Zustand der Session:
  - **Scharfer Zeitgeber:** „setzt automatisch fort“, hinter „gemeldet“ und „fertig“ ebenso („gemeldet · setzt automatisch fort“), mit dem neutralen Chip „ab 13:30“, nie dem bernsteinfarbenen.
  - **Ohne Zeitgeber:** „pausiert“ bzw. „pausiert · wartet auf dich“ mit dem bernsteinfarbenen „Kontingent bis 13:30“, denn dann muss jemand handeln.
  - **Vom Halt gehalten:** „wartet auf Kontingent“ (ein Neustart „Fehler · wartet auf Kontingent“) mit dem neutralen „ab 13:30“.
- Orange im Mockup markiert nur, was sich ändert. In der App stehen die Texte und Chips ohne Hervorhebung, im Ton ihrer Nachbarn.

## Entscheidung des Benutzers

Am 2026-11-24 entschieden: „Kein Platz“ und „Eine Regel“ für die Anzeige. Beschluss und Umsetzung: [ADR 068](../../decisions/068-globale-warteschlange.md) und [ADR 064](../../decisions/064-automatisch-fortsetzen.md), jeweils der Nachtrag vom 2026-11-24.
