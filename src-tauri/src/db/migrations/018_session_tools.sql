CREATE TABLE session_inbox (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  session_id    TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  sender_id     TEXT NOT NULL,
  sender_number INTEGER NOT NULL,
  sender_title  TEXT NOT NULL,
  text          TEXT NOT NULL,
  chain         INTEGER NOT NULL,
  created_at    REAL NOT NULL
);
CREATE INDEX session_inbox_session ON session_inbox (session_id, id);
ALTER TABLE projects ADD COLUMN self_release_messages INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN self_release_tldr INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN self_release_open_session INTEGER NOT NULL DEFAULT 0;
ALTER TABLE projects ADD COLUMN self_release_tldr_model TEXT NOT NULL DEFAULT 'haiku';
UPDATE projects SET self_release_messages = auto_handoff, self_release_open_session = auto_handoff;
