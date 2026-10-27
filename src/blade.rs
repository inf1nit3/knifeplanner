//! Die Klinge als Volumen.
//!
//! Aus dem Rohling ist nur bekannt, wie die Klinge am Heft aussieht
//! (Abstand der Angelmitte zu Ruecken und Schneide), wie lang sie ist und
//! wie dick die Schneide ist. Daraus entsteht eine Klinge, deren Kontur am
//! Heft genau die Kontur des Rohlings fortsetzt - dadurch passt sie
//! ohne Absatz in den Griff.
//!
//! Die Dicke verteilt sich vom dicken Ruecken zur duennen Schneide, mit
//! einer scharfen Schneide und einem feinen Secondary-Bevel.

use crate::mesh::Mesh;
use crate::model::BladeBlank;

/// Wie viele Stationen entlang der Klinge gerechnet werden.
const STATIONEN: usize = 28;

/// Die Schneide ist so duenn, dass sie fast eine Schneide ist.
const SCHNEIDEN_DICKE: f64 = 0.7;

/// Der Ruecken wird zur Spitze hin duenn.
const RUECKEN_DICKE_SPITZE: f64 = 1.1;

/// Wie breit der Secondary-Bevel an der Schneide ist.
const BEVEL_BREITE: f64 = 1.3;

/// So hoch bleibt die Klinge an der Spitze mindestens.
///
/// Sonst ist der letzte Abschnitt so schmal, dass die Flaeche beim
/// Rechnen degeneriert und das Volumen ungueltig wird.
const MINDEST_HOEHE: f64 = 0.35;

/// Die Form der Klinge: x, y vom Ruecken, y an der Schneide.
///
/// `x` laeuft von 0 (am Heft) nach vorn (negativ) bis zur Spitze.
fn stationen(b: &BladeBlank) -> Vec<(f64, f64, f64)> {
    let laenge = b.blade_length_mm.max(20.0);
    let hoehe = b.spine_offset_mm + b.edge_offset_mm.abs();
    let spitze = -b.edge_offset_mm.abs() + 0.26 * hoehe;
    let ricasso = b.ricasso_mm.max(0.0).min(laenge * 0.4);
    let mut raus: Vec<(f64, f64, f64)> = Vec::with_capacity(STATIONEN + 1);
    for i in 0..=STATIONEN {
        let t = i as f64 / STATIONEN as f64;
        let x = -laenge * t;

        // Der Ruecken faellt zur Spitze hin ab, aber erst in der letzten
        // Haelfte - sonst sieht die Klinge aus wie eine Nadel. Ganz vorn
        // bleibt ein Rest stehen, damit der Abschnitt nicht degeneriert.
        let y_schneide_roh = if ricasso > 0.0 && -x <= ricasso {
            -b.edge_offset_mm.abs()
        } else {
            let span = (laenge - ricasso).max(1.0);
            let p = ((-x - ricasso) / span).clamp(0.0, 1.0);
            -b.edge_offset_mm.abs() + (spitze - 0.04 + b.edge_offset_mm.abs()) * p.powf(2.2)
        };
        let y_ruecken_roh =
            b.spine_offset_mm + (spitze + 0.04 - b.spine_offset_mm) * t.powf(2.6);
        let y_schneide = y_schneide_roh.min(y_ruecken_roh - MINDEST_HOEHE);
        let y_ruecken = y_ruecken_roh.max(y_schneide + MINDEST_HOEHE);
        raus.push((x, y_ruecken, y_schneide));
    }
    raus
}

/// Die Dicke an der Schneide an der Stelle t (0 = Heft, 1 = Spitze).
fn dicke_schneide(b: &BladeBlank, t: f64) -> f64 {
    let start = b.blade_thickness_mm.max(0.4);
    start + (SCHNEIDEN_DICKE - start) * t
}

/// Die Dicke am Ruecken an der Stelle t (0 = Heft, 1 = Spitze).
fn dicke_ruecken(b: &BladeBlank, t: f64) -> f64 {
    // Der Ruecken darf nicht dicker werden als die Klinge braucht -
    // sonst sieht das Heft aus wie ein Klotz.
    let start = (b.tang_thickness_mm.max(0.4)).min(b.blade_thickness_mm.max(0.4) * 1.8);
    start + (RUECKEN_DICKE_SPITZE - start) * t.powf(0.7)
}

/// Der Querschnitt der Klinge an der Stelle x: Liste von (y, z).
fn querschnitt(b: &BladeBlank, y_ruecken: f64, y_schneide: f64, t: f64) -> Vec<(f64, f64)> {
    let d_schneide = dicke_schneide(b, t) / 2.0;
    let d_ruecken = dicke_ruecken(b, t) / 2.0;
    // Der Secondary-Bevel darf nie breiter werden als der Abschnitt
    // hoch ist - sonst kreuzen sich die Punkte und die Flaeche wird
    // ungueltig.
    let hoehe = (y_ruecken - y_schneide).max(0.01);
    let bevel = (hoehe * 0.12).min(BEVEL_BREITE).min(hoehe * 0.4);
    let b_y = y_schneide + bevel;
    vec![
        (y_schneide, d_schneide),
        (b_y, d_schneide),
        (y_ruecken, d_ruecken),
        (y_ruecken, -d_ruecken),
        (b_y, -d_schneide),
        (y_schneide, -d_schneide),
    ]
}

