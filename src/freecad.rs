//! FreeCAD-Anbindung: das 3D-Modell im Hintergrund von FreeCAD bauen.
//!
//! FreeCAD liefert mit seiner Python-Schnittstelle echte Volumen mit sauberen
//! Flaechen, exakten Bohrungen und STEP-Ausgabe - das ist genauer als jedes
//! selbst gebaute Netz.
//!
//! Der Griff entsteht als Loft durch dieselben Ringe, aus denen auch die
//! 3D-Ansicht der App aufgebaut wird. Komfortrundung, Laengsgrat,
//! Verjuengung und Kantenfase stecken dadurch von Anfang an im Volumen -
//! es muss nichts nachtraeglich weggefranst werden.
//!
//! Das Programm schreibt die Geometrie in eine Datendatei und ein kleines
//! Makro, startet `FreeCADCmd` (ohne Fenster) und liest das Ergebnis als
//! STL und STEP ein.

use std::path::{Path, PathBuf};
use std::process::Command;

use crate::model::{BladeBlank, HandleParams};
use crate::plan::DesignPlan;

/// Wo FreeCAD liegen koennte.
const KANDIDATEN: &[&str] = &[
    // Windows
    r"C:\Program Files\FreeCAD 1.0\bin\FreeCADCmd.exe",
    r"C:\Program Files\FreeCAD 1.0\bin\freecadcmd.exe",
    r"C:\Program Files\FreeCAD\bin\FreeCADCmd.exe",
    r"C:\Program Files\FreeCAD\bin\freecadcmd.exe",
    r"C:\Program Files\FreeCAD 1.1\bin\FreeCADCmd.exe",
    r"C:\Program Files\FreeCAD 1.1\bin\freecadcmd.exe",
    r"C:\Program Files (x86)\FreeCAD 1.0\bin\FreeCADCmd.exe",
    // macOS
    "/Applications/FreeCAD.app/Contents/Resources/bin/freecadcmd",
    "/Applications/FreeCAD.app/Contents/Resources/bin/FreeCADCmd",
    "/Applications/FreeCAD.app/Contents/MacOS/FreeCADCmd",
    "/Applications/FreeCAD.app/Contents/MacOS/freecadcmd",
    // Linux
    "/usr/bin/freecadcmd",
    "/usr/bin/FreeCADCmd",
    "/usr/local/bin/freecadcmd",
];

/// Sucht die FreeCAD-Kommandozeile.
///
/// `pfad` ist der in den Einstellungen hinterlegte Pfad (darf leer sein).
pub fn finde_freecad(pfad: &str) -> Option<PathBuf> {
    if !pfad.trim().is_empty() {
        let p = PathBuf::from(pfad.trim());
        if p.is_file() {
            return Some(p);
        }
        // Auch ein Verzeichnis (z. B. der bin-Ordner) ist erlaubt
        if p.is_dir() {
            for name in ["FreeCADCmd.exe", "freecadcmd.exe", "FreeCADCmd", "freecadcmd"] {
                let c = p.join(name);
                if c.is_file() {
                    return Some(c);
                }
            }
        }
    }
    if let Ok(env) = std::env::var("FREECAD_CMD") {
        let p = PathBuf::from(env);
        if p.is_file() {
            return Some(p);
        }
    }
    for k in KANDIDATEN {
        let p = PathBuf::from(k);
        if p.is_file() {
            return Some(p);
        }
    }
    // Im PATH suchen
    if let Ok(paths) = std::env::var("PATH") {
        for dir in paths.split(':') {
            for name in ["FreeCADCmd", "freecadcmd", "FreeCADCmd.exe", "freecadcmd.exe"] {
                let p = Path::new(dir).join(name);
                if p.is_file() {
                    return Some(p);
                }
            }
        }
    }
    None
}

/// Das Ergebnis eines FreeCAD-Laufs.
pub struct Ergebnis {
    pub stl: PathBuf,
    pub step: PathBuf,
    pub dauer_ms: u128,
    pub meldung: String,
    /// Gesamtvolumen des Modells in mm^3 (0, wenn nicht ermittelt).
    pub volumen_mm3: f64,
    /// Volumen des Holzteils in mm^3 (die Griffschalen).
    pub holzvolumen_mm3: f64,
}

