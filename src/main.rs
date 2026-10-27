//! Programmstart.

// Ohne Konsolenfenster starten - die Kommandozeilen-Ausgaben stehen
// deshalb im Protokoll (siehe logfile). Das Attribut wirkt nur auf
// diese Datei; Abhaengigkeiten und deren Build-Skripte bleiben
// normale Konsolenprogramme.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use knifeplanner::export::{dxf, preview, stl, svg, Frame};
use knifeplanner::model::{BladeBlank, HandleParams, HandleStyle};
use knifeplanner::plan::{self, DesignPlan};

/// Schreibt eine Zeile auf den Bildschirm und ins Protokoll.
///
/// Ohne Konsolenfenster (siehe windows_subsystem) geht die Ausgabe auf
/// dem Bildschirm verloren - das Protokoll fuehrt sie weiter.
macro_rules! ausgabe {
    ($($teil:tt)*) => {{
        let text = format!($($teil)*);
        println!("{text}");
        knifeplanner::logfile::log(&text);
    }};
}

/// Wie `ausgabe`, nur fuer Fehlermeldungen (zusätzlich auf stderr).
macro_rules! fehler_ausgabe {
    ($($teil:tt)*) => {{
        let text = format!($($teil)*);
        eprintln!("{text}");
        knifeplanner::logfile::log(&text);
    }};
}

