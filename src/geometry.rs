//! Geometrie-Engine: aus Rohling + Gestaltungsidee wird ein baubarer Griff.
//!
//! Koordinaten:
//!  * x = 0 an der Vorderkante des Griffs (Anschluss an die Klinge), x waechst zum Ende.
//!  * y = 0 auf der Mittellinie der Angel, +y = Ruecken, -y = Bauch.
//!  * z = 0 in der Griffmitte, +/- = Dickenrichtung.

use crate::model::{
    BladeBlank, ButtStyle, HandleGeometry, HandleParams, HandleStyle, PinSpec, ProfileStyle,
    SectionShape, SectionSpec,
};
use crate::text::de_mm;

pub type Point = (f64, f64);

/// Abtastschritt in mm entlang der Grifflaenge.
const SAMPLE_STEP: f64 = 0.75;

// ---------------------------------------------------------------------------
// Hilfsfunktionen
// ---------------------------------------------------------------------------

fn clamp(v: f64, lo: f64, hi: f64) -> f64 {
    v.max(lo).min(hi)
}

fn smoothstep(t: f64) -> f64 {
    let t = clamp(t, 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// Gleitender Mittelwert, Haelt die Laenge bei.
fn smooth(values: &[f64], window: usize) -> Vec<f64> {
    if window < 2 || values.len() < 3 {
        return values.to_vec();
    }
    let half = window / 2;
    let n = values.len();
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            values[lo..hi].iter().sum::<f64>() / (hi - lo) as f64
        })
        .collect()
}

/// Gleitendes Maximum - behaelt die Wandstaerke garantiert ein.
fn smooth_max(values: &[f64], window: usize) -> Vec<f64> {
    if window < 2 || values.len() < 3 {
        return values.to_vec();
    }
    let half = window / 2;
    let n = values.len();
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            values[lo..hi].iter().copied().fold(f64::MIN, f64::max)
        })
        .collect()
}

