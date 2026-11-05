# 059 — Remote Sessions: Vorhaben einer anderen laufenden Instanz steuern

**Status:** angenommen · **Datum:** 2026-11-05

## Kontext

Dieselbe Person arbeitet an einem anderen Rechner weiter: Eine Instanz übernimmt über das Netz die Kontrolle über Vorhaben einer anderen laufenden Instanz. Vorgaben: Gearbeitet wird nur am Host, der Gast übernimmt nur die Kontrolle. Ein Vorhaben steht exklusiv dem Host oder einem Gast zur Verfügung. Der Host kann Gäste immer herauswerfen. Windows als Gast und Linux als Host müssen laufen.

Vor dieser Entscheidung schloss `docs/PROJECT.md` einen Server aus. Die Oberfläche spricht nur über Tauri mit dem Core, und die CSP erlaubt ihr kein Netz.

## Entscheidungen

| | Entscheidung | Verworfen |
|---|---|---|
| **E1** | Ausführung nur am Host: Agent-Prozesse, Workspace, Repositories, Git, Konto und Kontingent bleiben dort. Die Datenbank des Hosts ist die einzige Quelle, der Gast speichert nur Verbindungsdaten und seine lokale Gruppe. | Verlauf oder Vorhaben zum Gast kopieren: zwei Schreiber auf einem Verlauf. |
| **E2** | Verbindung von Core zu Core: Oberfläche des Gastes → Core des Gastes → Netz → Core des Hosts. Die CSP bleibt, das Token kommt nie in die Oberfläche. | Oberfläche direkt zum Host; Host liefert die Oberfläche an einen Browser; Bildschirmfreigabe. |
| **E3** | Eine Weiche im Core des Gastes, als Wrapper um `generate_handler!`, entscheidet an der ID, ob ein Aufruf lokal bleibt. Ereignisse des Hosts kommen unter denselben Namen an, die Dateien mit `invoke` und `listen` bleiben unverändert (Z3). | Modul `src/lib/ipc.ts`; Remote-Aufrufe je Feature; eine Weiche in jedem Command. |
| **E4** | Erlaubnisliste am Host: Aus dem Netz erreichbar ist nur, was in der Tabelle in `remote/access.rs` steht. Der Server ruft Befehle über `on_message` auf, also dieselben Commands mit denselben Antworten. Ein neuer Command ist unerreichbar, bis ihn jemand einträgt; ein Test über `lib.rs` wird rot, solange seine Klasse fehlt. | Alle Commands durchreichen und einzelne sperren. |
| **E5** | Ein Steuerrecht je Vorhaben, geprüft im Core des Hosts, auch für seine eigene Oberfläche (Abschnitt „Steuerrecht“). Automatik im Vorhaben läuft weiter: Selbstfreigabe, Inbox, Stationen. | Sperre nur in der Oberfläche. |
| **E6** | Befehle in Klassen (Abschnitt „Klassen“). | |
| **E7** | Transport: Je Paar aus Gast und Host eine dauerhafte TLS-Verbindung, darin je Zeile ein JSON-Objekt (Z4). Neue direkte Abhängigkeiten nur `rustls` und `rcgen`, beide mit `ring`, weil `aws-lc-rs` unter Windows CMake verlangen kann. | WebSocket; HTTP mit Polling; gRPC; unverschlüsselt hinter VPN oder Tunnel. |
| **E8** | Kopplung: Der Server ist standardmäßig aus. Eine Einladung trägt Adresse, Port, Zertifikats-Fingerabdruck und Einmal-Geheimnis und verfällt. Danach hat der Gast ein Geräte-Token, der Host speichert nur dessen Hash, zeigt die Geräte und kann sie sperren. | Passwort; offener Zugang im LAN; Anmeldung über einen fremden Dienst. |
| **E9** | Steuerrecht ist eine Shell am Host: Wer steuert, führt über den Agenten Befehle mit den Rechten des Host-Nutzers aus, und die Workspace-Grenze begrenzt das nicht verlässlich. Deshalb gibt der Host jedes Vorhaben ausdrücklich frei, der Gast sieht nur Freigegebenes, und Pfade aus dem Netz prüft der Core wie die aus der Oberfläche. Für dieselbe Person hingenommen; bekommt je eine andere Person Zugang, ist das neu zu entscheiden. | |
| **E10** | Herauswerfen heißt Freigabe beenden und wirkt zuerst am Host, sofort und ohne Netz. Der Agent läuft weiter, offene Rückfragen bleiben offen. Ein Verbindungsabbruch ist kein Herauswerfen. | Eigener Knopf „Herauswerfen“. |
| **E11** | Inhalte ohne Command: Der Artefakt-Server des Gastes bleibt auf `127.0.0.1` und reicht über die Verbindung durch. Bilder nehmen denselben Weg, dafür kommt `http://127.0.0.1:*` zu `img-src`. | |
| **E12** | Eine Protokollnummer im Handschlag, bei Abweichung eine klare Ablehnung. Die App-Version taugt nicht, denn sie steigt mit jedem PR. | Kompatibilitätsschicht zwischen Versionen. |
| **E13** | Direkte Verbindung ohne Vermittler; das Nicht-Ziel „Cloud-Dienst“ bleibt. Das Feature heißt in allen Schichten `remote`. | Relay. |