/// Die Ringe der Klinge fuer den Loft.
///
/// Jeder Ring liegt in einer Ebene senkrecht zur x-Achse - so rechnet
/// FreeCAD daraus einen sauberen Solid.
pub fn ringe(b: &BladeBlank) -> Vec<Vec<(f64, f64, f64)>> {
    let mut raus: Vec<Vec<(f64, f64, f64)>> = Vec::new();
    let n = stationen(b);
    for (i, &(x, y_ruecken, y_schneide)) in n.iter().enumerate() {
        let t = i as f64 / (n.len() - 1).max(1) as f64;
        // Ganz hinten laeuft die Klinge spitz zu - der letzte Ring ist
        // deshalb winzig, aber mit derselben Punktzahl.
        let mut q = querschnitt(b, y_ruecken, y_schneide, t);
        if i == n.len() - 1 {
            let ys: f64 = q.iter().map(|p| p.0).sum::<f64>() / q.len() as f64;
            let zs: f64 = q.iter().map(|p| p.1).sum::<f64>() / q.len() as f64;
            q = q
                .iter()
                .map(|&(y, z)| (ys + (y - ys) * 0.04, zs + (z - zs) * 0.04))
                .collect();
        }
        raus.push(q.iter().map(|&(y, z)| (x, y, z)).collect());
    }
    raus
}

/// Das Netz der Klinge.
///
/// Die Ringe liegen senkrecht zur x-Achse, deshalb wird hier eigens
/// gewickelt - `mesh::loft` rechnet Ringe in der xy-Ebene.
pub fn klinge(b: &BladeBlank) -> Mesh {
    let r = ringe(b);
    let mut mesh = Mesh::default();
    if r.len() < 2 {
        return mesh;
    }
    let n = r[0].len();
    if r.iter().any(|ring| ring.len() != n) {
        return mesh;
    }
    for ring in &r {
        for &p in ring {
            mesh.vertices.push(p);
        }
    }
    // Die Mantelflaechen zwischen den Ringen
    for k in 0..r.len() - 1 {
        let a0 = k * n;
        let b0 = (k + 1) * n;
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.faces.push((a0 + i, b0 + j, b0 + i));
            mesh.faces.push((a0 + i, a0 + j, b0 + j));
        }
    }
    // Die beiden Endflaechen: der Ring am Heft und die Spitze.
    // Die Wicklung entspricht der des Mantels: der Ring am Heft wird in
    // seiner Reihenfolge gefaecht, die Spitze gespiegelt.
    let yz: Vec<(f64, f64)> = r[0].iter().map(|p| (p.1, p.2)).collect();
    for &(a, b, c) in &crate::mesh::triangulate(&yz) {
        mesh.faces.push((a, b, c));
    }
    let last = (r.len() - 1) * n;
    let yz2: Vec<(f64, f64)> = r[r.len() - 1].iter().map(|p| (p.1, p.2)).collect();
    for &(a, b, c) in &crate::mesh::triangulate(&yz2) {
        mesh.faces.push((last + c, last + b, last + a));
    }
    mesh
}

/// Die Kontur der Klinge in der Seitenansicht (x, y) - fuer die Zeichnung.
///
/// Von der Spitze laeuft sie ueber den Ruecken zum Heft und ueber die
/// Schneide zurueck.
pub fn profil(b: &BladeBlank) -> Vec<(f64, f64)> {
    let n = stationen(b);
    let mut raus: Vec<(f64, f64)> = Vec::new();
    // Der Ruecken von der Spitze zum Heft
    for &(x, y_ruecken, _) in n.iter().rev() {
        raus.push((x, y_ruecken));
    }
    // Die Schneide vom Heft zur Spitze
    for &(x, _, y_schneide) in n.iter() {
        raus.push((x, y_schneide));
    }
    raus
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn klinge_ist_ein_sauberer_koerper() {
        let b = BladeBlank::default();
        let r = ringe(&b);
        assert!(r.len() > 10, "zu wenige Ringe: {}", r.len());
        // Alle Ringe haben gleich viele Punkte
        let n = r[0].len();
        for ring in &r {
            assert_eq!(ring.len(), n);
        }
        // Die Klinge laeuft nach vorn und wird zur Spitze hin schmaler
        let erste_hoehe = {
            let ys: Vec<f64> = r[0].iter().map(|p| p.1).collect();
            ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min)
        };
        let letzte_hoehe = {
            let ys: Vec<f64> = r[r.len() - 1].iter().map(|p| p.1).collect();
            ys.iter().cloned().fold(f64::MIN, f64::max) - ys.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!(erste_hoehe > 5.0, "Klinge zu schmal am Heft: {erste_hoehe}");
        assert!(
            letzte_hoehe < erste_hoehe * 0.2,
            "Klinge laeuft zur Spitze nicht spitz zu: {letzte_hoehe} statt {erste_hoehe}"
        );
        // Das Netz hat ein positives Volumen
        let m = klinge(&b);
        let v = crate::mesh::signed_volume(&m);
        assert!(v > 100.0, "Volumen der Klinge: {v}");
    }

    #[test]
    fn klinge_sitzt_am_heft_auf_der_griffkontur() {
        let b = BladeBlank::default();
        let r = ringe(&b);
        // Der erste Ring liegt bei x = 0 und reicht von der Schneide bis
        // zum Ruecken des Rohlings.
        let ys: Vec<f64> = r[0].iter().map(|p| p.1).collect();
        let oben = ys.iter().cloned().fold(f64::MIN, f64::max);
        let unten = ys.iter().cloned().fold(f64::MAX, f64::min);
        assert!((oben - b.spine_offset_mm).abs() < 0.01, "Ruecken passt nicht: {oben}");
        assert!((unten + b.edge_offset_mm.abs()).abs() < 0.01, "Schneide passt nicht: {unten}");
    }
}
