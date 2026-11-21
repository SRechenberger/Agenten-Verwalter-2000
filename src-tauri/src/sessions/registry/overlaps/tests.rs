//! Der Schreib-Ordner einer Session (ADR 037): Ein Ticket-Worktree zählt nur mit einer geschriebenen
//! Datei darin, ein bloßes Lesen macht ihn nicht zum Schreib-Ordner.
use super::*;

fn snapshot(ticket_folders: &[&str]) -> WriterSnapshot {
    WriterSnapshot {
        id: "s".to_owned(),
        number: 1,
        name: "Review".to_owned(),
        project_name: "P".to_owned(),
        repositories: Vec::new(),
        ticket_folders: ticket_folders.iter().map(PathBuf::from).collect(),
    }
}

fn written(paths: &[&str]) -> HashMap<String, f64> {
    paths
        .iter()
        .map(|path: &&str| (normalize_path(path), 1.0))
        .collect()
}

#[test]
fn a_ticket_worktree_only_read_is_no_write_folder() {
    let session = snapshot(&["/work/repo-wt-fix"]);
    assert!(write_folders(&session, &written(&[])).is_empty());
    // Eine Datei im Haupt-Checkout macht den Worktree nicht zum Schreib-Ordner.
    assert!(write_folders(&session, &written(&["/work/repo/a.rs"])).is_empty());
}

#[test]
fn a_ticket_worktree_with_a_written_file_is_a_write_folder() {
    let session = snapshot(&["/work/repo-wt-fix", "/work/repo-wt-other"]);
    let folders = write_folders(&session, &written(&["/work/repo-wt-fix/src/a.rs"]));
    assert_eq!(
        folders.keys().cloned().collect::<Vec<String>>(),
        vec![normalize_path("/work/repo-wt-fix")]
    );
}
