-- Erste Uhrzeit, zu der eine Session eine Datei schrieb: bestimmt die anlegende Session eines
-- Artefakts (ADR 047). Altbestand bleibt NULL.
ALTER TABLE session_files ADD COLUMN first_touched_at REAL;
