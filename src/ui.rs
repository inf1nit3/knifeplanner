//! Die native Oberflaeche (egui/eframe).

use std::path::PathBuf;
use std::sync::mpsc::{channel, Receiver, Sender};

use eframe::egui;

use crate::export::{dxf, preview, stl, svg, Frame};
use crate::geometry::recommend_style;
use crate::materials;
use crate::measure::{Calibration, MeasureTarget, Measurement};
use crate::model::{
    BladeBlank, ButtStyle, GripType, HandSize, HandleParams, HandleStyle, ProfileStyle,
    SectionShape, TangStyle,
};
use crate::plan::{self, DesignPlan};
use crate::project::Project;
use crate::render3d::{Camera, Scene};
use crate::vision::{self, ApiConfig, BladeAnalysis, MaterialAnalysis, VisionClient};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Job {
    Blade,
    Material,
}

/// Foto mit seiner Pixelgroesse (fuer das Messwerkzeug).
#[derive(Clone)]
struct Photo {
    tex: egui::TextureHandle,
    size: egui::Vec2,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum MeasureMode {
    /// Referenz kalibrieren
    Calibrate,
    /// Messen
    Measure,
}

struct JobResult {
    job: Job,
    blade: Option<BladeAnalysis>,
    material: Option<MaterialAnalysis>,
    raw: String,
    error: Option<String>,
}

pub struct App {
    blank: BladeBlank,
    params: HandleParams,
    material_key: String,
    material_search: String,

    blade_photo: Option<PathBuf>,
    material_photo: Option<PathBuf>,
    blade_texture: Option<Photo>,
    material_texture: Option<Photo>,

    // Messwerkzeug
    measure_mode: MeasureMode,
    calibration: Option<Calibration>,
    measurements: Vec<Measurement>,
    measure_targets: Vec<MeasureTarget>,
    drag_start: Option<egui::Pos2>,
    drag_now: Option<egui::Pos2>,
    ref_mm: f64,
    photo_zoom: f32,

    // 3D-Ansicht
    scene: Option<Scene>,
    cam: Camera,
    scene_tex: Option<egui::TextureHandle>,
    scene_dirty: bool,
    scene_size: (usize, usize),

    api: ApiConfig,
    api_visible: bool,

    plan: DesignPlan,
    tx: Sender<JobResult>,
    rx: Receiver<JobResult>,
    busy: bool,
    status: String,
    log: String,
    tab: usize,

    export_dir: PathBuf,
    project_name: String,
    project_note: String,
    /// Regler fuer die vom Rohling unabhaengige Grifflaenge
    handle_length_slider: f64,
    /// Position fuer den Querschnitt (mm ab dem Heft)
    section_x: f64,
    /// Maschineneinstellungen (FreeCAD-Pfad)
    settings: crate::settings::Einstellungen,
    /// Meldung des letzten FreeCAD-Laufs
    freecad_meldung: String,
    /// FreeCAD soll nach jeder Aenderung neu rechnen (mit Verzoegerung)
    freecad_pending: bool,
    letzte_aenderung: Option<std::time::Instant>,
    /// Laeuft gerade ein FreeCAD-Durchlauf
    freecad_laeuft: bool,
    /// Der Kanal, ueber den das FreeCAD-Ergebnis zurueckkommt.
    ///
    /// Das Warten passiert im Zeichenloop, nicht hier - so friert die
    /// Oberflaeche waehrend des Rechnens nicht ein.
    freecad_kanal: Option<std::sync::mpsc::Receiver<Result<crate::freecad::Ergebnis, String>>>,
    /// Die beim Start gefundene FreeCAD-Kommandozeile (leer = keine)
    freecad_cmd: Option<PathBuf>,
    /// Die angehaengten Datenblaetter (z. B. vom Klingenstahl)
    datenblaetter: Vec<crate::project::Datenblatt>,
    /// Der Zustand des Update-Vorgangs
    update_zustand: crate::update::Zustand,
    /// Der Kanal, ueber den das Update-Ergebnis zurueckkommt
    update_kanal: Option<std::sync::mpsc::Receiver<crate::update::Zustand>>,
}

impl App {
    pub fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let api = vision::load_config();
        let blank = BladeBlank::default();
        let params = HandleParams::default();
        let plan = DesignPlan::build(&blank, &params, "walnuss");
        let (tx, rx) = channel();
        let export_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join("export");

        let mut app = App {
            blank,
            params,
            material_key: "walnuss".to_string(),
            material_search: String::new(),
            blade_photo: None,
            material_photo: None,
            blade_texture: None,
            material_texture: None,
            measure_mode: MeasureMode::Calibrate,
            calibration: None,
            measurements: Vec::new(),
            measure_targets: Vec::new(),
            drag_start: None,
            drag_now: None,
            ref_mm: 85.6,
            photo_zoom: 1.0,

            scene: None,
            cam: Camera::default(),
            scene_tex: None,
            scene_dirty: true,
            scene_size: (820, 520),
            api,
            api_visible: false,
            plan,
            tx,
            rx,
            busy: false,
            status: "Bereit".to_string(),
            log: String::new(),
            tab: 0,
            export_dir,
            project_name: "Neuer Griff".to_string(),
            project_note: String::new(),
            handle_length_slider: 110.0,
            section_x: 45.0,
            settings: crate::settings::lade(),
            freecad_meldung: String::new(),
            freecad_pending: false,
            letzte_aenderung: None,
            freecad_laeuft: false,
            freecad_kanal: None,
            freecad_cmd: crate::freecad::finde_freecad(&crate::settings::lade().freecad_pfad),
            datenblaetter: Vec::new(),
            update_zustand: crate::update::Zustand::Leer,
            update_kanal: None,
        };
        app.rebuild_scene();
        // Die beim letzten Update ersetzte Datei wegraeumen
        crate::update::raeume_auf();
        // Ist FreeCAD installiert, wird das Modell gleich damit gerechnet -
        // der Nutzer muss nichts einstellen.
        if app.freecad_aktiv() {
            app.freecad_pending = true;
            app.letzte_aenderung = Some(std::time::Instant::now());
            if let Some(p) = &app.freecad_cmd {
                app.status = format!("FreeCAD gefunden ({}). Das Modell wird gerechnet ...", p.display());
            }
        }
        crate::logfile::log(&format!(
            "Start: Rohling {:.0}x{:.0} mm, Angel {:.0} mm, Material {}, Stil {:?}",
            app.blank.blade_length_mm,
            app.blank.blade_width_mm,
            app.blank.tang_length_mm,
            app.material_key,
            app.params.style
        ));
        app
    }

    fn recompute(&mut self) {
        self.params = plan::DesignPlan::adapt_params(&self.blank, &self.params);
        self.plan = DesignPlan::build(&self.blank, &self.params, &self.material_key);
        self.rebuild_scene();
        if self.freecad_aktiv() {
            self.freecad_pending = true;
            self.letzte_aenderung = Some(std::time::Instant::now());
        }
    }

    /// Wird das 3D-Modell mit FreeCAD gerechnet?
    ///
    /// Ohne eigene Einstellung entscheidet die Installation: ist
    /// FreeCAD da, wird es benutzt.
    fn freecad_aktiv(&self) -> bool {
        self.settings
            .freecad_bevorzugen
            .unwrap_or(self.freecad_cmd.is_some())
    }

    /// Baut das 3D-Modell der Baugruppe neu auf.
    fn rebuild_scene(&mut self) {
        let mat = materials::get(&self.material_key).unwrap_or(&materials::MATERIALS[0]);
        let parts = stl::build_scene_parts(&self.plan.geometry, &self.blank, &self.params, mat);
        self.scene = Some(Scene {
            parts: parts
                .into_iter()
                .map(|p| crate::render3d::Part {
                    mesh: p.mesh,
                    color: p.color,
                    kind: p.kind,
                })
                .collect(),
        });
        self.scene_dirty = true;
    }

    fn pick_photo(&mut self, blade: bool, ctx: &egui::Context) {
        let mut dialog = rfd::FileDialog::new();
        dialog = dialog.add_filter("Bilder", &["jpg", "jpeg", "png", "webp", "bmp", "tif", "tiff"]);
        let picked = dialog.pick_file();
        if let Some(path) = picked {
            let tex = load_texture(ctx, &path);
            if blade {
                self.blade_photo = Some(path);
                self.blade_texture = tex;
            } else {
                self.material_photo = Some(path);
                self.material_texture = tex;
            }
            self.status = if blade {
                "Foto des Klingenrohlinges geladen".into()
            } else {
                "Foto des Griffmaterials geladen".into()
            };
        }
    }

    fn start_analysis(&mut self, job: Job) {
        let path = match job {
            Job::Blade => self.blade_photo.clone(),
            Job::Material => self.material_photo.clone(),
        };
        let Some(path) = path else {
            self.status = "Kein Foto ausgewaehlt.".into();
            return;
        };
        let cfg = self.api.clone();
        let tx = self.tx.clone();
        self.busy = true;
        self.status = match job {
            Job::Blade => "Rohling wird analysiert ...".into(),
            Job::Material => "Material wird analysiert ...".into(),
        };
        std::thread::spawn(move || {
            let mut client = VisionClient::new(cfg);
            let result = match job {
                Job::Blade => client.analyze_blade(&path).map(|(a, raw)| JobResult {
                    job,
                    blade: Some(a),
                    material: None,
                    raw,
                    error: None,
                }),
                Job::Material => client.analyze_material(&path).map(|(a, raw)| JobResult {
                    job,
                    blade: None,
                    material: Some(a),
                    raw,
                    error: None,
                }),
            };
            let msg = match result {
                Ok(r) => r,
                Err(e) => JobResult {
                    job,
                    blade: None,
                    material: None,
                    raw: String::new(),
                    error: Some(e.to_string()),
                },
            };
            let _ = tx.send(msg);
        });
    }

