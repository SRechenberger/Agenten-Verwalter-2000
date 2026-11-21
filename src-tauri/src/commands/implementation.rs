use std::sync::Arc;

use tauri::AppHandle;

use crate::error::CommandError;
use crate::implementation::model::{
    ImplementationView, InquiryCreated, PhaseView, PlanEdit, StationOptions, StationSettings,
    StationView,
};
use crate::implementation::slots::AgentQueue;
use crate::implementation::{
    self, ALREADY_FINISHED, SwitchEffect, after_auto_start_switch, announce, announce_removed,
    changes, edit, now_ms, replans_after_plan_change, start,
};
use crate::remote::client::RemoteClients;
use crate::remote::guest;
use crate::sessions::registry::SessionRegistry;

/// Alle Umsetzungen nicht archivierter Vorhaben, neueste zuerst (ADR 048). `async`, damit die
/// Datenbank nicht auf dem Haupt-Thread gelesen wird. Beim Gast kommen die Umsetzungen der Vorhaben
/// dazu, die er an einem Host hält (ADR 059).
#[tauri::command]
pub async fn implementation_list(
    registry: tauri::State<'_, SessionRegistry>,
    clients: tauri::State<'_, Arc<RemoteClients>>,
) -> Result<Vec<ImplementationView>, CommandError> {
    let mut views = registry.implementation_views()?;
    views.extend(guest::entries(Arc::clone(&clients), "implementation_list").await?);
    Ok(views)
}

