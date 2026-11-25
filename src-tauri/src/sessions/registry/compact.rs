//! `/compact` der ruhenden Master-Session über der Grenze (ADR 070): die Regel rein, der Zug in der
//! Registry. Der Zug ist keine Eingabe: kein Chat-Eintrag des Benutzers, kein Ungelesen, keine Antwort.
use std::sync::Arc;
use std::thread;

use tauri::{AppHandle, Manager};

use super::session_tools::Route;
use super::{
    ChatEntry, Outbox, Session, SessionRegistry, SessionState, SessionStatus, TurnOrigin,
    schedule_pass, start_turn, update,
};
use crate::db::implementations::{
    self as implementation_rows, ImplementationRow, StoredImplementation,
};
use crate::db::session_inbox;
use crate::error::CommandError;
use crate::implementation::wake;
use crate::notices::model::Notice;
use crate::sessions::model::CompactPhase;
use crate::settings;
use crate::settings::model::OperatingMode;

/// Der Befehl, den der Verwalter dem Agenten schickt.
pub(super) const COMPACT_TEXT: &str = "/compact";

/// Betreff der Bitte und der Erinnerung im Chat (nicht im Text an den Agenten).
const REQUEST_SUBJECT: &str = "Compact vorbereiten";
const REMINDER_SUBJECT: &str = "Erinnerung: Compact";

/// Die Erinnerung, wenn der Vorbereitungszug ohne Meldung endet; einmal. Der Wortlaut ist an Haiku
/// gemessen (ADR 070, Nachtrag „Compact mit Vorbereitung“); eine Änderung braucht eine neue Messung.
pub const REMINDER_TEXT: &str = "Du hast dich noch nicht mit mcp__verwalter__verwalter_compact_ready gemeldet. Sichere deinen Stand und ruf das Werkzeug auf; endet dieser Zug ohne Aufruf, fasse ich trotzdem zusammen.";
/// Was `verwalter_compact_ready` in der Vorbereitung antwortet.
pub const READY_REPLY: &str = "Gemeldet. Der Compact folgt, wenn dein Zug endet.";
/// Was `verwalter_compact_ready` sonst antwortet.
pub(super) const NO_COMPACT_PENDING: &str = "Gerade steht kein Compact an.";
/// Der Grund, warum nichts in die Session geht, wie der Chat und „Jetzt senden“ ihn nennen.
pub(super) const COMPACT_LOCKED: &str =
    "Die Session bereitet einen Compact vor. Nachrichten warten, bis er fertig ist.";
const COMPACT_RUNNING: &str = "Der Compact läuft schon.";
const END_OF_IMPLEMENTATION: &str = "Die Umsetzung ist fertig – kein Compact nötig";

/// Die Bitte des Verwalters an den ruhenden Master über der Grenze (ADR 070, Nachtrag). Der Wortlaut ist
/// an Haiku gemessen; er nennt den vollen Namen des Werkzeugs (Laden aufgeschobener Werkzeuge).
pub fn request_text(limit: u32) -> String {
    format!(
        "Du bist über dem Limit von {} Tokens Kontext. Bereite dich auf /compact vor und melde dich mit einem Aufruf von mcp__verwalter__verwalter_compact_ready beim Verwalter zurück.",
        grouped(limit)
    )
}

/// Tausendergruppen mit Leerzeichen: `150 000`.
fn grouped(number: u32) -> String {
    let digits = number.to_string();
    let first = digits.len() % 3;
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (index + 3 - first).is_multiple_of(3) {
            grouped.push(' ');
        }
        grouped.push(digit);
    }
    grouped
}

/// Der Weg einer Nachricht an eine Session im Compact: Alles, was zustellbar wäre, wartet; auch ein
/// Anstoß des Durchgangs holt sie nicht ab (`compact_locked`). Unzustellbar bleibt unzustellbar.
pub(super) fn locked_route(route: Route, locked: bool) -> Route {
    match route {
        Route::Undeliverable(reason) => Route::Undeliverable(reason),
        _ if locked => Route::Queue { nudge: false },
        other => other,
    }
}

/// Was die Registry nach dem Ende eines Zugs tut (`start_phase_turn`), wenn die Session im Compact steht.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Step {
    /// Nichts: Der Compact läuft, oder die Session ruht nicht.
    Nothing,
    /// Die Erinnerung schicken.
    Remind,
    /// Den Compact-Zug starten.
    Compact,
}

