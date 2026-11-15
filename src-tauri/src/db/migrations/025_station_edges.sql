-- Constraint-Abläufe (ADR 061): Plan-Kennung, Vorgänger (JSON-Liste von Plan-Kennungen) und Vorrang
-- je Station, Grenze gleichzeitig aktiver Stationen je Umsetzung (NULL = unbegrenzt). Kennungen und
-- Kanten der bestehenden Stationen setzt danach `db::implementations::backfill_edges` aus Position
-- und Welle (Regel R1), damit bestehende Umsetzungen unverändert weiterlaufen.
ALTER TABLE implementation_stations ADD COLUMN plan_id TEXT NOT NULL DEFAULT '';
ALTER TABLE implementation_stations ADD COLUMN after_ids TEXT NOT NULL DEFAULT '[]';
ALTER TABLE implementation_stations ADD COLUMN priority INTEGER NOT NULL DEFAULT 0;
ALTER TABLE implementations ADD COLUMN max_parallel INTEGER;
