//! Einfache Mesh-Erzeugung und STL-Export.
//!
//! Ohne CSG-Bibliothek: jedes Volumen ist ein sauber geschlossenes Prisma mit
//! triangulierten Deckflaechen (Ear-Clipping). Der Aussparungs-koerper wird als
//! eigenes Volumen im selben STL abgelegt - die Differenz uebernimmt CAD/Slicer.

use std::fs;
use std::io::Write;
use std::path::Path;

pub type Point = (f64, f64);
pub type Tri = (usize, usize, usize);

// ---------------------------------------------------------------------------
// Ear-Clipping
// ---------------------------------------------------------------------------


/// Trianguliert ein einfaches Polygon (ohne Selbstueberschneidung).
///
/// Verwendet earcut, das auch mit sehr dichten, fast kollinearen Konturen
/// zuverlaessig arbeitet - bei einer Griffkontur mit Hunderten von Punkten
/// scheitert eine selbst geschriebene Ohr-Kappung an den kollinearen
/// Stellen.
pub fn triangulate(poly: &[Point]) -> Vec<Tri> {
    if poly.len() < 3 {
        return Vec::new();
    }
    let mut data: Vec<f64> = Vec::with_capacity(poly.len() * 2);
    for &(x, y) in poly {
        data.push(x);
        data.push(y);
    }
    match earcutr::earcut(&data, &[], 2) {
        Ok(idx) => idx
            .chunks_exact(3)
            .map(|c| (c[0], c[1], c[2]))
            .collect(),
        // Bei entarteten Konturen: leeres Ergebnis, der Aufrufer
        // verkraftet das ueber die Flaechenpruefung.
        Err(_) => Vec::new(),
    }
}

// ---------------------------------------------------------------------------
// Volumen
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<(f64, f64, f64)>,
    pub faces: Vec<Tri>,
}

impl Mesh {
    pub fn translated(&self, dx: f64, dy: f64, dz: f64) -> Mesh {
        Mesh {
            vertices: self
                .vertices
                .iter()
                .map(|&(x, y, z)| (x + dx, y + dy, z + dz))
                .collect(),
            faces: self.faces.clone(),
        }
    }

    /// Dreht das Netz um die x-Achse (Winkel in Grad).
    ///
    /// Damit wird ein Zylinder, der entlang z laeuft, zu einem, der
    /// entlang y laeuft - so lassen sich Querbohrungen durch die
    /// Griffhoehe darstellen.
    pub fn gedreht_um_x(&self, grad: f64) -> Mesh {
        let a = grad.to_radians();
        let (s, c) = (a.sin(), a.cos());
        let mut m = Mesh {
            vertices: self.vertices.clone(),
            faces: self.faces.clone(),
        };
        for v in m.vertices.iter_mut() {
            let (y, z) = (v.1, v.2);
            v.1 = y * c - z * s;
            v.2 = y * s + z * c;
        }
        m
    }
}

/// Ein Zylinder, der entlang der y-Achse laeuft.
///
/// `x` ist die Lage quer, `y0` und `y1` die beiden Enden.
pub fn zylinder_y(x: f64, y0: f64, y1: f64, radius: f64, segments: usize) -> Mesh {
    let laenge = (y1 - y0).abs().max(0.01);
    // Der Zylinder entsteht entlang z und wird dann in die y-Richtung
    // gedreht: (x, y, z) wird zu (x, -z, y).
    cylinder(x, 0.0, -laenge, 0.0, radius, segments)
        .gedreht_um_x(90.0)
        .translated(0.0, y0.min(y1), 0.0)
}

