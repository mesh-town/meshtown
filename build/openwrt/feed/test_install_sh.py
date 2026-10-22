"""Tests of install.sh as rendered by build_feed.render_install (no external tools).

Besides reading the text, the script is run against a fake router: the paths under /etc are
redirected into a temporary root, and wget, apk, opkg and opkg-key are small stubs that
log their calls and imitate what the real package managers do to the files.

Run from the tree root: python3 -m unittest discover -s build/openwrt/feed -t build/openwrt/feed
"""

import os
import shutil
import subprocess
import unittest
from pathlib import Path

import build_feed
from feed_fixtures import FEED_DIR, tmpdir

BASE = "http://feed.test"
ARCH = "aarch64_generic"
PACKAGE_INIT = '#!/bin/sh\necho "init $1" >> "$LOG"\n'
MANUAL_SYSCTL = "net.core.rmem_max=1\n"
MANUAL_INIT = "#!/bin/sh\n# edited by hand\n"


class RenderedText(unittest.TestCase):
    def test_no_placeholder_is_left(self):
        # given / when
        text = build_feed.render_install("https://x.example.com/f")
        # then
        self.assertNotIn("@BASE@", text)

    def test_the_template_has_a_placeholder_and_each_one_is_replaced_by_the_base(self):
        # given
        template = (FEED_DIR / "install.sh.in").read_text()
        # when
        text = build_feed.render_install("https://x.example.com/f")
        # then
        self.assertGreaterEqual(template.count("@BASE@"), 1)
        self.assertGreaterEqual(text.count("https://x.example.com/f"), template.count("@BASE@"))

    def test_characters_special_to_substitution_survive_in_the_base(self):
        # given
        base = "https://x.example.com/f?a=1&b=2|c\\d"
        # when
        text = build_feed.render_install(base)
        # then
        self.assertIn(base, text)

    def test_script_is_valid_posix_shell(self):
        # given
        script = tmpdir(self) / "install.sh"
        script.write_text(build_feed.render_install(BASE))
        # when / then
        for shell in filter(None, ("sh", shutil.which("dash"))):
            with self.subTest(shell=shell):
                done = subprocess.run([shell, "-n", str(script)], capture_output=True, text=True)
                self.assertEqual(done.returncode, 0, done.stderr)

    def test_names_the_file_apk_leaves_next_to_the_protected_ones(self):
        # given / when
        text = build_feed.render_install(BASE)
        # then
        self.assertIn("/etc/init.d/meshtown.apk-new", text)
        self.assertNotIn("sysctl", text)


class ApkRouter(unittest.TestCase):
    def test_adds_the_key_and_the_repository_then_installs(self):
        # given
        router = setup(self, "apk")
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual([k.read_text() for k in (router.root / "etc/apk/keys").iterdir()], ["apk-public-key\n"])
        self.assertIn(f"{BASE}/apk/{ARCH}/packages.adb", router.read("etc/apk/repositories.d/customfeeds.list"))
        self.assertIn("apk add meshtown", router.calls())

    def test_files_apk_set_aside_are_moved_over_the_hand_made_ones_and_the_service_restarts(self):
        # given: a manual install whose init script was edited; apk leaves its own as .apk-new
        router = setup(self, "apk", manual_install=True)
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(router.read("etc/init.d/meshtown"), PACKAGE_INIT)
        self.assertEqual(router.read("etc/sysctl.d/90-meshtown.conf"), MANUAL_SYSCTL)
        self.assertEqual(router.leftovers("apk-new"), [])
        self.assertEqual(router.read("etc/meshtown/config.json"), '{"keep": "me"}\n')
        self.assertTrue(router.restarted(), "the service must be restarted to run the new binary")


class SecondRun(unittest.TestCase):
    def test_keeps_a_single_repository_line(self):
        for manager, config in (("apk", "etc/apk/repositories.d/customfeeds.list"), ("opkg", "etc/opkg/customfeeds.conf")):
            with self.subTest(manager=manager):
                # given
                router = setup(self, manager)
                router.when_install()
                # when
                router.when_install()
                # then
                # The probe script drops old lines containing "meshtown", which does not match a base like
                # https://example.github.io/feed
                lines = [line for line in router.read(config).splitlines() if BASE in line]
                self.assertEqual(len(lines), 1, router.read(config))


