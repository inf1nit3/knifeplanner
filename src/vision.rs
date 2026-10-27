//! Bildanalyse mit dem MiniMax-Vision-Modell.
//!
//! Der Client nutzt die Anthropic-kompatible Messages-Schnittstelle. Die
//! Zugangsdaten kommen aus (in dieser Reihenfolge):
//!   1. Umgebungsvariable MINIMAX_API_KEY
//!   2. Nutzerkonfiguration (~/.knifeplanner/config.json bzw. %APPDATA%)
//!   3. lokale MiniMax-Authentifizierung (Entwicklung auf dem Mac)

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use base64::Engine;
use serde::{Deserialize, Serialize};

use crate::model::{BladeBlank, TangStyle};

// ---------------------------------------------------------------------------
// Konfiguration
// ---------------------------------------------------------------------------

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct ApiConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
}

impl ApiConfig {
    pub fn base_url(&self) -> String {
        if self.base_url.trim().is_empty() {
            "https://agent.minimax.io/mavis/api/v1/llm/v1".to_string()
        } else {
            self.base_url.trim().trim_end_matches('/').to_string()
        }
    }
    pub fn model(&self) -> &str {
        if self.model.trim().is_empty() {
            "MiniMax-M3"
        } else {
            self.model.trim()
        }
    }
    pub fn is_configured(&self) -> bool {
        !self.api_key.trim().is_empty()
    }
    /// Arbeitet das Modell erst lange nach, bevor es antwortet?
    pub fn is_reasoning(&self) -> bool {
        let m = self.model().to_lowercase();
        m.contains("flash") || m.contains("thinking") || m.contains("reason")
    }
}

/// Pfad der Nutzerkonfiguration je Plattform.
pub fn config_path() -> PathBuf {
    if cfg!(target_os = "windows") {
        if let Some(appdata) = std::env::var_os("APPDATA") {
            return PathBuf::from(appdata).join("knifeplanner").join("config.json");
        }
    }
    if let Some(home) = std::env::var_os("HOME") {
        return PathBuf::from(home).join(".knifeplanner").join("config.json");
    }
    PathBuf::from("knifeplanner-config.json")
}

/// Die Modelle, die mit Bild-Eingabe arbeiten.
pub const KNOWN_MODELS: &[(&str, &str)] = &[
    ("MiniMax-M3", "M3 - starkes Modell, antwortet direkt"),
    ("MiniMax-M3.1-Flash", "M3.1-Flash - kostenlos, denkt erst lange nach"),
];

/// Laedt die API-Konfiguration aus allen Quellen.
pub fn load_config() -> ApiConfig {
    let mut cfg = ApiConfig::default();

    if let Ok(text) = std::fs::read_to_string(config_path()) {
        if let Ok(v) = serde_json::from_str::<ApiConfig>(&text) {
            cfg = v;
        }
    }

    if let Ok(key) = std::env::var("MINIMAX_API_KEY") {
        if !key.trim().is_empty() {
            cfg.api_key = key.trim().to_string();
        }
    }
    if let Ok(url) = std::env::var("MINIMAX_BASE_URL") {
        if !url.trim().is_empty() {
            cfg.base_url = url.trim().to_string();
        }
    }
    if let Ok(model) = std::env::var("MINIMAX_MODEL") {
        if !model.trim().is_empty() {
            cfg.model = model.trim().to_string();
        }
    }
    cfg
}

/// Speichert die API-Konfiguration.
pub fn save_config(cfg: &ApiConfig) -> Result<()> {
    let path = config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(cfg)?)?;
    Ok(())
}

/// Traegt ein OAuth-Token der lokalen Desktop-App nach (Entwicklung).
pub fn local_oauth_token() -> Option<String> {
    let home = std::env::var_os("HOME")?;
    let path = PathBuf::from(home)
        .join(".minimax")
        .join("auth")
        .join("prod")
        .join("en")
        .join("mcode-public")
        .join("auth.json");
    let text = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&text).ok()?;
    let records = v.get("records")?.as_object()?;
    for (_, rec) in records {
        if let Some(token) = rec.get("accessToken").and_then(|t| t.as_str()) {
            return Some(token.to_string());
        }
    }
    None
}

