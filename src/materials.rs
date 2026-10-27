//! Material-Datenbank und Materialbewertung.
//!
//! Groessen in mm, Dichte in g/cm^3, Janka in lbf.

use crate::model::{BladeBlank, HandleGeometry, HandleParams, HandleStyle};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaterialCategory {
    Wood,
    Horn,
    Bone,
    Antler,
    Metal,
    Composite,
}

impl MaterialCategory {
    pub fn label(&self) -> &'static str {
        match self {
            MaterialCategory::Wood => "Holz",
            MaterialCategory::Horn => "Horn",
            MaterialCategory::Bone => "Knochen",
            MaterialCategory::Antler => "Geweih",
            MaterialCategory::Metal => "Metall",
            MaterialCategory::Composite => "Komposit",
        }
    }
    pub fn all() -> [MaterialCategory; 6] {
        [
            MaterialCategory::Wood,
            MaterialCategory::Horn,
            MaterialCategory::Bone,
            MaterialCategory::Antler,
            MaterialCategory::Metal,
            MaterialCategory::Composite,
        ]
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Material {
    pub key: &'static str,
    pub name_de: &'static str,
    pub category: MaterialCategory,
    pub density: f64,
    pub janka: Option<f64>,
    pub hardness_class: &'static str,
    pub grain: &'static str,
    /// 0 = nimmt kein Oel an, 1 = saugt stark
    pub oil_absorption: f64,
    /// 1 = sehr formstabil
    pub stability: f64,
    /// 1 = neigt zu Rissen
    pub crack_risk: f64,
    /// 1 = sehr gut bearbeitbar
    pub workability: f64,
    pub figure: &'static str,
    pub adhesives: &'static [&'static str],
    pub finishes: &'static [&'static str],
    pub fastener: &'static str,
    pub notes: &'static str,
    /// 1 = guenstig ... 5 = Luxus
    pub price_class: u8,
    pub good_for: &'static [&'static str],
}

impl Material {
    pub fn is_organic(&self) -> bool {
        matches!(
            self.category,
            MaterialCategory::Wood | MaterialCategory::Horn | MaterialCategory::Bone | MaterialCategory::Antler
        )
    }
    /// Horn/Knochen brauchen einen Spalt, sonst bricht die Klebefuge.
    pub fn needs_epoxy_gap(&self) -> bool {
        matches!(
            self.category,
            MaterialCategory::Horn | MaterialCategory::Bone | MaterialCategory::Antler
        )
    }
    /// Eignet sich als zweigeteilte Griffschale?
    pub fn scales_well(&self) -> bool {
        match self.category {
            MaterialCategory::Horn | MaterialCategory::Bone | MaterialCategory::Antler | MaterialCategory::Metal => false,
            MaterialCategory::Wood => self.crack_risk < 0.85 && self.workability > 0.2,
            MaterialCategory::Composite => true,
        }
    }
    pub fn label(&self) -> String {
        format!("{} ({})", self.name_de, self.category.label())
    }
}

macro_rules! mat {
    (
        key: $key:expr, name: $name:expr, cat: $cat:expr,
        density: $density:expr, janka: $janka:expr, hard: $hard:expr,
        grain: $grain:expr, oil: $oil:expr, stab: $stab:expr,
        crack: $crack:expr, work: $work:expr, figure: $figure:expr,
        adh: $adh:expr, fin: $fin:expr, fast: $fast:expr,
        notes: $notes:expr, price: $price:expr, good: $good:expr
    ) => {
        Material {
            key: $key,
            name_de: $name,
            category: $cat,
            density: $density,
            janka: $janka,
            hardness_class: $hard,
            grain: $grain,
            oil_absorption: $oil,
            stability: $stab,
            crack_risk: $crack,
            workability: $work,
            figure: $figure,
            adhesives: $adh,
            finishes: $fin,
            fastener: $fast,
            notes: $notes,
            price_class: $price,
            good_for: $good,
        }
    };
}

