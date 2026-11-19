//! Befehls-Stationen (ADR 073): Zerlegen und Einsetzen der Schritte, Prüfung und der Lauf ohne Shell.
//! Alles hier ist rein oder braucht nur `processes::command`; Datenbank, Zustand und Wecken liegen in
//! `start` und der Registry.
use std::collections::{BTreeMap, VecDeque};
use std::io::{BufRead, BufReader, Read};
use std::path::{Component, Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use super::model::{StationInput, StationPatch};
use crate::db::implementations::StationRow;
use crate::processes;

pub const MAX_STEPS: usize = 5;
pub const MAX_STEP_CHARS: usize = 1000;
pub const MAX_VARS: usize = 20;
pub const MAX_VAR_NAME_CHARS: usize = 32;
pub const MAX_VAR_CHARS: usize = 500;
/// So viele Zeilen der Ausgabe bleiben in `output`.
pub const OUTPUT_LINES: usize = 200;
/// Zeitlimit je Station; für Tests ein Parameter von `run`.
pub const TIME_LIMIT: Duration = Duration::from_secs(60 * 60);
/// So oft folgt die Ansicht der Ausgabe höchstens.
pub const PROGRESS_EVERY: Duration = Duration::from_secs(2);
pub const DEFAULT_PROGRAMS: [&str; 2] = ["gh", "glab"];
pub const RESULT_OK: &str = "Befehl erfolgreich";
const MAX_RESULT_CHARS: usize = 200;

/// Gründe eines Fehlers; „Schritt <n>: Exit <k>“ baut `step_failed`.
pub const REASON_TIME_LIMIT: &str = "Zeitlimit";
pub const REASON_ABORTED: &str = "abgebrochen";
pub const REASON_INTERRUPTED: &str = "unterbrochen";

pub fn step_failed(step: usize, code: i32) -> String {
    format!("Schritt {step}: Exit {code}")
}

/// Der Satz, wenn ein Programm nicht in der Freigabeliste steht.
pub fn not_allowed(program: &str) -> String {
    format!("Programm „{program}“ ist für Befehls-Stationen nicht freigegeben.")
}

/// Ein Wort in Stücken: Text, ein Wert eines Vorgängers oder eine Variable der Umgebung. Eingesetzt wird
/// aus den Stücken, nie aus dem Text eines Werts: Ein Wert, der selbst nach Platzhalter aussieht, bleibt
/// Text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Piece {
    Text(String),
    Var(String),
    Env(String),
}

/// Zerlegt einen Schritt in Wörter: Leerraum trennt, `"…"` hält Leerraum zusammen, `\"` ist ein
/// Anführungszeichen. Keine Shell: `|`, `>`, `&&`, `$` und `*` sind gewöhnliche Zeichen, jeder andere
/// Backslash bleibt stehen (Windows-Pfade).
pub fn split_words(step: &str) -> Vec<String> {
    let mut words: Vec<String> = Vec::new();
    let mut word = String::new();
    let mut has_word = false;
    let mut in_quotes = false;
    let mut chars = step.chars().peekable();
    while let Some(character) = chars.next() {
        match character {
            '\\' if chars.peek() == Some(&'"') => {
                word.push('"');
                has_word = true;
                chars.next();
            }
            '"' => {
                in_quotes = !in_quotes;
                has_word = true;
            }
            _ if character.is_whitespace() && !in_quotes => {
                if has_word {
                    words.push(std::mem::take(&mut word));
                    has_word = false;
                }
            }
            _ => {
                word.push(character);
                has_word = true;
            }
        }
    }
    if has_word {
        words.push(word);
    }
    words
}

fn is_var_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first: char| first.is_ascii_lowercase())
        && name.chars().count() <= MAX_VAR_NAME_CHARS
        && chars.all(|c: char| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
}

pub fn is_valid_var_name(name: &str) -> bool {
    is_var_name(name)
}

fn is_env_name(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first: char| first.is_ascii_alphabetic() || first == '_')
        && name.chars().count() <= 64
        && chars.all(|c: char| c.is_ascii_alphanumeric() || c == '_')
}

