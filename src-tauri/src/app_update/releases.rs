//! Abfrage der GitHub-Releases und Wahl der Datei, die zur Art der Installation passt.
use std::cmp::Ordering;
use std::time::Duration;

use serde::Deserialize;

use super::model::{InstallKind, ReleaseInfo};
use crate::cli_update::compare_versions;

const RELEASES_URL: &str =
    "https://api.github.com/repos/SRechenberger/Agenten-Verwalter-2000/releases?per_page=10";
/// Nur Dateien unter dieser Adresse werden geladen; eine fremde Adresse im Release ist ein Fehler.
pub const DOWNLOAD_PREFIX: &str =
    "https://github.com/SRechenberger/Agenten-Verwalter-2000/releases/download/";
const FETCH_TIMEOUT: Duration = Duration::from_secs(10);
const DIGEST_PREFIX: &str = "sha256:";
// Vertrag mit dem Schritt „Dateien einsammeln“ in `.github/workflows/release.yml` (ADR 041).
const PORTABLE_SUFFIX: &str = "_x64-portable.exe";
const INSTALLER_SUFFIX: &str = "-setup.exe";

pub const MANUAL_TEXT: &str = "Dieses Paket aktualisiert sich nicht selbst.";
const NO_MATCHING_ASSET: &str = "Das Release enthält keine passende Datei.";
const FOREIGN_URL: &str = "Die Datei des Releases liegt nicht bei den Releases des Verwalters.";
const NO_DIGEST: &str = "Das Release enthält keine Prüfsumme.";

#[derive(Debug, Clone)]
pub struct Release {
    pub version: String,
    pub notes: String,
    pub published_at: Option<String>,
    pub url: String,
    pub assets: Vec<Asset>,
}

#[derive(Debug, Clone)]
pub struct Asset {
    pub name: String,
    pub url: String,
    pub size: u64,
    /// SHA-256 als Hex, ohne `sha256:`; `None`, wenn GitHub keine nennt.
    pub sha256: Option<String>,
}

impl Release {
    pub fn info(&self) -> ReleaseInfo {
        ReleaseInfo {
            version: self.version.clone(),
            notes: self.notes.clone(),
            published_at: self.published_at.clone(),
            url: self.url.clone(),
        }
    }
}

#[derive(Deserialize)]
struct ApiRelease {
    tag_name: String,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    body: Option<String>,
    published_at: Option<String>,
    html_url: String,
    #[serde(default)]
    assets: Vec<ApiAsset>,
}

#[derive(Deserialize)]
struct ApiAsset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}

/// Fragt die veröffentlichten Releases ab, neueste zuerst; Entwürfe und Vorabversionen fehlen.
/// `app_version` geht nur in den `User-Agent`.
pub fn fetch(app_version: &str) -> Result<Vec<Release>, String> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(FETCH_TIMEOUT))
        .build()
        .into();
    let mut response = agent
        .get(RELEASES_URL)
        .header(
            "User-Agent",
            format!("Agenten-Verwalter-2000/{app_version}"),
        )
        .header("Accept", "application/vnd.github+json")
        .call()
        .map_err(fetch_error)?;
    let body = response
        .body_mut()
        .read_to_string()
        .map_err(|error: ureq::Error| format!("Antwort von GitHub nicht lesbar: {error}"))?;
    parse(&body)
}

/// Liest die Antwort der Releases-API; öffentlich für das Prüfprogramm.
pub fn parse(body: &str) -> Result<Vec<Release>, String> {
    let api_releases: Vec<ApiRelease> =
        serde_json::from_str(body).map_err(|_| "Antwort von GitHub unlesbar".to_owned())?;
    let mut releases: Vec<Release> = api_releases
        .into_iter()
        .filter(|release: &ApiRelease| !release.draft && !release.prerelease)
        .map(into_release)
        .collect();
    releases
        .sort_by(|left: &Release, right: &Release| compare_versions(&right.version, &left.version));
    Ok(releases)
}

/// Alle Releases neuer als `current`, in der Reihenfolge von `releases`.
pub fn newer_than<'a>(releases: &'a [Release], current: &str) -> Vec<&'a Release> {
    releases
        .iter()
        .filter(|release: &&Release| {
            compare_versions(&release.version, current) == Ordering::Greater
        })
        .collect()
}

/// Die Datei, die für `kind` geladen wird; jede Prüfung, die scheitert, verhindert den Download.
pub fn pick_asset(release: &Release, kind: InstallKind) -> Result<&Asset, String> {
    let suffix = match kind {
        InstallKind::Portable => PORTABLE_SUFFIX,
        InstallKind::Installer => INSTALLER_SUFFIX,
        InstallKind::Manual => return Err(MANUAL_TEXT.to_owned()),
    };
    let asset = release
        .assets
        .iter()
        .find(|asset: &&Asset| asset.name.ends_with(suffix))
        .ok_or_else(|| NO_MATCHING_ASSET.to_owned())?;
    if !asset.url.starts_with(DOWNLOAD_PREFIX) {
        return Err(FOREIGN_URL.to_owned());
    }
    if asset.sha256.is_none() {
        return Err(NO_DIGEST.to_owned());
    }
    Ok(asset)
}

fn into_release(release: ApiRelease) -> Release {
    let version = release
        .tag_name
        .strip_prefix('v')
        .unwrap_or(&release.tag_name)
        .to_owned();
    Release {
        version,
        notes: release.body.unwrap_or_default(),
        published_at: release.published_at,
        url: release.html_url,
        assets: release.assets.into_iter().map(into_asset).collect(),
    }
}

fn into_asset(asset: ApiAsset) -> Asset {
    // Ein `digest` mit anderem Verfahren als SHA-256 zählt wie ein fehlender.
    let sha256 = asset
        .digest
        .as_deref()
        .and_then(|digest: &str| digest.strip_prefix(DIGEST_PREFIX))
        .filter(|hex: &&str| {
            hex.len() == 64 && hex.chars().all(|digit: char| digit.is_ascii_hexdigit())
        })
        .map(str::to_ascii_lowercase);
    Asset {
        name: asset.name,
        url: asset.browser_download_url,
        size: asset.size,
        sha256,
    }
}

fn fetch_error(error: ureq::Error) -> String {
    match error {
        ureq::Error::StatusCode(status @ (403 | 429)) => format!(
            "GitHub antwortet mit {status} — Abfragegrenze erreicht, später erneut versuchen."
        ),
        other => format!("Abfrage der Releases scheitert: {other}"),
    }
}
