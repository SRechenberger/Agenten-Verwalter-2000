-- Eigene Nachrichten warten im Posteingang (Taiga-Story 190): `entry_seq` ist der Eintrag `user` im
-- Chat, den die Freigabe sendet; NULL bei Nachrichten anderer Sessions.
ALTER TABLE session_inbox ADD COLUMN entry_seq INTEGER;
