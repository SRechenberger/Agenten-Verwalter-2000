use tauri::Manager;

use crate::error::CommandError;
use crate::voice::VoiceService;
use crate::voice::model::VoiceModelState;

// `async`, damit der Dateizugriff nicht auf dem Haupt-Thread läuft.

#[tauri::command]
pub async fn voice_model_status(
    app: tauri::AppHandle,
    voice: tauri::State<'_, VoiceService>,
) -> Result<VoiceModelState, CommandError> {
    voice.model_state(&app)
}

/// Startet den Download im Hintergrund; der Fortschritt kommt als `voice://model`.
#[tauri::command]
pub async fn voice_model_download(
    app: tauri::AppHandle,
    voice: tauri::State<'_, VoiceService>,
) -> Result<(), CommandError> {
    voice.start_download(&app)
}

#[tauri::command]
pub async fn voice_model_cancel_download(
    voice: tauri::State<'_, VoiceService>,
) -> Result<(), CommandError> {
    voice.cancel_download();
    Ok(())
}

/// Startet ein Diktat; Pegel und Zwischentext kommen als `voice://level` und `voice://partial`.
#[tauri::command]
pub async fn voice_start(
    app: tauri::AppHandle,
    voice: tauri::State<'_, VoiceService>,
) -> Result<(), CommandError> {
    voice.start(app)
}

/// Kehrt erst zurück, wenn auch der letzte Abschnitt erkannt ist; liefert den gesamten Text.
#[tauri::command]
pub async fn voice_stop(app: tauri::AppHandle) -> Result<String, CommandError> {
    tauri::async_runtime::spawn_blocking(move || app.state::<VoiceService>().stop())
        .await
        .map_err(|error: tauri::Error| CommandError::Internal(format!("Diktat: {error}")))?
}

#[tauri::command]
pub async fn voice_cancel(voice: tauri::State<'_, VoiceService>) -> Result<(), CommandError> {
    voice.cancel();
    Ok(())
}
