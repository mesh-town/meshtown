"""Tests of the feed README text and of build_feed.py writing it to the feed root.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/openwrt/feed -t platforms/linux/build/openwrt/feed
"""

import re
import unittest
from pathlib import Path

from readme import render_readme
from test_build_feed import BASE, FIVE_LABELS, setup


class RenderReadme(unittest.TestCase):
    def test_carries_the_base_url_and_every_label(self):
        # when
        text = render_readme(BASE + "/", FIVE_LABELS)
        # then
        self.assertIn(f"wget -O- {BASE}/install.sh | sh", text)
        for label in FIVE_LABELS:
            self.assertIn(f"`{label}`", text)

    def test_has_no_unfilled_placeholders(self):
        # when
        text = render_readme(BASE, FIVE_LABELS)
        # then
        self.assertNotIn("{base}", text)
        self.assertNotIn("{code}", text)

    def test_manual_setup_matches_the_install_script(self):
        # when
        text = render_readme(BASE, FIVE_LABELS)
        # then
        self.assertIn(f'src/gz meshtown {BASE}/opkg/$ARCH', text)
        self.assertIn(f"{BASE}/apk/$ARCH/packages.adb", text)
        self.assertIn("/etc/apk/keys/meshtown.pem", text)

    def test_each_package_manager_has_its_own_section(self):
        # when
        text = render_readme(BASE, FIVE_LABELS)
        opkg_part, apk_part = text.split("## OpenWrt 25.12 (apk)")
        # then
        self.assertIn("## OpenWrt 24.10 (opkg)", opkg_part)
        self.assertIn("opkg upgrade meshtown", opkg_part)
        self.assertNotIn("apk upgrade", opkg_part)
        self.assertIn("apk upgrade meshtown", apk_part)
        self.assertNotIn("opkg ", apk_part)


class FeedRoot(unittest.TestCase):
    def test_build_writes_the_readme_next_to_the_index_page(self):
        # given
        feed = setup(self)
        # when
        done = feed.when_built("--only", "ipk", *feed.ephemeral())
        # then
        self.assertEqual(done.code, 0, done.err)
        text = (feed.out / "README.md").read_text()
        self.assertIn(BASE, text)
        for label in FIVE_LABELS:
            self.assertIn(label, text)


    def test_links_point_at_existing_paths(self):
        # given
        root = Path(__file__).parents[5]
        text = render_readme(BASE, FIVE_LABELS)
        # when
        paths = re.findall(r"/(?:tree|blob)/main/(\S+?)\)", text)
        # then
        self.assertTrue(paths)
        for rel in paths:
            self.assertTrue((root / rel).exists(), rel)
            self.assertFalse(rel.startswith("build/"), rel)


if __name__ == "__main__":
    unittest.main()
