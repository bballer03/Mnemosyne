#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod state;

use state::HeapSession;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(HeapSession::new())
        .invoke_handler(tauri::generate_handler![
            commands::pick_heap_file,
            commands::load_heap_from_source,
            commands::run_desktop_analysis,
            commands::run_ci_check,
            commands::generate_desktop_flamegraph,
            commands::load_heap,
            commands::unload_heap,
            commands::get_references,
            commands::get_referrers,
            commands::query_heap,
            commands::regroup_histogram,
            commands::explain_leak,
            commands::inspect_object,
            commands::find_all_gc_paths,
            commands::diff_objects,
            commands::find_gc_path,
            commands::map_to_code,
            commands::propose_fix,
            commands::describe_workflow,
            commands::start_workflow,
            commands::next_step,
            commands::get_workflow,
            commands::close_workflow,
            commands::list_snapshots,
            commands::save_snapshot,
            commands::remove_snapshot,
            commands::open_snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}