/// Was nach dem Zugende zu tun ist, je Phase („Zustandsdiagramm #215“). `paused`: Die Session ist
/// pausiert; ihr Fortsetzen nimmt den Compact wieder auf, sobald die Meldung (`Ready`) vorliegt. Vor der
/// Meldung setzt das Fortsetzen den Zug selbst fort (`send_resume`), dort gibt es keinen Schritt: Ein
/// pausierter Erinnerungszug (`Reminded`) endet erst mit dem Zugende im Ruhen. Eine zweite
/// Bitte gibt es nie: Die Phase endet erst mit dem Compact.
pub(super) fn step_after_turn(phase: CompactPhase, rests: bool, paused: bool) -> Step {
    match phase {
        CompactPhase::Preparing if rests => Step::Remind,
        CompactPhase::Reminded if rests => Step::Compact,
        CompactPhase::Ready if rests || paused => Step::Compact,
        CompactPhase::Compacting if paused => Step::Compact,
        _ => Step::Nothing,
    }
}

/// „Jetzt senden“ gibt es im Compact nicht.
pub(super) fn refuse_send_now(locked: bool) -> Result<(), CommandError> {
    if locked {
        Err(CommandError::Implementation(COMPACT_LOCKED.to_owned()))
    } else {
        Ok(())
    }
}

/// Die automatischen Wege (Plan, Station, Erinnerung des Wächters) gehen nie in eine Session im Compact;
/// nur das Fortsetzen einer pausierten (`SessionRegistry::send_resume`).
pub(super) fn refuses_start(locked: bool, status: SessionStatus, is_resume: bool) -> bool {
    locked && !(status == SessionStatus::Paused && is_resume)
}

/// Was die Regel wissen muss; jedes Feld ist eine Bedingung des Vertrags.
pub(super) struct Facts {
    /// Der Zug endete mit `completed`.
    pub(super) completed: bool,
    /// Das Zugende gehört zu einem Compact-Zug: Der löst keinen weiteren aus.
    pub(super) after_compact: bool,
    /// Master mindestens einer Umsetzung, die weder fertig noch abgebrochen ist.
    pub(super) master_of_open_implementation: bool,
    /// `rests()`: abgeschlossen, keine Rückfrage, kein Subagent, kein Skript.
    pub(super) rests: bool,
    pub(super) inbox_empty: bool,
    pub(super) context_used: u32,
    /// „Master zusammenfassen ab“; `None` = aus.
    pub(super) limit: Option<u32>,
    pub(super) mode: OperatingMode,
}

/// Soll die Session `/compact` bekommen? Nur die Claude-Kommandozeile (Claude oder LM Studio): Autark
/// fasst selbst zusammen, Codex lässt sich nicht auslösen.
pub(super) fn wanted(facts: &Facts) -> bool {
    facts.completed
        && !facts.after_compact
        && facts.master_of_open_implementation
        && facts.rests
        && facts.inbox_empty
        && above_limit(facts.context_used, facts.limit)
        && on_claude_command_line(facts.mode)
}

/// Master mindestens einer Umsetzung ohne Ende (weder fertig noch abgebrochen; ein Abbruch setzt auch
/// `finished_at`).
pub(super) fn leads_open_implementation<'a>(
    rows: impl IntoIterator<Item = &'a ImplementationRow>,
    session_id: &str,
) -> bool {
    rows.into_iter().any(|row: &ImplementationRow| {
        row.master_session_id.as_deref() == Some(session_id)
            && row.finished_at.is_none()
            && row.cancelled_at.is_none()
    })
}

/// Die billigen Bedingungen vor allem anderen: Master, Grenze und Betriebsart. Ohne Netz, ohne
/// Datenbank je Session; erst wenn die Regel greifen kann, kommt `prepare_turn` (LM Studio, Vorhaben).
pub(super) fn may_apply(master: bool, limit: Option<u32>, mode: OperatingMode) -> bool {
    master && limit.is_some() && on_claude_command_line(mode)
}

/// Die Grenze ist überschritten. Billig: nur der Kontext der Session, keine Datenbank.
pub(super) fn above_limit(context_used: u32, limit: Option<u32>) -> bool {
    limit.is_some_and(|limit: u32| context_used > limit)
}

