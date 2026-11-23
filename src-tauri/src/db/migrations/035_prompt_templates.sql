-- Prompt-Vorlagen aus den Einstellungen (ADR 075), global für alle Vorhaben. `name_template` leer =
-- keine Namensvorlage. `position` ist die Reihenfolge der Anlage.
CREATE TABLE prompt_templates (
  id            TEXT PRIMARY KEY,
  name          TEXT NOT NULL,
  text          TEXT NOT NULL,
  name_template TEXT NOT NULL,
  position      INTEGER NOT NULL
);
