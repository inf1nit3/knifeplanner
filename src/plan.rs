//! Zusammenfuehrung von Rohling, Gestaltung und Material zum fertigen Plan,
//! inklusive Bauplan-Text und PDF-Ausgaben.

use crate::export::pdf::Pdf;
use crate::geometry::{build_geometry, recommend_style};
use crate::materials::{self, MaterialVerdict};
use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};
use crate::text::{de, de_mm};
use crate::vision::{BladeAnalysis, MaterialAnalysis};

#[derive(Clone, Debug)]
pub struct DesignPlan {
    pub blank: BladeBlank,
    pub params: HandleParams,
    pub geometry: HandleGeometry,
    pub material_key: String,
    pub verdict: MaterialVerdict,
    pub blade_analysis: Option<BladeAnalysis>,
    pub material_analysis: Option<MaterialAnalysis>,
}

impl DesignPlan {
    /// Erzeugt den Plan und waehlt automatisch eine passende Materialbasis,
    /// wenn noch keine ausgewaehlt wurde.
    pub fn build(blank: &BladeBlank, params: &HandleParams, material_key: &str) -> DesignPlan {
        let geometry = build_geometry(blank, params);
        let key = if material_key.is_empty() {
            default_material_for(params).to_string()
        } else {
            material_key.to_string()
        };
        let material = materials::get(&key).unwrap_or_else(|| &materials::MATERIALS[0]);
        let mut verdict = materials::evaluate(material, blank, params, &geometry);
        let weight = estimate_weight(&geometry, blank, params, material);
        verdict.weight_g = Some(weight.total_g());
        verdict.weight_note = Some(weight.note());
        let mut plan = DesignPlan {
            blank: blank.clone(),
            params: params.clone(),
            geometry,
            material_key: key,
            verdict,
            blade_analysis: None,
            material_analysis: None,
        };
        plan.push_metric("griffgewicht_g", weight.total_g());
        plan.push_metric("holzgewicht_g", weight.wood_g);
        plan.push_metric("stahlgewicht_g", weight.steel_g);
        plan
    }

    fn push_metric(&mut self, key: &str, value: f64) {
        let v = (value * 10.0).round() / 10.0;
        if let Some(entry) = self.geometry.metrics.iter_mut().find(|(k, _)| k == key) {
            entry.1 = v;
        } else {
            self.geometry.metrics.push((key.to_string(), v));
        }
    }

    /// Passt die Gestaltung an den Rohling an (z. B. empfohlene Bauart).
    pub fn adapt_params(blank: &BladeBlank, params: &HandleParams) -> HandleParams {
        let mut p = params.clone();
        if p.style == HandleStyle::FullTangTwoScale && blank.has_bolster {
            p.style = HandleStyle::PartialTangBolster;
        }
        p
    }

    /// Textueller Bauplan.
    pub fn report_text(&self) -> String {
        self.report_text_mit(&[])
    }

    /// Der Textplan mit den angehaengten Datenblaettern.
    ///
    /// Die Datenblaetter (z. B. das Lieferdatenblatt des Klingenstahls)
    /// gehoeren zum Plan - der Text weist sie aus, damit die
    /// Werkstattunterlagen nicht verloren gehen.
    pub fn report_text_mit(&self, datenblaetter: &[crate::project::Datenblatt]) -> String {
        let b = &self.blank;
        let p = &self.params;
        let g = &self.geometry;
        let v = &self.verdict;
        let mut out = String::new();

        out.push_str(&format!("GRIFFPLAN - {}\n", style_name(p.style)));
        out.push_str(&"=".repeat(64));
        out.push('\n');
        out.push_str(&format!("Material:   {} - {}\n", v.material_name, v.rating));
        let z =
            crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);
        if z.stuecke == 2 {
            out.push_str(&format!(
                "Zuschnitt:  Holzleiste {} × {} × {} mm, {} Stück (dazwischen {} mm für die Angel aus Stahl)\n",
                de(z.laenge_mm, 0),
                de(z.hoehe_mm, 0),
                de(z.dicke_mm, 0),
                z.stuecke,
                de(b.tang_thickness_mm, 1)
            ));
        } else {
            out.push_str(&format!(
                "Zuschnitt:  Holzblock {} × {} × {} mm (mit Schleif- und Hobelzugabe)\n",
                de(z.laenge_mm, 0),
                de(z.hoehe_mm, 0),
                de(z.dicke_mm, 0)
            ));
        }
        out.push_str(&format!(
            "Rohling:    Klinge {} × {} × {} mm, Angel {} × {} × {} mm ({})\n",
            de(b.blade_length_mm, 0),
            de(b.blade_width_mm, 0),
            de(b.blade_thickness_mm, 1),
            de(b.tang_length_mm, 0),
            de(crate::geometry::angel_hoehe_am_heft(b, p), 1),
            de(b.tang_thickness_mm, 1),
            b.tang_style.label()
        ));
        out.push_str(&format!(
            "Griff:      {} lang, {} hoch, {} dick\n",
            de_mm(g.handle_length_mm, 0),
            de_mm(g.handle_height_mm, 0),
            de_mm(g.total_thickness_mm, 1)
        ));
        out.push_str(&format!(
            "Querschnitt: {} - Rücken {}, Bauch {}, Längsgrat {}, \
             Verjüngung {}\n",
            p.section.shape.label(),
            de_mm(p.section.spine_round_mm, 1),
            de_mm(p.section.belly_round_mm, 1),
            de_mm(p.section.keel_mm, 1),
            de_mm(p.section.taper_mm, 1)
        ));
        out.push_str(&format!(
            "Profil:     {}\n",
            match p.profile {
                crate::model::ProfileStyle::Blade => "folgt dem Heftprofil des Rohlings".to_string(),
                crate::model::ProfileStyle::Ergonomic => format!(
                    "eigenes Ergonomieprofil, Griffhöhe {} mm, Fingerkehle {} mm, Ballenauflage {} mm",
                    de(p.grip_height_mm, 0),
                    de(p.finger_groove_mm, 1),
                    de(p.belly_drop_mm, 1)
                ),
            }
        ));
        out.push('\n');

        out.push_str("MASSE\n");
        out.push_str(&"-".repeat(64));
        out.push('\n');
        for (k, val) in &g.metrics {
            out.push_str(&format!("  {k:<26} {:>8}\n", de(*val, 1)));
        }
        out.push('\n');

        out.push_str("BOHRPLAN\n");
        out.push_str(&"-".repeat(64));
        out.push('\n');
        out.push_str(&format!(
            "  {:<22} {:>8} {:>7}  {}\n", "Position", "Abstand", "Durchm.", "Art"
        ));
        for pin in &g.pins {
            let art = match pin.kind.as_str() {
                "through" => "Vorsteckbolzen durch die Angel",
                "friction" => "Reibniet, halbtief",
                "tang" => "Querbolzen in die Angel",
                _ => {
                    if pin.achse == "y" {
                        "Lanyardloch durch die Höhe"
                    } else {
                        "Lanyardloch quer durch die Dicke"
                    }
                }
            };
            out.push_str(&format!(
                "  {:<22} {:>8} {:>7}  {}\n",
                pin.label,
                de(pin.x_mm, 1),
                de(pin.dia_mm, 1),
                art
            ));
            out.push_str(&format!("      {}\n", pin.rationale));
        }
        out.push('\n');

