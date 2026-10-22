#!/usr/bin/env python3
"""Generator of the source bundle icon, no external dependencies — stdlib only (`zlib` for
IDAT compression, `struct` for PNG chunk headers, `binascii.crc32` for checksums). Writes a
1024x1024 RGBA PNG on a transparent background; `build/macos/app.sh` grows an .iconset from it with
`sips` and assembles `iconutil -c icns`.

By default it draws the LOGO: a dark rounded plate on the macOS icon grid (824 of 1024, margin 100),
a ring of 8 sectors with gaps and flat radial cuts, clockwise from 12 o'clock — blue (the color of the
exit sector in the tray), purple, five light grey, and green as the last one (just left of 12 o'clock) —
and a white house (gable roof with eaves, plain wall) in the center; outside the plate it is transparent. The same numbers are also written as `build/logo.svg`.
With `R G B` arguments it draws the old solid circle of that color (no SVG). The logo is static
(Finder/Info), while the tray icon is recolored at runtime by network status.

Run: `python3 build/macos/generate-icon.py [output .png] [R G B] [--svg path.svg]`.
"""

import math
import struct
import sys
import zlib

SIZE = 1024
LOGO_GREEN = (60, 200, 90)
LOGO_GREY = (190, 190, 190)  # light grey: the quiet sectors, so the colored ones stand out
LOGO_BLUE = (90, 170, 255)
LOGO_PURPLE = (175, 75, 205)  # purple, as for exit clients in the tray
SECTOR_COLORS = [LOGO_BLUE, LOGO_PURPLE] + [LOGO_GREY] * 5 + [LOGO_GREEN]
OUTER_K = 0.32  # outer radius of the ring, fraction of the size
THICK_K = 0.11  # ring thickness, fraction of the size
GAP_FRACTION = 0.02  # gap width between sectors — fraction of the midline circumference; sides are parallel
TWO_PI = 2 * math.pi
CIRCLE_K = 0.46  # radius of the solid circle (R G B mode), fraction of the size
PLATE_RGB = (28, 28, 30)
PLATE_MARGIN, PLATE_SIDE, PLATE_RADIUS = 100, 824, 185  # of 1024 (macOS icon grid)
HOUSE_RGB = (255, 255, 255)
ROOF_W_K, ROOF_H_K = 0.20, 0.08  # roof width (with eaves) and height, fraction of the size
WALL_W_K, WALL_H_K = 0.15, 0.095  # wall width and height, fraction of the size
Layer = tuple[tuple[int, int, int], float]  # color, coverage 0..1


def _chunk(tag: bytes, data: bytes) -> bytes:
    return (
        struct.pack(">I", len(data))
        + tag
        + data
        + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
    )


def _ramp(value: float) -> float:
    return min(1.0, max(0.0, value))


def _plate_cover(px: float, py: float, size: int) -> float:
    k = size / 1024.0
    half = PLATE_SIDE * k / 2.0
    radius = PLATE_RADIUS * k
    qx, qy = abs(px - size / 2.0) - half + radius, abs(py - size / 2.0) - half + radius
    dist = math.hypot(max(qx, 0.0), max(qy, 0.0)) + min(max(qx, qy), 0.0) - radius
    return _ramp(0.5 - dist)


def gap_half_width(size: int) -> float:
    """Half the gap width in px — the same at every radius, so the gap has parallel sides."""
    return GAP_FRACTION * TWO_PI * size * (OUTER_K - THICK_K / 2) / 2


def _ring_layer(px: float, py: float, size: int) -> Layer:
    r_out = size * OUTER_K
    r_in = r_out - size * THICK_K
    dx, dy = px - size / 2.0, py - size / 2.0
    dist = math.hypot(dx, dy)
    radial = _ramp(r_out - dist + 0.5) * _ramp(dist - r_in + 0.5)  # 1 px edge, as for the cuts and the house
    ang = math.atan2(dx, -dy) % TWO_PI
    step = TWO_PI / len(SECTOR_COLORS)
    index = min(int(ang / step), len(SECTOR_COLORS) - 1)
    local = ang - index * step
    # px to the nearest cut: perpendicular distance to the boundary ray minus half the gap width
    edge = dist * math.sin(min(local, step - local)) - gap_half_width(size)
    return SECTOR_COLORS[index], radial * _ramp(edge + 0.5)