/// Die Geometrie fuer das FreeCAD-Makro: die Ringe und die Profile.
///
/// Die Punktlisten stehen in einer eigenen Datei. Als Zahlenliteral im
/// Makro wuerde Python die langen Listen zaehlnal durchdenken - der
/// Ein-Stueck-Griff brauchte dann fast eine Minute, statt in wenigen
/// Sekunden fertig zu sein.
pub struct Daten {
    /// Die Ringe je Griffkoerper.
    pub ringe: Vec<Vec<Vec<(f64, f64, f64)>>>,
    /// Die Ringe der Klinge.
    pub klinge: Vec<Vec<(f64, f64, f64)>>,
    /// Die Keilprofile fuer die Verjuengung: je Koerper die Schnitte oben
    /// und unten (leer, wenn keine Verjuengung da ist).
    pub keile: Vec<(Vec<(f64, f64)>, Vec<(f64, f64)>)>,
    /// Die Angel.
    pub angel: Vec<(f64, f64)>,
    /// Die Bastion, falls vorhanden.
    pub bastion: Option<Vec<(f64, f64)>>,
    /// Die Zwinge als Mantel: aussen und innen.
    pub zwinge: Option<(Vec<(f64, f64)>, Vec<(f64, f64)>)>,
    /// Die Ringe der Bastion (Mantel mit gebrochenen Kanten).
    pub bastion_ringe: Vec<Vec<(f64, f64, f64)>>,
    /// Die Vorsteckbolzen: Radius, x, y.
    pub durch: Vec<(f64, f64, f64)>,
    /// Die Nieten und Querbolzen in der Angel: Radius, x, y, Tiefe.
    pub in_der_angel: Vec<(f64, f64, f64, f64)>,
    /// Das Lanyardloch quer durch die Dicke: Radius, x, y.
    pub lanyard_dicke: Vec<(f64, f64, f64)>,
    /// Das Lanyardloch durch die Hoehe: Radius, x, y unten, y oben.
    pub lanyard_hoehe: Vec<(f64, f64, f64, f64)>,
}

/// Sammelt die Geometrie fuer das Makro.
pub fn sammle_daten(plan: &DesignPlan, blank: &BladeBlank, params: &HandleParams) -> Daten {
    let g = &plan.geometry;

    // Die Ringe ohne Verjuengung: der Loft braucht ebene Schnitte.
    // Die Verjuengung kommt danach als Schnitt mit einem Keil dazu.
    let mut ringe = crate::geometry::griff_ringe_ebene(blank, params);
    let n_slot = if params.style == crate::model::HandleStyle::OnePiece {
        crate::geometry::slot_path(&g.recess).len()
    } else {
        0
    };
    for body in ringe.iter_mut() {
        for ring in body.iter_mut() {
            *ring = verdichte_ring(ring, n_slot);
        }
    }

    // Die Keile fuer die Verjuengung
    let (scale_t, total_t) = crate::geometry::schalendicken(blank, params);
    let span = if params.style == crate::model::HandleStyle::FullTangTwoScale {
        scale_t
    } else {
        total_t / 2.0
    };
    let x_start = g.outline.iter().map(|q| q.0).fold(f64::MAX, f64::min);
    let x_end = g.outline.iter().map(|q| q.0).fold(f64::MIN, f64::max);
    let x_span = (x_end - x_start).max(1.0);

    let klinge = crate::blade::ringe(blank);

    let mut keile = Vec::new();
    for body in &ringe {
        let (z_min, z_max) = body_z_grenze(body);
        let anker = if z_min >= -0.001 {
            z_min
        } else if z_max <= 0.001 {
            z_max
        } else {
            0.0
        };
        let mut oben = Vec::new();
        let mut unten = Vec::new();
        if params.section.taper_mm > 0.01 {
            if (z_max - anker).abs() > 0.01 {
                oben = keilprofil(params, x_start, x_span, span, anker, z_max, true);
            }
            if (z_min - anker).abs() > 0.01 {
                unten = keilprofil(params, x_start, x_span, span, anker, z_min, false);
            }
        }
        keile.push((oben, unten));
    }

    // Die Bohrungen
    let tang_t = blank.tang_thickness_mm;
    let mut durch = Vec::new();
    let mut in_der_angel = Vec::new();
    let mut lanyard_dicke = Vec::new();
    let mut lanyard_hoehe = Vec::new();
    for pin in &g.pins {
        match pin.kind.as_str() {
            "through" => durch.push((pin.dia_mm / 2.0, pin.x_mm, pin.y_mm)),
            "friction" | "tang" => {
                let tiefe = pin.depth_mm.unwrap_or(2.0).min(tang_t);
                in_der_angel.push((pin.dia_mm / 2.0, pin.x_mm, pin.y_mm, tiefe));
            }
            "lanyard" => {
                let r = pin.dia_mm / 2.0;
                if pin.achse == "y" {
                    // Durch die Hoehe: vom Bauch bis unter die Angel
                    let (unten, oben) = crate::geometry::kontur_hoehe(g, pin.x_mm);
                    let tang_dort = crate::geometry::angel_breite(blank, pin.x_mm);
                    let y_ende = if tang_dort > 0.02 { -tang_dort } else { oben };
                    lanyard_hoehe.push((r, pin.x_mm, unten - 0.5, y_ende));
                } else {
                    lanyard_dicke.push((r, pin.x_mm, pin.y_mm));
                }
            }
            _ => {}
        }
    }

    Daten {
        ringe,
        klinge,
        keile,
        angel: g.tang_profile.clone(),
        bastion: g.bolster_profile.clone(),
        zwinge: g.ferrule_profile.clone().map(|f| {
            // Die Zwinge ist ein Mantel um das Holz: aussen etwas groesser,
            // innen mit kleiner Passung
            let aussen = crate::mesh::offset(&f, -crate::export::stl::WAND_ZWINGE);
            let innen = crate::mesh::offset(&f, -crate::export::stl::PASSUNG_ZWINGE);
            (aussen, innen)
        }),
        bastion_ringe: g.bolster_profile.as_ref().map(|b| {
            crate::mesh::platten_ringe(
                b,
                -g.total_thickness_mm / 2.0,
                g.total_thickness_mm / 2.0,
                1.2,
                1.2,
                4,
            )
            .into_iter()
            .map(|(kontur, z)| {
                kontur
                    .into_iter()
                    .map(|(x, y)| (x, y, z))
                    .collect::<Vec<_>>()
            })
            .collect()
        })
        .unwrap_or_default(),
        durch,
        in_der_angel,
        lanyard_dicke,
        lanyard_hoehe,
    }
}

