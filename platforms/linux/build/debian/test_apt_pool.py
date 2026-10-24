"""Tests of apt_repo.py: package file names, the incoming directory and the pool.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import contextlib
import io
import shutil
import unittest

import apt_repo
from apt_repo import RepoError
from apt_support import NOW, build_debs, pool_map, published, setup


class ParseName(unittest.TestCase):
    def test_numeric_version_and_arch_of_a_package_name(self):
        # given
        name = "meshtown_0.3.10_arm64.deb"
        # when
        parsed = apt_repo.parse_name(name)
        # then
        self.assertEqual(parsed, ((0, 3, 10), "arm64"))

    def test_foreign_names_are_none(self):
        # given
        names = ["meshtown_0.3_amd64.deb", "meshtown_0.3.1_i386.deb", "other_0.3.1_amd64.deb",
                 "meshtown_0.3.1_amd64.deb.bak", "meshtown_a.b.c_amd64.deb"]
        # when / then
        for name in names:
            with self.subTest(name=name):
                self.assertIsNone(apt_repo.parse_name(name))


class Incoming(unittest.TestCase):
    def test_both_arches_land_in_the_pool_byte_for_byte(self):
        # given
        env = setup(self)
        debs = build_debs(self, env.incoming, {"amd64": ["0.3.1"], "arm64": ["0.3.1"]})
        # when
        apt_repo.add_to_pool(env.repo, debs)
        # then
        for source in debs:
            self.assertEqual((env.pool / source.name).read_bytes(), source.read_bytes(), source.name)

    def test_same_version_with_other_bytes_keeps_the_published_file_and_says_so(self):
        # given
        env = setup(self)
        first = build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        apt_repo.add_to_pool(env.repo, first)
        published = (env.pool / first[0].name).read_bytes()
        other = build_debs(self, env.tmp / "other", {"amd64": ["0.3.1"]}, payload=b"#!/bin/sh\necho different\n")
        # when
        with contextlib.redirect_stderr(io.StringIO()) as err:
            apt_repo.add_to_pool(env.repo, other)
        # then
        self.assertEqual((env.pool / first[0].name).read_bytes(), published)
        self.assertIn("apt_repo: notice: pool keeps the published meshtown_0.3.1_amd64.deb (incoming differs)",
                      err.getvalue())

    def test_same_version_with_same_bytes_is_silent(self):
        # given
        env = setup(self)
        debs = build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        apt_repo.add_to_pool(env.repo, debs)
        # when
        with contextlib.redirect_stderr(io.StringIO()) as err:
            apt_repo.add_to_pool(env.repo, debs)
        # then
        self.assertEqual(err.getvalue(), "")
        self.assertEqual([p.name for p in env.pool.iterdir()], [debs[0].name])

    def test_only_deb_files_are_taken_from_the_folder(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        (env.incoming / "README.md").write_text("not a package\n")
        # when
        checked = apt_repo.check_incoming(env.incoming)
        # then
        self.assertEqual([p.name for p in checked], ["meshtown_0.3.1_amd64.deb"])

    def test_bad_names_and_contents_are_refused(self):
        # given
        env = setup(self)
        good = build_debs(self, env.tmp / "src", {"amd64": ["0.3.1"]})[0]
        cases = {
            "no patch number": "meshtown_0.3_amd64.deb",
            "unknown arch": "meshtown_0.3.1_i386.deb",
            "name version differs from Version": "meshtown_0.3.2_amd64.deb",
            "name arch differs from Architecture": "meshtown_0.3.1_arm64.deb",
        }
        for number, (label, name) in enumerate(cases.items()):
            with self.subTest(label):
                folder = env.tmp / f"in-{number}"
                folder.mkdir()
                shutil.copy(good, folder / name)
                # when / then
                with self.assertRaises(RepoError):
                    apt_repo.check_incoming(folder)

    def test_empty_folder_is_refused(self):
        # given
        env = setup(self)
        # when / then
        with self.assertRaises(RepoError):
            apt_repo.check_incoming(env.incoming)

    def test_bad_input_leaves_repo_untouched(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        good = env.incoming / "meshtown_0.3.1_amd64.deb"
        shutil.copy(good, env.incoming / "meshtown_0.3.2_amd64.deb")  # Version inside says 0.3.1
        dummy_key = env.tmp / "key.asc"
        dummy_key.write_text("not used: the input is refused first\n")
        # when
        with self.assertRaises(RepoError):
            apt_repo.update(env.repo, env.incoming, 4, dummy_key, pubkey=env.pubkey, now=NOW)
        # then
        self.assertEqual(list(env.repo.rglob("*")), [], "REPO must stay empty")


class Pool(unittest.TestCase):
    def test_prune_keeps_the_newest_per_arch_by_numeric_version(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": [f"0.3.{n}" for n in range(5, 11)], "arm64": ["0.3.9", "0.3.10"]})
        # when
        kept = apt_repo.prune_pool(env.repo, 4)
        # then
        want = {"amd64": [f"meshtown_0.3.{n}_amd64.deb" for n in range(7, 11)],
                "arm64": ["meshtown_0.3.9_arm64.deb", "meshtown_0.3.10_arm64.deb"]}
        self.assertEqual({arch: [p.name for p in paths] for arch, paths in kept.items()}, want)
        self.assertEqual(sorted(p.name for p in env.pool.iterdir()), sorted(want["amd64"] + want["arm64"]))

    def test_keep_below_one_is_refused_and_nothing_is_deleted(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1", "0.3.2"]})
        # when / then
        with self.assertRaises(RepoError):
            apt_repo.prune_pool(env.repo, 0)
        self.assertEqual(len(list(env.pool.iterdir())), 2)

    def test_listing_has_every_arch_even_when_empty_in_ascending_order(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.10", "0.3.9"]})
        # when
        listing = apt_repo.pool_by_arch(env.repo)
        # then
        self.assertEqual(set(listing), {"amd64", "arm64"})
        self.assertEqual([p.name for p in listing["amd64"]], ["meshtown_0.3.9_amd64.deb", "meshtown_0.3.10_amd64.deb"])
        self.assertEqual(listing["arm64"], [])

    def test_foreign_file_in_the_pool_is_refused(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        (env.pool / "notes.txt").write_text("stray\n")
        # when / then
        with self.assertRaises(RepoError):
            apt_repo.pool_by_arch(env.repo)


class DirectLinks(unittest.TestCase):
    def test_newest_version_is_copied_and_an_empty_arch_has_no_link(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.9", "0.3.10"]})
        # when
        apt_repo.write_direct_links(env.repo, pool_map(env.repo))
        # then
        link = env.repo / "meshtown_amd64.deb"
        self.assertEqual(link.read_bytes(), (env.pool / "meshtown_0.3.10_amd64.deb").read_bytes())
        self.assertFalse(link.is_symlink(), "a copy, not a symlink")
        self.assertFalse((env.repo / "meshtown_arm64.deb").exists())

    def test_stale_link_of_an_arch_without_packages_is_removed(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        (env.repo / "meshtown_arm64.deb").write_bytes(b"stale")
        # when
        apt_repo.write_direct_links(env.repo, pool_map(env.repo))
        # then
        self.assertFalse((env.repo / "meshtown_arm64.deb").exists())

    def test_older_link_is_replaced_by_the_newest(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1", "0.3.2"]})
        (env.repo / "meshtown_amd64.deb").write_bytes(b"old")
        # when
        apt_repo.write_direct_links(env.repo, pool_map(env.repo))
        # then
        self.assertEqual((env.repo / "meshtown_amd64.deb").read_bytes(),
                         (env.pool / "meshtown_0.3.2_amd64.deb").read_bytes())


if __name__ == "__main__":
    unittest.main()
