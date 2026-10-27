//! Datenmodell des Griff-Planers.
//!
//! Alle Masse in Millimeter, Winkel in Grad.

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TangStyle {
    /// Durchgehende Angel
    Full,
    /// Teildeckende Angel
    Partial,
    /// Verdeckte Angel
    Hidden,
}

impl TangStyle {
    pub fn label(&self) -> &'static str {
        match self {
            TangStyle::Full => "Durchgehende Angel (Full Tang)",
            TangStyle::Partial => "Teildeckende Angel (Partial Tang)",
            TangStyle::Hidden => "Verdeckte Angel (Hidden Tang)",
        }
    }
    pub fn all() -> [TangStyle; 3] {
        [TangStyle::Full, TangStyle::Partial, TangStyle::Hidden]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandleStyle {
    /// Full Tang mit zwei Griffschalen
    FullTangTwoScale,
    /// Verdeckter Tang mit Bastion
    PartialTangBolster,
    /// Ein Stueck Holz
    OnePiece,
    /// Holzgriff mit Messing-Zwinge
    Ferrule,
}

impl HandleStyle {
    pub fn label(&self) -> &'static str {
        match self {
            HandleStyle::FullTangTwoScale => "Full Tang, zwei Griffschalen",
            HandleStyle::PartialTangBolster => "Verdeckter Tang mit Bastion",
            HandleStyle::OnePiece => "Ein-Stueck-Griff aus einem Stueck Holz",
            HandleStyle::Ferrule => "Holzgriff mit Messing-Zwinge",
        }
    }
    pub fn all() -> [HandleStyle; 4] {
        [
            HandleStyle::FullTangTwoScale,
            HandleStyle::PartialTangBolster,
            HandleStyle::OnePiece,
            HandleStyle::Ferrule,
        ]
    }
    /// Braucht diese Bauart zwei Griffschalen (statt einer Halbschale)?
    pub fn has_two_scales(&self) -> bool {
        matches!(self, HandleStyle::FullTangTwoScale | HandleStyle::PartialTangBolster)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GripType {
    Hunting,
    Tactical,
    Chef,
    Folder,
    Display,
}

impl GripType {
    pub fn label(&self) -> &'static str {
        match self {
            GripType::Hunting => "Jagdmesser",
            GripType::Tactical => "Taktisch / fest",
            GripType::Chef => "Kochmesser",
            GripType::Folder => "Klappmesser",
            GripType::Display => "Sammlungs-/Ausstellungsstueck",
        }
    }
    pub fn all() -> [GripType; 5] {
        [
            GripType::Hunting,
            GripType::Tactical,
            GripType::Chef,
            GripType::Folder,
            GripType::Display,
        ]
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ButtStyle {
    Round,
    Flat,
    Flare,
}

impl ButtStyle {
    pub fn label(&self) -> &'static str {
        match self {
            ButtStyle::Round => "Rund",
            ButtStyle::Flat => "Flach",
            ButtStyle::Flare => "Ausgeweitet",
        }
    }
    pub fn all() -> [ButtStyle; 3] {
        [ButtStyle::Round, ButtStyle::Flat, ButtStyle::Flare]
    }
}

/// Der gemessene bzw. aus dem Foto geschaetzte Klingenrohling.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BladeBlank {
    pub blade_length_mm: f64,
    pub blade_width_mm: f64,
    pub blade_thickness_mm: f64,

    pub tang_length_mm: f64,
    pub tang_width_mm: f64,
    pub tang_thickness_mm: f64,
    pub tang_style: TangStyle,
    pub tang_taper: bool,
    pub tang_back_width_mm: f64,
    pub tang_taper_start: f64,

    /// Abstand der Tangmitte zur Klingenkontur am Heft
    pub spine_offset_mm: f64,
    pub edge_offset_mm: f64,

    pub has_bolster: bool,
    pub has_guard: bool,
    pub ricasso_mm: f64,
    pub notes: String,
}

impl Default for BladeBlank {
    fn default() -> Self {
        Self {
            blade_length_mm: 100.0,
            blade_width_mm: 26.0,
            blade_thickness_mm: 3.0,
            tang_length_mm: 110.0,
            tang_width_mm: 6.5,
            // Die Angel ist so dick wie der Ruecken der Klinge - bei einer
            // 3 mm dicken Schneide sind das etwa 4,5 mm.
            tang_thickness_mm: 4.5,
            tang_style: TangStyle::Full,
            tang_taper: true,
            tang_back_width_mm: 0.0,
            tang_taper_start: 0.55,
            // Der Abstand der Angelmitte zur Klingenkontur am Heft ergibt
            // zusammen die Klingenbreite (26 mm) - die Griffkontur sitzt
            // also genau auf der Klinge.
            spine_offset_mm: 13.0,
            edge_offset_mm: 13.0,
            has_bolster: false,
            has_guard: false,
            ricasso_mm: 0.0,
            notes: String::new(),
        }
    }
}

impl BladeBlank {
    pub fn tang_half_width(&self) -> f64 {
        self.tang_width_mm / 2.0
    }
}

/// Wie die Griffkontur entsteht.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileStyle {
    /// Die Kontur folgt dem Heftprofil des Rohlings.
    Blade,
    /// Eigenes Ergonomieprofil: Griffvolumen, Ballenauflage, Fingerkehle.
    Ergonomic,
}

