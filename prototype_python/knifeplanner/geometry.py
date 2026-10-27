"""Geometrie-Engine: aus Rohling + Gestaltungsidee wird ein baubarer Griff.

Erzeugt:
  * Griffkontur (Seitenprofil der Griffschale)
  * Angelprofil und daraus die Aussparung in der Schale
  * Bohrplan: Vorsteckbolzen, Reibnieten, Tangbolzen, Lanyardloch
  * Bastions- bzw. Zwingenkontur

Koordinaten:
  x = 0 an der Vorderkante des Griffs (Anschluss an die Klinge),
      x waechst zum Griffende.
  y = 0 auf der Mittellinie der Angel,
      +y = Ruecken (Klingenkontur oben), -y = Bauch (Schneidseite unten).
  z = 0 in der Griffmitte, +/- = Dickenrichtung.
"""

from __future__ import annotations

import math
from typing import Iterable, Sequence

from .models import BladeBlank, HandleGeometry, HandleParams, PinSpec

Point = tuple[float, float]

SAMPLE_STEP = 0.75  # Abtastschritt in mm entlang der Grifflaenge


# ---------------------------------------------------------------------------
# Hilfsfunktionen
# ---------------------------------------------------------------------------

def _smooth(values: Sequence[float], window: int = 5) -> list[float]:
    """Gleitender Mittelwert, Haelt die Laenge bei."""
    if window < 2 or len(values) < 3:
        return list(values)
    half = window // 2
    out: list[float] = []
    n = len(values)
    for i in range(n):
        lo = max(0, i - half)
        hi = min(n, i + half + 1)
        out.append(sum(values[lo:hi]) / (hi - lo))
    return out


def _smooth_max(values: Sequence[float], window: int = 5) -> list[float]:
    """Gleitendes Maximum - behaelt die Wandstärke garantiert ein."""
    if window < 2 or len(values) < 3:
        return list(values)
    half = window // 2
    out: list[float] = []
    n = len(values)
    for i in range(n):
        lo = max(0, i - half)
        hi = min(n, i + half + 1)
        out.append(max(values[lo:hi]))
    return out


def _smooth_min(values: Sequence[float], window: int = 5) -> list[float]:
    if window < 2 or len(values) < 3:
        return list(values)
    half = window // 2
    out: list[float] = []
    n = len(values)
    for i in range(n):
        lo = max(0, i - half)
        hi = min(n, i + half + 1)
        out.append(min(values[lo:hi]))
    return out


def _lerp(a: float, b: float, t: float) -> float:
    return a + (b - a) * t


def _clamp(v: float, lo: float, hi: float) -> float:
    return max(lo, min(hi, v))


def _smoothstep(t: float) -> float:
    t = _clamp(t, 0.0, 1.0)
    return t * t * (3.0 - 2.0 * t)


# ---------------------------------------------------------------------------
# Angelprofil
# ---------------------------------------------------------------------------

def tang_half_width(b: BladeBlank, x: float) -> float:
    """Halbe Angelbreite an der Stelle x (0 = Vorderkante des Griffs)."""
    L = float(b.tang_length_mm)
    w0 = float(b.tang_width_mm) / 2.0
    if x <= 0.0:
        return w0
    if x >= L:
        return 0.0
    if not b.tang_taper:
        return w0

    x0 = _clamp(b.tang_taper_start, 0.0, 0.95) * L
    if x <= x0:
        return w0

    w_back = float(b.tang_back_width_mm) / 2.0
    if w_back > 0.0:
        t = (x - x0) / max(1e-6, L - x0)
        return max(0.0, w0 + (w_back - w0) * t)

    # Verjuengung auf eine runde Spitze
    span = max(1e-6, L - x0)
    x_lin_end = x0 + span * 0.90
    r = max(0.5, min(w0 * 0.5, span * 0.10))
    if x <= x_lin_end:
        t = (x - x0) / max(1e-6, x_lin_end - x0)
        return max(0.0, w0 * (1.0 - t))
    xc = x_lin_end
    d = x - xc
    if d >= r:
        return 0.0
    return math.sqrt(max(0.0, r * r - d * d))


