#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::HeapSession;

fn main() {
    tauri::Builder::default()
        .manage(HeapSession::new())
        .invoke_handler(tauri::generate_handler![
            commands::load_heap,
            commands::unload_heap,
            commands::get_references,
            commands::get_referrers,
            commands::query_heap,
            commands::explain_leak,
            commands::find_gc_path,
            commands::map_to_code,
            commands::propose_fix,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}