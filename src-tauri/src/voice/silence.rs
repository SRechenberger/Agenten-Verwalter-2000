//! Stille-Schwellen des Diktierens: ob ein Stück Audio überhaupt Ton und Sprache enthält.

// Gesetzt im Plan, am echten Mikrofon per Smoke-Test zu bestätigen.
/// Spitzenwert, unter dem eine Aufnahme als komplett still gilt (Mikrofon stumm oder tot).
pub const SILENT_PEAK: f32 = 0.001;
/// RMS, ab dem ein Fenster als laut genug für Sprache gilt.
pub const SPEECH_RMS: f32 = 0.01;
/// Anteil lauter Fenster, ab dem ein Stück an die Erkennung geht — darunter lohnt ein Durchlauf nicht.
pub const SPEECH_SHARE: f32 = 0.05;
/// 30 ms bei 16 kHz.
pub const WINDOW: usize = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    NoAudio,
    NoSpeech,
    Speech,
}

/// Erwartet 16 kHz mono.
pub fn is_silent(audio: &[f32]) -> Silence {
    let peak = audio
        .iter()
        .fold(0.0_f32, |peak: f32, sample: &f32| peak.max(sample.abs()));
    if peak < SILENT_PEAK {
        return Silence::NoAudio;
    }
    let window_count = audio.chunks(WINDOW).len();
    let loud_count = audio
        .chunks(WINDOW)
        .filter(|window: &&[f32]| rms(window) > SPEECH_RMS)
        .count();
    if (loud_count as f32) < SPEECH_SHARE * window_count as f32 {
        Silence::NoSpeech
    } else {
        Silence::Speech
    }
}

fn rms(samples: &[f32]) -> f32 {
    let sum_sq: f32 = samples.iter().map(|sample: &f32| sample * sample).sum();
    (sum_sq / samples.len() as f32).sqrt()
}