/// Schreibt die Geometrie in die Datendatei.
pub fn schreibe_daten(daten: &Daten) -> String {
    let mut s = String::new();
    s.push_str("# Geometrie fuer das FreeCAD-Makro - von KnifePlanner erzeugt.\n");
    s.push_str("# RINGE: x y z je Zeile, ein Block je Koerper\n");
    for (i, body) in daten.ringe.iter().enumerate() {
        s.push_str(&format!("RINGE {i}\n"));
        for ring in body {
            s.push_str("SCHNITT\n");
            for &(x, y, z) in ring {
                s.push_str(&format!("{:.4} {:.4} {:.4}\n", x, y, z));
            }
        }
    }
    s.push_str("KLINGE\n");
    for ring in &daten.klinge {
        s.push_str("SCHNITT\n");
        for &(x, y, z) in ring {
            s.push_str(&format!("{:.4} {:.4} {:.4}\n", x, y, z));
        }
    }
    s.push_str("ANGEL\n");
    for &(x, y) in &daten.angel {
        s.push_str(&format!("{:.4} {:.4}\n", x, y));
    }
    if !daten.bastion_ringe.is_empty() {
        s.push_str("BASTION\n");
        for ring in &daten.bastion_ringe {
            s.push_str("SCHNITT\n");
            for &(x, y, z) in ring {
                s.push_str(&format!("{:.4} {:.4} {:.4}\n", x, y, z));
            }
        }
    }
    if let Some((aussen, innen)) = &daten.zwinge {
        s.push_str("ZWINGEAUSSEN\n");
        for &(x, y) in aussen {
            s.push_str(&format!("{:.4} {:.4}\n", x, y));
        }
        s.push_str("ZWINGEINNEN\n");
        for &(x, y) in innen {
            s.push_str(&format!("{:.4} {:.4}\n", x, y));
        }
    }
    for (i, (oben, unten)) in daten.keile.iter().enumerate() {
        if !oben.is_empty() {
            s.push_str(&format!("KEIL {i} oben\n"));
            for &(x, z) in oben {
                s.push_str(&format!("{:.4} {:.4}\n", x, z));
            }
        }
        if !unten.is_empty() {
            s.push_str(&format!("KEIL {i} unten\n"));
            for &(x, z) in unten {
                s.push_str(&format!("{:.4} {:.4}\n", x, z));
            }
        }
    }
    s
}