fn main() -> anyhow::Result<()> {
    knifeplanner::logfile::init();
    let args: Vec<String> = std::env::args().collect();

    if args.iter().any(|a| a == "--version" || a == "-V") {
        ausgabe!("KnifePlanner {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    if args.iter().any(|a| a == "--demo") {
        return demo();
    }

    // Modell mit FreeCAD bauen: --freecad [pfad]
    if args.iter().any(|a| a == "--freecad") {
        let pfad = args
            .iter()
            .position(|a| a == "--freecad")
            .and_then(|i| args.get(i + 1))
            .map(|s| s.as_str())
            .unwrap_or("");
        let ziel = args
            .iter()
            .position(|a| a == "--freecad")
            .and_then(|i| args.get(i + 2))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("freecad-out"));
        match knifeplanner::freecad::finde_freecad(pfad) {
            Some(cmd) => {
                ausgabe!("FreeCAD: {}", cmd.display());
                let blank = BladeBlank::default();
                let params = HandleParams::default();
                let plan = DesignPlan::build(&blank, &params, "walnuss");
                match knifeplanner::freecad::baue(&plan, &blank, &params, &cmd, &ziel) {
                    Ok(ergebnis) => {
                        ausgabe!(
                            "Fertig in {} ms\nSTL:  {}\nSTEP: {}",
                            ergebnis.dauer_ms,
                            ergebnis.stl.display(),
                            ergebnis.step.display()
                        );
                        if !ergebnis.meldung.is_empty() {
                            ausgabe!("Meldungen: {}", ergebnis.meldung);
                        }
                    }
                    Err(e) => {
                        fehler_ausgabe!("FEHLER: {e}");
                        std::process::exit(1);
                    }
                }
            }
            None => {
                fehler_ausgabe!(
                    "FreeCAD nicht gefunden. Pfad angeben: --freecad <pfad/ziel> \
                     oder FREECAD_CMD setzen."
                );
                std::process::exit(2);
            }
        }
        return Ok(());
    }

    // Eine beliebige STL-Datei mit dem Renderer ansehen:
    // --renderstl <stl> <png> [yaw] [roll]
    if let Some(pos) = args.iter().position(|a| a == "--renderstl") {
        let stl = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("--renderstl braucht eine STL-Datei"))?;
        let png = args
            .get(pos + 2)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("stl.png"));
        let yaw: f32 = args.get(pos + 3).and_then(|s| s.parse().ok()).unwrap_or(40.0);
        let roll: f32 = args.get(pos + 4).and_then(|s| s.parse().ok()).unwrap_or(-20.0);
        let mesh = knifeplanner::freecad::lade_stl(&PathBuf::from(stl)).map_err(anyhow::Error::msg)?;
        let scene = knifeplanner::render3d::Scene {
            parts: vec![knifeplanner::render3d::Part {
                mesh,
                color: [0.72, 0.50, 0.30],
                kind: knifeplanner::render3d::PartKind::Wood,
            }],
        };
        let cam = knifeplanner::render3d::Camera {
            yaw,
            roll,
            dist: 0.0,
            fov: 32.0,
        };
        let (w, h) = (1600usize, 1000usize);
        let rgba = knifeplanner::render3d::render(&scene, w, h, &cam, 4);
        knifeplanner::render3d::save_png(&png, &rgba, w, h)?;
        ausgabe!("Render: {}", png.display());
        return Ok(());
    }

    // Selbsttest ohne Fenster
    if args.iter().any(|a| a == "--selbsttest") {
        let dir = args
            .iter()
            .position(|a| a == "--selbsttest")
            .and_then(|i| args.get(i + 1))
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("selbsttest"));
        std::fs::create_dir_all(&dir).ok();
        let (report, text) = knifeplanner::selftest::run(&dir);
        ausgabe!("{text}");
        let path = knifeplanner::selftest::write_report(&dir, &text);
        ausgabe!("Bericht gespeichert: {}", path.display());
        return if report.failed() == 0 { Ok(()) } else {
            anyhow::bail!("{} Pruefpunkte fehlgeschlagen", report.failed())
        };
    }

    // Bildanalyse ohne Fenster: --analyse <datei> [--rohling|--material]
    if let Some(pos) = args.iter().position(|a| a == "--analyse") {
        let path = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("--analyse braucht einen Dateipfad"))?;
        let material = args.iter().any(|a| a == "--material");
        let cfg = knifeplanner::vision::load_config();
        let mut client = knifeplanner::vision::VisionClient::new(cfg);
        if !client.is_ready() {
            anyhow::bail!(
                "Kein API-Schlüssel. Setze MINIMAX_API_KEY oder trage ihn in der App ein."
            );
        }
        let path = PathBuf::from(path);
        if material {
            let (analysis, raw) = client.analyze_material(&path)?;
            ausgabe!("{}", serde_json::to_string_pretty(&analysis)?);
            ausgabe!("\n--- Rohtext ---\n{raw}");
        } else {
            let (analysis, raw) = client.analyze_blade(&path)?;
            ausgabe!("{}", serde_json::to_string_pretty(&analysis)?);
            ausgabe!("\n--- Rohtext ---\n{raw}");
        }
        return Ok(());
    }

    // 3D-Render ohne Fenster: --render3d <datei> [yaw] [roll] [stil]
    if let Some(pos) = args.iter().position(|a| a == "--render3d") {
        let out = args
            .get(pos + 1)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("griff3d.png"));
        let yaw: f32 = args
            .get(pos + 2)
            .and_then(|s| s.parse().ok())
            .unwrap_or(32.0);
        let roll: f32 = args
            .get(pos + 3)
            .and_then(|s| s.parse().ok())
            .unwrap_or(-22.0);
        let style = match args.get(pos + 4).map(|s| s.as_str()) {
            Some("bastion") | Some("partial") => HandleStyle::PartialTangBolster,
            Some("einstueck") | Some("one") => HandleStyle::OnePiece,
            Some("zwinge") | Some("ferrule") => HandleStyle::Ferrule,
            _ => HandleStyle::FullTangTwoScale,
        };
        let material_key = args.get(pos + 5).map(|s| s.as_str()).unwrap_or("walnuss");
        let shape = match args.get(pos + 6).map(|s| s.as_str()) {
            Some("oval") => knifeplanner::model::SectionShape::Oval,
            Some("keel") => knifeplanner::model::SectionShape::Keel,
            Some("flach") => knifeplanner::model::SectionShape::Flat,
            _ => knifeplanner::model::SectionShape::Oval,
        };
        let blank = BladeBlank::default();
        let mut params = HandleParams::default();
        params.style = style;
        params.section.shape = shape;
        let plan = DesignPlan::build(&blank, &params, material_key);
        let mat = knifeplanner::materials::get(material_key).unwrap();
        let parts =
            knifeplanner::export::stl::build_scene_parts(&plan.geometry, &blank, &params, mat);

        for part in &parts {
            let mut mn = [f64::MAX; 3];
            let mut mx = [f64::MIN; 3];
            for v in &part.mesh.vertices {
                mn[0] = mn[0].min(v.0);
                mn[1] = mn[1].min(v.1);
                mn[2] = mn[2].min(v.2);
                mx[0] = mx[0].max(v.0);
                mx[1] = mx[1].max(v.1);
                mx[2] = mx[2].max(v.2);
            }
            ausgabe!(
                "{:>14}: X {:7.2}..{:7.2}  Y {:7.2}..{:7.2}  Z {:7.2}..{:7.2}  ({} Dreiecke)",
                part.name,
                mn[0],
                mx[0],
                mn[1],
                mx[1],
                mn[2],
                mx[2],
                part.mesh.faces.len()
            );
        }

        let scene = knifeplanner::render3d::Scene {
            parts: parts
                .into_iter()
                .map(|p| knifeplanner::render3d::Part {
                    mesh: p.mesh,
                    color: p.color,
                    kind: p.kind,
                })
                .collect(),
        };
        let cam = knifeplanner::render3d::Camera {
            yaw,
            roll,
            dist: 0.0,
            fov: 32.0,
        };
        let (w, h) = (1600usize, 1000usize);
        let rgba = knifeplanner::render3d::render(&scene, w, h, &cam, 4);
        knifeplanner::render3d::save_png(&out, &rgba, w, h)?;
        ausgabe!("3D-Render: {}", out.display());
        return Ok(());
    }

    // Gespeicherten Plan laden: --plan <datei>
    if let Some(pos) = args.iter().position(|a| a == "--plan") {
        let path = args
            .get(pos + 1)
            .ok_or_else(|| anyhow::anyhow!("--plan braucht einen Dateipfad"))?;
        let project = knifeplanner::project::Project::load(&PathBuf::from(path))
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        return knifeplanner::ui::run(None, Some(project));
    }

    // Foto direkt oeffnen: --foto <datei>
    let photo = args
        .iter()
        .position(|a| a == "--foto")
        .and_then(|pos| args.get(pos + 1))
        .map(PathBuf::from);

    knifeplanner::ui::run(photo, None)
}

