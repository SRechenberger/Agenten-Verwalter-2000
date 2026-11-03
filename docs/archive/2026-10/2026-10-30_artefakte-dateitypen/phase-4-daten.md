# Phase 4 — Strukturierte Daten

**Rating:** standard · **Wave:** 4 · **Aufwand:** M

Betrachter für JSON, JSON Lines, YAML, TOML/INI/.env, XML und CSV/TSV.

## Kontext — vor dem Start lesen

- `README.md` dieses Plans, **Kontrakt → Artefakte**; `docs/decisions/047-artefakte-alle-dateitypen.md`.
- `src-tauri/src/artifacts/viewers/` — `mod.rs`, `shell.html`, `common.js` (`loadScript`, `rawFallback`, `lines`, `capText`, `languageOf`), `text.js` als Vorbild.
- Fehlerklassen geprüft: keine Vault-Entities einschlägig.

## Entscheidungen

- `viewers/tree.js` (gemeinsam für JSON, YAML, JSON-Lines-Datensatz, XML): rendert einen Wert als aufklappbaren Baum; je Knoten `id="n-<pfad>"` mit Pfad in JSONPath-Schreibweise (`$.typen[0].endung`, Sonderzeichen URL-kodiert) als Kommentar-Anker; Typfarben (Schlüssel Akzent, Text grün, Zahl/Bool gelb, `null` gedämpft); Listen zeigen `[ n ]`; Suchfeld filtert Knoten nach Schlüssel oder Wert und klappt Treffer auf; Knopf „Pfad kopieren“ je Knoten (`navigator.clipboard`, bei Fehler Pfad markiert anzeigen). Ebene 1 und 2 offen, tiefer zu.
- JSON: `JSON.parse`; Fehler → Rohtext mit markierter Zeile aus der Fehlermeldung. Umschalter „Baum | Rohtext“ (Rohtext formatiert mit 2 Leerzeichen, mit Syntaxfarben).
- JSON Lines: Tabelle mit einer Zeile je Datensatz; Spalten = Vereinigung der Schlüssel der ersten 200 Datensätze; Klick auf eine Zeile öffnet den Datensatz als Baum darunter; fehlerhafte Zeilen rot mit Fehlermeldung.
- YAML: `js-yaml@4.1.0/dist/js-yaml.min.js` mit `loadAll`; mehrere Dokumente als Abschnitte; Umschalter „Quelltext | Baum“, Standard Quelltext (mit Syntaxfarben und Einklappen nach Einrückung).
- TOML/INI/.env: Quelltext mit Syntaxfarben (highlight.js `ini`), Abschnitte `[name]` als Sprungmarken oben; bei `.env*` Werte hinter `=` verdeckt (`••••`), Aufdecken je Zeile per Klick.
- XML: `DOMParser`; Fehler → Rohtext; Baum der Elemente mit Attributen; Umschalter „Baum | Quelltext“.
- CSV/TSV: `papaparse@5.4.1/papaparse.min.js` mit Trennzeichen-Erkennung (`,` `;` Tab); feste Kopfzeile, Sortieren per Klick auf Spaltenkopf (Zahlen numerisch), Filterfeld über alle Spalten, Zahlen rechtsbündig; ab 5000 Zeilen nur die ersten 5000 plus Hinweis; Zeilen-`id="r<n>"` als Kommentar-Anker.

## Abnahmekriterien

- [x] Jeder der sechs Typen zeigt seinen Betrachter; ohne Internet (YAML, CSV) den Rohtext. (Prüfstand Edge headless, CDN gesperrt: YAML Quelltext, CSV Rohtext)
- [x] JSON-Baum: Aufklappen, Suche; kaputtes JSON zeigt die Fehlerzeile. „Pfad kopieren“ liefert `$.…` — nur der Rückfall (Pfad zum Markieren) belegt, der Erfolgsweg nur in der App prüfbar.
- [x] `.env`-Werte sind verdeckt, bis sie angeklickt werden.
- [x] CSV mit `;` wird richtig in Spalten zerlegt; Sortieren und Filtern funktionieren (Sortieren belegt, Filter nur gebaut).
- [ ] Kommentar auf einen JSON-Knoten bleibt nach „Neu laden“ am selben Knoten. (Zeilen-IDs sind gültige, stabile Selektoren; die Brücke fehlt im Prüfstand — in der App prüfen.)

## Checkliste

- [x] `viewers/tree.js`, `viewers/json.js` (Json, JsonLines), `viewers/yaml.js`, `viewers/config.js`, `viewers/xml.js`, `viewers/csv.js`; `tree.js` wird in `mod.rs` bei Json, JsonLines, Yaml, Xml vor das Typ-Skript gehängt.
- [x] Weiche `page` um die sechs Typen ergänzen.
- [x] Probe-Dateien in `artifacts/proben/`: `beispiel.json`, `kaputt.json`, `beispiel.jsonl`, `beispiel.yaml` (zwei Dokumente), `beispiel.toml`, `beispiel.env`, `beispiel.xml`, `komma.csv`, `semikolon.csv`; dazu `kaputt.xml`.

## Definition of Done

- [x] Screenshots je Betrachter und der Fehlerfälle in `artifacts/phase-4-*.png` (Edge headless, dunkel); Nachweis „Pfad kopieren“ siehe oben.
- [x] Sichtbares Ergebnis: Daten-Ausgaben des Agenten sind im Reiter durchsuchbar statt als Textwand.
- [x] `pnpm check` grün; Seven-Goals-Check.

## Report-Back

**Stand:** umgesetzt, im Browser-Prüfstand belegt, in der App nicht gelaufen.

**Entscheidungen / Abweichungen:**
- Zeilen-ID eines Baumknotens ist nicht der JSONPath selbst, sondern `n<prefix>-<pfad>` mit allen Zeichen außer `A–Z a–z 0–9 -` als `_<hex>_`: die Brücke findet Kommentare per `#id` wieder, und ein ID mit `$ . [ ]` ist kein gültiger Selektor (querySelector wirft). Der echte Pfad steht in `data-path` und ist das, was „Pfad kopieren“ liefert.
- JSON-Fehlerzeile: Edge nennt in der Meldung keine Stelle mehr, nur einen Textausschnitt. `json.js` sucht die Stelle selbst (Halbierung über die Länge des gültigen Anfangs). Die Plan-Annahme „Zeile aus der Fehlermeldung“ trägt nicht.
- YAML mit `CORE_SCHEMA` (Zeitstempel bleiben Text, kein Date/Binary im Baum).
- Rohtext von JSON ist neu formatiert (2 Leerzeichen); seine Zeilennummern passen nicht zur Datei.
- `.env`: die echten Werte liegen nur in einem JS-Feld, nicht im DOM — ein Kommentar auf die Zeile trägt sie nicht weiter.
- XML: gemischter Inhalt in Dokumentreihenfolge als `#text`-Knoten; Pfade im XPath-Stil.

**Offen / unsicherste Stelle:** `tree.js` `copyPath` — Erfolgsweg (`execCommand`/`navigator.clipboard`) im Prüfstand nicht auslösbar (headless: beides abgelehnt), nur der Rückfall belegt. In der App prüfen, ob der Rahmen das Kopieren zulässt.

**Prüfstand:** Edge headless gegen einen Mini-Server, der `viewers::render` nachbaut; `pnpm check` grün (Rust hat keine Tests).
