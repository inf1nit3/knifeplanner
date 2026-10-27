//! Software-Renderer fuer das 3D-Modell des Griffes.
//!
//! Z-Buffer-Rasterizer mit perspektivischer Projektion, glaetten Normalen,
//! Studio-Licht (Key/Fill/Rim) und optionaler Kantenglaettung durch
//! Supersampling. Der Renderer laeuft auf der CPU - bei einem Griffmodell mit
//! wenigen tausend Dreiecken ist das auf jedem aktuellen Rechner flüssig.

use crate::mesh::{smooth_normals, Mesh};

/// Ein Volumen mit Farbe und Materialart.
pub struct Part {
    pub mesh: Mesh,
    pub color: [f32; 3],
    pub kind: PartKind,
}

/// Materialart - steuert die Oberflaechendarstellung.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PartKind {
    /// Holz: bekommt eine prozedurale Maserung
    Wood,
    /// Stahl (Angel, Klingen)
    Steel,
    /// Messing, Edelstahl, Titan
    Metal,
    /// Ausgesparte Volumen (Frasbahn)
    Cavity,
}

/// Die komplette Szene.
pub struct Scene {
    pub parts: Vec<Part>,
}

/// Kameraposition und Blickrichtung.
#[derive(Clone, Copy, Debug)]
pub struct Camera {
    /// Drehung um die Hochachse (Grad)
    pub yaw: f32,
    /// Drehung um die Laengsachse des Messers (Grad)
    pub roll: f32,
    /// Abstand zur Kamera
    pub dist: f32,
    /// Bildwinkel in Grad
    pub fov: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Camera {
            yaw: 32.0,
            roll: -22.0,
            dist: 0.0,
            fov: 32.0,
        }
    }
}

impl Camera {
    pub fn reset(&mut self) {
        self.yaw = 32.0;
        self.roll = -22.0;
        self.dist = 0.0;
    }
}

#[inline]
fn rot_x(p: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [p[0], p[1] * c - p[2] * s, p[1] * s + p[2] * c]
}

#[inline]
fn rot_y(p: [f32; 3], a: f32) -> [f32; 3] {
    let (s, c) = a.sin_cos();
    [p[0] * c + p[2] * s, p[1], -p[0] * s + p[2] * c]
}

#[inline]
fn norm(v: [f32; 3]) -> [f32; 3] {
    let l = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
    if l > 1e-9 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        [0.0, 1.0, 0.0]
    }
}

/// Hash-basierte Rauschfunktion fuer die Holzmaserung.
fn hash2(x: f32, y: f32) -> f32 {
    let s = (x * 12.9898 + y * 78.233).sin() * 43758.5453;
    s - s.floor()
}

/// Glaettes Wertrauschen.
fn vnoise(x: f32, y: f32) -> f32 {
    let xi = x.floor();
    let yi = y.floor();
    let xf = x - xi;
    let yf = y - yi;
    let u = xf * xf * (3.0 - 2.0 * xf);
    let v = yf * yf * (3.0 - 2.0 * yf);
    let a = hash2(xi, yi);
    let b = hash2(xi + 1.0, yi);
    let c = hash2(xi, yi + 1.0);
    let d = hash2(xi + 1.0, yi + 1.0);
    a + (b - a) * u + (c - a) * v + (a - b - c + d) * u * v
}

/// Prozedurale Holzmaserung: 0 = dunkel, 1 = hell.
///
/// Die Adern laufen entlang der Grifflaenge (x) und werden quer zur
/// Schichtung (z) gestoert - so wie bei einem echten Bohlen.
fn wood_grain(p: [f32; 3]) -> f32 {
    let warp = vnoise(p[1] * 0.22, p[2] * 1.1) * 1.5 + vnoise(p[0] * 0.03, p[1] * 0.5) * 0.7;
    let band = (p[0] * 0.13 + warp) * 1.0;
    let base = (band.sin() * 0.5 + 0.5).powf(1.5);
    let pores = vnoise(p[0] * 3.5, p[2] * 5.0) * 0.07;
    (base * 0.92 + pores).clamp(0.0, 1.0)
}

/// Feine Schattierung fuer Stahl (Schliffspuren).
fn steel_shade(p: [f32; 3]) -> f32 {
    let s = vnoise(p[0] * 3.0, p[1] * 26.0) * 0.10 + vnoise(p[1] * 30.0, p[2] * 3.0) * 0.06;
    (0.94 + s).min(1.06)
}