def tang_profile(b: BladeBlank, step: float = SAMPLE_STEP) -> list[Point]:
    """Angelprofil als geschlossenes Polygon (x, y) mit y = 0 auf der Mittellinie."""
    L = float(b.tang_length_mm)
    xs = [i * step for i in range(int(L / step) + 1)]
    if xs[-1] < L:
        xs.append(L)
    top = [(x, tang_half_width(b, x)) for x in xs]
    bottom = [(x, -tang_half_width(b, x)) for x in reversed(xs)]
    return top + bottom


def offset_polygon(poly: Sequence[Point], delta: float) -> list[Point]:
    """Polygon nach aussen aufweiten (positiver delta = nach aussen)."""
    if not poly or delta == 0.0:
        return list(poly)
    n = len(poly)
    out: list[Point] = []
    for i in range(n):
        px, py = poly[i - 1]
        cx, cy = poly[i]
        nx, ny = poly[(i + 1) % n]
        # Einheitsnormalen der beiden angrenzenden Kanten
        def normal(ax: float, ay: float, bx: float, by: float) -> tuple[float, float]:
            dx, dy = bx - ax, by - ay
            ln = math.hypot(dx, dy)
            if ln < 1e-9:
                return (0.0, 0.0)
            return (-dy / ln, dx / ln)

        n1 = normal(px, py, cx, cy)
        n2 = normal(cx, cy, nx, ny)
        mx, my = n1[0] + n2[0], n1[1] + n2[1]
        ml = math.hypot(mx, my)
        if ml < 1e-9:
            out.append((cx, cy))
            continue
        mx, my = mx / ml, my / ml
        cosang = max(0.2, mx * n1[0] + my * n1[1])
        out.append((cx + mx * delta / cosang, cy + my * delta / cosang))
    return out


# ---------------------------------------------------------------------------
# Griffkontur
# ---------------------------------------------------------------------------

def _spine_curve(b: BladeBlank, p: HandleParams, x: float, x_end: float) -> float:
    """Rueckenkontur des Griffs (y-Wert)."""
    base = b.spine_offset_mm
    t = x / max(1e-6, x_end)
    # Der Ruecken faellt zum Griffende leicht ab, bleibt aber waagerecht beim Angriff.
    drop = p.spine_camber_mm * _smoothstep(_clamp((t - 0.08) / 0.92, 0.0, 1.0))
    return base - drop


def _belly_curve(b: BladeBlank, p: HandleParams, x: float, x_end: float) -> float:
    """Bauchkontur des Griffs (negativer y-Wert)."""
    base = -abs(b.edge_offset_mm)
    t = x / max(1e-6, x_end)
    # Wölbung in der Griffzone, faellt zum Ende hin wieder ab.
    belly = p.belly_drop_mm * (4.0 * t * (1.0 - t)) ** 0.7
    return base - belly


def _butt_cap(
    x_end: float, y_top: float, y_bot: float, style: str
) -> list[Point]:
    """Gerundetes oder flaches Griffende."""
    mid = (y_top + y_bot) / 2.0
    half = (y_top - y_bot) / 2.0
    if half <= 0.1:
        return [(x_end, y_top), (x_end, y_bot)]

    if style == "flat":
        depth = half * 0.35
    elif style == "flare":
        depth = half * 1.25
    else:
        depth = half

    # Ellipsen-Bogen von (x_end, y_top) nach (x_end, y_bot) ueber x_end + depth
    steps = 28
    pts: list[Point] = []
    for i in range(steps + 1):
        a = math.pi * i / steps          # 0 -> pi
        y = mid + half * math.cos(a)
        x = x_end + depth * math.sin(a)
        pts.append((x, y))
    return pts


