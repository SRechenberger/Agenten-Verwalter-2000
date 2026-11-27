pub mod workspace;

use std::path::PathBuf;

/// Ein Pfad aus Git-Ausgabe oder URL, die `/` als Trenner nennen, als Dateisystempfad. Nur unter
/// Windows werden die Schrägstriche zu `\`; anderswo ist `\` ein gültiges Zeichen im Dateinamen
/// und der Text bleibt, wie er ist.
pub fn native_path(path: &str) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.replace('/', "\\"))
    } else {
        PathBuf::from(path)
    }
}

/// Ein kanonischer Pfad so, wie die App ihn speichert und zeigt: unter Windows liefert
/// `fs::canonicalize` `\\?\C:\…` oder `\\?\UNC\server\…`; daraus wird `C:\…` und `\\server\…`.
/// Jeder andere Text bleibt, wie er ist.
pub fn display_path(path: &str) -> String {
    match path.strip_prefix(r"\\?\UNC\") {
        Some(share) => format!(r"\\{share}"),
        None => path.strip_prefix(r"\\?\").unwrap_or(path).to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::display_path;

    #[test]
    fn a_displayed_path_has_no_verbatim_prefix() {
        assert_eq!(display_path(r"\\?\C:\Users\a\vault"), r"C:\Users\a\vault");
        assert_eq!(
            display_path(r"\\?\UNC\server\teil\vault"),
            r"\\server\teil\vault"
        );
        assert_eq!(display_path(r"C:\Users\a"), r"C:\Users\a");
        assert_eq!(display_path(r"\\server\teil"), r"\\server\teil");
        assert_eq!(display_path("/home/a/vault"), "/home/a/vault");
    }
}
