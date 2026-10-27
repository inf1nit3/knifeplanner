//! Zusammenfuehrung von Rohling, Gestaltung und Material zum fertigen Plan,
//! inklusive Bauplan-Text und PDF-Ausgaben.

use crate::export::pdf::Pdf;
use crate::geometry::{build_geometry, recommend_style};
use crate::materials::{self, MaterialVerdict};
use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};
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
        let (z_laenge, z_hoehe, z_dicke) =
            crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);
        let zusatz = if p.style.has_two_scales() {
            " (zwei Schalen daraus, Angel aus Stahl)"
        } else {
            " (mit Schleif- und Hobelzugabe)"
        };
        out.push_str(&format!(
            "Zuschnitt:  Holzleiste {:.0} x {:.0} x {:.0} mm{}\n",
            z_laenge, z_hoehe, z_dicke, zusatz
        ));
        out.push_str(&format!(
            "Rohling:    Klinke {:.0} x {:.0} x {:.1} mm, Angel {:.1} x {:.1} x {:.0} mm ({})\n",
            b.blade_length_mm,
            b.blade_width_mm,
            b.blade_thickness_mm,
            b.tang_length_mm,
            b.tang_width_mm,
            b.tang_thickness_mm,
            b.tang_style.label()
        ));
        out.push_str(&format!(
            "Griff:      {:.0} mm lang, {:.0} mm hoch, {:.1} mm dick, Schalendicke {:.1} mm\n",
            g.handle_length_mm, g.handle_height_mm, g.total_thickness_mm, g.scale_thickness_mm
        ));
        out.push_str(&format!(
            "Querschnitt: {} - Ruecken {:.1} mm, Bauch {:.1} mm, Laengsgrat {:.1} mm, \
             Verjuengung {:.1} mm\n",
            p.section.shape.label(),
            p.section.spine_round_mm,
            p.section.belly_round_mm,
            p.section.keel_mm,
            p.section.taper_mm
        ));
        out.push_str(&format!(
            "Profil:     {}\n",
            match p.profile {
                crate::model::ProfileStyle::Blade => "folgt dem Heftprofil des Rohlings".to_string(),
                crate::model::ProfileStyle::Ergonomic => format!(
                    "eigenes Ergonomieprofil, Griffhöhe {:.0} mm, Fingerkehle {:.1} mm, Ballenauflage {:.1} mm",
                    p.grip_height_mm, p.finger_groove_mm, p.belly_drop_mm
                ),
            }
        ));
        out.push('\n');

        out.push_str("MASSE\n");
        out.push_str(&"-".repeat(64));
        out.push('\n');
        for (k, val) in &g.metrics {
            out.push_str(&format!("  {k:<26} {val:>8.1}\n"));
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
                        "Lanyardloch durch die Hoehe"
                    } else {
                        "Lanyardloch quer durch die Dicke"
                    }
                }
            };
            out.push_str(&format!(
                "  {:<22} {:>8.1} {:>7.1}  {}\n",
                pin.label, pin.x_mm, pin.dia_mm, art
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
                    out.push_str(&format!("    Auffaelligkeiten: {}\n", defects.join(", ")));
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
            out.push_str("DATENBLAETTER\n");
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
            "Rohling aufarbeiten: Angel auf {:.1} mm Breite und {:.1} mm Dicke bringen, \
             Heft sauber entgraten.",
            b.tang_width_mm, b.tang_thickness_mm
        ));
        if p.style == HandleStyle::PartialTangBolster {
            steps.push(
                "Bastion anpassen und mit der Angel verschweissen - Schweissnaht planieren, \
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
        steps.push(format!(
            "Material auswaehlen und im Block besorgen - mindestens {:.0} x {:.0} x {:.1} mm, \
             inklusive{zugabe} fuer den Beschnitt.",
            g.handle_length_mm + 10.0,
            g.handle_height_mm + 8.0,
            g.scale_thickness_mm + 2.0,
            zugabe = if g.scale_thickness_mm > 4.0 { " 2 mm" } else { " 1 mm" }
        ));
        steps.push("Material konditionieren: mehrere Tage bei Raumtemperatur lagern, \
                    Krümmung prüfen."
            .to_string());
        steps.push(format!(
            "Griffschale(n) zuschneiden - Kontur aus dem DXF (Layer SCHALE), \
             Schalendicke {:.1} mm.",
            g.scale_thickness_mm
        ));
        steps.push(format!(
            "Aussparung fuer die Angel fräsen: Nut {:.1} mm tief (Angel + {:.2} mm Spalt), \
             Kontur aus dem DXF (Layer AUSSPA).",
            b.tang_thickness_mm + p.glue_gap_mm,
            p.glue_gap_mm
        ));
        steps.push(format!(
            "Bohrungen setzen nach der Bohrplan-Tabelle, Vorsteckbohrungen mit \
             Ø {:.1} mm, Reibnieten mit Ø 2,4 mm halbtief.",
            g.pins
                .iter()
                .find(|x| x.kind == "through")
                .map(|x| x.dia_mm)
                .unwrap_or(3.5)
        ));
        steps.push("Kanten abrunden: umlaufend etwa {:.1} mm, danach die \
                    Vorderkante anschrägen."
            .replace("{:.1}", &format!("{:.1}", p.edge_break_mm))
            .to_string());
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
            "Finish: {} auftragen, zwischen den Oelgaengen 24 Stunden trocknen lassen.",
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
            "Griff rund {:.0} g (Holz {:.0} g, Stahl {:.0} g, Metall {:.0} g). {}",
            self.total_g(),
            self.wood_g,
            self.steel_g,
            self.metal_g,
            self.balance_note
        )
    }
}

