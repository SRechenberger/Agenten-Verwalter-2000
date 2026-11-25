-- Freigabe je Gruppe (ADR 059, Nachtrag): Jedes Vorhaben einer freigegebenen Gruppe ist für Gäste
-- freigegeben, auch ein später hinzugekommenes. `projects.remote_shared` bleibt die eigene Freigabe.
ALTER TABLE project_groups ADD COLUMN remote_shared INTEGER NOT NULL DEFAULT 0;