fn on_claude_command_line(mode: OperatingMode) -> bool {
    matches!(mode, OperatingMode::Claude | OperatingMode::ClaudeCodeLocal)
}

impl SessionState {
    /// Der Zustand eines Compact-Zugs: Er läuft, aber es ist keine Eingabe. Kein Eintrag, kein
    /// `note_input`; `seen_at`, `auto_chain` und die Antwort des Dashboards bleiben.
    pub(super) fn begin_compact_turn(&mut self, outbox: &mut Outbox) {
        self.compact_turn = true;
        self.set_status(outbox, SessionStatus::Running);
    }

    /// Der Zug läuft: `start_turn` ruft das für jede Herkunft; der Compact-Zug fasst nichts anderes an.
    pub(super) fn enter_running(&mut self, outbox: &mut Outbox, is_compact: bool, by_hand: bool) {
        self.compact_by_hand = by_hand;
        if is_compact {
            self.begin_compact_turn(outbox);
        } else {
            self.compact_turn = false;
            self.set_status(outbox, SessionStatus::Running);
        }
    }

    /// Im Compact mit Vorbereitung: Es geht keine Nachricht in die Session.
    pub(super) fn compact_locked(&self) -> bool {
        self.compact_phase.is_some()
    }

    /// Die Session ruht im Compact und braucht einen Platz für den nächsten Schritt (Erinnerung oder
    /// Compact-Zug): Sie wartet dafür wie ein Fortsetzen (C9).
    pub(super) fn compact_waits_for_slot(&self) -> bool {
        self.status == SessionStatus::Completed
            && self.compact_phase.is_some_and(|phase: CompactPhase| {
                step_after_turn(phase, self.rests(), false) != Step::Nothing
            })
    }

    /// Seit wann der Compact-Wartende auf einen Platz wartet, oder `None`, wenn er nicht wartet. Der
    /// Beginn (erster Durchgang, der ihn sieht) bleibt je Phase fest; ohne ihn wäre der Wartende in jedem
    /// Durchgang der jüngste seiner Gruppe und käme nie vor eine später eingegangene Nachricht.
    pub(super) fn compact_waiting_since(&mut self, now: f64) -> Option<f64> {
        if !self.compact_waits_for_slot() {
            return None;
        }
        let phase = self.compact_phase?;
        match self.compact_due_at {
            Some((held, since)) if held == phase => Some(since),
            _ => {
                self.compact_due_at = Some((phase, now));
                Some(now)
            }
        }
    }

