//! Die Umgebung der Kindprozesse im AppImage: was der Helfer aus den Variablen herausnimmt und wie
//! er es auf `Command` und `CommandBuilder` anwendet. Die Umgebung ist synthetisch, das Bündel
//! heißt `/tmp/.mount_test`, und kein Test ruft `set_var`. Die Werte baut `join_paths`, damit sie
//! unter Windows (Trenner `;`) ebenso gelten wie unter Unix (Trenner `:`).
use std::collections::BTreeMap;
use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::*;

/// Das Bündel der synthetischen Umgebung; im echten AppImage `/tmp/.mount_<zufall>`.
const BUNDLE: &str = "/tmp/.mount_test";
/// Ein Programm des Systems, das nicht im Bündel liegt.
const SYSTEM_PROGRAM: &str = "/usr/bin/node";

/// Die Variablen, die im AppImage nur aus Einträgen des Bündels bestehen: Der ursprüngliche Wert,
/// den der Start hinten anhängt, war leer oder fehlte.
const BUNDLE_ONLY: [&str; 15] = [
    "GDK_PIXBUF_MODULE_FILE",
    "GIO_MODULE_DIR",
    "GI_TYPELIB_PATH",
    "GSETTINGS_SCHEMA_DIR",
    "GST_PLUGIN_SYSTEM_PATH",
    "GST_PLUGIN_SYSTEM_PATH_1_0",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GTK_PATH",
    "LD_LIBRARY_PATH",
    "PERLLIB",
    "PYTHONHOME",
    "PYTHONPATH",
    "QT_PLUGIN_PATH",
];

/// Die Einträge des Systems in `PATH` und `XDG_DATA_DIRS`, hinter denen der Start des AppImage den
/// ursprünglichen Wert anhängt.
const SYSTEM_PATH: [&str; 8] = [
    "/usr/local/sbin",
    "/usr/local/bin",
    "/usr/bin",
    "/usr/bin/site_perl",
    "/usr/bin/vendor_perl",
    "/usr/bin/core_perl",
    "/home/user/.cargo/bin",
    "/home/user/.local/bin",
];
const SYSTEM_DATA_DIRS: [&str; 5] = [
    "/home/user/.local/share/flatpak/exports/share",
    "/var/lib/flatpak/exports/share",
    "/usr/local/share",
    "/usr/share",
    "/home/user/.local/share",
];

fn os(text: &str) -> OsString {
    OsString::from(text)
}

/// Der Wert einer Listenvariablen aus ihren Einträgen.
fn list<S: AsRef<OsStr>>(entries: impl IntoIterator<Item = S>) -> OsString {
    env::join_paths(entries).expect("Einträge ohne Trenner lassen sich verbinden")
}

fn var<S: AsRef<OsStr>>(name: &str, entries: impl IntoIterator<Item = S>) -> (OsString, OsString) {
    (os(name), list(entries))
}

fn set<S: AsRef<OsStr>>(name: &str, entries: impl IntoIterator<Item = S>) -> EnvChange {
    EnvChange::Set(os(name), list(entries))
}

fn remove(name: &str) -> EnvChange {
    EnvChange::Remove(os(name))
}

fn no_changes() -> Vec<EnvChange> {
    Vec::new()
}

/// Die Änderungen für `vars` im Bündel `BUNDLE`.
fn changes(vars: Vec<(OsString, OsString)>) -> Vec<EnvChange> {
    bundle_free_env(vars, Path::new(BUNDLE))
}

/// `count` verschiedene Einträge im Bündel, etwa `/tmp/.mount_test/usr/lib3`.
fn in_bundle(count: usize) -> Vec<String> {
    (1..=count)
        .map(|index| format!("{BUNDLE}/usr/lib{index}"))
        .collect()
}

/// Mit einem leeren Eintrag am Ende bleibt eine Variable zurück, die vor dem Start des AppImage
/// fehlte: Der Start hängt `:<alter Wert>` an, und der alte Wert war leer.
fn with_empty_tail(mut entries: Vec<String>) -> Vec<String> {
    entries.push(String::new());
    entries
}

/// `count` Einträge im Bündel, danach die des Systems: So setzt der Start `PATH`.
fn bundle_then(count: usize, system: &[&str]) -> Vec<String> {
    let mut entries = in_bundle(count);
    entries.extend(system.iter().map(|entry: &&str| (*entry).to_owned()));
    entries
}

