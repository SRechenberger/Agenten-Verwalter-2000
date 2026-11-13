pub mod account;
pub mod agent_types;
pub mod agents;
pub mod app_update;
pub mod archive;
pub mod artifacts;
pub mod attachments;
pub mod background;
pub mod changes;
pub mod cli_update;
pub mod codex;
pub mod codex_account;
pub mod commands;
pub mod context;
pub mod db;
pub mod environment;
pub mod error;
pub mod file_links;
pub mod filesystem;
pub mod git;
pub mod implementation;
pub mod lmstudio;
pub mod mcp;
pub mod mcp_access;
pub mod notices;
pub mod processes;
pub mod projects;
pub mod prompt_cache;
pub mod remote;
pub mod repositories;
pub mod retro;
pub mod review;
pub mod session_tools;
pub mod sessions;
pub mod settings;
pub mod side;
pub mod skills;
pub mod sourcing;
pub mod standalone;
pub mod terminal;
pub mod tldr;
pub mod usage;
pub mod voice;
pub mod worktrees;

use std::sync::Arc;

use tauri::{Emitter, Manager};

use account::AccountService;
use app_update::AppUpdateService;
use cli_update::CliUpdateService;
use codex::models::CodexService;
use codex_account::CodexAccountService;
use db::Database;
use environment::EnvironmentService;
use filesystem::workspace::data_dir;
use remote::RemoteControl;
use remote::client::{ClientHooks, RemoteClients};
use remote::dispatch::{self as remote_dispatch, AppHost};
use remote::server::{RemoteServer, ServerSetup};
use remote::wire::Timing;
use sessions::registry::SessionRegistry;
use terminal::TerminalService;
use usage::UsageService;
use voice::VoiceService;

const DATABASE_FILE: &str = "verwalter.db";

