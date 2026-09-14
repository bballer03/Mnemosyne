//! Desktop host logging — file sink under the platform data-local dir.
//!
//! Windows: `%LOCALAPPDATA%\mnemosyne\logs\desktop.log`
//! macOS: `~/Library/Application Support/mnemosyne/logs/desktop.log`
//! Linux: `~/.local/share/mnemosyne/logs/desktop.log`
//!
//! Override directory with `MNEMOSYNE_LOG_DIR`. Level via `RUST_LOG`
//! (default `info,mnemosyne_desktop=debug`).

use std::{fs, path::PathBuf, sync::OnceLock};

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

static LOG_DIR: OnceLock<PathBuf> = OnceLock::new();
static LOG_GUARD: OnceLock<WorkerGuard> = OnceLock::new();

/// Resolve (and create) the desktop log directory.
pub fn log_dir() -> PathBuf {
    LOG_DIR
        .get_or_init(|| {
            if let Ok(override_dir) = std::env::var("MNEMOSYNE_LOG_DIR") {
                let path = PathBuf::from(override_dir);
                let _ = fs::create_dir_all(&path);
                return path;
            }

            let base = dirs::data_local_dir()
                .or_else(dirs::data_dir)
                .unwrap_or_else(std::env::temp_dir);
            let path = base.join("mnemosyne").join("logs");
            let _ = fs::create_dir_all(&path);
            path
        })
        .clone()
}

/// Initialize tracing once. Safe to call repeatedly.
pub fn init() {
    if LOG_GUARD.get().is_some() {
        return;
    }

    let dir = log_dir();
    let file_appender = tracing_appender::rolling::never(&dir, "desktop.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,mnemosyne_desktop=debug,mnemosyne_core=info"));

    let file_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(non_blocking);

    // Best-effort stderr (visible in `tauri dev` / when launched from a console).
    let stderr_layer = fmt::layer()
        .with_ansi(false)
        .with_target(true)
        .with_writer(std::io::stderr);

    let _ = tracing_subscriber::registry()
        .with(filter)
        .with(file_layer)
        .with(stderr_layer)
        .try_init();

    tracing::info!(
        path = %dir.join("desktop.log").display(),
        "Mnemosyne desktop logging initialized"
    );
}

pub fn log_file_path() -> PathBuf {
    log_dir().join("desktop.log")
}
