-- „Automatisch fortsetzen“ nach aufgebrauchtem Kontingent je Umsetzung und je Station (ADR 064):
-- Umsetzung 1 an, 0 aus; Station NULL = wie die Umsetzung, 1 an, 0 aus. Bestehende Umsetzungen
-- stehen danach auf „aus“, ihre Stationen auf „wie die Umsetzung“.
ALTER TABLE implementations ADD COLUMN auto_resume INTEGER NOT NULL DEFAULT 0;
ALTER TABLE implementation_stations ADD COLUMN auto_resume INTEGER;
