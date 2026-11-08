//! Aufgaben einer Session: Prozesse aus `Bash`/`PowerShell` mit `run_in_background` und
//! Subagenten. Hintergrundprozesse und -subagenten laufen bei Esc weiter, enden auf `TaskStop`
//! bzw. `stop_task` und mit dem Agent-Prozess.
//!
//! Je Prozess wartet ein Überwacher-Thread auf das echte Prozessende (`wait`), nie auf eine Zeit:
//! eine Ausgabedatei, die eine Weile nicht wächst, heißt nicht, dass der Prozess tot ist. Erst der
//! Überwacher schreibt `task_notification` und den Hinweis fürs Modell. Ebenso meldet ein Subagent
//! sein Ende erst, wenn sein Thread zurück ist.
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::Receiver;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use uuid::Uuid;

use super::output::Output;
use crate::processes::{self, hide_console};

const PROCESS_ID_PREFIX: char = 'b';
const SUBAGENT_ID_PREFIX: char = 'a';
const ID_HEX_CHARS: usize = 8;
const TASK_TYPE_PROCESS: &str = "local_bash";
const TASK_TYPE_SUBAGENT: &str = "local_agent";
/// So viel vom letzten Text bzw. Fehler eines Subagenten steht in seiner `task_notification`.
const SUMMARY_CHARS: usize = 200;
const OUTPUT_EXTENSION: &str = "output";
/// So lange wartet der Überwacher nach Prozessende auf den Rest der Ausgabe — ein Enkelprozess, der
/// die Pipe offen hält, soll das Ende nicht aufhalten.
const READER_GRACE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskStatus {
    Completed,
    Stopped,
    Failed,
}

impl TaskStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            TaskStatus::Completed => "completed",
            TaskStatus::Stopped => "stopped",
            TaskStatus::Failed => "failed",
        }
    }
}

/// Wie eine Aufgabe beendet wird.
enum TaskControl {
    Process {
        pid: u32,
        output_file: PathBuf,
    },
    /// Der Subagent prüft den Schalter vor jeder Modellanfrage und jedem Werkzeug.
    Subagent {
        cancel: Arc<AtomicBool>,
    },
}

struct Task {
    description: String,
    control: TaskControl,
    /// Gesetzt von `stop`, damit der Überwacher „stopped“ statt „failed“ meldet.
    is_stopped: bool,
}

impl Task {
    /// Markiert die Aufgabe als gestoppt; bei einem Prozess ist das Ergebnis die PID, deren Baum
    /// noch beendet werden muss — außerhalb der Sperre, `taskkill` dauert.
    fn mark_stopped(&mut self) -> Option<u32> {
        self.is_stopped = true;
        match &self.control {
            TaskControl::Process { pid, .. } => Some(*pid),
            TaskControl::Subagent { cancel } => {
                cancel.store(true, Ordering::SeqCst);
                None
            }
        }
    }
}

/// Ein Hintergrundprozess, der gerade gestartet wurde.
pub struct NewProcess {
    pub task_id: String,
    pub tool_use_id: String,
    pub description: String,
    pub output_file: PathBuf,
    pub child: Child,
    /// Meldet je Lese-Thread einmal, dass er fertig ist.
    pub readers: Receiver<()>,
}

pub struct Tasks {
    output: Arc<Output>,
    /// `<scratchpad>\tasks`, dort liegen die Ausgabedateien.
    output_dir: PathBuf,
    running: Mutex<HashMap<String, Task>>,
    /// Hinweise über beendete Aufgaben, die mit der nächsten Modellanfrage ans Modell gehen.
    notes: Mutex<Vec<String>>,
}

impl Tasks {
    pub fn new(output: Arc<Output>, output_dir: PathBuf) -> Tasks {
        Tasks {
            output,
            output_dir,
            running: Mutex::new(HashMap::new()),
            notes: Mutex::new(Vec::new()),
        }
    }

    /// Neue Aufgaben-ID und ihre Ausgabedatei `<scratchpad>\tasks\<id>.output`.
    pub fn new_process_slot(&self) -> (String, PathBuf) {
        let task_id = new_task_id(PROCESS_ID_PREFIX);
        let output_file = self
            .output_dir
            .join(format!("{task_id}.{OUTPUT_EXTENSION}"));
        (task_id, output_file)
    }

    /// Trägt einen Subagenten ein und meldet `task_started`; Rückgabe ist seine Aufgaben-ID.
    /// Sein Ende meldet `finish_subagent`.
    pub fn start_subagent(
        &self,
        tool_use_id: &str,
        description: &str,
        subagent_type: &str,
        cancel: Arc<AtomicBool>,
    ) -> String {
        let task_id = new_task_id(SUBAGENT_ID_PREFIX);
        self.lock_running().insert(
            task_id.clone(),
            Task {
                description: description.to_owned(),
                control: TaskControl::Subagent { cancel },
                is_stopped: false,
            },
        );
        self.output.line(&json!({
            "type": "system",
            "subtype": "task_started",
            "task_id": task_id,
            "tool_use_id": tool_use_id,
            "task_type": TASK_TYPE_SUBAGENT,
            "description": description,
            "subagent_type": subagent_type,
        }));
        task_id
    }

    /// `tool_uses` ist der Gesamtstand des Subagenten, nicht der Zuwachs seit der letzten Meldung.
    pub fn report_progress(&self, task_id: &str, tool_uses: u32) {
        self.output.line(&json!({
            "type": "system",
            "subtype": "task_progress",
            "task_id": task_id,
            "usage": { "tool_uses": tool_uses },
        }));
    }