// ---------------------------------------------------------------------------
// Antworten der Bildanalyse
// ---------------------------------------------------------------------------

/// Schaetzung der Rohlinggeometrie aus einem Foto.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct BladeAnalysis {
    pub blade_length_mm: Option<f64>,
    pub blade_width_mm: Option<f64>,
    pub blade_thickness_mm: Option<f64>,
    pub tang_length_mm: Option<f64>,
    pub tang_width_mm: Option<f64>,
    pub tang_thickness_mm: Option<f64>,
    pub tang_style: Option<String>,
    pub tang_taper: Option<bool>,
    pub has_bolster: Option<bool>,
    pub has_guard: Option<bool>,
    pub spine_offset_mm: Option<f64>,
    pub edge_offset_mm: Option<f64>,

    /// Verhaeltnisse - aus einem Foto deutlich verlaesslicher als Absolutwerte.
    /// tang_width_ratio   = Angelbreite / Klingenueberhoehe am Heft
    /// tang_thickness_ratio = Angeldicke / Klingendicke
    /// tang_length_ratio  = Angel-Laenge / Klingenlaenge
    /// tang_center_ratio  = Lage der Angelmitte zwischen Ruecken (0) und Schneide (1)
    pub tang_width_ratio: Option<f64>,
    pub tang_thickness_ratio: Option<f64>,
    pub tang_length_ratio: Option<f64>,
    pub tang_center_ratio: Option<f64>,

    pub confidence: Option<f64>,
    pub visible: Option<String>,
    pub notes: Option<String>,
}

impl BladeAnalysis {
    /// Uebernimmt die Schaetzung in den Rohling, ohne vorhandene Werte zu ueberschreiben.
    ///
    /// Fehlen absolute Masse, werden sie aus den Verhaeltnissen abgeleitet -
    /// so bleibt der Plan auch dann nutzbar, wenn nur ein Foto ohne Massstab
    /// vorliegt und der Nutzer nur die Klingenlaenge kennt.
    pub fn apply(&mut self, blank: &mut BladeBlank) {
        // Verhaeltnisse zuerst, damit sie als Reserve fuer fehlende
        // Absolutwerte zur Verfuegung stehen.
        let mut derived_tang_width = self.tang_width_mm;
        let mut derived_tang_thickness = self.tang_thickness_mm;
        let mut derived_tang_length = self.tang_length_mm;

        if derived_tang_width.is_none() {
            if let (Some(r), Some(bw)) = (self.tang_width_ratio, self.blade_width_mm) {
                derived_tang_width = Some(r * bw);
            }
        }
        if derived_tang_thickness.is_none() {
            if let (Some(r), Some(bt)) = (self.tang_thickness_ratio, self.blade_thickness_mm) {
                derived_tang_thickness = Some(r * bt);
            }
        }
        if derived_tang_length.is_none() {
            if let (Some(r), Some(bl)) = (self.tang_length_ratio, self.blade_length_mm) {
                derived_tang_length = Some(r * bl);
            }
        }

        if let Some(v) = self.blade_length_mm {
            blank.blade_length_mm = v;
        }
        if let Some(v) = self.blade_width_mm {
            blank.blade_width_mm = v;
        }
        if let Some(v) = self.blade_thickness_mm {
            blank.blade_thickness_mm = v;
        }
        if let Some(v) = derived_tang_length {
            blank.tang_length_mm = v;
        }
        if let Some(v) = derived_tang_width {
            blank.tang_width_mm = v;
        }
        if let Some(v) = derived_tang_thickness {
            blank.tang_thickness_mm = v;
        }
        if let Some(s) = &self.tang_style {
            blank.tang_style = match s.to_lowercase().as_str() {
                "full" | "full tang" | "durchgehend" | "durchgehende angel" => TangStyle::Full,
                "partial" | "partial tang" | "teildeckend" | "teildeckende angel" => TangStyle::Partial,
                "hidden" | "hidden tang" | "verdeckt" | "verdeckte angel" => TangStyle::Hidden,
                _ => blank.tang_style,
            };
        }
        if let Some(v) = self.tang_taper {
            blank.tang_taper = v;
        }
        if let Some(v) = self.has_bolster {
            blank.has_bolster = v;
        }
        if let Some(v) = self.has_guard {
            blank.has_guard = v;
        }
        if let Some(v) = self.spine_offset_mm {
            blank.spine_offset_mm = v;
        } else if let (Some(r), Some(bw)) = (self.tang_center_ratio, self.blade_width_mm) {
            // Lage der Angelmitte zwischen Ruecken und Schneide
            blank.spine_offset_mm = (r * bw).max(1.0);
            blank.edge_offset_mm = ((1.0 - r) * bw).max(1.0);
        }
        if let Some(v) = self.edge_offset_mm {
            blank.edge_offset_mm = v;
        }
        if let Some(n) = &self.notes {
            if !n.trim().is_empty() {
                blank.notes = n.trim().to_string();
            }
        }
    }
}

