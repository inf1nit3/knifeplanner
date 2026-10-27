//! Der eingebaute Updater.
//!
//! Das Programm fragt bei GitHub nach, ob es eine neuere Version gibt, und
//! laedt sie herunter. Auf Windows wird die laufende Datei zurueckbenannt
//! und durch die neue ersetzt - Windows laesst sich eine laufende Datei
//! namens nicht ueberschreiben.

use std::io::Read;
use std::path::{Path, PathBuf};

/// Das Repository, aus dem die Versionen kommen.
pub const REPO: &str = "inf1nit3/knifeplanner";

/// Wie lange auf das Netz gewartet wird.
const ZEIT_LIMIT: std::time::Duration = std::time::Duration::from_secs(20);

/// Die Version, die in diesem Programm steckt.
pub fn eigene_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}

/// Was der Updater gefunden hat.
#[derive(Clone, Debug)]
pub struct Update {
    /// Die neue Version, ohne fuehrendes "v".
    pub version: String,
    /// Der direkte Download der neuen Programmdatei.
    pub url: String,
    /// Die Groesse der Datei in Bytes.
    pub groesse: u64,
    /// Die Anmerkungen zur Version.
    pub notiz: String,
}

/// Der Zustand des Update-Vorgangs.
#[derive(Clone, Debug, Default)]
pub enum Zustand {
    /// Es wurde noch nicht gesucht.
    #[default]
    Leer,
    /// Die Anfrage laeuft.
    Prueft,
    /// Es ist nichts Neues da.
    Aktuell,
    /// Es gibt eine neuere Version.
    Neu(Box<Update>),
    /// Die Datei wird geladen.
    Laedt {
        version: String,
        geladen: u64,
        gesamt: u64,
    },
    /// Die Datei ist da und wartet auf das Einsetzen.
    Fertig {
        version: String,
        datei: PathBuf,
    },
    /// Da ist etwas schiefgegangen.
    Fehler(String),
}

/// Vergleicht zwei Versionsstände.
///
/// `"0.2.0"` ist neuer als `"0.1.9"`, `"1.0.0"` neuer als `"0.9.9"`.
pub fn ist_neuer(neu: &str, alt: &str) -> bool {
    let teile = |v: &str| -> Vec<i64> {
        v.trim_start_matches('v')
            .split(['.', '-', '+'])
            .map(|t| t.parse::<i64>().unwrap_or(0))
            .collect()
    };
    let a = teile(neu);
    let b = teile(alt);
    for i in 0..a.len().max(b.len()) {
        let x = a.get(i).copied().unwrap_or(0);
        let y = b.get(i).copied().unwrap_or(0);
        if x != y {
            return x > y;
        }
    }
    false
}

/// Fragt GitHub nach der neuesten Version.
///
/// Der Aufruf blockiert - er gehört in einen eigenen Thread.
pub fn pruefe(repo: &str) -> Result<Option<Update>, String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("KnifePlanner/{}", eigene_version()))
        .timeout(ZEIT_LIMIT)
        .build()
        .map_err(|e| format!("Netzwerk-Client: {e}"))?;
    let url = format!("https://api.github.com/repos/{repo}/releases/latest");
    let antwort = client
        .get(&url)
        .send()
        .map_err(|e| format!("GitHub nicht erreichbar: {e}"))?;
    if !antwort.status().is_success() {
        return Err(format!(
            "GitHub antwortet mit {} - gibt es dort Releases?",
            antwort.status()
        ));
    }
    let daten: serde_json::Value = antwort
        .json()
        .map_err(|e| format!("Antwort nicht lesbar: {e}"))?;
    let tag = daten
        .get("tag_name")
        .and_then(|w| w.as_str())
        .unwrap_or_default()
        .to_string();
    if tag.is_empty() {
        return Err("Die Antwort von GitHub war unvollstaendig".into());
    }
    let version = tag.trim_start_matches('v').to_string();
    if !ist_neuer(&version, eigene_version()) {
        return Ok(None);
    }
    // Die Programmdatei aus dem Release heraussuchen
    let mut url = String::new();
    let mut groesse = 0u64;
    if let Some(assets) = daten.get("assets").and_then(|w| w.as_array()) {
        for a in assets {
            let name = a.get("name").and_then(|w| w.as_str()).unwrap_or("");
            if name.ends_with(".exe") {
                url = a
                    .get("browser_download_url")
                    .and_then(|w| w.as_str())
                    .unwrap_or_default()
                    .to_string();
                groesse = a.get("size").and_then(|w| w.as_u64()).unwrap_or(0);
                break;
            }
        }
    }
    if url.is_empty() {
        return Err(format!(
            "Version {version} gefunden, aber ohne Programmdatei im Release."
        ));
    }
    let notiz = daten
        .get("body")
        .and_then(|w| w.as_str())
        .unwrap_or_default()
        .trim()
        .to_string();
    Ok(Some(Update {
        version,
        url,
        groesse,
        notiz,
    }))
}

