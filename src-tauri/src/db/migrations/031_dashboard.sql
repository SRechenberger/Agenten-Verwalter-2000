-- Dashboard (ADR 071): Zeitpunkt und Auszug der letzten Antwort eines Zugs, ihre Einordnung durch das
-- Modell und der Zeitpunkt von „Gelesen“. Alle Zeitpunkte in ms seit der Epoche wie `last_activity_at`.
ALTER TABLE sessions ADD COLUMN answer_at REAL;
ALTER TABLE sessions ADD COLUMN answer_excerpt TEXT;
ALTER TABLE sessions ADD COLUMN answer_kind TEXT;
ALTER TABLE sessions ADD COLUMN answer_line TEXT;
ALTER TABLE sessions ADD COLUMN card_dismissed_at REAL;
-- Bestand: Nur eine abgegebene, ungelesene Session ohne Station bekommt eine Antwort-Karte.
UPDATE sessions SET answer_at = last_activity_at
WHERE status = 'completed'
  AND last_activity_at > seen_at
  AND NOT EXISTS (SELECT 1 FROM implementation_stations WHERE implementation_stations.session_id = sessions.id);
