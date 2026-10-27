//! Formatierung der Werkstatttexte.
//!
//! Die Dokumente (Bohrplan, Bauplan, Textplan) gehen an die Werkstatt.
//! Dort schreibt man 14,3 mm und nicht 14.3 mm - deshalb steht hier die
//! deutsche Zahlformatierung. Die CAD-Ausgaben (DXF, SVG) behalten den
//! Punkt, weil er dort zum gaengigen Format gehoert.

/// Zahl mit deutschem Dezimalkomma.
pub fn de(v: f64, stellen: usize) -> String {
    let s = match stellen {
        0 => format!("{v:.0}"),
        1 => format!("{v:.1}"),
        2 => format!("{v:.2}"),
        _ => format!("{v:.3}"),
    };
    s.replace('.', ",")
}

/// Zahl mit deutschem Dezimalkomma und Einheit mm.
pub fn de_mm(v: f64, stellen: usize) -> String {
    format!("{} mm", de(v, stellen))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn komma_statt_punkt() {
        assert_eq!(de(14.3, 1), "14,3");
        assert_eq!(de(14.0, 0), "14");
        assert_eq!(de(2.45, 2), "2,45");
        assert_eq!(de(-0.5, 1), "-0,5");
    }

    #[test]
    fn mit_einheit() {
        assert_eq!(de_mm(14.3, 1), "14,3 mm");
        assert_eq!(de_mm(110.0, 0), "110 mm");
    }
}
