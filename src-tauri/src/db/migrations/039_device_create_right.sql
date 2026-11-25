-- Recht je Gerät, Vorhaben beim Host anzulegen (ADR 059, Nachtrag „Gast legt Vorhaben an“): Standard
-- ist „nicht erlaubt“. `create_group` ist die Gruppe des Hosts, in der die Vorhaben des Geräts
-- landen; fällt die Gruppe weg, landen sie ohne Gruppe.
ALTER TABLE remote_devices ADD COLUMN create_allowed INTEGER NOT NULL DEFAULT 0;
ALTER TABLE remote_devices ADD COLUMN create_group TEXT REFERENCES project_groups(id) ON DELETE SET NULL;
