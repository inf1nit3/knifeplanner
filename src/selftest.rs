//! Selbsttest: prueft die ganze Kette vom Rohling bis zur Exportdatei.
//!
//! Aufruf:  knifeplanner.exe --selbsttest [Zielordner]
//!
//! Der Test laeuft ohne Fenster und schreibt einen Bericht in die Konsole
//! und in eine Datei. Exit-Code 0 = alles bestanden, 1 = mindestens ein Fehler.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use crate::export::{dxf, preview, stl, svg, Frame};
use crate::geometry::{build_geometry, plan_pins, recommend_style};
use crate::mesh::is_simple_polygon;
use crate::materials;
use crate::model::{BladeBlank, HandleParams, HandleStyle, ProfileStyle, TangStyle};
use crate::plan::DesignPlan;

/// Das Ergebnis eines Pruefpunktes.
struct Result_ {
    name: String,
    ok: bool,
    detail: String,
}

pub struct Report {
    results: Vec<Result_>,
    passed: usize,
    failed: usize,
}

impl Report {
    /// Anzahl der fehlgeschlagenen Pruefpunkte.
    pub fn failed(&self) -> usize {
        self.failed
    }
    /// Anzahl der bestandenen Pruefpunkte.
    pub fn passed(&self) -> usize {
        self.passed
    }
}

impl Report {
    fn new() -> Report {
        Report {
            results: Vec::new(),
            passed: 0,
            failed: 0,
        }
    }

    fn check(&mut self, name: &str, ok: bool, detail: impl Into<String>) {
        let detail = detail.into();
        if ok {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
        self.results.push(Result_ {
            name: name.to_string(),
            ok,
            detail,
        });
    }

    fn section(&mut self, name: &str) {
        self.results.push(Result_ {
            name: format!("== {name}"),
            ok: true,
            detail: String::new(),
        });
    }

    /// Schreibt den Bericht als Text.
    pub fn to_text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(
            out,
            "KnifePlanner Selbsttest - {}",
            chrono_like_now()
        );
        let _ = writeln!(out, "{}", "=".repeat(64));
        for r in &self.results {
            if r.name.starts_with("== ") {
                let _ = writeln!(out, "\n{}", r.name);
                continue;
            }
            let _ = writeln!(
                out,
                "  [{}] {:<42} {}",
                if r.ok { " ok " } else { "FEHL" },
                r.name,
                r.detail
            );
        }
        let _ = writeln!(out, "\n{}", "=".repeat(64));
        let _ = writeln!(
            out,
            "{} Pruefpunkte bestanden, {} fehlgeschlagen",
            self.passed, self.failed
        );
        out
    }
}

/// Ein Zeitstempel ohne externe crate.
fn chrono_like_now() -> String {
    let d = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    format!("Laufzeit seit Epoch {d} s")
}

/// Prueft, ob eine PDF-Datei plausibel ist.
fn pdf_ok(data: &[u8]) -> bool {
    data.starts_with(b"%PDF-") && data.windows(4).any(|w| w == b"xref") && data.ends_with(b"%%EOF\n")
}

/// Prueft, ob eine PNG-Datei plausibel ist.
fn png_ok(data: &[u8]) -> bool {
    data.len() > 24 && data.starts_with(&[0x89, b'P', b'N', b'G'])
}

/// Prueft, ob ein binares STL plausibel ist: die Dateigroesse muss zur
/// Dreieckszahl passen.
fn stl_ok(data: &[u8]) -> bool {
    if data.len() < 84 {
        return false;
    }
    let n = u32::from_le_bytes([data[80], data[81], data[82], data[83]]) as usize;
    data.len() == 84 + n * 50 && n > 0
}

/// Prueft das DXF auf Vollstaendigkeit.
fn dxf_ok(text: &str) -> bool {
    text.contains("SECTION") && text.contains("ENTITIES") && text.contains("SCHALE") && text.ends_with("EOF\n")
}

