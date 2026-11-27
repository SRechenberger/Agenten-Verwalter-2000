//! Review-Kommentare aus der Changes-Ansicht: Prüfung beim Senden und der Text, den der Agent
//! dazu bekommt.
pub mod model;

use std::path::PathBuf;

use crate::artifacts::extern_files;
use crate::changes::model::DiffLineKind;
use crate::error::CommandError;
use crate::filesystem::native_path;
use crate::review::model::{CommentTarget, ReviewComment};

const MIN_FENCE_LENGTH: usize = 3;
const FENCE_CHARACTER: char = '`';
const MAX_SELECTOR_CHARS: usize = 300;

/// Ein Kommentar auf einem Abschnittskopf oder ohne Text kann die Oberfläche nicht erzeugen —
/// kommt er an, ist etwas kaputt, und es wird nichts gesendet.
pub fn validate(comments: &[ReviewComment]) -> Result<(), CommandError> {
    for comment in comments {
        match &comment.target {
            CommentTarget::Diff if comment.kind == DiffLineKind::Hunk => {
                return Err(CommandError::Internal(
                    "Review-Kommentar auf einem Abschnittskopf".to_owned(),
                ));
            }
            CommentTarget::Artifact { file, selector, .. }
                if file.trim().is_empty()
                    || selector.trim().is_empty()
                    || selector.chars().count() > MAX_SELECTOR_CHARS =>
            {
                return Err(CommandError::Internal(
                    "UngÃ¼ltiges Ziel fÃ¼r Artefakt-Kommentar".to_owned(),
                ));
            }
            CommentTarget::Image {
                x,
                y,
                width,
                height,
                image_width,
                image_height,
                ..
            } if *width == 0
                || *height == 0
                || x.checked_add(*width)
                    .is_none_or(|right| right > *image_width)
                || y.checked_add(*height)
                    .is_none_or(|bottom| bottom > *image_height) =>
            {
                return Err(CommandError::Internal(
                    "UngÃ¼ltiger Bildbereich im Review-Kommentar".to_owned(),
                ));
            }
            _ => {}
        }
        if comment.text.trim().is_empty() {
            return Err(CommandError::Internal("Leerer Review-Kommentar".to_owned()));
        }
    }
    Ok(())
}

/// Der Text einer Nachricht samt Kommentaren. `folders[index]` ist der aufgelöste Ordner zu
/// `comments[index]`; ohne Ordner nennt der Ort den Repository-Namen. Ohne Kommentare bleibt der
/// Text unverändert.
pub fn agent_text(text: &str, comments: &[ReviewComment], folders: &[Option<PathBuf>]) -> String {
    if comments.is_empty() {
        return text.to_owned();
    }
    let items: Vec<String> = comments
        .iter()
        .enumerate()
        .map(|(index, comment): (usize, &ReviewComment)| {
            let folder: Option<&PathBuf> = folders.get(index).and_then(Option::as_ref);
            match &comment.target {
                CommentTarget::Diff => {
                    let fence = fence(&comment.code);
                    format!(
                        "{}. {}\n{fence}\n{}{}\n{fence}\n{}",
                        index + 1,
                        location(comment, folder),
                        sign(comment.kind),
                        comment.code,
                        comment.text,
                    )
                }
                CommentTarget::Artifact { file, selector, .. } => {
                    // Eine Datei aus einem eingetragenen Ordner (`.extern/<schlüssel>/<name>`): `folder`
                    // ist ihr eigener Ordner, es zählt nur der Name.
                    let file = extern_files::file_name(file).unwrap_or(file);
                    let location = match folder {
                        Some(folder) => folder.join(native_path(file)).display().to_string(),
                        None => format!("Artefakt {file}"),
                    };
                    let fence = fence(&comment.code);
                    format!(
                        "{}. {location} — Element {selector}\n{fence}\n{}\n{fence}\n{}",
                        index + 1,
                        comment.code,
                        comment.text,
                    )
                }
                CommentTarget::Image {
                    path,
                    x,
                    y,
                    width,
                    height,
                    image_width,
                    image_height,
                    ..
                } => format!(
                    "{}. Bild {path} — Bereich x {x}, y {y}, {width} × {height} Pixel (Bild {image_width} × {image_height})\n{}",
                    index + 1,
                    comment.text,
                ),
            }
        })
        .collect();
    let mut result = String::new();
    // Das Voranstellen lässt ein `/skill` am Anfang der Nachricht stehen.
    if !text.is_empty() {
        result.push_str(text);
        result.push_str("\n\n");
    }
    let heading = if comments
        .iter()
        .all(|comment| comment.target == CommentTarget::Diff)
    {
        "Review-Kommentare zu den Changes"
    } else {
        "Review-Kommentare"
    };
    result.push_str(&format!(
        "{heading} ({}):\n\n{}",
        comments.len(),
        items.join("\n\n")
    ));
    result
}

fn location(comment: &ReviewComment, folder: Option<&PathBuf>) -> String {
    let path: String = match folder {
        Some(folder) => {
            let mut full: PathBuf = folder.clone();
            for part in comment.path.split('/') {
                full.push(part);
            }
            full.display().to_string()
        }
        None => format!("{}/{}", comment.repository_name, comment.path),
    };
    match comment.kind {
        DiffLineKind::Deleted => format!("{path} — gelöschte Zeile, vorher Zeile {}", comment.line),
        _ => format!("{path}:{}", comment.line),
    }
}

fn sign(kind: DiffLineKind) -> &'static str {
    match kind {
        DiffLineKind::Added => "+ ",
        DiffLineKind::Deleted => "- ",
        DiffLineKind::Context => "  ",
        DiffLineKind::Hunk => "",
    }
}

/// Länger als jede Backtick-Folge im Code, damit der Code den Zaun nicht beenden kann.
fn fence(code: &str) -> String {
    let mut longest_run: usize = 0;
    let mut current_run: usize = 0;
    for character in code.chars() {
        if character == FENCE_CHARACTER {
            current_run += 1;
            longest_run = longest_run.max(current_run);
        } else {
            current_run = 0;
        }
    }
    FENCE_CHARACTER
        .to_string()
        .repeat((longest_run + 1).max(MIN_FENCE_LENGTH))
}

#[cfg(test)]
mod extern_tests {
    use std::path::PathBuf;

    use super::agent_text;
    use crate::changes::model::DiffLineKind;
    use crate::review::model::{CommentTarget, ReviewComment};

    #[test]
    fn a_comment_on_a_file_from_a_registered_folder_names_its_real_path() {
        let comment = ReviewComment {
            target: CommentTarget::Artifact {
                file: ".extern/ab12/notiz.md".to_owned(),
                selector: "body > h1".to_owned(),
                label: "Notiz".to_owned(),
            },
            repository_key: String::new(),
            repository_name: String::new(),
            path: String::new(),
            kind: DiffLineKind::Context,
            line: 0,
            code: String::new(),
            text: "Bitte kürzen".to_owned(),
        };
        let folder = PathBuf::from("/home/a/vault");
        let text = agent_text("", &[comment], &[Some(folder.clone())]);
        let expected = format!("{} — Element body > h1", folder.join("notiz.md").display());
        assert!(text.contains(&expected), "{text}");
        assert!(!text.contains(".extern"), "{text}");
    }
}