/// Schaetzt das Gewicht des Griffes aus Volumen und Materialdichte.
pub fn estimate_weight(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &materials::Material,
) -> Weight {
    let outline_area = crate::geometry::polygon_area(&geo.outline);
    let tang_area = crate::geometry::polygon_area(&geo.tang_profile);

    // Holz: zwei Schalen bzw. ein Block
    let wood_mm3 = if params.style.has_two_scales() {
        outline_area * geo.scale_thickness_mm * 2.0
    } else {
        outline_area * geo.total_thickness_mm
    };
    let wood_cm3 = wood_mm3 / 1000.0;
    let wood_g = wood_cm3 * material.density;

    // Stahl: die Angel im Griff
    let steel_mm3 = tang_area * blank.tang_thickness_mm;
    let steel_g = (steel_mm3 / 1000.0) * 7.85;

    // Metall: Bastion oder Zwinge
    let metal_area = geo
        .bolster_profile
        .as_ref()
        .map(|p| crate::geometry::polygon_area(p))
        .or_else(|| geo.ferrule_profile.as_ref().map(|p| crate::geometry::polygon_area(p)))
        .unwrap_or(0.0);
    let metal_mm3 = metal_area * geo.total_thickness_mm;
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
            "Angel {:.1} x {:.1} mm, Laenge {:.1} mm | Schalendicke {:.1} mm | Griff {:.0} x {:.0} mm",
            b.tang_width_mm,
            b.tang_thickness_mm,
            b.tang_length_mm,
            g.scale_thickness_mm,
            g.handle_length_mm,
            g.handle_height_mm
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
            let tang_dort = crate::geometry::angel_breite(b, pin.x_mm);
            let y_ende = if tang_dort > 0.02 { -tang_dort } else { oben };
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
    pdf.text(15.0, ly, 7.0, "gruen gestrichelt: Angel mit Aussparung", false);
    ly += 4.0;
    pdf.set_fill(0.78, 0.16, 0.16);
    pdf.text(15.0, ly, 7.0, "rote Kreise: Bohrungen, Nummern siehe Tabelle", false);

    // Tabelle der Bohrungen
    let mut ty = draw_y + draw_h + 22.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, ty, 11.0, "Bohrungen", true);
    ty += 8.0;
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
        pdf.text(95.0, ty, 8.0, &format!("{:.1} mm", pin.x_mm), false);
        pdf.text(125.0, ty, 8.0, &format!("{:.1} mm", pin.dia_mm), false);
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
        let mut min_y = cs.tang_half_width;
        let mut max_y = -cs.tang_half_width;
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
        let w = cs.tang_half_width;
        pdf.polyline(
            &[
                map(w, -th),
                map(w, th),
                map(-w, th),
                map(-w, -th),
            ],
            true,
        );
        pdf.set_fill(0.15, 0.15, 0.15);
        pdf.text(
            bx,
            by + bh + 3.0,
            7.0,
            &format!(
                "bei {:.0} mm | Hoehe {:.1} | Wand {:.1} / {:.1} mm",
                x_mitte, cs.height, cs.wall_spine, cs.wall_belly
            ),
            false,
        );
    }

    ty += 4.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(
        15.0,
        ty,
        8.0,
        &format!(
            "Aussparung: {:.1} mm tief (Angel + Spalt) | Kantenabrundung {:.1} mm",
            b.tang_thickness_mm + p.glue_gap_mm,
            p.edge_break_mm
        ),
        false,
    );
    ty += 7.0;
    for w in g.warnings.iter().take(3) {
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

    let (z_laenge, z_hoehe, z_dicke) =
        crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);

    let mut y = 52.0;
    pdf.text(15.0, y, 12.0, "Rohling und Masse", true);
    y += 12.0;
    let rows = vec![
        ("Klingenlaenge", format!("{:.1} mm", b.blade_length_mm)),
        ("Klingenbreite am Heft", format!("{:.1} mm", b.blade_width_mm)),
        ("Klingendicke", format!("{:.1} mm", b.blade_thickness_mm)),
        ("Angel", format!("{:.1} mm lang", b.tang_length_mm)),
        ("Angelbreite", format!("{:.1} mm", b.tang_width_mm)),
        ("Angeldicke", format!("{:.1} mm", b.tang_thickness_mm)),
        ("Grifffaenge", format!("{:.1} mm", g.handle_length_mm)),
        ("Griffhoehe", format!("{:.1} mm", g.handle_height_mm)),
        ("Gesamtdicke", format!("{:.1} mm", g.total_thickness_mm)),
        ("Schalendicke", format!("{:.1} mm", g.scale_thickness_mm)),
        ("Zuschnitt Holz", format!("{:.0} x {:.0} x {:.0} mm", z_laenge, z_hoehe, z_dicke)),
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
            &format!("bei {:.1} mm, Ø {:.1} mm", pin.x_mm, pin.dia_mm),
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
            pdf.text(15.0, y, 12.0, "Datenblaetter", true);
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
        assert!(text.contains("DATENBLAETTER"), "{text}");
        assert!(text.contains("Stahl 1.2316"), "{text}");
        assert!(text.contains("2316.pdf"), "{text}");
        assert!(text.contains("haerte.txt"), "{text}");

        // Ohne Datenblaetter steht kein Abschnitt drin
        let ohne = plan().report_text();
        assert!(!ohne.contains("DATENBLAETTER"), "{ohne}");
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
    // 12 mm Abstand zum Beschnitt - so wie im DXF.
    let zwei_schalen = p.style.has_two_scales();
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
            "{} - {} | Massstab 1:1 | Angel {:.1} x {:.1} mm, {} mm lang",
            style_name(p.style),
            plan.verdict.material_name,
            b.tang_width_mm,
            b.tang_thickness_mm,
            b.tang_length_mm
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
    let (z_laenge, z_hoehe, z_dicke) =
        crate::geometry::zuschnitt(&g.outline, p, g.scale_thickness_mm, g.total_thickness_mm);
    pdf.set_fill(0.1, 0.1, 0.1);
    pdf.text(
        15.0,
        35.0,
        8.5,
        &format!("Zuschnitt: Holzleiste {:.0} x {:.0} x {:.0} mm", z_laenge, z_hoehe, z_dicke),
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
        pdf.polyline(&punkte(&g.recess), true);
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
        pdf.text(105.0, y, 8.0, &format!("{:.1} mm", pin.x_mm), false);
        pdf.text(135.0, y, 8.0, &format!("{:.1} mm", pin.dia_mm), false);
        pdf.text(160.0, y, 8.0, art, false);
        y += 5.0;
    }

    // --- Hinweise ---
    y += 6.0;
    pdf.set_fill(0.0, 0.0, 0.0);
    pdf.text(15.0, y, 11.0, "Hinweise", true);
    y += 9.0;
    let hinweise = [
        "Die Schablone an der Vorderkante des Rohlings ausrichten und festkleben.",
        "Vor dem Bohben die 100-mm-Linie pruefen, dann stimmen alle Masse.",
        "Die gruen gestrichelte Angel liegt mittig im Holz - die Bohrungen gehen durch die Angel.",
        "Bei zwei Schalen: beide Konturen sind spiegelbildlich, es sind zwei gleiche Schalen.",
    ];
    for h in hinweise {
        y = pdf.text_block(15.0, y, 8.0, 180.0, &format!("- {h}"), false, 5.0);
    }

    pdf
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
