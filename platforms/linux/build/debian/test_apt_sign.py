"""Tests of apt_repo.py: signing, the update step and the command line. They need gpg, gpgv and gpgconf.

Run from the tree root: python3 -m unittest discover -s platforms/linux/build/debian -t platforms/linux/build/debian
"""

import email.utils
import os
import unittest
from unittest import mock

import apt_repo
from test_support import HAS_GPG
from apt_repo import RepoError
from apt_support import (
    NOW,
    build_debs,
    date_of,
    gpgv,
    index_path,
    keys,
    published,
    remove_keys,
    run_main,
    setup,
)


def tearDownModule():
    remove_keys()


@unittest.skipUnless(HAS_GPG, "gpg, gpgv and gpgconf are required")
class Signing(unittest.TestCase):
    def test_gpgv_accepts_both_signatures(self):
        # given
        env = published(self)
        stable = env.repo / "dists" / "stable"
        # when
        in_release = gpgv(self, env.pubkey, stable / "InRelease")
        detached = gpgv(self, env.pubkey, stable / "Release.gpg", stable / "Release")
        # then
        self.assertEqual(in_release.returncode, 0, in_release.stderr)
        self.assertEqual(detached.returncode, 0, detached.stderr)

    def test_detached_signature_is_armored(self):
        # given
        env = published(self)
        # when
        text = (env.repo / "dists" / "stable" / "Release.gpg").read_text()
        # then
        self.assertTrue(text.startswith("-----BEGIN PGP SIGNATURE-----"), text[:60])

    def test_inrelease_is_clearsigned_with_sha512(self):
        # given
        env = published(self)
        # when
        text = (env.repo / "dists" / "stable" / "InRelease").read_text()
        # then
        self.assertTrue(text.startswith("-----BEGIN PGP SIGNED MESSAGE-----"), text[:60])
        self.assertIn("Hash: SHA512", text)

    def test_repo_carries_a_byte_copy_of_the_public_key(self):
        # given
        env = published(self)
        # when
        copy = (env.repo / "meshtown.gpg").read_bytes()
        # then
        self.assertEqual(copy, env.pubkey.read_bytes())

    def test_public_key_of_another_pair_fails_the_check_with_a_signature_error(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        secret, _ = keys("a")
        _, other_public = keys("b")
        # when / then
        with self.assertRaisesRegex(RepoError, "(?i)signature"):
            apt_repo.update(env.repo, env.incoming, 4, secret, pubkey=other_public, now=NOW)

    def test_two_secret_keys_in_one_file_are_refused(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        secret_a, public_a = keys("a")
        secret_b, _ = keys("b")
        both = env.tmp / "both.asc"
        both.write_bytes(secret_a.read_bytes() + secret_b.read_bytes())
        # when / then
        with self.assertRaisesRegex(RepoError, "expected one secret key"):
            apt_repo.update(env.repo, env.incoming, 4, both, pubkey=public_a, now=NOW)

    def test_users_gnupg_home_stays_empty(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        secret, public = keys("a")
        home = env.tmp / "gnupg-home"
        home.mkdir(mode=0o700)
        # when
        with mock.patch.dict(os.environ, {"GNUPGHOME": str(home)}):
            apt_repo.update(env.repo, env.incoming, 4, secret, pubkey=public, now=NOW)
        # then
        self.assertEqual(list(home.iterdir()), [], "update must use a throwaway home directory")

    def test_tampered_release_fails_verification(self):
        # given
        env = published(self)
        stable = env.repo / "dists" / "stable"
        with open(stable / "Release", "ab") as f:
            f.write(b"Extra: tampered\n")
        # when / then
        with self.assertRaises(RepoError):
            apt_repo.verify_release(stable, env.pubkey)


@unittest.skipUnless(HAS_GPG, "gpg, gpgv and gpgconf are required")
class Update(unittest.TestCase):
    def test_direct_links_and_dists_are_made_and_foreign_files_are_left_alone(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        (env.repo / "README.md").write_text("readme\n")
        (env.repo / "SHA256SUMS").write_text("sums\n")
        secret, public = keys("a")
        # when
        apt_repo.update(env.repo, env.incoming, 4, secret, pubkey=public, now=NOW)
        # then
        self.assertTrue((env.repo / "meshtown_amd64.deb").is_file())
        self.assertFalse((env.repo / "meshtown_arm64.deb").exists())
        self.assertTrue((env.repo / "dists" / "stable" / "InRelease").is_file())
        self.assertEqual((env.repo / "README.md").read_text(), "readme\n")
        self.assertEqual((env.repo / "SHA256SUMS").read_text(), "sums\n")

    def test_repo_is_created_when_missing(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        secret, public = keys("a")
        repo = env.tmp / "new" / "repo"
        # when
        apt_repo.update(repo, env.incoming, 4, secret, pubkey=public, now=NOW)
        # then
        self.assertTrue((repo / "dists" / "stable" / "Release").is_file())

    def test_keep_limits_the_pool_after_the_incoming_files_are_added(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1", "0.3.2"]})
        secret, public = keys("a")
        # when
        apt_repo.update(env.repo, env.incoming, 1, secret, pubkey=public, now=NOW)
        # then
        self.assertEqual([p.name for p in env.pool.iterdir()], ["meshtown_0.3.2_amd64.deb"])
        self.assertNotIn("0.3.1", index_path(env.repo, "amd64").read_text())

    def test_second_publication_is_newer_and_verifies(self):
        # given
        env = published(self)
        build_debs(self, env.tmp / "next", {"amd64": ["0.3.2"], "arm64": ["0.3.2"]})
        before = date_of(env.repo / "dists" / "stable" / "Release")
        secret, public = keys("a")
        # when
        apt_repo.update(env.repo, env.tmp / "next", 4, secret, pubkey=public, now=NOW)
        # then
        after = date_of(env.repo / "dists" / "stable" / "Release")
        self.assertEqual(after, email.utils.formatdate(NOW + 1, usegmt=True))
        self.assertNotEqual(after, before)
        self.assertEqual(len(list(env.pool.iterdir())), 4)


class Cli(unittest.TestCase):
    @unittest.skipUnless(HAS_GPG, "gpg, gpgv and gpgconf are required")
    def test_update_with_a_key_exits_zero_and_publishes(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"], "arm64": ["0.3.1"]})
        secret, public = keys("a")
        # when
        code, err = run_main("update", env.repo, env.incoming, "--keep", "4", "--key", secret, "--pubkey", public)
        # then
        self.assertEqual(code, 0, err)
        self.assertTrue((env.repo / "dists" / "stable" / "InRelease").is_file())

    def test_missing_key_file_exits_one_with_a_prefixed_message(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        # when
        code, err = run_main("update", env.repo, env.incoming, "--keep", "4", "--key", env.tmp / "absent.asc",
                             "--pubkey", env.pubkey)
        # then
        self.assertEqual(code, 1)
        self.assertTrue(err.startswith("apt_repo: "), err)

    def test_keep_zero_exits_one_with_a_prefixed_message(self):
        # given
        env = setup(self)
        build_debs(self, env.incoming, {"amd64": ["0.3.1"]})
        key = env.tmp / "key.asc"
        key.write_text("placeholder\n")
        # when
        code, err = run_main("update", env.repo, env.incoming, "--keep", "0", "--key", key, "--pubkey", env.pubkey)
        # then
        self.assertEqual(code, 1)
        self.assertTrue(err.startswith("apt_repo: "), err)

    def test_unknown_option_exits_one_instead_of_raising_system_exit(self):
        # given
        env = setup(self)
        # when
        code, err = run_main("update", env.repo, env.incoming, "--bogus")
        # then
        self.assertEqual(code, 1)
        self.assertIn("apt_repo: ", err)

    def test_empty_incoming_folder_exits_one(self):
        # given
        env = setup(self)
        key = env.tmp / "key.asc"
        key.write_text("placeholder\n")
        # when
        code, err = run_main("update", env.repo, env.incoming, "--keep", "4", "--key", key, "--pubkey", env.pubkey)
        # then
        self.assertEqual(code, 1)
        self.assertIn("apt_repo: ", err)


if __name__ == "__main__":
    unittest.main()
