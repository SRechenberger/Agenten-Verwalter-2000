//! Diktieren: Sprachmodell laden und prüfen, Sprache aufnehmen und satzweise lokal erkennen.
pub mod dictation;
pub mod model;
pub mod model_file;
pub mod recorder;
pub mod resample;
pub mod segmenter;
pub mod silence;
pub mod transcribe;

use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::error::CommandError;
use model::{VoiceModelEvent, VoiceModelState};
use model_file::{download, is_model_ready, model_dir, total_bytes};
use recorder::Recorder;
use silence::SILENT_PEAK;
use transcribe::Transcriber;

pub const VOICE_MODEL_EVENT: &str = "voice://model";
pub const VOICE_LEVEL_EVENT: &str = "voice://level";
pub const VOICE_PARTIAL_EVENT: &str = "voice://partial";

const PROGRESS_EVENT_INTERVAL: Duration = Duration::from_millis(250);

/// Zustand des Diktierens im Core: Modell-Download, laufendes Diktat und das geladene Modell.
/// App-weit höchstens ein Diktat.
pub struct VoiceService {
    inner: Mutex<VoiceInner>,
    /// Schaltet nur das Laden des Modells hintereinander, damit es nie zweimal gleichzeitig geladen
    /// wird; `inner` bleibt während des sekundenlangen Ladens frei.
    load_lock: Mutex<()>,
}

struct VoiceInner {
    download: Option<DownloadHandle>,
    recorder: Option<Recorder>,
    /// Bleibt nach `cancel` liegen, bis das nächste `start` ihn fallen lässt.
    worker: Option<JoinHandle<Result<String, CommandError>>>,
    /// Ein `stop` wartet gerade auf die Erkennung.
    is_transcribing: bool,
    /// Je Diktat neu, damit ein auslaufender abgebrochener Thread das nächste Diktat nicht sieht.
    cancel: Arc<AtomicBool>,
    transcriber: Option<Arc<Transcriber>>,
}

struct DownloadHandle {
    cancel: Arc<AtomicBool>,
    received_bytes: u64,
    total_bytes: u64,
}

impl Default for VoiceService {
    fn default() -> Self {
        VoiceService::new()
    }
}

impl VoiceService {
    pub fn new() -> VoiceService {
        VoiceService {
            inner: Mutex::new(VoiceInner {
                download: None,
                recorder: None,
                worker: None,
                is_transcribing: false,
                cancel: Arc::new(AtomicBool::new(false)),
                transcriber: None,
            }),
            load_lock: Mutex::new(()),
        }
    }

    /// Startet Aufnahme und Erkennungs-Thread und kehrt zurück, sobald das Mikrofon läuft.
    pub fn start(&self, app: AppHandle) -> Result<(), CommandError> {
        let dir = model_dir(&app)?;
        let mut inner = self.lock();
        // Eine abgebrochene Erkennung läuft womöglich noch aus (das Modell lässt sich mitten im Rechnen
        // nicht sofort anhalten); sie sendet nichts mehr und darf ein neues Diktat nicht sperren.
        let is_stopping = inner.is_transcribing && !inner.cancel.load(Ordering::Relaxed);
        if inner.recorder.is_some() || is_stopping {
            return Err(CommandError::VoiceBusy);
        }
        if !is_model_ready(&dir) {
            return Err(CommandError::VoiceModelMissing);
        }
        inner.is_transcribing = false;
        // Ein liegengebliebener Thread stammt von einem abgebrochenen Diktat: er läuft selbst aus und
        // sendet nichts mehr, weil sein `cancel` gesetzt ist. Nicht joinen — sonst wartet `voice_start`,
        // und der Thread selbst wartet womöglich in `transcriber` auf `inner`.
        inner.worker = None;
        let cancel = Arc::new(AtomicBool::new(false));
        inner.cancel = Arc::clone(&cancel);
        let (segment_sender, segment_receiver) = mpsc::channel();
        let recorder = Recorder::start(app.clone(), segment_sender)?;
        let (channels, rate) = (recorder.channels, recorder.rate);
        let spawned = thread::Builder::new()
            .name("voice-transcribe".to_owned())
            .spawn(move || dictation::run(app, segment_receiver, channels, rate, cancel));
        match spawned {
            Ok(worker) => {
                inner.recorder = Some(recorder);
                inner.worker = Some(worker);
                Ok(())
            }
            Err(error) => {
                recorder.cancel();
                Err(CommandError::Internal(format!(
                    "Erkennungs-Thread startet nicht: {error}"
                )))
            }
        }
    }

