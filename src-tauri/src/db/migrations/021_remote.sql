CREATE TABLE remote_devices (
  id           TEXT PRIMARY KEY,
  name         TEXT NOT NULL,
  token_hash   TEXT NOT NULL,
  created_at   REAL NOT NULL,
  last_seen_at REAL,
  revoked_at   REAL
);
ALTER TABLE projects ADD COLUMN remote_shared INTEGER NOT NULL DEFAULT 0;
CREATE TABLE remote_project_groups (
  project_id TEXT PRIMARY KEY,
  group_id   TEXT NOT NULL REFERENCES project_groups(id)
);