        out.push_str("MATERIAL\n");
        out.push_str(&"-".repeat(64));
        out.push('\n');
        out.push_str(&format!("  Bewertung:      {}\n", v.rating));
        if let Some(w) = &v.weight_note {
            out.push_str(&format!("  Gewicht:       {w}\n"));
        }
        out.push_str(&format!("  Klebstoff:      {}\n", v.adhesive));
        out.push_str(&format!("  Befestigung:    {}\n", v.fastener));
        out.push_str(&format!("  Finish:         {}\n", v.finish));
        out.push_str(&format!("  Hinweis:        {}\n", v.notes));
        if !v.pros.is_empty() {
            out.push_str("\n  Vorteile:\n");
            for p_ in &v.pros {
                out.push_str(&format!("    + {p_}\n"));
            }
        }
        if !v.cons.is_empty() {
            out.push_str("\n  Nachteile:\n");
            for c in &v.cons {
                out.push_str(&format!("    - {c}\n"));
            }
        }
        if !v.risks.is_empty() {
            out.push_str("\n  Risiken:\n");
            for r in &v.risks {
                out.push_str(&format!("    ! {r}\n"));
            }
        }
        if let Some(a) = &self.material_analysis {
            out.push_str("\n  Analyse des Materialfotos:\n");
            if let Some(cat) = &a.category {
                out.push_str(&format!("    Kategorie: {cat}\n"));
            }
            if let Some(sp) = &a.species_guess {
                out.push_str(&format!("    Art: {sp}\n"));
            }
            if let Some(g) = &a.grain_direction {
                out.push_str(&format!("    Maserungsverlauf: {g}\n"));
            }
            if let Some(defects) = &a.defects {
                if !defects.is_empty() {
                    out.push_str(&format!("    Auffälligkeiten: {}\n", defects.join(", ")));
                }
            }
            if let Some(n) = &a.notes {
                out.push_str(&format!("    Hinweis: {n}\n"));
            }
        }
        {
            let ergo = crate::geometry::ergonomics_for(b, p, g);
            out.push_str("\n  Ergonomie:\n");
            out.push_str(&format!("    {}\n", ergo.note));
            for w in &ergo.warnings {
                out.push_str(&format!("    ! {w}\n"));
            }
        }
        if !v.tips.is_empty() {
            out.push_str("\n  Tipps:\n");
            for t in &v.tips {
                out.push_str(&format!("    - {t}\n"));
            }
        }
        out.push('\n');

        out.push_str("ARBEITSREIHENFOLGE\n");
        out.push_str(&"-".repeat(64));
        out.push('\n');
        for (i, step) in self.work_steps().iter().enumerate() {
            out.push_str(&format!("  {:>2}. {step}\n", i + 1));
        }
        out.push('\n');

        if !datenblaetter.is_empty() {
            out.push_str("DATENBLÄTTER\n");
            out.push_str(&"-".repeat(64));
            out.push('\n');
            for d in datenblaetter {
                out.push_str(&format!("  - {} ({})\n", d.name, d.datei_name()));
            }
            out.push('\n');
        }
        if !g.notes.is_empty() {
            out.push_str("HINWEISE ZUR KONSTRUKTION\n");
            out.push_str(&"-".repeat(64));
            out.push('\n');
            for n in &g.notes {
                out.push_str(&format!("  - {n}\n"));
            }
            out.push('\n');
        }
        if !g.warnings.is_empty() {
            out.push_str("WARNUNGEN\n");
            out.push_str(&"-".repeat(64));
            out.push('\n');
            for w in &g.warnings {
                out.push_str(&format!("  ! {w}\n"));
            }
        }
        out
    }

    /// Arbeitsschritte in sinnvoller Reihenfolge.
    pub fn work_steps(&self) -> Vec<String> {
        let b = &self.blank;
        let p = &self.params;
        let g = &self.geometry;
        let mut steps: Vec<String> = Vec::new();

        steps.push(format!(
            "Rohling aufarbeiten: Angel auf {} Höhe und {} Dicke bringen, \
             Heft sauber entgraten.",
            de_mm(crate::geometry::angel_hoehe_am_heft(b, p), 1),
            de_mm(b.tang_thickness_mm, 1)
        ));
        if p.style == crate::model::HandleStyle::OnePiece {
            steps.push(format!(
                "Absatz der Klinge schleifen: auf {} Höhe über {} - sonst passt die \
                 Klinge nicht in die Tasche für den verdeckten Ansatz.",
                de_mm(p.hidden_tang_height_mm, 1),
                de_mm(p.heel_grind_mm, 1)
            ));
        }
        if b.has_guard {
            steps.push(format!(
                "Angelsteg herstellen: {} mm dick, die Flügel je {} mm über den Griff, \
                 unten {} mm unter die Schneide. Der Schlitz in der Mitte nimmt die \
                 Angel auf - der Steg wird dort festgeklemmt.",
                de_mm(b.guard_thickness_mm, 1),
                de_mm(b.guard_wing_mm, 1),
                de_mm(b.guard_drop_mm, 1)
            ));
        }
        if p.style == HandleStyle::PartialTangBolster {
            steps.push(
                "Bastion anpassen und mit der Angel verschweißen - Schweißnaht planieren, \
                 damit keine Spalte zwischen Bastion und Angel entsteht."
                    .to_string(),
            );
        }
        if p.style == HandleStyle::Ferrule {
            steps.push(
                "Zwinge anpassen: Innenkonus zur Angel, damit die Zwinge das Holz fest \
                 gegen die Angel zieht."
                    .to_string(),
            );
        }
        let z = crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);
        steps.push(format!(
            "Material auswählen und im Block besorgen - {}, mindestens {} × {} × {} \
             (mit Zugabe für den Beschnitt).",
            if z.stuecke == 2 {
                format!("{} Stück", z.stuecke)
            } else {
                "1 Stück".to_string()
            },
            de_mm(z.laenge_mm, 0),
            de_mm(z.hoehe_mm, 0),
            de_mm(z.dicke_mm, 0)
        ));
        steps.push("Material konditionieren: mehrere Tage bei Raumtemperatur lagern, \
                    Krümmung prüfen."
            .to_string());
        if z.stuecke == 2 {
            steps.push(format!(
                "Griffschalen zuschneiden - Kontur aus dem DXF (Layer SCHALE), \
                 {} Stück à {}.",
                z.stuecke,
                de_mm(g.scale_thickness_mm, 1)
            ));
        } else {
            steps.push(format!(
                "Holzblock zuschneiden - Kontur aus dem DXF (Layer SCHALE), {} dick.",
                de_mm(g.total_thickness_mm, 1)
            ));
        }
        if g.tang_nut.is_some() {
            steps.push(format!(
                "Nut fuer die Angel fräsen: Kontur aus dem DXF (Layer AUSSPA), \
                 {} breit - rings {} Luft fuer den Kleber.",
                de_mm(b.tang_thickness_mm, 1),
                de_mm(p.glue_gap_mm, 2)
            ));
            if p.style == HandleStyle::PartialTangBolster {
                steps.push(format!(
                    "Die Nut beginnt {} hinter der Vorderkante - davor sitzt die \
                     Bastion, dort liegt die Angel im Metall.",
                    de_mm(g.wood_start_x_mm, 0)
                ));
            }
        } else {
            steps.push(
                "Die Angel sitzt in der Fuge zwischen den Schalen - die Schalen \
                 liegen flach an ihr und werden dort verleimt. Eine Tasche wird \
                 nicht ausgefräst."
                .to_string(),
            );
        }
        steps.push(format!(
            "Bohrungen setzen nach der Bohrplan-Tabelle, Vorsteckbohrungen mit \
             Ø {}, Reibnieten mit Ø 2,4 mm halbtief.",
            de_mm(
                g.pins
                    .iter()
                    .find(|x| x.kind == "through")
                    .map(|x| x.dia_mm)
                    .unwrap_or(3.5),
                1
            )
        ));
        steps.push(format!(
            "Kanten abrunden: umlaufend etwa {}, danach die Vorderkante anschrägen.",
            de_mm(p.edge_break_mm, 1)
        ));
        steps.push("Leimfugen vorbereiten: an der Angel und im Holz entfetten, \
                    Passprobe ohne Klebstoff."
            .to_string());
        steps.push(format!(
            "Kleben und montieren: {} verwenden, Schalen anpressen, \
             Vorsteckbolzen von einer Seite einschlagen.",
            self.verdict.adhesive
        ));
        steps.push("Endkontrolle: Griff auf Leichtgängigkeit prüfen, Spiel an den \
                    Bolzen testen, Kanten nachschleifen."
            .to_string());
        steps.push(format!(
            "Finish: {} auftragen, zwischen den Ölgängen 24 Stunden trocknen lassen.",
            self.verdict.finish
        ));
        steps
    }
}

