//! Pseudo-Terminals je Vorhaben (ADR 055). Eine Shell im Workspace des Vorhabens, ihre Ausgabe als
//! Verlauf im Speicher und live an jeden Zuschauer: die eigene Oberfläche und das Gerät, das das
//! Vorhaben steuert (ADR 059), je über einen eigenen Kanal.
//!
//! Sperren: die Map nur zum Holen, Einfügen und Entfernen einer `Arc`; je Terminal nie zwei seiner
//! Sperren zugleich.
pub mod model;
pub mod shell;

mod buffer;

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use portable_pty::{ChildKiller, MasterPty, PtySize, native_pty_system};
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::error::CommandError;
use crate::processes;
use buffer::{ScrollBuffer, Utf8Carry};
use model::{TerminalChanged, TerminalInfo, TerminalOutput};

const MAX_RUNNING: usize = 8;
const READ_CHUNK: usize = 8192;
pub const CHANGED_EVENT: &str = "terminal://changed";

struct Terminal {
    info: Mutex<TerminalInfo>,
    io: Mutex<TerminalIo>,
    output: Mutex<OutputState>,
}

struct TerminalIo {
    writer: Box<dyn Write + Send>,
    /// Fällt nach dem Prozessende weg; unter Windows löst das den blockierten Lese-Thread.
    master: Option<Box<dyn MasterPty + Send>>,
    killer: Box<dyn ChildKiller + Send + Sync>,
}

/// Wer die Ausgabe eines Terminals sieht.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Viewer {
    /// Die Oberfläche dieser Instanz.
    Own,
    /// Ein Gerät, das das Vorhaben steuert; seine ID.
    Device(String),
}

struct OutputState {
    buffer: ScrollBuffer,
    /// Je Zuschauer höchstens ein Kanal.
    viewers: Vec<(Viewer, Channel<TerminalOutput>)>,
    /// `Some(Exit-Code)`, sobald die Shell beendet ist. Hier statt in `info`, damit `attach` das
    /// Ende unter derselben Sperre sieht, unter der es den Kanal einhängt — sonst ginge `Exit` verloren.
    exit: Option<Option<u32>>,
}

impl OutputState {
    /// Schickt an jeden Zuschauer; wessen Kanal scheitert, der ist danach keiner mehr.
    fn send(&mut self, output: &TerminalOutput) {
        self.viewers
            .retain(|(_, channel): &(Viewer, Channel<TerminalOutput>)| {
                channel.send(output.clone()).is_ok()
            });
    }
}

pub struct TerminalService {
    terminals: Mutex<HashMap<String, Arc<Terminal>>>,
    numbers: Mutex<HashMap<String, u32>>,
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

fn now_millis() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed: std::time::Duration| elapsed.as_secs_f64() * 1000.0)
        .unwrap_or_default()
}

fn not_found() -> CommandError {
    CommandError::Terminal("Terminal nicht gefunden.".to_owned())
}

fn not_started(error: impl std::fmt::Display) -> CommandError {
    CommandError::Terminal(format!("Terminal nicht gestartet: {error}"))
}

impl Default for TerminalService {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminalService {
    pub fn new() -> Self {
        Self {
            terminals: Mutex::new(HashMap::new()),
            numbers: Mutex::new(HashMap::new()),
        }
    }

    fn get(&self, id: &str) -> Result<Arc<Terminal>, CommandError> {
        lock(&self.terminals).get(id).cloned().ok_or_else(not_found)
    }

    fn project_terminals(&self, project_id: &str) -> Vec<Arc<Terminal>> {
        // Erst die Arcs holen, dann ohne Map-Sperre je Terminal lesen.
        let all: Vec<Arc<Terminal>> = lock(&self.terminals).values().cloned().collect();
        all.into_iter()
            .filter(|terminal: &Arc<Terminal>| lock(&terminal.info).project_id == project_id)
            .collect()
    }

    /// Das Vorhaben eines Terminals; `None`, wenn es das Terminal nicht gibt.
    pub fn project_of(&self, id: &str) -> Option<String> {
        let terminal = self.get(id).ok()?;
        let project_id = lock(&terminal.info).project_id.clone();
        Some(project_id)
    }

    pub fn list(&self, project_id: &str) -> Vec<TerminalInfo> {
        let mut infos: Vec<TerminalInfo> = self
            .project_terminals(project_id)
            .iter()
            .map(|terminal: &Arc<Terminal>| lock(&terminal.info).clone())
            .collect();
        infos.sort_by_key(|info: &TerminalInfo| info.number);
        infos
    }

