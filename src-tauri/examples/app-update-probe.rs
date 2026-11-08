//! Prüft die Bausteine des App-Updates (ADR 041) ohne Datenbank und ohne laufende App. Aufruf:
//! `cargo run --manifest-path src-tauri/Cargo.toml --example app-update-probe -- <Modus>` mit
//! `check [--current <Version>]` (Status als JSON), `download <Ordner> [--current <Version>]` (lädt
//! die Datei der neuesten Version, prüft sie, prüft eine um ein Byte veränderte Kopie), `swap`
//! (tauscht eine laufende Kopie von `ping.exe`, Rücknahme, Neustart in einem Ordner mit
//! Leerzeichen), `pick` (Wahl der Datei auf selbst gebauten Antworten), `kind <Ordner>`.
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use verwalter_lib::app_update::model::{InstallKind, InstallPhase};
use verwalter_lib::app_update::releases::{self, Release};
use verwalter_lib::app_update::{self, download, swap};

const USAGE: &str = "Aufruf: app-update-probe check [--current <Version>] | download <Ordner> [--current <Version>] | swap | pick | kind <Ordner>";
const PROBE_VERSION: &str = "0.0.1";
const PING_SECONDS: &str = "30";
const RELAUNCH_WAIT: Duration = Duration::from_secs(10);

type ProbeResult = Result<ExitCode, Box<dyn std::error::Error>>;

fn main() -> ProbeResult {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let current = option_value(&arguments, "--current").unwrap_or(PROBE_VERSION);
    match arguments.first().map(String::as_str) {
        Some("check") => check(current),
        Some("download") => match arguments.get(1) {
            Some(dir) if !dir.starts_with("--") => download_newest(Path::new(dir), current),
            _ => usage(),
        },
        Some("swap") => swap_running(),
        Some("pick") => pick(),
        Some("kind") => match arguments.get(1) {
            Some(dir) => {
                let kind = swap::install_kind(&Path::new(dir).join("verwalter.exe"));
                println!("{kind:?}");
                Ok(ExitCode::SUCCESS)
            }
            None => usage(),
        },
        _ => usage(),
    }
}

fn usage() -> ProbeResult {
    eprintln!("{USAGE}");
    Ok(ExitCode::from(2))
}

fn option_value<'a>(arguments: &'a [String], name: &str) -> Option<&'a str> {
    let index = arguments
        .iter()
        .position(|argument: &String| argument == name)?;
    arguments.get(index + 1).map(String::as_str)
}

fn check(current: &str) -> ProbeResult {
    let exe = env::current_exe().ok();
    let kind = exe
        .as_deref()
        .map_or(InstallKind::Manual, swap::install_kind);
    let status = app_update::status(current, kind, exe.as_deref(), releases::fetch(current));
    println!("{}", serde_json::to_string_pretty(&status)?);
    Ok(ExitCode::SUCCESS)
}

fn download_newest(dir: &Path, current: &str) -> ProbeResult {
    let all = releases::fetch(current)?;
    let Some(newest) = releases::newer_than(&all, current).into_iter().next() else {
        eprintln!("Kein Release neuer als {current}.");
        return Ok(ExitCode::FAILURE);
    };
    let asset = releases::pick_asset(newest, InstallKind::Portable)?;
    fs::create_dir_all(dir)?;
    let dest = dir.join(&asset.name);
    let mut last_phase: Option<InstallPhase> = None;
    download::fetch_verified(asset, &dest, |phase, downloaded, total| {
        if last_phase != Some(phase) {
            eprintln!("{phase:?}: {downloaded} von {total:?} Bytes");
            last_phase = Some(phase);
        }
    })?;
    println!(
        "OK {} ({} Bytes, SHA-256 {:?})",
        dest.display(),
        asset.size,
        asset.sha256
    );

    let tampered = sibling_named(&dest, "verändert");
    let mut bytes = fs::read(&dest)?;
    let middle = bytes.len() / 2;
    if let Some(byte) = bytes.get_mut(middle) {
        *byte ^= 0xFF;
    }
    fs::write(&tampered, bytes)?;
    match download::verify(asset, &tampered) {
        Ok(()) => println!("FEHLER veränderte Datei besteht die Prüfung"),
        Err(message) => println!(
            "Veränderte Datei: „{message}“, danach vorhanden: {}",
            tampered.exists()
        ),
    }
    Ok(ExitCode::SUCCESS)
}

fn sibling_named(path: &Path, tag: &str) -> PathBuf {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    path.with_file_name(format!("{tag}-{name}"))
}