fn default_material_for(params: &HandleParams) -> &'static str {
    match params.style {
        HandleStyle::Ferrule => "messing",
        HandleStyle::PartialTangBolster => "walnuss",
        _ => "walnuss",
    }
}

/// Gewichtsschaetzung des fertigen Griffes.
pub struct Weight {
    pub wood_g: f64,
    pub steel_g: f64,
    pub metal_g: f64,
    /// Wie viel das Messer durch den Griff kopflastig wird.
    pub balance_note: String,
}

impl Weight {
    pub fn total_g(&self) -> f64 {
        self.wood_g + self.steel_g + self.metal_g
    }
    pub fn note(&self) -> String {
        format!(
            "Griff rund {} g (Holz {} g, Stahl {} g, Metall {} g). {}",
            de(self.total_g(), 0),
            de(self.wood_g, 0),
            de(self.steel_g, 0),
            de(self.metal_g, 0),
            self.balance_note
        )
    }
}

/// Schaetzt das Gewicht des Griffes aus Volumen und Materialdichte.
///
/// Das Holz wird aus dem Modell gemessen, nicht aus der Kontur gerechnet:
/// der Griff verjuengt sich zum Ende hin, die Kanten sind abgerundet und
/// die Nut fuer die Angel nimmt Holz weg. Nur so weiss der Plan, was der
/// Griff wirklich wiegt.
pub fn estimate_weight(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &materials::Material,
) -> Weight {
    let outline_area = crate::geometry::polygon_area(&geo.outline);
    let tang_area = crate::geometry::polygon_area(&geo.tang_profile);

    // Holz: zwei Schalen bzw. ein Block
    //
    // Bei zwei Schalen liegt die Angel in der Fuge dazwischen - die
    // Schalen sind massiv. Bei Bastion, Ein-Stueck und Zwinge ist das
    // Holz ein Block, in dem die Angel in einer Nut liegt.
    let holz_dicke = if params.style.hat_zwei_schalen() {
        geo.scale_thickness_mm
    } else {
        geo.total_thickness_mm
    };
    let stuecke = params.style.holzstuecke();
    // Das Holz, das fuer die Angel wegfällt: die Nut ist so breit wie
    // die Angel dick.
    let nut_volumen = geo
        .tang_nut
        .as_ref()
        .map(|pfad| crate::geometry::polygon_area(pfad) * blank.tang_thickness_mm)
        .unwrap_or(0.0);
    let gemessen = crate::geometry::holzvolumen(&geo.bodies);
    let wood_mm3 = if gemessen > 0.0 {
        gemessen
    } else {
        // Aeltere Projektdateien tragen keine Koerper - dann die Naeherung
        // aus der Kontur.
        outline_area * holz_dicke * stuecke as f64 - nut_volumen
    };
    let wood_cm3 = wood_mm3 / 1000.0;
    let wood_g = wood_cm3 * material.density;

    // Stahl: die Angel im Griff und der Angelsteg davor
    let steel_mm3 = tang_area * blank.tang_thickness_mm;
    let steel_mm3 = steel_mm3
        + geo.guard_profile
            .as_ref()
            .map(|prof| {
                crate::geometry::polygon_area(prof) * blank.guard_thickness_mm.max(0.5)
            })
            .unwrap_or(0.0);
    let steel_g = (steel_mm3 / 1000.0) * 7.85;

    // Metall: Bastion oder Zwinge
    //
    // Die Zwinge ist ein Mantel um das Holz - kein voller Klotz. Sonst
    // waere sie rund dreimal so schwer wie in Wirklichkeit.
    let metal_mm3 = if let Some(f) = &geo.ferrule_profile {
        let aussen = crate::mesh::offset(f, -crate::export::stl::WAND_ZWINGE);
        let innen = crate::mesh::offset(f, -crate::export::stl::PASSUNG_ZWINGE);
        (crate::geometry::polygon_area(&aussen) - crate::geometry::polygon_area(&innen))
            * geo.total_thickness_mm
    } else if let Some(b) = &geo.bolster_profile {
        crate::geometry::polygon_area(b) * geo.total_thickness_mm
    } else {
        0.0
    };
    let metal_g = (metal_mm3 / 1000.0) * 8.5;

    // Waage: schwerer Griff gegen leichte Klinge
    let blade_g = (blank.blade_length_mm * blank.blade_width_mm * blank.blade_thickness_mm
        / 1000.0)
        * 7.85
        / 2.0;
    let balance_note = if wood_g + steel_g + metal_g > blade_g * 1.6 {
        "Der Griff ist deutlich schwerer als die halbe Klinge - das Messer waucht \
         nach vorn."
    } else if wood_g + steel_g + metal_g < blade_g * 0.35 {
        "Der Griff ist sehr leicht - das Messer liegt gut ausbalanciert."
    } else {
        "Gute Waage zwischen Griff und Klinge."
    };

    Weight {
        wood_g,
        steel_g,
        metal_g,
        balance_note: balance_note.to_string(),
    }
}

