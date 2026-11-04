CREATE TABLE project_groups (
  id         TEXT PRIMARY KEY,
  name       TEXT NOT NULL,
  collapsed  INTEGER NOT NULL DEFAULT 0,
  created_at REAL NOT NULL
);
ALTER TABLE projects ADD COLUMN group_id TEXT REFERENCES project_groups(id) ON DELETE SET NULL;