/// Teilt ein Netz an der Ebene x = grenze in zwei Teile.
///
/// Die FreeCAD-Modelle enthalten Klinge und Griff in einem Netz. Fuer die
/// Anzeige wird die Klinge (vor dem Griff, also bei negativem x) getrennt,
/// damit sie als Stahl und nicht als Holz erscheint.
pub fn teile_bei_x(mesh: &Mesh, grenze: f64) -> (Mesh, Mesh) {
    let mut vorn = Mesh::default();
    let mut hinten = Mesh::default();
    // Die Punkte werden auf beide Netze verteilt, aber jeder Teil bekommt
    // seine eigenen Punkte - so bleiben die Indizes gueltig.
    let mut vorn_idx: Vec<Option<usize>> = vec![None; mesh.vertices.len()];
    let mut hinten_idx: Vec<Option<usize>> = vec![None; mesh.vertices.len()];
    for (i, v) in mesh.vertices.iter().enumerate() {
        if v.0 <= grenze {
            vorn_idx[i] = Some(vorn.vertices.len());
            vorn.vertices.push(*v);
        }
        if v.0 >= grenze {
            hinten_idx[i] = Some(hinten.vertices.len());
            hinten.vertices.push(*v);
        }
    }
    for &(a, b, c) in &mesh.faces {
        let xs = [a, b, c].map(|i| mesh.vertices[i].0);
        // Eine Flaeche, die genau auf der Ebene liegt, gehoert zu beiden
        // Teilen - sonst bekaeme einer von beiden keine Deckflaeche.
        if xs.iter().all(|&x| x <= grenze) {
            vorn.faces.push((
                vorn_idx[a].unwrap(),
                vorn_idx[b].unwrap(),
                vorn_idx[c].unwrap(),
            ));
        }
        if xs.iter().all(|&x| x >= grenze) {
            hinten.faces.push((
                hinten_idx[a].unwrap(),
                hinten_idx[b].unwrap(),
                hinten_idx[c].unwrap(),
            ));
        }
    }
    (vorn, hinten)
}

/// Erzeugt ein Hohlprisma - eine Manschette aus zwei Konturen.
///
/// `aussen` und `innen` muessen gleich viele Punkte haben und
/// punktabwechselnd zueinander gehoeren. Die Enden sind Ringe, kein
/// geschlossener Deckflaeche. So bekommt man eine Zwinge, die ueber das
/// Holz passt, statt eines vollen Blocks.
///
/// Wird fuer die Zwinge gebraucht: sie ist ein Mantel um das Holz, kein
/// aufgesetzter Klotz.
pub fn hohlprism(aussen: &[Point], innen: &[Point], z0: f64, z1: f64) -> Mesh {
    let n = aussen.len().min(innen.len());
    if n < 3 || aussen.len() != innen.len() {
        return Mesh::default();
    }
    // Die Kontur wird auf gegen den Uhrzeigersinn gebracht, damit die
    // Normalen nach aussen zeigen.
    let (aussen, innen) = if polygon_signed_area(aussen) < 0.0 {
        let mut a = aussen.to_vec();
        let mut i = innen.to_vec();
        a.reverse();
        i.reverse();
        (a, i)
    } else {
        (aussen.to_vec(), innen.to_vec())
    };

    let mut m = Mesh::default();

    // Die Mantelflaechen: aussen und innen (innen gegenlaeufig, damit
    // die Normalen nach innen zeigen)
    for i in 0..n {
        let j = (i + 1) % n;
        let a = m.vertices.len();
        m.vertices.push((aussen[i].0, aussen[i].1, z0));
        m.vertices.push((aussen[j].0, aussen[j].1, z0));
        m.vertices.push((aussen[j].0, aussen[j].1, z1));
        m.vertices.push((aussen[i].0, aussen[i].1, z1));
        m.faces.push((a, a + 1, a + 2));
        m.faces.push((a, a + 2, a + 3));

        let b = m.vertices.len();
        m.vertices.push((innen[j].0, innen[j].1, z0));
        m.vertices.push((innen[i].0, innen[i].1, z0));
        m.vertices.push((innen[i].0, innen[i].1, z1));
        m.vertices.push((innen[j].0, innen[j].1, z1));
        m.faces.push((b, b + 1, b + 2));
        m.faces.push((b, b + 2, b + 3));
    }

    // Die Ringe an den beiden Enden
    for i in 0..n {
        let j = (i + 1) % n;
        // Unten: die Normalen zeigen nach unten
        let a = m.vertices.len();
        m.vertices.push((aussen[i].0, aussen[i].1, z0));
        m.vertices.push((innen[i].0, innen[i].1, z0));
        m.vertices.push((innen[j].0, innen[j].1, z0));
        m.vertices.push((aussen[j].0, aussen[j].1, z0));
        m.faces.push((a, a + 1, a + 2));
        m.faces.push((a, a + 2, a + 3));

        // Oben: die Normalen zeigen nach oben
        let b = m.vertices.len();
        m.vertices.push((innen[i].0, innen[i].1, z1));
        m.vertices.push((aussen[i].0, aussen[i].1, z1));
        m.vertices.push((aussen[j].0, aussen[j].1, z1));
        m.vertices.push((innen[j].0, innen[j].1, z1));
        m.faces.push((b, b + 1, b + 2));
        m.faces.push((b, b + 2, b + 3));
    }
    m
}