/// Gleitendes Minimum.
fn smooth_min(values: &[f64], window: usize) -> Vec<f64> {
    if window < 2 || values.len() < 3 {
        return values.to_vec();
    }
    let half = window / 2;
    let n = values.len();
    (0..n)
        .map(|i| {
            let lo = i.saturating_sub(half);
            let hi = (i + half + 1).min(n);
            values[lo..hi].iter().copied().fold(f64::MAX, f64::min)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Angelprofil
// ---------------------------------------------------------------------------

/// Halbe Angelbreite an der Stelle x (0 = Vorderkante des Griffs).
pub fn tang_half_width(b: &BladeBlank, x: f64) -> f64 {
    let l = b.tang_length_mm;
    let w0 = b.tang_width_mm / 2.0;
    if x <= 0.0 {
        return w0;
    }
    if x >= l {
        return 0.0;
    }
    if !b.tang_taper {
        return w0;
    }

    let x0 = clamp(b.tang_taper_start, 0.0, 0.95) * l;
    if x <= x0 {
        return w0;
    }

    let w_back = b.tang_back_width_mm / 2.0;
    if w_back > 0.0 {
        let t = (x - x0) / (l - x0).max(1e-6);
        return (w0 + (w_back - w0) * t).max(0.0);
    }

    // Verjuengung auf eine runde Spitze.
    //
    // Die Verjuengung laeuft bis zum Beginn der runden Spitze bei l - r;
    // die Spitze selbst ist ein Halbkreis, der in der Angelmitte endet.
    // So bleibt der Draht geschlossen - ein Ansatz auf Null Breite wuerde
    // zweimal dieselbe Strecke erzeugen und den Draht ueberschneiden.
    let span = (l - x0).max(1e-6);
    let r = (w0 * 0.5).max(0.5).min(span * 0.10);
    let x_rund = (l - r).max(x0);
    if x <= x_rund {
        let t = (x - x0) / (x_rund - x0).max(1e-6);
        return (w0 + (r - w0) * t).max(0.0);
    }
    let d = x - x_rund;
    if d >= r {
        return 0.0;
    }
    ((r * r - d * d).max(0.0)).sqrt()
}

/// Angelprofil als geschlossenes Polygon.
///
/// Die Punkte mit Null Breite werden weggelassen, damit kein Stueck
/// zweimal durchlaufen wird. Die Spitze ist ein eigener Punkt.
pub fn tang_profile(b: &BladeBlank, step: f64) -> Vec<Point> {
    let l = b.tang_length_mm;
    let mut xs: Vec<f64> = (0..=(l / step).floor() as usize)
        .map(|i| i as f64 * step)
        .collect();
    if xs.last().copied().unwrap_or(0.0) < l {
        xs.push(l);
    }
    let mut oben: Vec<Point> = Vec::new();
    let mut unten: Vec<Point> = Vec::new();
    for &x in &xs {
        let w = tang_half_width(b, x);
        if w > 1e-4 {
            oben.push((x, w));
            unten.push((x, -w));
        }
    }
    // Die runde Spitze als eigenen Punkt aufnehmen
    if oben.last().map(|p| (p.0 - l).abs() > 1e-4).unwrap_or(false) {
        oben.push((l, 0.0));
    }
    let mut poly = oben;
    poly.extend(unten.into_iter().rev());
    poly
}

/// Schneidet ein Polygon an der Ebene x = grenze ab.
///
/// Nur der Teil mit x >= grenze bleibt erhalten. Schnittpunkte werden
/// eingefuegt, damit das Polygon geschlossen und einfach bleibt.
pub fn clip_x_min(poly: &[(f64, f64)], grenze: f64) -> Vec<(f64, f64)> {
    let mut out: Vec<(f64, f64)> = Vec::with_capacity(poly.len());
    let n = poly.len();
    if n < 3 {
        return poly.to_vec();
    }
    for i in 0..n {
        let a = poly[i];
        let b = poly[(i + 1) % n];
        let a_in = a.0 >= grenze - 1e-9;
        let b_in = b.0 >= grenze - 1e-9;
        if a_in {
            out.push(a);
        }
        if a_in != b_in {
            let dx = b.0 - a.0;
            if dx.abs() > 1e-12 {
                let t = ((grenze - a.0) / dx).clamp(0.0, 1.0);
                out.push((grenze, a.1 + t * (b.1 - a.1)));
            }
        }
    }
    out
}

/// Polygon versetzen - positiv zieht nach innen, negativ nach aussen.
pub fn offset_polygon(poly: &[(f64, f64)], delta: f64) -> Vec<(f64, f64)> {
    crate::mesh::offset(poly, delta)
}

// ---------------------------------------------------------------------------
// Griffkontur
// ---------------------------------------------------------------------------

fn spine_curve(b: &BladeBlank, p: &HandleParams, x: f64, x_end: f64) -> f64 {
    match p.profile {
        ProfileStyle::Blade => {
            let t = x / x_end.max(1e-6);
            let drop = p.spine_camber_mm * smoothstep(clamp((t - 0.08) / 0.92, 0.0, 1.0));
            b.spine_offset_mm - drop
        }
        ProfileStyle::Ergonomic => {
            // Der Ruecken waechst aus dem Heft heraus auf die Griffhoehe und
            // faellt zum Griffende sanft ab.
            let t = (x / x_end.max(1e-6)).clamp(0.0, 1.0);
            let grow = grip_growth(b, p);
            let rise = grow * smoothstep((t / 0.22).min(1.0));
            let drop = p.spine_camber_mm * smoothstep((t - 0.58) / 0.42);
            b.spine_offset_mm + rise - drop
        }
    }
}

fn belly_curve(b: &BladeBlank, p: &HandleParams, x: f64, x_end: f64) -> f64 {
    match p.profile {
        ProfileStyle::Blade => {
            let t = x / x_end.max(1e-6);
            let belly = p.belly_drop_mm * (4.0 * t * (1.0 - t)).powf(0.7);
            -b.edge_offset_mm.abs() - belly
        }
        ProfileStyle::Ergonomic => {
            // Ballenauflage: die Bauchlinie senkt sich unter der Hand und
            // bekommt eine flache Fingerkehle.
            let t = (x / x_end.max(1e-6)).clamp(0.0, 1.0);
            let grow = grip_growth(b, p);
            let swell = grow * smoothstep((t / 0.22).min(1.0));
            let palm = p.belly_drop_mm * bell(t, 0.46, 0.52);
            let groove = p.finger_groove_mm * bell(t, 0.70, 0.13);
            -b.edge_offset_mm.abs() - swell - palm + groove
        }
    }
}

/// Wie weit der Griff ueber das Heftprofil hinaus waechst.
fn grip_growth(b: &BladeBlank, p: &HandleParams) -> f64 {
    let front_height = b.spine_offset_mm + b.edge_offset_mm.abs();
    ((p.grip_height_mm - front_height) / 2.0).max(0.0)
}

/// Die Rueckenlinie mit der Wand um die Angel herum.
///
/// Ist die Angel hoeher als der Ruecken des Rohlings, muss der Griff
/// mitwachsen - sonst steht der Stahl aus dem Holz heraus. Bastion,
/// Zwinge und die Kurven fuer die Komfortrundung nehmen dieselbe Linie,
/// damit kein Absatz zwischen den Bauteilen entsteht.
fn spine_mit_wand(b: &BladeBlank, p: &HandleParams, x: f64, x_end: f64) -> f64 {
    let w = tang_half_width(b, x);
    if w <= 0.0 {
        return spine_curve(b, p, x, x_end);
    }
    let wall = p.wall_min_mm.max(0.8);
    spine_curve(b, p, x, x_end).max(w + wall)
}

/// Die Bauchlinie mit der Wand um die Angel herum.
fn bauch_mit_wand(b: &BladeBlank, p: &HandleParams, x: f64, x_end: f64) -> f64 {
    let w = tang_half_width(b, x);
    if w <= 0.0 {
        return belly_curve(b, p, x, x_end);
    }
    let wall = p.wall_min_mm.max(0.8);
    belly_curve(b, p, x, x_end).min(-(w + wall))
}

/// Glockenkurve: 0 an den Raendern, 1 in der Mitte bei `center`.
fn bell(t: f64, center: f64, width: f64) -> f64 {
    let d = ((t - center) / width.max(1e-6)).abs();
    if d >= 1.0 {
        0.0
    } else {
        (1.0 - d * d) * (1.0 - d * d)
    }
}

fn butt_cap(x_end: f64, y_top: f64, y_bot: f64, style: ButtStyle) -> Vec<Point> {
    let mid = (y_top + y_bot) / 2.0;
    let half = (y_top - y_bot) / 2.0;
    if half <= 0.1 {
        return vec![(x_end, y_top), (x_end, y_bot)];
    }
    let depth = match style {
        ButtStyle::Flat => half * 0.35,
        ButtStyle::Flare => half * 1.25,
        ButtStyle::Round => half,
    };
    let steps = 28;
    (0..=steps)
        .map(|i| {
            let a = std::f64::consts::PI * i as f64 / steps as f64;
            (x_end + depth * a.sin(), mid + half * a.cos())
        })
        .collect()
}

/// Griffkontur der Holzschale + Warnungen.
pub fn build_outline(b: &BladeBlank, p: &HandleParams) -> (Vec<Point>, Vec<String>) {
    let mut warnings: Vec<String> = Vec::new();

    let x_start = match p.style {
        // Bei der Bastion beginnt das Holz hinter dem Metallmantel.
        HandleStyle::PartialTangBolster => p.bolster_width_mm,
        // Die Zwinge ist ein Mantel ueber dem vorderen Griffende -
        // das Holz laeuft deshalb bis ganz nach vorn durch.
        HandleStyle::Ferrule => 0.0,
        _ => 0.0,
    };
    // Die gewuenschte Grifflaenge ist unabhaengig von der Angel.
    let base_len = match p.handle_length_mm {
        Some(v) if v > 0.0 => v.max(b.tang_length_mm),
        _ => b.tang_length_mm,
    };
    let x_end = base_len + p.butt_overhang_mm.max(0.0);

    let n = ((x_end - x_start) / SAMPLE_STEP).ceil().max(8.0) as usize;
    let mut xs: Vec<f64> = (0..=n)
        .map(|i| x_start + (x_end - x_start) * i as f64 / n as f64)
        .collect();

    let mut tops: Vec<f64> = xs.iter().map(|&x| spine_curve(b, p, x, x_end)).collect();
    let mut bots: Vec<f64> = xs.iter().map(|&x| belly_curve(b, p, x, x_end)).collect();

    // Wandstaerke garantieren: Abstand Angel <-> Aussenkontur >= wall_min
    for (i, &x) in xs.iter().enumerate() {
        tops[i] = spine_mit_wand(b, p, x, x_end);
        bots[i] = bauch_mit_wand(b, p, x, x_end);
    }

    tops = smooth(&tops, 5);
    bots = smooth(&bots, 5);
    for (i, &x) in xs.iter().enumerate() {
        tops[i] = spine_mit_wand(b, p, x, x_end);
        bots[i] = bauch_mit_wand(b, p, x, x_end);
    }
    tops = smooth_max(&tops, 3);
    bots = smooth_min(&bots, 3);

    // Vorderkante: gerade oder mit Fase
    let mut x_start = x_start;
    if p.front_bevel_deg.abs() > 0.01 {
        let tan_b = p.front_bevel_deg.abs().to_radians().tan();
        let shift = (tops[0] - bots[0]) * tan_b;
        tops.insert(0, tops[0] - shift);
        bots.insert(0, bots[0]);
        xs.insert(0, x_start - shift);
        x_start -= shift;
    }
    let _ = x_start;

    // Kontur: Vorderkante -> Ruecken -> Endkappe -> Bauch
    let mut outline: Vec<Point> = vec![(xs[0], bots[0])];
    outline.extend(xs.iter().copied().zip(tops.iter().copied()));
    let cap = butt_cap(x_end, tops[tops.len() - 1], bots[bots.len() - 1], p.butt_style);
    outline.extend(cap.into_iter().skip(1));
    // Der erste Punkt der Bauchlinie ist identisch mit dem letzten Punkt
    // der Endkappe - er wird nicht doppelt gesetzt.
    outline.extend(
        xs.iter()
            .rev()
            .copied()
            .zip(bots.iter().rev().copied())
            .skip(1),
    );
    if let Some(last) = outline.last() {
        if (last.0 - outline[0].0).abs() < 1e-9 && (last.1 - outline[0].1).abs() < 1e-9 {
            outline.pop();
        }
    }

    let wall = p.wall_min_mm.max(0.8);
    if b.tang_half_width() + wall > b.spine_offset_mm.min(b.edge_offset_mm.abs()) {
        warnings.push(
            "Die Angel ist breiter als das Heftprofil zulässt - die Griffkontur muss \
             aufweiten. Eine Bastion oder eine Änderung der Rohlinggeometrie ist nötig, \
             sonst steht die Griffkontur über dem Heft vor."
                .to_string(),
        );
    }

    (outline, warnings)
}

// ---------------------------------------------------------------------------
// Bohrbild
// ---------------------------------------------------------------------------

fn pin_diameter(b: &BladeBlank) -> f64 {
    let d = b.tang_thickness_mm * 0.5;
    (clamp(d, 2.0, 5.0) * 10.0).round() / 10.0
}

/// Bereich der Angel, in dem die Breite konstant ist.
fn parallel_span(b: &BladeBlank) -> (f64, f64) {
    let l = b.tang_length_mm;
    let x0 = clamp(b.tang_taper_start, 0.0, 0.95) * l;
    if !b.tang_taper {
        (0.0, l)
    } else {
        (x0, l)
    }
}

/// Bohrplan nach den ueblichen Regeln des Messerbaus.
pub fn plan_pins(b: &BladeBlank, p: &HandleParams) -> Vec<PinSpec> {
    let mut pins: Vec<PinSpec> = Vec::new();
    let l = b.tang_length_mm;
    let (x0, x1) = parallel_span(b);
    if x1 - x0 < 12.0 {
        return pins;
    }

    let dia = pin_diameter(b);
    let can_through = b.tang_thickness_mm >= 2.6;

    // Ein Vorsteckbolzen braucht volle Angelbreite plus Rand. Der hintere
    // Bolzen sitzt deshalb so weit hinten, wie die Angel noch dick genug ist.
    let min_half = dia / 2.0 + 0.8;
    let mut rear = x1 - 3.0;
    while rear > x0 + 4.0 && tang_half_width(b, rear) < min_half {
        rear -= 0.5;
    }
    if rear <= x0 + 4.0 {
        rear = x0 + 4.0;
    }

    let margin = (l * 0.13).max(10.0).min(18.0);
    let front = margin.min(rear - 12.0).max(6.0);

    if can_through {
        let mut positions = vec![front];
        let span = rear - front;
        let max_span = 75.0;
        if span > max_span {
            let count = (span / max_span).ceil() as usize;
            for k in 1..count {
                positions.push(front + span * k as f64 / count as f64);
            }
        }
        positions.push(rear);

        let last = positions.len() - 1;
        for (i, &x) in positions.iter().enumerate() {
            let (label, why) = if i == 0 {
                (
                    "Vorsteckbolzen vorn".to_string(),
                    "Sichert den vorderen Griffanschluss am Rohling.".to_string(),
                )
            } else if i == last {
                (
                    "Vorsteckbolzen hinten".to_string(),
                    "Sichert das Griffende gegen Abplatzen und Ausziehen. Die Angel ist hier \
                     noch mindestens so breit wie der Bolzen."
                        .to_string(),
                )
            } else {
                (
                    format!("Vorsteckbolzen Mitte {i}"),
                    "Begrenzt den Abstand zwischen den Bolzen auf unter 75 mm.".to_string(),
                )
            };
            pins.push(PinSpec {
                kind: "through".into(),
                label,
                x_mm: (x * 100.0).round() / 100.0,
                y_mm: 0.0,
                dia_mm: dia,
                depth_mm: None,
                through_tang: true,
                rationale: why,
                achse: "z".into(),
            });
        }

        // Reibnieten gegen Verdrehen im vorderen Griffdrittel.
        // Ein Reibniet braucht eine Nut in der Angel - die kann nur in den
        // geraden, vollen Teil der Angel gefraest werden.
        if rear - front > 45.0 && x0 > front + 12.0 {
            let xf = (front + (rear - front) * 0.35).clamp(front + 8.0, x0 - 3.0);
            pins.push(PinSpec {
                kind: "friction".into(),
                label: "Reibniet".into(),
                x_mm: (xf * 100.0).round() / 100.0,
                y_mm: 0.0,
                dia_mm: 2.4,
                depth_mm: Some((b.tang_thickness_mm / 2.0 * 100.0).round() / 100.0),
                through_tang: false,
                rationale: "Reibt in einer halbtiefen Nut in der Angel und verhindert, dass \
                            sich die Schale im vorderen Griffdrittel verdreht. Die Nut wird \
                            in den geraden Teil der Angel gefraest, so breit, dass der Niet \
                            mit Spiel darin sitzt."
                    .into(),
                achse: "z".into(),
            });
        }
    } else {
        pins.push(PinSpec {
            kind: "tang".into(),
            label: "Querbolzen in die Angel".into(),
            x_mm: ((x0 + 6.0).min(x1) * 100.0).round() / 100.0,
            y_mm: 0.0,
            dia_mm: 2.0,
            depth_mm: Some((b.tang_thickness_mm + 1.0).round()),
            through_tang: false,
            rationale: "Die Angel ist zu duenn fuer einen durchgehenden Vorsteckbolzen. \
                        Querbolzen quer durch die Angel in die Schale verankern."
                .into(),
            achse: "z".into(),
        });
    }

    // Lanyardloch
    //
    // Liegt der Griff weit genug hinter dem Angelende, geht das Loch quer
    // durch die Dicke - so, wie ein Lanyardloch ueblicherweise sitzt. Es
    // ist dann vollstaendig im Holz und die Angel stoert nicht.
    // Reicht der Griff nicht weit genug, wird das Loch durch die Hoehe
    // gebohrt und endet unter der Angel.
    if p.lanyard_hole && p.lanyard_hole_dia_mm > 0.0 {
        // Das hintere Griffende: der gerade Teil plus die Endkappe
        let base_len = match p.handle_length_mm {
            Some(v) if v > 0.0 => v.max(b.tang_length_mm),
            _ => b.tang_length_mm,
        };
        let x_ende = base_len + p.butt_overhang_mm.max(0.0);
        let top = spine_curve(b, p, x_ende, x_ende);
        let bot = belly_curve(b, p, x_ende, x_ende);
        let halb = (top - bot).abs() / 2.0;
        let kappe = match p.butt_style {
            ButtStyle::Flat => halb * 0.35,
            ButtStyle::Flare => halb * 1.25,
            ButtStyle::Round => halb,
        };
        let handle_end = x_ende + kappe;
        let hinter_der_angel = b.tang_length_mm + 4.0;
        let (x_lanyard, achse) = if handle_end - hinter_der_angel >= 6.0 {
            (
                clamp(handle_end - 6.0, hinter_der_angel, handle_end - 3.0),
                "z",
            )
        } else {
            (clamp(l - 8.0, 6.0, (l - 2.0).max(6.0)), "y")
        };
        let tang_there = tang_half_width(b, x_lanyard);
        let note = if tang_there > 0.02 {
            format!(
                "Durch das Griffende gebohrt, quer zur Grifflaenge. Die Angel reicht hier \
                 noch {} mm an das Griffende - entweder schraeg nach unten bohren, sodass \
                 der Bohler unter der Angel durchgeht, oder in der Angel einen \
                 Entlastungsslot von {:.1} mm Breite fräsen.",
                (l - x_lanyard).max(0.0),
                p.lanyard_hole_dia_mm + 1.0
            )
        } else {
            format!(
                "Durch das Griffende gebohrt, quer zur Grifflaenge. Die Angel endet {} mm \
                 davor, das Loch liegt vollstaendig im Holz.",
                (x_lanyard - l).max(0.0).round()
            )
        };
        pins.push(PinSpec {
            kind: "lanyard".into(),
            label: "Lanyardloch".into(),
            x_mm: (x_lanyard * 100.0).round() / 100.0,
            y_mm: 0.0,
            dia_mm: (p.lanyard_hole_dia_mm * 10.0).round() / 10.0,
            depth_mm: None,
            through_tang: false,
            rationale: note,
            achse: achse.into(),
        });
    }

    pins
}

// ---------------------------------------------------------------------------
// Bastion / Zwinge
// ---------------------------------------------------------------------------

pub fn bolster_profile(b: &BladeBlank, p: &HandleParams) -> Vec<Point> {
    // Die Bastion ist ein Metallmantel zwischen Heft und Holzkoerper.
    // Ihre Kontur folgt oben dem Ruecken und unten der Bauchlinie des
    // Heftprofils, die Vorderkante liegt am Rohling.
    let w = p.bolster_width_mm.max(6.0);
    let x1 = w;
    let y_top0 = b.spine_offset_mm;
    let y_bot0 = -b.edge_offset_mm.abs();
    // Die Hinterkante folgt dem Holz - mit der Wand um die Angel herum.
    // Sonst entstuende zwischen Bastion und Holz ein Absatz.
    let y_top1 = spine_mit_wand(b, p, x1, b.tang_length_mm);
    let y_bot1 = bauch_mit_wand(b, p, x1, b.tang_length_mm);
    vec![
        (0.0, y_top0),
        (x1, y_top1),
        (x1, y_bot1),
        (0.0, y_bot0),
    ]
}

pub fn ferrule_profile(b: &BladeBlank, p: &HandleParams) -> Vec<Point> {
    // Die Zwinge ist ein Mantel ueber dem vorderen Griffende. Ihre
    // Kontur folgt dem Griffprofil, damit sie bündig anliegt.
    // 0,1 mm kuerzer als das Holzende - vermeidet, dass die Flächen
    // exakt uebereinanderliegen und im Renderer flimmern.
    let w = (p.ferrule_width_mm.max(4.0) - 0.1).max(3.0);
    let steps = 14;
    let mut pts: Vec<Point> = Vec::new();
    pts.push((0.0, -b.edge_offset_mm.abs()));
    pts.push((0.0, b.spine_offset_mm));
    for i in 0..=steps {
        let x = w * i as f64 / steps as f64;
        pts.push((x, spine_mit_wand(b, p, x, b.tang_length_mm)));
    }
    for i in (0..=steps).rev() {
        let x = w * i as f64 / steps as f64;
        pts.push((x, bauch_mit_wand(b, p, x, b.tang_length_mm)));
    }
    pts
}

// ---------------------------------------------------------------------------
// Gesamtergebnis
// ---------------------------------------------------------------------------

/// Empfohlene Griffbauart fuer einen Rohling.
pub fn recommend_style(b: &BladeBlank) -> HandleStyle {
    if b.has_bolster || b.tang_style == crate::model::TangStyle::Partial {
        HandleStyle::PartialTangBolster
    } else if b.tang_style == crate::model::TangStyle::Hidden {
        HandleStyle::OnePiece
    } else if b.tang_thickness_mm < 3.0 || b.tang_length_mm > 135.0 {
        HandleStyle::OnePiece
    } else {
        HandleStyle::FullTangTwoScale
    }
}

/// Schalendicke und Gesamtdicke des Griffes.
pub fn schalendicken(b: &BladeBlank, p: &HandleParams) -> (f64, f64) {
    // Bei zweigeteilten Griffen sitzen zwei Schalen um die Angel herum,
    // bei Ein-Stueck- und Zwingengriffen ist der Holzkoerper ein Block.
    let two_part = p.style.has_two_scales() || p.style == HandleStyle::Ferrule;
    let scale_t = match p.scale_thickness_mm {
        Some(v) if v > 0.0 => v,
        _ => {
            if two_part {
                (p.target_thickness_mm - b.tang_thickness_mm) / 2.0
            } else {
                p.target_thickness_mm
            }
        }
    };
    let max_scale = if two_part { 10.0 } else { 60.0 };
    let scale_t = (clamp(scale_t, 2.5, max_scale) * 100.0).round() / 100.0;
    let total_t = if two_part {
        ((b.tang_thickness_mm + 2.0 * scale_t) * 100.0).round() / 100.0
    } else {
        scale_t
    };
    (scale_t, total_t)
}

/// Die Ringe des Griffkoerpers ohne Verjuengung.
///
/// FreeCAD rechnet den Griff als Loft durch ebene Schnitte. Eine
/// Verjuengung wuerde die Schnitte in sich verdrehen und den Loft
/// unbrauchbar machen - der Loft braucht ebene Ringe. Die Verjuengung
/// wird dort stattdessen als Schnitt mit einem Keil gerechnet.
pub fn griff_ringe_ebene(
    b: &BladeBlank,
    p: &HandleParams,
) -> Vec<Vec<Vec<(f64, f64, f64)>>> {
    let mut p2 = p.clone();
    p2.section.taper_mm = 0.0;
    let (outline, _) = build_outline(b, &p2);
    let tang = tang_profile(b, SAMPLE_STEP);
    let recess = if p2.glue_gap_mm != 0.0 {
        offset_polygon(&tang, -p2.glue_gap_mm)
    } else {
        tang.clone()
    };
    let (scale_t, total_t) = schalendicken(b, &p2);
    build_bodies(b, &p2, &outline, &recess, scale_t, total_t, 3)
}

/// Die Hoehe der Griffkontur an der Stelle x: (unten, oben).
///
/// Wird gebraucht, um zu sehen, wo eine Bohrung in das Holz eintritt.
pub fn kontur_hoehe(geo: &HandleGeometry, x: f64) -> (f64, f64) {
    let mut lo = f64::MAX;
    let mut hi = f64::MIN;
    let n = geo.outline.len();
    for i in 0..n {
        let (x1, y1) = geo.outline[i];
        let (x2, y2) = geo.outline[(i + 1) % n];
        if (x1 - x).abs() < 1e-9 {
            lo = lo.min(y1);
            hi = hi.max(y1);
        }
        if (x1 - x) * (x2 - x) < 0.0 {
            let t = (x - x1) / (x2 - x1);
            let y = y1 + t * (y2 - y1);
            lo = lo.min(y);
            hi = hi.max(y);
        }
    }
    if lo > hi {
        (0.0, 0.0)
    } else {
        (lo, hi)
    }
}

/// Das Ende des Griffes in x.
pub fn griff_ende(b: &BladeBlank, p: &HandleParams) -> f64 {
    let base_len = match p.handle_length_mm {
        Some(v) if v > 0.0 => v.max(b.tang_length_mm),
        _ => b.tang_length_mm,
    };
    base_len + p.butt_overhang_mm.max(0.0)
}

/// Die Bauchlinie (Unterkante) des Griffes an der Stelle x.
pub fn bauchlinie(b: &BladeBlank, p: &HandleParams, x: f64) -> f64 {
    belly_curve(b, p, x, griff_ende(b, p))
}

/// Die Rueckenlinie (Oberkante) des Griffes an der Stelle x.
pub fn rueckenlinie(b: &BladeBlank, p: &HandleParams, x: f64) -> f64 {
    spine_curve(b, p, x, griff_ende(b, p))
}

/// Die halbe Breite der Angel an der Stelle x.
pub fn angel_breite(b: &BladeBlank, x: f64) -> f64 {
    tang_half_width(b, x)
}

/// Die empfohlene Groesse des Holzleisten fuer den Griff.
///
/// Der Rohling wird mit einem kleinen Uebermass angesetzt: der Holzschnitter
/// schneibt grob vor und schleift dann auf die Schablone. Bei zwei
/// Schalen ist die Angel aus Stahl - das Holz sind nur die beiden Schalen.
pub fn zuschnitt(
    outline: &[(f64, f64)],
    params: &HandleParams,
    scale_t: f64,
    total_t: f64,
) -> (f64, f64, f64) {
    /// Uebermass zum Schleifen, je Seite
    const SCHLEIF_ALLOWANCE: f64 = 2.0;
    /// Uebermass zum Hobeln, je Seite
    const HOBEL_ALLOWANCE: f64 = 1.0;

    let x_min = outline.iter().map(|q| q.0).fold(f64::MAX, f64::min);
    let x_max = outline.iter().map(|q| q.0).fold(f64::MIN, f64::max);
    let y_min = outline.iter().map(|q| q.1).fold(f64::MAX, f64::min);
    let y_max = outline.iter().map(|q| q.1).fold(f64::MIN, f64::max);

    let laenge = (x_max - x_min) + 2.0 * SCHLEIF_ALLOWANCE;
    let hoehe = (y_max - y_min) + 2.0 * SCHLEIF_ALLOWANCE;
    let holz_dicke = if params.style.has_two_scales() {
        // Nur die beiden Schalen sind Holz - die Angel dazwischen ist Stahl
        2.0 * scale_t
    } else {
        total_t
    } + 2.0 * HOBEL_ALLOWANCE;
    (laenge, hoehe, holz_dicke)
}

/// Erzeugt die komplette Griffgeometrie.
pub fn build_geometry(b: &BladeBlank, p: &HandleParams) -> HandleGeometry {
    let mut warnings: Vec<String> = Vec::new();
    let mut notes: Vec<String> = Vec::new();

    let (outline, outline_warnings) = build_outline(b, p);
    warnings.extend(outline_warnings);

    let tang = tang_profile(b, SAMPLE_STEP);
    let recess = if p.glue_gap_mm != 0.0 {
        offset_polygon(&tang, -p.glue_gap_mm)
    } else {
        tang.clone()
    };

    // Schalendicke
    let (scale_t, total_t) = schalendicken(b, p);

    if scale_t <= 2.6 && p.style.has_two_scales() {
        warnings.push(format!(
            "Die Schalendicke liegt bei nur {}. Unter 2,6 mm reißt Holz beim \
             Anziehen der Vorsteckbolzen aus - dickeres Holz oder Epoxy- und Reibnieten-\
             Verbindung wählen.",
            de_mm(scale_t, 1)
        ));
    }

    // Holz ausserhalb der Angel
    let wood_outside = scale_t - b.tang_thickness_mm - p.glue_gap_mm;
    if p.style.has_two_scales() && wood_outside < 1.5 {
        warnings.push(format!(
            "Außerhalb der Angel bleiben nur {} Holz. Unter 1,5 mm ist die \
             Schale zu schwach - schmalere Angel, dünneres Holz oder Epoxy- statt Stift-\
             verbindung in Betracht ziehen.",
            de_mm(wood_outside, 1)
        ));
    }

    // Die Angel muss die Klinge am Heft aufnehmen.
    //
    // Die Klinge ist am Heft so hoch wie der Rohling, die Angel läuft im
    // Griff weiter. Ist die Angel deutlich niedriger als die Klinge, ragt
    // der Stahl über die Angel hinaus - die Klinge hängt dann nur an einem
    // schmalen Streifen. Bei zwei Schalen und bei der Zwinge ist das ein
    // Fehler; bei Bastion und Ein-Stück-Griff ist eine schmale Angel
    // gewollt, weil sie im Metall bzw. als Nase im Holz liegt.
    let klingen_hoehe = b.spine_offset_mm + b.edge_offset_mm.abs();
    let angel_hoehe = 2.0 * tang_half_width(b, 0.0);
    let angel_durch_holz = p.style.has_two_scales() || p.style == HandleStyle::Ferrule;
    if angel_durch_holz && klingen_hoehe - angel_hoehe > 1.0 {
        let ueberhang = (klingen_hoehe - angel_hoehe) / 2.0;
        warnings.insert(
            0,
            format!(
                "Die Angel ist nur {} hoch, die Klinge am Heft {}: der Stahl ragt je {} über \
                 die Angel hinaus, die Klinge hängt nur an einem schmalen Streifen. Für eine \
                 durchgehende Angel die Angel-Breite auf {} setzen - der Griff passt sich dann \
                 an.",
                de_mm(angel_hoehe, 1),
                de_mm(klingen_hoehe, 1),
                de_mm(ueberhang, 1),
                de_mm(klingen_hoehe, 1)
            ),
        );
    }

    let pins = plan_pins(b, p);

    let bolster = match p.style {
        HandleStyle::PartialTangBolster => Some(bolster_profile(b, p)),
        _ => None,
    };
    let ferrule = match p.style {
        HandleStyle::Ferrule => Some(ferrule_profile(b, p)),
        _ => None,
    };

    let y_values: Vec<f64> = outline.iter().map(|&(_, y)| y).collect();
    let height = ((y_values.iter().cloned().fold(f64::MIN, f64::max)
        - y_values.iter().cloned().fold(f64::MAX, f64::min))
        * 100.0)
        .round()
        / 100.0;
    let x_values: Vec<f64> = outline.iter().map(|&(x, _)| x).collect();
    let length = ((x_values.iter().cloned().fold(f64::MIN, f64::max)
        - x_values.iter().cloned().fold(f64::MAX, f64::min))
        * 100.0)
        .round()
        / 100.0;

    let wood_start = match p.style {
        HandleStyle::PartialTangBolster => p.bolster_width_mm,
        HandleStyle::Ferrule => p.ferrule_width_mm,
        _ => 0.0,
    };
    let wood_end = b.tang_length_mm;

    // Der Griff reicht ueber die Angel hinaus
    if length > b.tang_length_mm + 3.0 {
        warnings.push(format!(
            "Der Griff reicht {} über die Angel hinaus. Dahinter hält nur noch der \
             Kleber die Schalen zusammen - das ist die Bauart eines Partial Tangs. Ein \
             längerer Rohling wäre robuster, die Angel läuft dann bis zum Griffende.",
            de_mm(length - b.tang_length_mm, 0)
        ));
    }

    let grip_len = length - wood_start;
    if grip_len < 85.0 {
        warnings.push(format!(
            "Der nutzbare Griffriff misst nur {}. Das ist kurz - für ein \
             Jagdmesser mit ganzer Hand zu knapp.",
            de_mm(grip_len, 0)
        ));
    } else if grip_len > 145.0 {
        warnings.push(format!(
            "Der Griff ist mit {} länger als bei den meisten Messern. \
             Ganzschalengriff an den Händen abgleichen.",
            de_mm(grip_len, 0)
        ));
    }

    match p.style {
        HandleStyle::FullTangTwoScale => notes.push(
            "Full Tang: die Angel läuft durch den gesamten Griff. Beide Griffschalen sind \
             spiegelbildlich, die Bohrungen für die Vorsteckbolzen gehen durch die Angel."
                .into(),
        ),
        HandleStyle::PartialTangBolster => notes.push(
            "Partial Tang mit Bastion: die Bastion überbrückt den vorderen Griffteil. Die \
             Bastion wird mit der Angel verschweißt, der Holzteil wird mit Reibnieten und \
             Epoxy fixiert."
                .into(),
        ),
        HandleStyle::OnePiece => notes.push(
            "Ein-Stück-Griff: die Angel wird als Nut in das Holz eingelassen. Nut mit \
             0,03 - 0,05 mm Spiel, sonst klebt das Holz an der Angel fest und spannt beim \
             Trocknen."
                .into(),
        ),
        HandleStyle::Ferrule => notes.push(
            "Griff mit Zwinge: die Zwinge klemmt das Holz am Rohling und fasst die \
             Angelenden. Die Angel wird eingelassen, die Zwinge verhindert das Auseinanderziehen \
             des Holzes."
                .into(),
        ),
    }

    // Materialvolumen einer Schale: Flaeche des Profils * Schalendicke
    let area = polygon_area(&outline);
    let volume = area * scale_t;

    // Die Ringe des Griffkoerpers fuer den 3D-Export
    let bodies = build_bodies(b, p, &outline, &recess, scale_t, total_t, 6);

    let mut metrics = vec![
        ("grifffaenge_mm".to_string(), round1(length)),
        ("griffhoehe_mm".to_string(), round1(height)),
        ("schalendicke_mm".to_string(), round1(scale_t)),
        ("gesamtdicke_mm".to_string(), round1(total_t)),
        ("angel_laenge_mm".to_string(), round1(b.tang_length_mm)),
        ("wandstaerke_mm".to_string(), round1(p.wall_min_mm)),
        ("holz_ausserhalb_angel_mm".to_string(), round1(wood_outside)),
        ("schalenvolumen_mm3".to_string(), volume.round()),
        (
            "anker_anzahl".to_string(),
            pins.iter().filter(|p| p.kind == "through").count() as f64,
        ),
    ];

    // Der Zuschnitt: so gross muss der Holzleisten sein
    let (z_laenge, z_hoehe, z_dicke) = zuschnitt(&outline, p, scale_t, total_t);
    metrics.push(("zuschnitt_laenge_mm".to_string(), round1(z_laenge)));
    metrics.push(("zuschnitt_hoehe_mm".to_string(), round1(z_hoehe)));
    metrics.push(("zuschnitt_dicke_mm".to_string(), round1(z_dicke)));

    HandleGeometry {
        outline,
        tang_profile: tang,
        recess,
        pins,
        scale_thickness_mm: scale_t,
        total_thickness_mm: total_t,
        handle_length_mm: length,
        handle_height_mm: height,
        wood_start_x_mm: wood_start,
        wood_end_x_mm: wood_end,
        bolster_profile: bolster,
        ferrule_profile: ferrule,
        bodies,
        metrics,
        warnings,
        notes,
    }
}

/// Pfad der Angel-Aussparung, von der unteren Vorderkante ueber die
/// Spitze zur oberen Vorderkante.
///
/// Wird fuer den Ein-Stueck-Griff gebraucht: die Nut fuer die Angel wird
/// in den Boden des Holzkoerpers gefraest und ist dort vorne offen.
///
/// Der Weg laeuft vom unteren Vorderkantenpunkt der Nut ueber die
/// Schlitzspitze zum oberen. Das Klebespiel ist nach aussen versetzt,
/// deshalb wird der Weg an der Vorderkante auf x = 0 beschnitten - sonst
/// stuende die Nut einen Spalt vor dem Griff.
pub fn slot_path(recess: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut path: Vec<(f64, f64)> = clip_x_min(recess, 0.0);
    path.reverse();
    path
}

/// Flaeche eines Polygons (positiv = gegen den Uhrzeigersinn).
pub fn polygon_area(poly: &[(f64, f64)]) -> f64 {
    let n = poly.len();
    if n < 3 {
        return 0.0;
    }
    let mut s = 0.0;
    for i in 0..n {
        let (x1, y1) = poly[i];
        let (x2, y2) = poly[(i + 1) % n];
        s += x1 * y2 - x2 * y1;
    }
    (s / 2.0).abs()
}

fn round1(v: f64) -> f64 {
    (v * 10.0).round() / 10.0
}

#[cfg(test)]
mod tests {
    use super::*;

    fn default_blank() -> BladeBlank {
        BladeBlank {
            tang_length_mm: 110.0,
            tang_width_mm: 6.5,
            tang_thickness_mm: 6.5,
            spine_offset_mm: 8.0,
            edge_offset_mm: 7.5,
            ..Default::default()
        }
    }

    #[test]
    fn die_angel_steckt_nie_aus_dem_holz() {
        // Rund um die Angel muss Holz bleiben - bei jeder Bauart und jeder
        // Angelbreite. Sonst steht der Stahl aus dem Griff heraus.
        for tang_w in [3.0_f64, 6.5, 12.0, 20.0, 26.0] {
            for style in [
                HandleStyle::FullTangTwoScale,
                HandleStyle::PartialTangBolster,
                HandleStyle::OnePiece,
                HandleStyle::Ferrule,
            ] {
                let mut b = BladeBlank::default();
                b.tang_width_mm = tang_w;
                let mut p = HandleParams::default();
                p.style = style;
                let geo = build_geometry(&b, &p);
                let x = geo.handle_length_mm * 0.45;
                if let Some(cs) = cross_section(&geo, &b, x) {
                    assert!(
                        cs.wall_spine > 0.0 && cs.wall_belly > 0.0,
                        "{style:?} bei {tang_w} mm Angel: Wand {}/{} mm",
                        cs.wall_spine,
                        cs.wall_belly
                    );
                }
            }
        }
    }

    #[test]
    fn schmale_angel_wird_gemahnt() {
        // Der Rohling mit 26 mm Klingenhoehe und 6,5 mm Angel ist der
        // Standardfall: die Angel nimmt die Klinge nicht auf. Das muss
        // im Plan stehen.
        let b = BladeBlank::default();
        let p = HandleParams::default();
        let g = build_geometry(&b, &p);
        assert!(
            g.warnings.iter().any(|w| w.contains("Angel ist nur")),
            "Es fehlt der Hinweis auf die zu schmale Angel: {:?}",
            g.warnings
        );

        // Mit der Angel in der Hoehe der Klinge verschwindet der Hinweis.
        let mut b2 = BladeBlank::default();
        b2.tang_width_mm = b2.spine_offset_mm + b2.edge_offset_mm.abs();
        let g2 = build_geometry(&b2, &p);
        assert!(
            !g2.warnings.iter().any(|w| w.contains("Angel ist nur")),
            "Der Hinweis muss verschwinden: {:?}",
            g2.warnings
        );
    }

    #[test]
    fn tang_width_starts_correct() {
        let b = default_blank();
        assert!((tang_half_width(&b, 0.0) - 3.25).abs() < 1e-9);
        assert!((tang_half_width(&b, 110.0) - 0.0).abs() < 1e-9);
    }

    #[test]
    fn outline_respects_wall_thickness() {
        let b = default_blank();
        let p = HandleParams::default();
        let (outline, _) = build_outline(&b, &p);
        assert!(outline.len() > 50);
        // Oberhalb der Angel muss immer >= wall_min Abstand bleiben
        for &x in &[10.0, 40.0, 55.0, 80.0] {
            let w = tang_half_width(&b, x);
            let top = outline
                .iter()
                .filter(|&&(px, _)| (px - x).abs() < 1.0)
                .map(|&(_, y)| y)
                .fold(f64::MIN, f64::max);
            assert!(top >= w + p.wall_min_mm - 1e-6, "Wand zu duenn bei x={x}");
        }
    }

    #[test]
    fn two_through_pins_on_short_handle() {
        let b = default_blank();
        let p = HandleParams::default();
        let pins = plan_pins(&b, &p);
        let through: Vec<_> = pins.iter().filter(|p| p.kind == "through").collect();
        assert_eq!(through.len(), 2);
        assert!(pins.iter().any(|p| p.kind == "friction"));
        assert!(pins.iter().any(|p| p.kind == "lanyard"));
    }

    #[test]
    fn long_handle_gets_middle_pin() {
        let mut b = default_blank();
        b.tang_length_mm = 160.0;
        let p = HandleParams::default();
        let pins = plan_pins(&b, &p);
        let through = pins.iter().filter(|p| p.kind == "through").count();
        assert!(through >= 3, "erwartet mindestens 3 Vorsteckbolzen, war {through}");
    }

    #[test]
    fn scale_thickness_derived_from_target() {
        let b = default_blank();
        let mut p = HandleParams::default();
        p.target_thickness_mm = 22.0;
        let g = build_geometry(&b, &p);
        assert!((g.scale_thickness_mm - 7.75).abs() < 0.01);
        assert!((g.total_thickness_mm - 22.0).abs() < 0.01);
    }

    #[test]
    fn ergonomic_profile_is_fuller_than_the_heel() {
        let b = default_blank();
        let mut p = HandleParams::default();
        p.profile = ProfileStyle::Ergonomic;
        p.grip_height_mm = 26.0;
        let (outline, _) = build_outline(&b, &p);

        // Hoehe direkt am Heft und in der Griffzone
        let height_at = |x: f64| -> f64 {
            let mut top = f64::MIN;
            let mut bot = f64::MAX;
            for &(px, py) in &outline {
                if (px - x).abs() < 1.5 {
                    top = top.max(py);
                    bot = bot.min(py);
                }
            }
            top - bot
        };
        let front = height_at(1.0);
        let grip = height_at(50.0);
        assert!(
            grip > front + 6.0,
            "Der ergonomische Griff muss fueller sein als das Heft: {grip} gegen {front}"
        );
        // Die Angel muss weiterhin Platz haben
        let w = tang_half_width(&b, 50.0);
        let mut top = f64::MIN;
        for &(px, py) in &outline {
            if (px - 50.0).abs() < 1.5 {
                top = top.max(py);
            }
        }
        assert!(top >= w + p.wall_min_mm - 1e-6, "Wand zu duenn");
    }

    #[test]
    fn blade_profile_stays_close_to_the_heel() {
        let b = default_blank();
        let mut p = HandleParams::default();
        p.profile = ProfileStyle::Blade;
        let (outline, _) = build_outline(&b, &p);
        let height_at = |x: f64| -> f64 {
            let mut top = f64::MIN;
            let mut bot = f64::MAX;
            for &(px, py) in &outline {
                if (px - x).abs() < 1.5 {
                    top = top.max(py);
                    bot = bot.min(py);
                }
            }
            top - bot
        };
        let front = height_at(1.0);
        let grip = height_at(50.0);
        assert!(
            (grip - front).abs() < 4.0,
            "Das Rohling-Profil darf nicht zur Wurst werden: {grip} gegen {front}"
        );
    }

    #[test]
    fn handle_length_can_exceed_the_tang() {
        let mut b = default_blank();
        b.tang_length_mm = 60.0;
        let mut p = HandleParams::default();
        p.handle_length_mm = Some(120.0);
        let g = build_geometry(&b, &p);
        assert!(
            g.handle_length_mm > 115.0,
            "Der Griff muss laenger sein als die kurze Angel: {}",
            g.handle_length_mm
        );
        assert!(
            g.warnings.iter().any(|w| w.contains("über die Angel hinaus")),
            "Es muss auf den Partial Tang hinweisen: {:?}",
            g.warnings
        );
        // Die Angel selbst bleibt unveraendert
        assert_eq!(g.wood_end_x_mm, 60.0);
    }

    #[test]
    fn handle_length_defaults_to_the_tang() {
        let b = default_blank();
        let p = HandleParams::default();
        let g = build_geometry(&b, &p);
        // Ohne eigene Angabe endet der Griff bei der Angel - das gerundete
        // Griffende liegt zusaetzlich dahinter.
        assert!(
            g.handle_length_mm < b.tang_length_mm + 12.0,
            "Der Griff darf nicht ueber die Angel hinausgehen: {}",
            g.handle_length_mm
        );
        assert_eq!(g.wood_end_x_mm, b.tang_length_mm);
    }

    #[test]
    fn grip_circumference_grows_with_the_height() {
        let klein = grip_circumference(20.0, 22.0, 2.5);
        let gross = grip_circumference(30.0, 22.0, 2.5);
        assert!(klein < gross, "{klein} < {gross}");
        // Ein Rechteck 20 x 22 ohne Rundung hat den Umfang 84 mm
        assert!((grip_circumference(20.0, 22.0, 0.0) - 84.0).abs() < 0.01);
        // Gerundete Ecken verkuerzen den Umfang (8r > 2*pi*r)
        assert!(grip_circumference(20.0, 22.0, 2.5) < 84.0);
    }

    #[test]
    fn ergonomics_flags_a_too_thin_grip() {
        let b = default_blank();
        let mut p = HandleParams::default();
        p.hand_size = crate::model::HandSize::Large;
        let g = build_geometry(&b, &p);
        let e = ergonomics_for(&b, &p, &g);
        assert!(
            !e.warnings.is_empty(),
            "Ein duenner Griff muss bei grosser Hand auffallen"
        );
    }

    #[test]
    fn outline_stays_a_simple_polygon_over_the_whole_range() {
        // Die Kontur darf sich nie selbst schneiden - sonst ist der
        // Zuschnitt unbrauchbar. Der Test streift den kompletten Bereich der
        // einstellbaren Werte.
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
                                let mut b = default_blank();
                                b.tang_length_mm = tang_len;
                                b.tang_width_mm = tang_w;
                                b.tang_thickness_mm = tang_t;
                                b.spine_offset_mm = 8.0;
                                b.edge_offset_mm = 7.5;
                                let mut p = HandleParams::default();
                                p.style = style;
                                p.profile = profile;
                                p.wall_min_mm = wall;
                                let (outline, _) = build_outline(&b, &p);
                                assert!(
                                    crate::mesh::is_simple_polygon(&outline),
                                    "Kontur schneidet sich selbst: Angel {tang_len}x{tang_w}x{tang_t},                                      Wand {wall}, Stil {style:?}, Profil {profile:?}"
                                );
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn pins_are_ordered_and_on_the_tang() {
        for tang_len in [25.0_f64, 45.0, 110.0, 190.0] {
            for tang_t in [2.0_f64, 6.5, 11.0] {
                let mut b = default_blank();
                b.tang_length_mm = tang_len;
                b.tang_thickness_mm = tang_t;
                let p = HandleParams::default();
                let pins = plan_pins(&b, &p);
                let through: Vec<f64> = pins
                    .iter()
                    .filter(|x| x.kind == "through")
                    .map(|x| x.x_mm)
                    .collect();
                for w in through.windows(2) {
                    assert!(w[1] > w[0], "Bohrungen nicht aufsteigend: {through:?}");
                }
                for pin in &pins {
                    // Das Lanyardloch darf hinter der Angel liegen - es
                    // geht ja gerade an der Angel vorbei.
                    if pin.kind == "lanyard" {
                        continue;
                    }
                    assert!(
                        pin.x_mm >= 0.0 && pin.x_mm <= tang_len + 0.01,
                        "Bohrung ausserhalb der Angel: {:?} bei {}",
                        pin.label, tang_len
                    );
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Ergonomie-Empfehlung
// ---------------------------------------------------------------------------

/// Empfehlung fuer Griffhoehe und Griffriff nach Handgroesse.
#[derive(Clone, Debug)]
pub struct Ergonomics {
    pub grip_height_mm: f64,
    pub grip_length_mm: f64,
    pub circumference_mm: f64,
    pub note: String,
    pub warnings: Vec<String>,
}

/// Umfang des geschlossenen Griffquerschnitts.
///
/// Der Querschnitt ist ein Rechteck (Hoehe x Dicke) mit gerundeten Ecken.
/// Der Umfang ergibt sich aus den geraden Stuecken plus den Viertelkreisen.
pub fn grip_circumference(height_mm: f64, thickness_mm: f64, radius_mm: f64) -> f64 {
    let r = radius_mm.min(height_mm / 2.2).min(thickness_mm / 2.2);
    2.0 * (height_mm - 2.0 * r) + 2.0 * (thickness_mm - 2.0 * r) + 2.0 * std::f64::consts::PI * r
}

/// Rechnet die passende Griffhoehe fuer eine Handgroesse.
pub fn ergonomics_for(
    b: &BladeBlank,
    p: &HandleParams,
    geo: &HandleGeometry,
) -> Ergonomics {
    let target = p.hand_size.target_circumference();
    let thickness = geo.total_thickness_mm.max(1.0);
    let radius = p.edge_break_mm.clamp(0.4, 4.0);

    // 2*h + 2*t - 8*r + 2*pi*r = Umfang  ->  nach h aufloesen
    let h = ((target - 2.0 * thickness + 8.0 * radius - 2.0 * std::f64::consts::PI * radius) / 2.0)
        .max(12.0);
    let actual = grip_circumference(geo.handle_height_mm, thickness, radius);
    let grip_length = p.hand_size.min_grip_length().max(b.tang_length_mm);

    let mut warnings: Vec<String> = Vec::new();
    if actual < target - 6.0 {
        warnings.push(format!(
            "Der Griffumfang liegt bei rund {:.0} mm, fuer eine {} waere etwa {:.0} mm \
             richtig. Griffhoehe auf {:.0} mm erhoehen.",
            actual,
            p.hand_size.label().to_lowercase(),
            target,
            h
        ));
    } else if actual > target + 12.0 {
        warnings.push(format!(
            "Der Griffumfang liegt bei rund {:.0} mm und ist damit {}.",
            actual,
            if actual > target + 20.0 {
                "deutlich zu dick - die Hand umfasst ihn nicht mehr"
            } else {
                "etwas dick fuer diese Handgroesse"
            }
        ));
    }
    let grip_now = geo.handle_length_mm - geo.wood_start_x_mm;
    if grip_now < p.hand_size.min_grip_length() - 6.0 {
        warnings.push(format!(
            "Der Griffriff misst nur {:.0} mm. Fuer eine {} sollten es mindestens {:.0} mm \
             sein - laengerer Rohling oder mehr Ueberstand am Griffende.",
            grip_now,
            p.hand_size.label().to_lowercase(),
            p.hand_size.min_grip_length()
        ));
    }

    Ergonomics {
        grip_height_mm: (h * 10.0).round() / 10.0,
        grip_length_mm: grip_length,
        circumference_mm: (actual * 10.0).round() / 10.0,
        note: format!(
            "Umfang {:.0} mm (Ziel {:.0} mm), empfohlene Griffhoehe {:.0} mm, \
             Griffriff mindestens {:.0} mm.",
            actual, target, h, p.hand_size.min_grip_length()
        ),
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Querschnitt: Ringe fuer den Griffkoerper
// ---------------------------------------------------------------------------

/// Wie viele Schnitte es im geraden Teil des Griffes gibt.
///
/// Mehr Schnitte treffen die Komfortrundung besser - sie laeuft namlich
/// ueber die ganze Dicke, nicht nur ueber die Fase am Rand.
const MITTLERE_SCHNITTE: usize = 4;

/// Der Verjuengungsfaktor an der Stelle x.
///
/// 1 = keine Verjuengung, kleinere Werte = die Dicke nimmt zum Griffende
/// hin ab. Die Verjuengung greift erst hinter der vorderen Haelfte und
/// laeuft weich zum Ende aus.
pub fn taper_faktor(
    spec: &SectionSpec,
    x_start: f64,
    x_span: f64,
    span: f64,
    x: f64,
) -> f64 {
    if spec.taper_mm <= 0.0 {
        return 1.0;
    }
    let t = ((x - x_start) / x_span.max(1e-6)).clamp(0.0, 1.0);
    let factor = (spec.taper_mm / span.max(1.0)).min(0.6);
    1.0 - factor * smoothstep((t - 0.35) / 0.65)
}

/// Ein Ring des Griffkoerpers: geschlossener Umriss auf einer Hoehe z.
pub type Ring = Vec<(f64, f64, f64)>;

/// Formt einen Konturpunkt auf die Hoehe z.
///
/// `g` ist der Stauchanteil (0 = unveraendert, 1 = voll), `keel_g` der
/// Anteil des Laengsgrats, `taper` der Verjuengungsfaktor fuer z.
///
/// `k_bauch` und `k_ruecken` sind die Anteile der Komfortrundung, die
/// uebrig bleiben, damit die Wand um die Angel herum erhalten bleibt.
fn shape_point(
    p: (f64, f64),
    top: f64,
    bot: f64,
    spec: &SectionSpec,
    g: f64,
    keel_g: f64,
    taper: f64,
    z: f64,
    k_bauch: f64,
    k_ruecken: f64,
) -> (f64, f64, f64) {
    let (x, y) = p;
    let height = (top - bot).max(0.5);
    // Lage im Profil: 0 = Bauchseite, 1 = Rueckenseite
    let u = ((y - bot) / height).clamp(0.0, 1.0);
    let w_top = smoothstep((u - 0.55) / 0.45);
    let w_bot = smoothstep((0.45 - u) / 0.45);
    // Die Bauchseite wird nach oben, der Ruecken nach unten gezogen
    let heben = g * spec.belly_round_mm * w_bot * k_bauch - keel_g * spec.keel_mm * w_bot;
    let senken = g * spec.spine_round_mm * w_top * k_ruecken;
    let y2 = y + heben - senken;
    (x, y2, z * taper)
}

/// Baut die Ringe eines Griffkoerpers von oben nach unten.
///
/// * `outline` - die Kontur in x-y
/// * `slot_path` - optional der Pfad der Angel-Nut (wird nicht gestaucht)
/// * `z_top`, `z_bottom` - der z-Bereich des Koerpers
/// * `span` - die halbe Dicke, bis zu der der Querschnitt gestaucht wird
/// * `radius` - die Kantenrundung an den Flaechen
pub fn body_rings(
    b: &BladeBlank,
    p: &HandleParams,
    outline: &[(f64, f64)],
    slot_path: Option<&[(f64, f64)]>,
    z_top: f64,
    z_bottom: f64,
    span: f64,
    radius_top: f64,
    radius_bottom: f64,
    keel_enabled: bool,
    steps: usize,
) -> Vec<Ring> {
    let spec = p.section;
    let steps = steps.max(2);
    let total = (z_top - z_bottom).max(0.1);
    let radius_top = radius_top.clamp(0.0, total * 0.45);
    let radius_bottom = radius_bottom.clamp(0.0, total * 0.45);

    // Verlauf der Kontur ueber x fuer die Einordnung der Punkte
    let x_end = outline
        .iter()
        .map(|p| p.0)
        .fold(f64::MIN, f64::max)
        .max(1.0);
    let x_start = outline
        .iter()
        .map(|p| p.0)
        .fold(f64::MAX, f64::min);
    let x_span = (x_end - x_start).max(1.0);

    let top_at = |x: f64| spine_curve(b, p, x, x_end);
    let bot_at = |x: f64| belly_curve(b, p, x, x_end);

    // Verjuengung: die Dicke nimmt zum Griffende hin ab
    let taper_at = |x: f64| -> f64 { taper_faktor(&spec, x_start, x_span, span, x) };

    // Die Wand um die Angel herum: die Komfortrundung darf sie nicht
    // auffressen, sonst steht die Angel aus dem Holz heraus. Das gilt
    // ueberall dort, wo die Angel im Holz liegt - bei der Bastion ohne
    // Nut, beim Ein-Stueck- und beim Zwingengriff mit Nut. Bei zwei
    // Schalen sitzt die Angel in der Fuge dazwischen.
    let wand_staerke = p.wall_min_mm.max(0.8);
    let (mut k_bauch, mut k_ruecken) = (1.0f64, 1.0f64);
    let angel_im_holz = slot_path.is_some() || p.style == HandleStyle::PartialTangBolster;
    if angel_im_holz {
        for &pt in outline.iter() {
            let x = pt.0;
            let w = tang_half_width(b, x);
            if w <= 0.0 {
                continue;
            }
            let bot = belly_curve(b, p, x, x_end);
            let top = spine_curve(b, p, x, x_end);
            let height = (top - bot).max(0.5);
            let u = ((pt.1 - bot) / height).clamp(0.0, 1.0);
            let grenze = w + wand_staerke;
            if u <= 0.001 && spec.belly_round_mm > 0.01 {
                // Die Bauchseite darf hoechstens bis zur Wand angehoben werden
                let erlaubt = (-grenze - pt.1).max(0.0);
                k_bauch = k_bauch.min(erlaubt / spec.belly_round_mm);
            }
            if u >= 0.999 && spec.spine_round_mm > 0.01 {
                let erlaubt = (pt.1 - grenze).max(0.0);
                k_ruecken = k_ruecken.min(erlaubt / spec.spine_round_mm);
            }
        }
        k_bauch = k_bauch.clamp(0.0, 1.0);
        k_ruecken = k_ruecken.clamp(0.0, 1.0);
    }

    // Stauchung: an den Flaechen voll, in der Mitte ueber dem Griff nichts.
    // Beim flachen Querschnitt gibt es keine Stauchung - die Schalen
    // bleiben ueber die ganze Dicke gleich dick.
    let compression_at = |z: f64| -> f64 {
        if spec.shape == SectionShape::Flat {
            return 0.0;
        }
        let t = ((z.abs() - 0.0) / span.max(0.5)).clamp(0.0, 1.0);
        smoothstep(t)
    };
    // Der Laengsgrat sitzt in der Mitte der Dicke
    let keel_at = |z: f64| -> f64 {
        (1.0 - (z.abs() / span.max(0.5)).clamp(0.0, 1.0)).powf(1.5)
    };

    let mut rings: Vec<Ring> = Vec::new();

    // Die Schnitte von oben nach unten: die Fase am oberen Rand, der
    // gerade Teil in mehreren Schritten und die Fase am unteren Rand.
    //
    // Die Liste enthaelt keine doppelten Hoehen - ein Loft in FreeCAD
    // braucht eindeutige Schnitte, sonst wird das Volumen unbrauchbar.
    let mut schnitte: Vec<(f64, f64)> = Vec::new();
    // obere Fase: vom eingezogenen Rand bis zur vollen Kontur
    if radius_top > 1e-6 {
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            schnitte.push((z_top - radius_top * t, radius_top * (1.0 - t)));
        }
    } else {
        schnitte.push((z_top, 0.0));
    }
    // gerader Teil: mehrere Schnitte, damit die Komfortrundung sauber
    // getroffen wird
    let z_a = z_top - radius_top;
    let z_b = z_bottom + radius_bottom;
    if z_b > z_a + 1e-6 {
        for k in 1..=MITTLERE_SCHNITTE {
            let t = k as f64 / (MITTLERE_SCHNITTE + 1) as f64;
            schnitte.push((z_a + (z_b - z_a) * t, 0.0));
        }
    }
    // untere Fase: von der vollen Kontur bis zum eingezogenen Rand
    if radius_bottom > 1e-6 {
        for k in 0..=steps {
            let t = k as f64 / steps as f64;
            schnitte.push((z_bottom + radius_bottom - radius_bottom * t, radius_bottom * t));
        }
    } else {
        schnitte.push((z_bottom, 0.0));
    }

    for (z, inset) in schnitte {
        let base = if inset > 1e-6 {
            crate::mesh::offset(outline, inset)
        } else {
            outline.to_vec()
        };
        let g = compression_at(z);
        let kg = if spec.shape == SectionShape::Keel && keel_enabled {
            keel_at(z)
        } else {
            0.0
        };
        let ring: Ring = base
            .iter()
            .map(|&pt| {
                let x = pt.0;
                shape_point(pt, top_at(x), bot_at(x), &spec, g, kg, taper_at(x), z, k_bauch, k_ruecken)
            })
            .collect();
        rings.push(ring);
    }

    // Die Nut wird nicht gestaucht - sie muss exakt zur Angel passen.
    //
    // Die Nut sitzt in der Vorderkante: der Ring beginnt am unteren
    // Vorderrandpunkt, laeuft ueber die Nut (unten hinein, ueber die
    // Nutspitze, oben heraus) und dann weiter ueber die Aussenkontur.
    // Haengt man die Nut hinten an, schneidet sie die Kontur diagonal
    // durch - der Ring waere dann keine einfache Flaeche mehr.
    if let Some(sp) = slot_path {
        for ring in rings.iter_mut() {
            let z = ring[0].2;
            let shaped: Vec<(f64, f64, f64)> = ring
                .iter()
                .take(outline.len())
                .cloned()
                .collect();
            if shaped.len() < 2 {
                continue;
            }
            let mut merged: Vec<(f64, f64, f64)> = Vec::with_capacity(shaped.len() + sp.len());
            merged.push(shaped[0]);
            merged.extend(sp.iter().map(|&(x, y)| (x, y, z)));
            merged.extend(shaped[1..].iter().copied());
            *ring = merged;
        }
    }

    rings
}

/// Baut die Ringe der Griffkoerper je nach Bauart.
fn build_bodies(
    b: &BladeBlank,
    p: &HandleParams,
    outline: &[(f64, f64)],
    recess: &[(f64, f64)],
    scale_t: f64,
    total_t: f64,
    schritte: usize,
) -> Vec<Vec<Vec<(f64, f64, f64)>>> {
    let r = p.edge_break_mm.clamp(0.3, scale_t * 0.45);
    let mut bodies: Vec<Vec<Vec<(f64, f64, f64)>>> = Vec::new();
    // Die Nut fuer die Angel - dort, wo die Angel im Holz liegt
    let path = slot_path(recess);

    match p.style {
        HandleStyle::FullTangTwoScale => {
            // Zwei Schalen: die Innenflaechen bleiben flach (Klebeflaeche),
            // die Aussenkontur bekommt die Kantenrundung.
            // Der Laengsgrat sitzt an der Innenkante der Schalen - im
            // montierten Griff bilden beide Schalen zusammen den Grat.
            let upper = body_rings(b, p, outline, None, scale_t, 0.0, scale_t, r, 0.0, true, schritte);
            let lower = body_rings(b, p, outline, None, 0.0, -scale_t, scale_t, 0.0, r, true, schritte);
            let tang_t = b.tang_thickness_mm;
            bodies.push(translate_rings(&upper, tang_t / 2.0));
            bodies.push(translate_rings(&lower, -tang_t / 2.0));
        }
        HandleStyle::PartialTangBolster | HandleStyle::Ferrule => {
            // Der Zwingengriff laesst die Angel ein - dafuer braucht das
            // Holz die Nut. Bei der Bastion liegt die Angel im Metall,
            // dort bleibt das Holz massiv.
            let nut: Option<&[(f64, f64)]> = if p.style == HandleStyle::Ferrule {
                Some(path.as_slice())
            } else {
                None
            };
            let rings = body_rings(
                b,
                p,
                outline,
                nut,
                total_t / 2.0,
                -total_t / 2.0,
                total_t / 2.0,
                r * 1.5,
                r * 1.5,
                false,
                schritte,
            );
            bodies.push(rings);
        }
        HandleStyle::OnePiece => {
            // Der Ein-Stueck-Griff hat zusaetzlich die Nut fuer die Angel.
            let rings = body_rings(
                b,
                p,
                outline,
                Some(&path),
                total_t / 2.0,
                -total_t / 2.0,
                total_t / 2.0,
                r * 1.5,
                r * 1.5,
                true,
                schritte,
            );
            bodies.push(rings);
        }
    }
    bodies
}

/// Verschiebt alle Ringe um dz in z.
fn translate_rings(rings: &[Ring], dz: f64) -> Vec<Vec<(f64, f64, f64)>> {
    rings
        .iter()
        .map(|ring| ring.iter().map(|&(x, y, z)| (x, y, z + dz)).collect())
        .collect()
}


#[cfg(test)]
mod ring_tests {
    use super::*;

    #[test]
    fn rings_are_ordered_and_consistent() {
        let b = BladeBlank::default();
        let p = HandleParams::default();
        let (outline, _) = build_outline(&b, &p);
        let rings = body_rings(&b, &p, &outline, None, 7.75, 0.0, 7.75, 0.8, 0.0, false, 5);
        assert!(rings.len() >= 3, "zu wenige Ringe: {}", rings.len());

        let n = rings[0].len();
        for r in &rings {
            assert_eq!(r.len(), n, "die Ringe muessen gleich viele Punkte haben");
        }
        // Die Ringe muessen von oben nach unten laufen
        let mut last_z = f64::MAX;
        for r in &rings {
            let z = r.iter().map(|p| p.2).sum::<f64>() / n as f64;
            assert!(z <= last_z + 0.001, "die Ringe muessen fallen: {last_z} -> {z}");
            last_z = z;
        }
        // Die Deckflaechen werden als Faecker vom Schwerpunkt gefuellt -
        // das muss exakt die Flaeche der Kontur ergeben.
        for (label, ring) in [("oben", &rings[0]), ("unten", &rings[rings.len()-1])] {
            let poly: Vec<(f64,f64)> = ring.iter().map(|p| (p.0, p.1)).collect();
            let want = crate::geometry::polygon_area(&poly);
            let cnt = poly.len();
            let cx = poly.iter().map(|p| p.0).sum::<f64>() / cnt as f64;
            let cy = poly.iter().map(|p| p.1).sum::<f64>() / cnt as f64;
            let mut got = 0.0;
            for i in 0..cnt {
                let j = (i + 1) % cnt;
                let (p1, p2, p3) = ((cx, cy), poly[i], poly[j]);
                got += 0.5 * ((p2.0-p1.0)*(p3.1-p1.1) - (p3.0-p1.0)*(p2.1-p1.1)).abs();
            }
            assert!((got - want).abs() < want * 0.02,
                "Deckflaeche {label} falsch gefuellt: {got} statt {want}");
            assert!(crate::mesh::is_simple_polygon(&poly),
                "Deckflaeche {label} ist keine einfache Kontur");
        }
        // Das Volumen muss positiv sein - sonst ist die Wicklung falsum
        let mesh = crate::mesh::loft(&rings);
        let vol = crate::mesh::signed_volume(&mesh);
        eprintln!("Volumen: {vol:.1} mm^3");
        assert!(vol > 100.0, "Volumen negativ oder zu klein: {vol} - die Wicklung stimmt nicht");
        // Der Querschnitt darf nicht auf den Kopf stehen
        for r in &rings {
            let ys: Vec<f64> = r.iter().map(|p| p.1).collect();
            let top = ys.iter().cloned().fold(f64::MIN, f64::max);
            let bot = ys.iter().cloned().fold(f64::MAX, f64::min);
            assert!(top - bot > 5.0, "Querschnitt zu flach: {}", top - bot);
        }
    }

    #[test]
    fn schnitte_sind_eindeutig() {
        // Doppelte Schnitte machen den Loft in FreeCAD unbrauchbar
        let b = BladeBlank::default();
        let p = HandleParams::default();
        for style in HandleStyle::all() {
            let mut p2 = p.clone();
            p2.style = style;
            let geo = build_geometry(&b, &p2);
            for body in &geo.bodies {
                let mut zs: Vec<f64> = body
                    .iter()
                    .map(|r| r.iter().map(|p| p.2).sum::<f64>() / r.len() as f64)
                    .collect();
                let anzahl = zs.len();
                zs.sort_by(|a, b| a.partial_cmp(b).unwrap());
                for i in 1..anzahl {
                    assert!(
                        (zs[i] - zs[i - 1]).abs() > 1e-6,
                        "Doppelter Schnitt bei {style:?}: {} und {}",
                        zs[i - 1],
                        zs[i]
                    );
                }
            }
        }
    }

    #[test]
    fn angel_bleibt_einfach_geschlossen() {
        // Die Angel darf nicht zweimal dieselbe Strecke durchlaufen -
        // sonst schneidet der Draht sich selbst.
        let b = BladeBlank::default();
        let p = HandleParams::default();
        let tang = tang_profile(&b, 0.75);
        assert!(tang.len() > 10, "Angel zu kurz: {}", tang.len());
        assert!(
            crate::mesh::is_simple_polygon(&tang),
            "Angel ist keine einfache Kontur"
        );
        // Keine zwei aufeinanderfolgenden Punkte duerfen gleich sein
        for i in 0..tang.len() {
            let a = tang[i];
            let b2 = tang[(i + 1) % tang.len()];
            let d = ((a.0 - b2.0).powi(2) + (a.1 - b2.1).powi(2)).sqrt();
            assert!(d > 1e-6, "Doppelpunkt in der Angel bei {i}");
        }
    }

    #[test]
    fn lanyardloch_liegt_hinter_der_angel() {
        // Das Loch darf nicht in der Angel liegen - sonst muesste die
        // Angel entlastet werden und der Bohler kaeme nicht durch.
        let b = BladeBlank::default();
        let p = HandleParams::default();
        let pins = plan_pins(&b, &p);
        let lanyard = pins
            .iter()
            .find(|pin| pin.kind == "lanyard")
            .expect("kein Lanyardloch geplant");
        assert!(
            lanyard.x_mm > b.tang_length_mm,
            "Das Lanyardloch bei {} mm liegt noch in der Angel",
            lanyard.x_mm
        );
        assert_eq!(lanyard.achse, "z", "Das Loch muss quer durch die Dicke gehen");
        // Und es muss innerhalb des Griffes liegen
        let geo = build_geometry(&b, &p);
        let (unten, oben) = kontur_hoehe(&geo, lanyard.x_mm);
        assert!(oben - unten > 5.0, "Am Lanyardloch ist kein Holz mehr");
    }

    #[test]
    fn bohrungen_haben_immer_eine_achse() {
        let b = BladeBlank::default();
        let p = HandleParams::default();
        for style in HandleStyle::all() {
            let mut p2 = p.clone();
            p2.style = style;
            for pin in plan_pins(&b, &p2) {
                assert!(
                    pin.achse == "z" || pin.achse == "y",
                    "unbekannte Bohrachse {}",
                    pin.achse
                );
                assert!(pin.dia_mm > 0.0, "Bohrung ohne Durchmesser");
                assert!(pin.x_mm >= 0.0, "Bohrung vor dem Griffanfang");
            }
        }
    }

    #[test]
    fn ein_stueck_griff_hat_einfache_nut() {
        // Die Nut fuer die Angel muss in der Vorderkante sitzen,
        // sonst schneidet der Ring die Kontur diagonal durch.
        let b = BladeBlank::default();
        let mut p = HandleParams::default();
        p.style = HandleStyle::OnePiece;
        let geo = build_geometry(&b, &p);
        for body in &geo.bodies {
            let poly: Vec<(f64, f64)> = body[0].iter().map(|q| (q.0, q.1)).collect();
            assert!(
                crate::mesh::is_simple_polygon(&poly),
                "Der Ring des Ein-Stueck-Griffes ist keine einfache Kontur"
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Querschnitt durch den Griff
// ---------------------------------------------------------------------------

/// Der Querschnitt des Griffes an einer Stelle: die untere und die obere
/// Grenze des Holzes ueber der Dicke z, dazu die Angel.
#[derive(Clone, Debug)]
pub struct CrossSection {
    /// Je Koerper (Schale) die untere und obere Grenze:
    /// (z, untere Grenze) und (z, obere Grenze), jeweils aufsteigend nach z
    pub bodies: Vec<(Vec<(f64, f64)>, Vec<(f64, f64)>)>,
    /// halbe Angelbreite an der Stelle
    pub tang_half_width: f64,
    /// halbe Angeldicke
    pub tang_half_thickness: f64,
    /// y-Ausdehnung des Profiles an der Stelle
    pub height: f64,
    /// Wandstaerke am Ruecken
    pub wall_spine: f64,
    /// Wandstaerke an der Bauchseite
    pub wall_belly: f64,
}

impl CrossSection {
    /// Der Umfang des geschlossenen Griffquerschnitts.
    pub fn circumference(&self) -> f64 {
        grip_circumference(self.height, self.height * 0.0 + self.thickness(), 0.0)
    }
    pub fn thickness(&self) -> f64 {
        let mut min_z = f64::MAX;
        let mut max_z = f64::MIN;
        for (z, _) in self
            .bodies
            .iter()
            .flat_map(|(l, u)| l.iter().chain(u.iter()))
        {
            min_z = min_z.min(*z);
            max_z = max_z.max(*z);
        }
        if min_z > max_z {
            0.0
        } else {
            max_z - min_z
        }
    }
}

/// Berechnet den Querschnitt an der Stelle x (mm ab dem Heft).
pub fn cross_section(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    x: f64,
) -> Option<CrossSection> {
    // Die y-Ausdehnung eines Rings an der Stelle x
    fn extent_at(ring: &[(f64, f64, f64)], x: f64) -> Option<(f64, f64)> {
        let n = ring.len();
        let mut hits: Vec<f64> = Vec::new();
        for i in 0..n {
            let (x1, y1, _) = ring[i];
            let (x2, y2, _) = ring[(i + 1) % n];
            if (x1 - x).abs() < 1e-9 {
                hits.push(y1);
            }
            if (x1 - x) * (x2 - x) < 0.0 {
                let t = (x - x1) / (x2 - x1);
                hits.push(y1 + t * (y2 - y1));
            }
        }
        if hits.len() < 2 {
            return None;
        }
        let lo = hits.iter().cloned().fold(f64::MAX, f64::min);
        let hi = hits.iter().cloned().fold(f64::MIN, f64::max);
        Some((lo, hi))
    }

    let mut bodies: Vec<(Vec<(f64, f64)>, Vec<(f64, f64)>)> = Vec::new();
    for body in &geo.bodies {
        let mut lower: Vec<(f64, f64)> = Vec::new();
        let mut upper: Vec<(f64, f64)> = Vec::new();
        for ring in body {
            let n = ring.len().max(1) as f64;
            let z = ring.iter().map(|p| p.2).sum::<f64>() / n;
            if let Some((lo, hi)) = extent_at(ring, x) {
                lower.push((z, lo));
                upper.push((z, hi));
            }
        }
        if lower.len() < 2 {
            continue;
        }
        lower.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        upper.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        bodies.push((lower, upper));
    }
    if bodies.is_empty() {
        return None;
    }
    let all: Vec<&(f64, f64)> = bodies
        .iter()
        .flat_map(|(l, u)| l.iter().chain(u.iter()))
        .collect();

    let tang_half_width = crate::geometry::tang_half_width(blank, x);
    let min_y = all.iter().map(|v| v.1).fold(f64::MAX, f64::min);
    let max_y = all.iter().map(|v| v.1).fold(f64::MIN, f64::max);

    Some(CrossSection {
        bodies,
        tang_half_width,
        tang_half_thickness: blank.tang_thickness_mm / 2.0,
        height: max_y - min_y,
        wall_spine: max_y - tang_half_width,
        wall_belly: tang_half_width - min_y,
    })
}

/// Die oberen und unteren Kanten des Griffprofils als Punktfolgen.
///
/// Wird fuer die FreeCAD-Komfortrundung gebraucht: der Schneidzylinder
/// muss der Kette folgen, weil die Hoehe entlang des Griffs wechselt.
pub fn profilkanten(
    b: &BladeBlank,
    p: &HandleParams,
    schritte: usize,
) -> (Vec<(f64, f64)>, Vec<(f64, f64)>) {
    let (outline, _) = build_outline(b, p);
    let x_end = outline
        .iter()
        .map(|q| q.0)
        .fold(f64::MIN, f64::max)
        .max(1.0);
    let x_start = outline.iter().map(|q| q.0).fold(f64::MAX, f64::min);
    let span = (x_end - x_start).max(1.0);
    let n = schritte.max(8);
    let mut oben = Vec::new();
    let mut unten = Vec::new();
    for i in 0..=n {
        let x = x_start + span * i as f64 / n as f64;
        oben.push((x, spine_mit_wand(b, p, x, x_end)));
        unten.push((x, bauch_mit_wand(b, p, x, x_end)));
    }
    (oben, unten)
}

#[cfg(test)]
mod zuschnitt_tests {
    use super::*;

    #[test]
    fn zuschnitt_umschliesst_die_kontur() {
        let b = BladeBlank::default();
        let p = HandleParams::default();
        let geo = build_geometry(&b, &p);
        let (laenge, hoehe, dicke) = zuschnitt(&geo.outline, &p, geo.scale_thickness_mm, geo.total_thickness_mm);
        let x_min = geo.outline.iter().map(|q| q.0).fold(f64::MAX, f64::min);
        let x_max = geo.outline.iter().map(|q| q.0).fold(f64::MIN, f64::max);
        let y_min = geo.outline.iter().map(|q| q.1).fold(f64::MAX, f64::min);
        let y_max = geo.outline.iter().map(|q| q.1).fold(f64::MIN, f64::max);
        // Der Zuschnitt muss groesser sein als die Kontur
        assert!(laenge > x_max - x_min, "Zu kurz: {laenge}");
        assert!(hoehe > y_max - y_min, "Zu niedrig: {hoehe}");
        // Aber nur mit vernuenftigem Uebermass
        assert!(laenge < x_max - x_min + 10.0, "Zu viel Zugabe: {laenge}");
        assert!(hoehe < y_max - y_min + 10.0, "Zu viel Zugabe: {hoehe}");
        // Bei zwei Schalen ist die Angel aus Stahl - das Holz ist duenner
        assert!(
            dicke < geo.total_thickness_mm,
            "Das Holz ist dicker als der ganze Griff: {dicke}"
        );
        assert!(dicke > 4.0, "Das Holz ist zu duenn: {dicke}");
    }
}