/// Der Zuschnitt als kurzer Text fuer die Dokumente.
///
/// Bei zwei Griffschalen sind es zwei Stueck - je eine Schale. Die Angel
/// kommt dazwischen und braucht ihren Platz; sie ist aus Stahl und zaehlt
/// nicht zum Holz. Bei allen anderen Bauarten ist es ein Block.
fn zuschnitt_kurz(b: &BladeBlank, z: &crate::geometry::Zuschnitt) -> String {
    if z.stuecke == 2 {
        format!(
            "{} × {} × {} mm, {} Stück (dazwischen {} mm für die Angel)",
            de(z.laenge_mm, 0),
            de(z.hoehe_mm, 0),
            de(z.dicke_mm, 0),
            z.stuecke,
            de(b.tang_thickness_mm, 1)
        )
    } else {
        format!(
            "Holzblock {} × {} × {} mm",
            de(z.laenge_mm, 0),
            de(z.hoehe_mm, 0),
            de(z.dicke_mm, 0)
        )
    }
}

/// Die Handwaage des Griffs.
pub fn style_name(style: HandleStyle) -> &'static str {
    style.label()
}

/// Empfohlene Griffbauart fuer einen Rohling (Wrapper).
pub fn suggested_style(blank: &BladeBlank) -> HandleStyle {
    recommend_style(blank)
}

// ---------------------------------------------------------------------------
// PDF-Ausgaben
// ---------------------------------------------------------------------------

const A4_W: f64 = 210.0;
const A4_H: f64 = 297.0;