def build_outline(b: BladeBlank, p: HandleParams) -> tuple[list[Point], list[str]]:
    """Griffkontur der Holzschale + Warnungen."""
    warnings: list[str] = []

    if p.style == "full_tang_two_scale":
        x_start = 0.0
    elif p.style == "partial_tang_bolster":
        x_start = p.bolster_width_mm
    elif p.style == "ferrule":
        x_start = p.ferrule_width_mm
    else:  # one_piece
        x_start = 0.0

    x_end = float(b.tang_length_mm) + max(0.0, p.butt_overhang_mm)

    n = max(8, int(math.ceil((x_end - x_start) / SAMPLE_STEP)))
    xs = [x_start + (x_end - x_start) * i / n for i in range(n + 1)]

    tops = [_spine_curve(b, p, x, x_end) for x in xs]
    bots = [_belly_curve(b, p, x, x_end) for x in xs]

    # Wandstaerke garantieren: Abstand Angel <-> Aussenkontur >= wall_min
    wall = max(0.8, p.wall_min_mm)
    for i, x in enumerate(xs):
        w = tang_half_width(b, x)
        if w > 0:
            tops[i] = max(tops[i], w + wall)
            bots[i] = min(bots[i], -(w + wall))

    tops = _smooth(tops, 5)
    bots = _smooth(bots, 5)
    # Nach dem Glaetten erneut erzwingen
    for i, x in enumerate(xs):
        w = tang_half_width(b, x)
        if w > 0:
            tops[i] = max(tops[i], w + wall)
            bots[i] = min(bots[i], -(w + wall))
    tops = _smooth_max(tops, 3)
    bots = _smooth_min(bots, 3)

    # Vorderkante: gerade oder mit Fase
    if abs(p.front_bevel_deg) > 0.01:
        tan_b = math.tan(math.radians(abs(p.front_bevel_deg)))
        shift = (tops[0] - bots[0]) * tan_b
        tops = [tops[0] - shift] + tops
        bots = [bots[0]] + bots
        xs = [x_start - shift] + xs
        x_start = xs[0]

    # Kontur zusammensetzen: Vorderkante -> Ruecken -> Endkappe -> Bauch
    outline: list[Point] = [(xs[0], bots[0])]
    outline += [(xs[i], tops[i]) for i in range(len(xs))]
    outline += _butt_cap(x_end, tops[-1], bots[-1], p.butt_style)[1:]
    outline += [(xs[i], bots[i]) for i in range(len(xs) - 1, -1, -1)]

    # Tang passt nicht in das Heftprofil?
    if b.tang_half_width + wall > min(b.spine_offset_mm, b.edge_offset_mm):
        warnings.append(
            "Die Angel ist breiter als das Heftprofil zulässt - die Griffkontur "
            "musst aufweiten. Eine Bastion oder eine Aenderung der Rohlinggeometrie "
            "ist nötig, sonst steht die Griffkontur über dem Heft vor."
        )

    return outline, warnings


# ---------------------------------------------------------------------------
# Bohrbild
# ---------------------------------------------------------------------------

def _pin_diameter(b: BladeBlank) -> float:
    d = float(b.tang_thickness_mm) * 0.5
    return round(_clamp(d, 2.0, 5.0), 1)


def _parallel_span(b: BladeBlank) -> tuple[float, float]:
    """Bereich der Angel, in dem die Breite konstant ist."""
    L = float(b.tang_length_mm)
    x0 = _clamp(b.tang_taper_start, 0.0, 0.95) * L
    if not b.tang_taper:
        return (0.0, L)
    return (x0, L)