    fn poll_jobs(&mut self) {
        while let Ok(res) = self.rx.try_recv() {
            self.busy = false;
            if let Some(err) = res.error {
                self.status = "Analyse fehlgeschlagen".into();
                self.log.push_str(&format!("FEHLER: {err}\n\n"));
                continue;
            }
            match res.job {
                Job::Blade => {
                    if let Some(mut a) = res.blade {
                        a.apply(&mut self.blank);
                        if let Some(style) = a.tang_style.as_ref() {
                            let _ = style;
                        }
                        self.params.style = recommend_style(&self.blank);
                        self.status = format!(
                            "Rohling analysiert (Sicherheit {}). Alle Masse bitte pruefen.",
                            a.confidence
                                .map(|c| format!("{:.0} %", c * 100.0))
                                .unwrap_or_else(|| "unbekannt".into())
                        );
                        self.note("--- Rohlinganalyse ---");
                        self.note(&serde_json::to_string_pretty(&a).unwrap_or_default());
                    }
                }
                Job::Material => {
                    if let Some(a) = res.material {
                        let mut matched: Option<&str> = None;
                        if let Some(list) = &a.suggested_matches {
                            for key in list {
                                if materials::get(key).is_some() {
                                    matched = Some(materials::get(key).unwrap().key);
                                    break;
                                }
                            }
                        }
                        if matched.is_none() {
                            let hints = format!(
                                "{} {}",
                                a.species_guess.clone().unwrap_or_default(),
                                a.category.clone().unwrap_or_default()
                            );
                            matched = materials::suggest_from_vision(&hints, a.species_guess.as_deref())
                                .first()
                                .map(|m| m.key);
                        }
                        if let Some(key) = matched {
                            self.material_key = key.to_string();
                        }
                        self.plan.material_analysis = Some(a.clone());
                        self.status = "Material analysiert".into();
                        self.note("--- Materialanalyse ---");
                        self.note(&serde_json::to_string_pretty(&a).unwrap_or_default());
                    }
                }
            }
            self.note(&format!("Rohantwort: {}", res.raw));
            self.recompute();
        }
    }

    // ---------------------------------------------------------------- Export