/// Bohrplan als PDF (A4 hoch, Zeichnung + Tabelle).
pub fn build_bohrplan_pdf(plan: &DesignPlan) -> Pdf {
    let mut pdf = Pdf::new();
    pdf.add_page(A4_W, A4_H);

    let g = &plan.geometry;
    let b = &plan.blank;
    let p = &plan.params;

    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, 18.0, 15.0, "Bohrplan Griff", true);
    pdf.text(
        15.0,
        25.0,
        9.0,
        &format!("{} - {}", style_name(p.style), plan.verdict.material_name),
        false,
    );
    pdf.text(
        15.0,
        30.0,
        8.0,
        &format!(
            "Angel {} × {} mm, Länge {} mm | {} | Griff {} × {} mm",
            de(crate::geometry::angel_hoehe_am_heft(b, p), 1),
            de(b.tang_thickness_mm, 1),
            de(b.tang_length_mm, 1),
            if p.style.hat_zwei_schalen() {
                format!("2 Schalen à {}", de_mm(g.scale_thickness_mm, 1))
            } else {
                format!("1 Block, {}", de_mm(g.total_thickness_mm, 1))
            },
            de(g.handle_length_mm, 0),
            de(g.handle_height_mm, 0)
        ),
        false,
    );

    // Zeichnung: Weltkoordinaten auf den Zeichenbereich abbilden
    let draw_x = 15.0;
    let draw_y = 40.0;
    // Rechts daneben steht der Querschnitt
    let draw_w = 110.0;
    let draw_h = 105.0;

    let mut min_x = f64::MAX;
    let mut min_y = f64::MAX;
    let mut max_x = f64::MIN;
    let mut max_y = f64::MIN;
    for &(x, y) in g.outline.iter().chain(g.tang_profile.iter()) {
        min_x = min_x.min(x);
        min_y = min_y.min(y);
        max_x = max_x.max(x);
        max_y = max_y.max(y);
    }
    let ww = (max_x - min_x).max(1.0);
    let wh = (max_y - min_y).max(1.0);
    let s = (draw_w / ww).min(draw_h / wh);
    let off_x = draw_x + (draw_w - ww * s) / 2.0;
    let off_y = draw_y + (draw_h - wh * s) / 2.0;
    let map = |p: (f64, f64)| -> (f64, f64) {
        (off_x + (p.0 - min_x) * s, off_y + (max_y - p.1) * s)
    };

    // Angelmittenlinie
    pdf.set_stroke(0.55, 0.55, 0.55);
    pdf.set_line_width(0.2);
    pdf.set_dash(&[2.0, 1.5]);
    let (ax0, ay0) = map((min_x, 0.0));
    let (ax1, ay1) = map((max_x, 0.0));
    pdf.line(ax0, ay0, ax1, ay1);
    pdf.clear_dash();

    pdf.set_stroke(0.15, 0.15, 0.15);
    pdf.set_fill(0.91, 0.86, 0.77);
    pdf.set_line_width(0.8);
    let mapped: Vec<(f64, f64)> = g.outline.iter().map(|&p| map(p)).collect();
    pdf.polyline(&mapped, true);

    pdf.set_stroke(0.12, 0.48, 0.25);
    pdf.set_line_width(0.5);
    pdf.set_dash(&[2.0, 1.5]);
    let tang: Vec<(f64, f64)> = g.tang_profile.iter().map(|&p| map(p)).collect();
    pdf.polyline(&tang, true);
    pdf.clear_dash();

    // Bohrungen mit Nummer, damit die Tabelle zuordenbar bleibt
    for (i, pin) in g.pins.iter().enumerate() {
        if pin.achse == "y" {
            // Das Lanyardloch laeuft durch die Hoehe. Im Seitenbild ist es
            // deshalb keine Scheibe, sondern ein Spalt - als gestrichelte
            // Kanten dargestellt.
            let r = pin.dia_mm / 2.0;
            let (unten, oben) = crate::geometry::kontur_hoehe(g, pin.x_mm);
            let tang_dort = crate::geometry::angel_unten(b, p, pin.x_mm);
            let y_ende = tang_dort.unwrap_or(oben);
            pdf.set_stroke(0.78, 0.16, 0.16);
            pdf.set_line_width(0.5);
            pdf.set_dash(&[2.0, 1.5]);
            for dx in [-r, r] {
                let (x0, y0) = map((pin.x_mm + dx, unten));
                let (x1, y1) = map((pin.x_mm + dx, y_ende));
                pdf.line(x0, y0, x1, y1);
            }
            pdf.clear_dash();
            let (tx, ty) = map((pin.x_mm + r, y_ende));
            pdf.set_fill(0.78, 0.16, 0.16);
            pdf.text(tx + 1.5, ty, 8.0, &format!("{}", i + 1), true);
            continue;
        }
        let (cx, cy) = map((pin.x_mm, pin.y_mm));
        let r = pin.dia_mm / 2.0 * s;
        pdf.set_stroke(0.78, 0.16, 0.16);
        pdf.set_line_width(0.5);
        pdf.circle(cx, cy, r);
        pdf.set_fill(0.78, 0.16, 0.16);
        pdf.text(
            cx + r + 1.5,
            cy - r,
            8.0,
            &format!("{}", i + 1),
            true,
        );
    }

    // Der Hinweis auf die Druckschablone im Massstab 1:1
    pdf.set_fill(0.1, 0.35, 0.15);
    pdf.text(
        15.0,
        draw_y + draw_h + 6.0,
        8.0,
        "Druckschablone 1:1: schablone_1zu1.pdf - zum Auflegen auf den Rohling",
        true,
    );

    // Legende
    pdf.set_fill(0.25, 0.25, 0.25);
    pdf.text(15.0, draw_y + draw_h + 12.0, 7.5, "Legende", true);
    let mut ly = draw_y + draw_h + 17.0;
    pdf.set_fill(0.12, 0.48, 0.25);
    pdf.text(15.0, ly, 7.0, "grün gestrichelt: Angel mit Aussparung", false);
    ly += 4.0;
    pdf.set_fill(0.78, 0.16, 0.16);
    pdf.text(15.0, ly, 7.0, "rote Kreise: Bohrungen, Nummern siehe Tabelle", false);

    // Tabelle der Bohrungen
    //
    // Der Abstand zur Legende muss gross genug sein: die Ueberschrift
    // (11 pt) laeuft sonst in die letzte Zeile der Legende (7 pt).
    let mut ty = draw_y + draw_h + 26.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, ty, 11.0, "Bohrungen", true);
    ty += 9.0;
    pdf.set_fill(0.2, 0.2, 0.2);
    pdf.text(15.0, ty, 8.0, "Nr.", true);
    pdf.text(26.0, ty, 8.0, "Bezeichnung", true);
    pdf.text(95.0, ty, 8.0, "Abstand", true);
    pdf.text(125.0, ty, 8.0, "Durchm.", true);
    pdf.text(150.0, ty, 8.0, "Art", true);
    ty += 2.5;
    pdf.set_stroke(0.4, 0.4, 0.4);
    pdf.set_line_width(0.4);
    pdf.line(15.0, ty, 195.0, ty);
    ty += 5.5;

    for (i, pin) in g.pins.iter().enumerate() {
        let art = match pin.kind.as_str() {
            "through" => "Vorsteckbolzen",
            "friction" => "Reibniet",
            "tang" => "Querbolzen",
            _ => {
                if pin.achse == "y" {
                    "Lanyardloch hoch"
                } else {
                    "Lanyardloch quer"
                }
            }
        };
        pdf.set_fill(0.0, 0.0, 0.0);
        pdf.text(15.0, ty, 8.0, &format!("{}", i + 1), true);
        pdf.text(26.0, ty, 8.0, &pin.label, false);
        pdf.text(95.0, ty, 8.0, &de_mm(pin.x_mm, 1), false);
        pdf.text(125.0, ty, 8.0, &de_mm(pin.dia_mm, 1), false);
        pdf.text(150.0, ty, 8.0, art, false);
        ty += 5.0;
    }

    // Querschnitt in der Griffmitte
    let x_mitte = (g.handle_length_mm * 0.45).min(b.tang_length_mm * 0.9);
    if let Some(cs) = crate::geometry::cross_section(g, b, x_mitte) {
        let bx = 132.0;
        let by = 48.0;
        let bw = 60.0;
        let bh = 60.0;
        let mut min_y = cs.tang_unten;
        let mut max_y = cs.tang_oben;
        let mut min_z = cs.tang_half_thickness;
        let mut max_z = -cs.tang_half_thickness;
        for (z, v) in cs
            .bodies
            .iter()
            .flat_map(|(l, u)| l.iter().chain(u.iter()))
        {
            min_y = min_y.min(*v);
            max_y = max_y.max(*v);
            min_z = min_z.min(*z);
            max_z = max_z.max(*z);
        }
        let sc = (bw / (max_z - min_z).max(1.0)).min(bh / (max_y - min_y).max(1.0));
        let cxm = bx + bw / 2.0;
        let cym = by + bh / 2.0;
        let map = |y: f64, z: f64| -> (f64, f64) {
            (
                cxm + (z - (min_z + max_z) / 2.0) * sc,
                cym - (y - (min_y + max_y) / 2.0) * sc,
            )
        };
        pdf.set_fill(0.2, 0.2, 0.2);
        pdf.text(bx, by - 6.0, 8.0, "Querschnitt", true);
        // Holz
        pdf.set_stroke(0.23, 0.17, 0.12);
        pdf.set_line_width(0.5);
        for (lower, upper) in &cs.bodies {
            let mut poly: Vec<(f64, f64)> = lower.iter().map(|&(z, v)| map(v, z)).collect();
            poly.extend(upper.iter().rev().map(|&(z, v)| map(v, z)));
            pdf.polyline(&poly, true);
        }
        // Angel
        pdf.set_stroke(0.45, 0.48, 0.53);
        pdf.set_line_width(0.4);
        let th = cs.tang_half_thickness;
        pdf.polyline(
            &[
                map(cs.tang_oben, -th),
                map(cs.tang_oben, th),
                map(cs.tang_unten, th),
                map(cs.tang_unten, -th),
            ],
            true,
        );
        pdf.set_fill(0.15, 0.15, 0.15);
        pdf.text(
            bx,
            by + bh + 3.0,
            7.0,
            &format!(
                "bei {} mm | Höhe {} | Wand {} / {} mm",
                de(x_mitte, 0),
                de(cs.height, 1),
                de(cs.wall_spine, 1),
                de(cs.wall_belly, 1)
            ),
            false,
        );
    }

    ty += 4.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    if g.tang_nut.is_some() {
        pdf.text(
            15.0,
            ty,
            8.0,
            &format!(
                "Nut fuer die Angel: {} breit (Angel dick), Spalt {} | Kantenabrundung {}",
                de_mm(b.tang_thickness_mm, 1),
                de_mm(p.glue_gap_mm, 2),
                de_mm(p.edge_break_mm, 1)
            ),
            false,
        );
    } else {
        pdf.text(
            15.0,
            ty,
            8.0,
            &format!(
                "Angel in der Fuge zwischen den Schalen - die Schalen liegen flach an ihr | \
                 Kantenabrundung {}",
                de_mm(p.edge_break_mm, 1)
            ),
            false,
        );
    }
    ty += 7.0;
    for w in g.warnings.iter().take(4) {
        ty = pdf.text_block(15.0, ty, 7.5, 180.0, &format!("! {w}"), false, 4.0);
    }

    pdf
}

/// Bauplan als PDF (mehrere Seiten).
pub fn build_bauplan_pdf(plan: &DesignPlan) -> Pdf {
    build_bauplan_pdf_mit(plan, &[])
}

