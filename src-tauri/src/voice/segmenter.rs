//! Schneidet die laufende Aufnahme an Sprechpausen in Abschnitte, die einzeln erkannt werden.
//!
//! Ein Tick ist ein 50-ms-Durchlauf des Aufnahme-Threads; er zählt als Sprache, wenn sein RMS
//! mindestens `silence::SPEECH_RMS` ist. Die Samples bleiben im Format des Geräts (verschränkte
//! Kanäle, Gerätefrequenz) — umgerechnet wird erst der fertige Abschnitt.

/// 600 ms Pause beenden einen Abschnitt.
pub const PAUSE_TICKS: u32 = 12;
/// Kürzere Geräusche (200 ms) werden verworfen, nicht erkannt.
pub const MIN_SPEECH_TICKS: u32 = 4;
/// So viel Stille vor dem ersten Wort bleibt erhalten, damit der Wortanfang nicht abgeschnitten wird.
pub const LEAD_IN_MS: u32 = 300;
/// Zwangsschnitt für Dauerredner, damit der Zwischentext nicht ausbleibt.
pub const MAX_SEGMENT_SECS: u32 = 25;

pub struct Segmenter {
    segment: Vec<f32>,
    speech_ticks: u32,
    silent_run: u32,
    lead_in_len: usize,
    max_len: usize,
}

impl Segmenter {
    /// Beide Längen sind ganze Frames, damit nie ein Frame zerschnitten wird — cpal liefert die
    /// Samples immer in ganzen Frames.
    pub fn new(channels: u16, rate: u32) -> Self {
        let channel_count = usize::from(channels);
        Segmenter {
            segment: Vec::new(),
            speech_ticks: 0,
            silent_run: 0,
            lead_in_len: (rate * LEAD_IN_MS / 1000) as usize * channel_count,
            max_len: (rate * MAX_SEGMENT_SECS) as usize * channel_count,
        }
    }

    pub fn push_tick(&mut self, samples: Vec<f32>, is_speech: bool) -> Option<Vec<f32>> {
        self.segment.extend(samples);
        if is_speech {
            self.speech_ticks += 1;
            self.silent_run = 0;
        } else {
            self.silent_run += 1;
        }
        if self.speech_ticks == 0 {
            let excess = self.segment.len().saturating_sub(self.lead_in_len);
            self.segment.drain(..excess);
            return None;
        }
        if self.silent_run >= PAUSE_TICKS || self.segment.len() >= self.max_len {
            return self.close_segment();
        }
        None
    }

    /// Schließt den Rest beim Stopp nach derselben Regel ab.
    pub fn finish(mut self) -> Option<Vec<f32>> {
        self.close_segment()
    }

    fn close_segment(&mut self) -> Option<Vec<f32>> {
        let is_long_enough = self.speech_ticks >= MIN_SPEECH_TICKS;
        self.speech_ticks = 0;
        self.silent_run = 0;
        if is_long_enough {
            Some(std::mem::take(&mut self.segment))
        } else {
            self.segment.clear();
            None
        }
    }
}
