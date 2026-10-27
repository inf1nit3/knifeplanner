//! SVG-Export: Bohrplan und Griffkontur als Vektorgrafik (Massstab 1:1 in mm).

use std::fmt::Write as _;

use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};
use crate::materials::MaterialVerdict;

use super::Frame;

fn path_data(pts: &[(f64, f64)], close: bool) -> String {
    let mut s = String::new();
    for (i, &(x, y)) in pts.iter().enumerate() {
        let _ = if i == 0 {
            write!(s, "M {x:.3},{y:.3}")
        } else {
            write!(s, " L {x:.3},{y:.3}")
        };
    }
    if close {
        s.push_str(" Z");
    }
    s
}

/// Erzeugt die SVG-Zeichnung des Bohrplans.
pub fn build_svg(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &MaterialVerdict,
    frame: Frame,
) -> String {
    let w = frame.width();
    let h = frame.height();
    let mut s = String::new();
    s.push_str(&format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
         <svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{w:.2}mm\" height=\"{h:.2}mm\" \
         viewBox=\"{:.3} {:.3} {:.3} {:.3}\">\n",
        frame.min_x, frame.min_y, w, h
    ));
    s.push_str(
        "<style>\
         .out{fill:#e8dcc4;stroke:#2b2118;stroke-width:0.6;}\
         .tang{fill:none;stroke:#1f7a3f;stroke-width:0.4;stroke-dasharray:3 1.5;}\
         .rec{fill:none;stroke:#b26a00;stroke-width:0.3;stroke-dasharray:1.5 1;}\
         .hole{fill:#fff;stroke:#c62828;stroke-width:0.35;}\
         .axis{stroke:#9a9a9a;stroke-width:0.2;}\
         .metal{fill:#cfd6dc;stroke:#37474f;stroke-width:0.5;}\
         text{font-family:Helvetica,Arial,sans-serif;fill:#2b2118;}\
         </style>\n",
    );
    s.push_str(&format!(
        "<rect x=\"{:.3}\" y=\"{:.3}\" width=\"{w:.3}\" height=\"{h:.3}\" fill=\"#fbfaf7\"/>\n",
        frame.min_x, frame.min_y
    ));

    // Tangmittenlinie
    s.push_str(&format!(
        "<line class=\"axis\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
        frame.min_x,
        frame.min_y + 8.0,
        frame.max_x,
        frame.min_y + 8.0
    ));

    // Aussenkontur
    s.push_str(&format!(
        "<path class=\"out\" d=\"{}\"/>\n",
        path_data(&geo.outline, true)
    ));
    if let Some(b) = &geo.bolster_profile {
        s.push_str(&format!(
            "<path class=\"metal\" d=\"{}\"/>\n",
            path_data(b, true)
        ));
    }
    if let Some(f) = &geo.ferrule_profile {
        s.push_str(&format!(
            "<path class=\"metal\" d=\"{}\"/>\n",
            path_data(f, true)
        ));
    }

    // Angel und Aussparung
    s.push_str(&format!(
        "<path class=\"tang\" d=\"{}\"/>\n",
        path_data(&geo.tang_profile, true)
    ));
    s.push_str(&format!(
        "<path class=\"rec\" d=\"{}\"/>\n",
        path_data(&geo.recess, true)
    ));

    // Bohrlöcher
    for pin in &geo.pins {
        let r = pin.dia_mm / 2.0;
        s.push_str(&format!(
            "<circle class=\"hole\" cx=\"{:.3}\" cy=\"{:.3}\" r=\"{r:.3}\"/>\n",
            pin.x_mm, pin.y_mm
        ));
        s.push_str(&format!(
            "<line class=\"axis\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
            pin.x_mm - r - 2.0,
            pin.y_mm,
            pin.x_mm + r + 2.0,
            pin.y_mm
        ));
        s.push_str(&format!(
            "<line class=\"axis\" x1=\"{:.3}\" y1=\"{:.3}\" x2=\"{:.3}\" y2=\"{:.3}\"/>\n",
            pin.x_mm,
            pin.y_mm - r - 2.0,
            pin.x_mm,
            pin.y_mm + r + 2.0
        ));
        s.push_str(&format!(
            "<text x=\"{:.3}\" y=\"{:.3}\" font-size=\"2.6\">{}</text>\n",
            pin.x_mm - r - 2.0,
            pin.y_mm - r - 3.2,
            xml_escape(&format!("{} d{:.1}", pin.label, pin.dia_mm))
        ));
    }

    // Massangaben
    let style_txt = match params.style {
        HandleStyle::FullTangTwoScale => "Full Tang, zwei Griffschalen",
        HandleStyle::PartialTangBolster => "Verdeckter Tang mit Bastion",
        HandleStyle::OnePiece => "Ein-Stueck-Griff",
        HandleStyle::Ferrule => "Holzgriff mit Messing-Zwinge",
    };
    let lines = vec![
        format!("Griffplan - {style_txt}"),
        format!(
            "Material: {} ({})",
            material.material_name, material.rating
        ),
        format!(
            "Angel: {:.1} x {:.1} mm, Laenge {:.1} mm | Schalendicke {:.1} mm | Gesamt {:.1} mm",
            crate::geometry::angel_hoehe_am_heft(blank, params),
            blank.tang_thickness_mm,
            blank.tang_length_mm,
            geo.scale_thickness_mm,
            geo.total_thickness_mm
        ),
        format!(
            "Grifflaenge {:.1} mm | Breite {:.1} mm | {}",
            geo.handle_length_mm, geo.handle_height_mm, params.grip.label()
        ),
    ];
    let mut ty = frame.min_y + h - 5.0;
    for (i, l) in lines.iter().enumerate() {
        let size = if i == 0 { 5.0 } else { 3.2 };
        s.push_str(&format!(
            "<text x=\"{:.3}\" y=\"{ty:.3}\" font-size=\"{size:.1}\" font-weight=\"{}\">{}</text>\n",
            frame.min_x + 2.0,
            if i == 0 { "bold" } else { "normal" },
            xml_escape(l)
        ));
        ty -= size * 1.5;
    }

    // Massstabsleiste
    let bar = 50.0;
    let bx = frame.max_x - bar - 2.0;
    let by = frame.min_y + 2.0;
    s.push_str(&format!(
        "<line x1=\"{bx:.2}\" y1=\"{by:.2}\" x2=\"{:.2}\" y2=\"{by:.2}\" stroke=\"#2b2118\" stroke-width=\"0.8\"/>\n",
        bx + bar
    ));
    for i in 0..=5 {
        let x = bx + bar * i as f64 / 5.0;
        s.push_str(&format!(
            "<line x1=\"{x:.2}\" y1=\"{:.2}\" x2=\"{x:.2}\" y2=\"{:.2}\" stroke=\"#2b2118\" stroke-width=\"0.5\"/>\n",
            by - 1.5,
            by + 1.5
        ));
    }
    s.push_str(&format!(
        "<text x=\"{bx:.2}\" y=\"{:.2}\" font-size=\"3\">50 mm</text>\n",
        by - 4.0
    ));

    s.push_str("</svg>\n");
    s
}

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}