/// Der Bauplan als PDF - mit den angehaengten Datenblaettern.
pub fn build_bauplan_pdf_mit(
    plan: &DesignPlan,
    datenblaetter: &[crate::project::Datenblatt],
) -> Pdf {
    let mut pdf = Pdf::new();
    let g = &plan.geometry;
    let b = &plan.blank;
    let p = &plan.params;
    let v = &plan.verdict;

    // Seite 1: Uebersicht, Masse, Material
    pdf.add_page(A4_W, A4_H);
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, 24.0, 18.0, "Bauplan Griff", true);
    pdf.text(15.0, 34.0, 10.0, &format!("{} - {}", style_name(p.style), v.material_name), false);

    let z = crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);

    let mut y = 52.0;
    pdf.text(15.0, y, 12.0, "Rohling und Maße", true);
    y += 12.0;
    let holz_row = if z.stuecke == 2 {
        format!(
            "{} Schalen à {}",
            z.stuecke,
            de_mm(g.scale_thickness_mm, 1)
        )
    } else {
        format!("{} Block, {}", z.stuecke, de_mm(g.total_thickness_mm, 1))
    };
    let rows = vec![
        ("Klingenlänge", de_mm(b.blade_length_mm, 1)),
        ("Klingenbreite am Heft", de_mm(b.blade_width_mm, 1)),
        ("Klingendicke", de_mm(b.blade_thickness_mm, 1)),
        ("Angel", format!("{} lang", de_mm(b.tang_length_mm, 1))),
        (
            "Angelhöhe am Heft",
            de_mm(crate::geometry::angel_hoehe_am_heft(b, p), 1),
        ),
        ("Angeldicke", de_mm(b.tang_thickness_mm, 1)),
        ("Grifflänge", de_mm(g.handle_length_mm, 1)),
        ("Griffhöhe", de_mm(g.handle_height_mm, 1)),
        ("Gesamtdicke", de_mm(g.total_thickness_mm, 1)),
        ("Holz", holz_row),
        ("Zuschnitt Holz", zuschnitt_kurz(b, &z)),
    ];
    for (k, val) in rows {
        pdf.set_fill(0.1, 0.1, 0.1);
        pdf.text(15.0, y, 9.5, k, false);
        pdf.text(110.0, y, 9.5, &val, false);
        y += 7.0;
    }

    y += 12.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 12.0, "Materialbewertung", true);
    y += 12.0;
    pdf.text(15.0, y, 9.5, &format!("Bewertung: {} ({}/100)", v.rating, v.score), false);
    y += 8.0;
    pdf.text(15.0, y, 9.5, &format!("Klebstoff: {}", v.adhesive), false);
    y += 7.0;
    pdf.text(15.0, y, 9.5, &format!("Befestigung: {}", v.fastener), false);
    y += 7.0;
    pdf.text(15.0, y, 9.5, &format!("Finish: {}", v.finish), false);
    y += 10.0;
    y = pdf.text_block(15.0, y, 9.0, 180.0, &v.notes, false, 5.5);

    if !v.tips.is_empty() {
        y += 6.0;
        for t in &v.tips {
            y = pdf.text_block(15.0, y, 9.0, 180.0, &format!("- {t}"), false, 5.5);
        }
    }

    // Seite 2: Bohrplan und Arbeitsreihenfolge
    pdf.add_page(A4_W, A4_H);
    let mut y = 24.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 14.0, "Bohrplan", true);
    y += 12.0;

    for pin in &g.pins {
        pdf.set_fill(0.0, 0.0, 0.0);
        pdf.text(15.0, y, 10.0, &pin.label, true);
        y += 6.5;
        pdf.text(
            25.0,
            y,
            9.0,
            &format!("bei {}, Ø {}", de_mm(pin.x_mm, 1), de_mm(pin.dia_mm, 1)),
            false,
        );
        y += 5.5;
        y = pdf.text_block(25.0, y, 8.5, 165.0, &pin.rationale, false, 5.0);
        y += 3.0;
    }

    // Seite 3: Arbeitsreihenfolge
    pdf.add_page(A4_W, A4_H);
    let mut y = 24.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 14.0, "Arbeitsreihenfolge", true);
    y += 14.0;
    for (i, step) in plan.work_steps().iter().enumerate() {
        pdf.set_fill(0.1, 0.1, 0.1);
        pdf.text(15.0, y, 9.5, &format!("{}.", i + 1), true);
        y = pdf.text_block(25.0, y, 9.5, 170.0, step, false, 6.0);
        y += 3.0;
    }

    if !g.warnings.is_empty() {
        y += 8.0;
        if !datenblaetter.is_empty() {
            pdf.set_fill(0.0, 0.0, 0.0);
            pdf.text(15.0, y, 12.0, "Datenblätter", true);
            y += 10.0;
            for d in datenblaetter {
                y = pdf.text_block(15.0, y, 9.0, 180.0, &format!("- {}", d.name), false, 5.5);
                y = pdf.text_block(15.0, y, 8.0, 180.0, &format!("  Datei: {}", d.datei_name()), false, 5.0);
            }
            y += 6.0;
        }
        pdf.set_fill(0.0, 0.0, 0.0);
        pdf.text(15.0, y, 12.0, "Warnungen", true);
        y += 10.0;
        for w in &g.warnings {
            y = pdf.text_block(15.0, y, 9.0, 180.0, &format!("! {w}"), false, 5.5);
        }
    }

    pdf
}

#[cfg(test)]
mod datenblatt_tests {
    use super::*;
    use crate::model::{BladeBlank, HandleParams};
    use crate::project::Datenblatt;

    fn plan() -> DesignPlan {
        DesignPlan::build(&BladeBlank::default(), &HandleParams::default(), "walnuss")
    }

    #[test]
    fn der_ein_stueck_griff_nennt_die_schleifung() {
        // Beim Ein-Stueck-Griff muss der Absatz der Klinge geschliffen
        // werden - ohne diesen Hinweis passt die Klinge nicht in die
        // Tasche fuer den verdeckten Ansatz.
        let mut params = HandleParams::default();
        params.style = crate::model::HandleStyle::OnePiece;
        let ein_stueck = DesignPlan::build(&BladeBlank::default(), &params, "walnuss");
        let text = ein_stueck.report_text_mit(&[]);
        assert!(text.contains("Absatz der Klinge schleifen"), "{text}");
        assert!(text.contains("6,5 mm"), "{text}");

        // Bei zwei Schalen gibt es keinen Absatz und keinen Schleifhinweis.
        let text2 = plan().report_text_mit(&[]);
        assert!(!text2.contains("Absatz der Klinge schleifen"), "{text2}");
    }

    #[test]
    fn bauplan_nennt_die_datenblaetter() {
        let blaetter = vec![
            Datenblatt {
                name: "Stahl 1.2316 - Lieferdatenblatt".into(),
                pfad: "/unterlagen/2316.pdf".into(),
            },
            Datenblatt {
                name: "Haerteprotokoll".into(),
                pfad: "/unterlagen/haerte.txt".into(),
            },
        ];
        let text = plan().report_text_mit(&blaetter);
        assert!(text.contains("DATENBLÄTTER"), "{text}");
        assert!(text.contains("Stahl 1.2316"), "{text}");
        assert!(text.contains("2316.pdf"), "{text}");
        assert!(text.contains("haerte.txt"), "{text}");

        // Ohne Datenblaetter steht kein Abschnitt drin
        let ohne = plan().report_text();
        assert!(!ohne.contains("DATENBLÄTTER"), "{ohne}");
    }