/// Der Wert einer Variablen aus `BUNDLE_ONLY`, in der Form, die im AppImage gemessen ist.
fn bundle_only_value(name: &str) -> OsString {
    match name {
        // Genau der Ordner des Bündels, kein Eintrag darin.
        "GTK_DATA_PREFIX" => list([BUNDLE]),
        "GSETTINGS_SCHEMA_DIR" => list(in_bundle(2)),
        "LD_LIBRARY_PATH" | "QT_PLUGIN_PATH" => list(with_empty_tail(in_bundle(10))),
        "PERLLIB" => list(with_empty_tail(in_bundle(2))),
        "GST_PLUGIN_SYSTEM_PATH" | "GST_PLUGIN_SYSTEM_PATH_1_0" | "PYTHONPATH" => {
            list(with_empty_tail(in_bundle(1)))
        }
        _ => list(in_bundle(1)),
    }
}

/// Die Umgebung, die der Start des AppImage hinterlässt (gemessen, hier ohne echte Werte): 18
/// Variablen zeigen in das Bündel, 15 bestehen nur daraus, `PATH` und `XDG_DATA_DIRS` behalten
/// Einträge des Systems, und `APPDIR` ist die Marke.
fn appimage_environment() -> Vec<(OsString, OsString)> {
    let mut environment: Vec<(OsString, OsString)> = BUNDLE_ONLY
        .into_iter()
        .map(|name: &str| (os(name), bundle_only_value(name)))
        .collect();
    environment.push(var("PATH", bundle_then(5, &SYSTEM_PATH)));
    environment.push(var("XDG_DATA_DIRS", bundle_then(2, &SYSTEM_DATA_DIRS)));
    environment.push(var("APPDIR", [BUNDLE]));
    environment.push(var("APPIMAGE", ["/home/user/Verwalter.AppImage"]));
    environment.push(var("OWD", ["/home/user"]));
    environment.push(var("HOME", ["/home/user"]));
    environment.push(var("LANG", ["de_DE.UTF-8"]));
    environment
}

/// Die Variablen eines `Command`, die ausdrücklich gesetzt oder entfernt sind (`None` heißt
/// entfernt); alles andere erbt der Kindprozess.
fn explicit_env(command: &Command) -> BTreeMap<OsString, Option<OsString>> {
    command
        .get_envs()
        .map(|(name, value): (&OsStr, Option<&OsStr>)| {
            (name.to_owned(), value.map(OsStr::to_owned))
        })
        .collect()
}

/// Die ganze Umgebung eines `CommandBuilder`, geerbte Variablen eingeschlossen.
fn full_env(builder: &CommandBuilder) -> BTreeMap<String, String> {
    builder
        .iter_full_env_as_str()
        .map(|(name, value): (&str, &str)| (name.to_owned(), value.to_owned()))
        .collect()
}

#[test]
fn without_app_dir_nothing_changes() {
    let vars = vec![var("LD_LIBRARY_PATH", [BUNDLE, ""])];
    assert_eq!(
        changes_for(OsStr::new(SYSTEM_PROGRAM), None, vars),
        no_changes()
    );
}

#[test]
fn empty_app_dir_counts_as_not_set() {
    let vars = || {
        vec![
            var("PATH", [BUNDLE, "/usr/bin"]),
            var("PYTHONHOME", ["/usr"]),
        ]
    };
    // Ein leerer Pfad ist Vorfahr jedes Pfads: Ohne die Prüfung verschwänden alle Variablen.
    assert_eq!(bundle_free_env(vars(), Path::new("")), no_changes());
    assert_eq!(
        changes_for(OsStr::new(SYSTEM_PROGRAM), Some(OsStr::new("")), vars()),
        no_changes()
    );
}

#[test]
fn list_variable_keeps_the_system_entries_in_order() {
    let vars = vec![var(
        "PATH",
        [
            "/tmp/.mount_test/usr/bin/",
            "/usr/local/bin",
            "/tmp/.mount_test/usr/sbin/",
            "/usr/bin",
            "/bin",
        ],
    )];
    assert_eq!(
        changes(vars),
        [set("PATH", ["/usr/local/bin", "/usr/bin", "/bin"])]
    );
}