/// Zerlegt ein Wort in Stücke. `{{…}}` mit einem Namen (Buchstabe oder `_` am Anfang) ist ein Platzhalter
/// und muss gültig sein; alles andere (`{{.number}}` einer Vorlage, ein einzelnes `{{`) bleibt Text.
pub fn pieces(word: &str) -> Result<Vec<Piece>, String> {
    let mut result: Vec<Piece> = Vec::new();
    let mut text = String::new();
    let mut rest = word;
    while let Some(open) = rest.find("{{") {
        let Some(length) = rest[open + 2..].find("}}") else {
            break;
        };
        let inner = &rest[open + 2..open + 2 + length];
        let looks_like_name = inner
            .chars()
            .next()
            .is_some_and(|first: char| first.is_ascii_alphabetic() || first == '_');
        let piece = if !looks_like_name {
            None
        } else if let Some(name) = inner.strip_prefix("env:") {
            if !is_env_name(name) {
                return Err(format!("Platzhalter „{{{{{inner}}}}}“: ungültiger Name."));
            }
            Some(Piece::Env(name.to_owned()))
        } else if inner.contains(':') {
            return Err(format!("Platzhalter „{{{{{inner}}}}}“: ungültiger Name."));
        } else {
            if !is_var_name(inner) {
                return Err(format!(
                    "Platzhalter „{{{{{inner}}}}}“: Name klein geschrieben, höchstens \
                     {MAX_VAR_NAME_CHARS} Zeichen (a bis z, 0 bis 9, _)."
                ));
            }
            Some(Piece::Var(inner.to_owned()))
        };
        text.push_str(&rest[..open]);
        match piece {
            Some(piece) => {
                if !text.is_empty() {
                    result.push(Piece::Text(std::mem::take(&mut text)));
                }
                result.push(piece);
            }
            None => text.push_str(&rest[open..open + 2 + length + 2]),
        }
        rest = &rest[open + 2 + length + 2..];
    }
    text.push_str(rest);
    if !text.is_empty() || result.is_empty() {
        result.push(Piece::Text(text));
    }
    Ok(result)
}

/// Die Schritte als Wörter in Stücken.
pub fn compile(command: &[String]) -> Result<Vec<Vec<Vec<Piece>>>, String> {
    command
        .iter()
        .enumerate()
        .map(|(index, step): (usize, &String)| {
            split_words(step)
                .iter()
                .map(|word: &String| pieces(word))
                .collect::<Result<Vec<_>, String>>()
                .map_err(|error: String| format!("Schritt {}: {error}", index + 1))
        })
        .collect()
}

/// Die Namen der Werte der Vorgänger, die die Schritte brauchen, in der Reihenfolge des Auftretens.
pub fn needed_vars(compiled: &[Vec<Vec<Piece>>]) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    for piece in compiled.iter().flatten().flatten() {
        if let Piece::Var(name) = piece
            && !names.contains(name)
        {
            names.push(name.clone());
        }
    }
    names
}

/// Führt die Werte der direkten Vorgänger zusammen. Nennen zwei denselben Namen mit verschiedenen
/// Werten, ist das ein Fehler, der den Namen nennt.
pub fn merge_vars(
    predecessors: &[BTreeMap<String, String>],
) -> Result<BTreeMap<String, String>, String> {
    let mut merged: BTreeMap<String, String> = BTreeMap::new();
    for vars in predecessors {
        for (name, value) in vars {
            match merged.get(name) {
                Some(other) if other != value => {
                    return Err(format!(
                        "Der Wert „{name}“ kommt von zwei Vorgängern mit verschiedenen Werten."
                    ));
                }
                _ => {
                    merged.insert(name.clone(), value.clone());
                }
            }
        }
    }
    Ok(merged)
}

/// Liest eine Variable der Umgebung des Verwalters.
pub type EnvLookup<'a> = &'a dyn Fn(&str) -> Option<String>;

/// Setzt die Werte ein. Mit `env` auch `{{env:NAME}}`; ohne bleibt es als Text stehen (die Ansicht
/// zeigt so nie einen Wert der Umgebung). Ein fehlender Wert ist ein Fehler, der den Namen nennt.
pub fn render(
    compiled: &[Vec<Vec<Piece>>],
    vars: &BTreeMap<String, String>,
    env: Option<EnvLookup<'_>>,
) -> Result<Vec<Vec<String>>, String> {
    compiled
        .iter()
        .map(|step: &Vec<Vec<Piece>>| {
            step.iter()
                .map(|word: &Vec<Piece>| render_word(word, vars, env))
                .collect()
        })
        .collect()
}

fn render_word(
    word: &[Piece],
    vars: &BTreeMap<String, String>,
    env: Option<EnvLookup<'_>>,
) -> Result<String, String> {
    let mut out = String::new();
    for piece in word {
        match piece {
            Piece::Text(text) => out.push_str(text),
            Piece::Var(name) => match vars.get(name) {
                Some(value) => out.push_str(value),
                None => {
                    return Err(format!(
                        "Der Wert „{name}“ fehlt: Kein Vorgänger hat ihn mit seiner Meldung hinterlassen."
                    ));
                }
            },
            Piece::Env(name) => match env {
                Some(lookup) => match lookup(name) {
                    Some(value) => out.push_str(&value),
                    None => {
                        return Err(format!("Die Umgebungsvariable „{name}“ ist nicht gesetzt."));
                    }
                },
                None => {
                    out.push_str("{{env:");
                    out.push_str(name);
                    out.push_str("}}");
                }
            },
        }
    }
    Ok(out)
}

