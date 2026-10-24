"""Tests of apt_repo.py: the real apt-get against a local HTTP server (runs only where apt-get is installed).

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import functools
import http.server
import os
import re
import shutil
import subprocess
import threading
import unittest
from pathlib import Path

import deb
import apt_repo
from test_support import HAS_APT, HAS_GPG
from apt_support import Env, POOL, build_debs, keys, remove_keys


def tearDownModule():
    remove_keys()


@unittest.skipUnless(HAS_APT and HAS_GPG, "apt-get, gpg, gpgv and gpgconf are required")
class RealApt(unittest.TestCase):
    def test_update_is_clean_and_the_package_downloads_byte_for_byte(self):
        # given
        apt = apt_setup(self)
        apt.publish("0.3.1")
        # when
        updated = apt.run("apt-get", "update")
        policy = apt.run("apt-cache", "policy", "meshtown")
        downloaded = apt.run("apt-get", "download", "meshtown", cwd=apt.download)
        # then
        self.assertEqual(updated.returncode, 0, updated.stderr)
        self.assertEqual(problem_lines(updated), [], "W:/E:/N: lines in apt-get update output")
        self.assertIn("Candidate: 0.3.1", policy.stdout)
        self.assertIn(apt.url, policy.stdout)
        self.assertEqual(downloaded.returncode, 0, downloaded.stderr)
        fetched = list(apt.download.glob("meshtown_*.deb"))
        self.assertEqual(len(fetched), 1)
        self.assertEqual(fetched[0].read_bytes(), (apt.repo / POOL / f"meshtown_0.3.1_{apt.arch}.deb").read_bytes())

    def test_client_with_the_previous_release_from_a_cdn_cache_still_updates(self):
        # given
        apt = apt_setup(self)
        apt.publish("0.3.1")
        cached = {name: (apt.stable / name).read_bytes() for name in ("InRelease", "Release", "Release.gpg")}
        apt.publish("0.3.2")
        for name, data in cached.items():
            (apt.stable / name).write_bytes(data)
        apt.requests.clear()
        # when
        updated = apt.run("apt-get", "update")
        policy = apt.run("apt-cache", "policy", "meshtown")
        # then
        self.assertEqual(updated.returncode, 0, updated.stderr)
        self.assertEqual(problem_lines(updated), [], "W:/E:/N: lines in apt-get update output")
        self.assertIn(200, [code for path, code in apt.requests if "/by-hash/SHA256/" in path], apt.requests)
        self.assertNotIn(404, [code for _, code in apt.requests], apt.requests)
        self.assertIn("Candidate: 0.3.1", policy.stdout)


def problem_lines(done) -> list:
    return re.findall(r"^(?:W|E|N):.*$", done.stdout + done.stderr, re.MULTILINE)


class AptSandbox:
    """A repository served over local HTTP and an apt configuration confined to a temporary tree."""

    CONFIG = """Dir::State "{t}/state"; Dir::State::status "{t}/state/status"; Dir::Cache "{t}/cache"; Dir::Log "{t}/log";
Dir::Etc::sourcelist "{t}/sources.list"; Dir::Etc::sourceparts "{t}/sources.list.d"; Dir::Etc::parts "{t}/apt.conf.d";
Dir::Etc::main "/dev/null"; Dir::Etc::trusted "{t}/trusted.gpg"; Dir::Etc::trustedparts "{t}/trusted.gpg.d";
Dir::Etc::preferences "{t}/preferences"; Dir::Etc::preferencesparts "{t}/preferences.d";
Debug::NoLocking "true"; Acquire::Languages "none";
"""

    def __init__(self, test, arch):
        self.test = test
        self.env = Env(test)
        self.repo = self.env.repo
        self.stable = self.repo / "dists" / "stable"
        self.arch = arch
        self.t = self.env.tmp / "apt"
        self.download = self.t / "dl"
        self.requests = []
        self.secret, self.pubkey = keys("a")
        self._start_server()
        self._write_config()

    def _start_server(self):
        requests = self.requests

        class Handler(http.server.SimpleHTTPRequestHandler):
            def log_request(self, code="-", size="-"):
                requests.append((self.path, int(code)))

            def log_message(self, *args):
                pass

        handler = functools.partial(Handler, directory=str(self.repo))
        server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        self.test.addCleanup(server.server_close)
        self.test.addCleanup(server.shutdown)
        self.url = f"http://127.0.0.1:{server.server_address[1]}"

    def _write_config(self):
        t = self.t
        for sub in ("state/lists/partial", "cache/archives/partial", "apt.conf.d", "trusted.gpg.d", "preferences.d",
                    "sources.list.d", "log", "dl"):
            (t / sub).mkdir(parents=True)
        (t / "state" / "status").write_text("")
        (t / "sources.list").write_text("")
        (t / "apt.conf").write_text(self.CONFIG.format(t=t))
        (t / "sources.list.d" / "meshtown.sources").write_text(
            f"Types: deb\nURIs: {self.url}\nSuites: stable\nComponents: main\nArchitectures: {self.arch}\n"
            f"Signed-By: {Path(self.pubkey).resolve()}\n")

    def publish(self, version):
        """Adds the version of both arches to the repository and signs a new publication."""
        folder = self.env.tmp / f"in-{version}"
        build_debs(self.test, folder, {"amd64": [version], "arm64": [version]})
        apt_repo.update(self.repo, folder, 4, self.secret, pubkey=self.pubkey)

    def run(self, *cmd, cwd=None):
        if cmd[0] == "apt-get" and cmd[1] == "update":
            shutil.rmtree(self.t / "state" / "lists")
            (self.t / "state" / "lists" / "partial").mkdir(parents=True)
        env = dict(os.environ, APT_CONFIG=str(self.t / "apt.conf"), LC_ALL="C")
        return subprocess.run(list(cmd), env=env, cwd=cwd, capture_output=True, text=True, timeout=120)


def apt_setup(test) -> AptSandbox:
    done = subprocess.run(["dpkg", "--print-architecture"], capture_output=True, text=True)
    arch = done.stdout.strip() if done.returncode == 0 else ""
    if arch not in deb.ARCHES:
        raise unittest.SkipTest(f"architecture {arch!r} is not one of {sorted(deb.ARCHES)}")
    return AptSandbox(test, arch)


if __name__ == "__main__":
    unittest.main()