use MaterialCategory as C;

pub static MATERIALS: &[Material] = &[
    mat! {
        key: "walnuss", name: "Nussbaum (Walnuss)", cat: C::Wood,
        density: 0.66, janka: Some(1010.0), hard: "mittel",
        grain: "geradelfasrig, feine Poren", oil: 0.55, stab: 0.75,
        crack: 0.25, work: 0.85,
        figure: "Warme braune Grundfarbe, oft roetlich bis violett, unregelmaessige Maserung",
        adh: &["Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"],
        fin: &["Hartwachs-Oel", "Leinoel + Bienenwachs", "Harte Oel"],
        fast: "Messing-Stift",
        notes: "Der Klassiker. Feine Poren, nimmt Oel gut auf, dunkelt mit der Zeit nach. Spannungsrisiko bei duennen Schalen nur gering.",
        price: 2, good: &["full_tang", "partial_tang"]
    },
    mat! {
        key: "eiche", name: "Eiche", cat: C::Wood,
        density: 0.75, janka: Some(1360.0), hard: "hart",
        grain: "porig, starke Fruehholzporen", oil: 0.8, stab: 0.9,
        crack: 0.2, work: 0.7,
        figure: "Deutliche Ringporen, markante Zeichnung, fleckige Durchzeichnung",
        adh: &["Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"],
        fin: &["Hartwachs-Oel", "Laugenbeize + Oel", "Brennlack"],
        fast: "Messing-Stift",
        notes: "Extrem formstabil, porig. Saugt Oel stark -> zweimalige Oelung noetig. Fuer kraeftige Optik ideal, die grossen Poren bleiben sichtbar.",
        price: 1, good: &["full_tang", "one_piece", "ferrule"]
    },
    mat! {
        key: "esche", name: "Esche", cat: C::Wood,
        density: 0.69, janka: Some(1320.0), hard: "hart",
        grain: "geradelfasrig, ringporig", oil: 0.55, stab: 0.7,
        crack: 0.3, work: 0.8,
        figure: "Helle Grundfarbe mit kraeftiger Streifenzeichnung, elastig",
        adh: &["Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"],
        fin: &["Hartwachs-Oel", "Beize + Oel"],
        fast: "Messing-Stift",
        notes: "Etwas elastig -> leichtes Durchbiegen im Griff, bei Full Tang sogar erwuenscht (Daempfung). Guter Allrounder.",
        price: 1, good: &["full_tang"]
    },
    mat! {
        key: "ahorn", name: "Ahorn (Hartholz)", cat: C::Wood,
        density: 0.72, janka: Some(1450.0), hard: "hart",
        grain: "fein, gleichmaessig, geschlossen", oil: 0.4, stab: 0.8,
        crack: 0.25, work: 0.8,
        figure: "Cremeweiss bis hellgrau, feine Quaste oder Augen-Maserung",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel", "Beize + Oel"],
        fast: "Messing-Stift",
        notes: "Sehr hart und dimensionstreu, nimmt Oel wenig an -> Schalen bleiben hell. Spannungsgarnitur empfohlen.",
        price: 2, good: &["full_tang", "partial_tang"]
    },
    mat! {
        key: "kirsche", name: "Kirschbaum", cat: C::Wood,
        density: 0.58, janka: Some(950.0), hard: "mittel",
        grain: "fein, dicht", oil: 0.45, stab: 0.6,
        crack: 0.4, work: 0.9,
        figure: "Warmes Honigbraun, feine Flammen- und Flickzeichnung",
        adh: &["Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"],
        fin: &["Hartwachs-Oel"],
        fast: "Messing-Stift",
        notes: "Sehr gut zu bearbeiten, dunkelt mit der Zeit kraeftig nach. Weicher -> groessere Wandstaerke und grosszuegige Bohrungen einplanen.",
        price: 2, good: &["full_tang", "one_piece"]
    },
    mat! {
        key: "birke", name: "Birke", cat: C::Wood,
        density: 0.60, janka: Some(980.0), hard: "mittel",
        grain: "fein, verschlossen", oil: 0.5, stab: 0.6,
        crack: 0.35, work: 0.85,
        figure: "Cremeweiss mit feiner Laengszeichnung, Birkenruebe",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel"],
        fast: "Messing-Stift",
        notes: "Guenstig, gut zu drechseln, haelt Stifte gut. Wird schnell fleckig.",
        price: 1, good: &["full_tang", "one_piece"]
    },
    mat! {
        key: "ebenholz", name: "Ebenholz", cat: C::Wood,
        density: 1.15, janka: Some(3220.0), hard: "sehr hart",
        grain: "sehr fein, dicht, geschlossen", oil: 0.15, stab: 0.9,
        crack: 0.35, work: 0.4,
        figure: "Tiefschwarz bis dunkelbraun, feine helle Linien bei bester Qualitaet",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur", "Hartwachs-Oel (wenig)"],
        fast: "Messing-Stift",
        notes: "Sehr hart und schwer, nimmt kaum Oel auf -> dunkel glaenzend polieren. Sproed bei Biegebelastung: Bohrlocher grosszuegig bohren, Faserrichtung beachten.",
        price: 5, good: &["full_tang"]
    },
    mat! {
        key: "palisander", name: "Palisander (Rosewood)", cat: C::Wood,
        density: 0.90, janka: Some(2200.0), hard: "sehr hart",
        grain: "geradelfasrig, feinporig", oil: 0.35, stab: 0.9,
        crack: 0.3, work: 0.55,
        figure: "Tiefbraun bis violett-schwarz, dunkle Adern, hoher Kontrast",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel", "Politur"],
        fast: "Messing-Stift",
        notes: "Olig, daher nur wenig Leimverlust und hohe Klebestigkeit. Sehr formstabil, dunkelt kaum nach. Breite Adern als Kontrastelement nutzbar.",
        price: 5, good: &["full_tang", "partial_tang", "one_piece"]
    },
    mat! {
        key: "wenge", name: "Wenge", cat: C::Wood,
        density: 0.87, janka: Some(1630.0), hard: "hart",
        grain: "geradelfasrig, sehr dunkel", oil: 0.3, stab: 0.85,
        crack: 0.45, work: 0.5,
        figure: "Tiefschwarz mit feinen helleren Faserlinien",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel", "Politur"],
        fast: "Messing-Stift",
        notes: "Sproede, daher nur fuer erfahrene Macher. Rissgefahr bei Zu- und Abkuehlung in der Werkstatt.",
        price: 4, good: &["full_tang"]
    },
    mat! {
        key: "olivenholz", name: "Olivenholz", cat: C::Wood,
        density: 0.92, janka: Some(1900.0), hard: "sehr hart",
        grain: "unregelmaessig, wild wachsend", oil: 0.45, stab: 0.8,
        crack: 0.5, work: 0.45,
        figure: "Stark kontrastierend, hell-gelb und dunkel-braun gestreift",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel", "Oel + Bienenwachs"],
        fast: "Messing-Stift",
        notes: "Unregelmaessige Struktur macht die Bohrungstoleranzen unberechenbar - Loecher immer mit Gegenprobe bohren. Sehr dekorativ.",
        price: 4, good: &["full_tang", "one_piece"]
    },
    mat! {
        key: "buchsbaum", name: "Buchsbaum", cat: C::Wood,
        density: 0.85, janka: Some(1800.0), hard: "sehr hart",
        grain: "fein, homogen, geschlossen", oil: 0.35, stab: 0.8,
        crack: 0.3, work: 0.65,
        figure: "Creme bis honigfarben, feine roetliche Adern",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel", "Politur"],
        fast: "Messing-Stift",
        notes: "Feinste Zeichnung, sehr teuer. Klassiker fuer edle Full-Tang-Messer.",
        price: 5, good: &["full_tang"]
    },
    mat! {
        key: "zebrawood", name: "Zebraholz", cat: C::Wood,
        density: 0.78, janka: Some(1570.0), hard: "hart",
        grain: "stark streifig", oil: 0.5, stab: 0.7,
        crack: 0.4, work: 0.6,
        figure: "Hochkontrastige Zebra-Streifen, hell-braun und dunkel-braun",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Hartwachs-Oel"],
        fast: "Messing-Stift",
        notes: "Maserung ist extrem auffaellig - Schalen spiegelbildlich ausrichten, sonst wirkt der Griff unruhig.",
        price: 4, good: &["full_tang"]
    },
    mat! {
        key: "thuja", name: "Thuja / Laerche (Knochenholz)", cat: C::Wood,
        density: 0.39, janka: Some(400.0), hard: "weich",
        grain: "sehr weich, langfaserig", oil: 0.7, stab: 0.6,
        crack: 0.35, work: 0.95,
        figure: "Cremeweiss mit kraeftiger, dunkler Laengsaderung",
        adh: &["Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"],
        fin: &["Hartwachs-Oel", "Oel + Bienenwachs"],
        fast: "Messing-Stift",
        notes: "Extrem leicht, sehr elastisch, ideal fuer verdeckten Tang und Faltmesser. Grosse Bohrungen noetig, Stifte nicht zu fest.",
        price: 2, good: &["hidden_tang", "one_piece", "ferrule"]
    },
    mat! {
        key: "stabilisiert", name: "Stabilisierter Wurzelholz", cat: C::Composite,
        density: 0.75, janka: Some(1200.0), hard: "mittel",
        grain: "durch Harz fixiert, Maserung bleibt sichtbar", oil: 0.05,
        stab: 1.0, crack: 0.05, work: 0.6,
        figure: "Kontrastreiche Wurzelmaserung, meist hell mit dunklen Adern",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur", "Hartwachs-Oel"],
        fast: "Messing-Stift",
        notes: "Harzgefuellt: kein Kleben noetig (oder nur minimal), nicht spannungsempfindlich. Ideal fuer Konstruktionen mit eingearbeitetem Material.",
        price: 4, good: &["full_tang", "one_piece", "hidden_tang"]
    },
    mat! {
        key: "composite", name: "Glasfaser-Komposit (CFK/FRP)", cat: C::Composite,
        density: 1.55, janka: None, hard: "sehr hart",
        grain: "Faserlaminat, Schichtaufbau", oil: 0.0,
        stab: 1.0, crack: 0.05, work: 0.5,
        figure: "Industriell, sichtbare Schichtkante",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur"],
        fast: "Messing-Stift",
        notes: "Sehr leicht und steif. Schichtkante sichtbar lassen oder ueberfraesen. Bohrstaub ist scharfkantig - Atemschutz.",
        price: 3, good: &["one_piece"]
    },
    mat! {
        key: "horn", name: "Horn (Bueffelhorn)", cat: C::Horn,
        density: 1.45, janka: Some(2000.0), hard: "sehr hart",
        grain: "schichtig, konzentrisch", oil: 0.25, stab: 0.7,
        crack: 0.6, work: 0.35,
        figure: "Tiefbraun bis schwarz mit Transparenz, oft helle Knochen-Einschluesse",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur", "Hartwachs-Oel"],
        fast: "Messing-Stift",
        notes: "Klassischer Full-Tang-Griff mit Angel-Einlage. Braucht einen Spalt von 0,03-0,05 mm, sonst bricht die Leimfuge. Nur fein schleifen, nicht viel Waerme - es reisst sonst aus.",
        price: 4, good: &["full_tang"]
    },
    mat! {
        key: "bone", name: "Knochen (Rinder-/Hirschknochen)", cat: C::Bone,
        density: 1.70, janka: None, hard: "hart",
        grain: "kompakt, feinporig", oil: 0.1, stab: 0.8,
        crack: 0.5, work: 0.5,
        figure: "Elfenbeinfarben, kann eingefaerbt werden",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur", "Faerbung + Oel"],
        fast: "Messing-Stift",
        notes: "Feiner als Horn, gut zu faerben. Muss trocken sein, sonst braucht die Leimfuge lange zum Abbinden. Stabile Alternative zu Horn.",
        price: 2, good: &["full_tang"]
    },
    mat! {
        key: "geweih", name: "Geweih (Reh/Hirsch)", cat: C::Antler,
        density: 1.55, janka: None, hard: "hart",
        grain: "poroes, Jahresringe sichtbar", oil: 0.3, stab: 0.75,
        crack: 0.45, work: 0.55,
        figure: "Cremefarben mit dunklen Jahresringen",
        adh: &["Zwei-Komponenten-Epoxy"],
        fin: &["Politur", "Faerbung + Oel"],
        fast: "Messing-Stift",
        notes: "Guter Fuell- und Abstandshalter zwischen Bastion und Schale. Trocknet langsam - Leim lange halten.",
        price: 2, good: &["full_tang", "partial_tang"]
    },
    mat! {
        key: "messing", name: "Messing", cat: C::Metal,
        density: 8.50, janka: None, hard: "hart",
        grain: "metallisch, kristallin", oil: 0.0, stab: 1.0,
        crack: 0.0, work: 0.8,
        figure: "Warmes Goldgelb, mit der Zeit Patina",
        adh: &["Kunstharzleim PUR"],
        fin: &["Politur", "Patina", "Buersten"],
        fast: "Messing-Stift",
        notes: "Standard fuer Bastionen, Zwingen und Stifte: weich genug, um die Schale nicht zu sprengen, aber fest genug. Bastion anschliessend an den Rohling anschweissen.",
        price: 2, good: &["partial_tang", "ferrule", "full_tang"]
    },
    mat! {
        key: "kupfer", name: "Kupfer", cat: C::Metal,
        density: 8.96, janka: None, hard: "hart",
        grain: "metallisch, weich", oil: 0.0, stab: 1.0,
        crack: 0.0, work: 0.85,
        figure: "Roetlich-orange, dunkelt zur Patina",
        adh: &["Epoxy mit Sekundenhaft"],
        fin: &["Politur", "Patina"],
        fast: "Kupfer-Stift",
        notes: "Noch weicher als Messing, klassische Wahl fuer Zwingen an Folienmessern. Nicht mit Stahl verkleben - differentielle Ausdehnung.",
        price: 3, good: &["ferrule"]
    },
    mat! {
        key: "edelstahl", name: "Edelstahl (1.4301 / 1.4404)", cat: C::Metal,
        density: 7.90, janka: None, hard: "sehr hart",
        grain: "metallisch", oil: 0.0, stab: 1.0,
        crack: 0.0, work: 0.6,
        figure: "Neutral, matt oder poliert",
        adh: &[],
        fin: &["Politur", "Buersten", "Sandstrahlen"],
        fast: "Edelstahl-Stift",
        notes: "Rostfrei, aber hart - als Bastion sprengt es Schalen aus weichem Holz. Nur mit Presspassung oder Schraube befestigen, nicht verkleben.",
        price: 2, good: &["partial_tang", "one_piece"]
    },
    mat! {
        key: "titan", name: "Titan", cat: C::Metal,
        density: 4.50, janka: None, hard: "sehr hart",
        grain: "metallisch", oil: 0.0, stab: 1.0,
        crack: 0.0, work: 0.45,
        figure: "Grau, matt, anodisierbar in Farben",
        adh: &[],
        fin: &["Sandstrahlen", "Anodisieren", "Politur"],
        fast: "Titan-Stift",
        notes: "Leicht und korrosionsbestaendig. Teuer; als Bastion meist Overkill.",
        price: 5, good: &["partial_tang", "one_piece"]
    },
    mat! {
        key: "aluminium", name: "Aluminium", cat: C::Metal,
        density: 2.70, janka: None, hard: "weich",
        grain: "metallisch, weich", oil: 0.0, stab: 1.0,
        crack: 0.0, work: 0.9,
        figure: "Mattes Grau, eloxierbar",
        adh: &["Epoxy mit Sekundenhaft"],
        fin: &["Eloxieren", "Sandstrahlen"],
        fast: "Messing-Stift",
        notes: "Sehr weich - als Bastion federst, als Griffhuelse aber sehr leicht.",
        price: 2, good: &["ferrule"]
    },
];

