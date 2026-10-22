"""Tests of the icon generator (stdlib). Run from the tree root:
`python3 -I -m unittest discover -s platforms/macos/build -p 'test_generate_icon.py' -v`."""

import importlib.util
import math
import os
import struct
import tempfile
import unittest
import xml.etree.ElementTree as ET
import zlib

HERE = os.path.dirname(os.path.abspath(__file__))
spec = importlib.util.spec_from_file_location("gen", os.path.join(HERE, "generate-icon.py"))
gen = importlib.util.module_from_spec(spec)
spec.loader.exec_module(gen)

N = 1024
C = (N - 1) / 2.0
R_MID = N * gen.OUTER_K - N * gen.THICK_K / 2


def decode(path):
    with open(path, "rb") as f:
        data = f.read()
    w, h = struct.unpack(">II", data[16:24])
    pos, idat = 8, b""
    while pos < len(data):
        (ln,) = struct.unpack(">I", data[pos : pos + 4])
        if data[pos + 4 : pos + 8] == b"IDAT":
            idat += data[pos + 8 : pos + 8 + ln]
        pos += 12 + ln
    raw = zlib.decompress(idat)
    stride = w * 4 + 1
    return w, h, [raw[y * stride + 1 : (y + 1) * stride] for y in range(h)]


def at(rows, x, y):
    return tuple(rows[y][x * 4 : x * 4 + 4])


def mid_point(deg):
    a = math.radians(deg)
    return round(C + R_MID * math.sin(a)), round(C - R_MID * math.cos(a))


def off_ray(deg, r, d):
    """A pixel at distance r along the ray at `deg` (clockwise from 12 o'clock), shifted d px clockwise off it."""
    a = math.radians(deg)
    return round(C + r * math.sin(a) + d * math.cos(a)), round(C - r * math.cos(a) + d * math.sin(a))


class LogoTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.tmp = tempfile.mkdtemp()
        cls.logo = os.path.join(cls.tmp, "logo.png")
        cls.circle = os.path.join(cls.tmp, "circle.png")
        gen.write_png(cls.logo, N, None)
        gen.write_png(cls.circle, N, (10, 20, 30))
        cls.w, cls.h, cls.rows = decode(cls.logo)

    def test_size(self):
        self.assertEqual((self.w, self.h), (1024, 1024))

    def test_sector_colors(self):
        for k, want in enumerate(gen.SECTOR_COLORS):
            x, y = mid_point(k * 45 + 22.5)
            px = at(self.rows, x, y)
            self.assertEqual(px[3], 255, k)
            for got, exp in zip(px, want):
                self.assertAlmostEqual(got, exp, delta=3, msg=f"sector {k}")

    def test_expected_palette(self):
        self.assertEqual(gen.SECTOR_COLORS[0], (90, 170, 255))
        self.assertEqual(gen.SECTOR_COLORS[1], (175, 75, 205))
        self.assertEqual(gen.SECTOR_COLORS[2:7], [(190, 190, 190)] * 5)
        self.assertEqual(gen.SECTOR_COLORS[7], (60, 200, 90))

    def test_house_center_white(self):
        self.assertEqual(at(self.rows, 512, 512), (255, 255, 255, 255))

    def test_plate_between_ring_and_edge(self):
        x = round(C + N * (gen.OUTER_K + 0.06))  # outside the ring, inside the plate (edge at 0.4 N from center)
        self.assertEqual(at(self.rows, x, 512), (28, 28, 30, 255))

    def test_corner_transparent(self):
        self.assertEqual(at(self.rows, 0, 0)[3], 0)
        self.assertEqual(at(self.rows, 120, 120)[3], 0)  # inside the plate square, cut by the corner radius

    def test_gap_shows_plate(self):
        x, y = mid_point(180)  # the cut between sectors 3 and 4 is at 180 degrees
        self.assertEqual(at(self.rows, x, y), (28, 28, 30, 255))

    def test_gap_sides_parallel(self):
        # Across the cut at 45 degrees: the same offset from the ray is plate near the inner edge and sector near
        # the outer one only for a wedge; a parallel gap is plate at half-2 px and sector at half+2 px on both edges.
        half = gen.gap_half_width(N)
        r_out = N * gen.OUTER_K
        r_in = r_out - N * gen.THICK_K
        for r in (r_in + 4, r_out - 4):
            self.assertEqual(at(self.rows, *off_ray(45, r, half - 2)), (28, 28, 30, 255), r)
            self.assertEqual(at(self.rows, *off_ray(45, r, half + 2))[:3], gen.SECTOR_COLORS[1], r)

    def test_svg_parses_with_all_colors(self):
        root = ET.fromstring(gen.render_logo_svg())
        self.assertEqual(root.get("viewBox"), "0 0 1024 1024")
        fills = {el.get("fill") for el in root.iter() if el.get("fill")}
        for rgb in (gen.LOGO_GREEN, gen.LOGO_GREY, gen.LOGO_BLUE, gen.LOGO_PURPLE, gen.HOUSE_RGB, gen.PLATE_RGB):
            self.assertIn("#%02x%02x%02x" % rgb, fills)

    def test_circle_mode(self):
        _, _, rows = decode(self.circle)
        self.assertEqual(at(rows, 512, 512), (10, 20, 30, 255))
        self.assertEqual(at(rows, 0, 0)[3], 0)

    def test_default_paths_are_beside_the_script(self):
        for path in gen.default_paths():
            self.assertEqual(os.path.dirname(path), HERE)
            self.assertTrue(os.path.isfile(path), path)


if __name__ == "__main__":
    unittest.main()
