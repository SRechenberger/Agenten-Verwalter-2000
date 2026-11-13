-- Station konfigurierbar (ADR 061): „Automatisch starten“ je Station (NULL = wie die Umsetzung,
-- 1 an, 0 aus), „Bei Fehler“, „Vorgänger dürfen“, die Änderungen am Auftrag mit Quelle (JSON-Liste)
-- und die Zahl der Neustarts nach einem Fehler. Bestehende Stationen verhalten sich wie vorher.
ALTER TABLE implementation_stations ADD COLUMN auto_start INTEGER;
ALTER TABLE implementation_stations ADD COLUMN on_error TEXT NOT NULL DEFAULT 'halt';
ALTER TABLE implementation_stations ADD COLUMN predecessors_may TEXT NOT NULL DEFAULT 'append';
ALTER TABLE implementation_stations ADD COLUMN brief_changes TEXT NOT NULL DEFAULT '[]';
ALTER TABLE implementation_stations ADD COLUMN restarts INTEGER NOT NULL DEFAULT 0;
