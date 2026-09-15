#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod logging;
mod state;

use state::HeapSession;
use tauri::Emitter;

#[cfg(target_os = "windows")]
use tauri::menu::{MenuBuilder, SubmenuBuilder};

const OPEN_HEAP_MENU_ID: &str = "open-heap";
const OPEN_HEAP_MENU_EVENT: &str = "mnemosyne://open-heap-requested";

fn main() {
    logging::init();
    tracing::info!(
        version = env!("CARGO_PKG_VERSION"),
        "starting Mnemosyne desktop"
    );

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(HeapSession::new())
        .setup(|app| {
            #[cfg(target_os = "windows")]
            {
                let file_menu = SubmenuBuilder::new(app, "File")
                    .text(OPEN_HEAP_MENU_ID, "Open Heap...")
                    .build()?;
                let menu = MenuBuilder::new(app).item(&file_menu).build()?;
                app.set_menu(menu)?;
            }
            Ok(())
        })
        .on_menu_event(|app, event| {
            if event.id() == OPEN_HEAP_MENU_ID {
                if let Err(error) = app.emit(OPEN_HEAP_MENU_EVENT, ()) {
                    tracing::warn!(%error, "failed to emit native Open Heap request");
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::pick_heap_file,
            commands::take_startup_heap_file,
            commands::load_heap_from_source,
            commands::run_desktop_analysis,
            commands::run_ci_check,
            commands::generate_desktop_flamegraph,
            commands::export_desktop_report,
            commands::load_heap,
            commands::unload_heap,
            commands::get_references,
            commands::get_referrers,
            commands::query_heap,
            commands::regroup_histogram,
            commands::list_class_instances,
            commands::get_dominator_children,
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
            commands::create_ai_session,
            commands::resume_ai_session,
            commands::get_ai_session,
            commands::close_ai_session,
            commands::chat_session,
            commands::list_snapshots,
            commands::save_snapshot,
            commands::remove_snapshot,
            commands::open_snapshot,
            commands::get_desktop_log_path,
            commands::cancel_operation,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
