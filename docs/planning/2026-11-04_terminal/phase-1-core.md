# Phase 1 — Core: Pseudo-Terminal (heikel)

## Kontext

- [README](README.md): Entscheidungen, Kontrakt, finale AK.
- `src-tauri/src/lib.rs` (Setup, `invoke_handler`, `.run`), `src-tauri/src/error.rs` (Muster `Implementation(String)`), `src-tauri/src/sessions/registry/artifacts.rs` (Muster für eine Registry-Erweiterung), `src-tauri/src/sessions/registry/tldr.rs` (`project_members`), `src-tauri/src/commands/projects.rs` (`project_archive`), `src-tauri/src/artifacts/model.rs` (Muster TS-Typen), `src-tauri/examples/gen-bindings.rs` (Export-Liste).
- [docs/conventions/rust.md](../../conventions/rust.md), [docs/conventions/linting.md](../../conventions/linting.md).
- Doku `portable-pty` 0.9.0: `native_pty_system`, `PtySize`, `CommandBuilder`, `MasterPty::try_clone_reader`/`take_writer`/`resize`, `Child::clone_killer`/`wait`, `ExitStatus::exit_code`.
- Fehlerklassen: keine Vault-Einträge für Rust/Tauri.

## AK der Phase

- Kontrakt (Terminal-Teil) vollständig umgesetzt; `pnpm check` grün; `pnpm bindings` erzeugt `TerminalInfo.ts`, `TerminalOutput.ts`, `TerminalChanged.ts`.
- Keine Sperre wird gehalten, während eine andere genommen wird (Code-Blick); kein `unwrap`.

## Checkliste

- [x] `portable-pty = "0.9.0"` in `src-tauri/Cargo.toml` unter `[dependencies]`.
- [x] `CommandError::Terminal(String)` mit `#[error("{0}")]` und Doc-Kommentar („Ein Satz über ein Terminal (ADR 055).“) in `src-tauri/src/error.rs`, direkt nach `Implementation`.
- [x] `src-tauri/src/terminal/model.rs`: Typen aus dem Kontrakt.
- [x] `src-tauri/src/terminal/shell.rs`: `pub struct ShellCommand { pub program: PathBuf, pub args: Vec<String>, pub label: String }` und `pub fn shell_command() -> ShellCommand` mit drei Zweigen: `#[cfg(windows)]` (`ComSpec`, sonst `cmd.exe`, keine Argumente), `#[cfg(target_os = "macos")]` (`SHELL`, sonst `/bin/zsh`, `["-l"]`), `#[cfg(all(unix, not(target_os = "macos")))]` (`SHELL`, sonst `/bin/bash`, `["-l"]`). Leere Umgebungsvariable zählt wie fehlend. Label = `Path::file_stem` des Programms, klein geschrieben (Windows ergibt `cmd`).
- [x] `src-tauri/src/terminal/buffer.rs`:
  - `pub struct Utf8Carry { pending: Vec<u8> }` mit `pub fn decode(&mut self, bytes: &[u8]) -> String`: `pending` + `bytes` zusammenfügen; Schleife über `std::str::from_utf8`: gültigen Teil anhängen; bei `error_len() == Some(n)` ein `'\u{FFFD}'` anhängen und `n` Bytes überspringen; bei `None` den Rest (≤ 3 Bytes) in `pending` behalten und enden.
  - `pub struct ScrollBuffer { text: String }` mit `const CAPACITY: usize = 1024 * 1024;`, `push(&mut self, chunk: &str)` (anhängen; ist `text.len() > CAPACITY`, vorne bis zur ersten Zeichengrenze ab `text.len() - CAPACITY` abschneiden, `is_char_boundary`), `snapshot(&self) -> String`.
