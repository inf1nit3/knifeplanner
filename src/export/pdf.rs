//! Minimaler PDF-Writer (ohne Fremdbibliothek).
//!
//! Erzeugt PDF 1.4 mit den Standard-14-Schriften Helvetica und Helvetica-Bold
//! (WinAnsiEncoding, damit Umlaute funktionieren). Das Koordinatensystem der
//! API hat den Ursprung oben links, PDF unten links - die Umrechnung passiert
//! intern.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

/// Punkte pro Millimeter.
const PT_PER_MM: f64 = 72.0 / 25.4;

struct Page {
    w: f64,
    h: f64,
    ops: String,
}

pub struct Pdf {
    pages: Vec<Page>,
    cur: Option<usize>,
    fill: (f64, f64, f64),
    stroke: (f64, f64, f64),
    line_width: f64,
}

impl Default for Pdf {
    fn default() -> Self {
        Self::new()
    }
}

impl Pdf {
    pub fn new() -> Pdf {
        Pdf {
            pages: Vec::new(),
            cur: None,
            fill: (0.0, 0.0, 0.0),
            stroke: (0.0, 0.0, 0.0),
            line_width: 0.3,
        }
    }

    pub fn add_page(&mut self, w: f64, h: f64) {
        self.pages.push(Page {
            w,
            h,
            ops: String::new(),
        });
        self.cur = Some(self.pages.len() - 1);
    }

    fn page(&self) -> &Page {
        &self.pages[self.cur.expect("keine Seite angelegt")]
    }

    fn page_mut(&mut self) -> &mut Page {
        let i = self.cur.expect("keine Seite angelegt");
        &mut self.pages[i]
    }

    fn page_height(&self) -> f64 {
        self.page().h
    }

    pub fn set_fill(&mut self, r: f64, g: f64, b: f64) {
        self.fill = (r, g, b);
        let _ = writeln!(self.page_mut().ops, "{r:.3} {g:.3} {b:.3} rg");
    }

    pub fn set_stroke(&mut self, r: f64, g: f64, b: f64) {
        self.stroke = (r, g, b);
        let _ = writeln!(self.page_mut().ops, "{r:.3} {g:.3} {b:.3} RG");
    }

    pub fn set_line_width(&mut self, w: f64) {
        self.line_width = w;
        let _ = writeln!(self.page_mut().ops, "{w:.3} w");
    }

    pub fn set_dash(&mut self, pattern: &[f64]) {
        let p: Vec<String> = pattern.iter().map(|v| format!("{v:.2}")).collect();
        let _ = writeln!(self.page_mut().ops, "[{}] 0 d", p.join(" "));
    }

    pub fn clear_dash(&mut self) {
        let _ = writeln!(self.page_mut().ops, "[] 0 d");
    }

    pub fn line(&mut self, x1: f64, y1: f64, x2: f64, y2: f64) {
        let h = self.page_height();
        let _ = writeln!(
            self.page_mut().ops,
            "{:.3} {:.3} m {:.3} {:.3} l S",
            x1 * PT_PER_MM,
            (h - y1) * PT_PER_MM,
            x2 * PT_PER_MM,
            (h - y2) * PT_PER_MM
        );
    }

    pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let ph = self.page_height();
        let _ = writeln!(
            self.page_mut().ops,
            "{:.3} {:.3} {:.3} {:.3} re S",
            x * PT_PER_MM,
            (ph - y - h) * PT_PER_MM,
            w * PT_PER_MM,
            h * PT_PER_MM
        );
    }

    pub fn fill_rect(&mut self, x: f64, y: f64, w: f64, h: f64) {
        let ph = self.page_height();
        let _ = writeln!(
            self.page_mut().ops,
            "{:.3} {:.3} {:.3} {:.3} re f",
            x * PT_PER_MM,
            (ph - y - h) * PT_PER_MM,
            w * PT_PER_MM,
            h * PT_PER_MM
        );
    }

    pub fn polyline(&mut self, pts: &[(f64, f64)], close: bool) {
        if pts.len() < 2 {
            return;
        }
        let h = self.page_height();
        let mut ops = String::new();
        let _ = writeln!(
            ops,
            "{:.3} {:.3} m",
            pts[0].0 * PT_PER_MM,
            (h - pts[0].1) * PT_PER_MM
        );
        for &(x, y) in &pts[1..] {
            let _ = writeln!(ops, "{:.3} {:.3} l", x * PT_PER_MM, (h - y) * PT_PER_MM);
        }
        if close {
            ops.push_str("h ");
        }
        ops.push_str("S");
        self.page_mut().ops.push_str(&ops);
        self.page_mut().ops.push('\n');
    }

    pub fn circle(&mut self, cx: f64, cy: f64, r: f64) {
        const K: f64 = 0.552_284_749_83;
        let h = self.page_height();
        let cx = cx * PT_PER_MM;
        let cy = (h - cy) * PT_PER_MM;
        let r = r * PT_PER_MM;
        let k = r * K;
        let mut ops = String::new();
        let _ = writeln!(ops, "{:.3} {:.3} m", cx + r, cy);
        let _ = writeln!(ops, "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c", cx + r, cy + k, cx + k, cy + r, cx, cy + r);
        let _ = writeln!(ops, "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c", cx - k, cy + r, cx - r, cy + k, cx - r, cy);
        let _ = writeln!(ops, "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c", cx - r, cy - k, cx - k, cy - r, cx, cy - r);
        let _ = writeln!(ops, "{:.3} {:.3} {:.3} {:.3} {:.3} {:.3} c", cx + k, cy - r, cx + r, cy - k, cx + r, cy);
        ops.push_str("S");
        self.page_mut().ops.push_str(&ops);
        self.page_mut().ops.push('\n');
    }

    /// Text an Position (x, y in mm) - y ist der Abstand von oben.
    /// `size` ist in Punkt angegeben.
    pub fn text(&mut self, x: f64, y: f64, size: f64, text: &str, bold: bool) {
        let h = self.page_height();
        let font = if bold { "/F2" } else { "/F1" };
        let escaped = escape_text(text);
        let _ = writeln!(
            self.page_mut().ops,
            "BT {font} {size:.2} Tf 1 0 0 1 {:.2} {:.2} Tm ({escaped}) Tj ET",
            x * PT_PER_MM,
            (h - y - size * 0.35) * PT_PER_MM
        );
    }

    /// Text mit einfacher Word-Wrapping-Logik. `max_width` in mm.
    pub fn text_block(
        &mut self,
        x: f64,
        y: f64,
        size: f64,
        max_width: f64,
        text: &str,
        bold: bool,
        leading: f64,
    ) -> f64 {
        let limit = max_width * PT_PER_MM;
        let mut cursor = y;
        for para in text.split('\n') {
            let mut line = String::new();
            for word in para.split_whitespace() {
                let candidate = if line.is_empty() {
                    word.to_string()
                } else {
                    format!("{line} {word}")
                };
                if estimate_width(&candidate, size) > limit && !line.is_empty() {
                    self.text(x, cursor, size, &line, bold);
                    cursor += leading;
                    line = word.to_string();
                } else {
                    line = candidate;
                }
            }
            if !line.is_empty() {
                self.text(x, cursor, size, &line, bold);
                cursor += leading;
            }
        }
        cursor
    }

    /// Schreibt die PDF-Datei.
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut buf: Vec<u8> = Vec::new();
        buf.extend_from_slice(b"%PDF-1.4\n%\xE2\xE3\xCF\xD3\n");

        let mut offsets: Vec<usize> = Vec::new();
        let mut objects: Vec<Vec<u8>> = Vec::new();

        // 1: Catalog, 2: Pages
        objects.push(b"<< /Type /Catalog /Pages 2 0 R >>".to_vec());
        objects.push(b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec());
        objects.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );
        objects.push(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica-Bold /Encoding /WinAnsiEncoding >>"
                .to_vec(),
        );

        // Seitenobjekte und Inhaltsstroeme
        let first_page_obj = 5;
        let kids: Vec<String> = (0..self.pages.len())
            .map(|i| format!("{} 0 R", first_page_obj + i * 2))
            .collect();
        objects[1] = format!(
            "<< /Type /Pages /Kids [{}] /Count {} >>",
            kids.join(" "),
            self.pages.len()
        )
        .into_bytes();

        for page in &self.pages {
            let content = format!(
                "q\n{:.3} {:.3} {:.3} RG\n{:.3} w\n{}Q\n",
                0.0, 0.0, 0.0, 0.3, page.ops
            );
            // Der Inhaltsstrom wird als Latin-1 geschrieben: die Schrift
            // ist auf WinAnsiEncoding gestellt, dort ist ein Umlaut genau
            // ein Byte (0xE4 = ae). Als UTF-8 waeren es zwei Bytes und die
            // PDF-Viewer zeigten "grÃ¤n".
            let content_bytes: Vec<u8> = content
                .chars()
                .map(|c| if (c as u32) < 256 { c as u8 } else { b'?' })
                .collect();
            // Das Seitenobjekt bekommt die Nummer objects.len() + 1,
            // der Inhaltsstrom darunter objects.len() + 2.
            let content_obj = objects.len() + 2;
            objects.push(
                format!(
                    "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 {:.2} {:.2}] \
                     /Resources << /Font << /F1 3 0 R /F2 4 0 R >> >> /Contents {} 0 R >>",
                    page.w * PT_PER_MM,
                    page.h * PT_PER_MM,
                    content_obj
                )
                .into_bytes(),
            );
            let mut stream = format!("<< /Length {} >>\nstream\n", content_bytes.len()).into_bytes();
            stream.extend_from_slice(&content_bytes);
            stream.extend_from_slice(b"\nendstream");
            objects.push(stream);
        }

        for obj in &objects {
            offsets.push(buf.len());
            buf.extend_from_slice(format!("{} 0 obj\n", offsets.len()).as_bytes());
            buf.extend_from_slice(obj);
            buf.extend_from_slice(b"\nendobj\n");
        }

        let xref_pos = buf.len();
        buf.extend_from_slice(format!("xref\n0 {}\n", objects.len() + 1).as_bytes());
        buf.extend_from_slice(b"0000000000 65535 f \n");
        for off in &offsets {
            buf.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
        }
        buf.extend_from_slice(
            format!(
                "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref_pos}\n%%EOF\n",
                objects.len() + 1
            )
            .as_bytes(),
        );

        fs::write(path, &buf)?;
        Ok(())
    }
}

fn escape_text(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        match c {
            '(' | ')' | '\\' => {
                out.push('\\');
                out.push(c);
            }
            // Typografische Zeichen, die es in WinAnsi nicht gibt, werden
            // auf ihre naechstliegende Entsprechung gebracht.
            '\u{2013}' | '\u{2014}' => out.push('-'),
            '\u{2018}' | '\u{2019}' => out.push('\''),
            '\u{201c}' | '\u{201d}' => out.push('"'),
            '\u{00a0}' => out.push(' '),
            c if (c as u32) < 256 => out.push(c as u8 as char),
            _ => out.push('?'),
        }
    }
    out
}

/// Grobe Breitenabschaetzung fuer Helvetica (fuer den Textumbruch).
pub fn estimate_width(s: &str, size: f64) -> f64 {
    let mut units = 0.0;
    for c in s.chars() {
        units += match c {
            'i' | 'l' | 'j' | 't' | 'f' | 'I' | '.' | ',' | '\'' | '|' | ':' | ';' | '!' => 0.28,
            ' ' => 0.28,
            'm' | 'w' | 'M' | 'W' => 0.85,
            'A'..='Z' => 0.68,
            _ => 0.53,
        };
    }
    units * size
}