#[test]
fn variable_of_bundle_entries_only_is_removed() {
    let vars = vec![var(
        "PYTHONPATH",
        [
            "/tmp/.mount_test/usr/share/pyshared/",
            "/tmp/.mount_test/usr/lib/python",
        ],
    )];
    assert_eq!(changes(vars), [remove("PYTHONPATH")]);
}

#[test]
fn empty_entry_left_by_the_launcher_does_not_keep_the_variable() {
    let vars = vec![var(
        "LD_LIBRARY_PATH",
        ["/tmp/.mount_test/usr/lib/", "/tmp/.mount_test/lib/", ""],
    )];
    assert_eq!(changes(vars), [remove("LD_LIBRARY_PATH")]);
}

#[test]
fn empty_entry_among_system_entries_stays() {
    // Der leere Eintrag gehört hier zum ursprünglichen Wert (`/usr/bin:`): Wer die Einträge des
    // Bündels herausnimmt, bekommt genau diesen Wert zurück.
    let vars = vec![var("PATH", ["/tmp/.mount_test/usr/bin/", "/usr/bin", ""])];
    assert_eq!(changes(vars), [set("PATH", ["/usr/bin", ""])]);
}

#[test]
fn entry_equal_to_the_bundle_folder_is_removed() {
    let vars = vec![
        var("GTK_DATA_PREFIX", [BUNDLE]),
        var("XDG_DATA_DIRS", [BUNDLE, "/usr/share"]),
        var("GTK_EXE_PREFIX", ["/tmp/.mount_test/"]),
    ];
    assert_eq!(
        changes(vars),
        [
            remove("GTK_DATA_PREFIX"),
            set("XDG_DATA_DIRS", ["/usr/share"]),
            remove("GTK_EXE_PREFIX"),
        ]
    );
}

#[test]
fn folder_that_only_shares_the_name_prefix_stays() {
    // `/tmp/.mount_ab` ist ein Namensvorsatz von `/tmp/.mount_abc`, aber kein Vorfahr.
    let vars = vec![
        var("PATH", ["/tmp/.mount_abc/usr/bin", "/usr/bin"]),
        var("QT_PLUGIN_PATH", ["/tmp/.mount_abc/plugins"]),
    ];
    assert_eq!(
        bundle_free_env(vars, Path::new("/tmp/.mount_ab")),
        no_changes()
    );
}

#[test]
fn launcher_marks_stay_untouched() {
    // Alle vier zeigen hier in das Bündel: Es zählt der Name, nicht der Wert.
    let vars = vec![
        var("APPDIR", [BUNDLE]),
        var("APPIMAGE", ["/tmp/.mount_test/Verwalter.AppImage"]),
        var("OWD", ["/tmp/.mount_test/work"]),
        var("ARGV0", ["/tmp/.mount_test/usr/bin/verwalter"]),
    ];
    assert_eq!(changes(vars), no_changes());
}

#[test]
fn variable_without_bundle_entries_is_not_listed() {
    let vars = vec![
        var("PATH", ["/usr/local/bin", "/usr/bin"]),
        var("HOME", ["/home/user"]),
        var("LD_LIBRARY_PATH", [""]),
    ];
    assert_eq!(changes(vars), no_changes());
}

#[test]
fn observed_environment_of_the_appimage_gives_15_removals_and_2_replacements() {
    let environment = appimage_environment();
    // Die Form stimmt mit der Messung überein: 18 Variablen zeigen in das Bündel, `APPDIR` mit.
    let pointing_into_bundle = environment
        .iter()
        .filter(|(_, value): &&(OsString, OsString)| {
            env::split_paths(value).any(|entry| entry.starts_with(BUNDLE))
        })
        .count();
    assert_eq!(pointing_into_bundle, 18);

    let mut expected: Vec<EnvChange> = BUNDLE_ONLY.into_iter().map(remove).collect();
    expected.push(set("PATH", SYSTEM_PATH));
    expected.push(set("XDG_DATA_DIRS", SYSTEM_DATA_DIRS));
    assert_eq!(changes(environment), expected);
}

#[test]
fn program_inside_the_bundle_keeps_the_whole_environment() {
    // Die eigene Exe des Verwalters (Autark, Codex) ist mit den Bibliotheken des Bündels gebaut.
    let own_exe = format!("{BUNDLE}/usr/bin/verwalter");
    assert_eq!(
        changes_for(
            OsStr::new(&own_exe),
            Some(OsStr::new(BUNDLE)),
            appimage_environment()
        ),
        no_changes()
    );
}

