"""Material-Datenbank fuer Griff- und Bastionsmaterial.

Gespeist werden:
  * Rohling (Geometrie aus blade.py) -> passt das Material in den Griff?
  * Materialfoto (Vision)           -> Materialklasse, Maserung, Fehlstellen
  * Plan-Ausgabe                    -> Leim, Befestigung, Finish, Risiken

Alle Masse in mm, Dichte in g/cm^3, Janka in lbf.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Iterable, Literal

Category = Literal["wood", "horn", "bone", "antler", "metal", "composite"]


@dataclass(frozen=True)
class Material:
    key: str
    name_de: str
    category: Category
    density: float
    janka: float | None = None
    hardness_class: str = "unbekannt"
    grain: str = "unbekannt"
    oil_absorption: float = 0.5          # 0 = nimmt kein Oel an, 1 = saugt stark
    stability: float = 0.5               # 1 = sehr formstabil
    crack_risk: float = 0.3              # 1 = neigt zu Rissen
    workability: float = 0.5             # 1 = sehr gut bearbeitbar
    figure: str = ""
    adhesives: tuple[str, ...] = ("Zwei-Komponenten-Epoxy",)
    finishes: tuple[str, ...] = ("Hartwachs-Öl",)
    fastener: str = "Messing-Stift"
    notes: str = ""
    price_class: int = 2                 # 1 = günstig ... 5 = Luxus
    good_for: tuple[str, ...] = ()

    # ------------------------------------------------------------------
    @property
    def label(self) -> str:
        return f"{self.name_de} ({self.category})"

    @property
    def is_organic(self) -> bool:
        return self.category in {"wood", "horn", "bone", "antler"}

    @property
    def needs_epoxy_gap(self) -> bool:
        """Horn/Knochen brauchen einen Spalt, sonst bricht die Klebefuge."""
        return self.category in {"horn", "bone", "antler"}

    @property
    def scales_well(self) -> bool:
        """Eignet sich als zweigeteilte Griffschale?"""
        if self.category in {"horn", "bone", "antler"}:
            return False
        if self.category == "composite":
            return True
        if self.category == "metal":
            return False
        return self.crack_risk < 0.85 and self.workability > 0.2

    def matches(self, text: str) -> bool:
        t = text.lower()
        return (
            self.key.lower() in t
            or self.name_de.lower() in t
            or self.category.lower() in t
            or t in self.name_de.lower()
        )


# ---------------------------------------------------------------------------
# Holz
# ---------------------------------------------------------------------------

_WOODS: tuple[Material, ...] = (
    Material(
        key="walnuss", name_de="Nussbaum (Walnuss)", category="wood",
        density=0.66, janka=1010, hardness_class="mittel",
        grain="geradelfasrig, feine Poren", oil_absorption=0.55, stability=0.75,
        crack_risk=0.25, workability=0.85,
        figure="Warme braune Grundfarbe, oft rötlich bis violett, unregelmässige Maserung",
        adhesives=("Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"),
        finishes=("Hartwachs-Öl", "Leinöl + Bienenwachs", "Harte Öl / Öl-Wachs"),
        fastener="Messing-Stift",
        notes="Der Klassiker. Feine Poren, nimmt Oel gut auf, dunkelt mit der Zeit nach. "
              "Spannungsrisiko bei dünnen Schalen nur gering.",
        price_class=2, good_for=("full_tang", "partial_tang"),
    ),
    Material(
        key="eiche", name_de="Eiche", category="wood",
        density=0.75, janka=1360, hardness_class="hart",
        grain="porig, starke Frühholzporen", oil_absorption=0.8, stability=0.9,
        crack_risk=0.2, workability=0.7,
        figure="Deutliche Ringporen, markante Zeichnung, fleckige Durchzeichnung",
        adhesives=("Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"),
        finishes=("Hartwachs-Öl", "Laugenbeize + Öl", "Brennlack"),
        fastener="Messing-Stift",
        notes="Extrem formstabil, porig. Saugt Oel stark -> zweimalige Oelung nötig. "
              "Für kräftige Optik ideal, die grossen Poren bleiben sichtbar.",
        price_class=1, good_for=("full_tang", "one_piece", "ferrule"),
    ),
    Material(
        key="esche", name_de="Esche", category="wood",
        density=0.69, janka=1320, hardness_class="hart",
        grain="geradelfasrig, ringporig", oil_absorption=0.55, stability=0.7,
        crack_risk=0.3, workability=0.8,
        figure="Helle Grundfarbe mit kräftiger Streifenzeichnung, elastig",
        adhesives=("Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"),
        finishes=("Hartwachs-Öl", "Beize + Öl"),
        fastener="Messing-Stift",
        notes="Etwas elastig -> leichtes Durchbiegen im Griff, bei Full Tang sogar "
              "erwünscht (Dämpfung). Guter Allrounder.",
        price_class=1, good_for=("full_tang",),
    ),
    Material(
        key="ahorn", name_de="Ahorn (Hartholz)", category="wood",
        density=0.72, janka=1450, hardness_class="hart",
        grain="fein, gleichmässig, geschlossen", oil_absorption=0.4, stability=0.8,
        crack_risk=0.25, workability=0.8,
        figure="Cremeweiss bis hellgrau, feine Quaste oder Augen-Maserung",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl", "Beize + Öl"),
        fastener="Messing-Stift",
        notes="Sehr hart und dimensionstreu, nimmt Oel wenig an -> Schalen bleiben hell. "
              "Spannungsgarnitur empfohlen.",
        price_class=2, good_for=("full_tang", "partial_tang"),
    ),
    Material(
        key="kirsche", name_de="Kirschbaum", category="wood",
        density=0.58, janka=950, hardness_class="mittel",
        grain="fein, dicht", oil_absorption=0.45, stability=0.6,
        crack_risk=0.4, workability=0.9,
        figure="Warmes Honigbraun, feine Flammen- und Flickzeichnung",
        adhesives=("Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"),
        finishes=("Hartwachs-Öl",),
        fastener="Messing-Stift",
        notes="Sehr gut zu bearbeiten, dunkelt mit der Zeit kräftig nach. "
              "Weicher -> grössere Wandstärke und grosszügige Bohrungen einplanen.",
        price_class=2, good_for=("full_tang", "one_piece"),
    ),
    Material(
        key="birke", name_de="Birke", category="wood",
        density=0.60, janka=980, hardness_class="mittel",
        grain="fein, verschlossen", oil_absorption=0.5, stability=0.6,
        crack_risk=0.35, workability=0.85,
        figure="Cremeweiss mit feiner Längszeichnung, Birkenrübe",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl",),
        fastener="Messing-Stift",
        notes="Günstig, gut zu drechseln, hält Stifte gut. Wird schnell fleckig.",
        price_class=1, good_for=("full_tang", "one_piece"),
    ),
    Material(
        key="ebenholz", name_de="Ebenholz", category="wood",
        density=1.15, janka=3220, hardness_class="sehr hart",
        grain="sehr fein, dicht, geschlossen", oil_absorption=0.15, stability=0.9,
        crack_risk=0.35, workability=0.4,
        figure="Tiefschwarz bis dunkelbraun, feine helle Linien bei bester Qualität",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur", "Hartwachs-Öl (wenig)", "Öl + Bienenwachs"),
        fastener="Messing-Stift",
        notes="Sehr hart und schwer, nimmt kaum Oel auf -> dunkel glänzend polieren. "
              "Spröd bei Biegebelastung: Bohrlöcher grosszügig bohren, "
              "kein Schrauben/Schlag, Faserrichtung beachten.",
        price_class=5, good_for=("full_tang",),
    ),
    Material(
        key="palisander", name_de="Palisander (Rosewood)", category="wood",
        density=0.90, janka=2200, hardness_class="sehr hart",
        grain="geradelfasrig, feinporig", oil_absorption=0.35, stability=0.9,
        crack_risk=0.3, workability=0.55,
        figure="Tiefbraun bis violett-schwarz, dunkle Adern, hoher Kontrast",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl", "Politur"),
        fastener="Messing-Stift",
        notes="Olig, daher nur wenig Leimverlust und hohe Klebestigkeit. "
              "Sehr formstabil, dunkelt kaum nach. Breite Adern als Kontrastelement nutzbar.",
        price_class=5, good_for=("full_tang", "partial_tang", "one_piece"),
    ),
    Material(
        key="wenge", name_de="Wenge", category="wood",
        density=0.87, janka=1630, hardness_class="hart",
        grain="geradelfasrig, sehr dunkel", oil_absorption=0.3, stability=0.85,
        crack_risk=0.45, workability=0.5,
        figure="Tiefschwarz mit feinen helleren Faserlinien",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl", "Politur"),
        fastener="Messing-Stift",
        notes="Spröde, daher nur für erfahrene Macher. Rissgefahr bei "
              "Zu- und Abkühlung in der Werkstatt.",
        price_class=4, good_for=("full_tang",),
    ),
    Material(
        key="olivenholz", name_de="Olivenholz", category="wood",
        density=0.92, janka=1900, hardness_class="sehr hart",
        grain="unregelmässig, wild wachsend", oil_absorption=0.45, stability=0.8,
        crack_risk=0.5, workability=0.45,
        figure="Stark kontrastierend, hell-gelb und dunkel-braun gestreift",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl", "Öl + Bienenwachs"),
        fastener="Messing-Stift",
        notes="Unregelmässige Struktur macht die Bohrungstoleranzen unberechenbar - "
              "Löcher immer mit Gegenprobe bohren. Sehr dekorativ.",
        price_class=4, good_for=("full_tang", "one_piece"),
    ),
    Material(
        key="buchsbaum", name_de="Buchsbaum", category="wood",
        density=0.85, janka=1800, hardness_class="sehr hart",
        grain="fein, homogen, geschlossen", oil_absorption=0.35, stability=0.8,
        crack_risk=0.3, workability=0.65,
        figure="Creme bis honigfarben, feine/rötliche Adern",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl", "Politur"),
        fastener="Messing-Stift",
        notes="Feinste Zeichnung, sehr teuer. Klassiker für edle Full-Tang-Messer.",
        price_class=5, good_for=("full_tang",),
    ),
    Material(
        key="zebrawood", name_de="Zebraholz", category="wood",
        density=0.78, janka=1570, hardness_class="hart",
        grain="stark streifig", oil_absorption=0.5, stability=0.7,
        crack_risk=0.4, workability=0.6,
        figure="Hochkontrastige Zebra-Streifen, hell-braun und dunkel-braun",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Hartwachs-Öl",),
        fastener="Messing-Stift",
        notes="Maserung ist extrem auffällig - Schalen spiegelbildlich ausrichten, "
              "sonst wirkt der Griff unruhig.",
        price_class=4, good_for=("full_tang",),
    ),
    Material(
        key="thuja", name_de="Thuja / Lärche (Knochenholz)", category="wood",
        density=0.39, janka=400, hardness_class="weich",
        grain="sehr weich, langfaserig", oil_absorption=0.7, stability=0.6,
        crack_risk=0.35, workability=0.95,
        figure="Cremeweiss mit kräftiger, dunkler Längsaderung",
        adhesives=("Zwei-Komponenten-Epoxy", "Kunstharzleim PUR"),
        finishes=("Hartwachs-Öl", "Öl + Bienenwachs"),
        fastener="Messing-Stift",
        notes="Extrem leicht, sehr elastisch, ideal fuer verdeckten Tang und "
              "Feder-/Faltmesser. Grosse Bohrungen nötig, Stifte nicht zu fest.",
        price_class=2, good_for=("hidden_tang", "one_piece", "ferrule"),
    ),
    Material(
        key="stabilisiert", name_de="Stabilisierter Wurzelholz", category="composite",
        density=0.75, janka=1200, hardness_class="mittel",
        grain="durch Harz fixiert, Maserung bleibt sichtbar", oil_absorption=0.05,
        stability=1.0, crack_risk=0.05, workability=0.6,
        figure="Kontrastreiche Wurzelmaserung, meist hell mit dunklen Adern",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur", "Hartwachs-Öl"),
        fastener="Messing-Stift",
        notes="Harzgefüllt: kein Kleben noetig (oder nur minimal), nicht spannungsempfindlich. "
              "Ideal fuer Ahorn-Art-Konstruktionen mit eingearbeitetem Material.",
        price_class=4, good_for=("full_tang", "one_piece", "hidden_tang"),
    ),
    Material(
        key="composite", name_de="Glasfaser-Komposit (CFK/FRP)", category="composite",
        density=1.55, janka=None, hardness_class="sehr hart",
        grain="Faserlaminat, Schichtaufbau", oil_absorption=0.0, stability=1.0,
        crack_risk=0.05, workability=0.5,
        figure="Industriell, sichtbare Schichtkante",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur",),
        fastener="Messing-Stift",
        notes="Sehr leicht und steif. Schichtkante sichtbar lassen oder überfräsen. "
              "Bohrstaub ist scharfkantig - Atemschutz.",
        price_class=3, good_for=("one_piece",),
    ),
)

# ---------------------------------------------------------------------------
# Horn / Knochen / Geweih
# ---------------------------------------------------------------------------

_ORGANIC: tuple[Material, ...] = (
    Material(
        key="horn", name_de="Horn (Büffelhorn)", category="horn",
        density=1.45, janka=2000, hardness_class="sehr hart",
        grain="schichtig, konzentrisch", oil_absorption=0.25, stability=0.7,
        crack_risk=0.6, workability=0.35,
        figure="Tiefbraun bis schwarz mit transparenz, oft helle Knochen-Einschlüsse",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur", "Hartwachs-Öl"),
        fastener="Messing-Stift",
        notes="Klassischer Full-Tang-Griff mit Angel-Einlage. Braucht einen Spalt von "
              "0,03-0,05 mm, sonst bricht die Leimfuge. Nur fein schleifen, "
              "nicht viel Wärme - es reisst sonst aus.",
        price_class=4, good_for=("full_tang",),
    ),
    Material(
        key="bone", name_de="Knochen (Rinder-/Hirschknochen)", category="bone",
        density=1.70, janka=None, hardness_class="hart",
        grain="kompakt, feinporig", oil_absorption=0.1, stability=0.8,
        crack_risk=0.5, workability=0.5,
        figure="Elfenbeinfarben, kann eingefärbt werden",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur", "Färbung + Öl"),
        fastener="Messing-Stift",
        notes="Feiner als Horn, gut zu färben. Muss trocken sein, sonst braucht die "
              "Leimfuge lange zum Abbinden. Stabile Alternative zu Horn.",
        price_class=2, good_for=("full_tang",),
    ),
    Material(
        key="geweih", name_de="Geweih (Reh/Hirsch)", category="antler",
        density=1.55, janka=None, hardness_class="hart",
        grain="porös, Jahresringe sichtbar", oil_absorption=0.3, stability=0.75,
        crack_risk=0.45, workability=0.55,
        figure="Cremefarben mit dunklen Jahresringen",
        adhesives=("Zwei-Komponenten-Epoxy",),
        finishes=("Politur", "Färbung + Öl"),
        fastener="Messing-Stift",
        notes="Guter Füll- und Abstandshalter zwischen Bastion und Schale. "
              "Trocknet langsam - Leim lange halten.",
        price_class=2, good_for=("full_tang", "partial_tang"),
    ),
)

# ---------------------------------------------------------------------------
# Metalle
# ---------------------------------------------------------------------------

_METALS: tuple[Material, ...] = (
    Material(
        key="messing", name_de="Messing", category="metal",
        density=8.50, janka=None, hardness_class="hart",
        grain="metallisch, kristallin", oil_absorption=0.0, stability=1.0,
        crack_risk=0.0, workability=0.8,
        figure="Warmes Goldgelb, mit der Zeit Patina",
        adhesives=("Kunstharzleim PUR", "Epoxy mit Sekundenhaft"),
        finishes=("Politur", "Patina", "Bürsten"),
        fastener="Messing-Stift",
        notes="Standard fuer Bastionen, Zwingen und Stifte: weich genug, um die "
              "Schale nicht zu sprengen, aber fest genug. Bastion anschliessend "
              "an den Rohling anschweissen.",
        price_class=2, good_for=("partial_tang", "ferrule", "full_tang"),
    ),
    Material(
        key="kupfer", name_de="Kupfer", category="metal",
        density=8.96, janka=None, hardness_class="hart",
        grain="metallisch, weich", oil_absorption=0.0, stability=1.0,
        crack_risk=0.0, workability=0.85,
        figure="Rötlich-orange, dunkelt zur Patina",
        adhesives=("Epoxy mit Sekundenhaft",),
        finishes=("Politur", "Patina"),
        fastener="Kupfer-Stift",
        notes="Noch weicher als Messing, klassische Wahl für Zwingen an Folienmessern. "
              "Nicht mit Stahl verkleben - differentielle Ausdehnung.",
        price_class=3, good_for=("ferrule",),
    ),
    Material(
        key="edelstahl", name_de="Edelstahl (1.4301 / 1.4404)", category="metal",
        density=7.90, janka=None, hardness_class="sehr hart",
        grain="metallisch", oil_absorption=0.0, stability=1.0,
        crack_risk=0.0, workability=0.6,
        figure="Neutral, matt oder poliert",
        adhesives=(),
        finishes=("Politur", "Bürsten", "Sandstrahlen"),
        fastener="Edelstahl-Stift",
        notes="Rostfrei, aber hart - als Bastion sprengt es Schalen aus weichem Holz. "
              "Nur mit Presspassung oder Schraube befestigen, nicht verkleben.",
        price_class=2, good_for=("partial_tang", "one_piece"),
    ),
    Material(
        key="titan", name_de="Titan", category="metal",
        density=4.50, janka=None, hardness_class="sehr hart",
        grain="metallisch", oil_absorption=0.0, stability=1.0,
        crack_risk=0.0, workability=0.45,
        figure="Grau, matt, anodisierbar in Farben",
        adhesives=(),
        finishes=("Sandstrahlen", "Anodisieren", "Politur"),
        fastener="Titan-Stift",
        notes="Leicht und korrosionsbeständig. Teuer; als Bastion meist Overkill.",
        price_class=5, good_for=("partial_tang", "one_piece"),
    ),
    Material(
        key="aluminium", name_de="Aluminium", category="metal",
        density=2.70, janka=None, hardness_class="weich",
        grain="metallisch, weich", oil_absorption=0.0, stability=1.0,
        crack_risk=0.0, workability=0.9,
        figure="Mattes Grau, eloxierbar",
        adhesives=("Epoxy mit Sekundenhaft",),
        finishes=("Eloxieren", "Sandstrahlen"),
        fastener="Messing-Stift",
        notes="Sehr weich - als Bastion federst, als Griffhülse aber sehr leicht.",
        price_class=2, good_for=("ferrule",),
    ),
)

MATERIALS: tuple[Material, ...] = _WOODS + _ORGANIC + _METALS

_BY_KEY: dict[str, Material] = {m.key: m for m in MATERIALS}


# ---------------------------------------------------------------------------
# Zugriff
# ---------------------------------------------------------------------------

def get(key: str) -> Material:
    try:
        return _BY_KEY[key.lower().strip().replace(" ", "_").replace("-", "_")]
    except KeyError:
        raise KeyError(f"Unbekanntes Material: {key!r}") from None


def has(key: str) -> bool:
    try:
        get(key)
        return True
    except KeyError:
        return False


def by_category(category: Category) -> list[Material]:
    return [m for m in MATERIALS if m.category == category]


def search(text: str) -> list[Material]:
    """Freitextsuche ueber Name, Kategorie und Merkmale."""
    if not text:
        return []
    t = text.lower()
    hits = []
    for m in MATERIALS:
        hay = " ".join(
            [m.key, m.name_de, m.category, m.hardness_class, m.grain, m.figure, m.notes]
        ).lower()
        if t in hay:
            hits.append(m)
    # Treffer mit dem dichtesten Vorkommen nach oben
    hits.sort(key=lambda m: -haystack_count(m, t))
    return hits


def haystack_count(m: Material, t: str) -> int:
    hay = " ".join([m.key, m.name_de, m.category, m.figure, m.notes]).lower()
    return hay.count(t)


def suggest_from_vision(class_hint: str, species_hint: str | None = None) -> list[Material]:
    """Materialvorschlaege aus einer Vision-Beschreibung ableiten."""
    out: list[Material] = []
    if species_hint:
        out.extend(search(species_hint))
    out.extend(search(class_hint))
    seen: set[str] = set()
    unique: list[Material] = []
    for m in out:
        if m.key not in seen:
            seen.add(m.key)
            unique.append(m)
    return unique[:8]