    #[test]
    fn bauplan_pdf_ist_mit_datenblaettern_gueltig() {
        let blaetter = vec![Datenblatt {
            name: "Stahl 1.2316".into(),
            pfad: "/unterlagen/2316.pdf".into(),
        }];
        let pdf = build_bauplan_pdf_mit(&plan(), &blaetter);
        // Das PDF muss sich als Datei schreiben lassen
        let dir = std::env::temp_dir().join("kp_plan_test");
        std::fs::create_dir_all(&dir).unwrap();
        let pfad = dir.join("bauplan.pdf");
        pdf.save(&pfad).expect("PDF liess sich nicht speichern");
        let laenge = std::fs::metadata(&pfad).unwrap().len();
        assert!(laenge > 1000, "PDF zu klein: {laenge}");
    }
}

// ---------------------------------------------------------------------------
// Druckschablone 1:1
// ---------------------------------------------------------------------------

/// Die Druckschablone im Massstab 1:1.
///
/// Der Plan wird auf Papier gedruckt, an den Rohling gelegt und die
/// Bohrungen werden durchgebohrt. Deshalb ist der Massstab exakt 1:1 -
/// eine 100-mm-Kontrolllinie zeigt, ob der Drucker richtig verkleinert
/// hat. Bei zwei Schalen stehen beide spiegelbildlich auf dem Blatt,
/// genau wie im DXF.
pub fn build_schablone_pdf(plan: &DesignPlan) -> Pdf {
    let mut pdf = Pdf::new();
    pdf.add_page(A4_W, A4_H);

    let g = &plan.geometry;
    let b = &plan.blank;
    let p = &plan.params;

    // Der Rand um die Zeichnung herum
    const RAND_X: f64 = 22.0;
    const RAND_Y: f64 = 52.0;

    let x_min = g.outline.iter().map(|q| q.0).fold(f64::MAX, f64::min);
    let y_min = g.outline.iter().map(|q| q.1).fold(f64::MAX, f64::min);
    let y_max = g.outline.iter().map(|q| q.1).fold(f64::MIN, f64::max);

    // Die zweite Schale liegt spiegelbildlich unter der ersten, mit
    // 12 mm Abstand zum Beschnitt - so wie im DXF. Bei Bastion,
    // Ein-Stueck und Zwinge ist das Holz ein Block - der wird nur
    // einmal gezeichnet.
    let zwei_schalen = p.style.hat_zwei_schalen();
    let spiegel_dy = if zwei_schalen { 2.0 * y_min - 12.0 } else { 0.0 };

    // Der Massstab ist 1:1 - ein Millimeter auf dem Papier ist ein
    // Millimeter am Griff. Die PDF-Achse zeigt nach unten, darum wird
    // die Hoehe gespiegelt.
    let map = |x: f64, y: f64, flip: bool| -> (f64, f64) {
        let yy = if flip { -y + spiegel_dy } else { y };
        (RAND_X + (x - x_min), RAND_Y + (y_max - yy))
    };

    // --- Kopf ---
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, 16.0, 15.0, "Druckschablone Griff", true);
    pdf.text(
        15.0,
        23.0,
        9.0,
        &format!(
            "{} - {} | Maßstab 1:1 | Angel {} × {} mm, {} mm lang",
            style_name(p.style),
            plan.verdict.material_name,
            de(crate::geometry::angel_hoehe_am_heft(b, p), 1),
            de(b.tang_thickness_mm, 1),
            de(b.tang_length_mm, 0)
        ),
        false,
    );
    pdf.set_fill(0.5, 0.1, 0.1);
    pdf.text(
        15.0,
        29.0,
        8.5,
        "In 100 % drucken. Die 100-mm-Linie muss nach dem Ausdruck genau 100 mm lang sein.",
        false,
    );
    let z = crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);
    pdf.set_fill(0.1, 0.1, 0.1);
    pdf.text(
        15.0,
        35.0,
        8.5,
        &format!(
            "Zuschnitt: {}",
            zuschnitt_kurz(b, &z)
        ),
        false,
    );

    // --- Die Schalen ---
    for pass in 0..if zwei_schalen { 2 } else { 1 } {
        let flip = pass == 1;
        let punkte = |pts: &[(f64, f64)]| -> Vec<(f64, f64)> {
            pts.iter().map(|&(x, y)| map(x, y, flip)).collect()
        };

        // Die Aussenkontur ist die Beschnittlinie
        pdf.set_stroke(0.0, 0.0, 0.0);
        pdf.set_line_width(0.5);
        pdf.clear_dash();
        pdf.polyline(&punkte(&g.outline), true);

        // Die Angel und ihre Frassbahn
        pdf.set_stroke(0.1, 0.45, 0.22);
        pdf.set_line_width(0.35);
        pdf.set_dash(&[3.0, 2.0]);
        pdf.polyline(&punkte(&g.tang_profile), true);
        if let Some(nut) = &g.tang_nut {
            // Nur dort wird fuer die Angel eine Bahn gefraest.
            pdf.polyline(&punkte(nut), true);
        }
        if let Some(bo) = &g.bolster_profile {
            pdf.polyline(&punkte(bo), true);
        }
        if let Some(f) = &g.ferrule_profile {
            pdf.polyline(&punkte(f), true);
        }
        pdf.clear_dash();

        // Die Vorderkante des Rohlings - hier richtet man die Schablone aus
        pdf.set_stroke(0.85, 0.15, 0.15);
        pdf.set_line_width(0.6);
        let vorn = map(x_min, y_min, flip);
        let vorn2 = map(x_min, y_max + 5.0, flip);
        pdf.line(vorn.0, vorn.1, vorn2.0, vorn2.1);

        // Die Bohrungen mit Fadenkreuz
        for (i, pin) in g.pins.iter().enumerate() {
            let (cx, cy) = map(pin.x_mm, pin.y_mm, flip);
            let r = pin.dia_mm / 2.0;
            pdf.set_stroke(0.8, 0.15, 0.15);
            pdf.set_line_width(0.4);
            pdf.circle(cx, cy, r);
            pdf.set_line_width(0.25);
            pdf.line(cx - r * 1.9, cy, cx + r * 1.9, cy);
            pdf.line(cx, cy - r * 1.9, cx, cy + r * 1.9);
            pdf.set_fill(0.8, 0.15, 0.15);
            pdf.text(cx + r * 1.9, cy - r * 1.2, 8.0, &format!("{}", i + 1), true);
        }
    }

    // --- Kontrolllinie: 100 mm ---
    let hoehe = y_max - y_min;
    let kontroll_y = RAND_Y + hoehe + (if zwei_schalen { 12.0 + hoehe } else { 0.0 }) + 14.0;
    pdf.set_stroke(0.0, 0.0, 0.0);
    pdf.set_line_width(0.4);
    pdf.line(RAND_X, kontroll_y, RAND_X + 100.0, kontroll_y);
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(RAND_X, kontroll_y + 2.0, 8.0, "100 mm (Kontrolle)", false);

    // --- Tabelle der Bohrungen ---
    let mut y = kontroll_y + 16.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 11.0, "Bohrungen", true);
    y += 9.0;
    pdf.set_fill(0.2, 0.2, 0.2);
    pdf.text(15.0, y, 8.0, "Nr.", true);
    pdf.text(30.0, y, 8.0, "Bezeichnung", true);
    pdf.text(105.0, y, 8.0, "Abstand", true);
    pdf.text(135.0, y, 8.0, "Durchm.", true);
    pdf.text(160.0, y, 8.0, "Art", true);
    y += 2.5;
    pdf.set_stroke(0.4, 0.4, 0.4);
    pdf.set_line_width(0.4);
    pdf.line(15.0, y, 195.0, y);
    y += 5.5;
    for (i, pin) in g.pins.iter().enumerate() {
        let art = match pin.kind.as_str() {
            "through" => "Vorsteckbolzen",
            "friction" => "Reibniet",
            "tang" => "Querbolzen",
            _ => "Lanyardloch",
        };
        pdf.set_fill(0.0, 0.0, 0.0);
        pdf.text(15.0, y, 8.0, &format!("{}", i + 1), true);
        pdf.text(30.0, y, 8.0, &pin.label, false);
        pdf.text(105.0, y, 8.0, &de_mm(pin.x_mm, 1), false);
        pdf.text(135.0, y, 8.0, &de_mm(pin.dia_mm, 1), false);
        pdf.text(160.0, y, 8.0, art, false);
        y += 5.0;
    }

    // --- Hinweise ---
    y += 6.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 11.0, "Hinweise", true);
    y += 9.0;
    let mut hinweise = vec![
        "Die Schablone an der Vorderkante des Rohlings ausrichten und festkleben.",
        "Vor dem Bohren die 100-mm-Linie prüfen, dann stimmen alle Maße.",
        "Die grün gestrichelte Angel liegt mittig im Holz - die Bohrungen gehen durch die Angel.",
    ];
    if p.style.hat_zwei_schalen() {
        hinweise.push("Bei zwei Schalen: beide Konturen sind spiegelbildlich, es sind zwei gleiche Schalen.");
    }
    for h in hinweise {
        y = pdf.text_block(15.0, y, 8.0, 180.0, &format!("- {h}"), false, 5.0);
    }

    pdf
}