pub fn get(key: &str) -> Option<&'static Material> {
    let k = key.trim().to_lowercase().replace([' ', '-'], "_");
    MATERIALS.iter().find(|m| m.key == k)
}

pub fn by_category(cat: MaterialCategory) -> Vec<&'static Material> {
    MATERIALS.iter().filter(|m| m.category == cat).collect()
}

/// Freitextsuche ueber Name, Kategorie und Merkmale.
pub fn search(text: &str) -> Vec<&'static Material> {
    let t = text.trim().to_lowercase();
    if t.is_empty() {
        return Vec::new();
    }
    let mut hits: Vec<(usize, &Material)> = MATERIALS
        .iter()
        .filter_map(|m| {
            let hay = format!(
                "{} {} {} {} {} {} {}",
                m.key, m.name_de, m.category.label(), m.hardness_class, m.grain, m.figure, m.notes
            )
            .to_lowercase();
            let count = hay.matches(&t).count();
            if count > 0 {
                Some((count, m))
            } else {
                None
            }
        })
        .collect();
    hits.sort_by(|a, b| b.0.cmp(&a.0));
    hits.into_iter().map(|(_, m)| m).collect()
}

/// Materialvorschlaege aus einer Vision-Beschreibung ableiten.
pub fn suggest_from_vision(class_hint: &str, species_hint: Option<&str>) -> Vec<&'static Material> {
    let mut out: Vec<&'static Material> = Vec::new();
    if let Some(s) = species_hint {
        out.extend(search(s));
    }
    out.extend(search(class_hint));
    let mut seen: Vec<&str> = Vec::new();
    out.retain(|m| {
        if seen.contains(&m.key) {
            false
        } else {
            seen.push(m.key);
            true
        }
    });
    out.truncate(8);
    out
}

