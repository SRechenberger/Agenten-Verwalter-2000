# Gast legt mit Erlaubnis Vorhaben auf dem Host an (Taiga 193)

Entwurf zum Nachtrag „Gast legt Vorhaben an“ von [ADR 059](../../decisions/059-remote-sessions.md); Migration 39 (`remote_devices.create_allowed`, `create_group`). `mockup.html` ist der eigenständige Prototyp (er zeigt auch den Entwurf der Gruppenfreigabe; für diese Lane zählen die Rolle Host mit „Neue Vorhaben“ in „Gekoppelte Geräte“ und die Rolle Gast mit „Anlegen auf“ auf der Seite „Neues Vorhaben“).

## Entscheidungen

- **A1:** Das Recht gilt je Gerät. In „Gekoppelte Geräte“ wählt der Host je Gerät „Neue Vorhaben: nicht erlaubt / erlaubt, ohne Gruppe / erlaubt, in „<Gruppe>““. Standard und Stand nach der Migration ist „nicht erlaubt“. Fällt die Gruppe weg, bleibt das Recht „erlaubt, ohne Gruppe“.
- **A2:** Das Vorhaben des Gastes ist sofort freigegeben (über die Gruppe, wenn sie freigegeben ist, sonst einzeln) und vom anlegenden Gerät übernommen. Herauswerfen geht jederzeit.
- **A3:** Der Gast wählt nur aus den Repositories des Hosts (`remote_repository_list`). Es gibt kein `repository_add` und keinen Ordnerdialog.
- **A4:** Erlaubt sind Auftrag, Anhänge (erst `attachment_add_bytes`, dann `remote_project_create`), Modell und Denkaufwand aus dem Profil des Hosts. Vorlagen des Gastes füllen nur den Text; es gibt kein /-Menü mit Skills des Hosts.
- **A5:** Beim Host landet das Vorhaben in der Gruppe aus A1, beim Gast in der lokalen Gruppe, die er im Dialog wählt (`remote_project_groups`).
- **A6:** Es gibt keine Grenze; die Warteschlange begrenzt die Agenten ohnehin.
- **A7:** Archivieren und Löschen bleiben Klasse D, auch für das Vorhaben, das der Gast angelegt hat.
- **A8:** Oben auf „Neues Vorhaben“ steht „Anlegen auf: Dieser Rechner / <Host>“, nur mit Hosts, die es dem Gerät erlauben. Ein verbundener Host ohne Recht steht in einem Hinweis.

## Umsetzung

- Die Prüfung steht im Kern des Hosts (`remote/dispatch.rs`, `create_right_of`), nie in der Oberfläche: ohne Recht, auch für ein gesperrtes Gerät, lehnt der Host `remote_project_create` und `remote_repository_list` ab. Das Recht liest `Host::create_right` aus der Datenbank; ein Host, der es nicht lesen kann, erlaubt nichts.
- `remote_project_create(hostId, …)` und `remote_repository_list(hostId)` sind Klasse G am Gast; der Host beantwortet die Namen vor der Prüfung wie `remote_take`. `remote_device_set_create` (Klasse E) setzt das Recht; ein Gast darf es nie setzen.
- Der Host setzt die Gruppe aus dem Recht ein (`groupId` des Gastes wird überschrieben), prüft jedes Repository gegen seine Liste, legt über `project_create` an, gibt frei (`SessionRegistry::share_created`), übernimmt für das Gerät und meldet `project://changed`.
- `HostProfile.canCreate` (`#[serde(default)]`) sagt dem Gast, ob er anlegen darf; die Oberfläche zeigt „Anlegen auf“ nur dann.
- Selbsttest `remote-probe --selftest`: „anlegen ohne recht“ und „anlegen mit recht“.