    fn export_all(&mut self) {
        let dir = self.export_dir.clone();
        let mut files: Vec<PathBuf> = Vec::new();
        let mut errors: Vec<String> = Vec::new();

        match dxf::write_dxf(&dir, &self.plan.geometry, &self.blank, &self.params, &self.plan.verdict) {
            Ok(p) => files.push(p),
            Err(e) => errors.push(format!("DXF: {e}")),
        }

        let frame = Frame::from_geometry(&self.plan.geometry, &self.blank);
        let svg_text = svg::build_svg(
            &self.plan.geometry,
            &self.blank,
            &self.params,
            &self.plan.verdict,
            frame,
        );
        let svg_path = dir.join(format!(
            "{}.svg",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) = std::fs::create_dir_all(&dir).and_then(|_| std::fs::write(&svg_path, svg_text)) {
            errors.push(format!("SVG: {e}"));
        } else {
            files.push(svg_path);
        }

        match stl::write_stl_file(&dir, &self.plan.geometry, &self.blank, &self.params, &self.plan.verdict)
        {
            Ok(p) => files.push(p),
            Err(e) => errors.push(format!("STL: {e}")),
        }

        // OBJ mit Materialien - damit bleiben Holz, Stahl und Messing farblich
        // getrennt.
        match stl::write_obj(&dir, &self.plan.geometry, &self.blank, &self.params, &self.plan.verdict)
        {
            Ok((o, m)) => {
                files.push(o);
                files.push(m);
            }
            Err(e) => errors.push(format!("OBJ: {e}")),
        }

        // 3D-Render des fertigen Griffs
        if let Some(scene) = &self.scene {
            let (w, h) = (1600usize, 1000usize);
            let rgba = crate::render3d::render(scene, w, h, &self.cam, 4);
            let png3d = dir.join(format!(
                "griff3d_{}.png",
                crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
            ));
            if let Err(e) = crate::render3d::save_png(&png3d, &rgba, w, h) {
                errors.push(format!("3D-Render: {e}"));
            } else {
                files.push(png3d);
            }
        }

        let bohrplan = dir.join(format!(
            "bohrplan_{}.pdf",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) = plan::build_bohrplan_pdf(&self.plan).save(&bohrplan) {
            errors.push(format!("Bohrplan-PDF: {e}"));
        } else {
            files.push(bohrplan);
        }

        // Die Druckschablone im Massstab 1:1 - zum Auflegen auf den
        // Rohling und Durchbohren der Löcher.
        let schablone = dir.join(format!(
            "schablone_1zu1_{}.pdf",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) = plan::build_schablone_pdf(&self.plan).save(&schablone) {
            errors.push(format!("Schablone-PDF: {e}"));
        } else {
            files.push(schablone);
        }

        let bauplan = dir.join(format!(
            "bauplan_{}.pdf",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) =
            plan::build_bauplan_pdf_mit(&self.plan, &self.datenblaetter).save(&bauplan)
        {
            errors.push(format!("Bauplan-PDF: {e}"));
        } else {
            files.push(bauplan);
        }

        let txt_path = dir.join(format!(
            "bauplan_{}.txt",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) = std::fs::create_dir_all(&dir)
            .and_then(|_| {
                std::fs::write(
                    &txt_path,
                    self.plan.report_text_mit(&self.datenblaetter),
                )
            })
        {
            errors.push(format!("Text: {e}"));
        } else {
            files.push(txt_path);
        }

        let png_path = dir.join(format!(
            "vorschau_{}.png",
            crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict)
        ));
        if let Err(e) = preview::render_preview(
            &self.plan.geometry,
            &self.blank,
            &self.params,
            &self.plan.verdict,
            &png_path,
        ) {
            errors.push(format!("Vorschau: {e}"));
        } else {
            files.push(png_path);
        }

        // Die Datenblaetter wandern mit in den Zielordner - so liegen
        // die Werkstattunterlagen (z. B. das Datenblatt des Klingenstahls)
        // vollstaendig bei den Plänen.
        if !self.datenblaetter.is_empty() {
            let ziel = dir.join("datenblaetter");
            if let Err(e) = std::fs::create_dir_all(&ziel) {
                errors.push(format!("Datenblaetter: {e}"));
            } else {
                for d in &self.datenblaetter {
                    let quelle = d.pfad();
                    let ziel_pfad = ziel.join(d.datei_name());
                    match std::fs::copy(&quelle, &ziel_pfad) {
                        Ok(_) => files.push(ziel_pfad),
                        Err(e) => errors.push(format!("Datenblatt {}: {e}", d.name)),
                    }
                }
            }
        }

        self.status = if errors.is_empty() {
            format!("{} Dateien exportiert nach {}", files.len(), dir.display())
        } else {
            format!("Export mit {} Fehlern", errors.len())
        };
        for f in &files {
            self.note(&format!("exportiert: {}", f.display()));
        }
        for e in &errors {
            self.note(&format!("FEHLER bei Export: {e}"));
        }
    }

    // ----------------------------------------------------------------- UI

    fn photo_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("1 - Fotos")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if ui.button("Klingenrohling wählen").clicked() {
                        let ctx = ui.ctx().clone();
                        self.pick_photo(true, &ctx);
                    }
                    if ui.button("Griffmaterial wählen").clicked() {
                        let ctx = ui.ctx().clone();
                        self.pick_photo(false, &ctx);
                    }
                });

                ui.horizontal(|ui| {
                    if let Some(photo) = &self.blade_texture {
                        let sized =
                            egui::load::SizedTexture::new(photo.tex.id(), egui::vec2(110.0, 74.0));
                        ui.image(sized);
                    }
                    if let Some(photo) = &self.material_texture {
                        let sized =
                            egui::load::SizedTexture::new(photo.tex.id(), egui::vec2(110.0, 74.0));
                        ui.image(sized);
                    }
                });

                let can_analyze = self.api.is_configured() || vision::local_oauth_token().is_some();
                ui.horizontal(|ui| {
                    if ui
                        .add_enabled(
                            self.blade_photo.is_some() && !self.busy,
                            egui::Button::new("Rohling analysieren"),
                        )
                        .clicked()
                    {
                        self.start_analysis(Job::Blade);
                    }
                    if ui
                        .add_enabled(
                            self.material_photo.is_some() && !self.busy,
                            egui::Button::new("Material analysieren"),
                        )
                        .clicked()
                    {
                        self.start_analysis(Job::Material);
                    }
                });
                if !can_analyze {
                    ui.colored_label(
                        egui::Color32::from_rgb(180, 120, 30),
                        "Kein API-Schlüssel - Einstellungen unten öffnen",
                    );
                }
                if self.busy {
                    ui.spinner();
                    ui.label("Analyse läuft ...");
                }
            });
    }

    fn blank_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("2 - Rohling")
            .default_open(true)
            .show(ui, |ui| {
                let mut changed = false;
                changed |= drag_row(ui, "Klingenlänge", &mut self.blank.blade_length_mm, 40.0..=400.0);
                changed |= drag_row(ui, "Klingenbreite", &mut self.blank.blade_width_mm, 10.0..=60.0);
                changed |= drag_row(ui, "Klingendicke", &mut self.blank.blade_thickness_mm, 1.0..=8.0);
                ui.separator();
                changed |= drag_row(ui, "Angel-Länge", &mut self.blank.tang_length_mm, 40.0..=220.0);
                changed |= drag_row(ui, "Angel-Breite", &mut self.blank.tang_width_mm, 1.0..=20.0);
                changed |= drag_row(ui, "Angel-Dicke", &mut self.blank.tang_thickness_mm, 1.0..=20.0);

                egui::ComboBox::from_id_salt("tang_style")
                    .selected_text(self.blank.tang_style.label())
                    .width(200.0)
                    .show_ui(ui, |ui| {
                        for st in TangStyle::all() {
                            changed |= ui
                                .selectable_value(&mut self.blank.tang_style, st, st.label())
                                .changed();
                        }
                    });

                ui.horizontal(|ui| {
                    changed |= ui.checkbox(&mut self.blank.tang_taper, "Angel verjüngt").changed();
                    changed |= ui
                        .checkbox(&mut self.blank.has_bolster, "Bastion am Rohling")
                        .changed();
                    changed |= ui
                        .checkbox(&mut self.blank.has_guard, "Paradeschärke")
                        .changed();
                });

                ui.separator();
                changed |= drag_row(ui, "Rückenabstand", &mut self.blank.spine_offset_mm, 2.0..=25.0);
                changed |= drag_row(ui, "Schneidenabstand", &mut self.blank.edge_offset_mm, 2.0..=25.0);

                if changed {
                    self.recompute();
                }
                if ui.button("Empfohlene Bauart übernehmen").clicked() {
                    self.params.style = recommend_style(&self.blank);
                    self.recompute();
                }
            });
    }

    fn handle_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("3 - Griff")
            .default_open(true)
            .show(ui, |ui| {
                let mut changed = false;
                egui::ComboBox::from_id_salt("handle_style")
                    .selected_text(self.params.style.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for st in HandleStyle::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.style, st, st.label())
                                .changed();
                        }
                    });

                egui::ComboBox::from_id_salt("grip")
                    .selected_text(self.params.grip.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for g in GripType::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.grip, g, g.label())
                                .changed();
                        }
                    });

                egui::ComboBox::from_id_salt("hand")
                    .selected_text(self.params.hand_size.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for h in HandSize::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.hand_size, h, h.label())
                                .changed();
                        }
                    });
                if ui.button("Griffstärke nach Handgröße übernehmen").clicked() {
                    let ergo = crate::geometry::ergonomics_for(
                        &self.blank,
                        &self.params,
                        &self.plan.geometry,
                    );
                    self.params.grip_height_mm = ergo.grip_height_mm;
                    self.status = format!(
                        "Griffhöhe auf {:.0} mm gesetzt (Umfang-Ziel {:.0} mm).",
                        ergo.grip_height_mm, self.params.hand_size.target_circumference()
                    );
                    changed = true;
                }

                egui::ComboBox::from_id_salt("profile")
                    .selected_text(self.params.profile.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for pr in ProfileStyle::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.profile, pr, pr.label())
                                .changed();
                        }
                    });

                if self.params.profile == ProfileStyle::Ergonomic {
                    changed |= drag_row(ui, "Griffhöhe", &mut self.params.grip_height_mm, 14.0..=40.0);
                    changed |= drag_row(
                        ui,
                        "Fingerkehle",
                        &mut self.params.finger_groove_mm,
                        0.0..=6.0,
                    );
                    changed |= drag_row(ui, "Ballenauflage", &mut self.params.belly_drop_mm, 0.0..=12.0);
                } else {
                    changed |= drag_row(ui, "Rückenwölbung", &mut self.params.spine_camber_mm, -3.0..=8.0);
                    changed |= drag_row(ui, "Bauchwölbung", &mut self.params.belly_drop_mm, 0.0..=10.0);
                }

                changed |= drag_row(ui, "Gesamtdicke", &mut self.params.target_thickness_mm, 12.0..=40.0);
                changed |= drag_row(ui, "Mindestwand", &mut self.params.wall_min_mm, 0.8..=6.0);
                changed |= drag_row(ui, "Klebespalt", &mut self.params.glue_gap_mm, 0.0..=0.15);
                changed |= drag_row(ui, "Kantenabrundung", &mut self.params.edge_break_mm, 0.0..=3.0);
                changed |= drag_row(
                    ui,
                    "Grifflänge",
                    &mut self.handle_length_slider,
                    60.0..=190.0,
                );
                self.params.handle_length_mm = if self.handle_length_slider + 0.5
                    < self.blank.tang_length_mm
                {
                    None
                } else {
                    Some(self.handle_length_slider)
                };

                ui.separator();
                ui.label("Querschnitt");
                egui::ComboBox::from_id_salt("section_shape")
                    .selected_text(self.params.section.shape.label())
                    .width(240.0)
                    .show_ui(ui, |ui| {
                        for sh in SectionShape::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.section.shape, sh, sh.label())
                                .changed();
                        }
                    });
                changed |= drag_row(
                    ui,
                    "Rückenrundung",
                    &mut self.params.section.spine_round_mm,
                    0.0..=10.0,
                );
                changed |= drag_row(
                    ui,
                    "Bauchrundung",
                    &mut self.params.section.belly_round_mm,
                    0.0..=10.0,
                );
                if self.params.style == crate::model::HandleStyle::OnePiece
                    || self.params.style == crate::model::HandleStyle::Ferrule
                {
                    changed |= drag_row(
                        ui,
                        "Längsgrat",
                        &mut self.params.section.keel_mm,
                        0.0..=6.0,
                    );
                }
                changed |= drag_row(
                    ui,
                    "Dickeverjüngung",
                    &mut self.params.section.taper_mm,
                    0.0..=12.0,
                );

                egui::ComboBox::from_id_salt("butt")
                    .selected_text(self.params.butt_style.label())
                    .width(160.0)
                    .show_ui(ui, |ui| {
                        for b in ButtStyle::all() {
                            changed |= ui
                                .selectable_value(&mut self.params.butt_style, b, b.label())
                                .changed();
                        }
                    });

                if self.params.style == HandleStyle::PartialTangBolster {
                    changed |= drag_row(ui, "Bastionsbreite", &mut self.params.bolster_width_mm, 8.0..=45.0);
                }
                if self.params.style == HandleStyle::Ferrule {
                    changed |= drag_row(ui, "Zwingenbreite", &mut self.params.ferrule_width_mm, 5.0..=35.0);
                }

                ui.horizontal(|ui| {
                    changed |= ui
                        .checkbox(&mut self.params.lanyard_hole, "Lanyardloch")
                        .changed();
                });
                if self.params.lanyard_hole {
                    changed |= drag_row(
                        ui,
                        "Lanyardloch Ø",
                        &mut self.params.lanyard_hole_dia_mm,
                        3.0..=10.0,
                    );
                }

                if changed {
                    self.recompute();
                }
            });
    }

    fn material_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("4 - Material")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Suche");
                    ui.text_edit_singleline(&mut self.material_search);
                });
                let hits = if self.material_search.trim().is_empty() {
                    materials::MATERIALS.iter().collect::<Vec<_>>()
                } else {
                    materials::search(&self.material_search)
                };
                egui::ComboBox::from_id_salt("material")
                    .selected_text(
                        materials::get(&self.material_key)
                            .map(|m| m.label())
                            .unwrap_or_else(|| self.material_key.clone()),
                    )
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        for m in hits.iter().take(40) {
                            if ui
                                .selectable_value(&mut self.material_key, m.key.to_string(), m.label())
                                .clicked()
                            {
                                self.recompute();
                            }
                        }
                    });
                let v = &self.plan.verdict;
                ui.add_space(4.0);
                ui.horizontal(|ui| {
                    ui.label("Eignung:");
                    let color = match v.score {
                        85..=100 => egui::Color32::from_rgb(40, 140, 60),
                        70..=84 => egui::Color32::from_rgb(90, 140, 40),
                        55..=69 => egui::Color32::from_rgb(190, 140, 30),
                        _ => egui::Color32::from_rgb(190, 60, 50),
                    };
                    ui.colored_label(color, format!("{} ({}/100)", v.rating, v.score));
                });
                ui.small(format!("Klebstoff: {}", v.adhesive));
                let ergo = crate::geometry::ergonomics_for(&self.blank, &self.params, &self.plan.geometry);
                ui.separator();
                ui.label(egui::RichText::new("Ergonomie").strong());
                ui.small(&ergo.note);
                for w in &ergo.warnings {
                    ui.colored_label(egui::Color32::from_rgb(200, 140, 40), format!("  ! {w}"));
                }
            });
    }

    fn export_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("5 - Export")
            .default_open(true)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    ui.label("Projektname");
                    ui.text_edit_singleline(&mut self.project_name);
                });
                ui.horizontal(|ui| {
                    ui.label("Zielordner");
                    let mut text = self.export_dir.display().to_string();
                    if ui.text_edit_singleline(&mut text).changed() {
                        self.export_dir = PathBuf::from(text);
                    }
                });
                if ui.button("Alle Dateien exportieren").clicked() {
                    self.export_all();
                }
                ui.horizontal(|ui| {
                    if ui.button("Plan speichern").clicked() {
                        self.save_project();
                    }
                    if ui.button("Plan laden").clicked() {
                        self.load_project();
                    }
                });
                ui.small("DXF (Schalen), SVG, STL, OBJ/MTL, 3D-PNG, Bohrplan-PDF, Bauplan-PDF, Text");
                let (z_laenge, z_hoehe, z_dicke) = crate::geometry::zuschnitt(
                    &self.plan.geometry.outline,
                    &self.params,
                    self.plan.geometry.scale_thickness_mm,
                    self.plan.geometry.total_thickness_mm,
                );
                ui.small(format!(
                    "Zuschnitt: Holzleiste {:.0} x {:.0} x {:.0} mm",
                    z_laenge, z_hoehe, z_dicke
                ));

                // Datenblaetter anhaengen - typisch das Lieferdatenblatt
                // des Klingenstahls. Sie wandern beim Export mit in den
                // Zielordner.
                ui.separator();
                ui.label("Datenblätter");
                let mut entfernen: Option<usize> = None;
                for (i, d) in self.datenblaetter.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut d.name)
                                .desired_width(200.0)
                                .hint_text("Bezeichnung"),
                        );
                        if ui.small_button("Öffnen").clicked() {
                            let p = d.pfad();
                            if let Err(e) = crate::settings::ordner_oeffnen(&p) {
                                self.status = format!("Datei liess sich nicht oeffnen: {e}");
                            }
                        }
                        if ui.small_button("Entfernen").clicked() {
                            entfernen = Some(i);
                        }
                    });
                    ui.small(format!("  {}", d.pfad));
                }
                if let Some(i) = entfernen {
                    self.datenblaetter.remove(i);
                }
                ui.horizontal(|ui| {
                    if ui.button("Datenblatt hinzufügen").clicked() {
                        self.datenblatt_hinzufuegen();
                    }
                    ui.small("PDF, Bild oder Text - z. B. das Datenblatt des Klingenstahls");
                });

                // Wo das FreeCAD-Modell liegt
                let fc_ziel = self.export_dir.join("freecad");
                ui.horizontal(|ui| {
                    ui.small(format!("FreeCAD-Modell (STL + STEP): {}", fc_ziel.display()));
                    if ui.small_button("Ordner öffnen").clicked() {
                        let _ = std::fs::create_dir_all(&fc_ziel);
                        if let Err(e) = crate::settings::ordner_oeffnen(&fc_ziel) {
                            self.status = e;
                        }
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("Mit FreeCAD neu bauen").clicked() {
                        self.bau_mit_freecad();
                    }
                    if ui.small_button("Exportordner öffnen").clicked() {
                        let _ = std::fs::create_dir_all(&self.export_dir);
                        if let Err(e) = crate::settings::ordner_oeffnen(&self.export_dir) {
                            self.status = e;
                        }
                    }
                });
            });
    }

    /// Der Bereich, in dem nach einer neuen Version gesucht wird.
    fn update_section(&mut self, ui: &mut egui::Ui) {
        ui.label("Update");
        ui.horizontal(|ui| {
            if ui.button("Nach Updates suchen").clicked() {
                self.update_pruefen();
            }
            ui.small(format!(
                "Installiert: Version {}",
                crate::update::eigene_version()
            ));
        });
        match self.update_zustand.clone() {
            crate::update::Zustand::Leer => {}
            crate::update::Zustand::Prueft => {
                ui.small("Suche auf GitHub ...");
            }
            crate::update::Zustand::Aktuell => {
                ui.small("Sie haben die neueste Version.");
            }
            crate::update::Zustand::Neu(u) => {
                ui.label(
                    egui::RichText::new(format!("Version {} ist da", u.version))
                        .color(egui::Color32::from_rgb(40, 140, 60)),
                );
                if !u.notiz.is_empty() {
                    ui.small(u.notiz.clone());
                }
                if ui.button("Herunterladen und einsetzen").clicked() {
                    self.update_laden(&u);
                }
            }
            crate::update::Zustand::Laedt {
                version,
                geladen,
                gesamt,
            } => {
                let anteil: f32 = if gesamt > 0 {
                    (geladen as f64 / gesamt as f64) as f32
                } else {
                    0.0
                };
                ui.add(egui::ProgressBar::new(anteil.clamp(0.0, 1.0)).show_percentage());
                ui.small(format!(
                    "lade Version {version} ... {:.1} von {:.1} MB",
                    geladen as f64 / 1_048_576.0,
                    gesamt as f64 / 1_048_576.0
                ));
            }
            crate::update::Zustand::Fertig { version, datei } => {
                ui.label(
                    egui::RichText::new(format!("Version {version} ist heruntergeladen"))
                        .color(egui::Color32::from_rgb(40, 140, 60)),
                );
                if ui.button("Jetzt einsetzen und beenden").clicked() {
                    self.update_einsetzen(&datei);
                }
                ui.small(format!("Die Datei liegt in {}", datei.display()));
            }
            crate::update::Zustand::Fehler(e) => {
                ui.colored_label(egui::Color32::from_rgb(160, 40, 40), e);
            }
        }
    }

    /// Sucht auf GitHub nach einer neuen Version.
    fn update_pruefen(&mut self) {
        self.update_zustand = crate::update::Zustand::Prueft;
        let (tx, rx) = std::sync::mpsc::channel::<crate::update::Zustand>();
        std::thread::spawn(move || {
            let zustand = match crate::update::pruefe(crate::update::REPO) {
                Ok(Some(u)) => crate::update::Zustand::Neu(Box::new(u)),
                Ok(None) => crate::update::Zustand::Aktuell,
                Err(e) => crate::update::Zustand::Fehler(e),
            };
            let _ = tx.send(zustand);
        });
        self.update_kanal = Some(rx);
    }

    /// Laedt die neue Programmdatei herunter.
    fn update_laden(&mut self, u: &crate::update::Update) {
        let ziel = crate::update::temp_datei(&u.version);
        self.update_zustand = crate::update::Zustand::Laedt {
            version: u.version.clone(),
            geladen: 0,
            gesamt: u.groesse,
        };
        let (tx, rx) = std::sync::mpsc::channel::<crate::update::Zustand>();
        let update = u.clone();
        std::thread::spawn(move || {
            let version = update.version.clone();
            let ziel = ziel.clone();
            let tx2 = tx.clone();
            let zustand = match crate::update::lade(&update, &ziel, &mut |geladen, gesamt| {
                let _ = tx2.send(crate::update::Zustand::Laedt {
                    version: version.clone(),
                    geladen,
                    gesamt,
                });
            }) {
                Ok(()) => crate::update::Zustand::Fertig {
                    version,
                    datei: ziel,
                },
                Err(e) => crate::update::Zustand::Fehler(e),
            };
            let _ = tx.send(zustand);
        });
        self.update_kanal = Some(rx);
    }

    /// Setzt die neue Programmdatei an die Stelle der alten.
    fn update_einsetzen(&mut self, datei: &std::path::Path) {
        let ziel = match std::env::current_exe() {
            Ok(p) => p,
            Err(e) => {
                self.update_zustand =
                    crate::update::Zustand::Fehler(format!("Programmpfad: {e}"));
                return;
            }
        };
        match crate::update::setze_ein(datei, &ziel) {
            Ok(()) => {
                self.status = "Die neue Version ist eingesetzt. Bitte das Programm neu starten."
                    .into();
                self.note("Update eingesetzt - bitte neu starten");
                self.update_zustand = crate::update::Zustand::Leer;
                // Der alte Aufruf beendet sich nach dem Einsetzen
                std::thread::sleep(std::time::Duration::from_millis(400));
                std::process::exit(0);
            }
            Err(e) => {
                self.update_zustand = crate::update::Zustand::Fehler(e);
            }
        }
    }

    fn api_section(&mut self, ui: &mut egui::Ui) {
        egui::CollapsingHeader::new("Einstellungen / API")
            .default_open(false)
            .show(ui, |ui| {
                self.update_section(ui);
                ui.separator();
                ui.horizontal(|ui| {
                    if ui.button("API-Einstellungen").clicked() {
                        self.api_visible = !self.api_visible;
                    }
                    if ui.button("Schlüssel speichern").clicked() {
                        match vision::save_config(&self.api) {
                            Ok(_) => self.status = "API-Schlüssel gespeichert".into(),
                            Err(e) => self.status = format!("Speichern fehlgeschlagen: {e}"),
                        }
                    }
                });
                if self.api_visible {
                    egui::Grid::new("api_grid").num_columns(1).show(ui, |ui| {
                        ui.label("API-Schlüssel");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.api.api_key)
                                .password(true)
                                .desired_width(280.0),
                        );
                        ui.end_row();
                        // Der Schlüssel steht im Klartext in der Datei - das
                        // muss der Nutzer wissen, bevor er ihn speichert.
                        ui.small(format!(
                            "Wird unverschlüsselt abgelegt in: {}",
                            vision::config_path().display()
                        ));
                        ui.small(
                            "Alternativ: Umgebungsvariable MINIMAX_API_KEY setzen, \
                             dann wird nichts gespeichert.",
                        );
                        ui.end_row();
                        ui.label("Basis-URL");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.api.base_url)
                                .desired_width(280.0),
                        );
                        ui.end_row();
                        ui.label("Modell");
                        let current = self.api.model.clone();
                        let known = vision::KNOWN_MODELS
                            .iter()
                            .any(|(id, _)| *id == current);
                        egui::ComboBox::from_id_salt("model")
                            .selected_text(if known {
                                current.clone()
                            } else {
                                format!("{current} (eigener Name)")
                            })
                            .width(280.0)
                            .show_ui(ui, |ui| {
                                for (id, desc) in vision::KNOWN_MODELS {
                                    ui.selectable_value(
                                        &mut self.api.model,
                                        id.to_string(),
                                        *desc,
                                    );
                                }
                            });
                        if !known {
                            ui.add(
                                egui::TextEdit::singleline(&mut self.api.model)
                                    .desired_width(280.0),
                            );
                        }
                        ui.end_row();
                    });
                    ui.small(format!(
                        "Gespeichert unter: {}",
                        vision::config_path().display()
                    ));
                    ui.separator();
                    ui.label("FreeCAD");
                    ui.horizontal(|ui| {
                        ui.label("Pfad");
                        ui.add(
                            egui::TextEdit::singleline(&mut self.settings.freecad_pfad)
                                .desired_width(200.0)
                                .hint_text("z. B. C:\\Program Files\\FreeCAD 1.0\\bin"),
                        );
                        if ui.button("Suchen").clicked() {
                            match crate::freecad::finde_freecad(&self.settings.freecad_pfad) {
                                Some(p) => {
                                    self.settings.freecad_pfad = p.to_string_lossy().to_string();
                                    self.freecad_cmd = Some(p.clone());
                                    self.status = format!("FreeCAD gefunden: {}", p.display());
                                }
                                None => {
                                    self.status =
                                        "FreeCAD nicht gefunden - Pfad von Hand eintragen.".into();
                                }
                            }
                        }
                        if ui.button("Speichern").clicked() {
                            match crate::settings::speichern(&self.settings) {
                                Ok(()) => self.status = "Einstellungen gespeichert".into(),
                                Err(e) => self.status = format!("Speichern fehlgeschlagen: {e}"),
                            }
                        }
                    });
                    // FreeCAD wird automatisch benutzt, wenn es installiert
                    // ist. Wer das anders will, stellt es hier ab - die
                    // Entscheidung wird gemerkt.
                    let mut aktiv = self.freecad_aktiv();
                    if ui
                        .checkbox(
                            &mut aktiv,
                            "3D-Modell bevorzugt mit FreeCAD bauen",
                        )
                        .changed()
                    {
                        self.settings.freecad_bevorzugen = Some(aktiv);
                        let _ = crate::settings::speichern(&self.settings);
                    }
                    match &self.freecad_cmd {
                        Some(p) => {
                            ui.small(format!(
                                "FreeCAD gefunden ({}). {}",
                                p.display(),
                                if aktiv {
                                    "Das Modell wird damit gerechnet."
                                } else {
                                    "Zurzeit wird das eigene Modell genutzt."
                                }
                            ));
                        }
                        None => {
                            ui.small(
                                "FreeCAD ist nicht installiert - das Programm nutzt sein eigenes \
                                 Modell. Pfad eintragen oder den Knopf *Suchen* benutzen.",
                            );
                        }
                    }

                    if self.api.model.contains("Flash") {
                        ui.colored_label(
                            egui::Color32::from_rgb(190, 120, 40),
                            "M3.1-Flash denkt vor der Antwort sehr lange (rund 15-40 Sekunden \
                             und viele Token). Für schnellere Ergebnisse MiniMax-M3 wählen.",
                        );
                    }
                }
            });
    }
}

