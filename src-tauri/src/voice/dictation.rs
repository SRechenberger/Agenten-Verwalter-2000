//! Der Erkennungs-Thread eines Diktats: erkennt die Abschnitte, während die Aufnahme weiterläuft, und
//! meldet den wachsenden Gesamttext als `voice://partial`.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager};

use super::model::VoicePartialEvent;
use super::resample::{TARGET_RATE, to_mono_16k};
use super::silence::{Silence, is_silent};
use super::{VOICE_PARTIAL_EVENT, VoiceService};
use crate::error::CommandError;

/// Läuft, bis der Aufnahme-Thread den Kanal schließt, und gibt den Gesamttext zurück (leer, wenn kein
/// Abschnitt Text ergab).
pub fn run(
    app: AppHandle,
    segments: mpsc::Receiver<Vec<f32>>,
    channels: u16,
    rate: u32,
    cancel: Arc<AtomicBool>,
) -> Result<String, CommandError> {
    // Lädt beim ersten Diktat das Modell; die Abschnitte stauen sich derweil im Kanal.
    let transcriber = app.state::<VoiceService>().transcriber(&app)?;
    let mut text = String::new();
    while let Ok(first) = segments.recv() {
        let mut audio = first;
        let mut merged_count = 1;
        // Liegengebliebene Abschnitte in einem Durchlauf: spart Aufrufe, wenn die Erkennung hinter dem
        // Sprechen herhinkt.
        while let Ok(next) = segments.try_recv() {
            audio.extend(next);
            merged_count += 1;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(CommandError::VoiceCancelled);
        }
        let mono = to_mono_16k(&audio, channels, rate);
        if is_silent(&mono) != Silence::Speech {
            continue;
        }
        let started = Instant::now();
        match transcriber.transcribe(&mono, &cancel) {
            Ok(part) => {
                if !text.is_empty() {
                    text.push(' ');
                }
                text.push_str(&part);
            }
            Err(CommandError::NoSpeech) => {}
            Err(error) => return Err(error),
        }
        if cfg!(debug_assertions) {
            eprintln!(
                "voice: Abschnitt {:.1} s Audio ({merged_count} zusammengefasst), erkannt in {} ms",
                mono.len() as f32 / TARGET_RATE as f32,
                started.elapsed().as_millis()
            );
        }
        // Direkt vor dem Senden geprüft: Text eines abgebrochenen Diktats darf nicht in einem neuen
        // landen.
        if !cancel.load(Ordering::Relaxed) && !text.is_empty() {
            emit_partial(&app, &text);
        }
    }
    if cancel.load(Ordering::Relaxed) {
        return Err(CommandError::VoiceCancelled);
    }
    Ok(text)
}

fn emit_partial(app: &AppHandle, text: &str) {
    let event = VoicePartialEvent {
        text: text.to_owned(),
    };
    if let Err(error) = app.emit(VOICE_PARTIAL_EVENT, event) {
        eprintln!("Diktat-Ereignis nicht gesendet: {error}");
    }
}
