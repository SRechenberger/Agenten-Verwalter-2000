# 057 — Einstellungen in Kategorien und mit Suche

**Status:** angenommen · **Datum:** 2026-11-05

## Kontext

Die Einstellungsseite ist ein langer Scroll mit acht Abschnitten; der neunte (Umgebung) kommt. Sie soll nicht flach weiterwachsen: vier Kategorien in einer Leiste links, rechts nur die Abschnitte der gewählten Kategorie, später ein Suchfeld über alle Zeilen.

## Optionen

- **Gliederung:** (a) vier Kategorien links; (b) ein Scroll mit Sprungmarken; (c) Reiter oben.
- **Rückgrat:** (i) ein Katalog mit Name, ⓘ-Text, Kategorie und Abschnitt je Zeile plus ein Renderer; (ii) Suche über den Text der gezeichneten Zeilen; (iii) jede Zeile bringt ihren Suchtext selbst mit.
- **Einhängen:** (A) nur die gewählte Kategorie; (B) alles eingehängt, der Rest ausgeblendet.

## Entscheidung

- **(a) Vier Kategorien links** (User 2026-11-04). (b) wächst weiter; (c) hat keinen Platz für Untertitel, und die Leiste links ist das Muster der App.
- **(i) Katalog** (`settingsCatalog.ts`) als einzige Stelle für Zeilenname, ⓘ-Text, Kategorie und Abschnitt. Ein `switch` über die Id in `renderRow` zeichnet die Bedienelemente; der Compiler meldet eine fehlende Id. (ii) verworfen: Zeilen nicht gewählter Kategorien stünden nicht im DOM. (iii) verworfen: vier Zeilen sind schon Komponenten mit eigenem Zustand, die Texte blieben über fünf Dateien verstreut, und die Suche müsste jede Zeile erst einhängen, um ihren Text zu kennen.
- **(B) Alles eingehängt, der Rest `hidden`.** (A) verworfen: Konto, Version und Claude-Version fragen beim Einhängen Programme und das Netz ab (bis 15 s); jeder Wechsel löste das neu aus.
- **Merken** der zuletzt gewählten Kategorie nur im Zustand-Store (`stores/settingsNav.ts`), nicht in der Datenbank: eine Ansichtsvorliebe, deren Verlust beim Neustart nicht weh tut.
- **Punkte an Kategorien** spiegeln den Punkt am Knopf „Einstellungen“: Update des Verwalters → „App“. Beim Öffnen gilt die erste Kategorie mit Punkt.

## Suche

- Der Suchtext wird getrimmt und an Leerzeichen in Wörter geteilt; jedes Wort muss als Teilstring im Zeilennamen **oder** im ⓘ-Text vorkommen (UND), Groß und klein egal. Abschnittsnamen, Pfade und Zahlen in den Bedienelementen zählen nicht; durchsucht werden nur sichtbare Zeilen. Gefiltert wird bei jedem Tastendruck.
- Die Treffer sind die echten Zeilen, gruppiert nach Kategorie und Abschnitt; nicht getroffene Zeilen und Gruppen sind `hidden`. Die Leiste zeigt je Kategorie die Trefferzahl, ohne Treffer gedimmt. Ein Klick auf eine Kategorie leert die Suche und öffnet sie.
- Treffer im Zeilennamen sind markiert. Fehlt ein Suchwort im Namen, zeigt die Zeile „Im Hilfetext: …“: der erste Satz des ⓘ-Textes mit einem fehlenden Wort; ist er länger als 140 Zeichen, 60 Zeichen vor und nach dem ersten Treffer, an Wortgrenzen geschnitten, mit „…“ an den abgeschnittenen Enden.
- **Verworfen:** Treffer als Kopien der Zeilen. Das wäre eine zweite Instanz je Zeile mit doppelten Abfragen (Konto, Version) und eigenem Zustand.
- Die Suchwörter erreichen `SettingRow` über einen Context; die Suchfunktionen sind rein (`settingsSearch.ts`).

## Konsequenzen

- Eine neue Zeile ist ein Katalogeintrag plus ein Zweig in `renderRow`; ein neuer Abschnitt landet in seiner Kategorie. Spätere Pläne bauen keinen Abschnitt hinter die Hülle.
- Ausgeblendete Zeilen bleiben im DOM; ihre Menüs (Portal in `document.body`) werden beim Wechsel der Kategorie geschlossen.
- Die Suche arbeitet auf dem Katalog, nicht auf dem DOM.