/// Das Programm eines Schritts: sein erstes Wort.
pub fn program_of(step: &str) -> Option<String> {
    split_words(step).into_iter().next()
}

/// Prüft die Form einer Befehls-Station: 1 bis 5 Schritte mit je 1 bis 1000 Zeichen, das Programm ein
/// bloßer Name ohne Pfad und ohne Platzhalter, gültige Platzhalter, `cwd` absolut. Der Satz nennt das
/// Feld. Die Freigabeliste und der Bereich von `cwd` prüft der Aufrufer (`check_programs`, `cwd_allowed`).
pub fn check_command(field: &str, command: &[String], cwd: Option<&str>) -> Result<(), String> {
    if command.is_empty() || command.len() > MAX_STEPS {
        return Err(format!(
            "{field}.command: 1 bis {MAX_STEPS} Schritte, nicht {}",
            command.len()
        ));
    }
    for (index, step) in command.iter().enumerate() {
        let step_field = format!("{field}.command[{index}]");
        let length = step.chars().count();
        if step.trim().is_empty() {
            return Err(format!("{step_field}: darf nicht leer sein"));
        }
        if length > MAX_STEP_CHARS {
            return Err(format!(
                "{step_field}: höchstens {MAX_STEP_CHARS} Zeichen, nicht {length}"
            ));
        }
        let Some(program) = program_of(step) else {
            return Err(format!("{step_field}: darf nicht leer sein"));
        };
        if program.is_empty() || program.contains('/') || program.contains('\\') {
            return Err(format!(
                "{step_field}: Das Programm ist ein bloßer Name ohne Pfad, nicht „{program}“."
            ));
        }
        if program.contains("{{") {
            return Err(format!(
                "{step_field}: Das Programm darf kein Platzhalter sein."
            ));
        }
    }
    compile(command).map_err(|error: String| format!("{field}.command: {error}"))?;
    if let Some(cwd) = cwd
        && !Path::new(cwd).is_absolute()
    {
        return Err(format!("{field}.cwd: ein absoluter Pfad, nicht „{cwd}“"));
    }
    Ok(())
}

/// Jedes Programm steht in der Freigabeliste; sonst der Satz des Vertrags.
pub fn check_programs(command: &[String], allowed: &[String]) -> Result<(), String> {
    for step in command {
        if let Some(program) = program_of(step)
            && !allowed.contains(&program)
        {
            return Err(not_allowed(&program));
        }
    }
    Ok(())
}

fn without_dots(path: &Path) -> Option<PathBuf> {
    let mut clean = PathBuf::new();
    for component in path.components() {
        match component {
            Component::ParentDir => return None,
            Component::CurDir => {}
            other => clean.push(other.as_os_str()),
        }
    }
    Some(clean)
}

/// Erlaubt sind der Ordner des Vorhabens, ein Repository und ein Ordner `<repo>-wt-*` daneben, jeweils samt
/// Unterordnern; `..` nie. Rein lexikalisch: Ein Symlink im Ordner wird nicht aufgelöst.
pub fn cwd_allowed(cwd: &str, folder: Option<&Path>, repositories: &[PathBuf]) -> bool {
    let Some(path) = without_dots(Path::new(cwd)).filter(|path: &PathBuf| path.is_absolute())
    else {
        return false;
    };
    if folder.is_some_and(|folder: &Path| path.starts_with(folder))
        || repositories
            .iter()
            .any(|repo: &PathBuf| path.starts_with(repo))
    {
        return true;
    }
    repositories.iter().any(|repo: &PathBuf| {
        let (Some(parent), Some(name)) = (repo.parent(), repo.file_name()) else {
            return false;
        };
        let prefix = format!("{}-wt-", name.to_string_lossy());
        path.strip_prefix(parent).ok().is_some_and(|below: &Path| {
            below
                .components()
                .next()
                .is_some_and(|first: Component<'_>| {
                    first.as_os_str().to_string_lossy().starts_with(&prefix)
                })
        })
    })
}

pub const CWD_REFUSED: &str = "cwd: erlaubt sind der Ordner des Vorhabens, ein Repository des Vorhabens und ein Ordner <repo>-wt-* daneben.";

/// Prüft die Werte, die ein Vorgänger mit seiner Meldung hinterlässt.
pub fn check_vars(vars: &BTreeMap<String, String>) -> Result<(), String> {
    if vars.len() > MAX_VARS {
        return Err(format!(
            "vars: höchstens {MAX_VARS} Werte, nicht {}",
            vars.len()
        ));
    }
    for (name, value) in vars {
        if !is_var_name(name) {
            return Err(format!(
                "vars.{name}: Name klein geschrieben, höchstens {MAX_VAR_NAME_CHARS} Zeichen (a bis z, 0 bis 9, _)"
            ));
        }
        let length = value.chars().count();
        if length == 0 || length > MAX_VAR_CHARS {
            return Err(format!(
                "vars.{name}: 1 bis {MAX_VAR_CHARS} Zeichen, nicht {length}"
            ));
        }
        if value.contains('\n') || value.contains('\r') {
            return Err(format!("vars.{name}: eine Zeile"));
        }
    }
    Ok(())
}

