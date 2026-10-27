//! Maschinenbezogene Einstellungen (eigene Datei neben der Konfiguration).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::logfile;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Einstellungen {
    /// Pfad zu FreeCADCmd (exe) oder dem bin-Ordner. Leer = automatisch suchen.
    pub freecad_pfad: String,
    /// Das 3D-Modell mit FreeCAD bauen.
    ///
    /// `None` heisst: automatisch - FreeCAD wird benutzt, wenn es
    /// installiert ist. `Some(true)` heisst: immer, `Some(false)`: nie.
    /// Aeltere Dateien haben kein Feld und bleiben damit automatisch.
    #[serde(default)]
    pub freecad_bevorzugen: Option<bool>,
}

/// Pfad der Einstellungsdatei.
pub fn pfad() -> PathBuf {
    logfile::app_dir().join("einstellungen.json")
}

/// Laedt die Einstellungen (fehlende Datei = Vorgaben).
pub fn lade() -> Einstellungen {
    std::fs::read_to_string(pfad())
        .ok()
        .and_then(|t| serde_json::from_str(&t).ok())
        .unwrap_or_default()
}

/// Speichert die Einstellungen.
pub fn speichern(e: &Einstellungen) -> Result<(), String> {
    let dir = logfile::app_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    std::fs::write(pfad(), serde_json::to_string_pretty(e).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}

/// Oeffnet einen Ordner im Dateimanager des Systems.
///
/// Geht unter Linux nicht immer - dann wird der Ordner nur im Log vermerkt.
pub fn ordner_oeffnen(pfad: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    let mut cmd = {
        let mut c = std::process::Command::new("explorer");
        c.arg(pfad);
        c
    };
    #[cfg(target_os = "macos")]
    let mut cmd = {
        let mut c = std::process::Command::new("open");
        c.arg(pfad);
        c
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut cmd = {
        let mut c = std::process::Command::new("xdg-open");
        c.arg(pfad);
        c
    };
    cmd.spawn()
        .map(|_| ())
        .map_err(|e| format!("Ordner konnte nicht geoeffnet werden: {e}"))
}
