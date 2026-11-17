# 069 — Zeitpunkt im Chat

**Status:** angenommen, umgesetzt (Story Taiga #106); die Abnahme in der echten App steht aus · **Datum:** 2026-11-15

## Kontext

Nur Nutzernachrichten und Session-Nachrichten speicherten einen Zeitpunkt (`sent_at`). Agententext, Werkzeug-Aufrufe, Gedanken, Rückfragen, Fehler und Hinweise hatten keinen; der Verlauf sagte nicht, wann etwas geschah. Der Benutzer wollte den Zeitpunkt an allen Einträgen und entschied am Mock ([Design](../design/2026-11-15_chat-zeitstempel/README.md)) Format und Lage.

## Entscheidung

- **Feld `at` je Eintrag:** Die acht Varianten von `ChatEntry` ohne Zeit (`Text`, `Thinking`, `Tool`, `Todos`, `Artifact`, `Question`, `Error`, `Notice`) bekommen `at: Option<f64>` (Millisekunden seit 1970, wie `sent_at`, `#[serde(default)]`). `User` und `SessionMessage` behalten `sent_at`; `ChatEntry::at()` und `entryTime` in der Oberfläche lesen beide gleich.
- **Kein Schemawechsel:** Der Eintrag liegt als JSON in `chat_entries.payload`; ein fehlender Schlüssel liest als `None`, ein unbekannter wird ignoriert. Keine Migration, keine neue Protokollnummer der Fernsteuerung.
- **Stempel in `push_entry`:** die einzige Stelle, die den Verlauf verlängert, setzt `at` einmal auf `now_ms()`, wenn der Eintrag keine Zeit trägt; die Bauer schreiben `at: None`. `replace_entry` nimmt die Zeit des ersetzten Eintrags mit und erfindet keine für einen alten. Änderungen an Ort und Stelle (Rückfrage beantworten, Aufruf beenden) lassen `at` unberührt.
- **Uhrzeit links über dem Eintrag:** `HH:MM`, bündig mit dem Inhalt. Nachrichten (Nutzer, Session, Agententext) zeigen sie immer, in einer eigenen Zeile; alles andere erst beim Überfahren oder bei Fokus im Block, schwebend in der Lücke über dem Block, ohne die Höhe zu ändern. Eine Werkzeug-Gruppe zeigt die Zeit ihres ersten Aufrufs.
- **Datum als Tagestrenner im Block-Rahmen:** „Heute, Sonntag 15.11.“, „Gestern, Samstag 14.11.“, sonst „Donnerstag 12.11.“ (mit Jahr in einem anderen Jahr), wenn der Tag gegenüber dem letzten Block mit Zeit wechselt. Der erste Block mit Zeit bekommt nur dann einen, wenn der Verlauf dort beginnt (kein älterer mehr nachzuladen ist): Sonst entfiele der Trenner beim Nachladen, wenn der Tag darüber weitergeht, und die Ansicht spränge um seine Höhe. Der Tooltip der Uhrzeit nennt den vollen Zeitpunkt samt Sekunden. Einträge von vor dem Feld zeigen nichts, bekommen keinen Trenner und unterbrechen keinen Tag. Uhrzeit und Trenner sind nicht markierbar.

## Verworfen

- **Eine Spalte in `chat_entries`:** zwei Wahrheiten (Spalte und Payload) und eine Migration für etwas, das der Payload ohnehin trägt.
- **Zeiten alter Einträge aus Transkripten schätzen:** Das wäre eine Angabe, die nicht stimmt; alte Einträge zeigen nichts.
- **Das Datum klein über der Uhrzeit im rechten Rand:** die erste Wahl, vom Benutzer am Mock durch Uhrzeit links und Tagestrenner ersetzt.
- **Der Tagestrenner als neuer Block-Typ der virtualisierten Liste:** verschöbe Schlüssel und Indizes (`buildBlocks`, Sprung zu einem Eintrag, Messung der Höhen); im Block-Rahmen bleibt die Liste, wie sie ist.

## Folgen

- Einträge von vor der Änderung bleiben ohne Zeit; erst neue tragen sie.
- Nachrichten werden eine Zeile höher: ein Block mit Agententext um 18 px, ein Nutzerblock um 14 px (sein Außenabstand fällt mit dem Abstand unter der Zeit zusammen).
- Ein Gast sieht die Zeiten des Hosts in seiner Ortszeit.
- „Heute“ und „Gestern“ im Tagestrenner gelten für den Zeitpunkt des Öffnens; ein Verlauf, der über Mitternacht offen bleibt, zeigt weiter „Heute“, bis er neu geöffnet wird. Es gibt keinen Zeitgeber.
- Solange älterer Verlauf nachzuladen ist, hat der älteste geladene Block keinen Tagestrenner (`dayDividers(times, now, atStartOfHistory)`, in `ChatTimeline` `!hasMore`). Rest: Wechselt der Tag genau an der Seitengrenze, bekommt der bisher älteste Block beim Nachladen einen Trenner und wird einmalig eine Zeile höher; ebenso der erste Block, wenn der Anfang des Verlaufs erreicht ist.
- Ein Rückfall der Kalendertage auf UTC fällt im Selbsttest nur außerhalb einer UTC-Zeitzone auf, und die CI läuft in UTC: bekannte Grenze. Eine Zeitzone als Parameter wäre eine API-Änderung für einen Fall, den die CI nicht braucht.
- Geprüft: `agents::event::` und `sessions::registry::tests::` (Rust, in der CI), `entryTimeText.selftest.ts` (`pnpm selftest`), Bildprüfung gegen den Mock.