/// Analyse eines Griffmaterials.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct MaterialAnalysis {
    pub category: Option<String>,
    pub species_guess: Option<String>,
    pub color: Option<String>,
    pub figure: Option<String>,
    pub grain_direction: Option<String>,
    pub defects: Option<Vec<String>>,
    pub porosity: Option<f64>,
    pub hardness_guess: Option<String>,
    pub suggested_matches: Option<Vec<String>>,
    pub notes: Option<String>,
}

// ---------------------------------------------------------------------------
// Client
// ---------------------------------------------------------------------------

pub struct VisionClient {
    pub cfg: ApiConfig,
    pub bearer: Option<String>,
    /// Das Denken des Modells aus dem letzten Aufruf (fuer das Protokoll).
    pub last_thinking: String,
}

impl VisionClient {
    pub fn new(cfg: ApiConfig) -> VisionClient {
        let bearer = if cfg.is_configured() {
            None
        } else {
            local_oauth_token()
        };
        VisionClient {
            cfg,
            bearer,
            last_thinking: String::new(),
        }
    }

    pub fn is_ready(&self) -> bool {
        self.cfg.is_configured() || self.bearer.is_some()
    }

    /// Kuerzt und konvertiert ein Bild auf max. `max_edge` Pixel, JPEG-Qualitaet.
    fn encode_image(&self, path: &Path, max_edge: u32) -> Result<(String, String)> {
        let img = image::open(path).with_context(|| format!("Bild nicht lesbar: {}", path.display()))?;
        let img = if img.width() > max_edge || img.height() > max_edge {
            img.thumbnail(max_edge, max_edge)
        } else {
            img
        };
        let rgb = img.to_rgb8();
        let _ = &rgb;
        let mut buf = Vec::new();
        let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, 85);
        encoder
            .encode_image(&img)
            .context("Bild konnte nicht komprimiert werden")?;
        Ok((
            base64::engine::general_purpose::STANDARD.encode(&buf),
            "image/jpeg".to_string(),
        ))
    }

    fn request(&mut self, prompt: &str, images: &[(String, String)]) -> Result<String> {
        if !self.is_ready() {
            return Err(anyhow!(
                "Kein API-Schluessel hinterlegt. In den Einstellungen einen MiniMax-\
                 API-Key eintragen (Umgebungsvariable MINIMAX_API_KEY oder Konfigurationsdatei)."
            ));
        }

        let mut content: Vec<serde_json::Value> = Vec::new();
        for (data, media_type) in images {
            content.push(serde_json::json!({
                "type": "image",
                "source": { "type": "base64", "media_type": media_type, "data": data }
            }));
        }
        content.push(serde_json::json!({ "type": "text", "text": prompt }));

        // Reasoning-Modelle wie M3.1-Flash denken vor der Antwort und
        // brauchen dafuer viel Platz - mit zu kleinem Limit kommt keine
        // Antwort, sondern nur Denkbloecke zurueck. Gemessen: M3.1-Flash
        // braucht bei einem Foto rund 13.000 Token fuer das Denken.
        let max_tokens = if self.cfg.is_reasoning() { 20_000 } else { 4_000 };
        let body = serde_json::json!({
            "model": self.cfg.model(),
            "max_tokens": max_tokens,
            "messages": [{ "role": "user", "content": content }]
        });

        let client = reqwest::blocking::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()?;

        let mut req = client
            .post(format!("{}/messages", self.cfg.base_url()))
            .header("Content-Type", "application/json")
            .header("anthropic-version", "2023-06-01")
            .json(&body);

        if let Some(token) = &self.bearer {
            req = req.header("Authorization", format!("Bearer {token}"));
        } else {
            req = req.header("x-api-key", self.cfg.api_key.trim());
        }

        let resp = req
            .send()
            .context("Verbindung zur MiniMax-API fehlgeschlagen")?;
        let status = resp.status();
        let text = resp.text().unwrap_or_default();
        if !status.is_success() {
            return Err(anyhow!("API-Fehler {status}: {}", truncate(&text, 400)));
        }
        let v: serde_json::Value =
            serde_json::from_str(&text).context("Antwort der API war kein gueltiges JSON")?;
        // Reasoning-Modelle liefern Denkbloecke und erst danach den Text.
        let mut out = String::new();
        let mut thinking = String::new();
        if let Some(blocks) = v.get("content").and_then(|c| c.as_array()) {
            for b in blocks {
                match b.get("type").and_then(|t| t.as_str()) {
                    Some("text") => {
                        if let Some(t) = b.get("text").and_then(|t| t.as_str()) {
                            out.push_str(t);
                        }
                    }
                    Some("thinking") => {
                        if let Some(t) = b.get("thinking").and_then(|t| t.as_str()) {
                            thinking.push_str(t);
                            thinking.push('\n');
                        }
                    }
                    _ => {}
                }
            }
        }
        if out.trim().is_empty() {
            if !thinking.trim().is_empty() {
                return Err(anyhow!(
                    "Das Modell {} lieferte nur Denkbloecke, aber keine Antwort. \
                     Die Antwort war vermutlich zu knapp bemessen - bitte ein anderes \
                     Modell waehlen oder ein aussagekraeftigeres Foto verwenden.",
                    self.cfg.model()
                ));
            }
            return Err(anyhow!(
                "Die API hat keinen Text geliefert. Modell: {}",
                self.cfg.model()
            ));
        }
        self.last_thinking = thinking;
        Ok(out)
    }

    /// Analysiert ein Foto des Klingenrohlinges.
    pub fn analyze_blade(&mut self, path: &Path) -> Result<(BladeAnalysis, String)> {
        let prompt = r#"Du bist ein erfahrener Messerbauer. Analysiere das Foto eines Klingenrohlinges.

Gib ausschliesslich ein JSON-Objekt zurueck, ohne Text davor oder danach:
{
  "blade_length_mm": Zahl | null,
  "blade_width_mm": Zahl | null,
  "blade_thickness_mm": Zahl | null,
  "tang_length_mm": Zahl | null,
  "tang_width_mm": Zahl | null,
  "tang_thickness_mm": Zahl | null,
  "tang_style": "full" | "partial" | "hidden" | null,
  "tang_taper": true | false | null,
  "has_bolster": true | false | null,
  "has_guard": true | false | null,
  "spine_offset_mm": Zahl | null,
  "edge_offset_mm": Zahl | null,
  "tang_width_ratio": Zahl | null,
  "tang_thickness_ratio": Zahl | null,
  "tang_length_ratio": Zahl | null,
  "tang_center_ratio": Zahl | null,
  "confidence": Zahl zwischen 0 und 1,
  "visible": "was auf dem Foto klar erkennbar war",
  "notes": "kurze fachliche Einschaetzung, was unsicher ist und wie man nachmessen sollte"
}

WICHTIG - Masse und Verhaeltnisse:
- Absolute Millimeter-Werte sind aus einem Foto ohne Referenz NICHT bestimmbar.
  Setze die absoluten Felder auf null, wenn kein Lineal, keine Kreditkarte oder
  sonst ein bekanntes Referenzmass neben dem Rohling liegt.
- Die Verhaeltnisfelder sind dann die verlaessliche Angabe und IMMER auszufuellen:
    tang_width_ratio     = Breite der Angel / Hoehe der Klinge am Heft
    tang_thickness_ratio = Dicke der Angel / Dicke der Klinge
    tang_length_ratio    = Laenge der Angel hinter dem Heft / Laenge der Klinge
    tang_center_ratio    = Lage der Angelmitte zwischen Ruecken (0.0) und Schneide (1.0)
- Ist eine Referenz sichtbar, dann zusaetzlich die absoluten Werte in mm schaetzen.
- tang_length_mm ist die Laenge der Angel HINTER dem Heft (das Stueck, das in den Griff geht).
- tang_style: "full" = durchgehend bis zum Griffende, "partial" = nur teilweise,
  "hidden" = verdeckt im Griff.
- Ist etwas nicht erkennbar, setze null. Rate keine Werte, die du nicht begruenden kannst.
- confidence bezieht sich auf die Gesamtschaetzung."#;

        let (data, media) = self.encode_image(path, 1600)?;
        let raw = self.request(prompt, &[(data, media)])?;
        let value = extract_json(&raw).ok_or_else(|| {
            anyhow!("Die Antwort enthielt kein JSON. Rohtext: {}", truncate(&raw, 300))
        })?;
        let analysis: BladeAnalysis = serde_json::from_value(value)
            .map_err(|e| anyhow!("Antwort nicht lesbar: {e}"))?;
        Ok((analysis, raw))
    }

    /// Analysiert ein Foto des Griffmaterials.
    pub fn analyze_material(&mut self, path: &Path) -> Result<(MaterialAnalysis, String)> {
        let prompt = r#"Du bist ein erfahrener Messerbauer und Holzkenner. Analysiere das Foto eines potenziellen Griffmaterials.

Gib ausschliesslich ein JSON-Objekt zurueck, ohne Text davor oder danach:
{
  "category": "Holz" | "Horn" | "Knochen" | "Metall" | "Komposit" | "unbekannt",
  "species_guess": "moeglichste Holzart auf Deutsch, z.B. Walnuss, Eiche, Ahorn, Ebenholz, Palisander, Thuja",
  "color": "Beschreibung der Grundfarbe",
  "figure": "Maserung, Zeichnung, Kontrast",
  "grain_direction": "Masernrichtung im Bild und wie sie im Griff laufen sollte",
  "defects": ["Risse", "Lochbildung", "Aeste", ...],
  "porosity": Zahl zwischen 0 und 1,
  "hardness_guess": "weich" | "mittel" | "hart" | "sehr hart",
  "suggested_matches": ["3-5 passende Keys aus dieser Liste: walnuss, eiche, esche, ahorn, kirsche, birke, ebenholz, palisander, wenge, olivenholz, buchsbaum, zebrawood, thuja, stabilisiert, composite, horn, bone, geweih, messing, kupfer, edelstahl, titan, aluminium"],
  "notes": "kurze fachliche Einschaetzung: Eignung als Griffmaterial, Risiken, Pflege"
}

Regeln:
- Beurteile anhand von Farbe, Maserung, Poren und Oberflaeche.
- Ist es eindeutig Holz, nenne die wahrscheinlichste Art, aber bleibe ehrlich bei Unsicherheit.
- suggested_matches enthaelt nur Keys aus der angegebenen Liste."#;

        let (data, media) = self.encode_image(path, 1600)?;
        let raw = self.request(prompt, &[(data, media)])?;
        let value = extract_json(&raw).ok_or_else(|| {
            anyhow!("Die Antwort enthielt kein JSON. Rohtext: {}", truncate(&raw, 300))
        })?;
        let analysis: MaterialAnalysis = serde_json::from_value(value)
            .map_err(|e| anyhow!("Antwort nicht lesbar: {e}"))?;
        Ok((analysis, raw))
    }
}