/// Erzeugt ein geschlossenes Prisma aus einem 2D-Polygon.
pub fn prism(poly: &[Point], z0: f64, z1: f64) -> Mesh {
    let pts = poly.to_vec();
    if pts.len() < 3 {
        return Mesh::default();
    }
    let tris = triangulate(&pts);
    let n = pts.len();
    let mut vertices: Vec<(f64, f64, f64)> = pts.iter().map(|&(x, y)| (x, y, z0)).collect();
    vertices.extend(pts.iter().map(|&(x, y)| (x, y, z1)));

    let mut faces: Vec<Tri> = Vec::new();
    for &(a, b, c) in &tris {
        faces.push((c, b, a));
        faces.push((a + n, b + n, c + n));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        faces.push((i, j, j + n));
        faces.push((i, j + n, i + n));
    }
    Mesh { vertices, faces }
}

/// Fasst mehrere Volumen zu einem Mesh zusammen.
pub fn merge(meshes: &[Mesh]) -> Mesh {
    let mut vertices = Vec::new();
    let mut faces = Vec::new();
    for m in meshes {
        let off = vertices.len();
        vertices.extend_from_slice(&m.vertices);
        faces.extend(m.faces.iter().map(|&(a, b, c)| (a + off, b + off, c + off)));
    }
    Mesh { vertices, faces }
}

/// Versetzt ein Polygon nach innen (positiver Wert) oder nach aussen
/// (negativer Wert).
///
/// Die Richtung ist unabhaengig von der Umlaufrichtung des Polygons: die
/// Kontur wird intern auf gegen den Uhrzeigersinn normiert und danach
/// wieder zurueckgedreht.
pub fn offset(poly: &[Point], delta: f64) -> Vec<Point> {
    if poly.is_empty() || delta == 0.0 {
        return poly.to_vec();
    }
    let mut work: Vec<Point> = poly.to_vec();
    let flipped = polygon_signed_area(&work) < 0.0;
    if flipped {
        work.reverse();
    }
    let n = work.len();
    let mut out = Vec::with_capacity(n);
    for i in 0..n {
        let (px, py) = work[(i + n - 1) % n];
        let (cx, cy) = work[i];
        let (nx, ny) = work[(i + 1) % n];
        let normal = |ax: f64, ay: f64, bx: f64, by: f64| -> (f64, f64) {
            let (dx, dy) = (bx - ax, by - ay);
            let ln = (dx * dx + dy * dy).sqrt();
            if ln < 1e-9 {
                (0.0, 0.0)
            } else {
                (-dy / ln, dx / ln)
            }
        };
        let n1 = normal(px, py, cx, cy);
        let n2 = normal(cx, cy, nx, ny);
        let (mut mx, mut my) = (n1.0 + n2.0, n1.1 + n2.1);
        let ml = (mx * mx + my * my).sqrt();
        if ml < 1e-9 {
            out.push((cx, cy));
            continue;
        }
        mx /= ml;
        my /= ml;
        // Gehrung mit begrenzter Laenge: an sehr spitzen Ecken entstehen so
        // keine Ausreisser, die Kontur bleibt geschlossen.
        let cosang = (mx * n1.0 + my * n1.1).max(0.25);
        let miter = (1.0 / cosang).min(1.5);
        // Fuer eine gegen den Uhrzeigersinn laufende Kontur zeigt die
        // Linke Normale nach innen.
        let mut vx = mx * delta * miter;
        let mut vy = my * delta * miter;

        // Der Gehrungspunkt darf nicht ueber die Nachbarn hinausragen.
        // Sonst klappt die Kontur an der Ecke um sich selbst: die versetzte
        // Kante laeuft rueckwaerts und der Eckpunkt liegt auf der
        // Nachbarkante. Die Gehrung wird deshalb auf dem Winkelhalbierenden
        // gekuerzt - der Punkt bleibt auf der Innenseite. Ein Zurueckziehen
        // waere falsch, das dreht den Punkt auf die falsche Seite.
        {
            // Die Kante davor: der Punkt darf nicht ueber den vorherigen
            // Punkt hinaus nach hinten ruecken.
            let (dx, dy) = (cx - px, cy - py);
            let len = (dx * dx + dy * dy).sqrt();
            if len > 1e-9 {
                let (ux, uy) = (dx / len, dy / len);
                let vor = vx * ux + vy * uy;
                if vor < -len * 0.999 {
                    let f = (-len * 0.999) / vor;
                    vx *= f;
                    vy *= f;
                }
            }
        }
        {
            // Die Kante danach: der Punkt darf nicht ueber den naechsten
            // Punkt hinaus nach vorn springen.
            let (dx, dy) = (nx - cx, ny - cy);
            let len = (dx * dx + dy * dy).sqrt();
            if len > 1e-9 {
                let (ux, uy) = (dx / len, dy / len);
                let vor = vx * ux + vy * uy;
                if vor > len * 0.999 {
                    let f = (len * 0.999) / vor;
                    vx *= f;
                    vy *= f;
                }
            }
        }
        out.push((cx + vx, cy + vy));
    }
    if flipped {
        out.reverse();
    }
    out
}