fn drag_row(ui: &mut egui::Ui, label: &str, v: &mut f64, range: std::ops::RangeInclusive<f64>) -> bool {
    let mut changed = false;
    ui.horizontal(|ui| {
        ui.label(label);
        let before = *v;
        ui.add(egui::Slider::new(v, range).suffix(" mm"));
        changed = before != *v;
    });
    changed
}

/// Skaliert eine Textur seitenverhaeltnisgleich in ein Rechteck ein.
fn fit_rect(size: egui::Vec2, rect: egui::Rect) -> egui::Rect {
    if size.x <= 0.0 || size.y <= 0.0 {
        return rect;
    }
    let scale = (rect.width() / size.x).min(rect.height() / size.y);
    egui::Rect::from_center_size(
        rect.center(),
        egui::vec2(size.x * scale, size.y * scale),
    )
}

fn _clamp_f(v: f32, lo: f32, hi: f32) -> f32 {
    v.max(lo).min(hi)
}

fn load_texture(ctx: &egui::Context, path: &std::path::Path) -> Option<Photo> {
    let img = image::open(path).ok()?;
    let size = [img.width() as usize, img.height() as usize];
    let (w, h) = (img.width() as f32, img.height() as f32);
    let img = img.into_rgba8();
    let pixels: Vec<u8> = img.into_raw();
    let color = egui::ColorImage::from_rgba_unmultiplied(size, &pixels);
    let tex = ctx.load_texture(
        format!("tex_{}", path.display()),
        color,
        egui::TextureOptions::LINEAR,
    );
    Some(Photo {
        tex,
        size: egui::vec2(w, h),
    })
}