/// Baut das FreeCAD-Makro fuer den Plan.
///
/// Das Makro erzeugt aus den Ringen der App echte Volumen:
/// * den Griffkoerper als Loft (Schale oder Ein-Stueck-Griff)
/// * die Angel, Bastion und Zwinge als Prismen
/// * die Vorsteckbolzen als echte Durchgangsbohrungen
///
/// und exportiert als STL und STEP.
pub fn makro(
    plan: &DesignPlan,
    blank: &BladeBlank,
    params: &HandleParams,
    daten_ziel: &str,
    stl_ziel: &str,
    step_ziel: &str,
) -> String {
    let g = &plan.geometry;
    let daten = sammle_daten(plan, blank, params);
    let tang_t = blank.tang_thickness_mm;
    let mut out = String::new();

    out.push_str("# -*- coding: utf-8 -*-\n");
    out.push_str("# Von KnifePlanner erzeugt - rechnet das 3D-Modell im Hintergrund.\n");
    out.push_str("#\n");
    out.push_str("# Der Griff entsteht als Loft durch dieselben Ringe, aus denen auch\n");
    out.push_str("# die 3D-Ansicht der App aufgebaut wird. Komfortrundung,\n");
    out.push_str("# Laengsgrat, Verjuengung und Kantenfase stecken dadurch von\n");
    out.push_str("# Anfang an im Volumen - es muss nichts nachtraeglich weggefranst\n");
    out.push_str("# werden und das Ergebnis bleibt ein echter, gueltiger Solid.\n");
    out.push_str("import FreeCAD as App\n");
    out.push_str("import Part\n");
    out.push_str("import time\n");
    out.push_str("\n");
    out.push_str("def lade_daten(pfad):\n");
    out.push_str("    # Die Geometrie steht in einer eigenen Datei - als Zahlenliteral\n");
    out.push_str("    # im Makro wuerde Python unendlich lange brauchen.\n");
    out.push_str("    bloecke = {}\n");
    out.push_str("    name = None\n");
    out.push_str("    with open(pfad) as f:\n");
    out.push_str("        for zeile in f:\n");
    out.push_str("            zeile = zeile.strip()\n");
    out.push_str("            if not zeile or zeile.startswith('#'):\n");
    out.push_str("                continue\n");
    out.push_str("            teile = zeile.split()\n");
    out.push_str("            kopf = teile[0]\n");
    out.push_str("            if kopf == 'RINGE':\n");
    out.push_str("                name = ('RINGE', int(teile[1]))\n");
    out.push_str("                bloecke.setdefault(name, [])\n");
    out.push_str("                continue\n");
    out.push_str("            if kopf == 'KLINGE':\n");
    out.push_str("                name = ('KLINGE',)\n");
    out.push_str("                bloecke.setdefault(name, [])\n");
    out.push_str("                continue\n");
    out.push_str("            if kopf == 'KEIL':\n");
    out.push_str("                name = ('KEIL', int(teile[1]), teile[2])\n");
    out.push_str("                bloecke.setdefault(name, [])\n");
    out.push_str("                continue\n");
    out.push_str("            if kopf == 'SCHNITT':\n");
    out.push_str("                bloecke[name].append([])\n");
    out.push_str("                continue\n");
    out.push_str("            if kopf in ('ANGEL', 'BASTION', 'ZWINGEAUSSEN', 'ZWINGEINNEN'):\n");
    out.push_str("                name = (kopf,)\n");
    out.push_str("                bloecke.setdefault(name, [])\n");
    out.push_str("                continue\n");
    out.push_str("            if name is None:\n");
    out.push_str("                continue\n");
    out.push_str("            if name[0] == 'RINGE' or name[0] == 'KLINGE' or name[0] == 'BASTION':\n");
    out.push_str("                bloecke[name][-1].append((float(teile[0]), float(teile[1]), float(teile[2])))\n");
    out.push_str("            else:\n");
    out.push_str("                bloecke[name].append((float(teile[0]), float(teile[1])))\n");
    out.push_str("    return bloecke\n");
    out.push_str("\n");
    out.push_str("def draht(pts):\n");
    out.push_str("    v = [App.Vector(x, y, z) for (x, y, z) in pts]\n");
    out.push_str("    return Part.makePolygon(v + [v[0]])\n");
    out.push_str("\n");
    out.push_str("def koerper_aus_ringen(ringe):\n");
    out.push_str("    # Loft durch die Ringe. Gerade Flaechen zwischen den Schnitten:\n");
    out.push_str("    # eine weiche Flaeche wuerde sich zwischen den vielen Punkten\n");
    out.push_str("    # der Kontur wellen.\n");
    out.push_str("    drahte = [draht(r) for r in ringe]\n");
    out.push_str("    return Part.makeLoft(drahte, True, True)\n");
    out.push_str("\n");
    out.push_str("def prism(punkte, unten, oben):\n");
    out.push_str("    # Ein Profil in der xy-Ebene, in der Dicke gezogen\n");
    out.push_str("    f = Part.Face(draht([(x, y, 0.0) for (x, y) in punkte]))\n");
    out.push_str("    k = f.extrude(App.Vector(0, 0, oben - unten))\n");
    out.push_str("    return k.translated(App.Vector(0, 0, unten))\n");
    out.push_str("\n");
    out.push_str("def keil(profil, y0, y1):\n");
    out.push_str("    # Ein Profil in der xz-Ebene, quer zur Grifflaenge gezogen\n");
    out.push_str("    v = [App.Vector(x, 0.0, z) for (x, z) in profil]\n");
    out.push_str("    d = Part.makePolygon(v + [v[0]])\n");
    out.push_str("    f = Part.Face(d)\n");
    out.push_str("    return f.extrude(App.Vector(0, y1 - y0, 0)).translated(App.Vector(0, y0, 0))\n");
    out.push_str("\n");

    // --- Daten laden ---
    out.push_str("# --- Geometrie laden ---\n");
    out.push_str(&format!(
        "daten = lade_daten('{}')\n",
        python_pfad(daten_ziel)
    ));

    // --- Griffkoerper ---
    out.push_str("# --- Griffkoerper ---\n");
    out.push_str("teile = []\n");
    out.push_str("namen = []\n");
    out.push_str("start = time.time()\n");
    for i in 0..daten.ringe.len() {
        out.push_str(&format!("ringe_{i} = daten[('RINGE', {i})]\n"));
        out.push_str("try:\n");
        out.push_str(&format!("    koerp = koerper_aus_ringen(ringe_{i})\n"));
        out.push_str("except Exception as e:\n");
        out.push_str(
            "    App.Console.PrintWarning('Loft misslungen, Notform wird gerechnet: %s\\n' % e)\n",
        );
        out.push_str(&format!(
            "    koerp = prism(ringe_{i}[0], ringe_{i}[0][0][2], ringe_{i}[-1][0][2])\n"
        ));

        // Die Verjuengung als Schnitt mit einem Keil
        let (oben, unten) = &daten.keile[i];
        if !oben.is_empty() {
            out.push_str(&format!("koerp = koerp.common(keil(daten[('KEIL', {i}, 'oben')], -500.0, 500.0))\n"));
        }
        if !unten.is_empty() {
            out.push_str(&format!("koerp = koerp.common(keil(daten[('KEIL', {i}, 'unten')], -500.0, 500.0))\n"));
        }
        out.push_str("teile.append(koerp)\n");
        out.push_str(&format!("namen.append('Griffschale {}')\n", i + 1));
    }
    out.push_str("App.Console.PrintMessage('KP_LOFT %d ms\\n' % ((time.time() - start) * 1000))\n");

    // --- Klinge ---
    out.push_str("\n# --- Klinge ---\n");
    out.push_str("try:\n");
    out.push_str("    teile.append(koerper_aus_ringen(daten[('KLINGE',)]))\n");
    out.push_str("    namen.append('Klinge')\n");
    out.push_str("except Exception as e:\n");
    out.push_str("    App.Console.PrintWarning('Klinge nicht gerechnet: %s\\n' % e)\n");

    // --- Angel ---
    //
    // Die Angel ist die Fortsetzung der Klinge im Griff - beides ist ein
    // Stueck Stahl. Deshalb werden die beiden Volumen zu einem
    // verschmolzen, sonst muesste der Benutzer sie in CAD noch zusammenfuegen.
    out.push_str("\n# --- Angel ---\n");
    out.push_str(&format!(
        "angel = prism(daten[('ANGEL',)], {}, {})\n",
        fmt(-tang_t / 2.0),
        fmt(tang_t / 2.0)
    ));
    out.push_str("try:\n");
    out.push_str("    teile[-1] = teile[-1].fuse(angel)\n");
    out.push_str("    namen[-1] = 'Klinge mit Angel'\n");
    out.push_str("except Exception as e:\n");
    out.push_str("    App.Console.PrintWarning('Klinge und Angel bleiben getrennt: %s\\n' % e)\n");
    out.push_str("    teile.append(angel)\n");
    out.push_str("    namen.append('Angel')\n");
    out.push_str("angel_index = len(teile) - 1\n");

    // --- Bastion und Zwinge ---
    if !daten.bastion_ringe.is_empty() {
        out.push_str("\n# --- Bastion ---\n");
        out.push_str("bastion = koerper_aus_ringen(daten[('BASTION',)])\n");
        out.push_str("teile.append(bastion)\n");
        out.push_str("namen.append('Bastion')\n");
    }
    if daten.zwinge.is_some() {
        // Die Zwinge ist ein Mantel um das Holz - ein voller Block
        // waere zum Zusammenbauen unbrauchbar.
        out.push_str("\n# --- Zwinge ---\n");
        out.push_str(&format!(
            "zwinge = prism(daten[('ZWINGEAUSSEN',)], {}, {})\n",
            fmt(-g.total_thickness_mm / 2.0),
            fmt(g.total_thickness_mm / 2.0)
        ));
        out.push_str(&format!(
            "hohl = prism(daten[('ZWINGEINNEN',)], {}, {})\n",
            fmt(-g.total_thickness_mm / 2.0),
            fmt(g.total_thickness_mm / 2.0)
        ));
        out.push_str("zwinge = zwinge.cut(hohl)\n");
        out.push_str("teile.append(zwinge)\n");
        out.push_str("namen.append('Zwinge')\n");
    }

    // --- Bohrungen ---
    //
    // Die Vorsteckbolzen gehen durch beide Schalen und durch die Angel.
    // Reibniet und Querbolzen sitzen in der Angel und werden dort
    // eingebohrt - das Holz bleibt unangetastet.
    out.push_str("\n# --- Bohrungen ---\n");
    for (r, x, y) in &daten.durch {
        out.push_str(&format!(
            "zyl = Part.makeCylinder({}, {}, App.Vector({}, {}, {}), App.Vector(0, 0, 1))\n",
            fmt(*r),
            fmt(g.total_thickness_mm + 2.0),
            fmt(*x),
            fmt(*y),
            fmt(-g.total_thickness_mm / 2.0 - 1.0)
        ));
        out.push_str("for i in range(len(teile)):\n");
        out.push_str("    teile[i] = teile[i].cut(zyl)\n");
    }
    for (r, x, y, t) in &daten.in_der_angel {
        out.push_str(&format!(
            "zyl = Part.makeCylinder({}, {}, App.Vector({}, {}, {}), App.Vector(0, 0, 1))\n",
            fmt(*r),
            fmt(*t),
            fmt(*x),
            fmt(*y),
            fmt(-tang_t / 2.0)
        ));
        out.push_str("teile[angel_index] = teile[angel_index].cut(zyl)\n");
    }
    for (r, x, y) in &daten.lanyard_dicke {
        out.push_str(&format!(
            "zyl = Part.makeCylinder({}, {}, App.Vector({}, {}, {}), App.Vector(0, 0, 1))\n",
            fmt(*r),
            fmt(g.total_thickness_mm + 2.0),
            fmt(*x),
            fmt(*y),
            fmt(-g.total_thickness_mm / 2.0 - 1.0)
        ));
        out.push_str("for i in range(len(teile)):\n");
        out.push_str("    teile[i] = teile[i].cut(zyl)\n");
    }
    for (r, x, y0, y1) in &daten.lanyard_hoehe {
        out.push_str(&format!(
            "zyl = Part.makeCylinder({}, {}, App.Vector({}, {}, {}), App.Vector(0, 1, 0))\n",
            fmt(*r),
            fmt(y1 - y0),
            fmt(*x),
            fmt(*y0),
            fmt(-g.total_thickness_mm / 2.0 - 1.0)
        ));
        out.push_str("for i in range(len(teile)):\n");
        out.push_str("    teile[i] = teile[i].cut(zyl)\n");
    }

    // --- Export ---
    out.push_str("\n# --- Export ---\n");
    out.push_str("if not teile:\n");
    out.push_str("    raise RuntimeError('FreeCAD hat kein Volumen erzeugt')\n");
    out.push_str("verbund = Part.Compound(teile)\n");
    out.push_str("volumen = 0.0\n");
    out.push_str("for i in range(len(teile)):\n");
    out.push_str("    t = teile[i]\n");
    out.push_str("    volumen += t.Volume\n");
    out.push_str("    App.Console.PrintMessage('KP %s: %d Flaechen, gueltig=%s, Volumen %.1f mm3\\n' % (namen[i], len(t.Faces), t.isValid(), t.Volume))\n");
    out.push_str("App.Console.PrintMessage('KP_GESAMT %.1f\\n' % volumen)\n");
    out.push_str(&format!(
        "App.Console.PrintMessage('KP_HOLZ %.1f\\n' % sum(t.Volume for t in teile[:{}]))\n",
        daten.ringe.len()
    ));
    out.push_str(&format!(
        "verbund.exportStep('{}')\n",
        python_pfad(step_ziel)
    ));
    out.push_str("import MeshPart\n");
    out.push_str("netz = MeshPart.meshFromShape(Shape=verbund, LinearDeflection=0.05, AngularDeflection=0.35, Relative=False)\n");
    out.push_str(&format!(
        "netz.write('{}')\n",
        python_pfad(stl_ziel)
    ));
    out.push_str("App.Console.PrintMessage('KNIFEPLANNER_OK')\n");
    out
}