class OpkgRouter(unittest.TestCase):
    def test_trusts_the_key_adds_the_feed_then_installs(self):
        # given
        router = setup(self, "opkg")
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(router.read("key-added.pub"), "usign-public-key\n")
        self.assertIn(f"src/gz meshtown {BASE}/opkg/{ARCH}", router.read("etc/opkg/customfeeds.conf"))
        self.assertIn("opkg update", router.calls())
        self.assertTrue(any(c.startswith("opkg install") and c.endswith("meshtown") for c in router.calls()))

    def test_a_clean_router_is_not_given_force_overwrite(self):
        # given
        router = setup(self, "opkg")
        # when
        router.when_install()
        # then
        # --force-overwrite is used only when a hand-made /usr/bin/meshtown is there
        self.assertFalse(any("--force-overwrite" in c for c in router.calls()))

    def test_a_hand_made_install_is_replaced_with_force_overwrite(self):
        # given: opkg refuses a file that no package owns, unless forced
        router = setup(self, "opkg", manual_install=True)
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 0, result.err)
        self.assertEqual(router.read("etc/init.d/meshtown"), PACKAGE_INIT)
        self.assertEqual(router.read("etc/meshtown/config.json"), '{"keep": "me"}\n')


class FeedAddress(unittest.TestCase):
    def test_the_rendered_base_is_used_by_default(self):
        # given
        router = setup(self, "apk")
        # when
        router.when_install()
        # then
        self.assertTrue(router.fetched() and all(u.startswith(BASE + "/") for u in router.fetched()))

    def test_environment_variable_replaces_the_base_everywhere(self):
        # given
        router = setup(self, "apk")
        # when
        router.when_install(env={"MESHTOWN_FEED": "http://mirror.test"})
        # then
        self.assertTrue(router.fetched() and all(u.startswith("http://mirror.test/") for u in router.fetched()))
        self.assertIn("http://mirror.test/apk/" + ARCH, router.read("etc/apk/repositories.d/customfeeds.list"))
        self.assertNotIn(BASE, router.read("etc/apk/repositories.d/customfeeds.list"))


class FeedWithoutTheRoutersLabel(unittest.TestCase):
    def test_exits_with_2_and_names_the_label(self):
        for manager in ("apk", "opkg"):
            with self.subTest(manager=manager):
                # given
                router = setup(self, manager, with_index=False)
                # when
                result = router.when_install()
                # then
                self.assertEqual(result.code, 2, result.err)
                self.assertIn(ARCH, result.out + result.err)

    def test_changes_nothing_on_the_router(self):
        for manager in ("apk", "opkg"):
            with self.subTest(manager=manager):
                # given
                router = setup(self, manager, with_index=False, manual_install=True)
                before = router.snapshot()
                # when
                router.when_install()
                # then
                self.assertEqual(router.snapshot(), before)
                self.assertEqual([c for c in router.calls() if not c.startswith("wget")], [])


class NotAnOpenwrtSystem(unittest.TestCase):
    def test_stops_with_an_error_before_touching_anything(self):
        # given
        router = setup(self, "apk")
        (router.root / "etc/openwrt_release").unlink()
        # when
        result = router.when_install()
        # then
        self.assertEqual(result.code, 1, result.err)
        self.assertEqual(router.calls(), [])


# ---------- setup: a fake router ----------

class Result:
    def __init__(self, done):
        self.code, self.out, self.err = done.returncode, done.stdout, done.stderr


class Router:
    def __init__(self, tmp, manager):
        self.tmp, self.manager = tmp, manager
        self.root, self.stubs, self.feed, self.payload = tmp / "root", tmp / "stubs", tmp / "feed", tmp / "payload"
        self.log = tmp / "log"
        self.log.write_text("")

    def when_install(self, env=None):
        shell = shutil.which("dash") or "sh"
        environ = {"PATH": f"{self.stubs}:/usr/bin:/bin", "LOG": str(self.log), "ROOT": str(self.root),
                   "FEED_ROOT": str(self.feed), "PAYLOAD": str(self.payload), "HOME": str(self.tmp),
                   "TMPDIR": str(self.tmp), "REAL_SED": shutil.which("sed"), **(env or {})}
        done = subprocess.run([shell, str(self.tmp / "install.sh")], capture_output=True, text=True,
                              env=environ, stdin=subprocess.DEVNULL, timeout=60)
        return Result(done)

    def read(self, relative):
        return (self.root / relative).read_text()

    def calls(self):
        return [line for line in self.log.read_text().splitlines() if line]

    def fetched(self):
        return [c.split(" ", 1)[1] for c in self.calls() if c.startswith("wget ")]

    def restarted(self):
        return any(c in ("init restart", "init start") for c in self.calls())

    def leftovers(self, suffix):
        return sorted(str(p) for p in (self.root / "etc").rglob(f"*.{suffix}"))

    def snapshot(self):
        return {str(p.relative_to(self.root)): p.read_bytes() for p in self.root.rglob("*") if p.is_file()}


