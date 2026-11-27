//! Zeigt die rohe Antwort von `get_usage` der echten Claude-Kommandozeile (Taiga 270): dieselbe
//! Anfrage wie `usage::fetch_claude`, kein Modellaufruf und kein Kontingentverbrauch. Ausgegeben
//! werden nur `subscription_type`, `rate_limits_available` und `rate_limits`, ohne Konto-Kennungen,
//! dazu die Fenster, die der Core daraus liest.
//! Aufruf: `usage-probe`
use std::process::ExitCode;
use std::time::Duration;

use serde_json::{Value, json};
use verwalter_lib::agents::claude::helper::ask_once;
use verwalter_lib::agents::claude::locate::find_claude;
use verwalter_lib::agents::claude::protocol::get_usage;
use verwalter_lib::agents::claude::stats::usage_snapshot;

fn main() -> ExitCode {
    let Some(exe) = find_claude() else {
        eprintln!("Claude-Kommandozeile nicht gefunden");
        return ExitCode::FAILURE;
    };
    let dir = std::env::temp_dir();
    let body = match ask_once(&exe, &dir, get_usage(), Duration::from_secs(60)) {
        Ok(body) => body,
        Err(error) => {
            eprintln!("get_usage fehlgeschlagen: {error}");
            return ExitCode::FAILURE;
        }
    };
    let short: Value = json!({
        "subscription_type": body.get("subscription_type"),
        "rate_limits_available": body.get("rate_limits_available"),
        "rate_limits": body.get("rate_limits"),
    });
    println!(
        "{}",
        serde_json::to_string_pretty(&short).unwrap_or_default()
    );
    match usage_snapshot(&body) {
        Some(snapshot) => {
            for limit in snapshot.limits {
                println!(
                    "LIMIT {} {} {:?}",
                    limit.kind, limit.percent, limit.resets_at
                );
            }
        }
        None => println!("UNLESBAR"),
    }
    ExitCode::SUCCESS
}
