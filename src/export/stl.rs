//! STL- und OBJ-Export des Griffes.
//!
//! Erzeugt wird die komplette Baugruppe:
//!   * Griffschalen mit gerundeten Kanten (Kantenradius = Kantenabrundung)
//!   * die Angel als eigener Koerper
//!   * Bastion bzw. Zwinge als Metallkoerper
//!   * die Aussparung fuer die Angel als eigener Koerper (Frassbahn)
//!
//! Die Volumen liegen als getrennte Schalen in einer Datei - die CAD-Software
//! kann sie einzeln einfuegen, farblich trennen oder per Differenz schneiden.

use std::path::Path;

use crate::materials::Material;
use crate::mesh::{merge, offset, prism, rounded_slab, write_stl, Mesh};
use crate::render3d::PartKind;
use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};

use super::file_stem;
use crate::materials::MaterialVerdict;

/// Ein Volumen mit Farbe fuer die Vorschau und den OBJ-Export.
pub struct Part {
    pub mesh: Mesh,
    pub color: [f32; 3],
    pub name: &'static str,
    pub kind: PartKind,
}

fn wood_color(material: &Material) -> [f32; 3] {
    // Grundton je Holzart, damit die Vorschau den Charakter erahnen laesst
    match material.key {
        "ebenholz" => [0.16, 0.13, 0.12],
        "palisander" => [0.32, 0.18, 0.14],
        "wenge" => [0.20, 0.17, 0.14],
        "olivenholz" => [0.62, 0.48, 0.24],
        "buchsbaum" => [0.85, 0.74, 0.50],
        "zebrawood" => [0.70, 0.55, 0.34],
        "eiche" => [0.68, 0.53, 0.36],
        "esche" => [0.86, 0.79, 0.64],
        "ahorn" => [0.90, 0.85, 0.74],
        "kirsche" => [0.60, 0.36, 0.22],
        "birke" => [0.88, 0.82, 0.70],
        "thuja" => [0.82, 0.74, 0.56],
        "walnuss" => [0.45, 0.30, 0.20],
        "horn" => [0.30, 0.22, 0.17],
        "bone" => [0.92, 0.88, 0.78],
        "geweih" => [0.85, 0.80, 0.68],
        "stabilisiert" => [0.78, 0.68, 0.52],
        "composite" => [0.28, 0.30, 0.33],
        _ => [0.60, 0.45, 0.30],
    }
}

/// Metallfarbe je Bauteil.
fn metal_color(material: &Material) -> [f32; 3] {
    match material.key {
        "messing" => [0.80, 0.65, 0.30],
        "kupfer" => [0.85, 0.45, 0.28],
        "titan" => [0.48, 0.50, 0.54],
        "edelstahl" => [0.62, 0.64, 0.67],
        "aluminium" => [0.70, 0.72, 0.75],
        _ => [0.70, 0.70, 0.72],
    }
}

