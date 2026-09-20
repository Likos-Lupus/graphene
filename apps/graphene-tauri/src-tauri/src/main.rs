#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    graphene_tauri_host::run();
}
