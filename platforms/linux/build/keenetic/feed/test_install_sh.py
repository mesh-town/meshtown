"""Tests of install.sh as rendered by build_feed.render_install, run against a fake Entware router.

Paths under /opt are redirected into a temporary root; wget and opkg are small stubs that log their calls and
imitate what the real tools do to the files.

Run from the tree root: python3 -m unittest discover -s build/keenetic/feed -t build/keenetic/feed
"""

import os
import shutil
import subprocess
import unittest
from collections import namedtuple
from pathlib import Path

import build_feed
from feed_fixtures import tmpdir

BASE = "http://feed.test"
ARCH = "mipselsf-k3.4"
Result = namedtuple("Result", "code out err")

WGET = """\
url=""
for a in "$@"; do case "$a" in -*) ;; *) url="$a" ;; esac; done
out=""
while [ $# -gt 0 ]; do case "$1" in -O|-qO) out="$2"; shift ;; esac; shift; done
echo "wget $url" >> "$LOG"
case "$url" in https:*) [ -e "$ROOT/ssl-ready" ] || { echo "wget: not an http URL" >&2; exit 1; } ;; esac
src="$FEED_ROOT/$(printf '%s' "$url" | sed 's#^[a-z]*://[^/]*/##')"
[ -f "$src" ] || { echo "wget: 404" >&2; exit 1; }
cp "$src" "$out"
"""
OPKG = """\
echo "opkg $*" >> "$LOG"
case "$1" in
  list-installed) [ -e "$ROOT/installed" ] && echo "meshtown - 0.3.30-r1" ;;
  install) case "$*" in *wget-ssl*) touch "$ROOT/ssl-ready" ;; *meshtown) touch "$ROOT/installed" ;; esac ;;
esac
exit 0
"""


class Router:
    def __init__(self, tmp):
        self.root, self.feed, self.stubs = tmp / "root", tmp / "feed", tmp / "stubs"
        self.log = tmp / "log"
        self.script = tmp / "install.sh"
        self.log.write_text("")

    def read(self, rel):
        return (self.root / rel).read_text()

    def calls(self):
        return self.log.read_text().splitlines()

    def when_install(self, base=BASE, env=None):
        environ = {**os.environ, "PATH": f"{self.stubs}:{os.environ['PATH']}", "LOG": str(self.log),
                   "ROOT": str(self.root), "FEED_ROOT": str(self.feed), **(env or {})}
        done = subprocess.run(["sh", str(self.script)], capture_output=True, text=True, env=environ,
                              stdin=subprocess.DEVNULL, timeout=60)
        return Result(done.returncode, done.stdout, done.stderr)


def write(path, text, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(mode)


def setup(test, with_entware=True, arch_dir=ARCH, with_index=True, conf_url=None, base=BASE):
    tmp = tmpdir(test)
    router = Router(tmp)
    if with_entware:
        write(router.root / "opt/bin/opkg", "#!/bin/sh\n" + OPKG, 0o755)
    url = conf_url or f"http://bin.entware.net/{arch_dir}"
    write(router.root / "opt/etc/opkg.conf", f"src/gz entware {url}\ndest root /\narch all 100\n")
    if with_index:
        write(router.feed / arch_dir / "Packages.gz", "index\n")
    write(router.stubs / "wget", "#!/bin/sh\n" + WGET, 0o755)
    write(router.stubs / "opkg", "#!/bin/sh\n" + OPKG, 0o755)
    script = build_feed.render_install(base).replace("/opt/", f"{router.root}/opt/")
    write(router.script, script)
    return router


class Refusals(unittest.TestCase):
    def test_without_entware_it_says_so_and_exits_1(self):
        # given
        router = setup(self, with_entware=False)
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 1)
        self.assertIn("install Entware first", result.err)
        self.assertEqual(router.calls(), [])

    def test_architecture_missing_from_the_feed_exits_2_and_changes_nothing(self):
        # given: the router is a kind the feed does not carry
        router = setup(self, arch_dir="armv7sf-k3.2", with_index=False)
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 2)
        self.assertIn("no packages for armv7sf-k3.2", result.err)
        self.assertFalse((router.root / "opt/etc/opkg/meshtown.conf").exists())
        self.assertFalse(any(c.startswith("opkg") for c in router.calls()))


class Install(unittest.TestCase):
    def test_writes_the_feed_line_then_updates_and_installs(self):
        # given
        router = setup(self)
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(router.read("opt/etc/opkg/meshtown.conf"), f"src/gz meshtown {BASE}/{ARCH}\n")
        self.assertEqual([c for c in router.calls() if c.startswith("opkg")],
                         ["opkg update", "opkg list-installed meshtown", "opkg install meshtown"])

    def test_a_second_run_upgrades_and_keeps_one_feed_line(self):
        # given
        router = setup(self)
        router.when_install()
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertIn("opkg upgrade meshtown", router.calls())
        self.assertEqual(router.read("opt/etc/opkg/meshtown.conf").count("src/gz"), 1)

    def test_the_address_can_be_overridden_by_the_environment(self):
        # given
        router = setup(self, base="http://elsewhere.test")
        # when
        result = router.when_install(env={"MESHTOWN_FEED": BASE})
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertIn(f"{BASE}/{ARCH}", router.read("opt/etc/opkg/meshtown.conf"))

    def test_architecture_is_read_from_an_address_with_a_trailing_slash(self):
        # given
        router = setup(self, conf_url=f"http://bin.entware.net/{ARCH}/")
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)


class Https(unittest.TestCase):
    def test_a_wget_without_ssl_is_replaced_before_the_install(self):
        # given: the stub wget refuses https until wget-ssl is installed
        router = setup(self, base="https://feed.test")
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        calls = router.calls()
        self.assertIn("opkg install ca-certificates wget-ssl", calls)
        self.assertIn("opkg remove wget-nossl", calls)
        self.assertLess(calls.index("opkg install ca-certificates wget-ssl"), calls.index("opkg install meshtown"))

    def test_plain_http_never_touches_wget_packages(self):
        # given
        router = setup(self)
        # when
        router.when_install()
        # then
        self.assertFalse(any("wget-ssl" in c for c in router.calls()))

    def test_script_is_valid_posix_shell(self):
        # given
        router = setup(self)
        # when / then
        for shell in filter(None, ("sh", shutil.which("dash"))):
            with self.subTest(shell=shell):
                done = subprocess.run([shell, "-n", str(router.script)], capture_output=True, text=True)
                self.assertEqual(done.returncode, 0, done.stderr)


if __name__ == "__main__":
    unittest.main()
