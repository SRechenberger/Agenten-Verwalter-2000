-- Globale Warteschlange (ADR 068): Priorität je Vorhaben; „Parallel max.“ je Umsetzung entfällt.
ALTER TABLE projects ADD COLUMN priority TEXT NOT NULL DEFAULT 'normal';
ALTER TABLE implementations DROP COLUMN max_parallel;
