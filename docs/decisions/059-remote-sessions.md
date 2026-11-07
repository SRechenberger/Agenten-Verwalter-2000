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
- Die Warteschlange hält höchstens 16 MiB, auch für eine einzelne Zeile: Eine Zeile über 16 MiB trennt die Verbindung, auf beiden Seiten. Gelesen werden Zeilen bis 64 MiB.
- Den Abbruch nach 45 s ohne Zeile erkennt ein eigener Wächter je Verbindung, nicht der Schreib-Thread; der kann im Schreiben hängen, wenn der Gast verschwunden ist.
- Der Gast verbindet nach einem Abbruch von selbst neu (1 s, dann doppelt so lange bis 30 s). Abgelehnt wegen Protokoll, Token oder Sperre, oder bei falschem Fingerabdruck, versucht er es nicht weiter. „Trennen“ ohne „Vergessen“ gilt bis zum nächsten Start der App.

## Übernahme über das Netz

Was der Vertrag offenlässt, ist so festgelegt:

- **Host.** `remote/dispatch.rs` beantwortet jeden Aufruf eines Gastes. `remote_take` und `remote_give_back` ändern das Steuerrecht direkt. `project_list`, `session_list`, `implementation_list` und `session_overlaps` liest der Host aus der Registry statt über `on_message` und filtert sie: freigegebene Vorhaben für jedes Gerät, Sessions, Umsetzungen und Überschneidungen nur für den Inhaber; eine Überschneidung zählt zum Vorhaben ihrer `sessionId`. So führt auch ein Host, der selbst Gast ist, keine Listen eines weiteren Hosts zusammen. Jeder andere Befehl geht durch `access::check` und dann über `Webview::on_message` auf dem Fenster `main`, mit `invoke_key` aus dem Core und der Kopfzeile `x-verwalter-remote`. `terminal_attach` lehnt der Host bis zum Terminal am Gast ab, denn sein Kanal ginge sonst an die Oberfläche des Hosts.
- **Ereignisse.** Am Host hört `listen_any` die Ereignisse aus dem Vertrag und legt sie in einen eigenen Thread, denn ein `emit` kann unter Sperren der Registry laufen, die die Suche nach dem Vorhaben selbst braucht. Weitergegeben wird nur an einen verbundenen Inhaber, `project://changed` an jedes verbundene Gerät, solange das Vorhaben freigegeben ist. Endet die Freigabe (Ausschalten, Archivieren, Löschen der letzten Session), bekommt jedes verbundene Gerät den Rahmen `access`. Der Gast gibt nur Ereignisse aus dem Vertrag unter ihrem Namen an die Oberfläche; `project://changed` zeigt er als `remote`, `access` wird `remote://changed`.
- **Frist.** Ein Thread am Host prüft alle fünf Sekunden, ob ein Inhaber seit fünf Minuten keine Verbindung hat.
- **Weiche.** Der Gast kennt je Host die IDs aus dessen Listen, Antworten und Ereignissen (`id`, `sessionId`, `projectId`, `implementation.id`). Nennt ein Aufruf eine davon, geht er an diesen Host; das gilt für die Klassen A, C, D und E außer `project_set_group`, denn was der Host nie erlaubt, soll er auch ablehnen. Die Listen wartet je Host höchstens fünf Sekunden und nehmen danach, was er zuletzt geschickt hat, mit `online: false`. Wie die Oberfläche, die Listen nur einmal lädt und danach Ereignisse einfügt, ersetzen oder ergänzen `project://changed`, `session://changed`, `implementation://changed` und die Antworten von `remote_take`, `remote_give_back`, `session_create_in_project` und `implementation_new_inquiry` den Eintrag mit derselben `id`. Der Zwischenspeicher lebt nur im Speicher; „Vergessen“ löscht die Gruppen der Vorhaben, die der Host zuletzt angeboten hat.
- **Gezeigte Session (Klasse B).** Die Registry führt je Oberfläche eine gezeigte Session: `None` ist die eigene, sonst die ID des Gastes, die der Server als `viewer` einträgt und damit überschreibt, was der Gast schickt. Eine Session gilt als gesehen, solange irgendeine Oberfläche sie zeigt. `session_set_viewed` geht beim Gast an beide Seiten: lokal mit der eigenen ID oder `None`, an jeden verbundenen Host mit seiner ID oder `None`; ob ein Host antwortet, ändert nichts an der Ansicht am Gast.
- **Protokoll.** `PROTOCOL` ist 3: Ein Host mit 2 kennt `origin` an `chat_send` und `chat_answer` und die Befehle `remote_artifact`, `remote_file` und `remote_host_profile` nicht, der Handschlag sagt das.

