use tauri::AppHandle;

use crate::error::CommandError;
use crate::implementation::model::{ImplementationView, InquiryCreated, PhaseView, StationView};
use crate::implementation::{self, announce, changes, now_ms, start};
use crate::sessions::registry::SessionRegistry;

/// Alle Umsetzungen nicht archivierter Vorhaben, neueste zuerst (ADR 048). `async`, damit die
/// Datenbank nicht auf dem Haupt-Thread gelesen wird.
#[tauri::command]
pub async fn implementation_list(
    registry: tauri::State<'_, SessionRegistry>,
) -> Result<Vec<ImplementationView>, CommandError> {
    registry.implementation_views()
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

/// Schaltet „Automatisch weiter“. Aus: laufende Countdowns enden. An: Stationen, die starten
/// dürfen, bekommen einen Countdown.
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
        for station in view
            .phases
            .iter()
            .flat_map(|phase: &PhaseView| phase.stations.iter())
        {
            let _ = registry.cancel_countdown(&station.id);
        }
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
