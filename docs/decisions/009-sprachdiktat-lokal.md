# 009 — Sprachdiktat lokal mit Whisper

**Status:** abgelöst durch [049](049-sprachdiktat-parakeet.md) für die Erkennung; Aufnahme, Pausen-Schnitt und Ein-Aufnahme-Regel gelten weiter · **Datum:** 2026-10-01

## Kontext

Die Eingabeleiste soll Sprache aufnehmen und den erkannten Text in den Entwurf schreiben („Diktieren“), wie es die Claude-Code-Desktop-App zeigt. Gesendet wird nie automatisch. Verwalter startet Claude als Kindprozess ohne dessen interaktive Oberfläche; eine Erkennung muss also vom Verwalter selbst kommen. Sie soll ohne Konto und ohne laufende Internetverbindung funktionieren.

## Optionen

- **(a) Claudes eigene Diktierfunktion.** Sie existiert nur in der interaktiven Terminal-Oberfläche und der VS-Code-Erweiterung. Der Dienst dahinter (`/api/ws/speech_to_text/voice_stream` in `claude.exe`) ist nicht dokumentiert, bräuchte die claude.ai-Anmeldung aus Claudes Anmeldedatei und bricht bei jedem Update still.
- **(b) Web-Spracherkennung (`webkitSpeechRecognition`).** In WebView2 nicht verfügbar.
- **(c) Windows-Spracheingabe (Win+H) per Knopf auslösen.** Cloud, eine fremde Leiste über der App, und Fehler bleiben für Verwalter unsichtbar.
- **(d) Whisper lokal über whisper.cpp** (Crate `whisper-rs`), Aufnahme im Core.

## Entscheidung

**(d).** Modell `ggml-small-q5_1.bin` (190 085 487 Bytes, SHA-256 `ae85e4a935d7a567bd102fe55afc16bb595bdb618e11b2fc7591bc08120411bb`; zuerst war es `ggml-large-v3-turbo-q5_0.bin`, das auf einem Ryzen 5 3600 ohne GPU 257 s für 5 s Audio brauchte und deshalb ersetzt wurde), geladen von einer auf einen festen Stand gepinnten Hugging-Face-Adresse, abgelegt unter `<Benutzerordner>\.verwalter\models\`. Nicht im Installer; der Download startet nur auf Klick „Einrichten“, nie von selbst, und prüft die Prüfsumme, bevor die Datei ihren endgültigen Namen bekommt.

- **Nur CPU.** Keine GPU-Features (`cuda`, `vulkan`): die brauchen CUDA-Toolkit bzw. Vulkan-SDK beim Bau, lokal und in der CI. Ist die Erkennung zu langsam (Abnahme: ein ~10-s-Satz in höchstens 5 s nach dem Stopp), wird Vulkan ein Folgeplan, kein Umbau hier.
- **Aufnahme im Core mit `cpal`** über WASAPI vom Standard-Eingabegerät, nicht im WebView (`getUserMedia`): so gibt es keinen Rechte-Dialog des WebView, und die Tauri-Grenze trägt keine Audiodaten. Die Umrechnung auf 16 kHz mono schreibt eine eigene kleine Funktion (Mittelwert der Kanäle, dann lineare Interpolation) statt einer Resampling-Crate.
- **Sprache einstellbar** (Einstellungen → Spracheingabe): Automatisch (Standard, Whisper erkennt je Abschnitt), Deutsch oder Englisch; gelesen bei jedem Diktatstart. Als Erkennungshilfe gehen die Repository-Namen der Session plus eine feste Wortliste mit.
- **Ein Mikrofon, eine Aufnahme:** app-weit höchstens eine Aufnahme oder Erkennung gleichzeitig. Das Modell bleibt nach dem ersten Diktat bis zum App-Ende im Speicher.

### Text während des Sprechens

Whisper erkennt nur fertige Tonabschnitte, kein laufendes Audio, und verarbeitet immer ein Fenster von bis zu 30 s auf einmal: ein kurzer Abschnitt kostet fast so viel Rechenzeit wie ein langer. Deshalb schneidet der Core die laufende Aufnahme an Sprechpausen (mindestens 600 ms leise) in Abschnitte und erkennt jeden genau einmal in einem eigenen Erkennungs-Thread, während die Aufnahme weiterläuft. Nach jedem Abschnitt geht der bisher erkannte Gesamttext als Ereignis an die Oberfläche; beim Stopp wird nur noch der letzte Abschnitt erkannt. Wer ohne Pause redet, sieht bis zur ersten Pause nichts; daher ein Zwangsschnitt nach 25 s Abschnittslänge. Jeder Abschnitt bekommt die letzten Wörter des bisherigen Textes als Erkennungshilfe mit, damit die Satzgrenzen nicht leiden.

Verworfen: (i) ein gleitendes Fenster (die letzten Sekunden etwa jede Sekunde neu erkennen, wie das `stream`-Beispiel von whisper.cpp) — auf reiner CPU zu teuer, ein Durchlauf pro Sekunde bei mehreren Sekunden Rechenzeit je Durchlauf, und der vorläufige Text würde laufend umgeschrieben; (ii) erst nach dem Stoppen alles erkennen — bis zum Stopp sähe man nichts.

## Konsequenzen

- **Bau-Voraussetzungen:** CMake (whisper.cpp wird bei jedem sauberen Bau übersetzt) und LLVM/libclang (`whisper-rs-sys` erzeugt seine Bindings per bindgen; die mitgelieferten Bindings sind unter Linux erzeugt und scheitern unter Windows an Größenprüfungen). Beides gehört in die Voraussetzungen in [linting.md](../conventions/linting.md).
- **190 MB Download** einmalig, danach ohne Internet; Arbeitsspeicher nicht gemessen (das große Modell brauchte rund 0,8 GB).
- **Abbrechen wartet nicht auf die Erkennung:** whisper.cpp lässt sich mitten im Rechnen nicht sofort anhalten. Der Abbruch gibt die Oberfläche und das Mikrofon sofort frei; eine auslaufende Erkennung sendet nichts mehr und sperrt kein neues Diktat.
- Ein Bau von Grund auf dauert durch die C++-Übersetzung deutlich länger; die CI-Zwischenspeicherung (`rust-cache`) fängt das ab.
