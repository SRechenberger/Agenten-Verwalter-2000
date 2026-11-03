# 026 — Artefakte: HTML-Seiten des Agenten im Verwalter zeigen

**Status:** angenommen · **Datum:** 2026-10-06 · ergänzt durch [ADR 047](047-artefakte-alle-dateitypen.md) (alle Dateitypen, Betrachter, anlegende Session)

## Kontext

Berichte, Präsentationen, Diagramme und Design-Entwürfe sind als HTML-Seite am besten lesbar. Der Agent soll sie ablegen, der Verwalter sie als echte, bedienbare Seite zeigen und nach jedem Speichern neu laden. Die Seite ist fremdes HTML mit Skripten im Fenster des Verwalters: sie darf weder dessen Befehle aufrufen noch Dateien außerhalb ihres Ordners lesen.

## Optionen

- Ablage: (a) Ordner `.artefakte` im Workspace + Anweisung an den Agenten, (b) eigenes MCP-Werkzeug „Ansicht zeigen“, (c) jede vom Agenten geschriebene `.html`-Datei.
- Darstellung: (a) `srcdoc` im Rahmen — erbt die CSP der App und findet keine relativ eingebundenen Bilder; (b) Asset-Protokoll von Tauri — Scope ist `$HOME/.verwalter/**`, Workspaces liegen woanders, und es setzt keine eigene CSP; (c) eigenes Tauri-Protokoll mit eigener CSP — zuerst gebaut, verworfen: Tauri 2.12 stuft Seiten eines von der App registrierten Protokolls als „lokal“ ein und gibt ihnen die Rechte der App, und unter Windows laufen die Init-Skripte samt `invoke` und Invoke-Key auch im iframe (Sicherheitsprobe); gehalten hat nur, dass WebView2 `postMessage` aus dem Rahmen nicht weiterreicht; (d) eigener HTTP-Server auf `127.0.0.1` — für Tauri entfernte Herkunft.

## Entscheidung

- **Ablageordner `.artefakte` je Vorhaben** (alle Sessions teilen den Workspace). Ein Artefakt ist jede `.html`/`.htm`-Datei direkt darin; Unterordner tragen Bilder, Skripte und Stylesheets.
- **Artefakt-Server auf `127.0.0.1`** (d) mit Zufalls-Port, `std::net`, keine neue Abhängigkeit: `http://127.0.0.1:<port>/<token>/<session-id>/<pfad>`. Jeder Befehl einer Seite von dort scheitert an der Rechteprüfung von Tauri (entfernte Herkunft, keine Remote-Capability) — unabhängig davon, ob WebView2 die Nachricht durchlässt. Der Zufalls-Token je App-Start hält Seiten im Browser draußen, die Prüfung des `Host`-Kopfes DNS-Rebinding. Ausgeliefert werden nur Dateien, deren kanonischer Pfad unter dem kanonischen Ordner liegt (sonst 403), mit eigener CSP je Antwort.
- **Rahmen mit `sandbox="allow-scripts allow-forms allow-modals"`, ohne `allow-same-origin`**: die Seite läuft mit undurchsichtiger Herkunft und kommt nicht an `parent`.
- **Internet über https erlaubt** (Bibliotheken von CDNs): der Agent, der das HTML schreibt, hat ohnehin eine Shell. `ipc:`, `http://ipc.localhost` und die Ordner anderer Sessions auf dem Server stehen nicht in der CSP.
- **Abfrage alle 2 s statt Dateiwächter**: keine neue Abhängigkeit, erfasst auch per Shell geschriebene Seiten.

## Konsequenzen

- Die App-CSP bekommt `frame-src http://127.0.0.1:*` (Port erst zur Laufzeit bekannt); Links nach außen öffnen sich im Rahmen nicht.
- Adressen der Vorschau gelten nur bis zum nächsten App-Start; die Oberfläche bekommt sie mit jeder Liste neu (`baseUrl`).
- Andere Programme des Benutzers können den Port finden, brauchen aber den Token; Seiten im `.artefakte`-Ordner sind ohnehin für jedes Programm des Benutzers lesbar.
- Formulare schicken nichts ab (`form-action 'none'`), Popups gehen nicht.
- Eine Seite mit `http://`-Quellen (ohne s) lädt diese nicht.
- Karte im Chat nur für `Write` des Hauptagenten; per Shell oder von Subagenten geschriebene Artefakte erscheinen nur im Reiter.
- Die Anweisung an den Agenten steht im selben `--append-system-prompt` wie die Scratchpad-Vorgabe (der autarke Agent liest nur eines).
- Jede HTML-Antwort trägt das Brücken-Skript für Kommentare (ADR 035).
- Die Abschottung belegt die Sicherheitsprobe im Plan (`artifacts/sicherheitsprobe.html`), kein automatisierter Test.