/// Erzeugt einen Beispielplan und schreibt alle Dateien - ohne Fenster.
fn demo() -> anyhow::Result<()> {
    let dir = PathBuf::from("export-demo");
    let blank = BladeBlank::default();
    let params = HandleParams::default();
    let plan = DesignPlan::build(&blank, &params, "walnuss");

    std::fs::create_dir_all(&dir)?;

    let dxf_path = dxf::write_dxf(&dir, &plan.geometry, &blank, &params, &plan.verdict)?;
    ausgabe!("DXF:      {}", dxf_path.display());

    let frame = Frame::from_geometry(&plan.geometry, &blank);
    let svg_path = dir.join("griff.svg");
    std::fs::write(
        &svg_path,
        svg::build_svg(&plan.geometry, &blank, &params, &plan.verdict, frame),
    )?;
    ausgabe!("SVG:      {}", svg_path.display());

    let stl_path = stl::write_stl_file(&dir, &plan.geometry, &blank, &params, &plan.verdict)?;
    ausgabe!("STL:      {}", stl_path.display());

    let (obj_path, mtl_path) = stl::write_obj(&dir, &plan.geometry, &blank, &params, &plan.verdict)?;
    ausgabe!("OBJ:      {}", obj_path.display());
    ausgabe!("MTL:      {}", mtl_path.display());

    let bohrplan = dir.join("bohrplan.pdf");
    plan::build_bohrplan_pdf(&plan).save(&bohrplan)?;
    ausgabe!("Bohrplan: {}", bohrplan.display());

    let bauplan = dir.join("bauplan.pdf");
    plan::build_bauplan_pdf(&plan).save(&bauplan)?;
    ausgabe!("Bauplan:  {}", bauplan.display());

    let schablone = dir.join("schablone_1zu1.pdf");
    plan::build_schablone_pdf(&plan).save(&schablone)?;
    ausgabe!("Schablone: {}", schablone.display());

    let txt = dir.join("bauplan.txt");
    std::fs::write(&txt, plan.report_text())?;
    ausgabe!("Text:     {}", txt.display());

    let png_path = dir.join("vorschau.png");
    preview::render_preview(
        &plan.geometry,
        &blank,
        &params,
        &plan.verdict,
        &png_path,
    )?;
    ausgabe!("Vorschau: {}", png_path.display());

    // Beispielprojekte zum Sofort-Ausprobieren
    let beispiele: Vec<(&str, HandleStyle, &str, f64, f64)> = vec![
        (
            "Jagdmesser_FullTang",
            HandleStyle::FullTangTwoScale,
            "walnuss",
            24.0,
            110.0,
        ),
        (
            "Bastion_Olivenholz",
            HandleStyle::PartialTangBolster,
            "olivenholz",
            26.0,
            125.0,
        ),
        (
            "EinStueck_Knochen",
            HandleStyle::OnePiece,
            "bone",
            22.0,
            100.0,
        ),
    ];
    for (name, style, matkey, grip_h, tang_len) in beispiele {
        let mut proj = knifeplanner::project::Project::default();
        proj.name = name.replace('_', " ");
        proj.material_key = matkey.to_string();
        proj.params.style = style;
        proj.params.grip_height_mm = grip_h;
        proj.params.profile = knifeplanner::model::ProfileStyle::Ergonomic;
        proj.blank.tang_length_mm = tang_len;
        proj.blank.blade_length_mm = 180.0;
        proj.blank.blade_width_mm = 30.0;
        // Die Angel liegt mittig in der Klinge, der Abstand zu Ruecken
        // und Schneide ergibt zusammen die Klingenbreite.
        proj.blank.spine_offset_mm = 15.0;
        proj.blank.edge_offset_mm = 15.0;
        let path = dir.join(format!("beispielprojekt_{name}.json"));
        match proj.save(&path) {
            Ok(()) => ausgabe!("Projekt:  {}", path.display()),
            Err(e) => ausgabe!("Projekt fehlgeschlagen: {e}"),
        }
    }

    // 3D-Render des Beispiels
    let mat = knifeplanner::materials::get("walnuss").unwrap();
    let parts = stl::build_scene_parts(&plan.geometry, &blank, &params, mat);
    let scene = knifeplanner::render3d::Scene {
        parts: parts
            .into_iter()
            .map(|p| knifeplanner::render3d::Part {
                mesh: p.mesh,
                color: p.color,
                kind: p.kind,
            })
            .collect(),
    };
    let cam = knifeplanner::render3d::Camera::default();
    let (w, h) = (1600usize, 1000usize);
    let rgba = knifeplanner::render3d::render(&scene, w, h, &cam, 4);
    let render_path = dir.join("griff3d.png");
    knifeplanner::render3d::save_png(&render_path, &rgba, w, h)?;
    ausgabe!("3D:       {}", render_path.display());

    ausgabe!("\n{}", plan.report_text());
    Ok(())
}