impl eframe::App for App {
    fn logic(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.poll_jobs();
        if self.busy {
            ctx.request_repaint();
        }

        // Das Ergebnis eines laufenden Update-Vorgangs einsammeln
        let update_fertig = match &self.update_kanal {
            Some(rx) => match rx.try_recv() {
                Ok(z) => Some(z),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(crate::update::Zustand::Fehler(
                        "Die Verbindung zum Update ist abgerissen".into(),
                    ))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            },
            None => None,
        };
        if let Some(z) = update_fertig {
            self.update_kanal = None;
            self.update_zustand = z;
        }

        // Das Ergebnis eines laufenden FreeCAD-Durchlaufs einsammeln.
        // Das Warten passiert hier, nicht im Thread - so bleibt die
        // Oberflaeche bedienbar, waehrend FreeCAD rechnet.
        let fertig = match &self.freecad_kanal {
            Some(rx) => match rx.try_recv() {
                Ok(r) => Some(r),
                Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    Some(Err("FreeCAD-Antwort verloren".into()))
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => None,
            },
            None => None,
        };
        if let Some(r) = fertig {
            self.freecad_kanal = None;
            self.freecad_laeuft = false;
            self.freecad_ergebnis(r);
        }
        if self.freecad_laeuft {
            ctx.request_repaint();
        }

        // Nach kurzer Pause neu rechnen, damit das Ziehen eines Reglers
        // nicht jedes Mal einen FreeCAD-Lauf ausloest.
        if self.freecad_pending && !self.freecad_laeuft {
            let warte = self
                .letzte_aenderung
                .map(|t| t.elapsed().as_millis() > 1500)
                .unwrap_or(true);
            if warte {
                self.freecad_pending = false;
                self.bau_mit_freecad();
            }
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        egui::Panel::top("header").show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.heading("KnifePlanner");
                ui.label("· Griff-Planer für Messerdesigns");
                ui.separator();
                ui.label(&self.status);
            });
        });

        egui::Panel::left("params")
            .default_size(340.0)
            .resizable(true)
            .show(ui, |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    self.photo_section(ui);
                    ui.add_space(4.0);
                    self.blank_section(ui);
                    ui.add_space(4.0);
                    self.handle_section(ui);
                    ui.add_space(4.0);
                    self.material_section(ui);
                    ui.add_space(4.0);
                    self.export_section(ui);
                    ui.add_space(4.0);
                    self.api_section(ui);
                });
            });

        egui::CentralPanel::default_margins().show(ui, |ui| {
            ui.horizontal(|ui| {
                for (i, name) in [
                    "3D",
                    "Schnitt",
                    "Vorschau",
                    "Messen",
                    "Bohrplan",
                    "Material",
                    "Bauplan",
                    "Log",
                ]
                .iter()
                .enumerate()
                {
                    if ui.selectable_label(self.tab == i, *name).clicked() {
                        self.tab = i;
                    }
                }
            });
            ui.separator();

            match self.tab {
                0 => self.three_d_tab(ui),
                1 => self.section_tab(ui),
                2 => self.preview_tab(ui),
                3 => self.measure_tab(ui),
                4 => self.drill_tab(ui),
                5 => self.material_tab(ui),
                6 => self.build_tab(ui),
                _ => {
                    egui::ScrollArea::vertical().show(ui, |ui| {
                        ui.monospace(&self.log);
                        ui.separator();
                        ui.small(format!(
                            "Protokolldatei: {}",
                            crate::logfile::log_path().display()
                        ));
                    });
                }
            }
        });
    }
}

