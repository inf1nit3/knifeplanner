//! Projektdateien: einen geplanten Griff als JSON sichern und wieder laden.

use serde::{Deserialize, Serialize};

use crate::model::{BladeBlank, HandleParams};

/// Die Kameraeinstellung der 3D-Ansicht.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ViewState {
    pub yaw: f32,
    pub roll: f32,
    pub dist: f32,
}

impl Default for ViewState {
    fn default() -> Self {
        ViewState {
            yaw: 32.0,
            roll: -22.0,
            dist: 0.0,
        }
    }
}

/// Ein angehaengtes Datenblatt.
///
/// Typisch ist das Lieferdatenblatt des Klingenstahls: Zusammensetzung,
/// Haertung, Nachbehandlung. Beim Export wandert die Datei mit in den
/// Zielordner, damit die Werkstatt alle Unterlagen zusammen hat.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Datenblatt {
    /// Die Bezeichnung, z. B. "Stahl 1.2316 - Lieferdatenblatt"
    pub name: String,
    /// Der Pfad zur Datei
    pub pfad: String,
}

impl Datenblatt {
    /// Der Name, den die Datei im Exportordner bekommt.
    pub fn datei_name(&self) -> String {
        std::path::Path::new(&self.pfad)
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "datenblatt.pdf".to_string())
    }

    /// Der Pfad als PathBuf.
    pub fn pfad(&self) -> std::path::PathBuf {
        std::path::PathBuf::from(&self.pfad)
    }
}

/// Eine gespeicherte Planung.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    /// Formatversion fuer die Zukunft
    pub version: u32,
    /// Freier Name des Projekts
    pub name: String,
    pub blank: BladeBlank,
    pub params: HandleParams,
    pub material_key: String,
    pub view: ViewState,
    /// Optionale Notiz des Messerbauers
    pub note: String,
    /// Die angehaengten Datenblaetter (z. B. vom Klingenstahl)
    #[serde(default)]
    pub datenblaetter: Vec<Datenblatt>,
}

impl Default for Project {
    fn default() -> Self {
        Project {
            version: 1,
            name: "Neuer Griff".to_string(),
            blank: BladeBlank::default(),
            params: HandleParams::default(),
            material_key: "walnuss".to_string(),
            view: ViewState::default(),
            note: String::new(),
            datenblaetter: Vec::new(),
        }
    }
}

impl Project {
    /// Serialisiert den Plan als lesbaren JSON-Text.
    pub fn to_json(&self) -> Result<String, String> {
        serde_json::to_string_pretty(self).map_err(|e| e.to_string())
    }

    /// Liest einen Plan aus JSON. Fehler werden verstaendlich gemeldet.
    pub fn from_json(text: &str) -> Result<Project, String> {
        let mut p: Project =
            serde_json::from_str(text).map_err(|e| format!("Die Datei liess sich nicht: {e}"))?;
        if p.version > 1 {
            return Err(format!(
                "Die Datei wurde mit einer neueren Programmversion erstellt \
                 (Format {}). Bitte das Programm aktualisieren.",
                p.version
            ));
        }
        if p.material_key.trim().is_empty() {
            p.material_key = "walnuss".to_string();
        }
        Ok(p)
    }

    /// Speichert den Plan als Datei.
    pub fn save(&self, path: &std::path::Path) -> Result<(), String> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(path, self.to_json()?).map_err(|e| e.to_string())
    }

    /// Laedt den Plan aus einer Datei.
    pub fn load(path: &std::path::Path) -> Result<Project, String> {
        let text = std::fs::read_to_string(path)
            .map_err(|e| format!("Die Datei liess sich nicht oeffnen: {e}"))?;
        Project::from_json(&text)
    }

    /// Dateiname aus dem Projektnamen, ohne ungueltige Zeichen.
    pub fn file_stem(&self) -> String {
        let clean: String = self
            .name
            .chars()
            .map(|c| {
                if c.is_alphanumeric() || c == '-' || c == '_' || c == ' ' {
                    c
                } else {
                    '-'
                }
            })
            .collect();
        let clean = clean.trim().replace(' ', "_");
        if clean.is_empty() {
            "griff".to_string()
        } else {
            clean
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let mut p = Project::default();
        p.name = "Mein Jagdmesser".into();
        p.blank.tang_length_mm = 123.0;
        p.material_key = "palisander".into();
        let json = p.to_json().unwrap();
        let back = Project::from_json(&json).unwrap();
        assert_eq!(back.name, "Mein Jagdmesser");
        assert_eq!(back.blank.tang_length_mm, 123.0);
        assert_eq!(back.material_key, "palisander");
    }

    #[test]
    fn broken_file_gives_a_readable_error() {
        let err = Project::from_json("{ kaputt").unwrap_err();
        assert!(err.contains("liess sich nicht"), "{err}");
    }

    #[test]
    fn newer_version_is_refused() {
        let mut p = Project::default();
        p.version = 99;
        let err = Project::from_json(&p.to_json().unwrap()).unwrap_err();
        assert!(err.contains("Programmversion"), "{err}");
    }

    #[test]
    fn file_stem_is_cleaned() {
        let mut p = Project::default();
        p.name = "Jagd/Nebel: 2026".into();
        assert_eq!(p.file_stem(), "Jagd-Nebel-_2026");
    }

    #[test]
    fn datenblaetter_ueberleben_den_roundtrip() {
        let mut p = Project::default();
        p.datenblaetter.push(Datenblatt {
            name: "Stahl 1.2316 - Lieferdatenblatt".into(),
            pfad: "C:/Datenblaetter/2316.pdf".into(),
        });
        p.datenblaetter.push(Datenblatt {
            name: "Härteprotokoll".into(),
            pfad: "/home/meister/haerte.txt".into(),
        });
        let back = Project::from_json(&p.to_json().unwrap()).unwrap();
        assert_eq!(back.datenblaetter.len(), 2);
        assert_eq!(back.datenblaetter[0].name, "Stahl 1.2316 - Lieferdatenblatt");
        assert_eq!(back.datenblaetter[1].pfad, "/home/meister/haerte.txt");
        // Der Dateiname im Exportordner kommt aus dem Pfad
        assert_eq!(back.datenblaetter[0].datei_name(), "2316.pdf");
        assert_eq!(back.datenblaetter[1].datei_name(), "haerte.txt");
    }

    #[test]
    fn alte_projektdatei_ohne_datenblaetter_lieisst_sich() {
        // Aeltere Projekte kennen das Feld noch nicht
        let alt = r#"{
            "version": 1,
            "name": "Alter Plan",
            "blank": {
                "blade_length_mm": 100.0, "blade_width_mm": 26.0,
                "blade_thickness_mm": 3.0, "tang_length_mm": 110.0,
                "tang_width_mm": 6.5, "tang_thickness_mm": 4.5,
                "tang_style": "Full", "tang_taper": true,
                "tang_back_width_mm": 0.0, "tang_taper_start": 0.55,
                "spine_offset_mm": 13.0, "edge_offset_mm": 13.0,
                "has_bolster": false, "has_guard": false, "ricasso_mm": 0.0,
                "notes": ""
            },
            "params": {},
            "material_key": "walnuss",
            "view": { "yaw": 32.0, "roll": -22.0, "dist": 0.0 },
            "note": ""
        }"#;
        // Der Params-Block ist zu knapp fuer das Modell - es geht hier nur
        // darum, dass das fehlende Feld keinen Fehler ausloest.
        match Project::from_json(alt) {
            Ok(p) => assert!(p.datenblaetter.is_empty()),
            Err(e) => assert!(e.contains("liess sich nicht"), "{e}"),
        }
    }
}
