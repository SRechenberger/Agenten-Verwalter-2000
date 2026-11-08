//! Was der Server am Gast aus den Dateien eines Hosts macht (ADR 059): Brücke und Policy wie für
//! lokale Artefakte, Betrachter ohne Abruf, Fehler als Status. Der Host ist hier ein Zähler.
use std::sync::{Arc, Mutex};

use super::*;
use crate::artifacts::server::{RemoteSource, Reply, Source, file_reply, respond};

const SCOPE: &str = "http://127.0.0.1:1/token/sitzung/";

/// Die Fragen an den Host als (Befehl, Pfad).
type Asked = Arc<Mutex<Vec<(String, String)>>>;

/// Ein Host, der `answers` liefert und festhält, was der Gast bei ihm erfragt hat.
fn host(
    answer: impl Fn(&str, &str) -> Result<Content, CommandError> + Send + Sync + 'static,
) -> (Source, Asked) {
    let asked: Asked = Arc::new(Mutex::new(Vec::new()));
    let log = Arc::clone(&asked);
    let source = Source::Remote(RemoteSource::new(move |command: &str, path: &str| {
        log.lock()
            .expect("Sperre")
            .push((command.to_owned(), path.to_owned()));
        answer(command, path)
    }));
    (source, asked)
}

fn content(content_type: &str, body: &[u8]) -> Result<Content, CommandError> {
    Ok(Content {
        content_type: content_type.to_owned(),
        data_base64: STANDARD.encode(body),
    })
}

fn get(source: &Source, relative: &str, query: &str) -> Reply {
    respond(
        source,
        SCOPE,
        relative.to_owned(),
        query == "list=1",
        query == "view=1",
    )
}

#[test]
fn a_remote_html_page_gets_the_bridge_and_the_policy_of_its_folder() {
    let (source, _) = host(|_, _| content("text/html; charset=utf-8", b"<body><p>x</p></body>"));
    let reply = get(&source, "seite.html", "");
    let body = String::from_utf8(reply.body).expect("UTF-8");
    assert_eq!(reply.status, 200);
    assert!(body.contains("<script>") && body.ends_with("</body>"));
    let csp = reply.csp.expect("Policy");
    assert!(csp.contains(SCOPE) && csp.contains("form-action 'none'"));
}

#[test]
fn a_remote_file_that_is_not_html_is_passed_on_unchanged() {
    let (source, _) = host(|_, _| content("image/png", b"png-bytes"));
    let reply = get(&source, "bild.png", "");
    assert_eq!(reply.content_type, "image/png");
    assert_eq!(reply.body, b"png-bytes");
}

#[test]
fn a_remote_site_folder_asks_the_host_for_its_index_page() {
    let (source, asked) = host(|_, _| content("text/html; charset=utf-8", b"<p>x</p>"));
    get(&source, "site/", "");
    assert_eq!(
        *asked.lock().expect("Sperre"),
        vec![("remote_artifact".to_owned(), "site/index.html".to_owned())]
    );
}

#[test]
fn a_remote_folder_listing_asks_the_host_for_the_folder_with_a_slash() {
    let (source, asked) = host(|_, _| content("application/json", b"{\"entries\":[]}"));
    let reply = get(&source, "ordner/", "list=1");
    assert_eq!(reply.content_type, "application/json");
    assert_eq!(reply.body, b"{\"entries\":[]}");
    assert_eq!(
        *asked.lock().expect("Sperre"),
        vec![("remote_artifact".to_owned(), "ordner/".to_owned())]
    );
}

#[test]
fn a_remote_viewer_page_is_rendered_here_without_asking_the_host() {
    let (source, asked) = host(|_, _| content("text/csv", b"a,b"));
    let reply = get(&source, "daten/tabelle.csv", "view=1");
    assert_eq!(reply.status, 200);
    assert_eq!(reply.content_type, "text/html; charset=utf-8");
    assert!(asked.lock().expect("Sperre").is_empty());
}

#[test]
fn a_remote_folder_viewer_is_rendered_here_without_asking_the_host() {
    let (source, asked) = host(|_, _| content("text/csv", b"a,b"));
    let reply = get(&source, "ordner/", "view=1");
    assert_eq!(reply.status, 200);
    assert!(asked.lock().expect("Sperre").is_empty());
}

#[test]
fn a_host_that_does_not_answer_is_a_bad_gateway() {
    let (source, _) = host(|_, _| Err(CommandError::HostUnreachable("Host".to_owned())));
    assert_eq!(get(&source, "seite.html", "").status, 502);
    assert_eq!(get(&source, "ordner/", "list=1").status, 502);
}

#[test]
fn a_file_the_host_refuses_is_forbidden_and_a_missing_one_is_not_found() {
    let (source, _) = host(|_, path| {
        if path == "fehlt.html" {
            Err(CommandError::FileNotFound(path.to_owned()))
        } else {
            Err(CommandError::FileNotAllowed(path.to_owned()))
        }
    });
    assert_eq!(get(&source, "fehlt.html", "").status, 404);
    assert_eq!(get(&source, "../geheim.txt", "").status, 403);
}

#[test]
fn an_attachment_of_a_host_is_served_as_data_that_runs_nothing() {
    let remote = RemoteSource::new(|command: &str, path: &str| {
        assert_eq!((command, path), ("remote_file", "/ws/.anhaenge/a/bild.svg"));
        content("image/svg+xml", b"<svg/>")
    });
    let reply = file_reply(&remote, "/ws/.anhaenge/a/bild.svg");
    assert_eq!(reply.status, 200);
    assert_eq!(reply.content_type, "image/svg+xml");
    let csp = reply.csp.expect("Policy");
    assert!(csp.contains("sandbox") && csp.contains("default-src 'none'"));
}

#[test]
fn an_attachment_the_host_refuses_is_forbidden() {
    let remote =
        RemoteSource::new(|_, path: &str| Err(CommandError::FileNotAllowed(path.to_owned())));
    assert_eq!(file_reply(&remote, "/fremd.txt").status, 403);
}

#[test]
fn a_local_html_page_still_gets_the_bridge_and_a_parent_segment_is_still_refused() {
    let dir = std::env::temp_dir().join(format!("verwalter-served-{}", uuid::Uuid::new_v4()));
    fs::create_dir_all(&dir).expect("Ordner");
    fs::write(dir.join("seite.html"), "<body>x</body>").expect("schreiben");
    let source = Source::Local(dir.clone());
    let page = get(&source, "seite.html", "");
    let refused = get(&source, "../x.txt", "");
    let missing = get(&source, "fehlt.html", "");
    let _ = fs::remove_dir_all(&dir);
    assert!(
        String::from_utf8(page.body)
            .expect("UTF-8")
            .contains("<script>")
    );
    assert_eq!((refused.status, missing.status), (403, 404));
}