## Inhalte über das Netz

Was der Vertrag offenlässt, ist so festgelegt:

- **Artefakte.** `remote_artifact(sessionId, path)` und `remote_file(sessionId, path)` sind Befehle nur des Servers (`remote/dispatch.rs`, vor der Tabelle der Klassen): `check_content` verlangt eine Session als ID und den Inhaber ihres Vorhabens, wie ein Lesebefehl der Klasse A. `remote_artifact` liest im Ordner `.artefakte` mit `artifacts::resolve` (`..`, Wurzel, Laufwerk und Symlinks nach draußen verboten); ein Pfad mit `/` am Ende liefert die Dateiliste des Ordners als JSON (`?list=1` am Gast). `remote_file` liest eine Datei, deren kanonischer Pfad unter dem kanonischen Workspace des Vorhabens liegt, absolut oder ab dem Workspace; so erreicht der Gast Anhänge (`.anhaenge`), nichts darüber hinaus.
- **Obergrenze.** Eine Datei darf höchstens 8 MiB groß sein (`remote::content::MAX_BYTES`): Als Base64 sind das rund 10,7 MiB, und eine Zeile über 16 MiB trennte die Verbindung. Größere lehnt der Host mit einem Satz ab; der Gast zeigt sie als nicht erlaubt.
- **Server am Gast.** Gehört die Session zu einem Host (`RemoteClients::host_of`), holt `artifacts/server.rs` Liste, Ordner-Betrachter, Seiten und Dateien über die Verbindung; Brücke, Betrachter-Seiten und Content-Security-Policy setzt er wie für lokale Artefakte. Bilder und Anhänge eines Hosts liefert `/<token>/file/<sessionId>/<Pfad>` mit der Policy `default-src 'none'; sandbox`; `remote_asset_base` nennt dafür die Adresse bis `file/`. `artifacts_list` ersetzt `baseUrl` durch die Adresse dieses Servers. In `tauri.conf.json` kam `img-src http://127.0.0.1:*` dazu.
- **Anhänge senden.** `chat_send` am Gast lädt Anhänge, die im Zwischenordner dieses Geräts liegen, über `attachment_add_bytes` zum Host (Klasse F) und ersetzt die IDs; IDs, die hier unbekannt sind (der Lagebericht einer Rückfrage zur Umsetzung), gehen unverändert durch. Gelöscht werden die lokalen Dateien erst, wenn das Senden gelungen ist; scheitert es, findet ein neuer Versuch sie wieder. Bereits hochgeladene Kopien am Host bleiben bis zu dessen nächstem Start im Zwischenordner (`attachment_discard` ist kein Befehl für Gäste).
- **Herkunft.** Der Server trägt bei `chat_send` und `chat_answer` den Namen des Geräts als `origin` ein und überschreibt, was der Gast schickt. `ChatEntry::User` und `ChatEntry::Question` halten ihn (`serde(default)`, keine Migration); der Text an den Agenten enthält ihn nie.
- **Profil und Kontingent.** `remote_host_profile` setzt Betriebsart, Modelle von LM Studio und Codex und die Skill-Aufrufe des Hosts aus den Commands `settings_load`, `settings_local_models`, `codex_models` und `skill_uses` zusammen. `remote_usage_load` und `remote_usage_refresh` rufen am Host `usage_load` und `usage_refresh`. `usage://changed` geht an jedes verbundene Gerät (nicht nur an Inhaber), und der Gast gibt es als `remote://usage-changed` mit `{ hostId }` weiter.

## Oberfläche