/// Der Lagebericht als Text, wie der Agent ihn über `implementation_get` bekommt.
#[tauri::command]
pub async fn implementation_report(
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<String, CommandError> {
    let view = registry.implementation_view(&id)?;
    Ok(registry.report_text(&view, false, now_ms()))
}

/// „Neue Rückfrage“: eine frische Session im Vorhaben mit dem Lagebericht als Anhang (ADR 048).
#[tauri::command]
pub async fn implementation_new_inquiry(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<InquiryCreated, CommandError> {
    registry.new_inquiry(&app, &id)
}

/// Benennt die Umsetzung um, wie der Nutzer es will.
#[tauri::command]
pub async fn implementation_rename(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    name: String,
) -> Result<(), CommandError> {
    registry.rename_implementation(&id, &name)?;
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// Schaltet „Automatisch weiter“. Aus: laufende Countdowns enden, außer an Stationen, deren
/// „Automatisch starten“ an ist; in die frei gewordenen Plätze rückt die globale Warteschlange
/// (ADR 061, 068). An: In einer begonnenen Umsetzung warten Stationen, die starten dürfen, auf einen
/// Platz; eine nicht begonnene startet erst „Umsetzung starten“ oder ▶ (#51).
#[tauri::command]
pub async fn implementation_set_auto_start(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    let view = registry.implementation_view(&id)?;
    registry.store_implementation_flags(&id, enabled, view.stop_after_current, view.finished_at)?;
    if !enabled {
        for station in registry
            .implementation_view(&id)?
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .filter(|station: &&StationView| !station.starts_automatically)
        {
            let _ = registry.cancel_countdown(&station.id);
        }
    }
    match after_auto_start_switch(&registry.implementation_view(&id)?, enabled) {
        SwitchEffect::Schedule => {
            start::schedule_ready(&app, &id)?;
        }
        SwitchEffect::Refill => start::refill(&app),
        SwitchEffect::Nothing => {}
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// Die globale Warteschlange (ADR 068): Grenze, belegte Plätze und die Wartenden in der Reihenfolge,
/// in der sie nachrücken. Änderungen meldet das Ereignis `agent_queue://changed`.
#[tauri::command]
pub async fn agent_queue(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<AgentQueue, CommandError> {
    Ok(registry.agent_queue())
}

/// „Automatisch fortsetzen“ der Umsetzung nach aufgebrauchtem Kontingent (ADR 064): Die letzte Aktion
/// gewinnt, alle nicht fertigen Stationen folgen; die Zeitgeber pausierter Stationen gehen an bzw. aus.
#[tauri::command]
pub async fn implementation_set_auto_resume(
    app: AppHandle,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    implementation::set_auto_resume(&app, &id, enabled)
}

/// „Automatisch fortsetzen“ einer Station (ADR 064), `None` = wie die Umsetzung; bis sie fertig ist,
/// auch mit Session. Nur der Benutzer setzt ihn.
#[tauri::command]
pub async fn implementation_set_station_auto_resume(
    app: AppHandle,
    id: String,
    station_id: String,
    value: Option<bool>,
) -> Result<(), CommandError> {
    implementation::set_station_auto_resume(&app, &id, &station_id, value)
}

/// „Anhalten nach dieser Session“: die nächste Annahme startet nichts mehr.
#[tauri::command]
pub async fn implementation_set_stop_after_current(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    let view = registry.implementation_view(&id)?;
    registry.store_implementation_flags(&id, view.auto_start, enabled, view.finished_at)?;
    if !enabled {
        // Ein Neustart nach Fehler, den der Halt aufhielt, hat sonst keinen Auslöser mehr.
        start::refill(&app);
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// „Umsetzung starten“ (#135): „Automatisch starten“ an, die bereiten Stationen bis zur Grenze in den
/// Countdown, der Rest wartet; gesperrt lehnt er mit einem Satz ab.
#[tauri::command]
pub async fn implementation_start(app: AppHandle, id: String) -> Result<(), CommandError> {
    start::start_implementation(&app, &id)
}

/// ▶: startet die Station sofort, auch mit laufendem Countdown.
#[tauri::command]
pub async fn implementation_start_station(
    app: AppHandle,
    id: String,
    station_id: String,
) -> Result<(), CommandError> {
    start::start_now(&app, &id, &station_id)
}

/// „Abbrechen“ einer laufenden Befehls-Station (ADR 073): beendet den laufenden Schritt, die Station endet
/// im Fehler „abgebrochen“.
#[tauri::command]
pub async fn implementation_stop_command(
    app: AppHandle,
    id: String,
    station_id: String,
) -> Result<(), CommandError> {
    implementation::command_run::stop_command(&app, &id, &station_id)
}

/// „Einstellungen“ einer Station ohne Session (ADR 060): ersetzt Typ, Modell, Denkaufwand,
/// Zusatztext und Skill auf einmal; die Prüfung liegt im Core.
#[tauri::command]
pub async fn implementation_set_station_agent(
    app: AppHandle,
    id: String,
    station_id: String,
    settings: StationSettings,
) -> Result<(), CommandError> {
    implementation::set_station_agent(&app, &id, &station_id, &settings)
}

/// „Automatisch starten“, „Bei Fehler“ und „Vorgänger dürfen“ einer Station (ADR 061); nur der Benutzer
/// setzt sie. Nach dem Start bleibt nur „Bei Fehler“ änderbar, eine fertige Station ist nur lesbar.
#[tauri::command]
pub async fn implementation_set_station_options(
    app: AppHandle,
    id: String,
    station_id: String,
    options: StationOptions,
) -> Result<(), CommandError> {
    implementation::set_station_options(&app, &id, &station_id, &options)
}

/// „Entfernen“ einer Ergänzung bzw. „Zurücksetzen“ eines Ersatzes am Auftrag, bis zum Start (ADR 061).
#[tauri::command]
pub async fn implementation_remove_brief_change(
    app: AppHandle,
    id: String,
    station_id: String,
    change_id: String,
) -> Result<(), CommandError> {
    implementation::remove_brief_change(&app, &id, &station_id, &change_id)
}

/// „Abbrechen“: der Countdown endet, die Station bleibt bereit.
#[tauri::command]
pub async fn implementation_cancel_countdown(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    station_id: String,
) -> Result<(), CommandError> {
    announce(&app, &registry.cancel_countdown_of(&id, &station_id)?);
    // Der Countdown belegte einen Platz; der Durchgang lässt die gehaltene Station aus (ADR 068).
    start::refill(&app);
    Ok(())
}

/// „Übernehmen“ oder „Verwerfen“ des Vorschlags „Plan ändern?“.
#[tauri::command]
pub async fn implementation_apply_change(
    app: AppHandle,
    id: String,
    accept: bool,
) -> Result<(), CommandError> {
    changes::decide(&app, &id, accept)
}

/// „Abbrechen…“ (ADR 066): stoppt die arbeitenden Stationen, nichts startet mehr; danach lässt sich im
/// Vorhaben eine neue Umsetzung anlegen.
#[tauri::command]
pub async fn implementation_cancel(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<(), CommandError> {
    announce(&app, &registry.cancel_implementation(&app, &id)?);
    Ok(())
}

/// „Löschen…“ einer abgebrochenen oder nie begonnenen Umsetzung (ADR 066); Sessions und Worktrees
/// bleiben.
#[tauri::command]
pub async fn implementation_delete(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<(), CommandError> {
    // Die Zeile ist danach weg; das Vorhaben braucht das Ereignis für den Gast.
    let project_id = registry.implementation_view(&id)?.project_id;
    registry.delete_implementation(&app, &id)?;
    announce_removed(&app, &id, &project_id);
    Ok(())
}

/// „Bestätigen“ der Anfrage eines Agenten (ADR 066): abbrechen, bei `delete` danach löschen. Ist die
/// Umsetzung inzwischen regulär fertig, gibt es nichts mehr abzubrechen: Die Anfrage fällt weg.
/// `Some(Grund)`: abgebrochen, aber noch nicht löschbar (etwa ein Start, der seinen Anspruch noch
/// hält); „Löschen…“ geht danach.
#[tauri::command]
pub async fn implementation_apply_cancel(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<Option<String>, CommandError> {
    let view = registry.implementation_view(&id)?;
    let Some(pending) = view.pending_cancel else {
        return Err(CommandError::Implementation(
            "Es liegt keine Anfrage zum Abbrechen vor.".to_owned(),
        ));
    };
    if view.finished_at.is_some() {
        announce(&app, &registry.discard_cancel(&id)?);
        return Err(CommandError::Implementation(ALREADY_FINISHED.to_owned()));
    }
    announce(&app, &registry.cancel_implementation(&app, &id)?);
    if !pending.delete {
        return Ok(None);
    }
    // Abgebrochen ist schon; ein Fehler beim Löschen ist kein gescheitertes Bestätigen.
    match registry.delete_implementation(&app, &id) {
        Ok(()) => {
            announce_removed(&app, &id, &view.project_id);
            Ok(None)
        }
        Err(error) => Ok(Some(error.to_string())),
    }
}

/// „Verwerfen“ der Anfrage (ADR 066): alles wie vorher; was die Anfrage aufhielt, läuft weiter wie
/// nach einer Plan-Änderung (Neustart nach Fehler, Countdowns der bereiten Stationen).
#[tauri::command]
pub async fn implementation_discard_cancel(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
) -> Result<(), CommandError> {
    let view = registry.discard_cancel(&id)?;
    if replans_after_plan_change(&view) {
        start::schedule_ready(&app, &id)?;
    } else {
        start::refill(&app);
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// Der Benutzer ändert den Plan direkt (ADR 061, Werkzeuge): dasselbe Delta und dieselbe Prüfung wie
/// `implementation_edit`, ohne Karte, etwa Kanten-Chips oder „Neue Lane“. Plant danach neu.
#[tauri::command]
pub async fn implementation_edit_plan(
    app: AppHandle,
    id: String,
    edit: PlanEdit,
) -> Result<(), CommandError> {
    edit::apply(&app, &id, &edit, None)
}

/// „Erledigt“ für eine Station des Benutzers.
#[tauri::command]
pub async fn implementation_complete_station(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    station_id: String,
    result: String,
) -> Result<(), CommandError> {
    let view = registry.implementation_view(&id)?;
    let is_by_user: bool = view
        .phases
        .iter()
        .flat_map(|phase: &PhaseView| phase.stations.iter())
        .any(|station: &StationView| station.id == station_id && station.by_user);
    if !is_by_user {
        return Err(CommandError::Implementation(
            "Diese Station meldet der Agent selbst.".to_owned(),
        ));
    }
    implementation::complete_station(&app, &id, &station_id, &result, false)?;
    Ok(())
}

/// Der Benutzer markiert eine Station im Fehler oder in einer Pause ohne Zeitgeber als fertig (ADR 048).
#[tauri::command]
pub async fn implementation_mark_station_done(
    app: AppHandle,
    implementation_id: String,
    station_id: String,
    result: String,
) -> Result<(), CommandError> {
    implementation::mark_station_done(&app, &implementation_id, &station_id, &result)?;
    Ok(())
}

/// Der Benutzer bestimmt einen neuen Master (ADR 070, D11): eine Session des Vorhabens, keine Station.
/// Der neue Master bekommt eine Nachricht mit der Umsetzung und den offenen Meldungen.
#[tauri::command]
pub async fn implementation_set_master(
    app: AppHandle,
    id: String,
    master_session_id: String,
) -> Result<ImplementationView, CommandError> {
    implementation::set_master(&app, &id, &master_session_id)
}

/// Der Benutzer nimmt eine gemeldete Station an, wenn der Master seine drei Rückgaben verbraucht hat
/// (ADR 070, D4). Dieselben Schritte wie die Annahme durch den Master.
#[tauri::command]
pub async fn implementation_accept_station(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    station_id: String,
) -> Result<ImplementationView, CommandError> {
    implementation::accept_station(
        &app,
        &id,
        &station_id,
        implementation::master::Decider::User,
    )?;
    registry.implementation_view(&id)
}

/// Der Benutzer gibt eine gemeldete Station zurück, wenn der Master seine drei Rückgaben verbraucht hat,
/// ohne Grenze (ADR 070, D4).
#[tauri::command]
pub async fn implementation_return_station(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    station_id: String,
    message: String,
) -> Result<ImplementationView, CommandError> {
    implementation::return_station(
        &app,
        &id,
        &station_id,
        None,
        &message,
        implementation::master::Decider::User,
    )?;
    registry.implementation_view(&id)
}
