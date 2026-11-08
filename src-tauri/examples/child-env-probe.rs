//! Zeigt, was der Helfer `processes::command` in der echten Umgebung tut, ohne Datenbank und ohne
//! laufende App. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example child-env-probe -- show | run <Programm> [Argumente …]`.
//! `show` druckt für die Umgebung dieses Prozesses je Zeile `REMOVE <Name>` oder
//! `SET <Name> <Anzahl bleibender Einträge>`, nie einen Wert; ohne `APPDIR` die Zeile
//! `KEINE (APPDIR nicht gesetzt)`. `run` startet das Programm über `processes::command`, wartet,
//! druckt `EXIT <Code>`, danach seine Ausgabe (Standardausgabe, dann Fehlerausgabe) und endet mit
//! demselben Code.
use std::env;
use std::ffi::{OsStr, OsString};
use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use verwalter_lib::processes::{self, EnvChange};

const USAGE: &str = "Aufruf: child-env-probe show | run <Programm> [Argumente …]";

fn main() -> ExitCode {
    let arguments: Vec<OsString> = env::args_os().skip(1).collect();
    match arguments.split_first() {
        Some((word, [])) if word == "show" => show(),
        Some((word, [program, rest @ ..])) if word == "run" => run(program, rest),
        _ => {
            eprintln!("{USAGE}");
            ExitCode::from(2)
        }
    }
}

/// Was der Helfer an der Umgebung dieses Prozesses ändern würde: nur Namen und Zahlen.
fn show() -> ExitCode {
    let Some(app_dir) = env::var_os("APPDIR").filter(|dir: &OsString| !dir.is_empty()) else {
        println!("KEINE (APPDIR nicht gesetzt)");
        return ExitCode::SUCCESS;
    };
    let changes = processes::bundle_free_env(env::vars_os(), Path::new(&app_dir));
    if changes.is_empty() {
        println!("KEINE (keine Variable zeigt in das Bündel)");
    }
    for change in changes {
        match change {
            EnvChange::Remove(name) => println!("REMOVE {}", name.to_string_lossy()),
            EnvChange::Set(name, value) => println!(
                "SET {} {}",
                name.to_string_lossy(),
                env::split_paths(&value).count()
            ),
        }
    }
    ExitCode::SUCCESS
}

/// Startet das Programm über den Helfer, wartet und gibt seinen Exit-Code weiter.
fn run(program: &OsStr, arguments: &[OsString]) -> ExitCode {
    let output = match processes::command(program).args(arguments).output() {
        Ok(output) => output,
        Err(error) => {
            eprintln!(
                "FEHLER {} startet nicht: {error}",
                program.to_string_lossy()
            );
            return ExitCode::from(127);
        }
    };
    // Ohne Code hat ein Signal das Programm beendet.
    let code = output.status.code();
    println!(
        "EXIT {}",
        code.map_or_else(|| "Signal".to_owned(), |code: i32| code.to_string())
    );
    let _ = io::stdout().write_all(&output.stdout);
    let _ = io::stderr().write_all(&output.stderr);
    code.map_or(ExitCode::FAILURE, |code: i32| {
        ExitCode::from(u8::try_from(code & 0xff).unwrap_or(1))
    })
}
