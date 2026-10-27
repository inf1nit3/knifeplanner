//! DXF-Export der Griffschalen (R12 / AC1009, ASCII).
//!
//! Layer:
//!   SCHALE      - Aussenkontur der Griffschale (Zuschnitt, 1:1 in mm)
//!   ANGEL       - Angelprofil als Referenzlage
//!   AUSSPA      - Aussparung (Angel + Klebespalt)
//!   BOHRUNGEN   - Bohrkonturen der Vorsteckbolzen, Reibnieten, Lanyardloch
//!   TEXT        - Beschriftung

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use crate::geometry::tang_half_width;
use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};

use super::{file_stem, Frame};
use crate::materials::MaterialVerdict;

const LAYERS: &[(&str, i32)] = &[
    ("SCHALE", 7),
    ("ANGEL", 3),
    ("AUSSPA", 5),
    ("BOHRUNGEN", 1),
    ("TEXT", 2),
];

fn polyline(out: &mut String, layer: &str, pts: &[(f64, f64)], closed: bool) {
    if pts.len() < 2 {
        return;
    }
    let _ = writeln!(out, "0\nPOLYLINE");
    let _ = writeln!(out, "8\n{layer}");
    let _ = writeln!(out, "66\n1");
    let _ = writeln!(out, "70\n{}", if closed { 1 } else { 0 });
    let _ = writeln!(out, "10\n0.0\n20\n0.0\n30\n0.0");
    for &(x, y) in pts {
        let _ = writeln!(out, "0\nVERTEX\n8\n{layer}\n10\n{x:.4}\n20\n{y:.4}\n30\n0.0");
    }
    let _ = writeln!(out, "0\nSEQEND");
}

fn circle(out: &mut String, layer: &str, x: f64, y: f64, r: f64) {
    let _ = writeln!(
        out,
        "0\nCIRCLE\n8\n{layer}\n10\n{x:.4}\n20\n{y:.4}\n30\n0.0\n40\n{r:.4}"
    );
}

fn text(out: &mut String, layer: &str, x: f64, y: f64, h: f64, s: &str) {
    let _ = writeln!(out, "0\nTEXT\n8\n{layer}\n10\n{x:.4}\n20\n{y:.4}\n30\n0.0\n40\n{h:.3}\n1\n{s}");
}

/// Erzeugt den DXF-Inhalt.
///
/// Bei zwei Griffschalen werden beide spiegelbildlich untereinander abgelegt,
/// damit der Zuschnitt direkt vom Blatt geschnitten werden kann.
pub fn build_dxf(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &MaterialVerdict,
) -> String {
    let mut out = String::new();
    out.push_str("0\nSECTION\n2\nHEADER\n9\n$ACADVER\n1\nAC1009\n9\n$INSUNITS\n70\n4\n0\nENDSEC\n");
    out.push_str("0\nSECTION\n2\nTABLES\n0\nTABLE\n2\nLAYER\n70\n");
    out.push_str(&format!("{}\n", LAYERS.len()));
    for (name, color) in LAYERS {
        out.push_str(&format!(
            "0\nLAYER\n2\n{name}\n70\n0\n62\n{color}\n6\nCONTINUOUS\n"
        ));
    }
    out.push_str("0\nENDTAB\n0\nENDSEC\n0\nSECTION\n2\nENTITIES\n");

    let two_scales = params.style.has_two_scales();
    // Die zweite Schale liegt spiegelbildlich (an der Angelmitte gespiegelt)
    // unter der ersten, mit 12 mm Abstand zum Beschnitt.
    let mirror_dy = if two_scales {
        let ys: Vec<f64> = geo.outline.iter().map(|&(_, y)| y).collect();
        let min_y = ys.iter().cloned().fold(f64::MAX, f64::min);
        2.0 * min_y - 12.0
    } else {
        0.0
    };

    for pass in 0..if two_scales { 2 } else { 1 } {
        let flip = pass == 1;
        let map = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> {
            pts.iter()
                .map(|&(x, y)| {
                    if flip {
                        (x, -y + mirror_dy)
                    } else {
                        (x, y)
                    }
                })
                .collect()
        };
        polyline(&mut out, "SCHALE", &map(&geo.outline), true);
        polyline(&mut out, "ANGEL", &map(&geo.tang_profile), true);
        polyline(&mut out, "AUSSPA", &map(&geo.recess), true);
        if let Some(b) = &geo.bolster_profile {
            polyline(&mut out, "SCHALE", &map(b), true);
        }
        if let Some(f) = &geo.ferrule_profile {
            polyline(&mut out, "SCHALE", &map(f), true);
        }

        // Die Bohrungen liegen auf der Angelmittenlinie - in jeder Schale an
        // ihrer eigenen Position.
        for pin in &geo.pins {
            let (x, y) = if flip {
                (pin.x_mm, -pin.y_mm + mirror_dy)
            } else {
                (pin.x_mm, pin.y_mm)
            };
            circle(&mut out, "BOHRUNGEN", x, y, pin.dia_mm / 2.0);
        }
    }

    // Beschriftung
    let (z_laenge, z_hoehe, z_dicke) = crate::geometry::zuschnitt(
        &geo.outline,
        params,
        geo.scale_thickness_mm,
        geo.total_thickness_mm,
    );
    let label = format!(
        "{} | Schalendicke {:.1} mm | Angel {:.1} x {:.1} mm | {} | Zuschnitt Holzleiste {:.0} x {:.0} x {:.0} mm",
        material.material_name,
        geo.scale_thickness_mm,
        blank.tang_width_mm,
        blank.tang_thickness_mm,
        match params.style {
            HandleStyle::FullTangTwoScale => "Full Tang, 2 Schalen",
            HandleStyle::PartialTangBolster => "Partial Tang + Bastion",
            HandleStyle::OnePiece => "Ein-Stueck-Griff",
            HandleStyle::Ferrule => "Holzgriff mit Zwinge",
        },
        z_laenge,
        z_hoehe,
        z_dicke
    );
    text(&mut out, "TEXT", 0.0, geo.tang_profile[0].1 + 14.0, 3.0, &label);

    for pin in &geo.pins {
        let t = format!(
            "{} d{:.1}",
            pin.label,
            pin.dia_mm
        );
        text(
            &mut out,
            "TEXT",
            pin.x_mm - 8.0,
            pin.y_mm + pin.dia_mm / 2.0 + 1.5,
            2.2,
            &t,
        );
    }

    out.push_str("0\nENDSEC\n0\nEOF\n");
    out
}

/// Schreibt die DXF-Datei.
pub fn write_dxf(
    dir: &Path,
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &MaterialVerdict,
) -> std::io::Result<std::path::PathBuf> {
    fs::create_dir_all(dir)?;
    let stem = file_stem(blank, params, material);
    let path = dir.join(format!("{stem}.dxf"));
    fs::write(&path, build_dxf(geo, blank, params, material))?;
    Ok(path)
}

/// Hilfsfunktion fuer die Vorschau: Tanghoehe an einer Stelle.
pub fn tang_width_at(blank: &BladeBlank, x: f64) -> f64 {
    tang_half_width(blank, x) * 2.0
}

/// Rahmen fuer die 2D-Ausgabe.
pub fn frame_for(geo: &HandleGeometry, blank: &BladeBlank) -> Frame {
    Frame::from_geometry(geo, blank)
}