/// Erzeugt eine Platte mit rund gebrochenen Kanten.
///
/// `poly` ist die Kontur in der Seitenansicht (x/y), `thickness` die Dicke
/// entlang z, `radius` der Radius der Kantenrundung. Die Platte ist ein
/// geschlossenes Volumen mit Deckflaechen und Seitenwaenden.
///
/// Aufbau: Von der Deckflaeche laeuft eine Viertelkreisfase nach unten in
/// die volle Kontur, dann die Flaeche ueber die halbe Dicke, dann die
/// untere Fase.
pub fn rounded_slab(poly: &[Point], thickness: f64, radius: f64, steps: usize) -> Mesh {
    if poly.len() < 3 || thickness <= 0.0 {
        return Mesh::default();
    }
    let half = thickness / 2.0;
    slab_between(poly, -half, half, radius, radius, steps)
}

/// Flaeche eines Polygons (positiv = gegen den Uhrzeigersinn).
pub fn polygon_signed_area(poly: &[Point]) -> f64 {
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
    s / 2.0
}

/// Baut die Kontur "aussen minus Schlitz".
///
/// `outer` ist die geschlossene Aussenkontur, die an der Vorderkante
/// beginnt: Index 0 ist der untere Vorderrandpunkt, Index 1 der obere.
/// `slot_path` laeuft vom unteren Schlitzpunkt ueber die Schlitzspitze zum
/// oberen Schlitzpunkt. So entsteht ein einfach geschlossenes Polygon mit
/// offener Nut an der Vorderkante.
pub fn u_shape(outer: &[Point], slot_path: &[Point]) -> Vec<Point> {
    let mut out: Vec<Point> = Vec::with_capacity(outer.len() + slot_path.len());
    out.push(outer[0]);
    out.extend_from_slice(slot_path);
    out.extend_from_slice(&outer[1..]);
    out
}

/// Die Ringe einer Platte mit gebrochenen Kanten.
///
/// Jeder Ring nennt seine Kontur und seine Hoehe. Damit kann dieselbe
/// Platte einmal als Netz und einmal als Loft gerechnet werden - der
/// Loft in FreeCAD braucht ebene Ringe.
pub fn platten_ringe(
    poly: &[Point],
    z0: f64,
    z1: f64,
    r_top: f64,
    r_bot: f64,
    steps: usize,
) -> Vec<(Vec<Point>, f64)> {
    let steps = steps.max(1);
    let mut raus: Vec<(Vec<Point>, f64)> = Vec::new();
    // Obere Fase: von der Deckflaeche nach unten auf die volle Kontur
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        raus.push((offset(poly, r_top * (1.0 - t)), z1 - r_top * t));
    }
    // Gerader Teil
    let z_mid_bot = z0 + r_bot;
    let z_mid_top = z1 - r_top;
    if z_mid_bot > z_mid_top {
        raus.push((poly.to_vec(), z_mid_bot));
    }
    // Untere Fase
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        raus.push((offset(poly, r_bot * t), z0 + r_bot - r_bot * t));
    }
    raus
}

