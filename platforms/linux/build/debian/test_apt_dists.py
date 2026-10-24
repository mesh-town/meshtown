"""Tests of apt_repo.py: the dists tree (Packages, Release and by-hash files).

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import email.utils
import gzip
import re
import shutil
import unittest
from unittest import mock

import deb
import apt_repo
from apt_repo import RepoError
from apt_support import (
    INDEX_PATHS,
    NOW,
    POOL,
    Published,
    RELEASE_HEADER,
    build_debs,
    date_of,
    hash_dir,
    header_lines,
    index_path,
    paragraph,
    pool_map,
    setup,
    sha,
)


class PackagesIndex(unittest.TestCase):
    def test_paragraph_is_control_with_filename_size_sha256_before_description(self):
        # given
        env = setup(self)
        debs = build_debs(self, env.pool, {"amd64": ["0.3.9", "0.3.10"]})
        # when
        apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        text = (index_path(env.repo, "amd64")).read_text()
        self.assertEqual(text, "\n\n".join(paragraph(p) for p in debs) + "\n")

    def test_paragraphs_ascend_by_version_with_one_blank_line_between(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.10", "0.3.9", "0.3.2"]})
        # when
        apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        text = index_path(env.repo, "amd64").read_text()
        versions = re.findall(r"^Version: (.*)$", text, re.MULTILINE)
        self.assertEqual(versions, ["0.3.2", "0.3.9", "0.3.10"])
        self.assertEqual(text.count("\n\n"), 2)
        self.assertTrue(text.endswith("\n") and not text.endswith("\n\n"), "exactly one final newline")

    def test_file_named_by_filename_exists_and_matches_size_and_hash(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # when
        fields = dict(line.split(": ", 1) for line in index_path(env.repo, "amd64").read_text().splitlines()
                      if not line.startswith(" "))
        data = (env.repo / fields["Filename"]).read_bytes()
        # then
        self.assertEqual(fields["Filename"], f"{POOL}/meshtown_0.3.1_amd64.deb")
        self.assertEqual(int(fields["Size"]), len(data))
        self.assertEqual(fields["SHA256"], sha(data))

    def test_no_weak_hash_fields(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when
        apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        text = index_path(env.repo, "amd64").read_text()
        for field in ("MD5sum:", "SHA1:"):
            self.assertNotIn(field, text)

    def test_blank_line_inside_control_is_refused(self):
        # given
        env = setup(self)
        debs = build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when / then
        with mock.patch.object(deb, "read_control", return_value="Package: meshtown\n\nVersion: 0.3.1\n"):
            with self.assertRaises(RepoError):
                apt_repo.packages_index(debs)

    def test_arch_without_packages_has_an_empty_file_listed_in_release(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when
        release = apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        self.assertEqual(index_path(env.repo, "arm64").stat().st_size, 0)
        self.assertIn(f" {sha(b'')} 0 main/binary-arm64/Packages", release.read_text().splitlines())


class PackagesGz(unittest.TestCase):
    def test_gunzip_gives_packages_and_repeated_builds_give_equal_bytes(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1", "0.3.2"]})
        pool = pool_map(env.repo)
        gz = index_path(env.repo, "amd64", "Packages.gz")
        apt_repo.write_dists(env.repo, pool, NOW)
        first = gz.read_bytes()
        # when
        apt_repo.write_dists(env.repo, pool, NOW + 60)
        # then
        self.assertEqual(gz.read_bytes(), first)
        self.assertEqual(gzip.decompress(first), index_path(env.repo, "amd64").read_bytes())

    def test_header_has_no_name_and_no_timestamp(self):
        # given
        data = b"Package: meshtown\n"
        # when
        packed = apt_repo.gzip_bytes(data)
        # then
        self.assertEqual(packed[3], 0, "no FNAME/FEXTRA/FCOMMENT flags")
        self.assertEqual(packed[4:8], b"\0\0\0\0", "mtime 0")
        self.assertEqual(packed, apt_repo.gzip_bytes(data))
        self.assertEqual(gzip.decompress(packed), data)


class ReleaseFile(unittest.TestCase):
    def test_fields_come_in_the_documented_order(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when
        release = apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        lines = release.read_text().splitlines()
        self.assertEqual(lines[:len(RELEASE_HEADER) + 1], header_lines(NOW) + ["SHA256:"])

    def test_sha256_lists_four_index_files_with_correct_hash_and_size(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"], "arm64": ["0.3.1"]})
        # when
        release = apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        want = []
        for path in INDEX_PATHS:
            data = (env.repo / "dists" / "stable" / path).read_bytes()
            want.append(f" {sha(data)} {len(data)} {path}")
        lines = release.read_text().splitlines()
        self.assertEqual(lines[len(RELEASE_HEADER) + 1:], want)

    def test_no_expiry_and_no_weak_hashes(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when
        text = apt_repo.write_dists(env.repo, pool_map(env.repo), NOW).read_text()
        # then
        for field in ("Valid-Until", "MD5Sum", "SHA1", "SHA512"):
            self.assertNotIn(field, text)

    def test_date_is_the_given_time(self):
        # given
        files = {path: b"data" for path in INDEX_PATHS}
        # when
        text = apt_repo.release_text(files, NOW)
        # then
        self.assertIn(f"Date: {email.utils.formatdate(NOW, usegmt=True)}", text.splitlines())

    def test_older_date_later_than_now_is_followed_by_one_second_more(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        pool = pool_map(env.repo)
        apt_repo.write_dists(env.repo, pool, NOW + 1000)
        # when
        release = apt_repo.write_dists(env.repo, pool, NOW)
        # then
        self.assertEqual(date_of(release), email.utils.formatdate(NOW + 1001, usegmt=True))

    def test_older_date_equal_to_now_is_followed_by_one_second_more(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        pool = pool_map(env.repo)
        apt_repo.write_dists(env.repo, pool, NOW)
        # when
        release = apt_repo.write_dists(env.repo, pool, NOW)
        # then
        self.assertEqual(date_of(release), email.utils.formatdate(NOW + 1, usegmt=True))

    def test_no_component_release_and_no_translation_or_contents_files(self):
        # given
        env = setup(self)
        build_debs(self, env.pool, {"amd64": ["0.3.1"]})
        # when
        apt_repo.write_dists(env.repo, pool_map(env.repo), NOW)
        # then
        files = sorted(p.relative_to(env.repo / "dists" / "stable").as_posix()
                       for p in (env.repo / "dists" / "stable").rglob("*") if p.is_file() and "by-hash" not in p.parts)
        self.assertEqual(files, ["Release"] + INDEX_PATHS)


class ByHash(unittest.TestCase):
    def test_first_publication_has_the_two_index_files_named_by_their_hash(self):
        # given
        pub = Published(self)
        # when
        pub.add("0.3.1")
        pub.publish()
        # then
        for arch in ("amd64", "arm64"):
            self.assertEqual(pub.stored(arch), pub.current(arch), arch)
            for name in ("Packages", "Packages.gz"):
                data = index_path(pub.repo, arch, name).read_bytes()
                self.assertEqual((hash_dir(pub.repo, arch) / sha(data)).read_bytes(), data)

    def test_second_publication_keeps_the_previous_one(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        pub.publish()
        first = {arch: pub.current(arch) for arch in ("amd64", "arm64")}
        # when
        pub.add("0.3.2")
        pub.publish()
        # then
        for arch in ("amd64", "arm64"):
            self.assertEqual(len(pub.stored(arch)), 4, arch)
            self.assertEqual(pub.stored(arch), first[arch] | pub.current(arch), arch)

    def test_third_publication_drops_the_first(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        pub.publish()
        first = {arch: pub.current(arch) for arch in ("amd64", "arm64")}
        pub.add("0.3.2")
        pub.publish()
        second = {arch: pub.current(arch) for arch in ("amd64", "arm64")}
        # when
        pub.add("0.3.3")
        pub.publish()
        # then
        for arch in ("amd64", "arm64"):
            self.assertEqual(len(pub.stored(arch)), 4, arch)
            self.assertEqual(pub.stored(arch), second[arch] | pub.current(arch), arch)
            self.assertFalse(pub.stored(arch) & first[arch], "the oldest generation must be gone")

    def test_repeat_without_changes_keeps_one_generation(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        pub.publish()
        # when
        pub.publish()
        # then
        for arch in ("amd64", "arm64"):
            self.assertEqual(len(pub.stored(arch)), 2, arch)

    def test_stray_files_under_dists_are_removed(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        pub.publish()
        junk = pub.repo / "dists" / "stable" / "junk"
        junk.write_text("left over\n")
        # when
        pub.publish()
        # then
        self.assertFalse(junk.exists())


class PreviousHashes(unittest.TestCase):
    def test_no_release_means_no_previous_generation(self):
        # given
        env = setup(self)
        # when
        previous = apt_repo.previous_hashes(env.repo / "dists" / "stable")
        # then
        self.assertEqual(previous, {})

    def test_release_without_sha256_means_no_previous_generation(self):
        # given
        env = setup(self)
        stable = env.repo / "dists" / "stable"
        stable.mkdir(parents=True)
        (stable / "Release").write_text("\n".join(header_lines(NOW)) + "\n")
        # when
        previous = apt_repo.previous_hashes(stable)
        # then
        self.assertEqual(previous, {})

    def test_bytes_of_every_listed_file_come_back_keyed_by_hash(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        release = pub.publish()
        want = {(release.parent / p).read_bytes() for p in INDEX_PATHS}
        # when
        previous = apt_repo.previous_hashes(release.parent)
        # then
        self.assertEqual(len(previous), 4)
        self.assertEqual(set(previous.values()), want)
        for (_, hex_digest), data in previous.items():
            self.assertEqual(hex_digest, sha(data))

    def test_missing_by_hash_file_falls_back_to_the_unchanged_index_file(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        release = pub.publish()
        shutil.rmtree(hash_dir(pub.repo, "amd64"))
        # when
        previous = apt_repo.previous_hashes(release.parent)
        # then
        self.assertIn(index_path(pub.repo, "amd64").read_bytes(), previous.values())

    def test_changed_index_file_without_by_hash_copy_is_skipped(self):
        # given
        pub = Published(self)
        pub.add("0.3.1")
        release = pub.publish()
        original = index_path(pub.repo, "amd64").read_bytes()
        shutil.rmtree(hash_dir(pub.repo, "amd64"))
        index_path(pub.repo, "amd64").write_bytes(original + b"tampered\n")
        # when
        previous = apt_repo.previous_hashes(release.parent)
        # then
        self.assertNotIn(original, previous.values())
        self.assertNotIn(original + b"tampered\n", previous.values())


if __name__ == "__main__":
    unittest.main()