impl ProfileStyle {
    pub fn label(&self) -> &'static str {
        match self {
            ProfileStyle::Blade => "Rohling-Profil folgen",
            ProfileStyle::Ergonomic => "Ergonomisches Griffprofil",
        }
    }
    pub fn all() -> [ProfileStyle; 2] {
        [ProfileStyle::Blade, ProfileStyle::Ergonomic]
    }
}

/// Handgroesse - bestimmt, wie dick und wie lang der Griff sein sollte.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HandSize {
    Small,
    Medium,
    Large,
}

impl HandSize {
    pub fn label(&self) -> &'static str {
        match self {
            HandSize::Small => "Kleine Hand (bis 90 mm Umfang)",
            HandSize::Medium => "Mittlere Hand (um 100 mm)",
            HandSize::Large => "Grosse Hand (ab 110 mm)",
        }
    }
    pub fn all() -> [HandSize; 3] {
        [HandSize::Small, HandSize::Medium, HandSize::Large]
    }
    /// Zielumfang des geschlossenen Griffs in Millimetern.
    pub fn target_circumference(&self) -> f64 {
        match self {
            HandSize::Small => 88.0,
            HandSize::Medium => 100.0,
            HandSize::Large => 112.0,
        }
    }
    /// Mindestlaenge des nutzbaren Griffriffs (Finger plus Ballen).
    pub fn min_grip_length(&self) -> f64 {
        match self {
            HandSize::Small => 92.0,
            HandSize::Medium => 100.0,
            HandSize::Large => 110.0,
        }
    }
}

/// Form des Griffquerschnitts.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SectionShape {
    /// Flache Seiten, nur die Kanten sind gerundet (zwei Griffschalen).
    Flat,
    /// Ovaler Querschnitt, der Griff liegt weich in der Hand.
    Oval,
    /// Oval mit einem Laengsgrat an der Bauchseite.
    Keel,
}

impl SectionShape {
    pub fn label(&self) -> &'static str {
        match self {
            SectionShape::Flat => "Flach (zwei Griffschalen)",
            SectionShape::Oval => "Oval",
            SectionShape::Keel => "Oval mit Längsgrat",
        }
    }
    pub fn all() -> [SectionShape; 3] {
        [SectionShape::Flat, SectionShape::Oval, SectionShape::Keel]
    }
}

/// Form des Griffquerschnitts.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct SectionSpec {
    pub shape: SectionShape,
    /// Komfortrundung am Ruecken in mm
    pub spine_round_mm: f64,
    /// Komfortrundung an der Bauchseite in mm
    pub belly_round_mm: f64,
    /// Tiefe des Laengsgrats an der Bauchseite in mm
    pub keel_mm: f64,
    /// Abnahme der Dicke zum Griffende hin in mm
    pub taper_mm: f64,
}

impl Default for SectionSpec {
    fn default() -> Self {
        SectionSpec {
            shape: SectionShape::Oval,
            spine_round_mm: 3.5,
            belly_round_mm: 4.0,
            keel_mm: 1.5,
            taper_mm: 2.0,
        }
    }
}