/// Baut die Baugruppe des Griffes.
pub fn build_parts(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &Material,
) -> Vec<Part> {
    let mut parts: Vec<Part> = Vec::new();
    let tang_t = blank.tang_thickness_mm;
    let gap = params.glue_gap_mm;

    let wood = wood_color(material);
    let steel = [0.58, 0.60, 0.64];
    let metal = metal_color(material);

    match params.style {
        HandleStyle::FullTangTwoScale => {
            // Zwei Schalen, dazwischen die Angel
            for body in &geo.bodies {
                parts.push(Part {
                    mesh: crate::mesh::loft(body),
                    color: wood,
                    name: "Griffschale",
                    kind: PartKind::Wood,
                });
            }
            parts.push(Part {
                mesh: prism(&geo.tang_profile, -tang_t / 2.0, tang_t / 2.0),
                color: steel,
                name: "Angel",
                kind: PartKind::Steel,
            });
        }
        HandleStyle::PartialTangBolster => {
            // Holzkoerper ueber der Angel, Bastion aus Metall davor
            for body in &geo.bodies {
                parts.push(Part {
                    mesh: crate::mesh::loft(body),
                    color: wood,
                    name: "Holzkoerper",
                    kind: PartKind::Wood,
                });
            }
            if let Some(b) = &geo.bolster_profile {
                let bm = rounded_slab(b, geo.total_thickness_mm, 1.2, 4);
                parts.push(Part { mesh: bm, color: metal, name: "Bastion", kind: PartKind::Metal });
            }
            parts.push(Part {
                mesh: prism(&geo.tang_profile, -tang_t / 2.0, tang_t / 2.0),
                color: steel,
                name: "Angel",
                kind: PartKind::Steel,
            });
        }
        HandleStyle::OnePiece => {
            // Ein-Stueck-Griff: massiver Holzkoerper mit einer in den Boden
            // gefraesten Nut fuer die Angel.
            for body in &geo.bodies {
                parts.push(Part {
                    mesh: crate::mesh::loft(body),
                    color: wood,
                    name: "Holzkoerper",
                    kind: PartKind::Wood,
                });
            }
            parts.push(Part {
                mesh: prism(&geo.tang_profile, -tang_t / 2.0, tang_t / 2.0),
                color: steel,
                name: "Angel",
                kind: PartKind::Steel,
            });
        }
        HandleStyle::Ferrule => {
            for body in &geo.bodies {
                parts.push(Part {
                    mesh: crate::mesh::loft(body),
                    color: wood,
                    name: "Holzkoerper",
                    kind: PartKind::Wood,
                });
            }
            if let Some(f) = &geo.ferrule_profile {
                // Die Zwinge ist ein Mantel um das Holz, kein aufgesetzter
                // Klotz - sonst laesst sich der Griff nicht zusammenbauen.
                let aussen = crate::mesh::offset(f, -WAND_ZWINGE);
                let innen = crate::mesh::offset(f, -PASSUNG_ZWINGE);
                parts.push(Part {
                    mesh: crate::mesh::hohlprism(
                        &aussen,
                        &innen,
                        -geo.total_thickness_mm / 2.0,
                        geo.total_thickness_mm / 2.0,
                    ),
                    color: metal,
                    name: "Zwinge",
                    kind: PartKind::Metal,
                });
            }
            parts.push(Part {
                mesh: prism(&geo.tang_profile, -tang_t / 2.0, tang_t / 2.0),
                color: steel,
                name: "Angel",
                kind: PartKind::Steel,
            });
        }
    }

    // Die Klinge selbst - sie sitzt vor dem Griff und setzt dessen
    // Kontur am Heft fort.
    let klinge = crate::blade::klinge(blank);
    if klinge.faces.len() > 3 {
        parts.push(Part {
            mesh: klinge,
            color: steel,
            name: "Klinge",
            kind: PartKind::Steel,
        });
    }

    // Aussparung fuer die Angel (Frassbahn) als eigenes Volumen.
    // Bei zwei Schalen sitzt je eine Tasche in der Innenflaeche, vom
    // Angelmass plus Klebespalt tief.
    let pocket = match params.style {
        HandleStyle::FullTangTwoScale => {
            let depth = tang_t + gap;
            let right = prism(&geo.recess, tang_t / 2.0, tang_t / 2.0 + depth);
            let left = prism(&geo.recess, -(tang_t / 2.0 + depth), -tang_t / 2.0);
            merge(&[right, left])
        }
        _ => prism(&geo.recess, -tang_t / 2.0, tang_t / 2.0),
    };
    parts.push(Part {
        mesh: pocket,
        color: [0.25, 0.25, 0.28],
        name: "Aussparung",
        kind: PartKind::Cavity,
    });

    parts
}

/// Die Wandstaerke der Zwinge (Messingmantel um das Holz).
pub const WAND_ZWINGE: f64 = 1.5;

/// So viel Luft bleibt zwischen Zwinge und Holz.
pub const PASSUNG_ZWINGE: f64 = 0.1;

