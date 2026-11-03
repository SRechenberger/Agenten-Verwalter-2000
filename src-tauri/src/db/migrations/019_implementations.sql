CREATE TABLE implementations (
  id TEXT PRIMARY KEY,
  project_id TEXT NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  source TEXT NOT NULL DEFAULT '',
  created_by_session_id TEXT,
  created_at REAL NOT NULL,
  finished_at REAL,
  auto_start INTEGER NOT NULL DEFAULT 1,
  stop_after_current INTEGER NOT NULL DEFAULT 0,
  pending_change TEXT
);
CREATE TABLE implementation_phases (
  id TEXT PRIMARY KEY,
  implementation_id TEXT NOT NULL REFERENCES implementations(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  key TEXT NOT NULL,
  title TEXT NOT NULL,
  description TEXT NOT NULL DEFAULT '',
  wave INTEGER,
  mark TEXT
);
CREATE TABLE implementation_stations (
  id TEXT PRIMARY KEY,
  phase_id TEXT NOT NULL REFERENCES implementation_phases(id) ON DELETE CASCADE,
  position INTEGER NOT NULL,
  stage TEXT NOT NULL,
  model TEXT,
  by_user INTEGER NOT NULL DEFAULT 0,
  skill TEXT,
  brief TEXT NOT NULL DEFAULT '',
  session_id TEXT REFERENCES sessions(id) ON DELETE SET NULL,
  started_at REAL,
  completed_at REAL,
  result TEXT
);
CREATE TABLE implementation_changes (
  id TEXT PRIMARY KEY,
  implementation_id TEXT NOT NULL REFERENCES implementations(id) ON DELETE CASCADE,
  session_id TEXT,
  at REAL NOT NULL,
  summary TEXT NOT NULL,
  after_station_id TEXT,
  stations_before INTEGER NOT NULL,
  stations_after INTEGER NOT NULL
);
CREATE INDEX implementation_stations_session ON implementation_stations(session_id);
ALTER TABLE sessions ADD COLUMN inquiry_of TEXT;