/// Gestaltungsparameter des Griffs.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleParams {
    pub style: HandleStyle,
    pub profile: ProfileStyle,
    /// Hoehe des Griffes in der Griffzone (nur beim ergonomischen Profil)
    pub grip_height_mm: f64,
    /// Tiefe der Fingerkehle
    pub finger_groove_mm: f64,
    pub target_thickness_mm: f64,
    pub scale_thickness_mm: Option<f64>,

    pub wall_min_mm: f64,
    pub glue_gap_mm: f64,

    pub spine_camber_mm: f64,
    pub belly_drop_mm: f64,
    pub butt_style: ButtStyle,
    pub butt_overhang_mm: f64,
    pub front_bevel_deg: f64,

    pub bolster_width_mm: f64,
    pub ferrule_width_mm: f64,

    pub edge_break_mm: f64,
    pub lanyard_hole: bool,
    pub lanyard_hole_dia_mm: f64,

    pub grip: GripType,
    pub hand_size: HandSize,
    /// Form des Griffquerschnitts
    pub section: SectionSpec,
    /// Gewuenschte Grifflaenge, unabhaengig von der Angel.
    /// None = so lang wie die Angel.
    pub handle_length_mm: Option<f64>,
}

impl Default for HandleParams {
    fn default() -> Self {
        Self {
            style: HandleStyle::FullTangTwoScale,
            profile: ProfileStyle::Ergonomic,
            grip_height_mm: 24.0,
            finger_groove_mm: 2.5,
            target_thickness_mm: 22.0,
            scale_thickness_mm: None,
            wall_min_mm: 2.0,
            glue_gap_mm: 0.03,
            spine_camber_mm: 1.5,
            belly_drop_mm: 3.0,
            butt_style: ButtStyle::Round,
            butt_overhang_mm: 0.0,
            front_bevel_deg: 0.0,
            bolster_width_mm: 18.0,
            ferrule_width_mm: 14.0,
            edge_break_mm: 0.8,
            lanyard_hole: true,
            lanyard_hole_dia_mm: 5.0,
            grip: GripType::Hunting,
            hand_size: HandSize::Medium,
            section: SectionSpec::default(),
            handle_length_mm: None,
        }
    }
}

/// Eine Bohrung im Griff.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PinSpec {
    /// through | friction | tang | lanyard
    pub kind: String,
    pub label: String,
    pub x_mm: f64,
    pub y_mm: f64,
    pub dia_mm: f64,
    pub depth_mm: Option<f64>,
    pub through_tang: bool,
    pub rationale: String,
    /// Die Achse der Bohrung: "z" = quer durch die Dicke, "y" = durch die
    /// Hoehe. Aeltere Projektdateien haben kein Feld - da gilt "z".
    #[serde(default = "standard_achse")]
    pub achse: String,
}

/// Die Achse, die fuer alte Dateien gilt.
fn standard_achse() -> String {
    "z".to_string()
}

/// Ergebnis der Geometrie-Engine.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct HandleGeometry {
    pub outline: Vec<(f64, f64)>,
    pub tang_profile: Vec<(f64, f64)>,
    pub recess: Vec<(f64, f64)>,
    pub pins: Vec<PinSpec>,

    pub scale_thickness_mm: f64,
    pub total_thickness_mm: f64,
    pub handle_length_mm: f64,
    pub handle_height_mm: f64,

    pub wood_start_x_mm: f64,
    pub wood_end_x_mm: f64,
    pub bolster_profile: Option<Vec<(f64, f64)>>,
    pub ferrule_profile: Option<Vec<(f64, f64)>>,

    /// Die Ringe der Griffkoerper (Loft) fuer den 3D-Export.
    pub bodies: Vec<Vec<Vec<(f64, f64, f64)>>>,
    pub metrics: Vec<(String, f64)>,
    pub warnings: Vec<String>,
    pub notes: Vec<String>,
}

impl HandleGeometry {
    pub fn metric(&self, key: &str) -> Option<f64> {
        self.metrics.iter().find(|(k, _)| k == key).map(|(_, v)| *v)
    }
}
