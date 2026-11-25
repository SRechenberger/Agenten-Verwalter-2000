# Freigabe für eine ganze Gruppe von Vorhaben (Taiga 137)

Entwurf zum Nachtrag von [ADR 059](../../decisions/059-remote-sessions.md); Migration 36 (`project_groups.remote_shared`). `mockup.html` ist der eigenständige Prototyp (Rollen Host und Gast; für die Umsetzung zählt die Rolle Host).

## Entscheidungen

- **G1:** Die Freigabe hängt an der Gruppe. Neue Vorhaben der Gruppe sind freigegeben. Wer ein Vorhaben aus der Gruppe nimmt, beendet dessen Freigabe aus der Gruppe.
- **G2:** Wirksam freigegeben heißt: eigene Freigabe **oder** freigegebene Gruppe, und nicht archiviert. Einzeln beenden geht in einer freigegebenen Gruppe nicht: Der Schalter in der Übersicht zeigt „Über die Gruppe „<Name>“ freigegeben“, ist gesperrt und trägt den Knopf „Gruppe „<Name>“ bearbeiten …“. Der Knopf öffnet den Dialog „Freigabe beenden?“ für die Gruppe (kein eigenes Menü: Es gäbe nur diesen einen Eintrag).
- **G3:** Verlässt ein gehaltenes Vorhaben die Gruppe, ist das dasselbe wie Herauswerfen (E10). Es kommt der Dialog „Freigabe beenden?“ mit „Laufende Agenten anhalten“; der Text nennt die gehaltenen Vorhaben und ihre Geräte. Dasselbe gilt für „Gruppe löschen“ und „Freigabe beenden …“ am Gruppenkopf. Die Oberfläche kennt die eigene Freigabe eines Vorhabens nicht: Hat es eine, bleibt es freigegeben, und der Dialog warnt dann zu viel.
- **G4:** Der Gast übernimmt weiter je Vorhaben (E5).
- **G5:** Der Gast sieht die Gruppe des Hosts nicht und sortiert lokal.
- **G6:** „Für Gäste freigeben“ und „Freigabe beenden …“ stehen im Kontextmenü des Gruppenkopfs, nur bei eingeschaltetem Server (oder solange die Gruppe freigegeben ist). Der Kopf trägt ⇄. Ein Vorhaben in eine freigegebene Gruppe zu ziehen, gibt es ohne Dialog frei, mit einer Meldung unter der Leiste.
- **Archiv:** Archivieren beendet die wirksame Freigabe; Wiederherstellen in einer freigegebenen Gruppe gibt wieder frei.

## Umsetzung

- Eine Abfrage (`db::remote::effective_shared_project_ids`) entscheidet; `Control::set_shared` gleicht den Speicher ab, die Registry ruft es nach jeder Änderung (`sync_shared`). Alle Leser gehen unverändert über `Control::is_shared`.
- Befehle: `remote_group_share_set` (Klasse E), `remote_group_share_list` (Klasse G); `stopAgents` optional an `project_set_group` und `project_group_delete`.