    /// `verwalter_compact_ready`: Die Session meldet sich bereit. Antwort an den Agenten.
    pub(super) fn call_compact_ready(&mut self, outbox: &mut Outbox) -> &'static str {
        match self.compact_phase {
            Some(CompactPhase::Preparing | CompactPhase::Reminded) => {
                self.compact_phase = Some(CompactPhase::Ready);
                outbox.summary_dirty = true;
                READY_REPLY
            }
            Some(CompactPhase::Ready) => READY_REPLY,
            Some(CompactPhase::Compacting) | None => NO_COMPACT_PENDING,
        }
    }

    /// Die Bitte ist hinaus: Der Empfang ist gesperrt.
    pub(super) fn begin_preparation(&mut self) {
        self.compact_phase = Some(CompactPhase::Preparing);
        self.compact_due_at = None;
        self.compact_abort_pending = false;
    }

    /// Der Vorbereitungszug endete ohne Meldung: Die Erinnerung geht hinaus.
    pub(super) fn begin_reminder(&mut self, outbox: &mut Outbox) {
        self.push_compact_note(outbox, "Zug beendet ohne Meldung");
        self.compact_phase = Some(CompactPhase::Reminded);
    }

    /// Der Compact-Zug beginnt. Kam er ohne Meldung, nennt der Chat es.
    pub(super) fn begin_compacting(&mut self, outbox: &mut Outbox) {
        if self.compact_phase == Some(CompactPhase::Reminded) {
            self.push_compact_note(
                outbox,
                "Wieder keine Meldung – der Verwalter fasst trotzdem zusammen",
            );
        }
        self.compact_phase = Some(CompactPhase::Compacting);
    }

    /// Die Umsetzung des Masters ist fertig oder abgebrochen: Ein Compact, der noch nicht läuft, ist nicht
    /// mehr nötig; ein laufender endet erst.
    pub(super) fn end_compact_unless_running(&mut self, outbox: &mut Outbox, reason: &str) {
        if self.compact_phase != Some(CompactPhase::Compacting) {
            self.release_compact(outbox, reason);
        }
    }

    /// „Jetzt zusammenfassen“: Der Nutzer gibt den Compact frei, ohne die Meldung abzuwarten. `true`,
    /// wenn die Phase wechselte.
    pub(super) fn compact_now(&mut self, outbox: &mut Outbox) -> bool {
        if !matches!(
            self.compact_phase,
            Some(CompactPhase::Preparing | CompactPhase::Reminded)
        ) {
            return false;
        }
        self.compact_phase = Some(CompactPhase::Ready);
        self.push_compact_note(outbox, "Jetzt zusammenfassen (von dir)");
        true
    }

    /// „Compact abbrechen“: Die Sperre geht weg. Ein Compact-Zug, der läuft, lässt sich nicht
    /// abbrechen.
    pub(super) fn abort_compact(&mut self, outbox: &mut Outbox) -> Result<(), CommandError> {
        match (self.compact_phase, self.status) {
            (None, _) => Ok(()),
            (Some(CompactPhase::Compacting), SessionStatus::Running | SessionStatus::Starting) => {
                Err(CommandError::Implementation(COMPACT_RUNNING.to_owned()))
            }
            (phase, status) => {
                // Läuft (oder ruht pausiert) ein Zug der Vorbereitung, löst dessen Ende keine neue Bitte
                // aus: Der Abbruch hält bis zum nächsten Ruhen nach einem anderen Zug.
                self.compact_abort_pending = status != SessionStatus::Completed
                    && matches!(
                        phase,
                        Some(
                            CompactPhase::Preparing | CompactPhase::Reminded | CompactPhase::Ready
                        )
                    );
                self.release_compact(outbox, "Compact abgebrochen (von dir)");
                Ok(())
            }
        }
    }

    /// Die Sperre geht weg; der Chat nennt den Grund. Ein Durchgang der Warteschlange spielt die
    /// wartenden Nachrichten ein (`update` sieht den Wechsel).
    pub(super) fn release_compact(&mut self, outbox: &mut Outbox, reason: &str) {
        if self.compact_phase.take().is_some() {
            self.push_compact_note(outbox, reason);
        }
    }

    pub(super) fn push_compact_note(&mut self, outbox: &mut Outbox, text: &str) {
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::Info {
                text: text.to_owned(),
            },
            at: None,
        });
        outbox.summary_dirty = true;
    }

    /// Ein Compact-Zug, der nicht abgeschlossen endet (Fehler, Kontingent, Abbruch): Die Session ruht
    /// wie vorher, ohne Ungelesen und ohne Karte (ein Master im Fehler gilt als ausgefallen, ADR 070).
    /// Der Chat nennt den Grund.
    pub(super) fn end_failed_compact(&mut self, outbox: &mut Outbox, reason: &str) {
        let (last_activity_at, seen_at) = (self.last_activity_at, self.seen_at);
        self.push_entry(outbox, |seq: u32| ChatEntry::Notice {
            seq,
            notice: Notice::Info {
                text: format!("Zusammenfassen gescheitert: {reason}"),
            },
            at: None,
        });
        self.pause_requested = false;
        // Gescheitert: Die Sperre geht trotzdem weg, die Nachrichten gehen hinaus.
        self.compact_phase = None;
        self.set_status(outbox, SessionStatus::Completed);
        self.last_activity_at = last_activity_at;
        self.seen_at = seen_at;
        outbox.turn_completed = true;
    }
}

/// Die Prüfung unter der Session-Sperre (Session vor Datenbank). `true`: Der Zug darf beginnen.
pub(super) fn decide_under_lock(
    session: &Session,
    state: &SessionState,
    master: bool,
    limit: Option<u32>,
    mode: OperatingMode,
) -> Result<bool, CommandError> {
    let inbox_empty = session
        .database
        .with(|connection| session_inbox::oldest(connection, &session.id))?
        .is_none();
    Ok(wanted(&Facts {
        completed: true,
        // Im Compact gibt es keine zweite Bitte: Die Phase endet erst mit dem Compact.
        after_compact: state.compact_phase.is_some(),
        master_of_open_implementation: master,
        rests: state.rests(),
        inbox_empty,
        context_used: state.context_used,
        limit,
        mode,
    }))
}