/// Formatiert eine Zahl fuer Python.
fn fmt(v: f64) -> String {
    format!("{:.4}", v)
}

/// Bindet einen Pfad als Python-Text ein.
///
/// Die Rueckwaertstriche werden verdoppelt und Anfuehrungszeichen
/// ausgewichen. Sonst bricht das Makro an einem Pfad mit einem Apostroph
/// im Benutzernamen - auf Windows nicht ausgeschlossen.
fn python_pfad(pfad: &str) -> String {
    let mut r = String::new();
    for c in pfad.chars() {
        match c {
            '\\' => r.push_str("\\\\"),
            '\'' => r.push_str("\\'"),
            '"' => r.push_str("\\\""),
            other => r.push(other),
        }
    }
    r
}

/// Behaelt jeden zweiten Punkt - der erste und der letzte bleiben erhalten.
///
/// Fuer den Loft in FreeCAD reicht die halbe Punktzahl: die Kontur
/// laeuft in zehntel Millimetern, die Datei wird dadurch deutlich kleiner.
fn verdichte_ring(ring: &[(f64, f64, f64)], n_slot: usize) -> Vec<(f64, f64, f64)> {
    let mut out: Vec<(f64, f64, f64)> = Vec::with_capacity(ring.len() / 2 + 4);
    if ring.is_empty() {
        return out;
    }
    out.push(ring[0]);
    // Die Nut fuer die Angel liegt dazwischen - sie muss zur Angel passen
    let n_slot = n_slot.min(ring.len().saturating_sub(1));
    out.extend(every_nth(&ring[1..1 + n_slot], 2));
    out.extend(every_nth(&ring[1 + n_slot..], 2));
    out
}

