//! Der Modellsatz des Diktierens: wo er liegt, ob er fertig ist, und der geprüfte Download.
use std::fmt::{Display, Write as _};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};

use crate::error::CommandError;
use crate::filesystem::workspace::data_dir;

struct ModelFile {
    name: &'static str,
    url: &'static str,
    bytes: u64,
    sha256: &'static str,
}

pub const MODEL_DIR_NAME: &str = "parakeet-tdt-0.6b-v3";
// Auf einen festen Stand gepinnt: unter `main` könnten die Dateien sich ändern, und die Prüfsummen
// unten würden nie wieder stimmen. int8 statt der Vollgewichte: auf der CPU zulässig und ein Viertel
// so groß.
const MODEL_FILES: [ModelFile; 3] = [
    ModelFile {
        name: "encoder-model.int8.onnx",
        url: "https://huggingface.co/Discourse/parakeet-tdt-0.6b-v3-onnx/resolve/b4ee2464a71b0a705915856c351b1cc19340e6b6/encoder-model.int8.onnx",
        bytes: 652_183_999,
        sha256: "6139d2fa7e1b086097b277c7149725edbab89cc7c7ae64b23c741be4055aff09",
    },
    ModelFile {
        name: "decoder_joint-model.int8.onnx",
        url: "https://huggingface.co/Discourse/parakeet-tdt-0.6b-v3-onnx/resolve/b4ee2464a71b0a705915856c351b1cc19340e6b6/decoder_joint-model.int8.onnx",
        bytes: 18_202_004,
        sha256: "eea7483ee3d1a30375daedc8ed83e3960c91b098812127a0d99d1c8977667a70",
    },
    ModelFile {
        name: "vocab.txt",
        url: "https://huggingface.co/Discourse/parakeet-tdt-0.6b-v3-onnx/resolve/b4ee2464a71b0a705915856c351b1cc19340e6b6/vocab.txt",
        bytes: 102_132,
        sha256: "ba8e4007c65f4bb4358ffe2ecc13d9ccc7a10351151065242b5c3a943e685742",
    },
];
/// Das abgelöste Modell im Ordner der Modelle; nach dem Download des neuen Satzes gelöscht.
const LEGACY_MODEL_FILE_NAME: &str = "ggml-small-q5_1.bin";
const MODELS_DIR: &str = "models";
const PART_SUFFIX: &str = ".part";
const READ_CHUNK_BYTES: usize = 1024 * 1024;
const CANCELLED_TEXT: &str = "abgebrochen";

/// Legt nichts an.
pub fn model_dir(app: &tauri::AppHandle) -> Result<PathBuf, CommandError> {
    Ok(data_dir(app)?.join(MODELS_DIR).join(MODEL_DIR_NAME))
}

pub fn total_bytes() -> u64 {
    MODEL_FILES.iter().map(|file: &ModelFile| file.bytes).sum()
}

/// Nur die Größen werden verglichen: 670 MB bei jedem Status-Aufruf zu hashen dauert Sekunden. Die
/// Prüfsumme gilt beim Download; nur eine Datei, die dort bestanden hat, trägt den endgültigen Namen.
pub fn is_model_ready(dir: &Path) -> bool {
    MODEL_FILES
        .iter()
        .all(|file: &ModelFile| has_expected_size(&dir.join(file.name), file))
}

/// Lädt jede fehlende Datei nach `<name>.part`, prüft die SHA-256 und benennt erst dann um; Dateien
/// mit stimmender Größe werden übersprungen. Bei Fehler und Abbruch ist die gerade geladene `.part`
/// danach gelöscht, fertige Dateien bleiben für den nächsten Versuch liegen. Der Aufrufer erkennt
/// einen Abbruch an `cancel`, nicht am Fehlertext.
pub fn download(
    dir: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(u64, u64),
) -> Result<(), CommandError> {
    fs::create_dir_all(dir).map_err(download_error)?;
    let mut finished_bytes: u64 = 0;
    for file in &MODEL_FILES {
        let path = dir.join(file.name);
        if !has_expected_size(&path, file) {
            let part_path = part_path(&path);
            let result = download_file(file, &part_path, cancel, finished_bytes, &mut on_progress)
                .and_then(|()| fs::rename(&part_path, &path).map_err(download_error));
            if result.is_err() {
                // Ein Fehler beim Aufräumen ändert nichts mehr am Ergebnis.
                let _ = fs::remove_file(&part_path);
                return result;
            }
        }
        finished_bytes += file.bytes;
        on_progress(finished_bytes, total_bytes());
    }
    if let Some(models_dir) = dir.parent() {
        remove_legacy_model(models_dir);
    }
    Ok(())
}

/// Löscht genau das abgelöste Modell, sonst nichts.
pub fn remove_legacy_model(models_dir: &Path) {
    // Ein Fehler beim Aufräumen ändert nichts mehr am Ergebnis.
    let _ = fs::remove_file(models_dir.join(LEGACY_MODEL_FILE_NAME));
}

fn has_expected_size(path: &Path, file: &ModelFile) -> bool {
    fs::metadata(path).is_ok_and(|metadata: fs::Metadata| metadata.len() == file.bytes)
}

/// `offset` sind die Bytes der schon fertigen Dateien; der Fortschritt zählt über den ganzen Satz.
fn download_file(
    file: &ModelFile,
    part_path: &Path,
    cancel: &AtomicBool,
    offset: u64,
    on_progress: &mut impl FnMut(u64, u64),
) -> Result<(), CommandError> {
    let mut part_file = File::create(part_path).map_err(download_error)?;

    // `call()` macht aus einem HTTP-Status ab 400 einen Fehler (Voreinstellung von ureq, nicht
    // abschalten): sonst läge eine Fehlerseite als „Modell“ auf der Platte und fiele erst an der
    // Prüfsumme auf.
    let response = ureq::get(file.url).call().map_err(download_error)?;
    // Der Reader hat kein Größenlimit (anders als `read_to_vec`, das bei 10 MB abbricht).
    let mut reader = response.into_body().into_reader();

    let total_bytes = total_bytes();
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; READ_CHUNK_BYTES];
    let mut received_bytes: u64 = 0;
    loop {
        if cancel.load(Ordering::Relaxed) {
            return Err(CommandError::VoiceDownload(CANCELLED_TEXT.to_owned()));
        }
        let read_bytes = reader.read(&mut buffer).map_err(download_error)?;
        if read_bytes == 0 {
            break;
        }
        let chunk = &buffer[..read_bytes];
        part_file.write_all(chunk).map_err(download_error)?;
        hasher.update(chunk);
        received_bytes += read_bytes as u64;
        on_progress(offset + received_bytes, total_bytes);
    }
    part_file.flush().map_err(download_error)?;

    if hex(&hasher.finalize()) != file.sha256 {
        return Err(CommandError::VoiceDownload(format!(
            "Prüfsumme von {} stimmt nicht",
            file.name
        )));
    }
    Ok(())
}

fn part_path(path: &Path) -> PathBuf {
    let mut part = path.as_os_str().to_owned();
    part.push(PART_SUFFIX);
    PathBuf::from(part)
}

fn download_error(error: impl Display) -> CommandError {
    CommandError::VoiceDownload(error.to_string())
}

fn hex(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        // Schreiben in einen `String` schlägt nicht fehl.
        let _ = write!(text, "{byte:02x}");
    }
    text
}