impl SessionRegistry {
    /// Die Datenbank ohne Session-Sperre.
    fn is_master_of_open_implementation(&self, session_id: &str) -> bool {
        self.database
            .with(|connection| implementation_rows::load_all(connection))
            .is_ok_and(|all: Vec<StoredImplementation>| {
                leads_open_implementation(all.iter().map(|stored| &stored.row), session_id)
            })
    }

    /// Alles vor dem Senden, ohne `AppHandle`: erst die billigen Bedingungen, dann `prepare`, dann das
    /// Tor der globalen Warteschlange. `Some`: Der Platz gehört der Session; der Aufrufer gibt ihn
    /// zurück (`release_session_slot`).
    pub(super) fn admit_compact<T>(
        &self,
        session_id: &str,
        master: bool,
        limit: Option<u32>,
        mode: OperatingMode,
        prepare: impl FnOnce() -> T,
    ) -> Option<T> {
        if !may_apply(master, limit, mode) {
            return None;
        }
        let prepared = prepare();
        self.claim_session_slot(session_id).then_some(prepared)
    }

    /// Unter der Grenze verlässt der Weg sich hier, vor Platz und `prepare`: ohne Netz und ohne
    /// Datenbank je Session.
    pub(super) fn admit_compact_over_limit<T>(
        &self,
        session_id: &str,
        context_used: u32,
        master: bool,
        limit: Option<u32>,
        mode: OperatingMode,
        prepare: impl FnOnce() -> T,
    ) -> Option<T> {
        if !above_limit(context_used, limit) {
            return None;
        }
        self.admit_compact(session_id, master, limit, mode, prepare)
    }

    /// Im eigenen Thread geprüft und geschickt, durch das Tor. Ohne Phase greift die Regel: Über der
    /// Grenze bekommt der Master in der Betriebsart Claude die Bitte, sich vorzubereiten (und der Empfang
    /// sperrt); die Kommandozeile mit LM Studio bekommt wie bisher gleich `/compact`. Ohne Platz entfällt
    /// die Bitte diesmal; in der Phase wartet der nächste Schritt auf einen Platz (C9).
    fn send_compact(&self, app: &AppHandle, session: &Arc<Session>) {
        let loaded = match settings::load(&self.database) {
            Ok(loaded) => loaded,
            Err(error) => {
                session.log_line(format!("Compact: Einstellungen nicht gelesen: {error}"));
                return;
            }
        };
        let limit = loaded.master_compact_tokens;
        // Vor der Session-Sperre: Datenbank und LM Studio gehören nicht darunter.
        let (phase, context_used) = {
            let state = session.lock();
            (state.compact_phase, state.context_used)
        };
        if phase.is_some() {
            self.advance_compact(app, session);
            return;
        }
        let prepares = loaded.operating_mode == OperatingMode::Claude;
        let text = if prepares {
            request_text(limit.unwrap_or_default())
        } else {
            COMPACT_TEXT.to_owned()
        };
        let master =
            above_limit(context_used, limit) && self.is_master_of_open_implementation(&session.id);
        let Some(input) = self.admit_compact_over_limit(
            &session.id,
            context_used,
            master,
            limit,
            loaded.operating_mode,
            || self.prepare_turn(session, &text),
        ) else {
            return;
        };
        let mut started = false;
        let sent = match &input.backend {
            Ok((mode, _)) => {
                let mode = *mode;
                update(
                    app,
                    session,
                    |state: &mut SessionState, outbox: &mut Outbox| {
                        if !decide_under_lock(session, state, master, limit, mode)? {
                            return Ok(());
                        }
                        started = true;
                        if prepares {
                            start_turn(
                                app,
                                session,
                                state,
                                outbox,
                                input,
                                &text,
                                TurnOrigin::Session {
                                    sender: &wake::sender(),
                                    reference: None,
                                    chat_subject: Some(REQUEST_SUBJECT),
                                },
                            )?;
                            // Erst nach dem Start: Scheitert er, gibt es keine Sperre ohne Bitte.
                            state.begin_preparation();
                        } else {
                            start_turn(
                                app,
                                session,
                                state,
                                outbox,
                                input,
                                COMPACT_TEXT,
                                TurnOrigin::Compact,
                            )?;
                        }
                        Ok(())
                    },
                )
            }
            Err(_) => Ok(()),
        };
        if let Err(error) = &sent {
            session.log_line(format!("Compact nicht gesendet: {error}"));
        }
        if self.give_back_slot(&session.id, started, sent.is_ok()) {
            schedule_pass(app);
        }
    }