/// Behaelt jeden n-ten Punkt - der erste und der letzte bleiben erhalten.
fn every_nth<T: Copy>(punkte: &[T], n: usize) -> Vec<T> {
    if n <= 1 {
        return punkte.to_vec();
    }
    let mut out: Vec<T> = Vec::new();
    let mut i = 0;
    while i < punkte.len() {
        out.push(punkte[i]);
        i += n;
    }
    // Der letzte Punkt bleibt erhalten
    if !punkte.is_empty() && (punkte.len() - 1) % n != 0 {
        out.push(punkte[punkte.len() - 1]);
    }
    out
}

/// Die kleinste und die groesste z-Hoehe eines Koerpers.
fn body_z_grenze(body: &[Vec<(f64, f64, f64)>]) -> (f64, f64) {
    let mut z_min = f64::MAX;
    let mut z_max = f64::MIN;
    for ring in body {
        for p in ring {
            if p.2 < z_min {
                z_min = p.2;
            }
            if p.2 > z_max {
                z_max = p.2;
            }
        }
    }
    if z_min > z_max {
        (0.0, 0.0)
    } else {
        (z_min, z_max)
    }
}

/// Das Profil des Keils, der die Verjuengung schneidet.
///
/// Der Keil liegt in der xz-Ebene: unten liegt er unter dem Griff, oben
/// folgt er der Verjuengungskurve. `anker` ist die Ebene, die stehen
/// bleibt - bei zwei Schalen die Innenflaeche, beim Ein-Stueck-Griff die
/// Mitte.
fn keilprofil(
    params: &HandleParams,
    x_start: f64,
    x_span: f64,
    span: f64,
    anker: f64,
    grenze: f64,
    oben: bool,
) -> Vec<(f64, f64)> {
    let schritte = 48;
    let mut profil: Vec<(f64, f64)> = Vec::new();
    let tief = if oben {
        anker - (anker - grenze).abs() - 5.0
    } else {
        anker + (anker - grenze).abs() + 5.0
    };
    profil.push((x_start, tief));
    for i in 0..=schritte {
        let x = x_start + x_span * i as f64 / schritte as f64;
        let f = crate::geometry::taper_faktor(&params.section, x_start, x_span, span, x);
        profil.push((x, anker + (grenze - anker) * f));
    }
    profil.push((x_start + x_span, tief));
    profil
}