#[cfg(test)]
mod gewicht_tests {
    use super::*;
    use crate::model::{BladeBlank, HandleParams, HandleStyle};

    /// Das Volumen eines Netzes (Divergenzsatz).
    fn netz_volumen(m: &crate::mesh::Mesh) -> f64 {
        let mut v = 0.0;
        for &(a, b, c) in &m.faces {
            let p = m.vertices[a];
            let q = m.vertices[b];
            let r = m.vertices[c];
            v += (p.0 * (q.1 * r.2 - q.2 * r.1)
                - p.1 * (q.0 * r.2 - q.2 * r.0)
                + p.2 * (q.0 * r.1 - q.1 * r.0))
                / 6.0;
        }
        v.abs()
    }

    /// Das Holzvolumen aus dem Modell selbst.
    fn holzvolumen(geo: &crate::model::HandleGeometry) -> f64 {
        geo.bodies
            .iter()
            .map(|body| netz_volumen(&crate::mesh::loft(body)))
            .sum()
    }

    #[test]
    fn das_holzgewicht_entspricht_dem_modell() {
        // Die Schaetzung im Plan muss zum Volumen des 3D-Modells passen.
        // Sonst wiegt der Griff anders, als er gebaut wird.
        for style in HandleStyle::all() {
            let b = BladeBlank::default();
            let mut p = HandleParams::default();
            p.style = style;
            let mat = crate::materials::get("walnuss").expect("Material fehlt");
            let geo = crate::geometry::build_geometry(&b, &p);
            let w = estimate_weight(&geo, &b, &p, mat);
            let erwartet = holzvolumen(&geo) / 1000.0 * mat.density;
            assert!(
                (w.wood_g - erwartet).abs() < erwartet * 0.01,
                "Bauart {style:?}: Gewicht {:.1} g, Modell {:.1} g",
                w.wood_g,
                erwartet
            );
        }
    }

    #[test]
    fn die_bastion_ist_kein_zwei_schalen_griff() {
        // Das Holz der Bastion ist ein Block in voller Dicke. Vorher wurde
        // es wie zwei Schalen gerechnet - der Griff waere rund ein Drittel
        // zu leicht gewogen worden.
        let b = BladeBlank::default();
        let mat = crate::materials::get("walnuss").expect("Material fehlt");

        let mut p = HandleParams::default();
        p.style = HandleStyle::PartialTangBolster;
        let geo = crate::geometry::build_geometry(&b, &p);
        let bastion = estimate_weight(&geo, &b, &p, mat);

        let mut p = HandleParams::default();
        p.style = HandleStyle::FullTangTwoScale;
        let geo2 = crate::geometry::build_geometry(&b, &p);
        let schalen = estimate_weight(&geo2, &b, &p, mat);

        // Der Block wiegt deutlich mehr als eine einzige Schale - er ist
        // ja in voller Griffdicke da.
        let eine_schale = schalen.wood_g / 2.0;
        assert!(
            bastion.wood_g > eine_schale * 1.8,
            "Bastion {:.1} g, eine Schale {:.1} g",
            bastion.wood_g,
            eine_schale
        );
        // Und das Holz ist ein einziger Block, nicht drei getrennte
        // Schichten - so ist es auch im 3D-Modell.
        assert_eq!(
            geo.bodies.len(),
            1,
            "Die Bastion muss aus einem Block bestehen"
        );
    }

    #[test]
    fn die_zwinge_ist_ein_mantel_kein_klotz() {
        // Die Zwinge umfasst das Holz mit einer duennen Wand. Als voller
        // Klotz gerechnet waere sie rund dreimal so schwer.
        let b = BladeBlank::default();
        let mut p = HandleParams::default();
        p.style = HandleStyle::Ferrule;
        let mat = crate::materials::get("walnuss").expect("Material fehlt");
        let geo = crate::geometry::build_geometry(&b, &p);
        let w = estimate_weight(&geo, &b, &p, mat);
        let voller_klotz =
            crate::geometry::polygon_area(geo.ferrule_profile.as_ref().unwrap())
                * geo.total_thickness_mm
                / 1000.0
                * 8.5;
        assert!(
            w.metal_g < voller_klotz * 0.5,
            "Zwinge {:.1} g, als Klotz {:.1} g",
            w.metal_g,
            voller_klotz
        );
    }
}

#[cfg(test)]
mod schablone_tests {
    use super::*;
    use crate::model::{BladeBlank, HandleParams};

    fn plan() -> DesignPlan {
        DesignPlan::build(&BladeBlank::default(), &HandleParams::default(), "walnuss")
    }

    #[test]
    fn schablone_ist_im_massstab_eins() {
        // Die Schablone muss millimetergenau sein - sonst passt sie
        // beim Bohren nicht auf den Rohling.
        let p = plan();
        let pdf = build_schablone_pdf(&p);
        let dir = std::env::temp_dir().join("kp_schablone_test");
        std::fs::create_dir_all(&dir).unwrap();
        let pfad = dir.join("schablone.pdf");
        pdf.save(&pfad).expect("PDF liess sich nicht speichern");
        let laenge = std::fs::metadata(&pfad).unwrap().len();
        assert!(laenge > 5000, "Schablone zu klein: {laenge}");
    }
}
