-- Token je Antwort des Modells (ADR 080, Taiga 271): eine Zeile je `message.id`, die spätere Zeile
-- derselben Antwort ersetzt die Zahlen. Die Zeilen fallen mit ihrer Session weg.
CREATE TABLE token_usage (
  session_id TEXT NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
  message_id TEXT NOT NULL,
  at_ms INTEGER NOT NULL,
  model TEXT NOT NULL DEFAULT '',
  input INTEGER NOT NULL DEFAULT 0,
  cache_write INTEGER NOT NULL DEFAULT 0,
  cache_read INTEGER NOT NULL DEFAULT 0,
  output INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (session_id, message_id)
);
CREATE INDEX token_usage_session_at ON token_usage(session_id, at_ms);
-- Der Tempo-Stand „alle Sessions“ liest nur das jüngste Fenster.
CREATE INDEX token_usage_at ON token_usage(at_ms);
