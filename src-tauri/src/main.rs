// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some(verwalter_lib::standalone::SUBCOMMAND) {
        std::process::exit(verwalter_lib::standalone::run(args[2..].to_vec()));
    }
    if args.get(1).map(String::as_str) == Some(verwalter_lib::codex::SUBCOMMAND) {
        std::process::exit(verwalter_lib::codex::run(args[2..].to_vec()));
    }
    // WebKitGTK legt mit NVIDIA-Treibern keine GBM-Puffer an ("Failed to create GBM buffer"),
    // das Fenster bleibt dann leer. Ein selbst gesetzter Wert hat Vorrang.
    // ponytail: gilt für jede GPU unter Linux; auf NVIDIA beschränken, falls das Rendern auf AMD/Intel spürbar leidet.
    #[cfg(target_os = "linux")]
    if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        // SAFETY: Hier läuft noch kein zweiter Thread; Tauri startet erst danach.
        unsafe { std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1") };
    }
    verwalter_lib::run()
}