/// Wie ein Lauf endet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ended {
    /// Alle Schritte mit Exit 0; `result` ist die letzte nicht leere Zeile oder `RESULT_OK`.
    Success {
        result: String,
    },
    Failed {
        reason: String,
    },
}

/// Die letzte nicht leere Zeile, höchstens 200 Zeichen; ohne eine `RESULT_OK`.
pub fn result_of(output: &[String]) -> String {
    output
        .iter()
        .rev()
        .map(|line: &String| line.trim())
        .find(|line: &&str| !line.is_empty())
        .map_or_else(
            || RESULT_OK.to_owned(),
            |line: &str| line.chars().take(MAX_RESULT_CHARS).collect(),
        )
}

fn pump(source: impl Read + Send + 'static, lines: mpsc::Sender<String>) {
    thread::spawn(move || {
        for line in BufReader::new(source).split(b'\n') {
            let Ok(bytes) = line else { return };
            let text = String::from_utf8_lossy(&bytes)
                .trim_end_matches('\r')
                .to_owned();
            if lines.send(text).is_err() {
                return;
            }
        }
    });
}

/// Führt die Schritte nacheinander aus, ohne Shell: Das erste Wort ist das Programm, der Rest seine
/// Argumente, stdout und stderr laufen zusammen. `progress` bekommt den laufenden Schritt (1-basiert) und
/// die letzten 200 Zeilen, höchstens alle `PROGRESS_EVERY`, und am Ende jedes Schritts. `stop` beendet den
/// laufenden Schritt; `limit` gilt für alle Schritte zusammen.
pub fn run(
    steps: &[Vec<String>],
    cwd: &Path,
    limit: Duration,
    stop: &AtomicBool,
    progress: &mut dyn FnMut(usize, &[String]),
) -> Ended {
    let deadline = Instant::now() + limit;
    let mut tail: VecDeque<String> = VecDeque::new();
    for (index, words) in steps.iter().enumerate() {
        let number = index + 1;
        let Some((program, arguments)) = words.split_first() else {
            return Ended::Failed {
                reason: step_failed(number, -1),
            };
        };
        let mut command = processes::command(program);
        command
            .args(arguments)
            .current_dir(cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        processes::hide_console(&mut command);
        let Ok(mut child) = command.spawn() else {
            return Ended::Failed {
                reason: format!("Schritt {number}: Programm nicht gestartet"),
            };
        };
        let (sender, receiver) = mpsc::channel::<String>();
        if let Some(out) = child.stdout.take() {
            pump(out, sender.clone());
        }
        if let Some(err) = child.stderr.take() {
            pump(err, sender.clone());
        }
        drop(sender);
        progress(number, tail.make_contiguous());
        let mut last_progress = Instant::now();
        let status = loop {
            while let Ok(line) = receiver.try_recv() {
                tail.push_back(line);
                if tail.len() > OUTPUT_LINES {
                    tail.pop_front();
                }
            }
            if last_progress.elapsed() >= PROGRESS_EVERY {
                progress(number, tail.make_contiguous());
                last_progress = Instant::now();
            }
            if stop.load(Ordering::SeqCst) || Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                // Zeilen, die der Prozess noch geschrieben hat, bevor er endete.
                thread::sleep(Duration::from_millis(50));
                while let Ok(line) = receiver.try_recv() {
                    tail.push_back(line);
                    if tail.len() > OUTPUT_LINES {
                        tail.pop_front();
                    }
                }
                progress(number, tail.make_contiguous());
                let reason = if stop.load(Ordering::SeqCst) {
                    REASON_ABORTED
                } else {
                    REASON_TIME_LIMIT
                };
                return Ended::Failed {
                    reason: reason.to_owned(),
                };
            }
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => thread::sleep(Duration::from_millis(20)),
                Err(_) => {
                    let _ = child.kill();
                    return Ended::Failed {
                        reason: step_failed(number, -1),
                    };
                }
            }
        };
        // Die Leser enden mit dem Prozess; der Rest der Zeilen liegt im Kanal.
        while let Ok(line) = receiver.recv_timeout(Duration::from_millis(200)) {
            tail.push_back(line);
            if tail.len() > OUTPUT_LINES {
                tail.pop_front();
            }
        }
        progress(number, tail.make_contiguous());
        if !status.success() {
            return Ended::Failed {
                reason: step_failed(number, status.code().unwrap_or(-1)),
            };
        }
    }
    Ended::Success {
        result: result_of(tail.make_contiguous()),
    }
}