/// Die Teile fuer die 3D-Ansicht - ohne die innen liegende Aussparung.
pub fn build_scene_parts(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &Material,
) -> Vec<Part> {
    let mut parts: Vec<Part> = build_parts(geo, blank, params, material)
        .into_iter()
        .filter(|p| p.kind != PartKind::Cavity)
        .map(|mut p| {
            // Die Angel in der Ansicht hinter die gerundete Schalenkante
            // setzen, damit ihre Vorderflaeche nicht mit der Fase flimmert.
            // Im STL bleibt die Angel exakt an der berechneten Stelle.
            if p.name == "Angel" {
                p.mesh = p.mesh.translated(2.6, 0.0, 0.0);
            }
            p
        })
        .collect();

    // Die Bohrungen als Zylinder - so sieht man im Modell, wie die Bohrer
    // durch die Schale gehen. Reibniet und Querbolzen enden an der Angel,
    // die Vorsteckbolzen gehen komplett durch. Das Lanyardloch laeuft
    // je nach Lage quer durch die Dicke oder durch die Hoehe.
    let half_t = geo.total_thickness_mm / 2.0;
    let tang_half = blank.tang_thickness_mm / 2.0;
    for pin in &geo.pins {
        if pin.achse == "y" {
            // Durch die Hoehe: vom Bauch nach oben, hoechstens bis zur Angel
            let (unten, oben) = crate::geometry::kontur_hoehe(geo, pin.x_mm);
            let tang_dort = crate::geometry::angel_unten(blank, params, pin.x_mm);
            let y_ende = tang_dort.unwrap_or(oben);
            parts.push(Part {
                mesh: crate::mesh::zylinder_y(
                    pin.x_mm,
                    unten,
                    y_ende,
                    pin.dia_mm / 2.0,
                    16,
                ),
                color: [0.85, 0.25, 0.20],
                name: "Bohrung",
                kind: PartKind::Steel,
            });
            continue;
        }
        let (z0, z1) = match pin.kind.as_str() {
            // Durchgehend durch beide Schalen
            "through" => (-half_t - 0.5, half_t + 0.5),
            // Reibniet: von der Aussenkante bis auf die Angel
            "friction" => (tang_half, half_t + 0.5),
            // Querbolzen: blind in der Angel
            "tang" => (tang_half - 1.0, half_t + 0.5),
            // Lanyardloch: durch die Griffende, quer durch die Dicke
            _ => (-half_t - 0.5, half_t + 0.5),
        };
        let r = pin.dia_mm / 2.0;
        parts.push(Part {
            mesh: crate::mesh::cylinder(pin.x_mm, pin.y_mm, z0, z1, r, 16),
            color: [0.85, 0.25, 0.20],
            name: "Bohrung",
            kind: PartKind::Steel,
        });
    }

    parts
}

/// Schreibt die STL-Datei der Baugruppe.
pub fn write_stl_file(
    dir: &Path,
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &MaterialVerdict,
) -> std::io::Result<std::path::PathBuf> {
    let mat = crate::materials::get(&material.material_key).unwrap_or(&crate::materials::MATERIALS[0]);
    let parts = build_parts(geo, blank, params, mat);
    let meshes: Vec<Mesh> = parts.iter().map(|p| p.mesh.clone()).collect();
    let merged = merge(&meshes);
    let stem = file_stem(blank, params, material);
    let path = dir.join(format!("{stem}.stl"));
    write_stl(&path, &[merged], "knifeplanner")?;
    Ok(path)
}

/// Schreibt OBJ plus MTL - damit bleiben Materialien und Farben erhalten.
pub fn write_obj(
    dir: &Path,
    geo: &HandleGeometry,
    blank: &BladeBlank,
    params: &HandleParams,
    material: &MaterialVerdict,
) -> std::io::Result<(std::path::PathBuf, std::path::PathBuf)> {
    let mat = crate::materials::get(&material.material_key).unwrap_or(&crate::materials::MATERIALS[0]);
    let parts = build_parts(geo, blank, params, mat);
    let stem = file_stem(blank, params, material);

    let obj_path = dir.join(format!("{stem}.obj"));
    let mtl_path = dir.join(format!("{stem}.mtl"));
    std::fs::create_dir_all(dir)?;

    let mut obj = String::new();
    obj.push_str(&format!("# KnifePlanner - Griff {stem}\n"));
    obj.push_str(&format!("mtllib {stem}.mtl\n"));

    let mut mtl = String::new();
    let mut offset = 1usize;
    for (i, part) in parts.iter().enumerate() {
        let mat_name = format!("material_{i}");
        mtl.push_str(&format!(
            "newmtl {mat_name}\nKa 0.30 0.30 0.30\nKd {:.3} {:.3} {:.3}\nKs 0.35 0.35 0.35\nNs 48\nd 1.0\nillum 2\n\n",
            part.color[0], part.color[1], part.color[2]
        ));

        obj.push_str(&format!("o {}\n", part.name.replace(' ', "_")));
        obj.push_str(&format!("usemtl {mat_name}\n"));
        for &(x, y, z) in &part.mesh.vertices {
            obj.push_str(&format!("v {x:.4} {y:.4} {z:.4}\n"));
        }
        for &(a, b, c) in &part.mesh.faces {
            obj.push_str(&format!(
                "f {} {} {}\n",
                a + offset,
                b + offset,
                c + offset
            ));
        }
        offset += part.mesh.vertices.len();
    }

    std::fs::write(&obj_path, obj)?;
    std::fs::write(&mtl_path, mtl)?;
    Ok((obj_path, mtl_path))
}

/// Hilfsfunktion: Aussenkontur um `d` einruecken (fuer die Vorschau).
pub fn inset_outline(outline: &[(f64, f64)], d: f64) -> Vec<(f64, f64)> {
    offset(outline, d)
}
