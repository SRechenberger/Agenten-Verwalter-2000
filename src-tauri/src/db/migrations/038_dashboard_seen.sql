-- Dashboard als Postfach: wann der Nutzer eine Karte zuletzt gesehen hat. `card_id` ist die Kennung der
-- Karte (`<Art>:<Session|Umsetzung>[:<Station|Anfrage>]`), `seen_at` in ms seit der Epoche wie
-- `last_activity_at`. Ohne Fremdschlüssel: Karten sind abgeleitet; verschwundene Zeilen räumt `dashboard_get` ab.
CREATE TABLE dashboard_seen (
    card_id TEXT PRIMARY KEY,
    seen_at REAL NOT NULL
);
