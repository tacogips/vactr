#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! Tauri shell for the Vactr editor.
//!
//! This binary wraps the identical Vite frontend build (`../dist`) in a native window. It
//! defines no custom `invoke` commands: file open/save go through the `dialog` and `fs`
//! plugins, and only paths granted by a user-driven dialog pick are ever readable or
//! writable, per the capability allowlist in `capabilities/default.json`.

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .run(tauri::generate_context!())
        .expect("error while running the Vactr Tauri shell");
}