- **Sperre in der Oberfläche.** Ein Bedienelement, das ins Vorhaben schreibt (Klassen C und D), sperrt sich über `useProjectLocked(projectId)`: am Host, solange ein Gast steuert, am Gast, solange der Host fehlt. Der Core lehnt ohnehin ab; die Sperre spart den Fehler. Die Eingabeleiste wird zur Sperrzeile, der Entwurf bleibt im Store.
- **Rechte aus den Listen.** `ProjectSummary.access` genügt: `shared` mit Inhaber am Host, `remote` mit `mine` und `online` am Gast. Die Restzeit der Reservierung liest die Oberfläche aus `RemoteDevice.lastSeenAt`, das der Host beim Abriss der Verbindung schreibt; sie zählt gegen die fünf Minuten des Steuerrechts.
- **Ende der Freigabe.** Das Vorhaben verschwindet aus den Listen des Gastes. Die offene Ansicht zeigt „Freigabe beendet“, solange der Host verbunden ist (sonst ist es der Zwischenspeicher oder eine vergessene Kopplung). Hält der Gast ein Vorhaben nicht mehr, das noch in der Liste steht (Neustart des Hosts, gesperrtes Gerät), zeigt sie das Angebot statt einer fremden Session.
- **Nach `remote://changed`** ersetzen die Listen der Vorhaben, Sessions und Umsetzungen die hiesigen, und der Verlauf einer entfernten Session lädt die neueste Seite neu (bei einem Fehler bleibt der Stand stehen). Ereignisse aus der Lücke fehlen sonst.
- **Betriebsart und Modelle des Hosts.** Bei einem entfernten Vorhaben gilt das Host-Profil: `useOperatingMode(projectId)` liest es (Voreinstellung `claude`, solange es fehlt) und die Eingabeleiste zeigt „<Betriebsart> · Host“ als gestrichelte Marke ohne Knopf; die Codex-Modelle kommen aus dem Profil. Die Oberfläche lädt die Profile nach jedem `remote://changed`; ändert der Host seine Betriebsart, merkt der Gast es erst beim nächsten Ereignis.
- **Kontingent.** Unter dem eigenen steht je Host eine Zeile im Stil der einen Zeile von früher (Schweregrad, Füllstandsbalken, Prozent); ein Host, der nicht erreichbar ist, zeigt „nicht erreichbar“. Das Fenster nennt den Host. `UsageAlertBar` bleibt beim eigenen Kontingent.
- **Bilder.** `assetUrl` (Hook `useAssetUrl`) setzt bei einer Session eines Hosts die Adresse des Artefakt-Servers des Gastes ein (`/<token>/file/<Session>/<Pfad>`); ungesendete Anhänge bleiben beim Asset-Protokoll. Die CSP erlaubt dafür `img-src http://127.0.0.1:*`, sonst ändert sie sich nicht.
- **Ausgeblendet** an entfernten Vorhaben: Dateiverweise (Klartext), Explorer und VS Code im Git-Menü und in der Konfliktzeile, „Im Browser/Mit Standardprogramm öffnen“ und „Im Explorer zeigen“ bei Artefakten und im Scratchpad, „In neuem Vorhaben starten“; Browser-Schalter, MCP-Anmeldung, Löschen, Archivieren und „Repository hinzufügen“ schon vorher.
- **Herkunft.** `OriginLine` zeigt „von <Gerät>“ über der Nachricht und der beantworteten Rückfrage, an Gast und Host.
- **Beenden.** `CloseGuard` fängt `onCloseRequested` ab, solange ein Gerät online ist, und fragt nach; „Trotzdem schließen“ ruft `destroy()` (`core:window:allow-destroy`). Der Dialog zum Installieren eines Updates warnt ebenso.

## Konsequenzen

- „Lokal: kein Server“ gilt nicht mehr uneingeschränkt: Der Host kann auf ausdrücklichen Wunsch im lokalen Netz lauschen.
- Die Klassen sind Teil jedes neuen Commands: Ohne Eintrag wird `cargo test --lib remote::` rot, und `check.yml` führt das auch unter Windows aus.
- Die Oberfläche des Hosts trägt ab jetzt `ProjectSummary.access`. Alte Daten ohne das Feld gelten als eigenes Vorhaben.
- Dass der Inhaber nach fünf Minuten abläuft, braucht einen Zeitgeber; er läuft am Host neben der Weitergabe der Ereignisse.
- Ein Gast hat über das Terminal und den Agenten eine Shell am Host (E9).