### Abweichungen von der ersten Fassung

| | Entscheidung | Verworfen |
|---|---|---|
| **Z1** | Die Freigabe liegt dauerhaft in SQLite, der Inhaber nur im Speicher. Nach einem Neustart oder fünf Minuten ohne Verbindung ist das Vorhaben „freigegeben, nicht übernommen“. | Inhaber dauerhaft, Host gesperrt, bis er ausschaltet. |
| **Z2** | „Nur lokal“ wird erzwungen: Der Host trennt jede Verbindung von außerhalb von Loopback, RFC 1918, Link-Local, 100.64.0.0/10 und IPv6 ULA. | Nur eine Empfehlung. |
| **Z3** | E3 kleiner: Die Weiche sitzt im Core, keine Datei mit `invoke` ändert sich. | Modul `src/lib/ipc.ts`. |
| **Z4** | E7 kleiner: JSON-Zeilen über TLS. | WebSocket mit `tungstenite`. |

### Antworten des Nutzers

- **Wer ist der Gast:** dieselbe Person an einem anderen PC.
- **Netz:** TCP über LAN oder VPN, nur lokal.
- **Freigabe:** Sie überlebt einen Neustart des Hosts. Nur ausdrückliches Ausschalten wirft den Gast hinaus.
- **Kontingent:** Das Kontingent verbundener Hosts steht unter dem eigenen, im gewohnten Stil.
- **Terminal:** auch am Gast, mit eigenem Datenstrom je Terminal.
- **Leiste:** normale Liste mit Marke; entfernte Vorhaben lassen sich lokal in Gruppen einsortieren.
- **Push und PR:** Stationen pushen und öffnen PRs. Mergen und Tags bleiben beim Nutzer, kein Force-Push.
- **Mehrere Hosts:** Ein Gast kann mit mehreren Hosts zugleich verbunden sein.

## Steuerrecht

Je Vorhaben `shared` (SQLite, `projects.remote_shared`) und `holder` aus Gerät und `Connected` oder `Lost{since}` (nur im Speicher). `remote/control.rs` ist ein reiner Zustandsautomat ohne Uhr und ohne Tauri, die Tests spielen jede Zeile durch:

| Ereignis | Bedingung | Wirkung |
|---|---|---|
| `share` | Host | `shared = true`. Nur ein Angebot, der Host arbeitet weiter. |
| `unshare(stop_agents)` | Host, sofort und ohne Netz | `shared = false`, `holder = None`; mit `stop_agents` werden laufende Sessions angehalten. Terminals laufen weiter. |
| `take` | `shared`, `holder` leer oder dasselbe Gerät, Gerät nicht gesperrt | `holder = Gerät`. Der Host hat nur noch Lesezugriff. |
| `give_back` | Inhaber | `holder = None`, `shared` bleibt. |
| Verbindung weg | | `link = Lost{jetzt}`; eine zweite Meldung verschiebt die Frist nicht. |
| Neu verbunden | gleiches Gerät | `link = Connected`, kein neues `take`. |
| Fünf Minuten `Lost` | | `holder = None`, `shared` bleibt. |
| Neustart des Hosts | | `holder` leer, `shared` aus SQLite. |
| Server aus | Host | Alle Verbindungen zu, alle `holder = None`, `shared` bleibt. |
| Gerät sperren | Host | Verbindung zu, seine `holder = None`, Token ungültig. |
| `project_archive` | Host, kein Inhaber | `shared = false`; auch nach dem Wiederherstellen nicht mehr freigegeben. |

`access::check` ist eine Funktion für beide Stellen: Für die Oberfläche des Hosts im Wrapper `remote/intercept.rs`, dort sperrt sie nur, solange ein Gast Inhaber ist, und ohne Inhaber bleibt der Aufruf unverändert. Für Gäste im Server vor `on_message`. Ein Aufruf des Servers kommt mit der Kopfzeile `x-verwalter-remote` zurück durch den Wrapper und überspringt ihn.