def setup(test, manager, with_index=True, manual_install=False):
    """A fake router running `manager` (apk or opkg) with install.sh rewritten to live under its root."""
    tmp = tmpdir(test)
    router = Router(tmp, manager)
    write(router.root / "etc/openwrt_release", f"DISTRIB_ID='OpenWrt'\nDISTRIB_ARCH='{ARCH}'\n")
    write(router.root / "etc/apk/repositories.d/distfeeds.list", "https://downloads.example.org/packages.adb\n")
    write(router.root / "etc/opkg/distfeeds.conf", "src/gz base https://downloads.example.org/base\n")
    write(router.feed / "meshtown-apk.pem", "apk-public-key\n")
    write(router.feed / "meshtown-usign.pub", "usign-public-key\n")
    for name in (INDEX_FILES[manager] if with_index else []):
        write(router.feed / name.format(arch=ARCH), "index\n")
    write(router.payload / "binary", "new binary\n", mode=0o755)
    write(router.payload / "init.d", PACKAGE_INIT, mode=0o755)
    if manual_install:
        write(router.root / "usr/bin/meshtown", "old binary\n", mode=0o755)
        write(router.root / "etc/init.d/meshtown", MANUAL_INIT, mode=0o755)
        write(router.root / "etc/sysctl.d/90-meshtown.conf", MANUAL_SYSCTL)
        write(router.root / "etc/meshtown/config.json", '{"keep": "me"}\n')
    for name, body in stubs_for(manager).items():
        write(router.stubs / name, "#!/bin/sh\n" + body, mode=0o755)
    script = build_feed.render_install(BASE).replace("/etc/", f"{router.root}/etc/")
    script = script.replace("/usr/bin/meshtown", f"{router.root}/usr/bin/meshtown")
    write(tmp / "install.sh", script)
    return router


def write(path, text, mode=0o644):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text)
    path.chmod(mode)


def stubs_for(manager):
    common = {"wget": WGET, "sed": SED}
    own = {"apk": APK} if manager == "apk" else {"opkg": OPKG, "opkg-key": OPKG_KEY}
    return {**common, **own}


INDEX_FILES = {
    "apk": ["apk/{arch}/packages.adb"],
    "opkg": ["opkg/{arch}/Packages", "opkg/{arch}/Packages.gz", "opkg/{arch}/Packages.sig"],
}

# The stubs are /bin/sh scripts; LOG, ROOT, FEED_ROOT, PAYLOAD and REAL_SED come from the environment.
WGET = """\
out=""; spider=0; url=""
while [ $# -gt 0 ]; do
  case "$1" in
    -O|-qO) out="$2"; shift ;;
    -qO*) out="${1#-qO}" ;;
    -O*) out="${1#-O}" ;;
    --spider) spider=1 ;;
    -T|-t|-U|-P) shift ;;
    -*) ;;
    *) url="$1" ;;
  esac
  shift
done
echo "wget $url" >> "$LOG"
src="$FEED_ROOT/$(printf '%s' "$url" | "$REAL_SED" 's#^[a-z]*://[^/]*/##')"
[ -f "$src" ] || { echo "wget: server returned error: HTTP/1.1 404 Not Found" >&2; exit 1; }
[ "$spider" = 1 ] && exit 0
if [ "$out" = "-" ]; then cat "$src"; else cp "$src" "${out:-./$(basename "$src")}"; fi
"""

# BusyBox-style `sed -i SCRIPT FILE` on the BSD sed of a Mac; other calls pass through.
SED = """\
if [ "$1" = "-i" ] && ! "$REAL_SED" --version >/dev/null 2>&1; then
  shift; exec "$REAL_SED" -i '' "$@"
fi
exec "$REAL_SED" "$@"
"""

OPKG_KEY = """\
echo "opkg-key $*" >> "$LOG"
[ "$1" = add ] && cp "$2" "$ROOT/key-added.pub"
exit 0
"""

# Shared by the package manager stubs: what the package puts on the router.
PUT = """\
put() { mkdir -p "$(dirname "$2")"; cp "$1" "$2"; }
put_files() {
  put "$PAYLOAD/binary" "$ROOT/usr/bin/meshtown"
  "$1" "$PAYLOAD/init.d" "$ROOT/etc/init.d/meshtown"
}
put_protected() {      # apk leaves its own copy next to a different file under /etc
  if [ -e "$2" ] && ! cmp -s "$1" "$2"; then put "$1" "$2.apk-new"; else put "$1" "$2"; fi
}
"""

APK = PUT + """\
echo "apk $*" >> "$LOG"
case "$1" in
  add) put_files put_protected; exit 0 ;;
  update|del|upgrade) exit 0 ;;
  *) exit 1 ;;
esac
"""

OPKG = PUT + """\
echo "opkg $*" >> "$LOG"
if [ "$1" = install ]; then
  case " $* " in *" --force-overwrite "*) ;; *)
    if [ -e "$ROOT/usr/bin/meshtown" ]; then
      echo "Collected errors: * check_data_file_clashes: file is already provided by package <no package>" >&2
      exit 255
    fi ;;
  esac
  put_files put
fi
exit 0
"""


if __name__ == "__main__":
    unittest.main()
