-- Reihenfolge der Vorhaben von Hand (Taiga 101, ADR 019 Nachtrag): kleinere Position steht weiter oben,
-- eine Folge für alle Vorhaben. Der erste Start nach dem Update zeigt die alte Reihenfolge: jüngste
-- Aktivität der Sessions (ohne Sessions der Anlegezeitpunkt), dann `created_at`, wie bisher die Leiste.
ALTER TABLE projects ADD COLUMN position INTEGER NOT NULL DEFAULT 0;
WITH ranked AS (
  SELECT p.id AS id,
         ROW_NUMBER() OVER (
           ORDER BY MAX(p.created_at, COALESCE((SELECT MAX(s.last_activity_at) FROM sessions s
                                                WHERE s.project_id = p.id AND s.archived_at IS NULL), p.created_at)) DESC,
                    p.created_at DESC, p.id
         ) - 1 AS position
  FROM projects p
  WHERE p.archived_at IS NULL
)
UPDATE projects SET position = (SELECT position FROM ranked WHERE ranked.id = projects.id)
WHERE id IN (SELECT id FROM ranked);
