-- Art der Station (ADR 073): `agent`, `user` (durch dich) oder `command` (Befehls-Station). `kind` ist die
-- Quelle; `by_user` bleibt als Spalte stehen, wird aber nicht mehr gelesen oder geschrieben.
ALTER TABLE implementation_stations ADD COLUMN kind TEXT NOT NULL DEFAULT 'agent';
UPDATE implementation_stations SET kind = 'user' WHERE by_user = 1;
-- Schritte als JSON-Liste und Ordner einer Befehls-Station.
ALTER TABLE implementation_stations ADD COLUMN command TEXT;
ALTER TABLE implementation_stations ADD COLUMN cwd TEXT;
-- Werte, die der Vorgänger mit seiner Meldung hinterlässt, als JSON-Objekt.
ALTER TABLE implementation_stations ADD COLUMN vars TEXT;
-- Lauf einer Befehls-Station: laufender Schritt (1-basiert; NULL = kein Schritt läuft), die letzten
-- Zeilen der Ausgabe und der Grund des Fehlers. Ein Neustart der App setzt `step` zurück und den Grund
-- auf „unterbrochen“.
ALTER TABLE implementation_stations ADD COLUMN step INTEGER;
ALTER TABLE implementation_stations ADD COLUMN output TEXT;
ALTER TABLE implementation_stations ADD COLUMN command_error TEXT;