// ---------------------------------------------------------------------------
// Bewertung
// ---------------------------------------------------------------------------

#[derive(Clone, Debug)]
pub struct MaterialVerdict {
    pub material_key: String,
    pub material_name: String,
    pub score: u8,
    pub rating: String,
    pub pros: Vec<String>,
    pub cons: Vec<String>,
    pub risks: Vec<String>,
    pub adhesive: String,
    pub fastener: String,
    pub finish: String,
    pub notes: String,
    pub tips: Vec<String>,
    /// Geschaetztes Griffgewicht in Gramm (wird vom Plan gefuellt).
    pub weight_g: Option<f64>,
    /// Bemerkung zur Waage des Messers.
    pub weight_note: Option<String>,
}

/// Bewertet ein Material fuer den konkreten Rohling und die gewaehlte Bauart.
pub fn evaluate(
    material: &Material,
    blank: &BladeBlank,
    params: &HandleParams,
    geo: &HandleGeometry,
) -> MaterialVerdict {
    let mut score: f64 = 62.0;
    let mut pros: Vec<String> = Vec::new();
    let mut cons: Vec<String> = Vec::new();
    let mut risks: Vec<String> = Vec::new();
    let mut tips: Vec<String> = Vec::new();

    // Passt die Materialart zur gewaehlten Griffbauart?
    match params.style {
        HandleStyle::FullTangTwoScale | HandleStyle::PartialTangBolster => {
            if material.scales_well() {
                score += 10.0;
                pros.push("Eignet sich als Griffschale mit durchgehender Angel.".into());
            } else {
                score -= 22.0;
                cons.push("Als zweigeteilte Griffschale ungeeignet - nur als Full-Tang-Einlage \
                          oder Zwinge einsetzbar."
                    .into());
            }
        }
        HandleStyle::OnePiece => {
            if material.is_organic() || material.category == MaterialCategory::Composite {
                score += 8.0;
                pros.push("Als Ein-Stueck-Griff mit eingelassener Angel gut machbar.".into());
            } else {
                score -= 20.0;
                cons.push("Metall eignet sich nicht als Ein-Stueck-Griff.".into());
            }
        }
        HandleStyle::Ferrule => {
            if material.is_organic() {
                score += 8.0;
                pros.push("Passt zur Bauart Holzgriff mit Zwinge.".into());
            } else {
                score -= 18.0;
                cons.push("Metall ist als Griffkoerper redundant.".into());
            }
        }
    }

    // Formstabilitaet
    if material.stability >= 0.85 {
        score += 10.0;
        pros.push("Sehr formstabil - spaetere Quellung und Verzug sind gering.".into());
    } else if material.stability < 0.6 {
        score -= 12.0;
        cons.push("Nur mittere Formstabilitaet - in der Werkstatt vor dem Einbau \
                  konditionieren."
            .into());
        risks.push("Klimaschwankungen in der Werkstatt koennen das Holz verziehen - \
                    vor der Bearbeitung several Tage bei Raumtemperatur lagern."
            .into());
    }

    // Rissgefahr
    if material.crack_risk >= 0.5 {
        score -= 10.0;
        risks.push(format!(
            "Neigt zu Rissen (Risiko {}) - Bohrlöcher vor dem Bohren anfeilen, \
             Gewinde nicht gewaltsam eindrehen, Werkstatttemperatur beachten.",
            material.crack_risk
        ));
    }

    // Oelaufnahme
    if material.oil_absorption >= 0.7 {
        score -= 5.0;
        cons.push("Saugt Oel stark - Oelung zweimal durchfuehren und 24 Stunden dazwischen \
                  trocknen lassen."
            .into());
        tips.push("Oel in zwei Durchgaengen auftragen, dazwischen mindestens \
                  24 Stunden trocknen lassen."
            .into());
    } else if material.oil_absorption <= 0.25 {
        score += 5.0;
        pros.push("Nimmt kaum Oel auf - die Masse bleibt stabil, Politur genuegt.".into());
    }

    // Haerte / Bohrbarkeit
    if let Some(j) = material.janka {
        if j > 2000.0 {
            score -= 6.0;
            risks.push("Sehr hartes Material: Bohrlocher langsam und mit niedriger \
                        Drehzahl bohren, Werkzeug scharf halten, keinen Hitzestau entstehen lassen."
                .into());
            tips.push("Beim Bohren eine Emulsion oder wenig Kuehlmittel verwenden - \
                       Harz und Staub setzen sich sonst in den Bohren fest."
                .into());
        }
    }

    // Gewicht
    if material.density < 0.45 {
        score += 4.0;
        pros.push("Sehr leicht - angenehm bei langer Grifflaenge.".into());
    } else if material.density > 1.3 && material.category != MaterialCategory::Composite {
        score -= 4.0;
        cons.push("Schwer - das Messer wird dadurch kopflastig.".into());
    }

    // Preis
    if material.price_class >= 5 {
        cons.push("Edles Material - zwingt zu sorgfaeltiger Arbeit, Fehler sind nicht \
                   mehr rueckgängig zu machen."
            .into());
    }

    // Bauartspezifische Hinweise
    if material.needs_epoxy_gap() {
        risks.push("Gehaeuse-/Knochenmaterial: die Klebefuge braucht 0,03 - 0,05 mm Spalt. \
                    Mit Epoxy verkleben, nicht mit Leim - sonst bricht die Fuge beim \
                    Ausdehnen."
            .into());
        tips.push("Klebefugen vor dem Kleben mit Alkohol entfetten und mit feinem Schleifpapier \
                   matt schleifen."
            .into());
    }
    if material.category == MaterialCategory::Wood && material.stability >= 0.85 && material.crack_risk <= 0.3 {
        score += 6.0;
        pros.push("Kombination aus hoher Stabilitaet und geringem Risikofaktor - \
                   die beste Wahl fuer eine Konstruktion, die halten soll."
            .into());
    }
    if params.style == HandleStyle::OnePiece && material.category == MaterialCategory::Wood {
        risks.push("Bei einem Ein-Stueck-Griff muss der Nut fuer die Angel mit \
                    0,03 - 0,05 mm Spiel frisch gefraest werden, sonst spannt das Holz."
            .into());
    }

    // Materialbezogene Gestaltungstipps aus der Datenbank
    if material.key == "zebrawood" {
        tips.push("Die Streifenmaserung ist sehr dominant - beide Schalen spiegelbildlich \
                   ausrichten, sonst bricht die Symmetrie."
            .into());
    }
    if material.key == "olivenholz" {
        tips.push("Wegen der unregelmaessigen Struktur jedes Loch mit einer Gegenprobe \
                   bohren und den Bohrtiefenanschlag nutzen."
            .into());
    }
    if material.key == "eiche" {
        tips.push("Die grossen Poren aufwischen und mit einer Beize schliessen, sonst \
                   sammelt sich dort Dreck."
            .into());
    }
    if material.key == "horn" {
        tips.push("Horn nur mit feiner Körnung schleifen - jede grobe Stelle wird zur \
                   Rissspur."
            .into());
    }
    if material.key == "thuja" {
        tips.push("Weiches Holz: die Vorsteckbolzen nur handfest anziehen und mit \
                   Epoxy sichern, sonst drueckt das Holz ein."
            .into());
    }

    // Warnungen der Geometrie uebernehmen
    for w in &geo.warnings {
        risks.push(w.clone());
    }

    // Sehr schwerer Griff im Verhaeltnis zur Klinge
    if blank.blade_length_mm > 0.0 && material.density > 1.2 {
        cons.push(format!(
            "Mit {:.0} kg/m^3 ist {} schwer - bei einem {:.0} mm langen Rohling              waucht das Messer deutlich nach vorn.",
            material.density,
            material.name_de,
            blank.blade_length_mm
        ));
    }

    let score = score.clamp(0.0, 100.0) as u8;
    let rating = match score {
        85..=100 => "Sehr gut geeignet",
        70..=84 => "Gut geeignet",
        55..=69 => "Bedingt geeignet",
        35..=54 => "Eher nicht geeignet",
        _ => "Nicht geeignet",
    };

    let adhesive = material.adhesives.first().copied().unwrap_or("").to_string();
    let finish = material.finishes.first().copied().unwrap_or("").to_string();
    let fastener = if material.category == MaterialCategory::Metal {
        material.fastener.to_string()
    } else if material.crack_risk > 0.55 || material.hardness_class == "sehr hart" {
        "Messing-Stift, handfest anziehen (nicht mit dem Schlaghammer)".to_string()
    } else {
        material.fastener.to_string()
    };

    MaterialVerdict {
        material_key: material.key.to_string(),
        material_name: material.name_de.to_string(),
        score,
        rating: rating.to_string(),
        pros,
        cons,
        risks,
        adhesive,
        fastener,
        finish,
        notes: material.notes.to_string(),
        tips,
        weight_g: None,
        weight_note: None,
    }
}