/// Mittelpunkt und Radius der Szene.
fn scene_bounds(scene: &Scene) -> ([f32; 3], f32) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for part in &scene.parts {
        for v in &part.mesh.vertices {
            let v = [v.0 as f32, v.1 as f32, v.2 as f32];
            for k in 0..3 {
                min[k] = min[k].min(v[k]);
                max[k] = max[k].max(v[k]);
            }
        }
    }
    if min[0] > max[0] {
        return ([0.0; 3], 1.0);
    }
    let center = [
        (min[0] + max[0]) / 2.0,
        (min[1] + max[1]) / 2.0,
        (min[2] + max[2]) / 2.0,
    ];
    let mut r: f32 = 0.0;
    for part in &scene.parts {
        for v in &part.mesh.vertices {
            let d = [
                v.0 as f32 - center[0],
                v.1 as f32 - center[1],
                v.2 as f32 - center[2],
            ];
            r = r.max((d[0] * d[0] + d[1] * d[1] + d[2] * d[2]).sqrt());
        }
    }
    (center, r.max(1.0))
}

/// Rendert die Szene in einen RGBA-Puffer (w * h * 4 Bytes).
///
/// `samples` ist 1 (schnell) oder 4 (2x2 Supersampling fuer glatte Kanten).
pub fn render(scene: &Scene, w: usize, h: usize, cam: &Camera, samples: usize) -> Vec<u8> {
    let ss = if samples >= 4 { 2 } else { 1 };
    let rw = w * ss;
    let rh = h * ss;

    // Der Puffer hat immer die Supersampling-Aufloesung; verkleinert wird
    // erst am Ende.
    let mut out = vec![0u8; rw * rh * 4];
    if rw == 0 || rh == 0 {
        return out;
    }

    // Hintergrund: sanfter Studio-Verlauf mit Vignette
    for y in 0..rh {
        let t = y as f32 / rh as f32;
        let r = (242.0 - 34.0 * t) as f32;
        let g = (240.0 - 36.0 * t) as f32;
        let b = (236.0 - 38.0 * t) as f32;
        for x in 0..rw {
            let fx = x as f32 / rw as f32 - 0.5;
            let fy = y as f32 / rh as f32 - 0.5;
            let vig = 1.0 - (fx * fx + fy * fy) * 0.55;
            let idx = (y * rw + x) * 4;
            out[idx] = (r * vig).clamp(0.0, 255.0) as u8;
            out[idx + 1] = (g * vig).clamp(0.0, 255.0) as u8;
            out[idx + 2] = (b * vig).clamp(0.0, 255.0) as u8;
            out[idx + 3] = 255;
        }
    }

    let (center, radius) = scene_bounds(scene);
    let dist = if cam.dist > 0.1 {
        cam.dist
    } else {
        radius * 3.9
    };
    let fov = cam.fov.to_radians();
    let focal = (rh as f32 / 2.0) / (fov / 2.0).tan();

    let yaw = cam.yaw.to_radians();
    let roll = cam.roll.to_radians();

    // Weicher Bodenschatten unter dem Griff
    {
        let mut p = [
            center[0] as f32 - center[0] as f32,
            center[1] as f32 - center[1] as f32,
            center[2] as f32 - center[2] as f32,
        ];
        p = rot_x(p, roll);
        p = rot_y(p, yaw);
        p[2] -= dist;
        if p[2] < -1.0 {
            let inv = focal / -p[2];
            let sx = rw as f32 / 2.0 + p[0] * inv;
            let sy = rh as f32 / 2.0 - p[1] * inv;
            let rx = radius * 1.35 * inv * 0.5;
            let ry = rx * 0.16;
            let cy = sy + rx * 0.42;
            let x0 = (sx - rx).floor().max(0.0) as usize;
            let x1 = ((sx + rx).ceil() as isize).clamp(0, rw as isize - 1) as usize;
            let y0 = (cy - ry).floor().max(0.0) as usize;
            let y1 = ((cy + ry).ceil() as isize).clamp(0, rh as isize - 1) as usize;
            for py in y0..=y1 {
                for px in x0..=x1 {
                    let dx = (px as f32 + 0.5 - sx) / rx;
                    let dy = (py as f32 + 0.5 - cy) / ry;
                    let d = (dx * dx + dy * dy).sqrt();
                    if d < 1.0 {
                        let a = (1.0 - d) * (1.0 - d) * 0.42;
                        let idx = (py * rw + px) * 4;
                        let k = 1.0 - a;
                        out[idx] = (out[idx] as f32 * k) as u8;
                        out[idx + 1] = (out[idx + 1] as f32 * k) as u8;
                        out[idx + 2] = (out[idx + 2] as f32 * k) as u8;
                    }
                }
            }
        }
    }

    // Lichtrichtungen im Kamerasystem
    let key = norm([-0.45, 0.68, 0.58]);
    let fill = norm([0.62, -0.32, 0.45]);
    let rim = norm([0.15, 0.30, -0.94]);
    // weiches Bodenlicht
    let bounce = norm([0.10, -0.92, 0.38]);

    // Die projizierte Tiefe ist negativ (Kamera schaut nach -z), daher ist
    // die naeher gelegene Flaeche die mit dem GROESSEREN Wert.
    let mut zbuf = vec![f32::MIN; rw * rh];

    for part in &scene.parts {
        let normals = smooth_normals(&part.mesh);
        let count = part.mesh.vertices.len();

        // Transformation in den Kameraraum
        let mut cam_pts: Vec<[f32; 3]> = Vec::with_capacity(count);
        let mut model_pts: Vec<[f32; 3]> = Vec::with_capacity(count);
        let mut screen: Vec<(f32, f32)> = Vec::with_capacity(count);
        for v in &part.mesh.vertices {
            let model = [v.0 as f32, v.1 as f32, v.2 as f32];
            model_pts.push(model);
            let mut p = [
                model[0] - center[0],
                model[1] - center[1],
                model[2] - center[2],
            ];
            p = rot_x(p, roll);
            p = rot_y(p, yaw);
            p[2] -= dist;
            cam_pts.push(p);
            if p[2] > -1.0 {
                screen.push((0.0, 0.0));
            } else {
                let inv = focal / -p[2];
                screen.push((
                    rw as f32 / 2.0 + p[0] * inv,
                    rh as f32 / 2.0 - p[1] * inv,
                ));
            }
        }

        let base = part.color;
        for &(ia, ib, ic) in &part.mesh.faces {
            let p0 = cam_pts[ia];
            let p1 = cam_pts[ib];
            let p2 = cam_pts[ic];
            // Near-Plane: Dreiecke teilweise dahinter werden uebersprungen
            if p0[2] > -1.0 || p1[2] > -1.0 || p2[2] > -1.0 {
                continue;
            }
            let (x0, y0) = screen[ia];
            let (x1, y1) = screen[ib];
            let (x2, y2) = screen[ic];

            let area = (x1 - x0) * (y2 - y0) - (x2 - x0) * (y1 - y0);
            // Die Bildschirmachse y zeigt nach unten, eine zur Kamera
            // zeigende Flaeche erscheint daher mit negativer Flaeche.
            if area >= 0.0 {
                continue; // Rueckseite
            }

            let min_x = x0.min(x1).min(x2).floor().max(0.0) as usize;
            let max_x = (x0.max(x1).max(x2).ceil() as isize).clamp(0, rw as isize - 1) as usize;
            let min_y = y0.min(y1).min(y2).floor().max(0.0) as usize;
            let max_y = (y0.max(y1).max(y2).ceil() as isize).clamp(0, rh as isize - 1) as usize;
            if min_x > max_x || min_y > max_y {
                continue;
            }

            let inv_area = 1.0 / area;
            for py in min_y..=max_y {
                for px in min_x..max_x {
                    let fx = px as f32 + 0.5;
                    let fy = py as f32 + 0.5;
                    let w0 = ((x1 - fx) * (y2 - fy) - (x2 - fx) * (y1 - fy)) * inv_area;
                    let w1 = ((x2 - fx) * (y0 - fy) - (x0 - fx) * (y2 - fy)) * inv_area;
                    let w2 = 1.0 - w0 - w1;
                    if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                        continue;
                    }
                    // Perspektivisch korrekte Tiefe
                    let iz = w0 / p0[2] + w1 / p1[2] + w2 / p2[2];
                    let z = 1.0 / iz;
                    let idx = py * rw + px;
                    if z <= zbuf[idx] {
                        continue;
                    }
                    zbuf[idx] = z;

                    // Normale interpolieren (Naeherung, perspektivisch gewichtet)
                    let n0 = normals[ia];
                    let n1 = normals[ib];
                    let n2 = normals[ic];
                    let mut n = [0.0f32; 3];
                    for k in 0..3 {
                        n[k] = w0 * n0[k] / p0[2]
                            + w1 * n1[k] / p1[2]
                            + w2 * n2[k] / p2[2];
                    }
                    let n = norm([n[0] * z, n[1] * z, n[2] * z]);

                    // Materialstruktur (Maserung, Schliffspuren)
                    let mp = [
                        (w0 * model_pts[ia][0] + w1 * model_pts[ib][0] + w2 * model_pts[ic][0]),
                        (w0 * model_pts[ia][1] + w1 * model_pts[ib][1] + w2 * model_pts[ic][1]),
                        (w0 * model_pts[ia][2] + w1 * model_pts[ib][2] + w2 * model_pts[ic][2]),
                    ];
                    let tex = match part.kind {
                        PartKind::Wood => 0.86 + 0.22 * wood_grain(mp),
                        PartKind::Steel => steel_shade(mp),
                        PartKind::Metal => 0.96 + 0.08 * vnoise(mp[0] * 2.0, mp[2] * 2.0),
                        PartKind::Cavity => 0.85,
                    };
                    let base = [base[0] * tex, base[1] * tex, base[2] * tex];

                    // Blickrichtung
                    let view = norm([-p0[0], -p0[1], -p0[2]]);

                    let diff_key = (n[0] * key[0] + n[1] * key[1] + n[2] * key[2]).max(0.0);
                    let diff_fill = (n[0] * fill[0] + n[1] * fill[1] + n[2] * fill[2]).max(0.0);
                    let diff_rim = (n[0] * rim[0] + n[1] * rim[1] + n[2] * rim[2]).max(0.0);
                    let diff_bounce = (n[0] * bounce[0] + n[1] * bounce[1] + n[2] * bounce[2]).max(0.0);

                    // Spiegelung (Blinn-Phong)
                    let mut h = [0.0f32; 3];
                    for k in 0..3 {
                        h[k] = key[k] + view[k];
                    }
                    let h = norm(h);
                    let ndh = (n[0] * h[0] + n[1] * h[1] + n[2] * h[2]).max(0.0);
                    let spec = ndh.powi(42) * 0.45 + ndh.powi(6) * 0.10;

                    // Hemisphaerisches Umgebungslicht
                    let sky = 0.5 + 0.5 * n[1];
                    let ambient = 0.26 + 0.20 * sky;

                    let fresnel = (1.0 - (n[0] * view[0] + n[1] * view[1] + n[2] * view[2]).abs())
                        .powi(3);

                    let mut col = [0.0f32; 3];
                    for k in 0..3 {
                        let lit = base[k]
                            * (ambient + diff_key * 0.92 + diff_fill * 0.40 + diff_bounce * 0.24)
                            + spec * 0.85
                            + diff_rim * 0.24 * base[k]
                            + fresnel * 0.18;
                        col[k] = lit.clamp(0.0, 1.0);
                    }
                    // Leichte Kontrastanhebung
                    for k in 0..3 {
                        col[k] = col[k].powf(0.92);
                    }

                    let o = (idx * 4) as usize;
                    out[o] = (col[0] * 255.0) as u8;
                    out[o + 1] = (col[1] * 255.0) as u8;
                    out[o + 2] = (col[2] * 255.0) as u8;
                    out[o + 3] = 255;
                }
            }
        }
    }

    // Supersampling: auf die Zielgroesse verkleinern
    if ss > 1 {
        let mut small = vec![0u8; w * h * 4];
        for y in 0..h {
            for x in 0..w {
                let mut acc = [0u32; 4];
                for sy in 0..ss {
                    for sx in 0..ss {
                        let src = ((y * ss + sy) * rw + (x * ss + sx)) * 4;
                        for k in 0..4 {
                            acc[k] += out[src + k] as u32;
                        }
                    }
                }
                let n = (ss * ss) as u32;
                let d = (y * w + x) * 4;
                for k in 0..4 {
                    small[d + k] = (acc[k] / n) as u8;
                }
            }
        }
        return small;
    }

    out
}

/// Speichert den Render als PNG-Datei.
pub fn save_png(
    path: &std::path::Path,
    rgba: &[u8],
    w: usize,
    h: usize,
) -> std::io::Result<()> {
    use image::RgbaImage;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let img = RgbaImage::from_raw(w as u32, h as u32, rgba.to_vec())
        .ok_or_else(|| std::io::Error::other("Bildpuffer passt nicht zur Groesse"))?;
    img.save(path)
        .map_err(|e| std::io::Error::other(e.to_string()))
}