/// Laesst FreeCAD das Modell bauen.
pub fn baue(
    plan: &DesignPlan,
    blank: &BladeBlank,
    params: &HandleParams,
    freecad: &Path,
    zielordner: &Path,
) -> Result<Ergebnis, String> {
    std::fs::create_dir_all(zielordner).map_err(|e| e.to_string())?;
    let stl = zielordner.join("griff_freecad.stl");
    let step = zielordner.join("griff_freecad.step");
    let daten_datei = zielordner.join("griff_freecad_daten.txt");
    let makro_datei = zielordner.join("griff_freecad.py");

    let daten = sammle_daten(plan, blank, params);
    std::fs::write(&daten_datei, schreibe_daten(&daten)).map_err(|e| e.to_string())?;

    let code = makro(
        plan,
        blank,
        params,
        &daten_datei.to_string_lossy(),
        &stl.to_string_lossy(),
        &step.to_string_lossy(),
    );
    std::fs::write(&makro_datei, code).map_err(|e| e.to_string())?;

    let start = std::time::Instant::now();
    let ausgabe = Command::new(freecad)
        .arg(&makro_datei)
        .output()
        .map_err(|e| format!("FreeCAD konnte nicht gestartet werden: {e}"))?;
    let dauer_ms = start.elapsed().as_millis();

    let stdout = String::from_utf8_lossy(&ausgabe.stdout);
    let stderr = String::from_utf8_lossy(&ausgabe.stderr);
    let alles = format!("{stdout}\n{stderr}");

    if !stl.exists() {
        return Err(format!(
            "FreeCAD hat keine Datei erzeugt.\n{}\nMakro: {}",
            alles.lines().rev().take(12).collect::<Vec<_>>().join("\n"),
            makro_datei.display()
        ));
    }
    if !alles.contains("KNIFEPLANNER_OK") {
        return Err(format!(
            "FreeCAD meldet einen Fehler:\n{}",
            alles.lines().rev().take(12).collect::<Vec<_>>().join("\n")
        ));
    }
    let hole_zahl = |marke: &str| -> f64 {
        alles
            .lines()
            .find_map(|l| l.trim().strip_prefix(marke))
            .and_then(|v| v.trim().parse::<f64>().ok())
            .unwrap_or(0.0)
    };
    let volumen_mm3 = hole_zahl("KP_GESAMT ");
    let holzvolumen_mm3 = hole_zahl("KP_HOLZ ");
    let step_ziel = if step.exists() { step } else { stl.clone() };
    Ok(Ergebnis {
        stl,
        step: step_ziel,
        dauer_ms,
        meldung: alles
            .lines()
            .filter(|l| {
                l.contains("Error")
                    || l.contains("Fehler")
                    || l.contains("Warning")
                    || l.contains("KP ")
            })
            .take(6)
            .collect::<Vec<_>>()
            .join(" | "),
        volumen_mm3,
        holzvolumen_mm3,
    })
}

