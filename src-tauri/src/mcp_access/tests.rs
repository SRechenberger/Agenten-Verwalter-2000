//! Rechte der Konfigurationsdateien und Art der Schlüssel (Taiga 33 und 28).
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{Caller, McpAccess, ProcessKind, TOKEN_VARIABLE};

const SESSION: &str = "session-1";

/// Ein Zugang ohne Werkzeug-Sätze (`tool_sets()` zöge Tauri in das Testprogramm) und ohne Server.
fn access(name: &str) -> (McpAccess, PathBuf) {
    let dir = env::temp_dir().join(format!("verwalter-mcp-{}-{name}", std::process::id()));
    let _ = fs::remove_dir_all(&dir);
    let access = McpAccess {
        origin: "http://127.0.0.1:1".to_owned(),
        keys: Mutex::new(HashMap::new()),
        config_dir: dir.join("mcp"),
        tool_sets: Vec::new(),
    };
    (access, dir)
}

fn key_in(file: &PathBuf) -> String {
    let text = fs::read_to_string(file).expect("Datei gelesen");
    let json: serde_json::Value = serde_json::from_str(&text).expect("JSON");
    let header = json["mcpServers"]["verwalter"]["headers"]["Authorization"]
        .as_str()
        .expect("Kopf")
        .to_owned();
    header.trim_start_matches("Bearer ").to_owned()
}

fn caller(access: &McpAccess, key: &str) -> Caller {
    access.caller_of(key).expect("Schlüssel bekannt")
}

#[cfg(unix)]
fn mode(path: &PathBuf) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path).expect("Metadaten").permissions().mode() & 0o777
}

#[cfg(unix)]
#[test]
fn the_config_file_is_readable_for_the_owner_only_and_so_is_its_folder() {
    let (access, dir) = access("rights");
    let config = access
        .config_for(SESSION, ProcessKind::Main, false)
        .expect("Zugang")
        .config;
    assert_eq!(mode(&config), 0o600);
    assert_eq!(mode(&dir.join("mcp")), 0o700);
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn rewriting_a_file_with_wrong_rights_sets_them_right_again() {
    use std::os::unix::fs::PermissionsExt;
    let (access, dir) = access("rewrite");
    let config = access
        .config_for(SESSION, ProcessKind::Main, false)
        .expect("Zugang")
        .config;
    fs::set_permissions(&config, fs::Permissions::from_mode(0o644)).expect("Rechte gesetzt");
    fs::set_permissions(dir.join("mcp"), fs::Permissions::from_mode(0o755)).expect("Rechte");
    access.config_for(SESSION, ProcessKind::Main, false);
    assert_eq!(mode(&config), 0o600);
    assert_eq!(mode(&dir.join("mcp")), 0o700);
    let _ = fs::remove_dir_all(&dir);
}

#[cfg(unix)]
#[test]
fn the_side_question_file_is_private_too() {
    let (access, dir) = access("side-rights");
    let side = access
        .config_for(SESSION, ProcessKind::Side, false)
        .expect("Zugang")
        .config;
    assert_eq!(mode(&side), 0o600);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn without_session_tools_a_side_question_gets_its_own_file_and_a_side_key() {
    let (access, dir) = access("side-key");
    let main = access
        .config_for(SESSION, ProcessKind::Main, false)
        .expect("Hauptprozess");
    let side = access
        .config_for(SESSION, ProcessKind::Side, false)
        .expect("Nebenfrage");
    assert_ne!(main.config, side.config);
    let main_key = key_in(&main.config);
    let side_key = key_in(&side.config);
    assert_ne!(main_key, side_key);
    assert_eq!(caller(&access, &side_key).kind, ProcessKind::Side);
    assert_eq!(caller(&access, &main_key).kind, ProcessKind::Main);
    // Die Datei des Hauptprozesses bleibt, wie sie war: derselbe Schlüssel nach dem zweiten Aufruf.
    access.config_for(SESSION, ProcessKind::Side, false);
    assert_eq!(key_in(&main.config), main_key);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn with_session_tools_both_processes_share_the_file_and_differ_by_the_environment_key() {
    let (access, dir) = access("session-tools");
    let main = access
        .config_for(SESSION, ProcessKind::Main, true)
        .expect("Hauptprozess");
    let side = access
        .config_for(SESSION, ProcessKind::Side, true)
        .expect("Nebenfrage");
    assert_eq!(main.config, side.config);
    assert_eq!(key_in(&main.config), format!("${{{TOKEN_VARIABLE}}}"));
    let (main_token, side_token) = (
        main.token.expect("Schlüssel"),
        side.token.expect("Schlüssel"),
    );
    assert_ne!(main_token, side_token);
    assert_eq!(caller(&access, &side_token).kind, ProcessKind::Side);
    assert_eq!(caller(&access, &main_token).kind, ProcessKind::Main);
    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn forgetting_a_session_removes_both_files_and_every_key() {
    let (access, dir) = access("forget");
    let main = access
        .config_for(SESSION, ProcessKind::Main, false)
        .expect("Zugang");
    let side = access
        .config_for(SESSION, ProcessKind::Side, false)
        .expect("Zugang");
    let (main_key, side_key) = (key_in(&main.config), key_in(&side.config));
    access.forget(SESSION);
    assert!(!main.config.exists());
    assert!(!side.config.exists());
    assert!(access.caller_of(&main_key).is_none());
    assert!(access.caller_of(&side_key).is_none());
    let _ = fs::remove_dir_all(&dir);
}
