# 064 — „Automatisch fortsetzen“ je Umsetzung und Station

**Status:** angenommen, umgesetzt: Kern (Migration 27, Befehle, Zeitgeber, Zustand, Einstellung) und Oberfläche (Leiste, Warnleiste, Zeile, Kartei, Einstellungszeile; Story Taiga #56); die Abnahme in der echten App steht aus; `WaitsForQuota` hält seither einen Platz der globalen Warteschlange statt einen unter „Parallel max.“ ([ADR 068](068-globale-warteschlange.md)) · **Datum:** 2026-11-15

## Kontext

Am aufgebrauchten Kontingent pausiert eine Session mit Begründung, und die Karte im Chat bietet „Automatisch fortsetzen“ an ([ADR 051](051-kontingent-warnung.md)): 30 Sekunden nach dem Zurücksetzen schickt der Verwalter der Session selbst die Aufforderung, weiterzumachen. Der Schalter gilt nur für diese eine Pause und lebt nur im Speicher. In einer Umsetzung ([ADR 048](048-umsetzung.md), [ADR 061](061-constraint-ablaeufe.md)) heißt das: Ist das Kontingent leer, muss der Benutzer in jede Station einzeln wechseln und schalten, und vorab, bevor das Kontingent leer ist, geht es gar nicht.

Die Story (Taiga #56) verlangt einen Schalter für alle Stationen einer Umsetzung, schon vorab setzbar, mit Standardwert aus den Einstellungen und je Station überschreibbar. Der Benutzer hat am 2026-11-15 entschieden: Der Wert wird je Station gespeichert (nicht nur im Speicher der Session), und die Beschriftung heißt überall „Automatisch fortsetzen“.

## Entscheidung

- **Wirksam ist Station vor Umsetzung** (Migration 27): `implementations.auto_resume` (0/1, Standard 0) und `implementation_stations.auto_resume` (leer = wie die Umsetzung, 1, 0). Bestehende Umsetzungen stehen danach auf „aus“, ihre Stationen auf leer. Die Ansicht trägt `ImplementationView.auto_resume`, `StationView.auto_resume` (gespeicherter Wert) und `StationView.resumes_automatically` (wirksam), wie `auto_start` und `starts_automatically` in ADR 061.
- **Die letzte Aktion gewinnt:** `implementation_set_auto_resume(id, enabled)` schreibt die Umsetzung und setzt alle nicht fertigen Stationen auf leer. `implementation_set_station_auto_resume(id, stationId, value)` schreibt eine Station, solange sie nicht fertig ist, auch mit Session; Stationen „durch dich“ lehnt er ab. Die Karte im Chat (`session_set_auto_resume`) schaltet wie bisher die Session und schreibt bei einer Station zusätzlich deren Wert; außerhalb einer Umsetzung bleibt sie wie bisher. Alle drei melden `implementation://changed`.
- **Eine fertig gemeldete Station setzt nie fort:** Die Fertigmeldung (`complete_station_with_handoff`) schaltet nach dem Speichern den Zeitgeber der eigenen Session der Station aus, auch bei `fix_needed`; scheitert das, steht es nur im Protokoll, die Meldung bleibt gültig. Beim Start des Fortsetzens (der Zeitgeber löst nur einen Durchgang der Warteschlange aus, [ADR 068](068-globale-warteschlange.md)) prüft `resume_claimed` nach dem Abbruch-Test (ADR 066) noch einmal (`resume_is_finished`, Session-Sperre nur kurz, Datenbank danach ohne sie) und schickt bei einer fertigen Station nichts; der Schalter geht dann ohne Fehlerkarte aus, denn das Ende ist kein Fehler.
- **Zwei eigene Befehle** statt `StationOptions` zu erweitern: Klasse C mit `IdKind::Implementation` in `remote/access.rs`. „Alle drei auf einmal“ der Optionen und ihre Sperren nach Zustand bleiben, wie sie sind.
- **Zeitgeber scharf schalten im Haken, kein Spiegel im Speicher:** `pause_for_quota` ist eine Methode des Session-Zustands und sieht weder Registry noch Datenbank; sie beginnt jede Pause wie bisher mit `auto_resume: false`. Danach setzt `set_status(Paused)` das Signal `station_idle`, und der Haken `station_session_ended` bekommt nun auch die Session-ID. Er prüft **zuerst** unter einer kurzen Session-Sperre, ob die Session mit `PauseReason::Quota { resets_at: Some(t), auto_resume: false }` pausiert, liest dann ohne Sperre aus der Datenbank den wirksamen Wert und ob die Station offen ist, entscheidet mit der reinen Funktion `continues_after_quota` (über `arms_on_new_pause`) und ruft `set_auto_resume(session, true)` (idempotent). **Erst danach** plant er die Warteschlange, sonst sähe sie „wartet auf dich“ und gäbe den Platz frei. Die Sperren bleiben nacheinander: nie zwei Session-Sperren, keine Datenbank unter der Session-Sperre im neuen Code.
- **Schutz vor einer Schleife:** Eine neue Pause schaltet nur scharf, wenn die Rücksetzzeit bekannt ist und in der Zukunft liegt; ein vergangenes Ziel feuerte sofort und liefe gleich wieder ins Kontingent. Die Schalter-Befehle schalten schon pausierte Sessions **ohne** diese Prüfung (ein vergangenes Ziel feuert sofort, wie die Karte heute) und übergehen `AutoResumeUnavailable` (Session läuft, Grund fehlt, Zeit unbekannt) und verschwundene Sessions. Eine fertig gemeldete Station schaltet nie scharf.
- **Zustand `WaitsForQuota`** („setzt automatisch fort“): Die Session pausiert mit `PauseReason::Quota { auto_resume: true }` (`SessionLive.waits_for_quota`, `state_of` bekommt es als dritten Parameter). Rang wie `WaitsForScript`, also hinter „fragt“, „wartet auf dich“ und „Fehler“. Er **zählt als aktiv**: Das Kontingent ist kontoweit leer, eine nachrückende Station liefe sofort wieder dagegen und überschritte nach dem Zurücksetzen die Grenze. Der Lagebericht nennt ihn unter „Jetzt“.
- **Eigenes Signal beim Ausschalten:** `set_auto_resume(false)` und ein gescheitertes Senden (`auto_resume_failed`) ändern den Zustand ohne `set_status`. Die Warteschlange muss danach neu planen, denn der Platz ist frei; dafür setzt der Session-Zustand das Signal `station_disarmed` in der `Outbox`, und `update` startet den Haken `station_disarmed`, der nur plant. Nicht `station_idle`: dessen Haken schaltete wieder scharf, bei einem Fehlstart des Zeitgebers eine Schleife.
- **Nur der Benutzer setzt den Wert:** Kein Feld an `StationInput`, `StationPatch`, `StationOptions` und an den Schemas von `implementation_create`, `implementation_edit` und `implementation_set_phases`; ein Test hält fest, dass die Schemas `auto_resume` und `autoResume` nicht nennen.
- **Einstellung** `default_auto_resume` (Standard aus, `SettingsChange::DefaultAutoResume`), nach dem Muster von `mark_unsourced`. `create_implementation` liest sie vor der Datenbank-Sperre und gibt sie `new_rows` mit; sie gilt nur für neue Umsetzungen.
- **Fix-Runde und zweites Review** übernehmen den Wert ihrer Vorlage (`Carried`, auch in der eigenen Einfügeliste von `complete_station_with_followups`), eine Plan-Änderung behält ihn an einer angepassten Station (`keep_settings`); der Schalter je Station setzt dafür die Marke `edited` wie die Optionen.

## Was das in früheren ADRs überholt

- **[ADR 051](051-kontingent-warnung.md):** „Nur im Speicher“ gilt nur noch für den Zeitgeber, die Marke und die Begründung der Pause; der Wert je Umsetzung und Station steht in der Datenbank. „Eine zweite Kontingent-Antwort nach dem Fortsetzen beginnt wieder mit `auto_resume: false`“ gilt nur noch außerhalb von Umsetzungen; an einer Station schaltet der Haken danach nach ihrem gespeicherten Wert scharf, aber nur bei einer Rücksetzzeit in der Zukunft.
- **[ADR 052](052-session-werkzeuge.md):** Die Kettenbremse beginnt mit jedem automatischen Fortsetzen neu, jetzt auch ohne Klick bei jeder Pause einer Station.
- **[ADR 061](061-constraint-ablaeufe.md):** Die Liste „Aktiv“ wächst um `WaitsForQuota`; der Haken am Sessionende schaltet zuerst scharf, dann plant er.

## Oberfläche (Phase 56.2)

Nach dem Entwurf [`docs/design/2026-11-15_automatisch-fortsetzen/`](../design/2026-11-15_automatisch-fortsetzen/README.md); die Abweichungen stehen dort.

- **Leiste:** Schalter ↻ hinter „Automatisch starten“ (`AutoResumeSwitch`, in der Leiste ohne sichtbaren Namen, „Automatisch fortsetzen“ als Tooltip und `aria-label`, damit die Leiste einzeilig bleibt; der Fortschrittsbalken wächst dafür von 60 bis 160 Pixel statt fest 160), zeigt `implementation.autoResume`, schaltet über `implementation_set_auto_resume`. Keine optimistische Anzeige: der Schalter liest die Ansicht, die Änderung kommt per `implementation://changed`.
- **Warnleiste** (`QuotaWaitBar`) zwischen Leiste und Fehlerzeile, außerhalb des Reiterinhalts, also unter „Ablauf“ und „Changes“: steht, solange eine nicht fertige Station mit Session am Kontingent pausiert (`pauseReason.kind === 'quota'`), und trägt denselben Schalter. Die Zahlen (wartend, fortsetzend mit frühester Rücksetzzeit, Schalter aus, ohne Rücksetzzeit, nach der Rücksetzzeit „zurückgesetzt“) rechnet die reine Funktion `quotaWaiting` mit Fällen im `pnpm selftest`. Sie liest die Begründung der Pause, die nur im Speicher lebt: nach einem Neustart der App steht sie nicht.
- **Zeile:** ↻ als eigener Knopf rechts im Zustand, neben der Mini-Aktion und außerhalb des Titel-Knopfs (`aria-pressed` = `resumesAutomatically`, ein Punkt bei einer Abweichung von der Umsetzung, gesperrt bei fertiger Station, nicht bei Stationen „durch dich“); ein Klick schreibt `implementation_set_station_auto_resume` mit dem Gegenteil des wirksamen Werts. Der Zustand „setzt automatisch fort“ steht in neutralem Ton mit ↻ im Punkt und dem Chip „ab HH:MM“; „wartet auf dich“ am Kontingent trägt den Chip „Kontingent“ oder „Rücksetzzeit unbekannt“. `WaitsForQuota` zählt in der Oberfläche als aktiv, nicht als „braucht dich“, und tickt nicht (kein Zähler).
- **Kartei:** Abschnitt „Automatik“ mit „wie Umsetzung · an · aus“ zwischen „Automatisch starten“ und „Bei Fehler“, änderbar bis zur Fertigmeldung, mit der Zeile „Läuft mit: … (von der Umsetzung | nur diese Station)“; die Kopfzeile und der Sperrhinweis nennen den Wert.
- **Einstellungen:** Zeile „Automatisch fortsetzen“ im Abschnitt „Agent“, gespeichert als `defaultAutoResume`.
- **Karte im Chat:** unverändert; bei einer Station zeigt sie `pauseReason.autoResume`, das der Kern beim Schalten der Station mitschaltet.
- Alle neuen Bedienelemente sperren für Gäste der Fernsteuerung über `useProjectLocked`.

## Neustart (Critical Rule 4)

Nach einem Neustart der App sind Zeitgeber und Begründung der Pause weg; die Session ist pausiert, ohne Grund. Keine Station wartet dann auf das Kontingent, keine setzt von selbst fort. Die Werte je Umsetzung und Station bleiben.

## Bekannte Grenzen

- **Fenster zwischen Pause und Scharfschalten:** Der Haken läuft in einem eigenen Thread nach der Session-Sperre. Plant in diesem Augenblick ein anderes Ereignis die Warteschlange, sieht es „wartet auf dich“ und kann den Platz vergeben; die Grenze wird dann einmal überschritten. Nicht ausgelöst, aus dem Code gelesen.
- **Die Schalter-Befehle schalten einen vergebenen Platz wieder scharf:** Sie prüfen die Warteschlange nicht. Pausiert Station A ohne Zeitgeber, rückt B bei Grenze 1 nach und schaltet der Benutzer dann „Automatisch fortsetzen“ ein, zählt A als `WaitsForQuota` wieder als aktiv; die Umsetzung steht dann über „Parallel max.“.
- **Die Kettenbremse beginnt bei jedem Fortsetzen neu** (siehe oben).
- **Die Fertigmeldung und das Feuern laufen ohne gemeinsame Sperre:** Meldet der Benutzer die Station genau zwischen der Prüfung beim Feuern und `send`, geht der Text noch einmal hinaus (wie bei einer Nachricht des Benutzers an dieser Stelle). Das Ausschalten bei der Fertigmeldung deckt den Rest ab.
- **Alle pausierten Stationen feuern im selben Moment:** Sie warten auf dieselbe Rücksetzzeit plus 30 Sekunden; eine Staffelung gibt es nicht.
- **Der echte Stream einer Kontingent-Sperre ist nie gemessen** (ADR 051); geprüft ist der Kern mit Tests, dem Selbsttest von `implementation-probe` und der Attrappe der Kommandozeile.

## Prüfung

`cargo test --manifest-path src-tauri/Cargo.toml --lib -- implementation:: agent_types::` prüft Wirksamkeit, Zukunftsprüfung, fertige Station, Zustand und Platz, Ausschalten gibt den Platz frei, Rang, Lagebericht, „letzte Aktion gewinnt“ in der Datenbank, den Schalter je Station bis zur Fertigmeldung, Migration 27, Standardkopie und dass eine Änderung des Standards bestehende Umsetzungen nicht berührt, die JSON-Form der Einstellung, Fix-Runde und Plan-Änderung, die Schemas der Werkzeuge und die Klasse der Befehle. `implementation-probe --selftest` hat vier Fälle „Automatisch fortsetzen“ (Wirksamkeit und letzte Aktion, Platz, Schutz vor der Schleife, Neustart). `pnpm selftest` prüft die Zählung der Warnleiste; die Bilder der echten App liegen im Entwurfsordner unter `umsetzung/`.

## Nachtrag 2026-11-24 (Taiga-Issue 226): eine zweite Kontingent-Meldung behält den Zeitgeber

Endet ein Hintergrund-Befehl (`Bash` im Hintergrund, `Monitor`), beginnt die Kommandozeile von selbst einen neuen Zug. Läuft der ins Kontingent, meldet die Session ein zweites Mal, obwohl sie schon pausiert. Früher setzte `SessionState::pause_for_quota` dann `auto_resume: false` und nahm den scharfen Zustand weg: Der Zeitgeber endete als abgebrochen, die Station setzte nach dem Reset nicht fort. Dafür gibt es keinen Haken, denn ohne Statuswechsel (schon pausiert) läuft er nicht noch einmal.

Als Wiederholung gilt: Die Session ist vor der Meldung `Paused` mit der Begründung Kontingent. `pause_for_quota` ist der eine Weg für Sessions mit und ohne Station; die Regeln:

| Vorher | Neue Rücksetzzeit | Danach |
|---|---|---|
| scharf mit `t` | gleich `t` | scharf mit `t`, Marke unverändert, der laufende Zeitgeber gilt weiter |
| scharf mit `t` | bekannt, `t2` ungleich `t` | scharf mit `t2`, Marke +1, ein neuer Zeitgeber für `t2` |
| scharf mit `t` | unbekannt | scharf mit `t`, wie in der ersten Zeile |
| nicht scharf | beliebig | `auto_resume: false` mit der neuen Zeit, wie zuvor |

- `awaits_arming` behält bei einer Wiederholung seinen Wert: Hat der Haken der ersten Pause noch nicht entschieden, hält die Session ihren Platz, bis er entscheidet.
- Der neue Zeitgeber startet nach dem Freigeben der Session-Sperre (`Outbox::restart_auto_resume`, wie bei `set_auto_resume`), nie unter ihr.
- Die Karte „Kontingent aufgebraucht“ kommt bei jeder Meldung.

## Nachtrag 2026-11-24 (Taiga #227): eine Anzeige für das Warten, kein `awaits_arming`

Löst im Nachtrag zu Taiga-Issue 226 den Satz zu `awaits_arming` ab: Der Merker und `slots::awaits_arming` entfallen, eine pausierte Session belegt nie einen Platz ([ADR 068](068-globale-warteschlange.md), Nachtrag 2026-11-24). Der Haken `station_session_ended` schaltet den Zeitgeber weiter zuerst scharf, ohne dass davon ein Platz abhängt.

Die Anzeige folgt dem gezeigten Zustand der Session (`shownState`: `live`, sonst `state`), nicht `station.state`. Eine Regel in `src/features/implementation/stationLive.ts` (`liveText`, `waitChip`), die Zeitleiste, Leiste, Kartei und Sidebar nutzen:

| Zustand | Text | Chip |
|---|---|---|
| scharfer Zeitgeber (`live` oder `state` ist `waitsForQuota`) | „setzt automatisch fort“, hinter „gemeldet“ und „fertig“ „gemeldet · setzt automatisch fort“ und „fertig · setzt automatisch fort“ | neutral „ab 13:30“ (`resumesAtChip`), nie bernsteinfarben |
| ohne Zeitgeber | „pausiert“ bzw. „pausiert · wartet auf dich“, hinter „gemeldet“ und „fertig“ „… · pausiert“ | bernsteinfarben „Kontingent bis 13:30“ (`pauseChip`), denn dann muss jemand handeln |
| vom Halt der Warteschlange gehalten | „wartet auf Kontingent“, ein Neustart „Fehler · wartet auf Kontingent“ | neutral „ab 13:30“, die Zeit aus `quotaUntil` der Warteschlange |

Früher machte `liveText` aus `waitsForQuota` und `waitsForYou` dasselbe „gemeldet · pausiert“, und `renderQuotaChip` prüfte `station.state` statt der Session: Eine gemeldete Station mit scharfem Zeitgeber sah aus wie eine Pause, die auf den Benutzer wartet. Entwurf: [`docs/design/2026-11-24_kontingent-warten/`](../design/2026-11-24_kontingent-warten/README.md).
