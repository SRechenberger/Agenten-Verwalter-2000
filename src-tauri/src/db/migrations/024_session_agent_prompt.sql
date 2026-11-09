-- Systemprompt einer Station (ADR 060, Phase 4): Text des Typs, seine Dateien und der Zusatztext der
-- Station, festgehalten beim Start der Station. Jeder spätere Prozessstart und jede Nebenfrage der
-- Session nimmt dieselben Bytes (ADR 044). Bestehende Sessions bekommen NULL: ohne Systemprompt.
ALTER TABLE sessions ADD COLUMN agent_prompt TEXT;
