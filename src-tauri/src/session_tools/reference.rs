//! Der Text, mit dem eine Nachricht beim Agenten ankommt (ADR 072): direkt oder als Betreff mit
//! Referenz `msg-<id>`. Die Zustellung (`sessions/registry/session_tools.rs`) und die Probe
//! `--wake-texts` bauen ihn mit denselben Funktionen; die erste Zeile setzt allein der Core.
use crate::agents::event::SessionRef;
use crate::implementation::wake;

/// Die erste Zeile jeder Nachricht einer Session.
fn header(sender: &SessionRef) -> String {
    format!(
        "[Nachricht von Session {} „{}“, ID {}]",
        sender.number, sender.title, sender.id
    )
}

/// Der ganze Text, wie bisher: in anderen Betriebsarten und für alle Texte ohne Betreff.
pub fn direct(sender: &SessionRef, text: &str) -> String {
    format!("{}\n\n{text}", header(sender))
}

/// Die drei Zeilen des Vertrags; den Wortlaut der dritten misst die Probe `--wake-texts`.
pub fn by_reference(sender: &SessionRef, subject: &str, id: i64) -> String {
    format!(
        "{}\nBetreff: {subject}\nInhalt: msg-{id} — lies ihn mit verwalter_read_message, bevor du handelst.",
        header(sender)
    )
}

/// „Gemeldet“: Nur die Notiz wird zur Referenz, der übrige Weck-Text bleibt.
pub fn by_note(sender: &SessionRef, text: &str, note: &str, id: i64) -> String {
    direct(
        sender,
        &text.replace(
            &wake::note_fragment(note),
            &format!("Notiz: msg-{id} (lies sie mit verwalter_read_message)"),
        ),
    )
}

/// Die erste Nachricht einer Station: Beginnt der Text mit `/<skill>`, ist diese Zeile der Befehl, der
/// allein vorn bleibt (die Kommandozeile erkennt ihn nur am Anfang); der Rest ist der Inhalt.
pub fn split_command(text: &str) -> (Option<&str>, &str) {
    if !text.starts_with('/') {
        return (None, text);
    }
    match text.split_once('\n') {
        Some((command, rest)) => (Some(command), rest),
        None => (Some(text), ""),
    }
}

/// Die erste Nachricht einer Station mit ihrem Befehl in der ersten Zeile.
pub fn station(command: Option<&str>, sender: &SessionRef, subject: &str, id: i64) -> String {
    let lines = by_reference(sender, subject, id);
    match command {
        Some(command) => format!("{command}\n{lines}"),
        None => lines,
    }
}