    /// Beendet die Aufnahme und wartet, bis auch der letzte Abschnitt erkannt ist. Blockiert — nur aus
    /// einem blockierenden Thread aufrufen.
    pub fn stop(&self) -> Result<String, CommandError> {
        let started = Instant::now();
        let (recorder, worker, cancel) = {
            let mut inner = self.lock();
            let Some(recorder) = inner.recorder.take() else {
                return Err(CommandError::VoiceCancelled);
            };
            let Some(worker) = inner.worker.take() else {
                recorder.cancel();
                return Err(CommandError::Internal("kein Erkennungs-Thread".to_owned()));
            };
            inner.is_transcribing = true;
            (recorder, worker, Arc::clone(&inner.cancel))
        };
        let _transcribing = TranscribingGuard {
            service: self,
            cancel: Arc::clone(&cancel),
        };

        let recorded = match recorder.stop() {
            Ok(recorded) => recorded,
            Err(error) => {
                cancel.store(true, Ordering::Relaxed);
                return Err(error);
            }
        };
        let result = worker
            .join()
            .map_err(|_| CommandError::Internal("Erkennungs-Thread abgestürzt".to_owned()))?;
        if cancel.load(Ordering::Relaxed) {
            return Err(CommandError::VoiceCancelled);
        }
        let text = result?;
        if cfg!(debug_assertions) {
            eprintln!("voice: Stopp bis Text {} ms", started.elapsed().as_millis());
        }
        if text.is_empty() {
            if recorded.peak < SILENT_PEAK {
                return Err(CommandError::NoAudio);
            }
            return Err(CommandError::NoSpeech);
        }
        Ok(text)
    }

    /// Bricht Aufnahme und Erkennung ab; ohne laufendes Diktat passiert nichts. Ein laufendes `stop`
    /// liefert danach `VoiceCancelled`.
    pub fn cancel(&self) {
        let mut inner = self.lock();
        inner.cancel.store(true, Ordering::Relaxed);
        if let Some(recorder) = inner.recorder.take() {
            recorder.cancel();
        }
    }

    /// Lädt das Modell beim ersten Aufruf und hält es bis zum App-Ende. Ein Ladefehler wird beim
    /// nächsten Aufruf neu versucht.
    pub fn transcriber(&self, app: &AppHandle) -> Result<Arc<Transcriber>, CommandError> {
        let _loading = self
            .load_lock
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        if let Some(transcriber) = &self.lock().transcriber {
            return Ok(Arc::clone(transcriber));
        }
        let started = Instant::now();
        let transcriber = Arc::new(Transcriber::load(&model_dir(app)?)?);
        if cfg!(debug_assertions) {
            eprintln!(
                "voice: Sprachmodell geladen in {} ms",
                started.elapsed().as_millis()
            );
        }
        self.lock().transcriber = Some(Arc::clone(&transcriber));
        Ok(transcriber)
    }

    pub fn model_state(&self, app: &AppHandle) -> Result<VoiceModelState, CommandError> {
        if let Some(download) = &self.lock().download {
            return Ok(downloading_state(
                download.received_bytes,
                download.total_bytes,
            ));
        }
        if is_model_ready(&model_dir(app)?) {
            Ok(VoiceModelState::Ready)
        } else {
            Ok(VoiceModelState::Missing)
        }
    }

