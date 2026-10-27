"""Datenmodell des Griff-Planers.

Alle Masse in Millimeter, Winkel in Grad.
"""

from __future__ import annotations

from dataclasses import dataclass, field, asdict
from typing import Any, Literal

TangStyle = Literal["full", "partial", "hidden"]
HandleStyle = Literal["full_tang_two_scale", "partial_tang_bolster", "one_piece", "ferrule"]
GripType = Literal["hunting", "tactical", "chef", "folder", "display"]
ButtStyle = Literal["round", "flat", "flare"]

STYLE_LABELS: dict[str, str] = {
    "full_tang_two_scale": "Full Tang, zwei Griffschalen",
    "partial_tang_bolster": "Verdeckter Tang mit Bastion",
    "one_piece": "Ein-Stück-Griff aus einem Stück Holz",
    "ferrule": "Holzgriff mit Messing-Zwinge",
}

TANG_LABELS: dict[str, str] = {
    "full": "Durchgehende Angel (Full Tang)",
    "partial": "Teildeckende Angel (Partial Tang)",
    "hidden": "Verdeckte Angel (Hidden Tang)",
}


@dataclass
class BladeBlank:
    """Der gemessene bzw. aus dem Foto geschätzte Klingenrohling."""

    blade_length_mm: float = 100.0
    blade_width_mm: float = 26.0
    blade_thickness_mm: float = 3.0

    tang_length_mm: float = 110.0
    tang_width_mm: float = 6.5
    tang_thickness_mm: float = 6.5
    tang_style: TangStyle = "full"
    tang_taper: bool = True
    tang_back_width_mm: float = 0.0
    tang_taper_start: float = 0.55

    # Abstand der Tangmitte zur Klingenkontur am Heft
    spine_offset_mm: float = 8.0
    edge_offset_mm: float = 7.5

    has_bolster: bool = False
    has_guard: bool = False
    ricasso_mm: float = 0.0
    notes: str = ""

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "BladeBlank":
        allowed = {f for f in cls.__dataclass_fields__}
        return cls(**{k: v for k, v in data.items() if k in allowed})

    @property
    def tang_half_width(self) -> float:
        return self.tang_width_mm / 2.0

    @property
    def handle_length_mm(self) -> float:
        return self.tang_length_mm


@dataclass
class HandleParams:
    """Gestaltungsparameter des Griffs."""

    style: HandleStyle = "full_tang_two_scale"
    target_thickness_mm: float = 22.0
    scale_thickness_mm: float | None = None

    wall_min_mm: float = 2.0
    glue_gap_mm: float = 0.03

    spine_camber_mm: float = 1.5
    belly_drop_mm: float = 3.0
    butt_style: ButtStyle = "round"
    butt_overhang_mm: float = 0.0
    front_bevel_deg: float = 0.0

    bolster_width_mm: float = 18.0
    ferrule_width_mm: float = 14.0

    edge_break_mm: float = 0.8
    lanyard_hole: bool = True
    lanyard_hole_dia_mm: float = 5.0

    grip: GripType = "hunting"

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "HandleParams":
        allowed = {f for f in cls.__dataclass_fields__}
        return cls(**{k: v for k, v in data.items() if k in allowed})


@dataclass
class PinSpec:
    """Eine Bohrung im Griff."""

    kind: str                 # through | friction | tang | lanyard
    label: str
    x_mm: float
    y_mm: float
    dia_mm: float
    depth_mm: float | None = None
    through_tang: bool = False
    rationale: str = ""

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)


@dataclass
class HandleGeometry:
    """Ergebnis der Geometrie-Engine."""

    outline: list[tuple[float, float]]
    tang_profile: list[tuple[float, float]]
    recess: list[tuple[float, float]]
    pins: list[PinSpec]

    scale_thickness_mm: float
    total_thickness_mm: float
    handle_length_mm: float
    handle_height_mm: float

    wood_start_x_mm: float
    wood_end_x_mm: float
    bolster_profile: list[tuple[float, float]] | None
    ferrule_profile: list[tuple[float, float]] | None

    metrics: dict[str, float] = field(default_factory=dict)
    warnings: list[str] = field(default_factory=list)
    notes: list[str] = field(default_factory=list)

    def to_dict(self) -> dict[str, Any]:
        return {
            "outline": self.outline,
            "tang_profile": self.tang_profile,
            "recess": self.recess,
            "pins": [p.to_dict() for p in self.pins],
            "scale_thickness_mm": self.scale_thickness_mm,
            "total_thickness_mm": self.total_thickness_mm,
            "handle_length_mm": self.handle_length_mm,
            "handle_height_mm": self.handle_height_mm,
            "wood_start_x_mm": self.wood_start_x_mm,
            "wood_end_x_mm": self.wood_end_x_mm,
            "metrics": self.metrics,
            "warnings": self.warnings,
            "notes": self.notes,
        }
