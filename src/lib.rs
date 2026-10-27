//! KnifePlanner - Griff-Planer fuer Messerdesigns.
//!
//! Kern: Rohling + Material -> Vision-Analyse -> Geometrie-Engine -> Plan ->
//! Exporte (DXF, SVG, STL, Bohrplan-PDF, Bauplan-PDF).

pub mod blade;
pub mod export;
pub mod freecad;
pub mod geometry;
pub mod logfile;
pub mod materials;
pub mod measure;
pub mod mesh;
pub mod model;
pub mod plan;
pub mod project;
pub mod render3d;
pub mod selftest;
pub mod settings;
pub mod update;
pub mod vision;

#[cfg(not(target_arch = "wasm32"))]
pub mod ui;
