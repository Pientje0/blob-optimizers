// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    // `--fps-helper` runs the elevated FPS collector instead of the app window.
    let args: Vec<String> = std::env::args().collect();
    if args.get(1).map(String::as_str) == Some("--fps-helper") {
        std::process::exit(blob_optimizers_lib::fps_helper(&args[2..]));
    }
    blob_optimizers_lib::run()
}
