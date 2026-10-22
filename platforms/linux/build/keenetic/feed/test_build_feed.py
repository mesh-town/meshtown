"""Tests of the Keenetic feed builder: layout, package contents, control fields, index, CLI errors.

Run from the tree root: python3 -m unittest discover -s build/keenetic/feed -t build/keenetic/feed
"""

import hashlib
import stat
import subprocess
import sys
import unittest
from pathlib import Path

import build_feed
from feed_fixtures import Unpacked, control_fields, fake_elf, tmpdir

HERE = Path(build_feed.__file__).resolve().parent
TRIPLES = {
    "mipsel-unknown-linux-musl": ("mipselsf-k3.4", "mipsel-3.4", "mipsel"),
    "mips-unknown-linux-musl": ("mipssf-k3.4", "mips-3.4", "mips"),
    "aarch64-unknown-linux-musl": ("aarch64-k3.10", "aarch64-3.10", "aarch64"),
}


def run_cli(*args):
    done = subprocess.run([sys.executable, str(HERE / "build_feed.py"), *map(str, args)],
                          capture_output=True, text=True, stdin=subprocess.DEVNULL)
    return done.returncode, done.stdout, done.stderr


def build(test, version="0.3.30", release="1", triples=tuple(TRIPLES)):
    """Run the CLI over fake binaries: (code, out dir, stderr)."""
    tmp = tmpdir(test)
    args = ["--out", tmp / "feed", "--version", version, "--pkg-release", release, "--base-url", "http://feed.test/"]
    for triple in triples:
        args += ["--bin", f"{triple}={fake_elf(tmp / triple, TRIPLES[triple][2])}"]
    code, _, err = run_cli(*args)
    return code, tmp / "feed", err


class Layout(unittest.TestCase):
    def test_each_architecture_has_its_directory_with_package_and_indexes(self):
        # given / when
        code, out, err = build(self)
        # then
        self.assertEqual(code, 0, err)
        for directory, architecture, _ in TRIPLES.values():
            names = sorted(p.name for p in (out / directory).iterdir())
            self.assertEqual(names, ["Packages", "Packages.gz", f"meshtown_0.3.30-r1_{architecture}.ipk"])

    def test_root_has_install_script_readme_index_and_nojekyll(self):
        # given / when
        _, out, _ = build(self)
        # then
        self.assertEqual(sorted(p.name for p in out.iterdir() if p.is_file()),
                         [".nojekyll", "README.md", "index.html", "install.sh"])
        self.assertTrue((out / "install.sh").stat().st_mode & stat.S_IXUSR)
        self.assertIn("http://feed.test/install.sh", (out / "index.html").read_text())

    def test_only_the_given_targets_are_built(self):
        # given / when
        _, out, _ = build(self, triples=("mips-unknown-linux-musl",))
        # then
        self.assertEqual(sorted(p.name for p in out.iterdir() if p.is_dir()), ["mipssf-k3.4"])


class Package(unittest.TestCase):
    def test_data_lives_under_opt_with_executable_modes(self):
        # given
        _, out, _ = build(self, triples=("mipsel-unknown-linux-musl",))
        # when
        package = Unpacked(next((out / "mipselsf-k3.4").glob("*.ipk")))
        # then
        modes = {name: stat.S_IMODE(info.mode) for name, (info, _) in package.data_files().items()}
        self.assertEqual(modes, {"opt/bin/meshtown": 0o755, "opt/etc/init.d/S99meshtown": 0o755})
        self.assertTrue(all(name.startswith("./opt") or name == "./" for name, _, _ in package.data[1:]))

    def test_control_names_the_entware_architecture_and_the_release(self):
        # given
        _, out, _ = build(self, release="3", triples=("aarch64-unknown-linux-musl",))
        # when
        fields = control_fields(Unpacked(next((out / "aarch64-k3.10").glob("*.ipk"))).control_text())
        # then
        self.assertEqual((fields["Package"], fields["Version"], fields["Architecture"]),
                         ("meshtown", "0.3.30-r3", "aarch64-3.10"))

    def test_control_scripts_drive_the_entware_init_script(self):
        # given
        _, out, _ = build(self, triples=("mips-unknown-linux-musl",))
        # when
        control = Unpacked(next((out / "mipssf-k3.4").glob("*.ipk"))).control_files()
        # then
        self.assertIn(b"/opt/etc/init.d/S99meshtown restart", control["postinst"][1])
        self.assertIn(b"/opt/etc/init.d/S99meshtown stop", control["prerm"][1])
        self.assertNotIn(b"functions.sh", control["postinst"][1] + control["prerm"][1])

    def test_index_entry_matches_the_file(self):
        # given
        _, out, _ = build(self, triples=("mipsel-unknown-linux-musl",))
        directory = out / "mipselsf-k3.4"
        ipk_file = next(directory.glob("*.ipk"))
        # when
        fields = control_fields((directory / "Packages").read_text())
        # then
        self.assertEqual(fields["Filename"], ipk_file.name)
        self.assertEqual(int(fields["Size"]), ipk_file.stat().st_size)
        self.assertEqual(fields["SHA256sum"], hashlib.sha256(ipk_file.read_bytes()).hexdigest())


class Refusals(unittest.TestCase):
    def test_binary_of_another_machine_is_refused(self):
        # given
        tmp = tmpdir(self)
        wrong = fake_elf(tmp / "bin", "mips")
        # when
        code, _, err = run_cli("--out", tmp / "o", "--version", "0.3.30", "--base-url", "http://x",
                               "--bin", f"mipsel-unknown-linux-musl={wrong}")
        # then
        self.assertEqual(code, 2, err)
        self.assertIn("not mipsel-unknown-linux-musl", err)

    def test_unknown_triple_and_missing_binaries_are_refused(self):
        for args in (["--bin", "x86_64-unknown-linux-musl=/nope"], []):
            with self.subTest(args=args):
                # given / when
                code, _, _ = run_cli("--out", tmpdir(self) / "o", "--version", "0.3.30", "--base-url", "http://x", *args)
                # then
                self.assertEqual(code, 2)

    def test_bad_version_and_release_are_refused(self):
        for version, release in (("0.3", "1"), ("0.3.30", "0")):
            with self.subTest(version=version, release=release):
                # given
                tmp = tmpdir(self)
                binary = fake_elf(tmp / "b", "mipsel")
                # when
                code, _, _ = run_cli("--out", tmp / "o", "--version", version, "--pkg-release", release,
                                     "--base-url", "http://x", "--bin", f"mipsel-unknown-linux-musl={binary}")
                # then
                self.assertEqual(code, 2)

    def test_non_empty_output_directory_is_refused(self):
        # given
        tmp = tmpdir(self)
        (tmp / "o").mkdir()
        (tmp / "o" / "old").write_text("x")
        binary = fake_elf(tmp / "b", "mipsel")
        # when
        code, _, _ = run_cli("--out", tmp / "o", "--version", "0.3.30", "--base-url", "http://x",
                             "--bin", f"mipsel-unknown-linux-musl={binary}")
        # then
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
