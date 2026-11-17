-- Master-Session (ADR 069): Eine Station meldet an ihren Master, der Master nimmt an, gibt zurück oder
-- gibt frei. `completed_at` heißt jetzt „angenommen“; `report` ist die Meldung als JSON, bis der Master
-- entschieden hat; `returns` zählt seine Rückgaben, `released_at` die Freigabe einer Station des Benutzers.
ALTER TABLE implementation_stations ADD COLUMN reported_at REAL;
ALTER TABLE implementation_stations ADD COLUMN report TEXT;
ALTER TABLE implementation_stations ADD COLUMN returns INTEGER NOT NULL DEFAULT 0;
ALTER TABLE implementation_stations ADD COLUMN released_at REAL;
ALTER TABLE implementations ADD COLUMN master_session_id TEXT;
UPDATE implementations SET master_session_id = created_by_session_id;