/// Server am Host und Verbindungen des Gastes (ADR 059). Der Server lauscht nur, wenn „Gäste
/// erlauben“ an ist; scheitert er, steht der Grund in seinem Status, und die App startet trotzdem.
fn start_remote(
    app: &tauri::AppHandle,
    database: &Arc<Database>,
    control: &Arc<RemoteControl>,
) -> Result<(), error::CommandError> {
    let folder = data_dir(app)?.join(remote::DIR_NAME);
    let emitter = app.clone();
    let notify: Arc<dyn Fn() + Send + Sync> = Arc::new(move || {
        let _ = emitter.emit(remote::CHANGED_EVENT, ());
    });
    let server = Arc::new(RemoteServer::new(ServerSetup {
        host_id: settings::remote_host_id(database)?,
        host_name: remote::host_name(),
        key_file: folder.join(remote::HOST_KEY_FILE),
        database: Arc::clone(database),
        control: Arc::clone(control),
        dispatch: {
            let host = AppHost { app: app.clone() };
            let control = Arc::clone(control);
            Arc::new(move |device, link, command: &str, args| {
                remote_dispatch::answer_on(&host, &control, device, link, command, args)
            })
        },
        notify: Arc::clone(&notify),
        timing: Timing::CONTRACT,
    }));
    let (enabled, port) = settings::remote_server(database)?;
    if enabled && let Err(error) = server.start(port) {
        eprintln!("Server der Fernsteuerung nicht gestartet: {error}");
    }
    remote::forward::start_host(app, Arc::clone(&server), Arc::clone(control));
    app.manage(server);
    let frames = app.clone();
    let clients = RemoteClients::load(
        folder.join(remote::HOSTS_FILE),
        Timing::CONTRACT,
        ClientHooks {
            changed: notify,
            frame: Arc::new(move |host_id: &str, frame| {
                remote::forward::on_guest_frame(&frames, host_id, frame);
            }),
        },
    );
    app.manage(Arc::new(clients));
    app.manage(remote::guest::GuestTerminals::default());
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // Ohne Datenbank startet die App nicht: ein stiller Weiterlauf verlöre jede Session beim Beenden.
            let database = Arc::new(Database::open(
                &data_dir(app.handle())?.join(DATABASE_FILE),
            )?);
            attachments::clear_staging(app.handle());
            let remote = Arc::new(RemoteControl::restore(&database)?);
            let registry =
                SessionRegistry::restore(app.handle(), Arc::clone(&database), Arc::clone(&remote))?;
            app.manage(registry);
            start_remote(app.handle(), &database, &remote)?;
            app.manage(remote);
            app.manage(database);
            app.manage(UsageService::new());
            app.manage(CodexService::new());
            app.manage(AccountService::new());
            app.manage(CodexAccountService::new());
            app.manage(CliUpdateService::new());
            app.manage(EnvironmentService::new());
            let app_update = AppUpdateService::new();
            if let Some(exe) = app_update.exe() {
                app_update::swap::cleanup(exe);
            }
            app.manage(app_update);
            app.manage(VoiceService::new());
            app.manage(TerminalService::new());
            SessionRegistry::start_reaper(app.handle().clone());
            // Ohne Server fehlen nur die Artefakte; `artifacts_list` meldet das.
            match artifacts::server::start(app.handle().clone()) {
                Ok(server) => {
                    app.manage(server);
                }
                Err(error) => eprintln!("Server für Artefakte nicht gestartet: {error}"),
            }
            // Ohne Zugang starten die Agenten ohne die Werkzeuge des Verwalters.
            match mcp_access::start(app.handle().clone()) {
                Ok(access) => {
                    app.manage(access);
                }
                Err(error) => eprintln!("MCP-Zugang nicht gestartet: {error}"),
            }
            SessionRegistry::deliver_inbox_on_start(app.handle().clone());
            Ok(())
        })
        .invoke_handler(remote::intercept::guarded(tauri::generate_handler![
            commands::app::app_info,
            commands::projects::project_list,
            commands::projects::project_create,
            commands::projects::project_rename,
            commands::projects::project_set_self_release,
            commands::projects::project_add_repository,
            commands::projects::project_archive,
            commands::projects::project_group_list,
            commands::projects::project_group_create,
            commands::projects::project_group_rename,
            commands::projects::project_group_delete,
            commands::projects::project_group_set_collapsed,
            commands::projects::project_set_group,
            commands::remote::remote_share_set,
            commands::remote::remote_server_status,
            commands::remote::remote_server_set,
            commands::remote::remote_invitation_create,
            commands::remote::remote_devices,
            commands::remote::remote_device_revoke,
            commands::remote::remote_connect,
            commands::remote::remote_hosts,
            commands::remote::remote_disconnect,
            commands::remote::remote_take,
            commands::remote::remote_give_back,
            commands::remote::remote_host_profile,
            commands::remote::remote_usage_load,
            commands::remote::remote_usage_refresh,
            commands::remote::remote_asset_base,
            commands::archive::archive_search,
            commands::archive::project_restore,
            commands::sessions::session_list,
            commands::sessions::session_overlaps,
            commands::sessions::session_create_in_project,
            commands::sessions::session_plan_vorhaben,
            commands::sessions::session_pause,
            commands::sessions::session_resume,
            commands::sessions::session_cancel,
            commands::sessions::session_rename,
            commands::sessions::session_delete,
            commands::sessions::session_set_viewed,
            commands::sessions::session_restart,
            commands::sessions::session_set_model,
            commands::sessions::session_set_auto_resume,
            commands::sessions::session_set_mode,
            commands::sessions::session_set_effort,
            commands::sessions::session_set_chrome,
            commands::sessions::session_set_codex_model,
            commands::sessions::session_log,
            commands::chat::chat_history,
            commands::chat::chat_send,
            commands::chat::chat_answer,
            commands::context::context_load,
            commands::context::context_refresh,
            commands::mcp::mcp_load,
            commands::mcp::mcp_refresh,
            commands::mcp::mcp_reconnect,
            commands::mcp::mcp_toggle,
            commands::mcp::mcp_authenticate,
            commands::usage::usage_load,
            commands::usage::usage_refresh,
            commands::account::account_load,
            commands::account::account_login,
            commands::cli_update::cli_update_load,
            commands::cli_update::cli_update_run,
            commands::app_update::app_update_load,
            commands::app_update::app_update_install,
            commands::attachments::attachment_add_files,
            commands::attachments::attachment_add_bytes,
            commands::attachments::attachment_discard,
            commands::skills::skill_list_for_session,
            commands::skills::skill_list_for_repositories,
            commands::skills::skill_uses,
            commands::background::background_load,
            commands::background::background_output,
            commands::background::background_stop,
            commands::background::scratchpad_list,
            commands::background::scratchpad_read,
            commands::tool_details::tool_detail,
            commands::repositories::repository_list,
            commands::repositories::repository_add,
            commands::repositories::repository_remove,
            commands::changes::changes_load,
            commands::changes::changes_file_diff,
            commands::changes::changes_file_lines,
            commands::git::git_status,
            commands::git::git_branches,
            commands::git::git_commit,
            commands::git::git_push,
            commands::git::git_pull,
            commands::git::git_fetch,
            commands::git::git_switch,
            commands::git::git_create_branch,
            commands::git::git_abort_operation,
            commands::git::git_discard,
            commands::git::git_stash_push,
            commands::git::git_stash_list,
            commands::git::git_stash_pop,
            commands::git::git_pull_rebase,
            commands::git::git_merge,
            commands::git::git_delete_branch,
            commands::git::git_create_ticket_worktree,
            commands::git::git_open,
            commands::git::git_log,
            commands::git::git_suggest_message,
            commands::file_links::file_link_open,
            commands::artifacts::artifacts_list,
            commands::artifacts::artifact_open_default,
            commands::artifacts::artifact_reveal,
            commands::side::side_ask,
            commands::side::side_load,
            commands::side::side_dismiss,
            commands::tldr::tldr_session_load,
            commands::tldr::tldr_session_create,
            commands::tldr::tldr_handoff_brief,
            commands::tldr::tldr_project_load,
            commands::tldr::tldr_project_create,
            commands::tldr::tldr_set_carry,
            commands::retro::retro_run,
            commands::implementation::implementation_list,
            commands::implementation::implementation_report,
            commands::implementation::implementation_new_inquiry,
            commands::implementation::implementation_rename,
            commands::implementation::implementation_set_auto_start,
            commands::implementation::implementation_set_stop_after_current,
            commands::implementation::implementation_set_max_parallel,
            commands::implementation::implementation_start_station,
            commands::implementation::implementation_set_station_agent,
            commands::implementation::implementation_set_station_options,
            commands::implementation::implementation_remove_brief_change,
            commands::implementation::implementation_cancel_countdown,
            commands::implementation::implementation_apply_change,
            commands::implementation::implementation_complete_station,
            commands::terminal::terminal_list,
            commands::terminal::terminal_open,
            commands::terminal::terminal_attach,
            commands::terminal::terminal_detach,
            commands::terminal::terminal_write,
            commands::terminal::terminal_resize,
            commands::terminal::terminal_close,
            commands::settings::settings_load,
            commands::settings::settings_update,
            commands::settings::settings_local_models,
            commands::agent_types::agent_types_list,
            commands::agent_types::agent_type_create,
            commands::agent_types::agent_type_update,
            commands::agent_types::agent_type_delete,
            commands::environment::environment_status,
            commands::environment::environment_mirror,
            commands::codex::codex_models,
            commands::codex_account::codex_account_load,
            commands::codex_account::codex_account_login,
            commands::voice::voice_model_status,
            commands::voice::voice_model_download,
            commands::voice::voice_model_cancel_download,
            commands::voice::voice_start,
            commands::voice::voice_stop,
            commands::voice::voice_cancel,
        ]))
        .build(tauri::generate_context!())
        .expect("Tauri-Laufzeit konnte nicht starten")
        .run(|app: &tauri::AppHandle, event: tauri::RunEvent| {
            // Kein Shell-Prozess überlebt die App.
            if let tauri::RunEvent::Exit = event
                && let Some(terminals) = app.try_state::<TerminalService>()
            {
                terminals.close_all();
            }
        });
}