def plan_pins(b: BladeBlank, p: HandleParams) -> list[PinSpec]:
    """Bohrplan nach den ueblichen Regeln des Messerbaus."""
    pins: list[PinSpec] = []
    L = float(b.tang_length_mm)
    x0, x1 = _parallel_span(b)
    usable = x1 - x0
    if usable < 12.0:
        return pins

    dia = _pin_diameter(b)
    can_through = b.tang_thickness_mm >= 2.6

    margin = max(8.0, min(18.0, L * 0.11))
    front = _clamp(x0 + margin, x0 + 3.0, x1 - 3.0)
    rear = _clamp(x1 - margin, x0 + 3.0, x1 - 3.0)

    if can_through:
        positions = [front]
        span = rear - front
        max_span = 75.0
        if span > max_span:
            count = int(math.ceil(span / max_span))
            for k in range(1, count):
                positions.append(front + span * k / count)
        positions.append(rear)

        for i, x in enumerate(positions):
            if i == 0:
                label = "Vorsteckbolzen vorn"
                why = "Sichert den vorderen Griffanschluss am Rohling."
            elif i == len(positions) - 1:
                label = "Vorsteckbolzen hinten"
                why = "Sichert das Griffende gegen Abplatzen und Ausziehen."
            else:
                label = f"Vorsteckbolzen Mitte {i}"
                why = "Begrenzt den Abstand zwischen den Bolzen auf unter 75 mm."
            pins.append(
                PinSpec(
                    kind="through",
                    label=label,
                    x_mm=round(x, 2),
                    y_mm=0.0,
                    dia_mm=dia,
                    depth_mm=None,
                    through_tang=True,
                    rationale=why,
                )
            )

        # Reibnieten gegen Verdrehen im vorderen Griffdrittel
        if (rear - front) > 45.0:
            xf = front + (rear - front) * 0.28
            xf = _clamp(xf, x0 + 2.0, x1 - 2.0)
            pins.append(
                PinSpec(
                    kind="friction",
                    label="Reibniet",
                    x_mm=round(xf, 2),
                    y_mm=0.0,
                    dia_mm=2.4,
                    depth_mm=round(float(b.tang_thickness_mm) / 2.0, 2),
                    through_tang=False,
                    rationale=(
                        "Reibt in einer halbtiefen Nut in der Angel und verhindert, "
                        "dass sich die Schale im vorderen Griffdrittel verdreht. "
                        "Nut in der Angel breit genug für den Niet anlegen."
                    ),
                )
            )
    else:
        pins.append(
            PinSpec(
                kind="tang",
                label="Querbolzen in die Angel",
                x_mm=round(_clamp(x0 + 6.0, x0, x1), 2),
                y_mm=0.0,
                dia_mm=2.0,
                depth_mm=round(float(b.tang_thickness_mm) + 1.0, 2),
                through_tang=False,
                rationale=(
                    "Die Angel ist zu dünn für einen durchgehenden Vorsteckbolzen. "
                    "Querbolzen quer durch die Angel in die Schale verankern."
                ),
            )
        )

    # Lanyardloch
    if p.lanyard_hole and p.lanyard_hole_dia_mm > 0:
        cap_extent = (tops[-1] - bots[-1]) / 2.0 if False else None
        x_lanyard = L - 8.0
        x_lanyard = _clamp(x_lanyard, 6.0, max(6.0, L - 2.0))
        tang_there = tang_half_width(b, x_lanyard)
        note = (
            "Durch das Griffende gebohrt, quer zur Grifflaenge. "
            "Liegt die Angel dort noch durch, braucht es einen Entlastungsslot "
            f"von {p.lanyard_hole_dia_mm + 1.0:.1f} mm Breite in der Angel."
            if tang_there > 0.4
            else "Durch das Griffende gebohrt, quer zur Grifflaenge. "
            "Die Angel endet davor, das Loch liegt vollstaendig im Holz."
        )
        pins.append(
            PinSpec(
                kind="lanyard",
                label="Lanyardloch",
                x_mm=round(x_lanyard, 2),
                y_mm=0.0,
                dia_mm=round(float(p.lanyard_hole_dia_mm), 1),
                depth_mm=None,
                through_tang=False,
                rationale=note,
            )
        )

    return pins


