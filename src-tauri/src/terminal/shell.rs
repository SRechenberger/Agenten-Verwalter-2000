//! Welche Shell ein Terminal startet — je Betriebssystem (ADR 055).
use std::path::{Path, PathBuf};

pub struct ShellCommand {
    pub program: PathBuf,
    pub args: Vec<String>,
    pub label: String,
}

/// Der Wert der Umgebungsvariable; leer zählt wie fehlend.
fn env_program(name: &str, fallback: &str) -> PathBuf {
    match std::env::var_os(name) {
        Some(value) if !value.is_empty() => PathBuf::from(value),
        _ => PathBuf::from(fallback),
    }
}

fn label_of(program: &Path) -> String {
    let label: String = program
        .file_stem()
        .map(|stem| stem.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    if label.is_empty() {
        return "shell".to_owned();
    }
    label
}

fn command(program: PathBuf, args: &[&str]) -> ShellCommand {
    ShellCommand {
        label: label_of(&program),
        program,
        args: args
            .iter()
            .map(|argument: &&str| (*argument).to_owned())
            .collect(),
    }
}

#[cfg(windows)]
pub fn shell_command() -> ShellCommand {
    command(env_program("ComSpec", "cmd.exe"), &[])
}

#[cfg(target_os = "macos")]
pub fn shell_command() -> ShellCommand {
    command(env_program("SHELL", "/bin/zsh"), &["-l"])
}

#[cfg(all(unix, not(target_os = "macos")))]
pub fn shell_command() -> ShellCommand {
    command(env_program("SHELL", "/bin/bash"), &["-l"])
}