/// Erzeugt eine Platte zwischen zwei Hoehen mit wahlweise gerundeten Kanten.
///
/// `r_top` rundet die obere, `r_bot` die untere Kante. Die Deckflaechen sind
/// jeweils um den Radius eingezogen, die Seitenwand steht dazwischen.
pub fn slab_between(
    outline: &[Point],
    z0: f64,
    z1: f64,
    r_top: f64,
    r_bot: f64,
    steps: usize,
) -> Mesh {
    let mut mesh = Mesh::default();
    if outline.len() < 3 || z1 <= z0 {
        return mesh;
    }
    let mut poly = outline.to_vec();
    if polygon_signed_area(&poly) < 0.0 {
        poly.reverse();
    }
    let steps = steps.max(2);
    let r_top = r_top.clamp(0.0, (z1 - z0) * 0.49);
    let r_bot = r_bot.clamp(0.0, (z1 - z0) * 0.49);

    let mut rings: Vec<Vec<Point>> = Vec::new();
    let mut zs: Vec<f64> = Vec::new();

    for (kontur, z) in platten_ringe(&poly, z0, z1, r_top, r_bot, steps) {
        rings.push(kontur);
        zs.push(z);
    }

    let count = rings[0].len();
    let base = mesh.vertices.len();
    for (ring, z) in rings.iter().zip(zs.iter()) {
        for &(x, y) in ring {
            mesh.vertices.push((x, y, *z));
        }
    }

    for r_i in 0..rings.len().saturating_sub(1) {
        let a0 = base + r_i * count;
        let b0 = base + (r_i + 1) * count;
        for i in 0..count {
            let j = (i + 1) % count;
            mesh.faces.push((a0 + i, b0 + i, b0 + j));
            mesh.faces.push((a0 + i, b0 + j, a0 + j));
        }
    }

    let top = base;
    let bottom = base + (rings.len() - 1) * count;
    let tris_top = triangulate(&rings[0]);
    let tris_bottom = triangulate(&rings[rings.len() - 1]);
    for &(a, b, c) in &tris_top {
        mesh.faces.push((top + a, top + b, top + c));
    }
    for &(a, b, c) in &tris_bottom {
        mesh.faces.push((bottom + c, bottom + b, bottom + a));
    }

    mesh
}

/// Erzeugt einen Zylinder entlang der z-Achse.
///
/// Wird fuer die Bohrungen im 3D-Modell benutzt, damit man sieht, wie die
/// Bohrer durch die Schale gehen.
pub fn cylinder(
    cx: f64,
    cy: f64,
    z0: f64,
    z1: f64,
    radius: f64,
    segments: usize,
) -> Mesh {
    let mut mesh = Mesh::default();
    let n = segments.max(6);
    let base = mesh.vertices.len();
    for i in 0..n {
        let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        let (x, y) = (cx + radius * a.cos(), cy + radius * a.sin());
        mesh.vertices.push((x, y, z0));
    }
    for i in 0..n {
        let a = 2.0 * std::f64::consts::PI * i as f64 / n as f64;
        let (x, y) = (cx + radius * a.cos(), cy + radius * a.sin());
        mesh.vertices.push((x, y, z1));
    }
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.faces.push((base + i, base + j, base + n + j));
        mesh.faces.push((base + i, base + n + j, base + n + i));
    }
    // Deckflaechen
    let c0 = mesh.vertices.len();
    mesh.vertices.push((cx, cy, z0));
    let c1 = mesh.vertices.len();
    mesh.vertices.push((cx, cy, z1));
    for i in 0..n {
        let j = (i + 1) % n;
        mesh.faces.push((c0, base + j, base + i));
        mesh.faces.push((c1, base + n + i, base + n + j));
    }
    mesh
}

/// Erzeugt ein Volumen aus einer Folge von Ringen (Loft).
///
/// Die Ringe muessen alle gleich viele Punkte haben. Die Ringe werden von
/// oben nach unten erwartet.
pub fn loft(rings: &[Vec<(f64, f64, f64)>]) -> Mesh {
    let mut mesh = Mesh::default();
    if rings.len() < 2 {
        return mesh;
    }
    let n = rings[0].len();
    if rings.iter().any(|r| r.len() != n) {
        return mesh;
    }
    for ring in rings {
        for &p in ring {
            mesh.vertices.push(p);
        }
    }
    for r in 0..rings.len() - 1 {
        let a0 = r * n;
        let b0 = (r + 1) * n;
        for i in 0..n {
            let j = (i + 1) % n;
            mesh.faces.push((a0 + i, b0 + j, b0 + i));
            mesh.faces.push((a0 + i, a0 + j, b0 + j));
        }
    }
    // Deckflaechen oben und unten
    let top: Vec<Point> = rings[0].iter().map(|p| (p.0, p.1)).collect();
    let bottom: Vec<Point> = rings[rings.len() - 1]
        .iter()
        .map(|p| (p.0, p.1))
        .collect();
    let last = (rings.len() - 1) * n;
    for &(a, b, c) in &triangulate(&top) {
        mesh.faces.push((a, b, c));
    }
    for &(a, b, c) in &triangulate(&bottom) {
        mesh.faces.push((last + c, last + b, last + a));
    }
    mesh
}