- [x] `src-tauri/src/terminal/mod.rs`:
  - `pub mod model; pub mod shell; mod buffer;`; `const MAX_RUNNING: usize = 8;`, `const READ_CHUNK: usize = 8192;`, `pub const CHANGED_EVENT: &str = "terminal://changed";`
  - `struct Terminal { info: Mutex<TerminalInfo>, io: Mutex<TerminalIo>, output: Mutex<OutputState> }`; `struct TerminalIo { writer: Box<dyn Write + Send>, master: Option<Box<dyn MasterPty + Send>>, killer: Box<dyn ChildKiller + Send + Sync> }`; `struct OutputState { buffer: ScrollBuffer, channel: Option<Channel<TerminalOutput>> }`.
  - `pub struct TerminalService { terminals: Mutex<HashMap<String, Arc<Terminal>>>, numbers: Mutex<HashMap<String, u32>> }` mit `new()`. Regel: Map-Sperre nur zum Holen/Einfügen/Entfernen der `Arc`, nie während eine Terminal-Sperre genommen wird; je Terminal nie zwei seiner Sperren zugleich. Vergiftete Sperren wie im übrigen Core behandeln (Muster `lock_sessions` in `sessions/registry.rs`).
  - `pub fn open(&self, app: &AppHandle, project_id: &str, workspace: &Path, cols: u16, rows: u16) -> Result<TerminalInfo, CommandError>`: laufende Terminals des Vorhabens zählen (≥ `MAX_RUNNING` → Fehler aus dem Kontrakt); `native_pty_system().openpty(PtySize { rows, cols, pixel_width: 0, pixel_height: 0 })`; `CommandBuilder::new(program)`, `args`, `cwd(workspace)`, `env("TERM", "xterm-256color")`, `env("COLORTERM", "truecolor")`; `slave.spawn_command`; `slave` sofort droppen; `reader = master.try_clone_reader()`, `writer = master.take_writer()`, `killer = child.clone_killer()`; Nummer aus `numbers` (+1); einfügen; Lese- und Warte-Thread starten; `app.emit(CHANGED_EVENT, TerminalChanged { project_id })`; `TerminalInfo` zurück. Fehler von `portable-pty` als `CommandError::Terminal(format!("Terminal nicht gestartet: {error}"))`.
  - **Lese-Thread** (`std::thread::Builder::new().name("terminal-read".into())`): Schleife `reader.read(&mut [0u8; READ_CHUNK])`; `0` oder Fehler → Ende; sonst `text = carry.decode(..)`, leer → weiter; `output`-Sperre: `buffer.push(&text)`, bei `Some(channel)` `channel.send(TerminalOutput::Data { text })`, scheitert das → `channel = None`.
  - **Warte-Thread** (`name("terminal-wait".into())`): `child.wait()`; Code `Ok(status) → Some(status.exit_code())`, sonst `None`; `info`-Sperre: `running = false`, `exit_code = code`; dann `output`-Sperre: `Exit { code }` an den Kanal, falls vorhanden; dann `io`-Sperre: `master.take()` droppen (löst unter Windows den blockierten Lese-Thread); `app.emit(CHANGED_EVENT, …)`.
  - `pub fn list(&self, project_id: &str) -> Vec<TerminalInfo>` (Klone, nach `number`).
  - `pub fn attach(&self, id: &str, channel: Channel<TerminalOutput>) -> Result<String, CommandError>`: erst `info` lesen (`running`, `exit_code`), dann `output`-Sperre: Kanal ersetzen, `snapshot` nehmen; ist `running == false`, unter derselben Sperre `Exit { code }` senden. Unbekannte ID → `CommandError::Terminal("Terminal nicht gefunden.".to_owned())`.
  - `pub fn detach(&self, id: &str)`: `channel = None`, unbekannte ID still.
  - `pub fn write(&self, id: &str, data: &str) -> Result<(), CommandError>`: `writer.write_all` + `flush`.
  - `pub fn resize(&self, id: &str, cols: u16, rows: u16) -> Result<(), CommandError>`: nur mit `Some(master)`.
  - `pub fn close(&self, app: &AppHandle, id: &str)`: aus der Map entfernen; `killer.kill()` (Fehler ignorieren); `master.take()`; Ereignis.
  - `pub fn close_project(&self, app: &AppHandle, project_id: &str)` und `pub fn close_all(&self)`: IDs sammeln, je ID wie `close` (bei `close_all` ohne Ereignis).
- [x] `src-tauri/src/sessions/registry/terminal.rs` (als Modul eintragen wie `artifacts`): `pub fn project_workspace(&self, project_id: &str) -> Result<PathBuf, CommandError>` = `workspace` der ersten Session aus `project_members`; leer → `project_not_found(project_id)`.
- [x] `src-tauri/src/commands/terminal.rs`: sieben Commands aus dem Kontrakt, je `#[tauri::command] pub async fn` mit `tauri::State<'_, TerminalService>` (`terminal_open` zusätzlich `SessionRegistry` und `AppHandle`); in `commands/mod.rs` eintragen.
- [x] `src-tauri/src/lib.rs`: `mod terminal;`; `app.manage(terminal::TerminalService::new());` im Setup; Commands im `invoke_handler`; `.run(tauri::generate_context!()).expect(…)` ersetzen durch `.build(tauri::generate_context!()).expect("Tauri-Laufzeit konnte nicht starten").run(|app, event| { if let tauri::RunEvent::Exit = event { app.state::<terminal::TerminalService>().close_all(); } });`.
- [x] `src-tauri/src/commands/projects.rs` `project_archive`: Parameter `terminals: tauri::State<'_, TerminalService>`; nach erfolgreichem `archive_project` `terminals.close_project(&app, &project_id)`.
- [x] `src-tauri/examples/gen-bindings.rs`: drei Typen exportieren; `pnpm bindings`.
- [x] `docs/decisions/055-terminal.md`: Kontext / Optionen (extern, eingebettet als Reiter, eingebettet als Dock unten/rechts, `!`-Befehle im Chat) / Entscheidung / Konsequenzen (nur im Speicher, 1 MiB Puffer, neu abgespielt nur nach Neuladen der Oberfläche, xterm-Instanzen leben bis zum Ende ihres Terminals — bewusste Ausnahme von Critical Rule 3, macOS/Linux ungeprüft, Enkelprozesse).
- [x] Commit `feat(terminal): Pseudo-Terminal je Vorhaben im Core`.

## Report-Back

Status: complete. `pnpm check` grün, Bindings erzeugt. Abweichung: das Ende der Shell (`exit`) liegt in `OutputState` statt nur in `info`, damit `attach` es unter der Ausgabe-Sperre sieht und `Exit` nicht verloren geht. `CommandError::Terminal` ergänzt in `src/lib/bindings/CommandError.ts` (generiert). Laufzeit ungeprüft (Wackelstellen README).