/// Laedt die neue Programmdatei herunter.
///
/// `fortschritt` bekommt (geladen, gesamt) gemeldet. Der Aufruf blockiert.
pub fn lade(
    update: &Update,
    ziel: &Path,
    fortschritt: &mut dyn FnMut(u64, u64),
) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .user_agent(format!("KnifePlanner/{}", eigene_version()))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("Netzwerk-Client: {e}"))?;
    let mut antwort = client
        .get(&update.url)
        .send()
        .map_err(|e| format!("Download fehlgeschlagen: {e}"))?;
    if !antwort.status().is_success() {
        return Err(format!("Download: {}", antwort.status()));
    }
    let gesamt = antwort.content_length().unwrap_or(update.groesse);
    if let Some(parent) = ziel.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mut datei = std::fs::File::create(ziel).map_err(|e| format!("Datei: {e}"))?;
    let mut geladen = 0u64;
    let mut puffer = [0u8; 64 * 1024];
    loop {
        let n = antwort
            .read(&mut puffer)
            .map_err(|e| format!("Lesefehler: {e}"))?;
        if n == 0 {
            break;
        }
        std::io::Write::write_all(&mut datei, &puffer[..n]).map_err(|e| format!("Schreiben: {e}"))?;
        geladen += n as u64;
        fortschritt(geladen, gesamt);
    }
    if gesamt > 0 && geladen < gesamt {
        return Err(format!("Download unvollstaendig: {geladen} von {gesamt} Bytes"));
    }
    Ok(())
}

/// Setzt die neue Programmdatei an die Stelle der alten.
///
/// Auf Windows laesst sich die laufende Datei nicht ueberschreiben.
/// Deshalb wird sie kurz umbenannt und die neue an ihren Platz gelegt.
/// Die umbenannte Datei bleibt liegen und wird beim naechsten Start
/// weggeraeumt.
pub fn setze_ein(neu: &Path, ziel: &Path) -> Result<(), String> {
    if !neu.exists() {
        return Err("Die heruntergeladene Datei fehlt".into());
    }
    // Die alte Datei wegrraeumen, falls sie noch liegt
    let alt = alt_datei(ziel);
    if alt.exists() {
        let _ = std::fs::remove_file(&alt);
    }
    // Die laufende Datei umbenennen - das geht unter Windows auch im
    // laufenden Betrieb
    if ziel.exists() {
        std::fs::rename(ziel, &alt)
            .map_err(|e| format!("Die alte Datei konnte nicht umbenannt werden: {e}"))?;
    }
    match std::fs::copy(neu, ziel) {
        Ok(_) => Ok(()),
        Err(e) => {
            // Wenn es nicht klappt, die alte Datei zurueckholen
            if alt.exists() {
                let _ = std::fs::rename(&alt, ziel);
            }
            Err(format!("Die neue Datei konnte nicht eingesetzt werden: {e}"))
        }
    }
}

/// Der Name, unter dem die ersetzte Datei liegt.
fn alt_datei(ziel: &Path) -> PathBuf {
    let mut name = ziel
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "knifeplanner.exe".to_string());
    name.push_str(".alt");
    ziel.with_file_name(name)
}

/// Raeumt die ersetzte Datei vom letzten Start weg.
///
/// Wird beim Programmstart gerufen - unter Windows ist die alte Datei
/// dann nicht mehr gesperrt.
pub fn raeume_auf() {
    if let Ok(pfad) = std::env::current_exe() {
        let alt = alt_datei(&pfad);
        if alt.exists() {
            let _ = std::fs::remove_file(alt);
        }
    }
}

/// Der Ort, an dem die neue Datei zwischengelagert wird.
pub fn temp_datei(version: &str) -> PathBuf {
    let ordner = std::env::temp_dir();
    ordner.join(format!("knifeplanner_{version}.neu"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versionen_werden_verglichen() {
        assert!(ist_neuer("0.2.0", "0.1.0"));
        assert!(ist_neuer("1.0.0", "0.9.9"));
        assert!(ist_neuer("0.1.1", "0.1.0"));
        assert!(ist_neuer("v0.3.0", "0.2.9"));
        assert!(!ist_neuer("0.1.0", "0.1.0"));
        assert!(!ist_neuer("0.1.0", "0.2.0"));
        assert!(!ist_neuer("1.0.0", "1.0.0-beta"));
    }

    #[test]
    fn eigene_version_ist_vollstaendig() {
        let v = eigene_version();
        assert!(!v.is_empty());
        assert_eq!(v.split('.').count(), 3, "unbekanntes Versionsformat: {v}");
    }

    #[test]
    fn alte_datei_hat_einen_anderen_namen() {
        let ziel = PathBuf::from("C:/Programme/knifeplanner.exe");
        let alt = alt_datei(&ziel);
        assert_ne!(ziel, alt);
        assert!(alt.to_string_lossy().ends_with(".exe.alt"), "{alt:?}");
    }
}