fn swap_running() -> ProbeResult {
    let system = PathBuf::from(env::var_os("SystemRoot").unwrap_or_else(|| "C:\\Windows".into()))
        .join("System32");
    let dir = env::temp_dir().join("app update probe");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(&dir)?;
    let target = dir.join("laufend.exe");
    let replacement = dir.join("laufend.exe.neu");
    fs::copy(system.join("PING.EXE"), &target)?;
    fs::copy(system.join("HOSTNAME.EXE"), &replacement)?;
    let old_bytes = fs::read(&target)?;
    let new_bytes = fs::read(&replacement)?;

    #[allow(
        clippy::disallowed_methods,
        reason = "Die Probe startet eine Kopie von PING.EXE, kein Programm des Verwalters"
    )]
    let mut running = Command::new(&target)
        .args(["-n", PING_SECONDS, "127.0.0.1"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;
    thread::sleep(Duration::from_millis(500));
    let swapped = swap::swap_files(&target, &replacement);
    let alt = swap::sibling(&target, swap::OLD_SUFFIX);
    println!(
        "Tausch mit laufender exe: {swapped:?}; Pfad = neue Datei: {}; .alt = alte Datei: {}",
        fs::read(&target)? == new_bytes,
        fs::read(&alt)? == old_bytes
    );

    swap::cleanup(&target);
    println!(
        "Aufräumen: .alt noch da (gesperrt durch laufende Kopie): {}",
        alt.exists()
    );
    let _ = running.kill();
    let _ = running.wait();

    // Rücknahme: `new` fehlt, das Einsetzen scheitert, die Datei am Pfad muss unverändert bleiben.
    let missing = dir.join("fehlt.exe");
    let failed = swap::swap_files(&target, &missing);
    println!(
        "Tausch ohne neue Datei: {failed:?}; Pfad unverändert: {}",
        fs::read(&target)? == new_bytes
    );

    swap::cleanup(&target);
    println!("Aufräumen nach Ende: .alt noch da: {}", alt.exists());

    relaunch_check(&dir)
}

/// `relaunch` startet über `start` eine Batch-Datei im Ordner mit Leerzeichen, die eine Marke
/// schreibt — belegt das Quoting der Befehlszeile.
fn relaunch_check(dir: &Path) -> ProbeResult {
    let marker = dir.join("neu gestartet.txt");
    let script = dir.join("neu starten.cmd");
    fs::write(&script, "@echo ok> \"%~dp0neu gestartet.txt\"\r\nexit\r\n")?;
    swap::relaunch(&script)?;
    let started = Instant::now();
    while !marker.exists() && started.elapsed() < RELAUNCH_WAIT {
        thread::sleep(Duration::from_millis(200));
    }
    println!(
        "Neustart über die Shell: Marke nach {} ms vorhanden: {}",
        started.elapsed().as_millis(),
        marker.exists()
    );
    Ok(ExitCode::SUCCESS)
}

fn pick() -> ProbeResult {
    let digest = format!("sha256:{}", "ab".repeat(32));
    let prefix = releases::DOWNLOAD_PREFIX;
    let cases: [(&str, String); 4] = [
        (
            "vollständig",
            asset_json(
                "x_1.0.0_x64-portable.exe",
                &format!("{prefix}v1.0.0/x"),
                Some(&digest),
            ),
        ),
        (
            "ohne digest",
            asset_json(
                "x_1.0.0_x64-portable.exe",
                &format!("{prefix}v1.0.0/x"),
                None,
            ),
        ),
        (
            "falsche Endung",
            asset_json(
                "x_1.0.0_amd64.deb",
                &format!("{prefix}v1.0.0/x"),
                Some(&digest),
            ),
        ),
        (
            "fremde Adresse",
            asset_json(
                "x_1.0.0_x64-portable.exe",
                "https://example.com/x.exe",
                Some(&digest),
            ),
        ),
    ];
    for (label, asset) in cases {
        let body = format!(
            r#"[{{"tag_name":"v1.0.0","draft":false,"prerelease":false,"body":null,"published_at":null,"html_url":"https://github.com/","assets":[{asset}]}}]"#
        );
        let parsed: Vec<Release> = releases::parse(&body)?;
        let Some(release) = parsed.first() else {
            println!("{label}: kein Release gelesen");
            continue;
        };
        match releases::pick_asset(release, InstallKind::Portable) {
            Ok(asset) => println!("{label}: OK {}", asset.name),
            Err(message) => println!("{label}: {message}"),
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn asset_json(name: &str, url: &str, digest: Option<&str>) -> String {
    let digest = digest.map_or_else(|| "null".to_owned(), |value: &str| format!("\"{value}\""));
    format!(r#"{{"name":"{name}","browser_download_url":"{url}","size":10,"digest":{digest}}}"#)
}