    fn next_number(&self, project_id: &str) -> u32 {
        let mut numbers = lock(&self.numbers);
        let number: &mut u32 = numbers.entry(project_id.to_owned()).or_insert(0);
        *number += 1;
        *number
    }

    pub fn open(
        &self,
        app: &AppHandle,
        project_id: &str,
        workspace: &Path,
        cols: u16,
        rows: u16,
    ) -> Result<TerminalInfo, CommandError> {
        let running: usize = self
            .list(project_id)
            .iter()
            .filter(|info: &&TerminalInfo| info.running)
            .count();
        if running >= MAX_RUNNING {
            return Err(CommandError::Terminal(format!(
                "Höchstens {MAX_RUNNING} Terminals je Vorhaben laufen gleichzeitig."
            )));
        }

        let pair = native_pty_system()
            .openpty(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(not_started)?;
        let shell = shell::shell_command();
        let mut builder = processes::pty_command(&shell.program);
        builder.args(&shell.args);
        builder.cwd(workspace);
        builder.env("TERM", "xterm-256color");
        builder.env("COLORTERM", "truecolor");
        let child = pair.slave.spawn_command(builder).map_err(not_started)?;
        // Der Slave gehört dem Kindprozess; bleibt er hier offen, endet das Lesen nie.
        drop(pair.slave);
        let reader = pair.master.try_clone_reader().map_err(not_started)?;
        let writer = pair.master.take_writer().map_err(not_started)?;
        let killer = child.clone_killer();

        let info = TerminalInfo {
            id: uuid::Uuid::new_v4().to_string(),
            project_id: project_id.to_owned(),
            number: self.next_number(project_id),
            label: shell.label,
            cwd: workspace.to_string_lossy().into_owned(),
            started_at: now_millis(),
            running: true,
            exit_code: None,
        };
        let terminal = Arc::new(Terminal {
            info: Mutex::new(info.clone()),
            io: Mutex::new(TerminalIo {
                writer,
                master: Some(pair.master),
                killer,
            }),
            output: Mutex::new(OutputState {
                buffer: ScrollBuffer::default(),
                viewers: Vec::new(),
                exit: None,
            }),
        });
        lock(&self.terminals).insert(info.id.clone(), Arc::clone(&terminal));

        spawn_reader(Arc::clone(&terminal), reader);
        spawn_waiter(Arc::clone(&terminal), app.clone(), child);
        emit_changed(app, project_id);
        Ok(info)
    }

    /// Hängt den Kanal des Zuschauers an und liefert den bisherigen Verlauf; beides unter derselben
    /// Sperre, damit zwischen Verlauf und Live-Ausgabe nichts fehlt und nichts doppelt kommt. Hatte
    /// der Zuschauer schon einen Kanal, ersetzt der neue nur diesen.
    pub fn attach(
        &self,
        id: &str,
        viewer: Viewer,
        channel: Channel<TerminalOutput>,
    ) -> Result<String, CommandError> {
        let terminal = self.get(id)?;
        let mut output = lock(&terminal.output);
        let snapshot: String = output.buffer.snapshot();
        if let Some(code) = output.exit {
            // Fehlschlag heißt: die Oberfläche ist weg; ein Kanal ohne Empfänger braucht keinen Halt.
            let _ = channel.send(TerminalOutput::Exit { code });
        }
        output
            .viewers
            .retain(|(attached, _): &(Viewer, Channel<TerminalOutput>)| *attached != viewer);
        output.viewers.push((viewer, channel));
        Ok(snapshot)
    }

    /// Nimmt nur diesen Zuschauer weg; die übrigen sehen weiter zu.
    pub fn detach(&self, id: &str, viewer: &Viewer) {
        if let Ok(terminal) = self.get(id) {
            lock(&terminal.output)
                .viewers
                .retain(|(attached, _): &(Viewer, Channel<TerminalOutput>)| attached != viewer);
        }
    }

    /// Nimmt an allen Terminals des Vorhabens jedes Gerät außer `keep` als Zuschauer weg: Die
    /// Inhaberschaft ist zu Ende oder ging an ein anderes Gerät. Die Shells laufen weiter.
    pub fn detach_devices(&self, project_id: &str, keep: Option<&str>) {
        for terminal in self.project_terminals(project_id) {
            lock(&terminal.output).viewers.retain(
                |(viewer, _): &(Viewer, Channel<TerminalOutput>)| match viewer {
                    Viewer::Own => true,
                    Viewer::Device(device_id) => Some(device_id.as_str()) == keep,
                },
            );
        }
    }

    pub fn write(&self, id: &str, data: &str) -> Result<(), CommandError> {
        let terminal = self.get(id)?;
        let mut io = lock(&terminal.io);
        io.writer
            .write_all(data.as_bytes())
            .and_then(|()| io.writer.flush())
            .map_err(|error: std::io::Error| {
                CommandError::Terminal(format!("Eingabe nicht möglich: {error}"))
            })
    }

    pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), CommandError> {
        let terminal = self.get(id)?;
        let io = lock(&terminal.io);
        let Some(master) = io.master.as_ref() else {
            return Ok(());
        };
        master
            .resize(PtySize {
                rows,
                cols,
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|error| CommandError::Terminal(format!("Größe nicht änderbar: {error}")))
    }

    pub fn close(&self, app: &AppHandle, id: &str) {
        if let Some(project_id) = self.remove(id) {
            emit_changed(app, &project_id);
        }
    }

    /// Beendet alle Terminals des Vorhabens (Archivieren) mit einem einzigen Ereignis.
    pub fn close_project(&self, app: &AppHandle, project_id: &str) {
        let mut removed_any = false;
        for terminal in self.project_terminals(project_id) {
            let id: String = lock(&terminal.info).id.clone();
            removed_any |= self.remove(&id).is_some();
        }
        if removed_any {
            emit_changed(app, project_id);
        }
    }

    /// App-Ende: kein Shell-Prozess bleibt zurück, die Oberfläche braucht kein Ereignis mehr.
    pub fn close_all(&self) {
        let ids: Vec<String> = lock(&self.terminals).keys().cloned().collect();
        for id in ids {
            self.remove(&id);
        }
    }

    /// Nimmt das Terminal aus der Map und beendet den Prozess; liefert sein Vorhaben.
    fn remove(&self, id: &str) -> Option<String> {
        let terminal = lock(&self.terminals).remove(id)?;
        let project_id: String = lock(&terminal.info).project_id.clone();
        let mut io = lock(&terminal.io);
        // Ein schon beendeter Prozess meldet hier einen Fehler — dann ist nichts mehr zu beenden.
        let _ = io.killer.kill();
        io.master = None;
        Some(project_id)
    }
}

fn emit_changed(app: &AppHandle, project_id: &str) {
    let _ = app.emit(
        CHANGED_EVENT,
        TerminalChanged {
            project_id: project_id.to_owned(),
        },
    );
}

fn spawn_reader(terminal: Arc<Terminal>, mut reader: Box<dyn Read + Send>) {
    let spawned = std::thread::Builder::new()
        .name("terminal-read".into())
        .spawn(move || {
            let mut carry = Utf8Carry::default();
            let mut chunk = [0u8; READ_CHUNK];
            loop {
                let length: usize = match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => return,
                    Ok(length) => length,
                };
                let text: String = carry.decode(&chunk[..length]);
                if text.is_empty() {
                    continue;
                }
                let mut output = lock(&terminal.output);
                output.buffer.push(&text);
                // Kein Kanal wartet auf das Netz: Der eines Geräts legt nur in die Warteschlange
                // seiner Verbindung (ADR 059).
                output.send(&TerminalOutput::Data { text });
            }
        });
    if let Err(error) = spawned {
        eprintln!("Lese-Thread des Terminals nicht gestartet: {error}");
    }
}

fn spawn_waiter(
    terminal: Arc<Terminal>,
    app: AppHandle,
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
) {
    let spawned = std::thread::Builder::new()
        .name("terminal-wait".into())
        .spawn(move || {
            let code: Option<u32> = child.wait().ok().map(|status| status.exit_code());
            let project_id: String = {
                let mut info = lock(&terminal.info);
                info.running = false;
                info.exit_code = code;
                info.project_id.clone()
            };
            {
                let mut output = lock(&terminal.output);
                output.exit = Some(code);
                output.send(&TerminalOutput::Exit { code });
            }
            lock(&terminal.io).master = None;
            emit_changed(&app, &project_id);
        });
    if let Err(error) = spawned {
        eprintln!("Warte-Thread des Terminals nicht gestartet: {error}");
    }
}