/// Erzeugt eine Platte mit rund gebrochenen Kanten und einer Nut in der
/// Unterseite.
///
/// Die Nut reicht `slot_depth` tief vom Boden nach oben; ihre Waende sind
/// senkrecht (so fräst man sie). Der Aussenrand bekommt denselben Radius
/// wie die uebrigen Kanten.
pub fn slab_with_slot(
    outline: &[Point],
    slot_path: &[Point],
    thickness: f64,
    radius: f64,
    slot_depth: f64,
    steps: usize,
) -> Mesh {
    let mut mesh = Mesh::default();
    if outline.len() < 3 || thickness <= 0.0 {
        return mesh;
    }
    let mut poly = outline.to_vec();
    if polygon_signed_area(&poly) < 0.0 {
        poly.reverse();
    }
    let half = thickness / 2.0;
    let r = radius.clamp(0.0, half * 0.98);
    let steps = steps.max(2);
    let slot_depth = slot_depth.clamp(0.0, thickness);

    let mut rings: Vec<Vec<Point>> = Vec::new();
    let mut zs: Vec<f64> = Vec::new();

    // Oberer Teil: Deckflaeche mit Fase bis zur vollen Kontur
    for k in 0..=steps {
        let t = k as f64 / steps as f64;
        rings.push(offset(&poly, r * (1.0 - t)));
        zs.push(half - r * t);
    }

    // Gerader Teil bis zum Boden der Nut
    let z_slot = -half + slot_depth;
    if z_slot > -half + r {
        rings.push(poly.clone());
        zs.push(z_slot);
    }

    // Nutwaende (senkrecht) und untere Fase
    let bottom_steps = 4;
    for k in 0..=bottom_steps {
        let t = k as f64 / bottom_steps as f64;
        let z = z_slot + (-half + r - z_slot) * t;
        // Der Aussenrand zieht sich zur unteren Fase hin zurueck,
        // die Nutwaende bleiben senkrecht.
        let inset = if z < -half + r {
            r * (1.0 - (z - (-half + r)) / r.max(1e-6))
        } else {
            0.0
        };
        let outer = offset(&poly, inset.max(0.0));
        rings.push(u_shape(&outer, slot_path));
        zs.push(z);
    }

    let count = rings[0].len();
    let base = mesh.vertices.len();
    for (ring, z) in rings.iter().zip(zs.iter()) {
        for &(x, y) in ring {
            mesh.vertices.push((x, y, *z));
        }
    }

    for r_i in 0..rings.len().saturating_sub(1) {
        let a0 = base + r_i * count;
        let b0 = base + (r_i + 1) * count;
        for i in 0..count {
            let j = (i + 1) % count;
            mesh.faces.push((a0 + i, b0 + i, b0 + j));
            mesh.faces.push((a0 + i, b0 + j, a0 + j));
        }
    }

    let top = base;
    let bottom = base + (rings.len() - 1) * count;
    let tris_top = triangulate(&rings[0]);
    let tris_bottom = triangulate(&rings[rings.len() - 1]);
    for &(a, b, c) in &tris_top {
        mesh.faces.push((top + a, top + b, top + c));
    }
    for &(a, b, c) in &tris_bottom {
        mesh.faces.push((bottom + c, bottom + b, bottom + a));
    }

    mesh
}
/// Prueft, ob ein Polygon einfach ist - also keine self-intersections hat.
///
/// Wird fuer die Griffkontur benutzt: eine Kontur, die sich selbst
/// schneidet, laesst sich nicht sauber schneiden und der Zuschchnitt wird
/// unbrauchbar.
pub fn is_simple_polygon(poly: &[Point]) -> bool {
    let n = poly.len();
    if n < 3 {
        return false;
    }
    let seg = |i: usize| -> (Point, Point) { (poly[i % n], poly[(i + 1) % n]) };

    let cross = |a: Point, b: Point, c: Point| {
        (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0)
    };
    let on_segment = |a: Point, b: Point, p: Point| -> bool {
        p.0 >= a.0.min(b.0) - 1e-9
            && p.0 <= a.0.max(b.0) + 1e-9
            && p.1 >= a.1.min(b.1) - 1e-9
            && p.1 <= a.1.max(b.1) + 1e-9
    };
    let intersects = |p1: Point, p2: Point, p3: Point, p4: Point| -> bool {
        let d1 = cross(p3, p4, p1);
        let d2 = cross(p3, p4, p2);
        let d3 = cross(p1, p2, p3);
        let d4 = cross(p1, p2, p4);
        if ((d1 > 0.0 && d2 < 0.0) || (d1 < 0.0 && d2 > 0.0))
            && ((d3 > 0.0 && d4 < 0.0) || (d3 < 0.0 && d4 > 0.0))
        {
            return true;
        }
        (d1 == 0.0 && on_segment(p3, p4, p1))
            || (d2 == 0.0 && on_segment(p3, p4, p2))
            || (d3 == 0.0 && on_segment(p1, p2, p3))
            || (d4 == 0.0 && on_segment(p1, p2, p4))
    };

    for i in 0..n {
        let (a1, a2) = seg(i);
        // Kanten ohne Laenge entstehen, wenn zwei Punkte identisch sind
        if (a1.0 - a2.0).abs() < 1e-9 && (a1.1 - a2.1).abs() < 1e-9 {
            continue;
        }
        for j in i + 1..n {
            // Nachbarkanten duerfen sich am Endpunkt treffen
            if j == i + 1 || (i == 0 && j == n - 1) {
                continue;
            }
            let (b1, b2) = seg(j);
            if (b1.0 - b2.0).abs() < 1e-9 && (b1.1 - b2.1).abs() < 1e-9 {
                continue;
            }
            if intersects(a1, a2, b1, b2) {
                return false;
            }
        }
    }
    true
}

