//! Prüft den Modellsatz und die Erkennung des Diktierens über dieselben Bausteine wie die App. Aufruf:
//! `cargo run --release --manifest-path src-tauri/Cargo.toml --example parakeet-probe -- <Unterbefehl>`:
//! `download <Ordner> [--cancel-after <Sekunden>]` lädt den Modellsatz (fertige Dateien werden
//! übersprungen, `--cancel-after` setzt nach der Zeit das Abbruch-Zeichen); `ready <Ordner>` sagt, ob
//! der Satz vollständig ist; `recognize <Ordner> <WAV> [--reference "<Text>"]` erkennt die Aufnahme
//! und druckt Ladezeit, Erkennungszeit, Echtzeitfaktor, Text und Wortfehlerrate.
use std::env;
use std::path::Path;
use std::process::ExitCode;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::{Duration, Instant};

use verwalter_lib::voice::model_file::{download, is_model_ready, total_bytes};
use verwalter_lib::voice::resample::{TARGET_RATE, to_mono_16k};
use verwalter_lib::voice::transcribe::Transcriber;

const USAGE: &str = "Aufruf: parakeet-probe download <Ordner> [--cancel-after <Sekunden>] | ready <Ordner> | recognize <Ordner> <WAV> [--reference \"<Text>\"]";
const PROGRESS_STEP_BYTES: u64 = 100 * 1024 * 1024;
const BYTES_PER_MB: u64 = 1024 * 1024;

type ProbeResult = Result<ExitCode, Box<dyn std::error::Error>>;

fn main() -> ProbeResult {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let positional: Vec<&str> = arguments
        .iter()
        .take_while(|argument: &&String| !argument.starts_with("--"))
        .map(String::as_str)
        .collect();
    match positional.as_slice() {
        ["download", dir] => {
            let cancel_after = option_value(&arguments, "--cancel-after")
                .map(str::parse::<f64>)
                .transpose()?;
            run_download(Path::new(dir), cancel_after)
        }
        ["ready", dir] => {
            let is_ready = is_model_ready(Path::new(dir));
            println!("bereit: {}", if is_ready { "ja" } else { "nein" });
            Ok(ExitCode::SUCCESS)
        }
        ["recognize", dir, wav] => run_recognize(
            Path::new(dir),
            Path::new(wav),
            option_value(&arguments, "--reference"),
        ),
        _ => {
            eprintln!("{USAGE}");
            Ok(ExitCode::from(2))
        }
    }
}

fn option_value<'a>(arguments: &'a [String], name: &str) -> Option<&'a str> {
    let index = arguments
        .iter()
        .position(|argument: &String| argument == name)?;
    arguments.get(index + 1).map(String::as_str)
}

fn run_download(dir: &Path, cancel_after: Option<f64>) -> ProbeResult {
    let cancel = Arc::new(AtomicBool::new(false));
    if let Some(seconds) = cancel_after {
        let timer_cancel = Arc::clone(&cancel);
        thread::spawn(move || {
            thread::sleep(Duration::from_secs_f64(seconds));
            timer_cancel.store(true, Ordering::Relaxed);
        });
    }
    println!("vorher bereit: {}", is_model_ready(dir));
    let started = Instant::now();
    let mut next_report: u64 = 0;
    let result = download(dir, &cancel, |received_bytes: u64, total: u64| {
        if received_bytes >= next_report {
            println!(
                "  {} / {} MB",
                received_bytes / BYTES_PER_MB,
                total / BYTES_PER_MB
            );
            next_report = received_bytes + PROGRESS_STEP_BYTES;
        }
    });
    let elapsed_ms = started.elapsed().as_millis();
    match result {
        Ok(()) => {
            println!(
                "bereit nach {elapsed_ms} ms ({} Bytes im Satz)",
                total_bytes()
            );
            Ok(ExitCode::SUCCESS)
        }
        Err(error) => {
            let kind = if cancel.load(Ordering::Relaxed) {
                "abgebrochen"
            } else {
                "Fehler"
            };
            println!("{kind} nach {elapsed_ms} ms: {error}");
            Ok(ExitCode::FAILURE)
        }
    }
}

fn run_recognize(dir: &Path, wav: &Path, reference: Option<&str>) -> ProbeResult {
    let audio = read_mono_16k(wav)?;
    let seconds = audio.len() as f64 / f64::from(TARGET_RATE);
    println!("Aufnahme: {} ({seconds:.2} s)", wav.display());
    let started = Instant::now();
    let transcriber = Transcriber::load(dir)?;
    println!("Ladezeit: {} ms", started.elapsed().as_millis());
    let cancel = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let text = transcriber.transcribe(&audio, &cancel)?;
    let recognition = started.elapsed();
    println!("Erkennungszeit: {} ms", recognition.as_millis());
    println!("Echtzeitfaktor: {:.3}", recognition.as_secs_f64() / seconds);
    println!("Text: {text}");
    if let Some(reference) = reference {
        println!("Referenz: {reference}");
        println!(
            "Wortfehlerrate: {:.1} %",
            word_error_rate(reference, &text) * 100.0
        );
    }
    Ok(ExitCode::SUCCESS)
}

fn read_mono_16k(path: &Path) -> Result<Vec<f32>, Box<dyn std::error::Error>> {
    let mut reader = hound::WavReader::open(path)?;
    let spec = reader.spec();
    let samples: Vec<f32> = match spec.sample_format {
        hound::SampleFormat::Int => reader
            .samples::<i16>()
            .map(|sample: Result<i16, hound::Error>| {
                sample.map(|value: i16| f32::from(value) / 32768.0)
            })
            .collect::<Result<Vec<f32>, hound::Error>>()?,
        hound::SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<f32>, hound::Error>>()?,
    };
    Ok(to_mono_16k(&samples, spec.channels, spec.sample_rate))
}

fn normalized_words(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split_whitespace()
        .map(|word: &str| {
            word.chars()
                .filter(|character: &char| character.is_alphanumeric() || *character == '-')
                .collect::<String>()
        })
        .filter(|word: &String| !word.is_empty())
        .collect()
}

/// Levenshtein über Wörter (kleingeschrieben, ohne Satzzeichen), geteilt durch die Wörter der Referenz.
fn word_error_rate(reference: &str, hypothesis: &str) -> f64 {
    let expected = normalized_words(reference);
    let actual = normalized_words(hypothesis);
    if expected.is_empty() {
        return 0.0;
    }
    let mut previous: Vec<usize> = (0..=actual.len()).collect();
    for (row, expected_word) in expected.iter().enumerate() {
        let mut current = vec![row + 1; actual.len() + 1];
        for (column, actual_word) in actual.iter().enumerate() {
            let substitution = previous[column] + usize::from(expected_word != actual_word);
            current[column + 1] = substitution
                .min(previous[column + 1] + 1)
                .min(current[column] + 1);
        }
        previous = current;
    }
    previous[actual.len()] as f64 / expected.len() as f64
}
