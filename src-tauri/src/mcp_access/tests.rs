//! Rechte der Konfigurationsdateien (Taiga 33).
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

use super::{McpAccess, ProcessKind};

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