/// Glaettet die Normalen je Vertex - damit wirken die Fasen weich.
pub fn smooth_normals(mesh: &Mesh) -> Vec<[f32; 3]> {
    let mut normals = vec![[0.0f32; 3]; mesh.vertices.len()];
    for &(a, b, c) in &mesh.faces {
        let (p1, p2, p3) = (
            mesh.vertices[a],
            mesh.vertices[b],
            mesh.vertices[c],
        );
        let u = [p2.0 - p1.0, p2.1 - p1.1, p2.2 - p1.2];
        let v = [p3.0 - p1.0, p3.1 - p1.1, p3.2 - p1.2];
        let n = [
            u[1] * v[2] - u[2] * v[1],
            u[2] * v[0] - u[0] * v[2],
            u[0] * v[1] - u[1] * v[0],
        ];
        for idx in [a, b, c] {
            normals[idx][0] += n[0] as f32;
            normals[idx][1] += n[1] as f32;
            normals[idx][2] += n[2] as f32;
        }
    }
    for n in normals.iter_mut() {
        let len = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
        if len > 1e-9 {
            n[0] /= len;
            n[1] /= len;
            n[2] /= len;
        }
    }
    normals
}

// ---------------------------------------------------------------------------
// STL
// ---------------------------------------------------------------------------

/// Binares STL schreiben.
pub fn write_stl(path: &Path, meshes: &[Mesh], name: &str) -> std::io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut out: Vec<u8> = Vec::new();
    let mut header = name.as_bytes().to_vec();
    header.resize(80, b' ');
    out.extend_from_slice(&header);

    let total: usize = meshes.iter().map(|m| m.faces.len()).sum();
    out.extend_from_slice(&(total as u32).to_le_bytes());

    for m in meshes {
        let v = &m.vertices;
        for &(a, b, c) in &m.faces {
            let (p1, p2, p3) = (v[a], v[b], v[c]);
            let u = (p2.0 - p1.0, p2.1 - p1.1, p2.2 - p1.2);
            let w = (p3.0 - p1.0, p3.1 - p1.1, p3.2 - p1.2);
            let mut n = (
                u.1 * w.2 - u.2 * w.1,
                u.2 * w.0 - u.0 * w.2,
                u.0 * w.1 - u.1 * w.0,
            );
            let ln = (n.0 * n.0 + n.1 * n.1 + n.2 * n.2).sqrt();
            if ln > 0.0 {
                n = (n.0 / ln, n.1 / ln, n.2 / ln);
            }
            // STL speichert Normalen und Koordinaten als 32-Bit-Werte
            out.extend_from_slice(&(n.0 as f32).to_le_bytes());
            out.extend_from_slice(&(n.1 as f32).to_le_bytes());
            out.extend_from_slice(&(n.2 as f32).to_le_bytes());
            for p in [p1, p2, p3] {
                out.extend_from_slice(&(p.0 as f32).to_le_bytes());
                out.extend_from_slice(&(p.1 as f32).to_le_bytes());
                out.extend_from_slice(&(p.2 as f32).to_le_bytes());
            }
            out.extend_from_slice(&0u16.to_le_bytes());
        }
    }

    let mut f = fs::File::create(path)?;
    f.write_all(&out)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn triangulates_square() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let tris = triangulate(&sq);
        assert_eq!(tris.len(), 2);
    }

    #[test]
    fn prism_is_closed() {
        let sq = [(0.0, 0.0), (10.0, 0.0), (10.0, 5.0), (0.0, 5.0)];
        let m = prism(&sq, 0.0, 3.0);
        assert_eq!(m.vertices.len(), 8);
        assert_eq!(m.faces.len(), 2 + 2 + 8);
    }

    #[test]
    fn ear_clipping_handles_concave() {
        // L-foermiges Polygon
        let l = [(0.0, 0.0), (10.0, 0.0), (10.0, 4.0), (4.0, 4.0), (4.0, 10.0), (0.0, 10.0)];
        let tris = triangulate(&l);
        assert_eq!(tris.len(), 4);
    }

    #[test]
    fn rounded_slab_face_is_inset() {
        // Die Deckflaeche muss kleiner sein als der Koerper - sonst waere
        // die Kante nach aussen aufgezogen statt gerundet.
        let sq = [
            (0.0, 0.0),
            (100.0, 0.0),
            (100.0, 20.0),
            (0.0, 20.0),
        ];
        let m = rounded_slab(&sq, 10.0, 2.0, 4);
        let width_at = |z: f64| -> f64 {
            m.vertices
                .iter()
                .filter(|v| (v.2 - z).abs() < 1e-6)
                .map(|v| v.0)
                .fold(f64::MIN, f64::max)
                - m.vertices
                    .iter()
                    .filter(|v| (v.2 - z).abs() < 1e-6)
                    .map(|v| v.0)
                    .fold(f64::MAX, f64::min)
        };
        let face = width_at(5.0);
        let body = width_at(3.0);
        assert!(
            face < body,
            "Deckflaeche ({face}) muss schmaler sein als der Koerper ({body})"
        );
        assert!((body - 100.0).abs() < 1e-6, "Koerperbreite {body}");
    }
}