# ---------------------------------------------------------------------------
# Bastion / Zwinge
# ---------------------------------------------------------------------------

def bolster_profile(b: BladeBlank, p: HandleParams) -> list[Point]:
    """Kontur der Bastion (Metall) als Uebergang vom Heft zum Griff."""
    w = max(6.0, p.bolster_width_mm)
    x1 = w
    x0 = 0.0
    y_top0 = b.spine_offset_mm
    y_bot0 = -abs(b.edge_offset_mm)
    y_top1 = _spine_curve(b, p, x1, b.tang_length_mm)
    y_bot1 = _belly_curve(b, p, x1, b.tang_length_mm)
    pts = [(x0, y_top0), (x1, y_top1)]
    pts += [(x1 + 0.4 * (x1 - x0) * 0, y_bot1)]
    # Bastion faegt nach vorn zum Heft hin und hat eine leichte Kehle
    keel = 0.6
    pts += [(x1 - w * 0.25, (y_top1 + y_bot1) / 2.0 - keel)]
    pts += [(x0, y_bot0)]
    return pts


def ferrule_profile(b: BladeBlank, p: HandleParams) -> list[Point]:
    """Kontur der Zwinge (Messing) als Huelse am Griffanfang."""
    w = max(4.0, p.ferrule_width_mm)
    y_top = b.spine_offset_mm
    y_bot = -abs(b.edge_offset_mm)
    mid = (y_top + y_bot) / 2.0
    half = (y_top - y_bot) / 2.0
    steps = 18
    pts: list[Point] = []
    for i in range(steps + 1):
        a = math.pi * i / steps
        pts.append((w * 0.15 + (w * 0.85) * math.sin(a), mid + half * math.cos(a)))
    # leichte Aufweitung nach hinten
    pts = [(x + w * 0.25 * (1 - abs((y - mid) / half)), y) for x, y in pts]
    return pts


# ---------------------------------------------------------------------------
# Gesamtergebnis
# ---------------------------------------------------------------------------

def recommend_style(b: BladeBlank) -> str:
    """Empfohlene Griffbauart fuer einen Rohling."""
    if b.has_bolster:
        return "partial_tang_bolster"
    if b.tang_style == "partial":
        return "partial_tang_bolster"
    if b.tang_style == "hidden":
        return "one_piece"
    if b.tang_thickness_mm < 3.0 or b.tang_length_mm > 135.0:
        return "one_piece"
    return "full_tang_two_scale"