impl App {
    /// 3D-Ansicht: Modell in alle Richtungen drehbar.
    fn three_d_tab(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if ui.button("Ansicht zurücksetzen").clicked() {
                self.cam.reset();
                self.scene_dirty = true;
            }
            if ui.button("PNG speichern (hochaufgelöst)").clicked() {
                self.save_scene_png();
            }
            if ui.button("Mit FreeCAD neu bauen").clicked() {
                self.bau_mit_freecad();
            }
            ui.separator();
            ui.label(format!(
                "Drehung {:.0}° / {:.0}°  ·  Ziehen zum Drehen, Mausrad zum Zoomen",
                self.cam.yaw, self.cam.roll
            ));
        });

        let size = egui::vec2(ui.available_width(), (ui.available_height() - 34.0).clamp(240.0, 900.0));
        let (rect, resp) = ui.allocate_exact_size(size, egui::Sense::click_and_drag());

        // Szene bei Bedarf neu rendern
        if self.scene_dirty {
            if let Some(scene) = &self.scene {
                let (w, h) = self.scene_size;
                let rgba = crate::render3d::render(scene, w, h, &self.cam, 1);
                let img = egui::ColorImage::from_rgba_unmultiplied([w, h], &rgba);
                self.scene_tex = Some(ui.ctx().load_texture(
                    "griff3d",
                    img,
                    egui::TextureOptions::LINEAR,
                ));
                self.scene_dirty = false;
            }
        }

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(236, 234, 228));
        if let Some(tex) = &self.scene_tex {
            let src_size = egui::vec2(self.scene_size.0 as f32, self.scene_size.1 as f32);
            let fitted = fit_rect(src_size, rect);
            painter.image(
                tex.id(),
                fitted,
                egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
                egui::Color32::WHITE,
            );
        }

        // Drehen und Zoomen
        if resp.dragged() {
            let d = resp.drag_delta();
            self.cam.yaw = (self.cam.yaw + d.x * 0.45).rem_euclid(360.0);
            self.cam.roll = (self.cam.roll + d.y * 0.45).rem_euclid(360.0);
            self.scene_dirty = true;
        }
        if resp.hovered() {
            let scroll = ui.input(|i| i.smooth_scroll_delta.y);
            if scroll.abs() > 0.01 {
                let f = 1.0 + scroll * 0.0015;
                self.cam.dist = (_clamp_f(self.cam.dist, 60.0, 900.0) * f).clamp(60.0, 900.0);
                self.scene_dirty = true;
            }
        }
        if resp.double_clicked() {
            self.cam.reset();
            self.scene_dirty = true;
        }

        ui.add_space(4.0);
        ui.horizontal(|ui| {
            let tris: usize = self
                .scene
                .as_ref()
                .map(|s| s.parts.iter().map(|p| p.mesh.faces.len()).sum())
                .unwrap_or(0);
            ui.small(format!(
                "{} Teile, {} Dreiecke · {}",
                self.scene.as_ref().map(|s| s.parts.len()).unwrap_or(0),
                tris,
                self.plan.verdict.material_name
            ));
        });
    }

    /// Schreibt eine Zeile in die Protokolldatei.
    fn note(&mut self, text: &str) {
        self.log.push_str(text);
        self.log.push('\n');
        crate::logfile::log_with_time(text);
    }

    /// Haengt ein Datenblatt an (z. B. das Datenblatt des Klingenstahls).
    fn datenblatt_hinzufuegen(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        dialog = dialog.add_filter(
            "Datenblatt",
            &["pdf", "png", "jpg", "jpeg", "txt", "md", "csv", "html"],
        );
        if let Some(path) = dialog.pick_file() {
            // Der Name des Files ist die Vorgabe fuer die Bezeichnung
            let name = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "Datenblatt".to_string());
            self.datenblaetter.push(crate::project::Datenblatt {
                name,
                pfad: path.to_string_lossy().to_string(),
            });
            self.status = format!("Datenblatt angehängt: {}", path.display());
        }
    }

    /// Speichert die Planung als JSON-Datei.
    fn save_project(&mut self) {
        let project = Project {
            version: 1,
            name: self.project_name.clone(),
            blank: self.blank.clone(),
            params: self.params.clone(),
            material_key: self.material_key.clone(),
            view: crate::project::ViewState {
                yaw: self.cam.yaw,
                roll: self.cam.roll,
                dist: self.cam.dist,
            },
            note: self.project_note.clone(),
            datenblaetter: self.datenblaetter.clone(),
        };
        let mut dialog = rfd::FileDialog::new();
        dialog = dialog.set_file_name(&format!("{}.json", project.file_stem()));
        dialog = dialog.add_filter("KnifePlanner-Plan", &["json"]);
        if let Some(path) = dialog.save_file() {
            match project.save(&path) {
                Ok(()) => {
                    self.status = format!("Plan gespeichert: {}", path.display());
                    self.note(&format!("Plan gespeichert: {}", path.display()));
                }
                Err(e) => self.status = format!("Speichern fehlgeschlagen: {e}"),
            }
        }
    }

    /// Laedt eine Planung aus einer JSON-Datei.
    fn load_project(&mut self) {
        let mut dialog = rfd::FileDialog::new();
        dialog = dialog.add_filter("KnifePlanner-Plan", &["json"]);
        if let Some(path) = dialog.pick_file() {
            match Project::load(&path) {
                Ok(p) => {
                    self.blank = p.blank;
                    self.params = p.params;
                    self.material_key = p.material_key;
                    self.project_name = p.name;
                    self.project_note = p.note;
                    self.datenblaetter = p.datenblaetter;
                    self.cam.yaw = p.view.yaw;
                    self.cam.roll = p.view.roll;
                    self.cam.dist = p.view.dist;
                    self.recompute();
                    self.status = format!("Plan geladen: {}", path.display());
                    self.note(&format!("Plan geladen: {}", path.display()));
                }
                Err(e) => {
                    self.status = format!("Laden fehlgeschlagen: {e}");
                    self.note(&format!("Laden fehlgeschlagen: {e}"));
                }
            }
        }
    }

    /// Baut das 3D-Modell im Hintergrund mit FreeCAD neu.
    ///
    /// Das Rechnen passiert in einem eigenen Thread - die Oberflaeche
    /// bleibt bedienbar. Das Ergebnis holt der Zeichenloop ab.
    fn bau_mit_freecad(&mut self) {
        if self.freecad_laeuft {
            self.freecad_pending = true;
            return;
        }
        let pfad = match crate::freecad::finde_freecad(&self.settings.freecad_pfad) {
            Some(p) => p,
            None => {
                self.status = "FreeCAD wurde nicht gefunden - Pfad in den Einstellungen eintragen."
                    .into();
                self.note("FreeCAD nicht gefunden");
                return;
            }
        };
        self.status = "FreeCAD rechnet das Modell ...".into();
        self.freecad_laeuft = true;
        let ziel = self.export_dir.join("freecad");
        let plan = self.plan.clone();
        let blank = self.blank.clone();
        let params = self.params.clone();
        let (tx, rx) = std::sync::mpsc::channel::<Result<crate::freecad::Ergebnis, String>>();
        std::thread::spawn(move || {
            let r = crate::freecad::baue(&plan, &blank, &params, &pfad, &ziel);
            let _ = tx.send(r);
        });
        self.freecad_kanal = Some(rx);
    }

    /// Nimmt das Ergebnis eines FreeCAD-Durchlaufs entgegen.
    fn freecad_ergebnis(&mut self, r: Result<crate::freecad::Ergebnis, String>) {
        self.freecad_laeuft = false;
        match r {
            Ok(ergebnis) => {
                let ziel_anzeige = self.export_dir.join("freecad").display().to_string();
                self.status = format!(
                    "FreeCAD hat das Modell in {} ms gebaut ({:.0} mm3). STL und STEP liegen in {}",
                    ergebnis.dauer_ms, ergebnis.volumen_mm3, ziel_anzeige
                );
                self.note(&format!(
                    "FreeCAD fertig in {} ms: {} (STEP: {})",
                    ergebnis.dauer_ms,
                    ergebnis.stl.display(),
                    ergebnis.step.display()
                ));
                if !ergebnis.meldung.is_empty() {
                    self.note(&format!("FreeCAD: {}", ergebnis.meldung));
                }
                // Das Ergebnis in der 3D-Ansicht anzeigen. Die Klinge
                // steckt vor dem Griff - sie wird abgetrennt und als
                // Stahl dargestellt, nicht als Holz.
                match crate::freecad::lade_stl(&ergebnis.stl) {
                    Ok(mesh) => {
                        let (klinge, griff) = crate::mesh::teile_bei_x(&mesh, 0.0);
                        let mut parts: Vec<crate::render3d::Part> = Vec::new();
                        if griff.faces.len() > 3 {
                            parts.push(crate::render3d::Part {
                                mesh: griff,
                                color: [0.72, 0.50, 0.30],
                                kind: crate::render3d::PartKind::Wood,
                            });
                        }
                        if klinge.faces.len() > 3 {
                            parts.push(crate::render3d::Part {
                                mesh: klinge,
                                color: [0.58, 0.60, 0.64],
                                kind: crate::render3d::PartKind::Steel,
                            });
                        }
                        if parts.is_empty() {
                            parts.push(crate::render3d::Part {
                                mesh,
                                color: [0.72, 0.50, 0.30],
                                kind: crate::render3d::PartKind::Wood,
                            });
                        }
                        let dreiecke: usize = parts.iter().map(|p| p.mesh.faces.len()).sum();
                        self.scene = Some(Scene { parts });
                        self.scene_dirty = true;
                        self.freecad_meldung = format!("FreeCAD-Modell geladen ({dreiecke} Dreiecke)");
                    }
                    Err(e) => {
                        self.status = format!("STL konnte nicht gelesen werden: {e}");
                        self.note(&format!("STL-Fehler: {e}"));
                    }
                }
            }
            Err(e) => {
                self.status = format!("FreeCAD meldet: {e}");
                self.note(&format!("FreeCAD-Fehler: {e}"));
            }
        }
    }

    /// Speichert einen hochaufgelösten Render als PNG.
    fn save_scene_png(&mut self) {
        let Some(scene) = &self.scene else { return };
        let dir = self.export_dir.clone();
        let w = 1800usize;
        let h = 1100usize;
        self.status = "Render wird erstellt ...".into();
        let rgba = crate::render3d::render(scene, w, h, &self.cam, 4);
        let stem = crate::export::file_stem(&self.blank, &self.params, &self.plan.verdict);
        let path = dir.join(format!("{stem}_3d.png"));
        match crate::render3d::save_png(&path, &rgba, w, h) {
            Ok(()) => self.status = format!("3D-Render gespeichert: {}", path.display()),
            Err(e) => self.status = format!("Render fehlgeschlagen: {e}"),
        }
    }

    fn preview_tab(&mut self, ui: &mut egui::Ui) {
        let g = &self.plan.geometry;

        let size = egui::vec2(ui.available_width(), (ui.available_height() - 26.0).clamp(200.0, 620.0));
        let (rect, _resp) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(252, 251, 248));

        // Bounding-Box der Zeichnung (ohne Rand), damit die Zeichnung
        // mittig im Fenster steht.
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
        if let Some(b) = &g.bolster_profile {
            for &(x, y) in b {
                min_x = min_x.min(x);
                min_y = min_y.min(y);
                max_x = max_x.max(x);
                max_y = max_y.max(y);
            }
        }

        let pad = 30.0f32;
        let bw = ((max_x - min_x) as f32).max(1.0);
        let bh = ((max_y - min_y) as f32).max(1.0);
        let s = ((rect.width() - 2.0 * pad) / bw).min((rect.height() - 2.0 * pad) / bh);
        // Mittig platzieren
        let off_x = rect.left() + pad + ((rect.width() - 2.0 * pad) - bw * s) / 2.0;
        let off_y = rect.top() + pad + ((rect.height() - 2.0 * pad) - bh * s) / 2.0;
        let to_screen = |p: (f64, f64)| -> egui::Pos2 {
            egui::pos2(
                off_x + (p.0 as f32 - min_x as f32) * s,
                off_y + (max_y as f32 - p.1 as f32) * s,
            )
        };

        // Tangmitte
        painter.line_segment(
            [to_screen((min_x, 0.0)), to_screen((max_x, 0.0))],
            egui::Stroke::new(1.0, egui::Color32::from_gray(200)),
        );

        let outline: Vec<egui::Pos2> = g.outline.iter().map(|&p| to_screen(p)).collect();
        painter.add(egui::Shape::convex_polygon(
            outline.clone(),
            egui::Color32::from_rgb(236, 226, 205),
            egui::Stroke::new(2.0, egui::Color32::from_rgb(43, 33, 24)),
        ));

        if let Some(b) = &g.bolster_profile {
            let pts: Vec<egui::Pos2> = b.iter().map(|&p| to_screen(p)).collect();
            painter.add(egui::Shape::closed_line(
                pts,
                egui::Stroke::new(1.6, egui::Color32::from_rgb(55, 71, 79)),
            ));
        }
        if let Some(f) = &g.ferrule_profile {
            let pts: Vec<egui::Pos2> = f.iter().map(|&p| to_screen(p)).collect();
            painter.add(egui::Shape::closed_line(
                pts,
                egui::Stroke::new(1.6, egui::Color32::from_rgb(55, 71, 79)),
            ));
        }

        let tang: Vec<egui::Pos2> = g.tang_profile.iter().map(|&p| to_screen(p)).collect();
        painter.add(egui::Shape::closed_line(
            tang,
            egui::Stroke::new(1.2, egui::Color32::from_rgb(110, 165, 130)),
        ));

        for (i, pin) in g.pins.iter().enumerate() {
            let c = to_screen((pin.x_mm, pin.y_mm));
            let r = ((pin.dia_mm / 2.0) * s as f64) as f32;
            painter.circle_filled(c, r, egui::Color32::WHITE);
            painter.circle_stroke(c, r, egui::Stroke::new(1.6, egui::Color32::from_rgb(198, 40, 40)));
            painter.text(
                c + egui::vec2(r + 4.0, -r - 2.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}", i + 1),
                egui::FontId::proportional(12.0),
                egui::Color32::from_rgb(150, 30, 30),
            );
        }

        ui.add_space(6.0);
        ui.horizontal(|ui| {
            ui.label(preview::summary(&self.plan.geometry, &self.params, &self.plan.verdict));
        });
    }

    /// Messwerkzeug: am Foto abmessen und Werte uebernehmen.
    fn measure_tab(&mut self, ui: &mut egui::Ui) {
        let Some(photo) = self.blade_texture.clone() else {
            ui.centered_and_justified(|ui| {
                ui.label(
                    "Kein Foto geladen.\n\nIm Bereich '1 - Fotos' links ein Foto des \
                     Klingenrohlinges waehlen.\n\nTipp: Legt zur Kalibrierung eine \
                     Kreditkarte (85,60 mm) oder ein Lineal neben den Rohling.",
                );
            });
            return;
        };

        // Kopfbereich: Modus, Zoom, Referenz
        ui.horizontal(|ui| {
            ui.radio_value(&mut self.measure_mode, MeasureMode::Calibrate, "Referenz kalibrieren");
            ui.radio_value(&mut self.measure_mode, MeasureMode::Measure, "Messen");
            ui.separator();
            ui.label("Zoom");
            ui.add(egui::Slider::new(&mut self.photo_zoom, 0.5..=8.0).suffix("x"));
            ui.label(format!(
                "Foto: {:.0} x {:.0} px",
                photo.size.x, photo.size.y
            ));
        });
        ui.horizontal(|ui| {
            ui.label("Referenzlaenge");
            ui.add(
                egui::DragValue::new(&mut self.ref_mm)
                    .speed(0.1)
                    .range(1.0..=1000.0)
                    .suffix(" mm"),
            );
            ui.small("Kreditkarte 85,60 mm | 10-Euro-Schein 147 mm | A4 kurz 210 mm");
            if let Some(cal) = &self.calibration {
                ui.separator();
                ui.label(format!("Referenz: {:.1} mm", cal.mm));
                if ui.button("Referenz zurücksetzen").clicked() {
                    self.calibration = None;
                }
            }
        });
        ui.separator();

        // Bildflaeche mit Interaktion
        let area = ui.available_height().max(220.0);
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), area), egui::Sense::click_and_drag());

        let fit = (rect.width() / photo.size.x).min(rect.height() / photo.size.y);
        let z = fit * self.photo_zoom;
        let img_w = photo.size.x * z;
        let img_h = photo.size.y * z;
        let origin = egui::pos2(
            rect.left() + (rect.width() - img_w) / 2.0,
            rect.top() + (rect.height() - img_h) / 2.0,
        );
        let to_screen = |p: egui::Pos2| -> egui::Pos2 {
            egui::pos2(origin.x + p.x * z, origin.y + p.y * z)
        };
        let to_image = |p: egui::Pos2| -> egui::Pos2 {
            egui::pos2((p.x - origin.x) / z, (p.y - origin.y) / z)
        };

        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 4.0, egui::Color32::from_rgb(40, 40, 44));
        painter.with_clip_rect(rect).image(
            photo.tex.id(),
            egui::Rect::from_min_size(origin, egui::vec2(img_w, img_h)),
            egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)),
            egui::Color32::WHITE,
        );

        // Zeichnen: Referenz und Messungen
        if let Some(cal) = &self.calibration {
            painter.line_segment(
                [to_screen(cal.p1), to_screen(cal.p2)],
                egui::Stroke::new(1.6, egui::Color32::from_rgb(80, 200, 120)),
            );
            let mid = to_screen(egui::pos2(
                (cal.p1.x + cal.p2.x) / 2.0,
                (cal.p1.y + cal.p2.y) / 2.0,
            ));
            painter.text(
                mid + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("Referenz {:.1} mm", cal.mm),
                egui::FontId::proportional(13.0),
                egui::Color32::from_rgb(120, 230, 160),
            );
        }
        for (i, m) in self.measurements.iter().enumerate() {
            painter.line_segment(
                [to_screen(m.p1), to_screen(m.p2)],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(240, 90, 60)),
            );
            for &p in &[m.p1, m.p2] {
                painter.circle_filled(to_screen(p), 3.5, egui::Color32::from_rgb(240, 90, 60));
            }
            let mid = to_screen(egui::pos2(
                (m.p1.x + m.p2.x) / 2.0,
                (m.p1.y + m.p2.y) / 2.0,
            ));
            painter.text(
                mid + egui::vec2(6.0, -6.0),
                egui::Align2::LEFT_BOTTOM,
                format!("{}  {:.2} mm", i + 1, m.mm),
                egui::FontId::proportional(13.0),
                egui::Color32::from_rgb(255, 150, 120),
            );
        }

        // Gummizug
        if let (Some(a), Some(b)) = (self.drag_start, self.drag_now) {
            painter.line_segment(
                [to_screen(a), to_screen(b)],
                egui::Stroke::new(2.0, egui::Color32::from_rgb(120, 190, 255)),
            );
            if let Some(cal) = &self.calibration {
                let mm = crate::measure::to_mm(cal, a, b);
                let mid = to_screen(egui::pos2((a.x + b.x) / 2.0, (a.y + b.y) / 2.0));
                painter.text(
                    mid + egui::vec2(6.0, 14.0),
                    egui::Align2::LEFT_TOP,
                    format!("{mm:.2} mm"),
                    egui::FontId::proportional(14.0),
                    egui::Color32::from_rgb(160, 210, 255),
                );
            }
        }

        // Maus: ziehen fuer eine Linie
        if resp.drag_started() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag_start = Some(to_image(p));
                self.drag_now = Some(to_image(p));
            }
        }
        if resp.dragged() {
            if let Some(p) = resp.interact_pointer_pos() {
                self.drag_now = Some(to_image(p));
            }
        }
        if resp.drag_stopped() {
            if let (Some(a), Some(b)) = (self.drag_start, self.drag_now) {
                if a.distance(b) > 3.0 {
                    match self.measure_mode {
                        MeasureMode::Calibrate => {
                            self.calibration = Some(Calibration {
                                p1: a,
                                p2: b,
                                mm: self.ref_mm,
                            });
                            self.status = format!(
                                "Referenz kalibriert: {:.1} mm entspricht {:.1} Bildpixel.",
                                self.ref_mm,
                                a.distance(b)
                            );
                            self.measure_mode = MeasureMode::Measure;
                        }
                        MeasureMode::Measure => {
                            if let Some(cal) = self.calibration.clone() {
                                let mm = crate::measure::to_mm(&cal, a, b);
                                self.measurements.push(Measurement {
                                    p1: a,
                                    p2: b,
                                    mm,
                                    label: String::new(),
                                });
                                self.measure_targets.push(MeasureTarget::BladeLength);
                                self.status = format!("Gemessen: {mm:.2} mm");
                            } else {
                                self.status =
                                    "Erst die Referenz kalibrieren (Modus oben umstellen)."
                                        .into();
                            }
                        }
                    }
                }
            }
            self.drag_start = None;
            self.drag_now = None;
        }

        // Werkzeugleiste unter dem Bild
        ui.add_space(6.0);
        ui.horizontal(|ui| {
            if ui.button("Alle Messungen löschen").clicked() {
                self.measurements.clear();
                self.measure_targets.clear();
            }
            if ui.button("Kalibrierung löschen").clicked() {
                self.calibration = None;
            }
            ui.separator();
            ui.label("Hinweis: von oben fotografieren, Kamera parallel über dem Objekt.");
        });

        // Messliste mit Zuordnung
        ui.add_space(6.0);
        egui::Grid::new("measure_grid")
            .num_columns(4)
            .striped(true)
            .show(ui, |ui| {
                ui.label(egui::RichText::new("Nr.").strong());
                ui.label(egui::RichText::new("Länge").strong());
                ui.label(egui::RichText::new("Zuordnen").strong());
                ui.label(egui::RichText::new("Wert").strong());
                ui.end_row();
                for i in 0..self.measurements.len() {
                    let mm = self.measurements[i].mm;
                    ui.label(format!("{}", i + 1));
                    ui.label(format!("{mm:.2} mm"));
                    egui::ComboBox::from_id_salt(("m_target", i))
                        .selected_text(self.measure_targets[i].label())
                        .width(170.0)
                        .show_ui(ui, |ui| {
                            for t in MeasureTarget::all() {
                                ui.selectable_value(&mut self.measure_targets[i], t, t.label());
                            }
                        });
                    let target = self.measure_targets[i];
                    if ui.button("übernehmen").clicked() {
                        target.apply(&mut self.blank, mm);
                        self.recompute();
                        self.status = format!(
                            "{} = {:.2} mm übernommen",
                            target.label(),
                            mm
                        );
                    }
                    ui.end_row();
                }
            });

        if self.calibration.is_none() {
            ui.colored_label(
                egui::Color32::from_rgb(220, 150, 40),
                "Noch keine Referenz kalibriert - ohne Referenz sind die Längen nicht \
                 in Millimeter umrechenbar.",
            );
        }
    }

    /// Querschnitt durch den Griff an einer waehlbaren Stelle.
    fn section_tab(&mut self, ui: &mut egui::Ui) {
        let g = &self.plan.geometry;
        let b = &self.blank;

        ui.horizontal(|ui| {
            ui.label("Schnittposition ab dem Heft");
            ui.add(
                egui::Slider::new(&mut self.section_x, 0.0..=g.handle_length_mm.max(1.0))
                    .suffix(" mm"),
            );
            ui.label(format!("von {:.0} mm", g.handle_length_mm));
        });
        ui.separator();

        let x = self.section_x.clamp(0.0, g.handle_length_mm);

        let section = crate::geometry::cross_section(g, b, x);
        let Some(section) = section else {
            ui.label("Zu dieser Stelle gibt es keinen Querschnitt.");
            return;
        };

        // Zeichenflaeche
        let size = egui::vec2(
            ui.available_width(),
            (ui.available_height() - 190.0).clamp(240.0, 720.0),
        );
        let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
        let painter = ui.painter_at(rect);
        painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(252, 251, 248));

        // Koordinatenbereich aus dem Querschnitt
        let mut min_y = section.tang_half_width.min(-section.tang_half_width);
        let mut max_y = section.tang_half_width;
        let mut min_z = -section.tang_half_thickness;
        let mut max_z = section.tang_half_thickness;
        for (z, v) in section
            .bodies
            .iter()
            .flat_map(|(l, u)| l.iter().chain(u.iter()))
        {
            min_y = min_y.min(*v);
            max_y = max_y.max(*v);
            min_z = min_z.min(*z);
            max_z = max_z.max(*z);
        }

        let pad = 40.0f32;
        let s = ((rect.height() - 2.0 * pad) / ((max_y - min_y) as f32).max(1.0))
            .min((rect.width() - 2.0 * pad) / ((max_z - min_z) as f32).max(1.0));
        let cx = rect.center().x;
        let cy = rect.center().y;
        let y_mid = (min_y + max_y) / 2.0;
        let z_mid = (min_z + max_z) / 2.0;
        let to_screen = |y: f64, z: f64| -> egui::Pos2 {
            egui::pos2(
                cx + ((z - z_mid) as f32) * s,
                cy - ((y - y_mid) as f32) * s,
            )
        };

        // Die Angel
        let w = section.tang_half_width;
        let th = section.tang_half_thickness;
        painter.add(egui::Shape::convex_polygon(
            vec![
                to_screen(w, -th),
                to_screen(w, th),
                to_screen(-w, th),
                to_screen(-w, -th),
            ],
            egui::Color32::from_rgb(150, 156, 166),
            egui::Stroke::new(1.2, egui::Color32::from_rgb(60, 66, 76)),
        ));

        // Die Schalen als gefuellter Querschnitt - je Schale fuer sich
        for (lower, upper) in &section.bodies {
            let mut poly: Vec<egui::Pos2> =
                lower.iter().map(|&(z, lo)| to_screen(lo, z)).collect();
            poly.extend(upper.iter().rev().map(|&(z, hi)| to_screen(hi, z)));
            painter.add(egui::Shape::convex_polygon(
                poly,
                egui::Color32::from_rgb(198, 174, 132),
                egui::Stroke::new(1.6, egui::Color32::from_rgb(60, 45, 30)),
            ));
        }

        // Markierungen der Bohrungen an dieser Stelle
        for pin in &g.pins {
            if (pin.x_mm - x).abs() < 1.5 {
                let p = to_screen(pin.y_mm, 0.0);
                painter.circle_stroke(
                    p,
                    (pin.dia_mm / 2.0) as f32 * s,
                    egui::Stroke::new(1.6, egui::Color32::from_rgb(198, 40, 40)),
                );
            }
        }

        // Masse
        let height = section.height;
        let thickness = section.thickness();
        let wall_spine = section.wall_spine;
        let wall_belly = section.wall_belly;
        egui::Grid::new("section_grid")
            .num_columns(2)
            .striped(true)
            .show(ui, |ui| {
                ui.label("Stelle");
                ui.label(format!("{x:.0} mm ab dem Heft"));
                ui.end_row();
                ui.label("Griffoehe");
                ui.label(format!("{height:.1} mm"));
                ui.end_row();
                ui.label("Dicke");
                ui.label(format!("{thickness:.1} mm"));
                ui.end_row();
                ui.label("Wand am Ruecken");
                ui.label(format!("{wall_spine:.1} mm"));
                ui.end_row();
                ui.label("Wand an der Bauchseite");
                ui.label(format!("{wall_belly:.1} mm"));
                ui.end_row();
                ui.label("Angel");
                ui.label(format!("{:.1} x {:.1} mm", w * 2.0, b.tang_thickness_mm));
                ui.end_row();
            });

        if wall_spine < 1.0 || wall_belly < 1.0 {
            ui.colored_label(
                egui::Color32::from_rgb(200, 60, 40),
                "Achtung: an dieser Stelle ist die Wand zu duenn.",
            );
        }
    }

    fn drill_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bohrplan");
            ui.add_space(4.0);
            egui::Grid::new("drill_grid")
                .num_columns(5)
                .striped(true)
                .show(ui, |ui| {
                    ui.label(egui::RichText::new("Bezeichnung").strong());
                    ui.label(egui::RichText::new("Abstand").strong());
                    ui.label(egui::RichText::new("Ø").strong());
                    ui.label(egui::RichText::new("Art").strong());
                    ui.label(egui::RichText::new("Begründung").strong());
                    ui.end_row();
                    for pin in &self.plan.geometry.pins {
                        let art = match pin.kind.as_str() {
                            "through" => "Vorsteckbolzen",
                            "friction" => "Reibniet",
                            "tang" => "Querbolzen",
                            _ => "Lanyardloch",
                        };
                        ui.label(&pin.label);
                        ui.label(format!("{:.1}", pin.x_mm));
                        ui.label(format!("{:.1}", pin.dia_mm));
                        ui.label(art);
                        ui.label(&pin.rationale);
                        ui.end_row();
                    }
                });
            ui.add_space(10.0);
            ui.label(format!(
                "Aussparung: {:.1} mm tief (Angel {:.1} + Spalt {:.2})",
                self.blank.tang_thickness_mm + self.params.glue_gap_mm,
                self.blank.tang_thickness_mm,
                self.params.glue_gap_mm
            ));
        });
    }

    fn material_tab(&mut self, ui: &mut egui::Ui) {
        let v = self.plan.verdict.clone();
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Materialbewertung");
            ui.add_space(4.0);
            ui.label(format!("{} - {} ({}/100)", v.material_name, v.rating, v.score));
            ui.separator();

            if !v.pros.is_empty() {
                ui.label("Vorteile");
                for p in &v.pros {
                    ui.label(format!("  + {p}"));
                }
                ui.add_space(6.0);
            }
            if !v.cons.is_empty() {
                ui.label("Nachteile");
                for c in &v.cons {
                    ui.label(format!("  - {c}"));
                }
                ui.add_space(6.0);
            }
            if !v.risks.is_empty() {
                ui.label("Risiken");
                for r in &v.risks {
                    ui.colored_label(egui::Color32::from_rgb(180, 60, 40), format!("  ! {r}"));
                }
                ui.add_space(6.0);
            }
            if !v.tips.is_empty() {
                ui.label("Tipps");
                for t in &v.tips {
                    ui.label(format!("  - {t}"));
                }
                ui.add_space(6.0);
            }

            if let Some(w) = &v.weight_note {
                ui.separator();
                ui.label(w);
            }
            if let Some(a) = &self.plan.material_analysis {
                ui.separator();
                ui.label(egui::RichText::new("Analyse des Materialfotos").strong());
                if let Some(cat) = &a.category {
                    ui.label(format!("  Kategorie: {cat}"));
                }
                if let Some(sp) = &a.species_guess {
                    ui.label(format!("  Art: {sp}"));
                }
                if let Some(c) = &a.color {
                    ui.label(format!("  Farbe: {c}"));
                }
                if let Some(f) = &a.figure {
                    ui.label(format!("  Maserung: {f}"));
                }
                if let Some(g) = &a.grain_direction {
                    ui.label(format!("  Maserungsverlauf: {g}"));
                }
                if let Some(h) = &a.hardness_guess {
                    ui.label(format!("  Härte: {h}"));
                }
                if let Some(defects) = &a.defects {
                    if !defects.is_empty() {
                        ui.colored_label(
                            egui::Color32::from_rgb(190, 120, 40),
                            format!("  Auffälligkeiten: {}", defects.join(", ")),
                        );
                    }
                }
                if let Some(n) = &a.notes {
                    ui.small(format!("  {}", n));
                }
            }
            ui.separator();
            ui.label(format!("Klebstoff: {}", v.adhesive));
            ui.label(format!("Befestigung: {}", v.fastener));
            ui.label(format!("Finish: {}", v.finish));
            ui.add_space(6.0);
            ui.label(&v.notes);
        });
    }

    fn build_tab(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.heading("Bauplan");
            ui.add_space(4.0);
            let text = self.plan.report_text_mit(&self.datenblaetter);
            ui.monospace(text);
        });
    }
}