/// Vorzeichenbehaftetes Volumen eines Meshs (Divergenzsatz).
/// Positiv = die Flaechen zeigen nach aussen.
pub fn signed_volume(mesh: &Mesh) -> f64 {
    let mut v = 0.0;
    for &(a, b, c) in &mesh.faces {
        let (p1, p2, p3) = (mesh.vertices[a], mesh.vertices[b], mesh.vertices[c]);
        v += (p1.0 * (p2.1 * p3.2 - p2.2 * p3.1)
            - p1.1 * (p2.0 * p3.2 - p2.2 * p3.0)
            + p1.2 * (p2.0 * p3.1 - p2.1 * p3.0))
            / 6.0;
    }
    v
}

#[cfg(test)]
mod teil_tests {
    use super::*;

    #[test]
    fn netz_teilt_sich_an_der_ebene() {
        // Zwei Quader, einer vor und einer hinter der Ebene x = 0
        let hinten = prism(&[(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)], -5.0, 5.0);
        let vorn = prism(
            &[(-10.0, 0.0), (0.0, 0.0), (0.0, 10.0), (-10.0, 10.0)],
            -5.0,
            5.0,
        );
        let m = merge(&[vorn, hinten]);
        let (v, h) = teile_bei_x(&m, 0.0);
        assert!(v.faces.len() > 0, "der vordere Teil ist leer");
        assert!(h.faces.len() > 0, "der hintere Teil ist leer");
        for p in &v.vertices {
            assert!(p.0 <= 0.0, "Punkt hinter der Ebene: {p:?}");
        }
        for p in &h.vertices {
            assert!(p.0 >= 0.0, "Punkt vor der Ebene: {p:?}");
        }
        // Die Volumen bleiben zusammen erhalten
        let v1 = signed_volume(&m);
        let v2 = signed_volume(&v) + signed_volume(&h);
        assert!(
            (v1 - v2).abs() < v1.abs() * 0.02,
            "Die Teile passen nicht zusammen: {v1} statt {v2}"
        );
    }
}

#[cfg(test)]
mod hohlprism_tests {
    use super::*;

    #[test]
    fn hohlprism_ist_ein_rohr() {
        // Ein quadratisches Rohr: aussen 10x10, innen 6x6, 5 mm hoch
        let aussen = vec![(0.0, 0.0), (10.0, 0.0), (10.0, 10.0), (0.0, 10.0)];
        let innen = vec![(2.0, 2.0), (8.0, 2.0), (8.0, 8.0), (2.0, 8.0)];
        let m = hohlprism(&aussen, &innen, 0.0, 5.0);
        assert!(m.faces.len() >= 16, "zu wenige Flaechen: {}", m.faces.len());
        // Das Volumen muss der Ringflaeche mal Hoehe entsprechen
        let v = signed_volume(&m);
        let erwartet = (100.0 - 36.0) * 5.0;
        assert!(
            (v - erwartet).abs() < 1.0,
            "Volumen {v} statt {erwartet}"
        );
    }
}