#[test]
fn program_outside_the_bundle_gets_the_changes() {
    let expected = changes(appimage_environment());
    assert!(!expected.is_empty());
    for program in [
        SYSTEM_PROGRAM,
        "node",
        "git",
        // Ein Nachbarordner mit gleichem Namensvorsatz liegt nicht im Bündel.
        "/tmp/.mount_testing/usr/bin/verwalter",
    ] {
        assert_eq!(
            changes_for(
                OsStr::new(program),
                Some(OsStr::new(BUNDLE)),
                appimage_environment()
            ),
            expected,
            "{program}"
        );
    }
}

#[test]
fn command_adapter_removes_and_sets_exactly_the_listed_variables() {
    let mut child = command("program");
    let mut expected = explicit_env(&child);
    apply_to_command(
        &mut child,
        &[
            remove("TEST_REMOVED"),
            set("TEST_REPLACED", ["/usr/bin", "/bin"]),
        ],
    );
    expected.insert(os("TEST_REMOVED"), None);
    expected.insert(os("TEST_REPLACED"), Some(list(["/usr/bin", "/bin"])));
    assert_eq!(explicit_env(&child), expected);
}

#[test]
fn pty_adapter_removes_and_sets_exactly_the_listed_variables() {
    let mut builder = pty_command("program");
    builder.env("TEST_REMOVED", "old");
    builder.env("TEST_REPLACED", "old");
    apply_to_pty(
        &mut builder,
        &[
            remove("TEST_REMOVED"),
            set("TEST_REPLACED", ["/usr/bin", "/bin"]),
            set("TEST_ADDED", ["/opt/bin"]),
        ],
    );
    let replaced = list(["/usr/bin", "/bin"]);
    assert_eq!(builder.get_env("TEST_REMOVED"), None);
    assert_eq!(builder.get_env("TEST_REPLACED"), Some(replaced.as_os_str()));
    assert_eq!(builder.get_env("TEST_ADDED"), Some(OsStr::new("/opt/bin")));
}

#[test]
fn adapter_leaves_other_variables_inherited() {
    // Terminal: Der Builder trägt die Umgebung des Verwalters; außer den genannten Namen bleibt
    // sie, wie sie war.
    let mut builder = pty_command("program");
    builder.env("TEST_UNRELATED", "kept");
    builder.env("TEST_REMOVED", "old");
    let mut expected = full_env(&builder);
    apply_to_pty(
        &mut builder,
        &[remove("TEST_REMOVED"), set("TEST_REPLACED", ["/usr/bin"])],
    );
    expected.remove("TEST_REMOVED");
    expected.insert("TEST_REPLACED".to_owned(), "/usr/bin".to_owned());
    assert_eq!(full_env(&builder), expected);

    // Kommandozeile: Der Kindprozess erbt, was nicht genannt ist; `env_clear` ließe `PATH` fehlen.
    #[cfg(unix)]
    {
        let mut child = command("env");
        apply_to_command(&mut child, &[remove("TEST_REMOVED")]);
        let output = child.output().expect("env startet");
        assert!(output.status.success());
        let printed = String::from_utf8_lossy(&output.stdout);
        assert!(
            printed.lines().any(|line: &str| line.starts_with("PATH=")),
            "Der Kindprozess hat PATH nicht geerbt"
        );
    }
}

#[cfg(unix)]
#[test]
fn entry_that_is_not_valid_unicode_is_kept() {
    use std::os::unix::ffi::OsStringExt;

    // Latin-1 „é“ (0xE9) ist kein UTF-8.
    let foreign = OsString::from_vec(b"/opt/caf\xe9/bin".to_vec());
    let foreign_in_bundle = OsString::from_vec(b"/tmp/.mount_test/caf\xe9".to_vec());
    let vars = vec![
        (
            os("PATH"),
            list([
                os("/tmp/.mount_test/usr/bin"),
                foreign.clone(),
                os("/usr/bin"),
            ]),
        ),
        (os("QT_PLUGIN_PATH"), list([foreign_in_bundle])),
    ];
    assert_eq!(
        changes(vars),
        [
            EnvChange::Set(os("PATH"), list([foreign, os("/usr/bin")])),
            remove("QT_PLUGIN_PATH"),
        ]
    );
}