def house_shapes(size: int) -> tuple[list[tuple[float, float]], tuple[float, float, float, float]]:
    """Roof triangle (3 points) and wall rectangle (x0, y0, x1, y1) in pixels, house centered vertically."""
    c = size / 2.0
    roof_h, wall_h = size * ROOF_H_K, size * WALL_H_K
    top = c - (roof_h + wall_h) / 2.0
    half_roof, half_wall = size * ROOF_W_K / 2.0, size * WALL_W_K / 2.0
    base = top + roof_h
    roof = [(c, top), (c + half_roof, base), (c - half_roof, base)]
    return roof, (c - half_wall, base, c + half_wall, base + wall_h)


def _triangle_cover(px: float, py: float, tri: list[tuple[float, float]]) -> float:
    """Coverage of a convex triangle (clockwise on screen): distance to the nearest edge line."""
    inside = 1.0
    for (ax, ay), (bx, by) in zip(tri, tri[1:] + tri[:1]):
        ex, ey = bx - ax, by - ay
        inside = min(inside, _ramp((ex * (py - ay) - ey * (px - ax)) / math.hypot(ex, ey) + 0.5))
    return inside


def _rect_cover(px: float, py: float, rect: tuple[float, float, float, float]) -> float:
    x0, y0, x1, y1 = rect
    return _ramp(min(px - x0, x1 - px, py - y0, y1 - py) + 0.5)


def _house_layer(px: float, py: float, shapes) -> Layer:
    roof, wall = shapes
    return HOUSE_RGB, max(_triangle_cover(px, py, roof), _rect_cover(px, py, wall))


def _over(bottom: tuple[tuple[float, ...], float], top: Layer) -> tuple[tuple[float, ...], float]:
    (b_rgb, b_a), (t_rgb, t_a) = bottom, top
    alpha = t_a + b_a * (1.0 - t_a)
    if alpha == 0.0:
        return b_rgb, 0.0
    return tuple((t * t_a + b * b_a * (1.0 - t_a)) / alpha for t, b in zip(t_rgb, b_rgb)), alpha


def _pixel(x: int, y: int, size: int, shapes) -> bytes:
    px, py = x + 0.5, y + 0.5
    plate = _plate_cover(px, py, size)
    if plate == 0.0:
        return b"\0\0\0\0"
    out = _over((PLATE_RGB, plate), _ring_layer(px, py, size))
    rgb, alpha = _over(out, _house_layer(px, py, shapes))
    return bytes(round(v) for v in rgb) + bytes((round(255 * alpha),))


def render_logo_rgba(size: int) -> bytes:
    """RGBA rows of the PNG (with filter bytes 0) — the logo on a dark plate, transparent outside."""
    shapes = house_shapes(size)
    margin = int(PLATE_MARGIN * size / 1024.0) - 1
    empty = bytes(size * 4)
    rows = bytearray()
    for y in range(size):
        rows.append(0)
        if y < margin or y >= size - margin:
            rows.extend(empty)
            continue
        row = bytearray(empty)
        for x in range(margin, size - margin):
            row[x * 4 : x * 4 + 4] = _pixel(x, y, size, shapes)
        rows.extend(row)
    return bytes(rows)


def _pt(c: float, r: float, ang: float) -> str:
    return f"{c + r * math.sin(ang):.3f},{c - r * math.cos(ang):.3f}"


def _sector_path(c: float, r_out: float, r_in: float, a0: float, a1: float, half: float) -> str:
    """A ring sector between the rays a0 and a1, each side shifted by `half` px off its ray (parallel to it)."""
    d_out, d_in = math.asin(half / r_out), math.asin(half / r_in)
    return (
        f"M{_pt(c, r_out, a0 + d_out)} A{r_out:.3f},{r_out:.3f} 0 0 1 {_pt(c, r_out, a1 - d_out)} "
        f"L{_pt(c, r_in, a1 - d_in)} A{r_in:.3f},{r_in:.3f} 0 0 0 {_pt(c, r_in, a0 + d_in)} Z"
    )