**Alle IDs zählen.** `ids_of` liest `sessionId`, `projectId` und bei `implementation_*` und `terminal_*` auch `id`. Für einen Gast muss jede davon zu einem Vorhaben führen, das er hält, und ein Befehl mit ID-Pflicht ohne seine ID wird abgelehnt. Für die Oberfläche des Hosts meldet eine unbekannte ID weiter der Command selbst. `stationId` führt über `implementation::station_in` zu `id`: Eine Station einer fremden Umsetzung wird abgelehnt, bevor an ihr etwas wirkt.

Die Sperre von `RemoteControl` ist die innerste der App: Unter ihr wird keine andere genommen.

## Klassen

Die Tabelle in `remote/access.rs` ist die Liste. Kurz:

| Klasse | Gast | Host-Oberfläche | Zum Beispiel |
|---|---|---|---|
| **A** lesend am Host | nur als Inhaber | immer | `chat_history`, `git_status`, `terminal_attach` |
| **B** je Oberfläche | ID freiwillig, sonst als Inhaber | immer | `session_set_viewed` |
| **C** schreibend am Host | nur als Inhaber | gesperrt, solange ein Gast steuert | `chat_send`, `git_push`, `terminal_write` |
| **D** | nie | gesperrt, solange ein Gast steuert | `session_delete`, `project_archive` |
| **E** am Bildschirm oder nur lokal geordnet | nie | immer | `git_open`, `project_set_group`, `remote_share_set` |
| **F** ohne ID, über Zusammenführung oder eigene Wege | ja | immer | `project_list`, `usage_load` |
| **G** ohne ID, nur lokal | nie | immer | `settings_update`, `project_create`, `voice_*` |

`remote_share_set` steht in E: Der Host muss die Freigabe immer beenden können, auch während ein Gast steuert.

## Was Tauri hergibt

Zur Laufzeit belegt in der echten App mit Tauri 2.12, unter Linux im Dev-Lauf (`http://localhost`) und ohne Dev-Server (`tauri://localhost`):

- Ein Wrapper um `generate_handler!` sieht Befehlsname, Argumente und Kopfzeilen und kann ablehnen; die Oberfläche bekommt den Fehler wie von einem Command.
- `on_message` ruft aus dem Core synchrone und `async`-Commands mit und ohne Argumente und liefert das JSON der Antwort, auch Fehlerobjekte. Das Webview kommt über `get_webview_window("main")`; `get_webview` hängt am Feature `unstable`. Die Adresse im `InvokeRequest` muss lokal sein, `url()` des Fensters ist es.
- `listen_any` sieht jedes `app.emit`, darunter `session://changed`.

Unter Windows ist das noch nicht gelaufen.

## Verbindung

Was der Vertrag offenlässt, ist so festgelegt:

- Der Host bindet `[::]` und, wo das nur IPv6 ist (Windows), zusätzlich `0.0.0.0`; unter Linux nimmt `[::]` IPv4 als v4-mapped-Adresse an. Standard-Port 47800.
- Das Zertifikat ist selbst signiert (`rcgen`, ECDSA P-256). Der Gast prüft nur den SHA-256 des Zertifikats und lässt die Signaturen des Handschlags über `ring` prüfen. Die erste Zeile geht erst nach dem Handschlag hinaus, ein falscher Fingerabdruck verbraucht also keine Einladung.
- Die erste Zeile (`pair`, `hello`) darf höchstens 64 KiB lang sein; erst nach `welcome` gilt die Obergrenze von 64 MiB.
- Ein falsches Geheimnis zählt als Fehlversuch für jede offene Einladung, denn es sagt nicht, welche gemeint war.
- Eine einzelne Zeile über 16 MiB passt in eine leere Warteschlange; sonst käme keine Antwort zwischen 16 und 64 MiB an. Erst eine weitere Zeile darüber trennt.
- Der Gast verbindet nach einem Abbruch von selbst neu (1 s, dann doppelt so lange bis 30 s). Abgelehnt wegen Protokoll, Token oder Sperre, oder bei falschem Fingerabdruck, versucht er es nicht weiter. „Trennen“ ohne „Vergessen“ gilt bis zum nächsten Start der App.

## Konsequenzen

- „Lokal: kein Server“ gilt nicht mehr uneingeschränkt: Der Host kann auf ausdrücklichen Wunsch im lokalen Netz lauschen.
- Die Klassen sind Teil jedes neuen Commands: Ohne Eintrag wird `cargo test --lib remote::` rot, und `check.yml` führt das auch unter Windows aus.
- Die Oberfläche des Hosts trägt ab jetzt `ProjectSummary.access`. Alte Daten ohne das Feld gelten als eigenes Vorhaben.
- Dass der Inhaber nach fünf Minuten abläuft, braucht einen Zeitgeber. Er kommt mit dem Server, denn erst dann kann es einen Inhaber geben.
- Ein Gast hat über das Terminal und den Agenten eine Shell am Host (E9).
