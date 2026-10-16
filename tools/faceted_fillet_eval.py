"""Independent SI binary-STL volume, closure, bounds and authored-phase contacts.

Numerical output measurements only: no native/grid certificate is inferred.
The eval uses a fixed 10 mm radius / 20 mm height authored XY cylinder.
"""
from __future__ import annotations
from collections import Counter
import math
from pathlib import Path
import struct


def triangles(data: bytes) -> list[list[tuple[float, float, float]]]:
    if len(data) < 84:
        raise ValueError("missing STL")
    count = struct.unpack_from("<I", data, 80)[0]
    if not 4 <= count <= 10000 or len(data) != 84 + 50 * count:
        raise ValueError("invalid bounded STL")
    faces = []
    for i in range(count):
        v = struct.unpack_from("<12fH", data, 84 + 50 * i)
        if not all(math.isfinite(x) for x in v[:12]):
            raise ValueError("nonfinite STL")
        faces.append([tuple(v[j:j+3]) for j in [3, 6, 9]])
    return faces


def physical_problems(data: bytes, maximum_m: float, volume_mm3: float) -> list[str]:
    try:
        faces = triangles(data)
        if len(faces) != 1088:
            return ["physical facet count differs from explicit example quality"]
        edges = Counter()
        volume = 0.0
        points = set()
        for a, b, c in faces:
            points.update([a, b, c])
            for p, q in [(a, b), (b, c), (c, a)]:
                edges[p, q] += 1
            volume += (a[0]*(b[1]*c[2]-b[2]*c[1]) + a[1]*(b[2]*c[0]-b[0]*c[2]) + a[2]*(b[0]*c[1]-b[1]*c[0])) / 6
        if any(n != 1 or edges[q, p] != 1 for (p, q), n in edges.items()):
            return ["actual STL is not a closed oriented facet shell"]
        if not math.isfinite(volume) or volume <= 0 or abs(volume*1e9-volume_mm3) > 0.002:
            return ["independent actual STL volume disagrees with native regeneration"]
        for axis, expected in enumerate([(-0.01, 0.01), (-0.01, 0.01), (0.0, 0.02)]):
            got = min(p[axis] for p in points), max(p[axis] for p in points)
            if any(abs(a-b) > 1e-8 for a, b in zip(got, expected)):
                return ["actual physical dimensions differ"]
        contacts = [(0.01-maximum_m, 0., 0.02), (-0.0095, 0., 0.02), (0.01, 0., 0.02-maximum_m), (-0.01, 0., 0.0195)]
        if any(min(math.dist(p, q) for p in points) > 1e-8 for q in contacts):
            return ["actual output violates authored positive-X cosine phase/contacts"]
        return []
    except (ValueError, struct.error, OverflowError):
        return ["actual bounded SI physical STL absent or malformed"]
