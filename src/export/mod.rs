//! Export der geplanten Griffe.

pub mod dxf;
pub mod pdf;
pub mod preview;
pub mod stl;
pub mod svg;

use std::path::{Path, PathBuf};

use crate::model::{BladeBlank, HandleGeometry, HandleParams};
use crate::materials::MaterialVerdict;

/// Gemeinsamer Koordinatenrahmen fuer alle 2D-Exporte.
#[derive(Clone, Copy, Debug)]
pub struct Frame {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Frame {
    pub fn from_geometry(geo: &HandleGeometry, blank: &BladeBlank) -> Frame {
        let mut pts: Vec<(f64, f64)> = geo.outline.clone();
        pts.extend(geo.tang_profile.iter().copied());
        if let Some(b) = &geo.bolster_profile {
            pts.extend(b.iter().copied());
        }
        if let Some(f) = &geo.ferrule_profile {
            pts.extend(f.iter().copied());
        }
        // Platz fuer die Bohrungsbeschriftung links und oben
        let min_x = pts.iter().map(|p| p.0).fold(f64::MAX, f64::min) - 28.0;
        let min_y = pts.iter().map(|p| p.1).fold(f64::MAX, f64::min) - 14.0;
        let max_x = pts.iter().map(|p| p.0).fold(f64::MIN, f64::max) + 14.0;
        let max_y = pts
            .iter()
            .map(|p| p.1)
            .fold(f64::MIN, f64::max)
            .max(blank.spine_offset_mm)
            + 16.0;
        Frame { min_x, min_y, max_x, max_y }
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }
    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }
}

/// Sammelt alle Dateien, die bei einem Export entstehen.
#[derive(Debug, Default)]
pub struct ExportResult {
    pub files: Vec<PathBuf>,
    pub errors: Vec<String>,
}

impl ExportResult {
    pub fn push(&mut self, path: &Path) {
        self.files.push(path.to_path_buf());
    }
}

/// Erzeugt einen sinnvollen Dateinamen aus dem Material und der Griffart.
pub fn file_stem(blank: &BladeBlank, params: &HandleParams, material: &MaterialVerdict) -> String {
    let style = match params.style {
        crate::model::HandleStyle::FullTangTwoScale => "fulltang",
        crate::model::HandleStyle::PartialTangBolster => "bastion",
        crate::model::HandleStyle::OnePiece => "einstueck",
        crate::model::HandleStyle::Ferrule => "zwinge",
    };
    format!(
        "griff_{}_{}_{}",
        style,
        blank.tang_length_mm.round() as i64,
        material.material_key
    )
}