/// Startet die grafische Oberflaeche.
///
/// `photo` wird optional direkt geoeffnet (Schaltflaeche Messen).
pub fn run(
    photo: Option<std::path::PathBuf>,
    project: Option<crate::project::Project>,
) -> anyhow::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1500.0, 940.0])
            .with_min_inner_size([1000.0, 640.0])
            .with_title("KnifePlanner - Griff-Planer für Messerdesigns"),
        ..Default::default()
    };
    eframe::run_native(
        "KnifePlanner",
        options,
        Box::new(move |cc| {
            let mut app = App::new(cc);
            if let Some(p) = project.clone() {
                app.blank = p.blank;
                app.params = p.params;
                app.material_key = p.material_key;
                app.project_name = p.name;
                app.project_note = p.note;
                app.cam.yaw = p.view.yaw;
                app.cam.roll = p.view.roll;
                app.cam.dist = p.view.dist;
                app.recompute();
                app.status = "Plan geladen".into();
            }
            if let Some(path) = photo.clone() {
                if let Some(tex) = load_texture(&cc.egui_ctx, &path) {
                    app.blade_photo = Some(path);
                    app.blade_texture = Some(tex);
                    app.tab = 3; // Reiter "Messen"
                    app.status = "Foto geladen - Referenz kalibrieren und messen".into();
                }
            }
            Ok(Box::new(app))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Das Fenster konnte nicht geoeffnet werden: {e}"))?;
    Ok(())
}
