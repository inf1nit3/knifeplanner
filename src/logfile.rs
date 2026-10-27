//! Protokolldatei.
//!
//! Alles, was in der App passiert, wird zusaetzlich in eine Datei geschrieben,
//! damit sich ein Fehler auch nach dem Beenden nachvollziehen laesst.
//!
//! Windows: %APPDATA%\\knifeplanner\\knifeplanner.log
//! sonst:    ~/.knifeplanner/knifeplanner.log

use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};

static LOGFILE: OnceLock<Mutex<Option<File>>> = OnceLock::new();

/// Der Ordner, in dem die App ihre Dateien ablegt.
pub fn app_dir() -> PathBuf {
    let dir = if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join("knifeplanner")
    } else {
        std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".knifeplanner")
    };
    dir
}

/// Der Pfad der Protokolldatei.
pub fn log_path() -> PathBuf {
    app_dir().join("knifeplanner.log")
}

/// Startet die Protokolldatei.
pub fn init() {
    let dir = app_dir();
    let _ = std::fs::create_dir_all(&dir);
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path());
    let slot = LOGFILE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        *guard = file.ok();
    }
    log(&format!(
        "=== KnifePlanner gestartet (Version {}) ===",
        env!("CARGO_PKG_VERSION")
    ));
}

/// Schreibt eine Zeile in die Protokolldatei.
pub fn log(text: &str) {
    let slot = LOGFILE.get_or_init(|| Mutex::new(None));
    if let Ok(mut guard) = slot.lock() {
        if let Some(file) = guard.as_mut() {
            let _ = writeln!(file, "{text}");
            let _ = file.flush();
        }
    }
}

/// Schreibt eine Zeile mit Zeitstempel.
pub fn log_with_time(text: &str) {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    log(&format!("[{secs}] {text}"));
}
