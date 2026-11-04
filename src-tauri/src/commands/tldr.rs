use crate::agents::claude::print::{HaikuRequest, PrintProgram, ask_haiku, print_program};
use crate::error::CommandError;
use crate::sessions::registry::SessionRegistry;
use crate::tldr::brief::{BriefInput, HandoffSummary, fallback_text, summarized_text};
use crate::tldr::model::{HandoffBrief, ProjectTldrView, SessionTldrView};
use crate::tldr::prompt::{HANDOFF_SCHEMA, HANDOFF_SYSTEM_PROMPT, TLDR_TIMEOUT};

// `async`, damit das Laden eines Verlaufs aus der Datenbank nicht auf dem Haupt-Thread läuft.

#[tauri::command]
pub async fn tldr_session_load(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<SessionTldrView, CommandError> {
    registry.session_tldr_view(&session_id)
}

#[tauri::command]
pub async fn tldr_session_create(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<(), CommandError> {
    registry.start_session_tldr(&app, &session_id)
}

/// Die Übergabe für den Entwurf der Folgesession (ADR 054). Scheitert Haiku, kommt der Auszug
/// ohne Modell statt eines Fehlers; nur eine unbekannte oder verlaufslose Session ist einer.
#[tauri::command]
pub async fn tldr_handoff_brief(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
) -> Result<HandoffBrief, CommandError> {
    let input: BriefInput = registry.handoff_brief_input(&session_id)?;
    // Haiku braucht bis zu anderthalb Minuten — nicht auf dem Thread der übrigen Befehle.
    tauri::async_runtime::spawn_blocking(move || {
        let asked: Result<HandoffSummary, String> = print_program(&app)
            .map_err(|error: CommandError| error.to_string())
            .and_then(|program: PrintProgram| {
                let request = HaikuRequest {
                    system_prompt: HANDOFF_SYSTEM_PROMPT,
                    json_schema: HANDOFF_SCHEMA,
                    input: &input.transcript,
                    timeout: TLDR_TIMEOUT,
                };
                ask_haiku(&app, &program, &request)
            });
        match asked {
            Ok(summary) => HandoffBrief {
                text: summarized_text(&input, &summary),
                summarized: true,
                failure: None,
            },
            Err(failure) => HandoffBrief {
                text: fallback_text(&input, &failure),
                summarized: false,
                failure: Some(failure),
            },
        }
    })
    .await
    .map_err(|error| CommandError::Internal(error.to_string()))
}

#[tauri::command]
pub async fn tldr_project_load(
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
) -> Result<ProjectTldrView, CommandError> {
    registry.project_tldr_view(&project_id)
}

#[tauri::command]
pub async fn tldr_project_create(
    app: tauri::AppHandle,
    registry: tauri::State<'_, SessionRegistry>,
    project_id: String,
) -> Result<(), CommandError> {
    registry.start_project_tldr(&app, &project_id)
}

#[tauri::command]
pub async fn tldr_set_carry(
    registry: tauri::State<'_, SessionRegistry>,
    session_id: String,
    carry: bool,
) -> Result<(), CommandError> {
    registry.set_carry(&session_id, carry)
}
