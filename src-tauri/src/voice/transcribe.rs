//! Erkennung eines Abschnitts mit Parakeet (ONNX).
use std::num::NonZero;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use parakeet_rs::{ExecutionConfig, ParakeetTDT, Transcriber as _};

use super::resample::TARGET_RATE;
use crate::error::CommandError;

const MAX_THREADS: usize = 8;
const MONO: u16 = 1;

/// `transcribe_samples` braucht `&mut self`; ein auslaufender abgebrochener Durchlauf hält die Sperre
/// bis zu seinem Ende.
pub struct Transcriber {
    model: Mutex<ParakeetTDT>,
}

impl Transcriber {
    pub fn load(dir: &Path) -> Result<Transcriber, CommandError> {
        // Ohne eigene ONNX-Umgebung lädt `from_pretrained` nichts.
        let environment = parakeet_rs::ort::init().build().map_err(model_error)?;
        let config = ExecutionConfig::new()
            .with_environment(environment)
            .with_intra_threads(thread_count());
        let model = ParakeetTDT::from_pretrained(dir, Some(config)).map_err(model_error)?;
        Ok(Transcriber {
            model: Mutex::new(model),
        })
    }

    /// Erwartet 16 kHz mono. Leeres Ergebnis → `NoSpeech`, abgebrochen → `VoiceCancelled`.
    pub fn transcribe(
        &self,
        audio: &[f32],
        cancel: &Arc<AtomicBool>,
    ) -> Result<String, CommandError> {
        if cancel.load(Ordering::Relaxed) {
            return Err(CommandError::VoiceCancelled);
        }
        let result = self
            .model
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .transcribe_samples(audio.to_vec(), TARGET_RATE, MONO, None)
            .map_err(|error: parakeet_rs::Error| {
                CommandError::Internal(format!("Erkennung: {error}"))
            })?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CommandError::VoiceCancelled);
        }
        let text = result
            .text
            .split_whitespace()
            .collect::<Vec<&str>>()
            .join(" ");
        if text.is_empty() {
            return Err(CommandError::NoSpeech);
        }
        Ok(text)
    }
}

fn thread_count() -> usize {
    let available = thread::available_parallelism().map_or(1, NonZero::get);
    available.min(MAX_THREADS)
}

fn model_error(error: impl std::fmt::Display) -> CommandError {
    CommandError::Internal(format!("Sprachmodell: {error}"))
}
