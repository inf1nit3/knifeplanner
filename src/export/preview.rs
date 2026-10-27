//! Raster-Vorschau (PNG) des Griffplans - ohne UI-Bibliothek.
//!
//! Eigener einfacher Rasterizer: Scanline-Fuellung fuer Flaechen, Linien per
//! Bresenham, Kreise per Ellipsenrasterung.

use image::{Rgb, RgbImage};

use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};
use crate::materials::MaterialVerdict;

use super::Frame;

type Point = (f64, f64);

struct Canvas {
    img: RgbImage,
    scale: f64,
    ox: f64,
    oy: f64,
}

impl Canvas {
    fn new(frame: Frame, target_width: u32) -> Canvas {
        let scale = target_width as f64 / frame.width().max(1.0);
        let w = target_width.max(100);
        let h = ((frame.height() * scale) as u32).max(50);
        Canvas {
            img: RgbImage::from_pixel(w, h, Rgb([252, 251, 248])),
            scale,
            ox: -frame.min_x * scale,
            oy: -frame.min_y * scale,
        }
    }

    fn to_px(&self, x: f64, y: f64) -> (f64, f64) {
        (x * self.scale + self.ox, y * self.scale + self.oy)
    }

    fn set(&mut self, x: i32, y: i32, c: Rgb<u8>) {
        if x >= 0 && y >= 0 && (x as u32) < self.img.width() && (y as u32) < self.img.height() {
            self.img.put_pixel(x as u32, y as u32, c);
        }
    }

    fn line(&mut self, a: Point, b: Point, c: Rgb<u8>, width: f64) {
        let (x0, y0) = self.to_px(a.0, a.1);
        let (x1, y1) = self.to_px(b.0, b.1);
        let steps = (((x1 - x0).abs()).max((y1 - y0).abs())).ceil().max(1.0) as usize;
        let half = (width * self.scale / 2.0).max(0.4);
        for i in 0..=steps {
            let t = i as f64 / steps as f64;
            let x = x0 + (x1 - x0) * t;
            let y = y0 + (y1 - y0) * t;
            let r = half.ceil() as i32;
            for dy in -r..=r {
                for dx in -r..=r {
                    if (dx * dx + dy * dy) as f64 <= half * half + 0.4 {
                        self.set(x as i32 + dx, y as i32 + dy, c);
                    }
                }
            }
        }
    }

    fn fill_polygon(&mut self, pts: &[Point], c: Rgb<u8>) {
        if pts.len() < 3 {
            return;
        }
        let px: Vec<(f64, f64)> = pts.iter().map(|&p| self.to_px(p.0, p.1)).collect();
        let min_y = px.iter().map(|p| p.1).fold(f64::MAX, f64::min).floor() as i32;
        let max_y = px.iter().map(|p| p.1).fold(f64::MIN, f64::max).ceil() as i32;
        let n = px.len();
        for y in min_y..=max_y {
            let sy = y as f64 + 0.5;
            let mut xs: Vec<f64> = Vec::new();
            for i in 0..n {
                let (x1, y1) = px[i];
                let (x2, y2) = px[(i + 1) % n];
                if (y1 <= sy && y2 > sy) || (y2 <= sy && y1 > sy) {
                    let t = (sy - y1) / (y2 - y1);
                    xs.push(x1 + t * (x2 - x1));
                }
            }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let mut k = 0;
            while k + 1 < xs.len() {
                let (x_start, x_end) = (xs[k], xs[k + 1]);
                for x in (x_start.round() as i32)..=(x_end.round() as i32) {
                    self.set(x, y, c);
                }
                k += 2;
            }
        }
    }

    fn circle(&mut self, center: Point, r_mm: f64, color: Rgb<u8>, fill: bool) {
        let (cx, cy) = self.to_px(center.0, center.1);
        let r = r_mm * self.scale;
        let ri = r.ceil() as i32;
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f64).sqrt();
                if (fill && d <= r) || (!fill && (d - r).abs() <= 0.8) {
                    self.set(cx as i32 + dx, cy as i32 + dy, color);
                }
            }
        }
    }
}

/// Rendert den Griffplan als PNG.
pub fn render_preview(
    geo: &HandleGeometry,
    blank: &BladeBlank,
    _params: &HandleParams,
    _material: &MaterialVerdict,
    path: &std::path::Path,
) -> std::io::Result<()> {
    let frame = Frame::from_geometry(geo, blank);
    let mut c = Canvas::new(frame, 1400);
    let wood = Rgb([232, 220, 196]);
    let edge = Rgb([43, 33, 24]);
    let tang = Rgb([31, 122, 63]);
    let hole = Rgb([198, 40, 40]);
    let metal = Rgb([207, 214, 220]);
    let recess = Rgb([178, 106, 0]);

    // Tangmittenlinie
    c.line(
        (frame.min_x, 0.0),
        (frame.max_x, 0.0),
        Rgb([190, 190, 190]),
        0.5,
    );

    if let Some(b) = &geo.bolster_profile {
        c.fill_polygon(b, metal);
        for i in 0..b.len() {
            c.line(b[i], b[(i + 1) % b.len()], edge, 1.0);
        }
    }
    if let Some(f) = &geo.ferrule_profile {
        c.fill_polygon(f, metal);
        for i in 0..f.len() {
            c.line(f[i], f[(i + 1) % f.len()], edge, 1.0);
        }
    }

    c.fill_polygon(&geo.outline, wood);
    for i in 0..geo.outline.len() {
        c.line(geo.outline[i], geo.outline[(i + 1) % geo.outline.len()], edge, 1.2);
    }

    // Angel (gestrichelt angedeutet)
    for i in 0..geo.tang_profile.len() {
        let a = geo.tang_profile[i];
        let b = geo.tang_profile[(i + 1) % geo.tang_profile.len()];
        if (i % 6) < 3 {
            c.line(a, b, tang, 1.0);
        }
    }
    for i in 0..geo.recess.len() {
        let a = geo.recess[i];
        let b = geo.recess[(i + 1) % geo.recess.len()];
        if (i % 6) < 3 {
            c.line(a, b, recess, 0.8);
        }
    }

    for pin in &geo.pins {
        c.circle((pin.x_mm, pin.y_mm), pin.dia_mm / 2.0, hole, true);
        c.circle((pin.x_mm, pin.y_mm), pin.dia_mm / 2.0 + 0.4, hole, false);
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    c.img
        .save(path)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e.to_string()))?;
    Ok(())
}

/// Beschreibt die aktuelle Planvorschau in einer Zeile (fuer die UI).
pub fn summary(geo: &HandleGeometry, params: &HandleParams, material: &MaterialVerdict) -> String {
    let style = match params.style {
        HandleStyle::FullTangTwoScale => "2 Griffschalen",
        HandleStyle::PartialTangBolster => "Bastion",
        HandleStyle::OnePiece => "Ein Stueck",
        HandleStyle::Ferrule => "Zwinge",
    };
    format!(
        "{} | {} | {:.0} x {:.0} x {:.1} mm | {}",
        style,
        material.material_name,
        geo.handle_length_mm,
        geo.handle_height_mm,
        geo.total_thickness_mm,
        material.rating
    )
}
