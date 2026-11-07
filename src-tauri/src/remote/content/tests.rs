//! Was `remote_artifact` und `remote_file` einem Gast liefern und was nicht (ADR 059).
use std::path::PathBuf;

use super::*;

/// Ein eigener Ordner unter dem temporären Ordner, der am Ende verschwindet.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        let dir = std::env::temp_dir().join(format!("verwalter-content-{}", uuid::Uuid::new_v4()));
        fs::create_dir_all(&dir).expect("Ordner anlegen");
        Scratch(dir)
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.0.join(relative)
    }

    fn write(&self, relative: &str, body: &[u8]) -> PathBuf {
        let path = self.path(relative);
        fs::create_dir_all(path.parent().expect("Elternordner")).expect("Ordner anlegen");
        fs::write(&path, body).expect("Datei schreiben");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn is_refused(result: Result<Content, CommandError>) -> bool {
    matches!(result, Err(CommandError::FileNotAllowed(_)))
}

#[test]
fn remote_artifact_delivers_a_file_of_the_artifact_folder_with_its_content_type() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    scratch.write(".artefakte/seite.html", b"<p>Hallo</p>");
    let found = artifact(&dir, "seite.html").expect("Datei");
    assert_eq!(found.content_type, "text/html; charset=utf-8");
    assert_eq!(found.bytes().expect("Base64"), b"<p>Hallo</p>");
}

#[test]
fn remote_artifact_delivers_a_file_inside_a_folder_artifact() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    scratch.write(".artefakte/site/css/stil.css", b"p {}");
    let found = artifact(&dir, "site/css/stil.css").expect("Datei");
    assert_eq!(found.content_type, "text/css; charset=utf-8");
}

#[test]
fn remote_artifact_refuses_a_path_with_parent_directory_segments() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    scratch.write(".artefakte/seite.html", b"x");
    scratch.write("geheim.txt", b"nicht fuer Gaeste");
    assert!(is_refused(artifact(&dir, "../geheim.txt")));
    assert!(is_refused(artifact(&dir, "site/../../geheim.txt")));
}

#[test]
fn remote_artifact_refuses_an_absolute_path() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    let outside = scratch.write("geheim.txt", b"nicht fuer Gaeste");
    assert!(is_refused(artifact(&dir, &outside.to_string_lossy())));
}

#[test]
fn remote_artifact_reports_a_missing_file_as_not_found() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    fs::create_dir_all(&dir).expect("Ordner");
    assert!(matches!(
        artifact(&dir, "fehlt.html"),
        Err(CommandError::FileNotFound(_))
    ));
}

#[cfg(unix)]
#[test]
fn remote_artifact_refuses_a_symlink_that_leaves_the_artifact_folder() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    fs::create_dir_all(&dir).expect("Ordner");
    let outside = scratch.write("geheim.txt", b"nicht fuer Gaeste");
    std::os::unix::fs::symlink(&outside, dir.join("link.txt")).expect("Symlink");
    assert!(is_refused(artifact(&dir, "link.txt")));
}

#[test]
fn remote_artifact_with_a_trailing_slash_lists_the_folder_as_json() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    scratch.write(".artefakte/ordner/a.png", b"png");
    scratch.write(".artefakte/ordner/unter/b.md", b"# b");
    let found = artifact(&dir, "ordner/").expect("Liste");
    assert_eq!(found.content_type, "application/json");
    let listing: serde_json::Value =
        serde_json::from_slice(&found.bytes().expect("Base64")).expect("JSON");
    let paths: Vec<&str> = listing["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect();
    assert!(paths.contains(&"a.png"));
    assert!(paths.contains(&"unter/b.md"));
}

#[test]
fn remote_artifact_refuses_to_list_a_folder_outside_the_artifact_folder() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    fs::create_dir_all(&dir).expect("Ordner");
    scratch.write("draussen/a.txt", b"x");
    assert!(is_refused(artifact(&dir, "../draussen/")));
}

#[test]
fn remote_artifact_refuses_a_file_over_the_size_limit() {
    let scratch = Scratch::new();
    let dir = scratch.path(".artefakte");
    let big = scratch.write(".artefakte/gross.mp4", b"");
    fs::OpenOptions::new()
        .write(true)
        .open(&big)
        .and_then(|file| file.set_len(MAX_BYTES + 1))
        .expect("Größe setzen");
    assert!(is_refused(artifact(&dir, "gross.mp4")));
}

#[test]
fn remote_file_delivers_an_attachment_by_absolute_path() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    let image = scratch.write("ws/.anhaenge/abc/bild.png", b"png-bytes");
    let found = workspace_file(&workspace, &image.to_string_lossy()).expect("Datei");
    assert_eq!(found.content_type, "image/png");
    assert_eq!(found.bytes().expect("Base64"), b"png-bytes");
}

#[test]
fn remote_file_delivers_a_file_by_path_relative_to_the_workspace() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    scratch.write("ws/.anhaenge/abc/bild.png", b"png-bytes");
    let found = workspace_file(&workspace, ".anhaenge/abc/bild.png").expect("Datei");
    assert_eq!(found.bytes().expect("Base64"), b"png-bytes");
}

#[test]
fn remote_file_refuses_a_file_outside_the_workspace() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(&workspace).expect("Ordner");
    let outside = scratch.write("fremd/geheim.txt", b"nicht fuer Gaeste");
    assert!(is_refused(workspace_file(
        &workspace,
        &outside.to_string_lossy()
    )));
}

#[test]
fn remote_file_refuses_parent_directory_segments_that_leave_the_workspace() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(&workspace).expect("Ordner");
    scratch.write("geheim.txt", b"nicht fuer Gaeste");
    assert!(is_refused(workspace_file(&workspace, "../geheim.txt")));
}

#[test]
fn remote_file_refuses_a_sibling_folder_that_only_shares_the_workspace_name_as_prefix() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(&workspace).expect("Ordner");
    let sibling = scratch.write("ws-fremd/geheim.txt", b"nicht fuer Gaeste");
    assert!(is_refused(workspace_file(
        &workspace,
        &sibling.to_string_lossy()
    )));
}

#[cfg(unix)]
#[test]
fn remote_file_refuses_a_symlink_that_leaves_the_workspace() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(&workspace).expect("Ordner");
    let outside = scratch.write("fremd/geheim.txt", b"nicht fuer Gaeste");
    std::os::unix::fs::symlink(&outside, workspace.join("link.txt")).expect("Symlink");
    assert!(is_refused(workspace_file(&workspace, "link.txt")));
}

#[test]
fn remote_file_refuses_a_folder() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(workspace.join("ordner")).expect("Ordner");
    assert!(is_refused(workspace_file(&workspace, "ordner")));
}

#[test]
fn remote_file_reports_a_missing_file_as_not_found() {
    let scratch = Scratch::new();
    let workspace = scratch.path("ws");
    fs::create_dir_all(&workspace).expect("Ordner");
    assert!(matches!(
        workspace_file(&workspace, "fehlt.png"),
        Err(CommandError::FileNotFound(_))
    ));
}
