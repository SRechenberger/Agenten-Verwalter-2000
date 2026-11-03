-- Erste Uhrzeit, zu der eine Session eine Datei schrieb: bestimmt die anlegende Session eines
-- Artefakts (ADR 047). Altbestand bekommt seine letzte Uhrzeit, die früheste, die er noch kennt:
-- ohne Wert verlöre seine Session gegen jede, die die Datei später anfasst.
ALTER TABLE session_files ADD COLUMN first_touched_at REAL;
UPDATE session_files SET first_touched_at = touched_at;