def build_geometry(b: BladeBlank, p: HandleParams) -> HandleGeometry:
    """Erzeugt die komplette Griffgeometrie."""
    warnings: list[str] = []
    notes: list[str] = []

    outline, outline_warnings = build_outline(b, p)
    warnings.extend(outline_warnings)

    tang = tang_profile(b)
    recess = offset_polygon(tang, p.glue_gap_mm) if p.glue_gap_mm else list(tang)

    # Schalendicke
    if p.scale_thickness_mm is not None and p.scale_thickness_mm > 0:
        scale_t = float(p.scale_thickness_mm)
    else:
        if p.style in {"full_tang_two_scale", "partial_tang_bolster", "ferrule"}:
            scale_t = (p.target_thickness_mm - b.tang_thickness_mm) / 2.0
        else:
            scale_t = p.target_thickness_mm
    scale_t = round(_clamp(scale_t, 2.5, 10.0), 2)
    total_t = round(b.tang_thickness_mm + 2 * scale_t, 2)

    if scale_t <= 2.6 and p.style in {"full_tang_two_scale", "partial_tang_bolster"}:
        warnings.append(
            f"Die Schalendicke liegt bei nur {scale_t:.1f} mm. Unter 2,6 mm reisst "
            "Holz beim Anziehen der Vorsteckbolzen aus - dickeres Holz oder "
            "Epoxy- und Reibnieten-Verbindung wählen."
        )

    # Bohrplan
    pins = plan_pins(b, p)

    # Bastion / Zwinge
    bolster = None
    ferrule = None
    if p.style == "partial_tang_bolster":
        bolster = bolster_profile(b, p)
    elif p.style == "ferrule":
        ferrule = ferrule_profile(b, p)

    # Masse
    y_tops = [y for _, y in outline]
    height = round(max(y_tops) - min(y_tops), 2)
    x_values = [x for x, _ in outline]
    length = round(max(x_values) - min(x_values), 2)

    wood_start = 0.0
    wood_end = float(b.tang_length_mm)
    if p.style == "partial_tang_bolster":
        wood_start = p.bolster_width_mm
    elif p.style == "ferrule":
        wood_start = p.ferrule_width_mm

    # Ergonomie-Hinweise
    grip_len = length - wood_start
    if grip_len < 85:
        warnings.append(
            f"Der nutzbare Griffriff misst nur {grip_len:.0f} mm. Das ist kurz - "
            "für ein Jagdmesser mit ganzer Hand zu knapp."
        )
    elif grip_len > 145:
        warnings.append(
            f"Der Griff ist mit {grip_len:.0f} mm länger als bei den meisten "
            "Mäusern. Ganzschalengriff an den Händen abgleichen."
        )

    if p.style == "full_tang_two_scale" and b.tang_style == "full":
        notes.append(
            "Full Tang: die Angel läuft durch den gesamten Griff. Beide Griffschalen "
            "sind spiegelbildlich, die Bohrungen für Vorsteckbolzen gehen durch die "
            "Angel."
        )
    if p.style == "partial_tang_bolster":
        notes.append(
            "Partial Tang mit Bastion: die Bastion ueberbrueckt den vorderen "
            "Griffteil. Die Bastion wird mit der Angel verschweisst, der Holzteil "
            "wird mit Reibnieten und Epoxy fixiert."
        )
    if p.style == "one_piece":
        notes.append(
            "Ein-Stück-Griff: die Angel wird als Nut in das Holz eingelassen. "
            "Nut mit 0,03 - 0,05 mm Spiel, sonst klebt das Holz an der Angel fest "
            "und spannt beim Trocknen."
        )
    if p.style == "ferrule":
        notes.append(
            "Griff mit Zwinge: die Zwinge klemmt das Holz am Rohling und fasst "
            "die Angelenden. Die Angel wird eingelassen, die Zwinge verhindert das "
            "Auseinanderziehen des Holzes."
        )

    metrics = {
        "grifffaenge_mm": round(length, 1),
        "griffhoehe_mm": height,
        "schalendicke_mm": scale_t,
        "gesamtdicke_mm": total_t,
        "angel_laenge_mm": round(float(b.tang_length_mm), 1),
        "wandstaerke_mm": round(p.wall_min_mm, 1),
        "holzvolumen_mm3": round(
            sum(
                (top + (-bot)) / 2.0 * SAMPLE_STEP * scale_t * 2.0
                for top, bot in zip(
                    [y for x, y in outline if x <= b.tang_length_mm],
                    [y for x, y in outline if x <= b.tang_length_mm],
                )
            ),
            0,
        ),
        "anker_anzahl": float(sum(1 for p_ in pins if p_.kind == "through")),
    }

    return HandleGeometry(
        outline=outline,
        tang_profile=tang,
        recess=recess,
        pins=pins,
        scale_thickness_mm=scale_t,
        total_thickness_mm=total_t,
        handle_length_mm=length,
        handle_height_mm=height,
        wood_start_x_mm=wood_start,
        wood_end_x_mm=wood_end,
        bolster_profile=bolster,
        ferrule_profile=ferrule,
        metrics=metrics,
        warnings=warnings,
        notes=notes,
    )
