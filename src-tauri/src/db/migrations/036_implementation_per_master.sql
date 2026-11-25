-- Umsetzungen je Session (ADR 076): Ein Vorhaben kann mehrere unfertige Umsetzungen haben, jede
-- Master-Session führt höchstens eine. Fertig und abgebrochen setzen `finished_at`; danach darf dieselbe
-- Session eine neue anlegen. Der Bestand erfüllt den Index: bisher gab es höchstens eine je Vorhaben.
CREATE UNIQUE INDEX implementations_one_unfinished_per_master
    ON implementations(master_session_id) WHERE finished_at IS NULL;