/// Die Werte der direkten Vorgänger (`after`, Plan-Kennungen), zusammengeführt.
pub fn predecessor_vars(
    after: &[String],
    rows: &[StationRow],
) -> Result<BTreeMap<String, String>, String> {
    let vars: Vec<BTreeMap<String, String>> = after
        .iter()
        .filter_map(|plan_id: &String| {
            rows.iter()
                .find(|row: &&StationRow| row.plan_id == *plan_id)
        })
        .map(|row: &StationRow| row.vars.clone())
        .collect();
    merge_vars(&vars)
}

/// Wörter wieder zu einem Schritt: Leerraum und leere Wörter in `"…"`, `"` als `\"`.
pub fn join_words(words: &[String]) -> String {
    words
        .iter()
        .map(|word: &String| {
            if word.is_empty() || word.contains(char::is_whitespace) || word.contains('"') {
                format!("\"{}\"", word.replace('"', "\\\""))
            } else {
                word.clone()
            }
        })
        .collect::<Vec<String>>()
        .join(" ")
}

/// `StationView::resolved`: die Schritte mit eingesetzten Werten, `{{env:…}}` bleibt stehen. `None`
/// bei einer anderen Art oder solange ein Wert fehlt oder sich zwei Vorgänger widersprechen.
pub fn resolved_steps(station: &StationRow, rows: &[StationRow]) -> Option<Vec<String>> {
    let run = station.command.as_ref()?;
    let vars = predecessor_vars(&station.after, rows).ok()?;
    let compiled = compile(&run.steps).ok()?;
    let steps = render(&compiled, &vars, None).ok()?;
    Some(
        steps
            .iter()
            .map(|words: &Vec<String>| join_words(words))
            .collect(),
    )
}