    /// Meldet das Ende eines Subagenten. Ein Hinweis fürs Modell entfällt: den Bericht eines
    /// Hintergrund-Subagenten bekommt der Hauptagent als eigene Nachricht.
    pub fn finish_subagent(&self, task_id: &str, status: TaskStatus, text: &str) {
        self.lock_running().remove(task_id);
        let summary: String = text.chars().take(SUMMARY_CHARS).collect();
        self.output.line(&json!({
            "type": "system",
            "subtype": "task_notification",
            "task_id": task_id,
            "status": status.as_str(),
            "summary": summary,
            "output_file": null,
        }));
    }

    /// Trägt den Prozess ein, meldet `task_started` und startet seinen Überwacher. Startet der
    /// Thread nicht, wird der Prozess beendet — unüberwacht liefe er bis zum Ende des Agenten.
    pub fn watch_process(self: &Arc<Self>, process: NewProcess) -> Result<(), String> {
        let NewProcess {
            task_id,
            tool_use_id,
            description,
            output_file,
            mut child,
            readers,
        } = process;
        let pid = child.id();
        self.lock_running().insert(
            task_id.clone(),
            Task {
                description: description.clone(),
                control: TaskControl::Process { pid, output_file },
                is_stopped: false,
            },
        );
        self.output
            .line(&task_started(&task_id, &tool_use_id, &description));
        let tasks = Arc::clone(self);
        let watched_id = task_id.clone();
        let spawned = thread::Builder::new()
            .name("agent-task".to_owned())
            .spawn(move || {
                let exit_code = child
                    .wait()
                    .ok()
                    .and_then(|status| status.code())
                    .unwrap_or(-1);
                wait_for_readers(&readers);
                tasks.finish(&watched_id, exit_code);
            });
        if let Err(error) = spawned {
            kill_process_tree(pid);
            self.finish(&task_id, -1);
            return Err(format!("Überwachung startet nicht: {error}"));
        }
        Ok(())
    }

    /// Beendet den Prozessbaum bzw. setzt den Abbruch des Subagenten; das Ende meldet der
    /// Überwacher bzw. der Subagent selbst.
    pub fn stop(&self, task_id: &str) -> Result<(), String> {
        let pid = {
            let mut running = self.lock_running();
            let Some(task) = running.get_mut(task_id) else {
                return Err(format!("Aufgabe unbekannt: {task_id}"));
            };
            task.mark_stopped()
        };
        if let Some(pid) = pid {
            kill_process_tree(pid);
        }
        Ok(())
    }

    /// Beim Ende des Agenten: alle laufenden Aufgaben mitnehmen.
    pub fn stop_all(&self) {
        let pids: Vec<u32> = self
            .lock_running()
            .values_mut()
            .filter_map(Task::mark_stopped)
            .collect();
        for pid in pids {
            kill_process_tree(pid);
        }
    }

    pub fn take_notes(&self) -> Vec<String> {
        std::mem::take(&mut *self.notes.lock().unwrap_or_else(PoisonError::into_inner))
    }

    fn finish(&self, task_id: &str, exit_code: i32) {
        let Some(task) = self.lock_running().remove(task_id) else {
            return;
        };
        let TaskControl::Process { output_file, .. } = &task.control else {
            return;
        };
        let status = if task.is_stopped {
            TaskStatus::Stopped
        } else if exit_code == 0 {
            TaskStatus::Completed
        } else {
            TaskStatus::Failed
        };
        self.output
            .line(&task_notification(task_id, status, exit_code, output_file));
        self.notes
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push(format!(
                "Background task {task_id} ({}) finished: {}, exit code {exit_code}. Output: {}",
                task.description,
                status.as_str(),
                output_file.display()
            ));
    }

    fn lock_running(&self) -> MutexGuard<'_, HashMap<String, Task>> {
        self.running.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Präfix plus 8 Hex-Zeichen, wie bei Claude Code (`b…` Prozess, `a…` Subagent).
fn new_task_id(prefix: char) -> String {
    let hex: String = Uuid::new_v4()
        .simple()
        .to_string()
        .chars()
        .take(ID_HEX_CHARS)
        .collect();
    format!("{prefix}{hex}")
}

/// `taskkill /T` nimmt die Kindprozesse mit — `bash -c` startet seine Befehle als eigene Prozesse.
pub fn kill_process_tree(pid: u32) {
    let mut taskkill = processes::command("taskkill");
    taskkill
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console(&mut taskkill);
    let _ = taskkill.status();
}

/// Wartet, bis alle Lese-Threads fertig sind (Kanal getrennt) oder die Frist um ist.
pub fn wait_for_readers(readers: &Receiver<()>) {
    let deadline = Instant::now() + READER_GRACE;
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if readers.recv_timeout(remaining).is_err() {
            return;
        }
    }
}

fn task_started(task_id: &str, tool_use_id: &str, description: &str) -> Value {
    json!({
        "type": "system",
        "subtype": "task_started",
        "task_id": task_id,
        "tool_use_id": tool_use_id,
        "task_type": TASK_TYPE_PROCESS,
        "description": description,
        "subagent_type": null,
    })
}

fn task_notification(
    task_id: &str,
    status: TaskStatus,
    exit_code: i32,
    output_file: &Path,
) -> Value {
    json!({
        "type": "system",
        "subtype": "task_notification",
        "task_id": task_id,
        "status": status.as_str(),
        "summary": format!("Exit code {exit_code}"),
        "output_file": output_file.to_string_lossy(),
    })
}
