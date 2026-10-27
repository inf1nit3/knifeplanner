//! Messwerkzeug: am Foto abmessen statt Werte schaetzen.
//!
//! Ablauf:
//!   1. Foto mit bekanntem Referenzmass aufnehmen (Kreditkarte 85,60 mm,
//!      Lineal, Zollstock).
//!   2. Referenz kalibrieren: Linie ueber das Referenzobjekt ziehen und die
//!      echte Laenge eintragen.
//!   3. Messen: Linie ueber das Messobjekt ziehen, Laenge ablesen und einem
//!      Parameter zuordnen.

use egui::Pos2;

use crate::model::BladeBlank;

/// Welchem Parameter eine Messung zugeordnet wird.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MeasureTarget {
    BladeLength,
    BladeWidth,
    BladeThickness,
    TangLength,
    TangWidth,
    TangThickness,
    SpineOffset,
    EdgeOffset,
}

impl MeasureTarget {
    pub fn all() -> [MeasureTarget; 8] {
        [
            MeasureTarget::BladeLength,
            MeasureTarget::BladeWidth,
            MeasureTarget::BladeThickness,
            MeasureTarget::TangLength,
            MeasureTarget::TangWidth,
            MeasureTarget::TangThickness,
            MeasureTarget::SpineOffset,
            MeasureTarget::EdgeOffset,
        ]
    }

    pub fn label(&self) -> &'static str {
        match self {
            MeasureTarget::BladeLength => "Klingenlänge",
            MeasureTarget::BladeWidth => "Klingenbreite am Heft",
            MeasureTarget::BladeThickness => "Klingendicke",
            MeasureTarget::TangLength => "Angel-Länge",
            MeasureTarget::TangWidth => "Angel-Breite",
            MeasureTarget::TangThickness => "Angel-Dicke",
            MeasureTarget::SpineOffset => "Rückenabstand",
            MeasureTarget::EdgeOffset => "Schneidenabstand",
        }
    }

    pub fn unit_hint(&self) -> &'static str {
        match self {
            MeasureTarget::BladeThickness | MeasureTarget::TangThickness => "Dicke am besten von der Seite fotografieren",
            _ => "am besten von oben fotografieren",
        }
    }

    /// Schreibt den gemessenen Wert in den Rohling.
    pub fn apply(&self, blank: &mut BladeBlank, value: f64) {
        match self {
            MeasureTarget::BladeLength => blank.blade_length_mm = value,
            MeasureTarget::BladeWidth => blank.blade_width_mm = value,
            MeasureTarget::BladeThickness => blank.blade_thickness_mm = value,
            MeasureTarget::TangLength => blank.tang_length_mm = value,
            MeasureTarget::TangWidth => blank.tang_width_mm = value,
            MeasureTarget::TangThickness => blank.tang_thickness_mm = value,
            MeasureTarget::SpineOffset => blank.spine_offset_mm = value,
            MeasureTarget::EdgeOffset => blank.edge_offset_mm = value,
        }
    }

    /// Wert, der aktuell im Rohling steht.
    pub fn current(&self, blank: &BladeBlank) -> f64 {
        match self {
            MeasureTarget::BladeLength => blank.blade_length_mm,
            MeasureTarget::BladeWidth => blank.blade_width_mm,
            MeasureTarget::BladeThickness => blank.blade_thickness_mm,
            MeasureTarget::TangLength => blank.tang_length_mm,
            MeasureTarget::TangWidth => blank.tang_width_mm,
            MeasureTarget::TangThickness => blank.tang_thickness_mm,
            MeasureTarget::SpineOffset => blank.spine_offset_mm,
            MeasureTarget::EdgeOffset => blank.edge_offset_mm,
        }
    }
}

/// Kalibrierung: eine Linie im Bild und ihre echte Laenge.
#[derive(Clone, Debug)]
pub struct Calibration {
    /// Startpunkt in Bildkoordinaten (Pixel)
    pub p1: Pos2,
    pub p2: Pos2,
    /// Echte Laenge der Referenz in Millimeter
    pub mm: f64,
}

impl Calibration {
    /// Millimeter pro Bildpixel.
    pub fn mm_per_px(&self) -> f64 {
        let px = self.p1.distance(self.p2);
        if px < 1e-6 {
            0.0
        } else {
            self.mm / px as f64
        }
    }
}

/// Eine abgenommene Messung.
#[derive(Clone, Debug)]
pub struct Measurement {
    pub p1: Pos2,
    pub p2: Pos2,
    /// Länge in Millimeter
    pub mm: f64,
    /// Text neben der Linie
    pub label: String,
}

/// Wandelt eine Bildstrecke in Millimeter um.
pub fn to_mm(cal: &Calibration, p1: Pos2, p2: Pos2) -> f64 {
    p1.distance(p2) as f64 * cal.mm_per_px()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calibration_converts_pixels_to_millimetres() {
        // Referenz: 100 Pixel = 85,60 mm (Kreditkarte)
        let cal = Calibration {
            p1: egui::pos2(0.0, 0.0),
            p2: egui::pos2(100.0, 0.0),
            mm: 85.6,
        };
        assert!((cal.mm_per_px() - 0.856).abs() < 1e-9);

        // 250 Pixel entsprechen dann 214 mm
        let mm = to_mm(&cal, egui::pos2(0.0, 0.0), egui::pos2(250.0, 0.0));
        assert!((mm - 214.0).abs() < 1e-6, "{mm}");
    }

    #[test]
    fn diagonal_measurement_uses_euclidean_distance() {
        // Referenz: 10 Pixel = 20 mm, also 2 mm pro Pixel
        let cal = Calibration {
            p1: egui::pos2(0.0, 0.0),
            p2: egui::pos2(10.0, 0.0),
            mm: 20.0,
        };
        // 3-4-5-Dreieck: 5 Pixel * 2 mm/Pixel = 10 mm
        let mm = to_mm(&cal, egui::pos2(0.0, 0.0), egui::pos2(3.0, 4.0));
        assert!((mm - 10.0).abs() < 1e-6, "{mm}");
    }

    #[test]
    fn degenerate_calibration_yields_zero() {
        let cal = Calibration {
            p1: egui::pos2(5.0, 5.0),
            p2: egui::pos2(5.0, 5.0),
            mm: 85.6,
        };
        assert_eq!(cal.mm_per_px(), 0.0);
    }

    #[test]
    fn target_writes_into_the_blank() {
        let mut blank = BladeBlank::default();
        MeasureTarget::TangLength.apply(&mut blank, 123.4);
        assert!((blank.tang_length_mm - 123.4).abs() < 1e-9);
        assert!((MeasureTarget::TangLength.current(&blank) - 123.4).abs() < 1e-9);
    }
}
