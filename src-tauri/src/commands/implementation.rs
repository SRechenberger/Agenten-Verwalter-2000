use std::sync::Arc;

use tauri::AppHandle;

use crate::error::CommandError;
use crate::implementation::model::{
    ImplementationView, InquiryCreated, PhaseView, PlanEdit, StationOptions, StationSettings,
    StationView, check_max_parallel,
};
use crate::implementation::{
    self, announce, changes, edit, has_begun, is_automatic, now_ms, refills_after_switch_off, start,
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
/// „Automatisch starten“ an ist; deren Warteschlange rückt in die frei gewordenen Plätze (ADR 061).
/// An: Stationen, die starten dürfen, bekommen einen Countdown.
#[tauri::command]
pub async fn implementation_set_auto_start(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    enabled: bool,
) -> Result<(), CommandError> {
    let view = registry.implementation_view(&id)?;
    registry.store_implementation_flags(&id, enabled, view.stop_after_current, view.finished_at)?;
    if enabled {
        start::schedule_ready(&app, &id)?;
    } else {
        for station in registry
            .implementation_view(&id)?
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
            .filter(|station: &&StationView| !station.starts_automatically)
        {
            let _ = registry.cancel_countdown(&station.id);
        }
        if refills_after_switch_off(&registry.implementation_view(&id)?) {
            start::schedule_ready(&app, &id)?;
        }
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// „Parallel max.“ (ADR 061): Grenze gleichzeitig aktiver Stationen, `None` = unbegrenzt. Plant die
/// Warteschlange neu, sobald die Umsetzung begonnen hat und die Automatik an ist.
#[tauri::command]
pub async fn implementation_set_max_parallel(
    app: AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    id: String,
    max_parallel: Option<u32>,
) -> Result<(), CommandError> {
    check_max_parallel(max_parallel).map_err(CommandError::Implementation)?;
    registry.implementation_view(&id)?;
    registry.store_max_parallel(&id, max_parallel)?;
    let view = registry.implementation_view(&id)?;
    if has_begun(&view)
        && is_automatic(&view)
        && !view.stop_after_current
        && view.finished_at.is_none()
    {
        start::schedule_ready(&app, &id)?;
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
}

/// „Anhalten nach dieser Session“: die nächste Fertigmeldung startet nichts mehr.
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
        start::restart_failed(&app, &id)?;
    }
    announce(&app, &registry.implementation_view(&id)?);
    Ok(())
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
