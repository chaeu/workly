// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // Started through the ~/.local/bin/wly link from Settings → Install CLI: be the CLI.
    let arg0 = std::env::args_os().next().unwrap_or_default();
    if wly::invoked_as_wly(std::path::Path::new(&arg0)) {
        std::process::exit(wly::run(std::env::args_os()));
    }
    workly_app_lib::run()
}