/// Fuehrt den Selbsttest aus und gibt den Bericht zurueck.
pub fn run(target_dir: &Path) -> (Report, String) {
    let mut r = Report::new();

    // ---------------------------------------------------------------
    r.section("Geometrie ueber den gesamten Wertebereich");
    let mut kombis = 0usize;
    let mut fehler = 0usize;
    let mut erster_fehler = String::new();
    for tang_len in [25.0_f64, 60.0, 110.0, 190.0] {
        for tang_w in [2.0_f64, 6.5, 13.0] {
            for tang_t in [2.0_f64, 6.5, 11.0] {
                for wall in [0.8_f64, 2.0, 4.5] {
                    for style in [
                        HandleStyle::FullTangTwoScale,
                        HandleStyle::PartialTangBolster,
                        HandleStyle::OnePiece,
                        HandleStyle::Ferrule,
                    ] {
                        for profile in [ProfileStyle::Blade, ProfileStyle::Ergonomic] {
                            kombis += 1;
                            let mut b = BladeBlank::default();
                            b.tang_length_mm = tang_len;
                            b.tang_width_mm = tang_w;
                            b.tang_thickness_mm = tang_t;
                            let mut p = HandleParams::default();
                            p.style = style;
                            p.profile = profile;
                            p.wall_min_mm = wall;
                            let geo = build_geometry(&b, &p);
                            let mut ok = is_simple_polygon(&geo.outline);
                            let mut why = String::new();
                            if !ok {
                                why = "Kontur schneidet sich selbst".into();
                            }
                            // Die Pins muessen aufsteigend und auf der Angel liegen
                            let pins = plan_pins(&b, &p);
                            let mut last = f64::MIN;
                            for pin in pins.iter().filter(|x| x.kind == "through") {
                                if pin.x_mm <= last || pin.x_mm < 0.0 || pin.x_mm > tang_len + 0.01 {
                                    ok = false;
                                    why = format!("Bohrung {} ausserhalb oder unsortiert", pin.label);
                                }
                                last = pin.x_mm;
                            }
                            if !ok {
                                fehler += 1;
                                if erster_fehler.is_empty() {
                                    erster_fehler = format!(
                                        "Angel {tang_len}x{tang_w}x{tang_t}, Wand {wall}, {style:?}, {profile:?}: {why}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    r.check(
        "Kontur schneidungsfrei und Bohrungen gueltig",
        fehler == 0,
        if fehler == 0 {
            format!("{kombis} Kombinationen geprueft")
        } else {
            format!("{fehler} von {kombis} fehlerhaft - {erster_fehler}")
        },
    );

    // ---------------------------------------------------------------
    r.section("Geometrie mit randfaelligen Werten");
    let mut rand_ok = true;
    let mut rand_detail = String::new();
    for (name, b, p) in randfaelle() {
        let geo = build_geometry(&b, &p);
        if geo.outline.len() < 3 {
            rand_ok = false;
            rand_detail = format!("{name}: Kontur zu kurz");
            break;
        }
        if !is_simple_polygon(&geo.outline) {
            rand_ok = false;
            rand_detail = format!("{name}: Kontur schneidet sich selbst");
            break;
        }
        if geo.handle_length_mm <= 0.0 || geo.total_thickness_mm <= 0.0 {
            rand_ok = false;
            rand_detail = format!("{name}: unplausible Masse");
            break;
        }
    }
    r.check(
        "Randfaelle (kurze Angel, dicke Wand, minimaler Winkel)",
        rand_ok,
        if rand_ok {
            "alleFaelle berechenbar".into()
        } else {
            rand_detail
        },
    );

    // ---------------------------------------------------------------
    r.section("Bauarten und Exporte");
    let mut export_ok = true;
    let mut export_detail = String::new();
    for style in [
        HandleStyle::FullTangTwoScale,
        HandleStyle::PartialTangBolster,
        HandleStyle::OnePiece,
        HandleStyle::Ferrule,
    ] {
        let blank = BladeBlank::default();
        let mut params = HandleParams::default();
        params.style = style;
        let plan = DesignPlan::build(&blank, &params, "walnuss");
        let dir = target_dir.join(format!("selbsttest_{}", style_label(style)));
        if let Err(e) = std::fs::create_dir_all(&dir) {
            export_ok = false;
            export_detail = format!("{}: Ordner nicht anlegbar: {e}", style_label(style));
            break;
        }

        // DXF
        let dxf_path = match dxf::write_dxf(&dir, &plan.geometry, &blank, &params, &plan.verdict) {
            Ok(p) => p,
            Err(e) => {
                export_ok = false;
                export_detail = format!("{}: DXF: {e}", style_label(style));
                break;
            }
        };
        let dxf_text = std::fs::read_to_string(&dxf_path).unwrap_or_default();
        if !dxf_ok(&dxf_text) {
            export_ok = false;
            export_detail = format!("{}: DXF unvollstaendig", style_label(style));
            break;
        }

        // STL
        let stl_path = match stl::write_stl_file(&dir, &plan.geometry, &blank, &params, &plan.verdict) {
            Ok(p) => p,
            Err(e) => {
                export_ok = false;
                export_detail = format!("{}: STL: {e}", style_label(style));
                break;
            }
        };
        let stl_data = std::fs::read(&stl_path).unwrap_or_default();
        if !stl_ok(&stl_data) {
            export_ok = false;
            export_detail = format!("{}: STL ungueltig", style_label(style));
            break;
        }
        // Die Klinge muss im Modell liegen - sie reicht nach vorn ueber
        // den Griff hinaus.
        if let Ok(mesh) = crate::freecad::lade_stl(&stl_path) {
            let vorne = mesh
                .vertices
                .iter()
                .map(|v| v.0)
                .fold(f64::MAX, f64::min);
            if vorne > -blank.blade_length_mm * 0.8 {
                export_ok = false;
                export_detail = format!(
                    "{}: Klinge fehlt im Modell (vorderster Punkt {:.1})",
                    style_label(style),
                    vorne
                );
                break;
            }
        }

        // OBJ
        let (obj_path, mtl_path) =
            match stl::write_obj(&dir, &plan.geometry, &blank, &params, &plan.verdict) {
                Ok(v) => v,
                Err(e) => {
                    export_ok = false;
                    export_detail = format!("{}: OBJ: {e}", style_label(style));
                    break;
                }
            };
        let obj_text = std::fs::read_to_string(&obj_path).unwrap_or_default();
        if !obj_text.contains("usemtl") || !std::fs::read_to_string(&mtl_path).unwrap_or_default().contains("newmtl") {
            export_ok = false;
            export_detail = format!("{}: OBJ/MTL unvollstaendig", style_label(style));
            break;
        }

        // SVG
        let frame = Frame::from_geometry(&plan.geometry, &blank);
        let svg_text = svg::build_svg(&plan.geometry, &blank, &params, &plan.verdict, frame);
        if !svg_text.contains("</svg>") {
            export_ok = false;
            export_detail = format!("{}: SVG unvollstaendig", style_label(style));
            break;
        }

        // PDFs
        let bohrplan = dir.join("bohrplan.pdf");
        if plan_pdf(&plan, true, &bohrplan).is_err() || !pdf_ok(&std::fs::read(&bohrplan).unwrap_or_default()) {
            export_ok = false;
            export_detail = format!("{}: Bohrplan-PDF fehlerhaft", style_label(style));
            break;
        }
        let bauplan = dir.join("bauplan.pdf");
        if plan_pdf(&plan, false, &bauplan).is_err() || !pdf_ok(&std::fs::read(&bauplan).unwrap_or_default()) {
            export_ok = false;
            export_detail = format!("{}: Bauplan-PDF fehlerhaft", style_label(style));
            break;
        }

        // Vorschau-PNG
        let png = dir.join("vorschau.png");
        if preview::render_preview(&plan.geometry, &blank, &params, &plan.verdict, &png).is_err()
            || !png_ok(&std::fs::read(&png).unwrap_or_default())
        {
            export_ok = false;
            export_detail = format!("{}: Vorschau-PNG fehlerhaft", style_label(style));
            break;
        }
    }
    r.check(
        "Alle Bauarten exportieren fehlerfrei",
        export_ok,
        if export_ok {
            "DXF, STL, OBJ, SVG, 2 PDFs, PNG fuer 4 Bauarten".into()
        } else {
            export_detail
        },
    );

    // ---------------------------------------------------------------
    r.section("Querschnitt und Materialtabelle");
    let mut cs_ok = true;
    let mut cs_detail = String::new();
    for style in [
        HandleStyle::FullTangTwoScale,
        HandleStyle::PartialTangBolster,
        HandleStyle::OnePiece,
        HandleStyle::Ferrule,
    ] {
        let blank = BladeBlank::default();
        let mut params = HandleParams::default();
        params.style = style;
        let geo = build_geometry(&blank, &params);
        match crate::geometry::cross_section(&geo, &blank, geo.handle_length_mm * 0.45) {
            Some(cs) => {
                if cs.height <= 0.0 || cs.wall_spine <= 0.0 {
                    cs_ok = false;
                    cs_detail = format!("{}: Querschnitt unplausible", style_label(style));
                }
            }
            None => {
                cs_ok = false;
                cs_detail = format!("{}: kein Querschnitt berechenbar", style_label(style));
            }
        }
    }
    r.check(
        "Querschnitt fuer alle Bauarten berechenbar",
        cs_ok,
        if cs_ok {
            "Hoehe und Wandstaerken plausibel".into()
        } else {
            cs_detail
        },
    );

    let mat_ok = materials::MATERIALS
        .iter()
        .all(|m| !m.name_de.is_empty() && m.density > 0.0);
    r.check(
        "Materialdatenbank lesbar",
        mat_ok,
        format!("{} Materialien", materials::MATERIALS.len()),
    );

    // ---------------------------------------------------------------
    r.section("Empfehlung und Plan");
    let blank = BladeBlank {
        has_bolster: true,
        ..Default::default()
    };
    let empfohlen = recommend_style(&blank);
    r.check(
        "Bauarten-Empfehlung folgt dem Rohling",
        empfohlen == HandleStyle::PartialTangBolster,
        format!("Bastion am Rohling -> {}", empfohlen.label()),
    );

    let mut plan_ok = true;
    let mut plan_detail = String::new();
    for tang_style in [TangStyle::Full, TangStyle::Partial, TangStyle::Hidden] {
        let mut b = BladeBlank::default();
        b.tang_style = tang_style;
        let p = HandleParams::default();
        let plan = DesignPlan::build(&b, &p, "palisander");
        if plan.report_text().len() < 500 {
            plan_ok = false;
            plan_detail = format!("{tang_style:?}: Bauplan zu kurz");
        }
        if plan.geometry.pins.is_empty() {
            plan_ok = false;
            plan_detail = format!("{tang_style:?}: keine Bohrungen geplant");
        }
    }
    r.check(
        "Bauplan und Bohrplan je Angelart",
        plan_ok,
        if plan_ok {
            "3 Angelarten mit Bauplan und Bohrungen".into()
        } else {
            plan_detail
        },
    );

    // ---------------------------------------------------------------
    r.section("Speichern und Laden");
    let mut io_ok = true;
    let mut io_detail = String::new();
    let mut proj = crate::project::Project::default();
    proj.name = "Selbsttest".into();
    proj.blank.tang_length_mm = 123.0;
    let file = target_dir.join("selbsttest_projekt.json");
    if proj.save(&file).is_err() {
        io_ok = false;
        io_detail = "Speichern fehlgeschlagen".into();
    } else if let Ok(back) = crate::project::Project::load(&file) {
        if back.blank.tang_length_mm != 123.0 || back.name != "Selbsttest" {
            io_ok = false;
            io_detail = "Werte beim Laden veraendert".into();
        }
    } else {
        io_ok = false;
        io_detail = "Laden fehlgeschlagen".into();
    }
    r.check(
        "Plan speichern und laden",
        io_ok,
        if io_ok {
            "Roundtrip verlustfrei".into()
        } else {
            io_detail
        },
    );

    // ---------------------------------------------------------------
    r.section("Bildanalyse (nur mit API-Schluessel)");
    let cfg = crate::vision::load_config();
    let mut client = crate::vision::VisionClient::new(cfg.clone());
    if client.is_ready() {
        // Ein echtes Testbild erzeugen und wirklich analysieren lassen
        let testbild = target_dir.join("selbsttest_bild.png");
        let _ = schreibe_testbild(&testbild);
        match client.analyze_material(&testbild) {
            Ok((a, _)) => {
                r.check(
                    "Bildanalyse mit echtem Bild",
                    true,
                    format!(
                        "Antwort erhalten (Kategorie: {})",
                        a.category.clone().unwrap_or_else(|| "-".into())
                    ),
                );
            }
            Err(e) => {
                r.check(
                    "Bildanalyse mit echtem Bild",
                    false,
                    truncate(&e.to_string(), 90),
                );
            }
        }
        r.check(
            "Modell konfiguriert",
            !cfg.model().is_empty(),
            format!("Modell {}", cfg.model()),
        );
    } else {
        r.check(
            "Verbindung zur API",
            false,
            "Kein Schluessel - in den Einstellungen oder per MINIMAX_API_KEY",
        );
    }

    // ---------------------------------------------------------------
    r.section("FreeCAD (optional)");
    let einstellungen = crate::settings::lade();
    match crate::freecad::finde_freecad(&einstellungen.freecad_pfad) {
        Some(cmd) => {
            let blank = BladeBlank::default();
            for style in crate::model::HandleStyle::all() {
                let mut params = HandleParams::default();
                params.style = style;
                let plan = DesignPlan::build(&blank, &params, "walnuss");
                let ziel = target_dir.join(format!("freecad_{:?}", style));
                match crate::freecad::baue(&plan, &blank, &params, &cmd, &ziel) {
                    Ok(ergebnis) => {
                        let stl_ok = std::fs::read(&ergebnis.stl)
                            .map(|d| stl_ok(&d))
                            .unwrap_or(false);
                        // Die Klinge muss sich vom Griff trennen lassen,
                        // damit sie in der 3D-Ansicht als Stahl erscheint.
                        let klinge_da = crate::freecad::lade_stl(&ergebnis.stl)
                            .map(|m| {
                                let (klinge, _) = crate::mesh::teile_bei_x(&m, 0.0);
                                klinge.faces.len()
                            })
                            .unwrap_or(0);
                        if stl_ok && klinge_da < 10 {
                            r.check(
                                &format!("FreeCAD baut {}", style.label()),
                                false,
                                "im Modell steckt keine Klinge".to_string(),
                            );
                            continue;
                        }
                        // Das eigene Netz und das FreeCAD-Volumen muessen
                        // zusammenpassen - sonst rechnen die beiden Modelle
                        // verschiedene Dinge.
                        let eigen: f64 = plan
                            .geometry
                            .bodies
                            .iter()
                            .map(|b| crate::mesh::signed_volume(&crate::mesh::loft(b)))
                            .sum();
                        if ergebnis.holzvolumen_mm3 > 0.0 {
                            let abweichung =
                                (eigen - ergebnis.holzvolumen_mm3).abs() / ergebnis.holzvolumen_mm3;
                            if abweichung > 0.03 {
                                r.check(
                                    &format!("FreeCAD baut {}", style.label()),
                                    false,
                                    format!(
                                        "eigenes Netz {eigen:.0} mm3, FreeCAD {:.0} mm3",
                                        ergebnis.holzvolumen_mm3
                                    ),
                                );
                                continue;
                            }
                        }
                        // Die STEP-Datei muss echte Flaechen enthalten -
                        // eine leere Datei waere kein brauchbares Modell.
                        let step_gross = std::fs::metadata(&ergebnis.step)
                            .map(|m| m.len() > 20_000)
                            .unwrap_or(false);
                        r.check(
                            &format!("FreeCAD baut {}", style.label()),
                            stl_ok && step_gross,
                            format!(
                                "{} in {} ms, STEP {:.1} MB, Volumen {:.0} mm3",
                                ergebnis.stl.display(),
                                ergebnis.dauer_ms,
                                std::fs::metadata(&ergebnis.step)
                                    .map(|m| m.len() as f64 / 1_000_000.0)
                                    .unwrap_or(0.0),
                                ergebnis.volumen_mm3
                            ),
                        );
                    }
                    Err(e) => {
                        r.check(
                            &format!("FreeCAD baut {}", style.label()),
                            false,
                            truncate(&e, 120),
                        );
                    }
                }
            }
        }
        None => {
            r.check(
                "FreeCAD gefunden",
                false,
                "nicht installiert - das eigene Modell wird genutzt (kein Fehler)",
            );
        }
    }

    // ---------------------------------------------------------------
    let text = r.to_text();
    (r, text)
}

/// Schreibt ein einfaches Testbild (Holzfarben mit Maserung).
fn schreibe_testbild(path: &Path) -> std::io::Result<()> {
    let (w, h) = (96u32, 72u32);
    let mut img = image::RgbImage::new(w, h);
    for y in 0..h {
        for x in 0..w {
            let welle = ((x as f64 * 0.25) + (y as f64 * 0.05)).sin();
            let r = (140.0 + welle * 25.0) as u8;
            let g = (95.0 + welle * 18.0) as u8;
            let b = (58.0 + welle * 10.0) as u8;
            img.put_pixel(x, y, image::Rgb([r, g, b]));
        }
    }
    img.save(path)
        .map_err(|e| std::io::Error::other(e.to_string()))
}

fn plan_pdf(plan: &DesignPlan, bohrplan: bool, path: &Path) -> std::io::Result<()> {
    let doc = if bohrplan {
        crate::plan::build_bohrplan_pdf(plan)
    } else {
        crate::plan::build_bauplan_pdf(plan)
    };
    doc.save(path)
}

fn style_label(style: HandleStyle) -> &'static str {
    match style {
        HandleStyle::FullTangTwoScale => "fulltang",
        HandleStyle::PartialTangBolster => "bastion",
        HandleStyle::OnePiece => "einstueck",
        HandleStyle::Ferrule => "zwinge",
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}...")
    }
}

/// Randfaelle fuer die Geometrie.
fn randfaelle() -> Vec<(String, BladeBlank, HandleParams)> {
    let mut out: Vec<(String, BladeBlank, HandleParams)> = Vec::new();

    // Sehr kurze Angel
    out.push((
        "sehr kurze Angel".into(),
        BladeBlank {
            tang_length_mm: 8.0,
            tang_width_mm: 3.0,
            ..Default::default()
        },
        HandleParams {
            wall_min_mm: 2.0,
            ..Default::default()
        },
    ));

    // Angel breiter als das Heft
    out.push((
        "Angel breiter als das Heft".into(),
        BladeBlank {
            tang_width_mm: 20.0,
            spine_offset_mm: 5.0,
            edge_offset_mm: 5.0,
            ..Default::default()
        },
        HandleParams::default(),
    ));

    // Sehr dicke Wand
    out.push((
        "sehr dicke Angel und Wand".into(),
        BladeBlank {
            tang_thickness_mm: 14.0,
            ..Default::default()
        },
        HandleParams {
            wall_min_mm: 5.0,
            ..Default::default()
        },
    ));

    // Starke Vorderkantenfase
    out.push((
        "starke Vorderkantenfase".into(),
        BladeBlank::default(),
        HandleParams {
            front_bevel_deg: 30.0,
            ..Default::default()
        },
    ));

    // Sehr duennes Material
    out.push((
        "sehr duennes Material".into(),
        BladeBlank {
            tang_thickness_mm: 2.0,
            ..Default::default()
        },
        HandleParams {
            target_thickness_mm: 12.0,
            edge_break_mm: 0.1,
            ..Default::default()
        },
    ));

    out
}



/// Schreibt den Bericht in eine Datei neben den Zielordner.
pub fn write_report(dir: &Path, text: &str) -> PathBuf {
    let path = dir.join("selbsttest.txt");
    let _ = std::fs::write(&path, text);
    path
}
