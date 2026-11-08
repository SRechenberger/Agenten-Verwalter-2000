-- Agententypen aus den Einstellungen (ADR 060). Listen als JSON-Text, Modell und Denkaufwand als
-- serde-Text, beide NULL für „Standard“. `position` ist die Reihenfolge der Anlage.
CREATE TABLE agent_types (
  id          TEXT PRIMARY KEY,
  name        TEXT NOT NULL,
  description TEXT NOT NULL,
  stages      TEXT NOT NULL,
  model       TEXT,
  effort      TEXT,
  prompt      TEXT NOT NULL,
  files       TEXT NOT NULL,
  skills      TEXT NOT NULL,
  position    INTEGER NOT NULL
);