    /// Startet den Download im Hintergrund und kehrt sofort zurück. Läuft schon einer oder liegt das
    /// Modell fertig da, passiert nichts.
    pub fn start_download(&self, app: &AppHandle) -> Result<(), CommandError> {
        let dir = model_dir(app)?;
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut inner = self.lock();
            if inner.download.is_some() || is_model_ready(&dir) {
                return Ok(());
            }
            inner.download = Some(DownloadHandle {
                cancel: Arc::clone(&cancel),
                received_bytes: 0,
                total_bytes: total_bytes(),
            });
        }
        let worker_app = app.clone();
        let spawned = thread::Builder::new()
            .name("voice-download".to_owned())
            .spawn(move || run_download(&worker_app, &dir, &cancel));
        if let Err(error) = spawned {
            self.lock().download = None;
            return Err(CommandError::VoiceDownload(format!(
                "Download startet nicht: {error}"
            )));
        }
        Ok(())
    }

    /// Setzt nur das Abbruch-Zeichen; der Download-Thread räumt auf und meldet `missing`.
    pub fn cancel_download(&self) {
        if let Some(download) = &self.lock().download {
            download.cancel.store(true, Ordering::Relaxed);
        }
    }

    fn set_progress(&self, received_bytes: u64, total_bytes: u64) {
        if let Some(download) = &mut self.lock().download {
            download.received_bytes = received_bytes;
            download.total_bytes = total_bytes;
        }
    }

    fn lock(&self) -> MutexGuard<'_, VoiceInner> {
        self.inner.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Setzt `is_transcribing` in jedem Ausgang von `stop` zurück — aber nur, solange noch das eigene
/// Diktat das aktuelle ist: ein auslaufendes abgebrochenes darf das Zeichen eines neuen nicht löschen.
struct TranscribingGuard<'a> {
    service: &'a VoiceService,
    cancel: Arc<AtomicBool>,
}

impl Drop for TranscribingGuard<'_> {
    fn drop(&mut self) {
        let mut inner = self.service.lock();
        if Arc::ptr_eq(&inner.cancel, &self.cancel) {
            inner.is_transcribing = false;
        }
    }
}

fn run_download(app: &AppHandle, dir: &Path, cancel: &AtomicBool) {
    let service = app.state::<VoiceService>();
    emit_model(app, downloading_state(0, total_bytes()), None);
    let mut last_event = Instant::now();
    let result = download(dir, cancel, |received_bytes: u64, total_bytes: u64| {
        service.set_progress(received_bytes, total_bytes);
        if last_event.elapsed() >= PROGRESS_EVENT_INTERVAL {
            last_event = Instant::now();
            emit_model(app, downloading_state(received_bytes, total_bytes), None);
        }
    });
    // Erst das Handle weg, dann melden: ein `voice_model_status` nach dem Ereignis darf nicht mehr
    // „Downloading“ sehen.
    service.lock().download = None;
    match result {
        Ok(()) => emit_model(app, VoiceModelState::Ready, None),
        // Ein Abbruch durch den Nutzer ist kein Fehler, den die Oberfläche anzeigen soll.
        Err(_) if cancel.load(Ordering::Relaxed) => {
            emit_model(app, VoiceModelState::Missing, None);
        }
        Err(error) => emit_model(app, VoiceModelState::Missing, Some(error.to_string())),
    }
}

fn downloading_state(received_bytes: u64, total_bytes: u64) -> VoiceModelState {
    VoiceModelState::Downloading {
        received_bytes: received_bytes as f64,
        total_bytes: total_bytes as f64,
    }
}

fn emit_model(app: &AppHandle, state: VoiceModelState, error: Option<String>) {
    if let Err(send_error) = app.emit(VOICE_MODEL_EVENT, VoiceModelEvent { state, error }) {
        eprintln!("Sprachmodell-Ereignis nicht gesendet: {send_error}");
    }
}