    /// Der nächste Schritt einer Session im Compact nach dem Zugende (`step_after_turn`), mit einem
    /// Platz vom Tor. Ohne Platz bleibt die Phase, und der Durchgang führt die Session als Wartende
    /// (`compact_waits_for_slot`). Ohne offene Umsetzung geht die Sperre weg.
    fn advance_compact(&self, app: &AppHandle, session: &Arc<Session>) {
        if !self.is_master_of_open_implementation(&session.id) {
            self.end_compact_unless_running(app, session, END_OF_IMPLEMENTATION);
            return;
        }
        if !self.claim_session_slot(&session.id) {
            schedule_pass(app);
            return;
        }
        let started = self.start_phase_turn(app, session);
        if let Err(error) = &started {
            session.log_line(format!("Compact nicht fortgesetzt: {error}"));
            self.fail_compact(app, session, &error.to_string());
        }
        let started = started.is_ok_and(|started: bool| started);
        // Ein gestarteter Zug bewegt die Warteschlange mit seinem Ende; sonst ein Durchgang.
        if self.give_back_slot(&session.id, started, true) {
            schedule_pass(app);
        }
    }

    /// Der Durchgang hat der Session einen Platz gegeben, weil sie im Compact wartet (C9), oder sie ist
    /// pausiert mit scharfem Zeitgeber: Der Compact geht vor dem Fortsetzen.
    pub(super) fn resume_or_compact_claimed(&self, app: &AppHandle, session_id: &str) {
        let Ok(session) = self.get(session_id) else {
            return;
        };
        if !session.lock().compact_waits_for_slot() {
            self.resume_claimed(app, session_id);
            return;
        }
        if !self.is_master_of_open_implementation(session_id) {
            self.end_compact_unless_running(app, &session, END_OF_IMPLEMENTATION);
            return;
        }
        if let Err(error) = self.start_phase_turn(app, &session) {
            session.log_line(format!("Compact nicht fortgesetzt: {error}"));
            self.fail_compact(app, &session, &error.to_string());
        }
    }

