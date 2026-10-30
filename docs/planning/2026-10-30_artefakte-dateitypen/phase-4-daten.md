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

- [ ] Jeder der sechs Typen zeigt seinen Betrachter; ohne Internet (YAML, CSV) den Rohtext.
- [ ] JSON-Baum: Aufklappen, Suche, „Pfad kopieren“ liefert `$.…`; kaputtes JSON zeigt die Fehlerzeile.
- [ ] `.env`-Werte sind verdeckt, bis sie angeklickt werden.
- [ ] CSV mit `;` wird richtig in Spalten zerlegt; Sortieren und Filtern funktionieren.
- [ ] Kommentar auf einen JSON-Knoten bleibt nach „Neu laden“ am selben Knoten.

## Checkliste

- [ ] `viewers/tree.js`, `viewers/json.js` (Json, JsonLines), `viewers/yaml.js`, `viewers/config.js`, `viewers/xml.js`, `viewers/csv.js`; `tree.js` wird in `mod.rs` bei Json, JsonLines, Yaml, Xml vor das Typ-Skript gehängt.
- [ ] Weiche `page` um die sechs Typen ergänzen.
- [ ] Probe-Dateien in `artifacts/proben/`: `beispiel.json`, `kaputt.json`, `beispiel.jsonl`, `beispiel.yaml` (zwei Dokumente), `beispiel.toml`, `beispiel.env`, `beispiel.xml`, `komma.csv`, `semikolon.csv`.

## Definition of Done

- Screenshots je Betrachter und der Fehlerfälle in `artifacts/phase-4-*.png`; Nachweis „Pfad kopieren“ im Report-Back.
- Sichtbares Ergebnis: Daten-Ausgaben des Agenten sind im Reiter durchsuchbar statt als Textwand.
- `pnpm check` grün; Seven-Goals-Check.

## Report-Back
