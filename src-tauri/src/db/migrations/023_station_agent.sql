-- Agententypen im Ablauf (ADR 060): Typ, Denkaufwand und Zusatztext je Station, der Typ beim Start
-- und ein Startfehler. Bestehende Stationen bekommen NULL und `edited = 0`.
ALTER TABLE implementation_stations ADD COLUMN agent TEXT;
ALTER TABLE implementation_stations ADD COLUMN agent_used TEXT;   -- Typname beim Start
ALTER TABLE implementation_stations ADD COLUMN effort TEXT;       -- ausdrücklicher Denkaufwand der Station
ALTER TABLE implementation_stations ADD COLUMN prompt TEXT;       -- Zusatztext der Station
ALTER TABLE implementation_stations ADD COLUMN start_error TEXT;
ALTER TABLE implementation_stations ADD COLUMN edited INTEGER NOT NULL DEFAULT 0;   -- 1 = auf der Karte angepasst (N15); erst Phase 5 schreibt sie
