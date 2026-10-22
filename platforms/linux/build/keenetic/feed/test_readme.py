"""Tests of readme.py: the README of the feed root.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/keenetic/feed -t platforms/linux/build/keenetic/feed
"""

import unittest

import readme

DIRS = ["aarch64-k3.10", "mipselsf-k3.4"]


class Rendered(unittest.TestCase):
    def test_has_both_install_paths_with_the_base_address(self):
        # given / when
        text = readme.render_readme("https://x.example.com/f/", DIRS)
        # then
        self.assertIn("wget -O- https://x.example.com/f/install.sh | sh", text)
        self.assertIn('echo "src/gz meshtown https://x.example.com/f/$ARCH"', text)
        self.assertIn("opkg update && opkg install meshtown", text)

    def test_says_what_is_needed_before_and_what_is_not_checked(self):
        # given / when
        text = readme.render_readme("https://x", DIRS)
        # then
        self.assertIn("Install Entware", text)
        self.assertIn("Not tested on a live KeeneticOS router", text)
        self.assertIn(":7777", text)
        self.assertIn("opkg remove meshtown", text)

    def test_lists_the_architectures_and_leaves_no_placeholder(self):
        # given / when
        text = readme.render_readme("https://x", DIRS)
        # then
        self.assertIn("`aarch64-k3.10`, `mipselsf-k3.4`", text)
        self.assertNotRegex(text, r"\{(code|help|base|dirs)\}")


if __name__ == "__main__":
    unittest.main()
