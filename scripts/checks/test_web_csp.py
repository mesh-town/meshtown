"""The node page under CSP `style-src 'self'; script-src 'self'` (`core/web_gui/routes/mod.rs`):
an inline `style=`, `<style>`, an inline `<script>` and `on…=` handlers are
blocked by the browser, and the Playwright `web-check` battery fails on a console error in every case. JS unit tests
do not see this — we catch it here, without a build or a browser."""

import re
import unittest
from pathlib import Path

WEB = Path(__file__).resolve().parents[2] / "core" / "web_gui" / "ui"

FORBIDDEN = [
    (re.compile(r"""\sstyle\s*=\s*["']"""), "style= attribute"),
    (re.compile(r"<style[\s>]"), "<style> element"),
    (re.compile(r"<script(?![^>]*\ssrc=)[^>]*>"), "inline <script>"),
    (re.compile(r"""\son[a-z]+\s*=\s*["']"""), "on…= handler"),
]
VUE_FORBIDDEN = [
    (pat, what) for pat, what in FORBIDDEN if what != "inline <script>"
] + [
    (re.compile(r"""(?:\s:|\sv-bind:)style\s*="""), ":style binding"),
    (re.compile(r"\sv-html\b"), "v-html directive"),
]


def violations(text, forbidden=FORBIDDEN):
    found = []
    for lineno, line in enumerate(text.splitlines(), 1):
        if line.lstrip().startswith(("//", "/*", "*")):
            continue
        for pattern, what in forbidden:
            if pattern.search(line):
                found.append(f"{lineno}: {what}: {line.strip()[:80]}")
    return found


class WebCspTest(unittest.TestCase):
    def test_index_html_has_no_inline_code(self):
        self.assertEqual(violations((WEB / "index.html").read_text(encoding="utf-8")), [])

    def test_vue_templates_have_no_inline_code(self):
        templates = sorted((WEB / "vue" / "src").rglob("*.vue"))
        self.assertTrue(templates)
        for path in templates:
            with self.subTest(file=path.relative_to(WEB).as_posix()):
                self.assertEqual(violations(path.read_text(encoding="utf-8"), VUE_FORBIDDEN), [])

    def test_vue_script_setup_is_not_inline_script(self):
        self.assertEqual(violations("<script setup>", VUE_FORBIDDEN), [])
        self.assertTrue(violations("<script setup>", FORBIDDEN))

    def test_detects_v_html(self):
        self.assertTrue(violations('<p v-html="raw"></p>', VUE_FORBIDDEN))

    def test_detects_bound_style(self):
        self.assertTrue(violations('<div :style="{color: c}">', VUE_FORBIDDEN))
        self.assertTrue(violations('<div v-bind:style="s">', VUE_FORBIDDEN))

    def test_detects_style_element_and_static_style_in_vue(self):
        self.assertTrue(violations("<style scoped>", VUE_FORBIDDEN))
        self.assertTrue(violations('<div style="margin:0">', VUE_FORBIDDEN))

    def test_detects_inline_style(self):
        self.assertTrue(violations('<div hidden style="margin:0">'))


if __name__ == "__main__":
    unittest.main()