def _hex(rgb: tuple[int, int, int]) -> str:
    return "#%02x%02x%02x" % rgb


def render_logo_svg(size: int = SIZE) -> str:
    """SVG of the same logo from the same constants (viewBox 0 0 size size)."""
    c = size / 2.0
    k = size / 1024.0
    r_out = size * OUTER_K
    r_in = r_out - size * THICK_K
    step, half = TWO_PI / len(SECTOR_COLORS), gap_half_width(size)
    roof, (x0, y0, x1, y1) = house_shapes(size)
    lines = [
        f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 {size} {size}" width="{size}" height="{size}">',
        f'  <rect x="{PLATE_MARGIN * k:g}" y="{PLATE_MARGIN * k:g}" width="{PLATE_SIDE * k:g}" '
        f'height="{PLATE_SIDE * k:g}" rx="{PLATE_RADIUS * k:g}" fill="{_hex(PLATE_RGB)}"/>',
    ]
    for i, rgb in enumerate(SECTOR_COLORS):
        d = _sector_path(c, r_out, r_in, i * step, (i + 1) * step, half)
        lines.append(f'  <path d="{d}" fill="{_hex(rgb)}"/>')
    pts = " ".join(f"{x:.3f},{y:.3f}" for x, y in roof)
    lines.append(f'  <polygon points="{pts}" fill="{_hex(HOUSE_RGB)}"/>')
    lines.append(
        f'  <rect x="{x0:.3f}" y="{y0:.3f}" width="{x1 - x0:.3f}" height="{y1 - y0:.3f}" fill="{_hex(HOUSE_RGB)}"/>'
    )
    lines.append("</svg>")
    return "\n".join(lines) + "\n"


def render_circle_rgba(size: int, rgb: tuple[int, int, int]) -> bytes:
    """RGBA rows of the PNG (with filter bytes 0) — a solid circle `rgb`, edge smoothed over 2 px."""
    c = (size - 1) / 2.0
    radius = size * CIRCLE_K
    px = bytes(rgb)
    empty = b"\0" + bytes(size * 4)
    rows = bytearray()
    for y in range(size):
        if abs(y - c) > radius + 1.0:
            rows.extend(empty)
            continue
        row = bytearray(size * 4)
        for x in range(size):
            alpha = round(255 * _ramp((radius + 1.0 - math.hypot(x - c, y - c)) / 2.0))
            row[x * 4 : x * 4 + 4] = px + bytes((alpha,))
        rows.append(0)
        rows.extend(row)
    return bytes(rows)


def write_png(path: str, size: int, rgb: tuple[int, int, int] | None) -> None:
    raw = render_logo_rgba(size) if rgb is None else render_circle_rgba(size, rgb)
    ihdr = struct.pack(">IIBBBBB", size, size, 8, 6, 0, 0, 0)  # 8 bits, RGBA (colortype 6)
    idat = zlib.compress(raw, 9)
    png = b"\x89PNG\r\n\x1a\n" + _chunk(b"IHDR", ihdr) + _chunk(b"IDAT", idat) + _chunk(b"IEND", b"")
    with open(path, "wb") as f:
        f.write(png)


def main(argv: list[str]) -> int:
    args = argv[1:]
    svg_path = "build/logo.svg"
    if "--svg" in args:
        at = args.index("--svg")
        svg_path = args[at + 1]
        del args[at : at + 2]
    out_path = args[0] if args else "build/macos/icon.png"
    rgb = tuple(int(v) for v in args[1:4]) if len(args) > 3 else None
    write_png(out_path, SIZE, rgb)
    print(f"icon: {out_path} ({SIZE}x{SIZE}, {'logo' if rgb is None else f'circle rgb={rgb}'})")
    if rgb is None:
        with open(svg_path, "w") as f:
            f.write(render_logo_svg())
        print(f"svg: {svg_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main(sys.argv))