    /// Startet den Zug des nächsten Schritts: nach der Bitte ohne Meldung die Erinnerung, nach der
    /// Meldung (auch nach der zweiten ausbleibenden) den Compact. `true`, wenn ein Zug begann. Ein
    /// pausierter Compact setzt hier fort. Ohne Platz geprüft: Wer einen Platz braucht, beansprucht ihn
    /// vorher.
    pub(super) fn start_phase_turn(
        &self,
        app: &AppHandle,
        session: &Arc<Session>,
    ) -> Result<bool, CommandError> {
        let step_of = |state: &SessionState| {
            state
                .compact_phase
                .map_or(Step::Nothing, |phase: CompactPhase| {
                    step_after_turn(phase, state.rests(), state.status == SessionStatus::Paused)
                })
        };
        let step = step_of(&session.lock());
        let text = match step {
            Step::Nothing => return Ok(false),
            Step::Remind => REMINDER_TEXT,
            Step::Compact => COMPACT_TEXT,
        };
        let input = self.prepare_turn(session, text);
        update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                // Noch einmal unter der Sperre: Zwischen Prüfung und Sperre kann sich vieles ändern.
                if step_of(state) != step {
                    return Ok(false);
                }
                match step {
                    Step::Remind => {
                        state.begin_reminder(outbox);
                        start_turn(
                            app,
                            session,
                            state,
                            outbox,
                            input,
                            text,
                            TurnOrigin::Session {
                                sender: &wake::sender(),
                                reference: None,
                                chat_subject: Some(REMINDER_SUBJECT),
                            },
                        )?;
                    }
                    Step::Compact => {
                        state.begin_compacting(outbox);
                        start_turn(
                            app,
                            session,
                            state,
                            outbox,
                            input,
                            text,
                            TurnOrigin::Compact,
                        )?;
                    }
                    Step::Nothing => return Ok(false),
                }
                Ok(true)
            },
        )
    }

    /// Der Compact kam nicht zustande (Start gescheitert): Die Sperre geht weg, die Nachrichten gehen
    /// hinaus, der Chat nennt den Grund wie bei einem gescheiterten Compact-Zug.
    fn fail_compact(&self, app: &AppHandle, session: &Arc<Session>, reason: &str) {
        let _ = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                if state.compact_phase.is_some() {
                    state.end_failed_compact(outbox, reason);
                }
                Ok(())
            },
        );
    }

    /// Die Umsetzung des Masters ist fertig oder abgebrochen: In der Vorbereitung ist kein Compact mehr
    /// nötig, die Sperre geht weg. Ein laufender Compact-Zug endet erst.
    fn end_compact_unless_running(&self, app: &AppHandle, session: &Arc<Session>, reason: &str) {
        let _ = update(
            app,
            session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.end_compact_unless_running(outbox, reason);
                Ok(())
            },
        );
    }

    /// Das Ende einer Umsetzung (fertig oder abgebrochen): Steht ihr Master im Compact, geht die Sperre
    /// weg. Nie unter einer Sperre aufrufen.
    pub fn end_compact_of_implementation(&self, app: &AppHandle, implementation_id: &str) {
        let master: Option<Arc<Session>> = self
            .database
            .with(|connection| implementation_rows::load(connection, implementation_id))
            .ok()
            .flatten()
            .and_then(|stored: StoredImplementation| stored.row.master_session_id)
            .and_then(|id: String| self.get(&id).ok());
        if let Some(master) = master {
            self.end_compact_unless_running(app, &master, END_OF_IMPLEMENTATION);
        }
    }

    /// „Jetzt zusammenfassen“: Der Nutzer gibt den Compact frei, ohne die Meldung abzuwarten. Ruht die
    /// Session gerade, geht es gleich weiter.
    pub fn compact_now(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| {
                state.compact_now(outbox);
                Ok(())
            },
        )?;
        if session.lock().compact_waits_for_slot() {
            schedule(app, &session);
        }
        Ok(())
    }

    /// „Compact abbrechen“: Die Sperre geht weg, ein Durchgang spielt die wartenden Nachrichten ein.
    pub fn compact_abort(&self, app: &AppHandle, session_id: &str) -> Result<(), CommandError> {
        let session = self.get(session_id)?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| state.abort_compact(outbox),
        )
    }

    /// `verwalter_compact_ready`: Antwort an den Agenten, ein Satz.
    pub fn tool_compact_ready(&self, app: &AppHandle, caller_id: &str) -> Result<String, String> {
        let session = self
            .get(caller_id)
            .map_err(|error: CommandError| error.to_string())?;
        update(
            app,
            &session,
            |state: &mut SessionState, outbox: &mut Outbox| Ok(state.call_compact_ready(outbox)),
        )
        .map(str::to_owned)
        .map_err(|error: CommandError| error.to_string())
    }

    /// Die eine Stelle, durch die der Compact-Weg den Platz zurückgibt. `true`: Es lief kein
    /// Compact-Zug, dessen Ende die Warteschlange bewegt (abgelehnt, oder der Start scheiterte);
    /// der Aufrufer stößt einen Durchgang an, sonst bliebe eine wartende Nachricht liegen, wenn
    /// der Durchgang des Zugendes in der Lücke lief.
    pub(super) fn give_back_slot(&self, session_id: &str, started: bool, sent_ok: bool) -> bool {
        self.release_session_slot(session_id);
        !(started && sent_ok)
    }
}

/// Nach dem Freigeben der Sperre: ein Thread, wie bei der Erinnerung des Wächters, weil `update` oft im
/// Lesethread des Agenten läuft.
pub(super) fn schedule(app: &AppHandle, session: &Arc<Session>) {
    let thread_app = app.clone();
    let thread_session = Arc::clone(session);
    let spawned = thread::Builder::new()
        .name("master-compact".to_owned())
        .spawn(move || {
            thread_app
                .state::<SessionRegistry>()
                .send_compact(&thread_app, &thread_session);
        });
    if let Err(error) = spawned {
        session.log_line(format!("Compact nicht gestartet: {error}"));
    }
}
