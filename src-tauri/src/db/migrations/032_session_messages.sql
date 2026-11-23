-- Nachrichten über Referenzen (ADR 072): Der Agent liest den Inhalt mit `verwalter_read_message`.
CREATE TABLE session_messages (
  id            INTEGER PRIMARY KEY AUTOINCREMENT,
  recipient_id  TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  sender_id     TEXT NOT NULL,
  sender_number INTEGER NOT NULL,
  sender_title  TEXT NOT NULL,
  subject       TEXT NOT NULL,
  text          TEXT NOT NULL,
  created_at    REAL NOT NULL
);
CREATE INDEX session_messages_recipient ON session_messages (recipient_id);
-- Eine wartende Nachricht behält ihren Betreff bis zur Zustellung; `note` ist bei „Gemeldet“ der
-- Inhalt der Referenz (NULL: der ganze Text ist der Inhalt).
ALTER TABLE session_inbox ADD COLUMN subject TEXT;
ALTER TABLE session_inbox ADD COLUMN note TEXT;