/// Zieht das erste JSON-Objekt aus einer Modellantwort.
pub fn extract_json(raw: &str) -> Option<serde_json::Value> {
    let trimmed = raw.trim();
    if let Ok(v) = serde_json::from_str::<serde_json::Value>(trimmed) {
        return Some(v);
    }
    // Erstes {...} finden, das balanciert ist
    let bytes: Vec<char> = trimmed.chars().collect();
    let start = bytes.iter().position(|&c| c == '{')?;
    let mut depth = 0i32;
    let mut in_str = false;
    let mut escape = false;
    for i in start..bytes.len() {
        let c = bytes[i];
        if in_str {
            if escape {
                escape = false;
            } else if c == '\\' {
                escape = true;
            } else if c == '"' {
                in_str = false;
            }
            continue;
        }
        match c {
            '"' => in_str = true,
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    let candidate: String = bytes[start..=i].iter().collect();
                    return serde_json::from_str(&candidate).ok();
                }
            }
            _ => {}
        }
    }
    None
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        let head: String = s.chars().take(n).collect();
        format!("{head}...")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BladeBlank;

    #[test]
    fn extracts_json_from_prose() {
        let raw = "Hier das Ergebnis:\n```json\n{\"a\": 1, \"b\": [2,3]}\n```\nEnde.";
        let v = extract_json(raw).expect("JSON gefunden");
        assert_eq!(v.get("a").and_then(|x| x.as_i64()), Some(1));
    }

    #[test]
    fn extracts_json_with_braces_in_string() {
        let raw = r#"text {"a": "ein { } zeichen", "b": 2} rest"#;
        let v = extract_json(raw).expect("JSON gefunden");
        assert_eq!(v.get("b").and_then(|x| x.as_i64()), Some(2));
    }

    #[test]
    fn ratios_fill_in_missing_absolute_values() {
        let mut analysis = BladeAnalysis {
            blade_length_mm: Some(200.0),
            blade_width_mm: Some(30.0),
            blade_thickness_mm: Some(3.0),
            tang_width_ratio: Some(0.2),
            tang_thickness_ratio: Some(0.7),
            tang_length_ratio: Some(0.45),
            tang_center_ratio: Some(0.4),
            ..Default::default()
        };
        let mut blank = BladeBlank::default();
        analysis.apply(&mut blank);

        assert!((blank.tang_length_mm - 90.0).abs() < 1e-6, "{}", blank.tang_length_mm);
        assert!((blank.tang_width_mm - 6.0).abs() < 1e-6, "{}", blank.tang_width_mm);
        assert!((blank.tang_thickness_mm - 2.1).abs() < 1e-6, "{}", blank.tang_thickness_mm);
        // Angelmitte bei 40 % der Klingenueberhoehe
        assert!((blank.spine_offset_mm - 12.0).abs() < 1e-6, "{}", blank.spine_offset_mm);
        assert!((blank.edge_offset_mm - 18.0).abs() < 1e-6, "{}", blank.edge_offset_mm);
    }

    #[test]
    fn absolute_values_win_over_ratios() {
        let mut analysis = BladeAnalysis {
            blade_width_mm: Some(30.0),
            tang_width_mm: Some(7.0),
            tang_width_ratio: Some(0.2),
            ..Default::default()
        };
        let mut blank = BladeBlank::default();
        analysis.apply(&mut blank);
        assert!((blank.tang_width_mm - 7.0).abs() < 1e-6);
    }

    #[test]
    fn tang_style_is_mapped() {
        let mut analysis = BladeAnalysis {
            tang_style: Some("partial tang".into()),
            ..Default::default()
        };
        let mut blank = BladeBlank::default();
        analysis.apply(&mut blank);
        assert_eq!(blank.tang_style, TangStyle::Partial);
    }
}
