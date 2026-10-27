"""Einfache Mesh-Erzeugung und STL-Export.

Ohne CSG-Bibliothek: jedes Volumen ist ein sauber geschlossenes Prisma mit
 triangulierten Deckflaechen (Ear-Clipping). Der Aussparungs-koerper wird als
eigenes Volumen im selben STL abgelegt - die Differenz uebernimmt CAD/Slicer.
"""

from __future__ import annotations

import math
import struct
from dataclasses import dataclass
from pathlib import Path
from typing import Sequence

Point = tuple[float, float]
Tri = tuple[int, int, int]


# ---------------------------------------------------------------------------
# Ear-Clipping
# ---------------------------------------------------------------------------

def _signed_area(poly: Sequence[Point]) -> float:
    s = 0.0
    n = len(poly)
    for i in range(n):
        x1, y1 = poly[i]
        x2, y2 = poly[(i + 1) % n]
        s += x1 * y2 - x2 * y1
    return s / 2.0


def _point_in_triangle(p: Point, a: Point, b: Point, c: Point) -> bool:
    def sign(p1: Point, p2: Point, p3: Point) -> float:
        return (p1[0] - p3[0]) * (p2[1] - p3[1]) - (p2[0] - p3[0]) * (p1[1] - p3[1])

    d1 = sign(p, a, b)
    d2 = sign(p, b, c)
    d3 = sign(p, c, a)
    has_neg = (d1 < 0) or (d2 < 0) or (d3 < 0)
    has_pos = (d1 > 0) or (d2 > 0) or (d3 > 0)
    return not (has_neg and has_pos)


def triangulate(poly: Sequence[Point]) -> list[Tri]:
    """Trianguliert ein einfaches Polygon (ohne Selbstueberschneidung)."""
    pts = list(poly)
    n = len(pts)
    if n < 3:
        return []
    if _signed_area(pts) < 0:
        pts.reverse()

    indices = list(range(n))
    tris: list[Tri] = []
    guard = 0
    max_iter = n * n + 10

    while len(indices) > 3 and guard < max_iter:
        guard += 1
        clipped = False
        m = len(indices)
        for i in range(m):
            ia = indices[(i - 1) % m]
            ib = indices[i]
            ic = indices[(i + 1) % m]
            a, b, c = pts[ia], pts[ib], pts[ic]
            # konvex?
            cross = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])
            if cross <= 0:
                continue
            # enthaelt das Dreieck andere Punkte?
            contains = False
            for j in indices:
                if j in (ia, ib, ic):
                    continue
                if _point_in_triangle(pts[j], a, b, c):
                    contains = True
                    break
            if contains:
                continue
            tris.append((ia, ib, ic))
            indices.pop(i)
            clipped = True
            break
        if not clipped:
            # Notfall: fan-triangulierung, damit ein Mesh entsteht
            for i in range(1, len(indices) - 1):
                tris.append((indices[0], indices[i], indices[i + 1]))
            return tris

    if len(indices) == 3:
        tris.append((indices[0], indices[1], indices[2]))
    return tris


# ---------------------------------------------------------------------------
# Volumen
# ---------------------------------------------------------------------------

@dataclass
class Mesh:
    vertices: list[tuple[float, float, float]]
    faces: list[Tri]

    def translated(self, dx: float, dy: float, dz: float) -> "Mesh":
        return Mesh(
            [(x + dx, y + dy, z + dz) for x, y, z in self.vertices],
            list(self.faces),
        )

    def scaled(self, s: float) -> "Mesh":
        return Mesh([(x * s, y * s, z * s) for x, y, z in self.vertices], list(self.faces))

    def bounds(self) -> tuple[tuple[float, float, float], tuple[float, float, float]]:
        xs = [v[0] for v in self.vertices]
        ys = [v[1] for v in self.vertices]
        zs = [v[2] for v in self.vertices]
        return (
            (min(xs), min(ys), min(zs)),
            (max(xs), max(ys), max(zs)),
        )


def prism(poly: Sequence[Point], z0: float, z1: float) -> Mesh:
    """Erzeugt ein geschlossenes Prisma aus einem 2D-Polygon."""
    pts = list(poly)
    if len(pts) < 3:
        return Mesh([], [])
    tris = triangulate(pts)
    n = len(pts)
    verts: list[tuple[float, float, float]] = [(x, y, z0) for x, y in pts]
    verts += [(x, y, z1) for x, y in pts]

    faces: list[Tri] = []
    for a, b, c in tris:
        faces.append((c, b, a))          # Bodenflaeche nach -z
        faces.append((a + n, b + n, c + n))  # Deckflaeche nach +z
    for i in range(n):
        j = (i + 1) % n
        faces.append((i, j, j + n))
        faces.append((i, j + n, i + n))
    return Mesh(verts, faces)


def merge(meshes: Sequence[Mesh]) -> Mesh:
    verts: list[tuple[float, float, float]] = []
    faces: list[Tri] = []
    for m in meshes:
        off = len(verts)
        verts.extend(m.vertices)
        faces.extend((a + off, b + off, c + off) for a, b, c in m.faces)
    return Mesh(verts, faces)


# ---------------------------------------------------------------------------
# STL
# ---------------------------------------------------------------------------

def write_stl(path: str | Path, meshes: Sequence[Mesh], name: str = "knifeplanner") -> Path:
    """Binares STL schreiben."""
    p = Path(path)
    p.parent.mkdir(parents=True, exist_ok=True)
    out = bytearray()
    header = f"{name}".encode("utf-8")[:80]
    out += header + b" " * (80 - len(header))
    total = sum(len(m.faces) for m in meshes)
    out += struct.pack("<I", total)
    for m in meshes:
        verts = m.vertices
        for a, b, c in m.faces:
            p1, p2, p3 = verts[a], verts[b], verts[c]
            ux, uy, uz = p2[0] - p1[0], p2[1] - p1[1], p2[2] - p1[2]
            vx, vy, vz = p3[0] - p1[0], p3[1] - p1[1], p3[2] - p1[2]
            nx = uy * vz - uz * vy
            ny = uz * vx - ux * vz
            nz = ux * vy - uy * vx
            ln = math.sqrt(nx * nx + ny * ny + nz * nz) or 1.0
            nx, ny, nz = nx / ln, ny / ln, nz / ln
            out += struct.pack("<3f", nx, ny, nz)
            for v in (p1, p2, p3):
                out += struct.pack("<3f", v[0], v[1], v[2])
            out += struct.pack("<H", 0)
    p.write_bytes(bytes(out))
    return p
