-- Station „Nachricht“ (ADR 073, Nachtrag Taiga 133): Empfänger (`master` oder Plan-Kennung einer Station),
-- Vorlage, der zugestellte Text und der Grund eines Fehlers. Eigene Spalten, `prompt`, `command` und `vars`
-- bleiben, was sie sind.
ALTER TABLE implementation_stations ADD COLUMN message_to TEXT;
ALTER TABLE implementation_stations ADD COLUMN message_text TEXT;
ALTER TABLE implementation_stations ADD COLUMN message_sent TEXT;
ALTER TABLE implementation_stations ADD COLUMN message_error TEXT;