/// Liest ein binares STL ein (fuer die 3D-Ansicht).
pub fn lade_stl(pfad: &Path) -> Result<crate::mesh::Mesh, String> {
    let daten = std::fs::read(pfad).map_err(|e| e.to_string())?;
    if daten.len() < 84 {
        return Err("STL zu klein".into());
    }
    let anzahl = u32::from_le_bytes([daten[80], daten[81], daten[82], daten[83]]) as usize;
    if daten.len() < 84 + anzahl * 50 {
        return Err("STL unvollstaendig".into());
    }
    let mut mesh = crate::mesh::Mesh::default();
    let lese = |off: usize| -> f64 {
        let mut b = [0u8; 4];
        b.copy_from_slice(&daten[off..off + 4]);
        f32::from_le_bytes(b) as f64
    };
    for i in 0..anzahl {
        let off = 84 + i * 50 + 12;
        for k in 0..3 {
            let p = (lese(off + k * 12), lese(off + k * 12 + 4), lese(off + k * 12 + 8));
            mesh.vertices.push(p);
        }
    }
    for i in 0..anzahl {
        let a = (i * 3) as usize;
        mesh.faces.push((a, a + 1, a + 2));
    }
    Ok(mesh)
}

#[cfg(test)]
mod pfad_tests {
    use super::*;

    #[test]
    fn pfad_wird_python_tauglich() {
        // Windows-Pfad mit Backslashes und Apostroph im Benutzernamen
        let fies = r"C:\Users\O'Brien\Kniffe\griff_freecad_daten.txt";
        let eingebettet = python_pfad(fies);
        assert!(
            eingebettet.contains(r"\'"),
            "der Apostroph muss ausgewichen werden: {eingebettet}"
        );
        assert!(eingebettet.contains(r"\\Users"));
        // Das Ergebnis muss wieder der Originalpfad sein, wenn Python
        // die Maske abarbeitet
        let zurueck = eingebettet.replace(r"\\", "\\").replace(r"\'", "'");
        assert_eq!(zurueck, fies);
    }

    #[test]
    fn makro_braucht_keine_rohen_pfade() {
        // Die Pfade stehen in normalen Python-Zeichenketten - sonst
        // waere ein Backslash ein Escape-Zeichen.
        let plan = crate::plan::DesignPlan::build(
            &BladeBlank::default(),
            &HandleParams::default(),
            "walnuss",
        );
        let code = makro(
            &plan,
            &BladeBlank::default(),
            &HandleParams::default(),
            r"C:\Users\Test\daten.txt",
            r"C:\Users\Test\modell.stl",
            r"C:\Users\Test\modell.step",
        );
        assert!(
            code.contains("lade_daten('C:\\\\Users\\\\Test\\\\daten.txt')"),
            "{code}"
        );
        assert!(!code.contains("r'C:"), "keine rohen Pfade im Makro");
    }
}