/// Was der Start einer Befehls-Station braucht: die Schritte als Wörter mit allen Werten, auch aus der
/// Umgebung, und der Ordner. Der Fehler ist der Text für `start_error`; er nennt Namen, nie Werte.
/// Prüft die Freigabeliste erneut (ADR 073): Sie kann sich seit dem Anlegen geändert haben.
pub fn prepare(
    station: &StationRow,
    rows: &[StationRow],
    allowed: &[String],
    default_cwd: &Path,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<(Vec<Vec<String>>, PathBuf), String> {
    let run = station
        .command
        .as_ref()
        .ok_or_else(|| "Die Station ist keine Befehls-Station.".to_owned())?;
    check_programs(&run.steps, allowed)?;
    let vars = predecessor_vars(&station.after, rows)?;
    let compiled = compile(&run.steps)?;
    let steps = render(&compiled, &vars, Some(env))?;
    let cwd = run
        .cwd
        .as_deref()
        .map_or_else(|| default_cwd.to_path_buf(), PathBuf::from);
    Ok((steps, cwd))
}

/// `run` mit „Bei Fehler“ (ADR 073): Nach einem Fehler außer „abgebrochen“ beginnt der Lauf von vorn, solange
/// `restarts` unter `max_restarts` liegt und `count_restart` (es zählt den Neustart und bekommt die bis dahin
/// gezählten) zustimmt.
#[allow(clippy::too_many_arguments)]
pub fn run_restarting(
    steps: &[Vec<String>],
    cwd: &Path,
    limit: Duration,
    stop: &AtomicBool,
    mut restarts: u32,
    max_restarts: u32,
    count_restart: &mut dyn FnMut(u32) -> bool,
    progress: &mut dyn FnMut(usize, &[String]),
) -> Ended {
    loop {
        let ended = run(steps, cwd, limit, stop, progress);
        match &ended {
            Ended::Failed { reason }
                if reason != REASON_ABORTED
                    && restarts < max_restarts
                    && count_restart(restarts) =>
            {
                restarts += 1;
            }
            _ => return ended,
        }
    }
}

/// Prüft die Befehls-Stationen neuer Eingaben (`stations`: Feldname, Station) und die Änderungen
/// (`patches`) gegen die Freigabeliste und den Bereich von `cwd` (ADR 073). Die Form prüft
/// `validate_station`.
pub fn check_inputs(
    stations: &[(String, &StationInput)],
    patches: &[StationPatch],
    allowed: &[String],
    folder: Option<&Path>,
    repositories: &[PathBuf],
) -> Result<(), String> {
    let check = |field: &str, steps: Option<&[String]>, cwd: Option<&str>| -> Result<(), String> {
        if let Some(steps) = steps {
            check_programs(steps, allowed)
                .map_err(|error: String| format!("{field}.command: {error}"))?;
        }
        if let Some(cwd) = cwd
            && !cwd.trim().is_empty()
            && !cwd_allowed(cwd.trim(), folder, repositories)
        {
            return Err(format!("{field}.{CWD_REFUSED}"));
        }
        Ok(())
    };
    for (field, station) in stations {
        check(field, station.command.as_deref(), station.cwd.as_deref())?;
    }
    for (index, patch) in patches.iter().enumerate() {
        check(
            &format!("change[{index}]"),
            patch.command.as_deref(),
            patch.cwd.as_deref(),
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(step: &str) -> Vec<String> {
        split_words(step)
    }

    fn vars(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(name, value): &(&str, &str)| ((*name).to_owned(), (*value).to_owned()))
            .collect()
    }

    fn rendered(step: &str, vars: &BTreeMap<String, String>) -> Result<Vec<String>, String> {
        let compiled = compile(&[step.to_owned()])?;
        render(&compiled, vars, None).map(|mut steps: Vec<Vec<String>>| steps.remove(0))
    }

    fn strings(values: &[&str]) -> Vec<String> {
        values
            .iter()
            .map(|value: &&str| (*value).to_owned())
            .collect()
    }

    #[test]
    fn splitting_keeps_quoted_whitespace_and_escaped_quotes() {
        assert_eq!(words("gh pr  merge   88"), ["gh", "pr", "merge", "88"]);
        assert_eq!(
            words(r#"gh pr create --title "Neu: zwei Wörter" --body "sagt \"hallo\"""#),
            [
                "gh",
                "pr",
                "create",
                "--title",
                "Neu: zwei Wörter",
                "--body",
                r#"sagt "hallo""#
            ]
        );
        assert_eq!(words(r#"git log --format="""#), ["git", "log", "--format="]);
        assert_eq!(words(r#"echo "" x"#), ["echo", "", "x"]);
        assert_eq!(
            words(r"git -C C:\work\repo status"),
            ["git", "-C", r"C:\work\repo", "status"]
        );
        assert!(words("   ").is_empty());
    }

    #[test]
    fn a_shell_operator_is_a_plain_word() {
        assert_eq!(
            words("gh pr list | grep x > out && rm -rf $HOME *"),
            [
                "gh", "pr", "list", "|", "grep", "x", ">", "out", "&&", "rm", "-rf", "$HOME", "*"
            ]
        );
    }

    #[test]
    fn a_placeholder_value_never_becomes_two_words() {
        let values = vars(&[("title", "zwei Wörter | und && mehr"), ("pr", "88")]);
        assert_eq!(
            rendered(
                "gh pr merge {{pr}} --subject {{title}} --x={{pr}}-{{pr}}",
                &values
            ),
            Ok(strings(&[
                "gh",
                "pr",
                "merge",
                "88",
                "--subject",
                "zwei Wörter | und && mehr",
                "--x=88-88"
            ]))
        );
    }

    #[test]
    fn a_missing_value_names_the_placeholder() {
        let error = rendered("gh pr merge {{pr_number}}", &BTreeMap::new()).expect_err("fehlt");
        assert!(error.contains("pr_number"), "{error}");
        let prepared = needed_vars(
            &compile(&strings(&["gh pr merge {{pr_number}} {{pr_number}} {{b}}"])).expect("gültig"),
        );
        assert_eq!(prepared, ["pr_number", "b"]);
    }

    #[test]
    fn two_predecessors_with_different_values_conflict() {
        let error = merge_vars(&[vars(&[("pr", "1"), ("a", "x")]), vars(&[("pr", "2")])])
            .expect_err("Widerspruch");
        assert!(error.contains("pr") && !error.contains("„a“"), "{error}");
        let merged =
            merge_vars(&[vars(&[("pr", "1")]), vars(&[("pr", "1"), ("b", "y")])]).expect("gleich");
        assert_eq!(merged, vars(&[("pr", "1"), ("b", "y")]));
    }

    #[test]
    fn an_env_placeholder_stays_in_resolved() {
        let values = vars(&[("v", "{{env:SECRET}}")]);
        // Ohne Umgebung bleibt der Platzhalter stehen; ein Wert, der wie einer aussieht, ist Text.
        assert_eq!(
            rendered("gh api {{env:TOKEN_NAME}} {{v}}", &values),
            Ok(strings(&[
                "gh",
                "api",
                "{{env:TOKEN_NAME}}",
                "{{env:SECRET}}"
            ]))
        );
        // Mit Umgebung wird nur der echte Platzhalter ersetzt, nie der Text eines Werts.
        let compiled = compile(&strings(&["gh api {{env:TOKEN_NAME}} {{v}}"])).expect("gültig");
        let lookup = |name: &str| (name == "TOKEN_NAME").then(|| "abc".to_owned());
        let steps = render(&compiled, &values, Some(&lookup)).expect("ersetzt");
        assert_eq!(steps[0], ["gh", "api", "abc", "{{env:SECRET}}"]);
        let unset = |_: &str| None;
        let error = render(&compiled, &values, Some(&unset)).expect_err("nicht gesetzt");
        assert!(
            error.contains("TOKEN_NAME") && !error.contains("{{"),
            "{error}"
        );
        // Eine Vorlage mit `{{.x}}` (Go) und ein einzelnes `{{` bleiben Text; ein ungültiger Name nicht.
        assert_eq!(
            rendered("gh pr view --template {{.number}} {{", &BTreeMap::new()),
            Ok(strings(&[
                "gh",
                "pr",
                "view",
                "--template",
                "{{.number}}",
                "{{"
            ]))
        );
        assert!(compile(&strings(&["gh {{Bad}}"])).is_err());
    }

    #[test]
    fn a_program_outside_the_list_is_refused_with_the_sentence() {
        let allowed = strings(&["gh", "glab"]);
        assert_eq!(
            check_programs(&strings(&["gh pr list", "glab mr list"]), &allowed),
            Ok(())
        );
        assert_eq!(
            check_programs(&strings(&["gh pr list", "rm -rf x"]), &allowed),
            Err("Programm „rm“ ist für Befehls-Stationen nicht freigegeben.".to_owned())
        );
        assert_eq!(
            not_allowed("curl"),
            "Programm „curl“ ist für Befehls-Stationen nicht freigegeben."
        );
    }

    #[test]
    fn a_program_with_a_path_is_refused() {
        for step in [
            "/usr/bin/gh pr list",
            "./gh pr list",
            r"C:\tools\gh.exe pr list",
            "bin/gh x",
            "{{prog}} x",
        ] {
            let error = check_command("stations[0]", &strings(&[step]), None).expect_err(step);
            assert!(error.contains("command[0]"), "{error}");
        }
        assert_eq!(check_command("s", &strings(&["gh pr list"]), None), Ok(()));
        assert!(check_command("s", &[], None).is_err());
        assert!(check_command("s", &strings(&["gh a"; 6]), None).is_err());
        assert!(check_command("s", &[format!("gh {}", "x".repeat(MAX_STEP_CHARS))], None).is_err());
        assert!(check_command("s", &strings(&["gh a"]), Some("relativ/pfad")).is_err());
    }

    #[test]
    fn cwd_outside_the_project_and_its_worktrees_is_refused() {
        let base = std::env::temp_dir().join("befehl-cwd");
        let folder = base.join("vorhaben");
        let repository = base.join("repo");
        let repositories = vec![repository.clone()];
        let allowed =
            |path: &Path| cwd_allowed(&path.to_string_lossy(), Some(&folder), &repositories);
        assert!(allowed(&folder));
        assert!(allowed(&folder.join("unter")));
        assert!(allowed(&repository));
        assert!(allowed(&repository.join("src")));
        assert!(allowed(&base.join("repo-wt-feature-x")));
        assert!(allowed(&base.join("repo-wt-feature-x").join("src")));
        assert!(!allowed(&base.join("repo-wtx")), "kein Worktree-Ordner");
        assert!(!allowed(&base.join("anderes")));
        assert!(!allowed(&base));
        assert!(!allowed(&folder.join("..").join("anderes")), "`..` nie");
        assert!(!cwd_allowed("relativ", Some(&folder), &repositories));
        assert!(!cwd_allowed(&folder.to_string_lossy(), None, &[]));
    }

    #[test]
    fn vars_check_name_length_and_count() {
        assert_eq!(check_vars(&vars(&[("pr", "88"), ("a_1", "x")])), Ok(()));
        assert!(check_vars(&vars(&[("Pr", "1")])).is_err());
        assert!(check_vars(&vars(&[("1a", "1")])).is_err());
        assert!(check_vars(&vars(&[(&"a".repeat(33), "1")])).is_err());
        assert_eq!(check_vars(&vars(&[(&"a".repeat(32), "1")])), Ok(()));
        assert!(check_vars(&vars(&[("a", "")])).is_err());
        assert!(check_vars(&vars(&[("a", &"x".repeat(501))])).is_err());
        assert_eq!(check_vars(&vars(&[("a", &"x".repeat(500))])), Ok(()));
        assert!(check_vars(&vars(&[("a", "zwei\nZeilen")])).is_err());
        let many: BTreeMap<String, String> = (0..21)
            .map(|index: u32| (format!("v{index}"), "1".to_owned()))
            .collect();
        assert!(check_vars(&many).is_err());
        let twenty: BTreeMap<String, String> = (0..20)
            .map(|index: u32| (format!("v{index}"), "1".to_owned()))
            .collect();
        assert_eq!(check_vars(&twenty), Ok(()));
    }

    fn steps_of(commands: &[&[&str]]) -> Vec<Vec<String>> {
        commands
            .iter()
            .map(|words: &&[&str]| strings(words))
            .collect()
    }

    fn run_in_temp(
        steps: &[Vec<String>],
        limit: Duration,
        stop: &AtomicBool,
    ) -> (Ended, Vec<usize>) {
        let mut seen: Vec<usize> = Vec::new();
        let ended = run(
            steps,
            &std::env::temp_dir(),
            limit,
            stop,
            &mut |step: usize, _: &[String]| seen.push(step),
        );
        (ended, seen)
    }

    #[test]
    fn a_successful_run_ends_with_the_last_output_line() {
        let (ended, seen) = run_in_temp(
            &steps_of(&[&["git", "--version"], &["git", "--version"]]),
            TIME_LIMIT,
            &AtomicBool::new(false),
        );
        let Ended::Success { result } = ended else {
            panic!("{ended:?}")
        };
        assert!(result.starts_with("git version"), "{result}");
        assert!(seen.contains(&1) && seen.contains(&2), "{seen:?}");
        assert_eq!(result_of(&[String::new(), "  ".to_owned()]), RESULT_OK);
        assert_eq!(result_of(&strings(&["a", "b", ""])), "b");
        assert_eq!(result_of(&["x".repeat(300)]).chars().count(), 200);
    }

    #[test]
    fn a_failing_step_sets_error_with_step_and_exit() {
        let (ended, seen) = run_in_temp(
            &steps_of(&[
                &["git", "--version"],
                &["git", "rev-parse", "--verify", "kein-ref"],
                &["git", "--version"],
            ]),
            TIME_LIMIT,
            &AtomicBool::new(false),
        );
        let Ended::Failed { reason } = ended else {
            panic!("{ended:?}")
        };
        assert!(
            reason.starts_with("Schritt 2: Exit ") && reason != "Schritt 2: Exit 0",
            "{reason}"
        );
        assert!(
            !seen.contains(&3),
            "der dritte Schritt läuft nicht mehr: {seen:?}"
        );
        assert_eq!(step_failed(2, 128), "Schritt 2: Exit 128");
        let (missing, _) = run_in_temp(
            &steps_of(&[&["kein-solches-programm-xyz"]]),
            TIME_LIMIT,
            &AtomicBool::new(false),
        );
        assert!(matches!(missing, Ended::Failed { reason } if reason.starts_with("Schritt 1:")));
    }

    #[test]
    fn the_time_limit_sets_error() {
        let (ended, _) = run_in_temp(
            &steps_of(&[&["git", "--version"]]),
            Duration::ZERO,
            &AtomicBool::new(false),
        );
        assert_eq!(
            ended,
            Ended::Failed {
                reason: REASON_TIME_LIMIT.to_owned()
            }
        );
    }

    #[test]
    fn stopping_ends_the_running_step_as_aborted() {
        let (ended, _) = run_in_temp(
            &steps_of(&[&["git", "--version"]]),
            TIME_LIMIT,
            &AtomicBool::new(true),
        );
        assert_eq!(
            ended,
            Ended::Failed {
                reason: REASON_ABORTED.to_owned()
            }
        );
    }

    #[test]
    fn on_error_restarts_the_steps_from_the_beginning() {
        let steps = steps_of(&[
            &["git", "--version"],
            &["git", "rev-parse", "--verify", "kein-ref"],
        ]);
        let mut starts = 0;
        let mut counted: Vec<u32> = Vec::new();
        let ended = run_restarting(
            &steps,
            &std::env::temp_dir(),
            TIME_LIMIT,
            &AtomicBool::new(false),
            0,
            2,
            &mut |seen: u32| {
                counted.push(seen);
                true
            },
            &mut |step: usize, _: &[String]| {
                if step == 1 {
                    starts += 1;
                }
            },
        );
        assert!(matches!(ended, Ended::Failed { .. }));
        assert_eq!(counted, [0, 1], "zwei Neustarts, dann gibt sie auf");
        assert!(starts >= 3, "drei Läufe von Schritt 1 an: {starts}");
        // „abgebrochen“ startet nie neu, und ein verweigerter Zähler beendet es.
        let stopped = run_restarting(
            &steps,
            &std::env::temp_dir(),
            TIME_LIMIT,
            &AtomicBool::new(true),
            0,
            3,
            &mut |_: u32| panic!("kein Neustart"),
            &mut |_: usize, _: &[String]| {},
        );
        assert_eq!(
            stopped,
            Ended::Failed {
                reason: REASON_ABORTED.to_owned()
            }
        );
        let refused = run_restarting(
            &steps,
            &std::env::temp_dir(),
            TIME_LIMIT,
            &AtomicBool::new(false),
            0,
            3,
            &mut |_: u32| false,
            &mut |_: usize, _: &[String]| {},
        );
        assert!(matches!(refused, Ended::Failed { .. }));
    }
}
