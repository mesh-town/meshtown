"""Tests of keys.py: usign signatures and generated one-off keys.

They need the usign tool (and openssl for the apk key); without them they are skipped, unless
MESHTOWN_FEED_REQUIRE_TOOLS=1 turns a skip into a failure.

Run from the tree root: python3 -m unittest discover -s build/openwrt/feed -t build/openwrt/feed
"""

import subprocess
import unittest
from collections import namedtuple
from pathlib import Path

import apk
import keys
from feed_fixtures import apk_or_skip, bare_apk, make_keys, openssl_or_skip, tmpdir, usign_or_skip


class UsignSignature(unittest.TestCase):
    def test_signing_writes_a_sig_file_next_to_the_signed_file(self):
        # given
        signed = setup(self)
        # when
        sig = signed.file.with_name(signed.file.name + ".sig")
        # then
        self.assertTrue(sig.is_file(), "usign puts the signature in <file>.sig")

    def test_matching_public_key_accepts_the_file(self):
        # given
        signed = setup(self)
        # when
        ok = keys.verify_usign(signed.file, signed.pub, signed.usign)
        # then
        self.assertIs(ok, True)

    def test_a_changed_byte_in_the_file_is_rejected(self):
        # given
        signed = setup(self)
        data = bytearray(signed.file.read_bytes())
        data[len(data) // 2] ^= 0x01
        signed.file.write_bytes(bytes(data))
        # when
        ok = keys.verify_usign(signed.file, signed.pub, signed.usign)
        # then
        self.assertIs(ok, False)

    def test_a_signature_that_was_tampered_with_is_rejected(self):
        # given: the base64 line of the .sig, one character replaced
        signed = setup(self)
        sig = signed.file.with_name(signed.file.name + ".sig")
        comment, body = sig.read_text().split("\n", 1)
        middle = len(body) // 2
        sig.write_text(comment + "\n" + body[:middle] + ("A" if body[middle] != "A" else "B") + body[middle + 1:])
        # when
        ok = keys.verify_usign(signed.file, signed.pub, signed.usign)
        # then
        self.assertIs(ok, False)

    def test_another_public_key_rejects_the_file(self):
        # given
        signed = setup(self)
        _, _, other_pubdir = make_keys(tmpdir(self) / "other")
        # when
        ok = keys.verify_usign(signed.file, other_pubdir / "meshtown-usign.pub", signed.usign)
        # then
        self.assertIs(ok, False)


class EphemeralKeys(unittest.TestCase):
    def test_pair_is_usable_and_public_half_has_the_published_name(self):
        # given
        usign, tmp = usign_or_skip(), tmpdir(self)
        openssl_or_skip()
        # when
        usign_sec, _, pubdir = map(Path, keys.ephemeral_keys(tmp, usign))
        # then
        file = tmp / "Packages"
        file.write_text("Package: meshtown\n")
        keys.sign_usign(file, usign_sec, usign)
        self.assertTrue(keys.verify_usign(file, pubdir / "meshtown-usign.pub", usign))

    def test_apk_key_is_a_p256_key_matching_the_published_pem(self):
        # given
        usign, openssl, tmp = usign_or_skip(), openssl_or_skip(), tmpdir(self)
        # when
        _, apk_sec, pubdir = map(Path, keys.ephemeral_keys(tmp, usign))
        # then
        secret = text_of(openssl, "ec", "-in", apk_sec, "-noout", "-text")
        derived = text_of(openssl, "ec", "-in", apk_sec, "-pubout")
        self.assertTrue("prime256v1" in secret or "P-256" in secret, "the apk key must be on the P-256 curve")
        self.assertEqual(derived.strip(), (pubdir / "meshtown-apk.pem").read_text().strip())

    def test_apk_pair_signs_an_index_the_public_half_verifies(self):
        # given
        usign, apk_bin, tmp = usign_or_skip(), apk_or_skip(), tmpdir(self)
        openssl_or_skip()
        _, apk_sec, pubdir = map(Path, keys.ephemeral_keys(tmp, usign))
        out = tmp / "out"
        out.mkdir()
        bare_apk(apk_bin, out)
        # when
        index = apk.write_index(out, apk_bin, apk_sec)
        # then
        self.assertTrue(apk.verify_apk_index(index, pubdir, apk_bin))

    def test_every_call_makes_new_keys(self):
        # given
        usign = usign_or_skip()
        openssl_or_skip()
        # when
        first = keys.ephemeral_keys(tmpdir(self), usign)
        second = keys.ephemeral_keys(tmpdir(self), usign)
        # then
        for one, other in zip(first[:2], second[:2]):
            self.assertNotEqual(Path(one).read_bytes(), Path(other).read_bytes())


# ---------- setup and helpers ----------

Signed = namedtuple("Signed", "file pub usign")


def setup(test):
    """A Packages file signed with a fresh usign key."""
    usign = usign_or_skip()
    tmp = tmpdir(test)
    usign_sec, _, pubdir = make_keys(tmp / "keys")
    file = tmp / "Packages"
    file.write_text("Package: meshtown\nVersion: 0.2.7-r1\nArchitecture: x86_64\n\n")
    keys.sign_usign(file, usign_sec, usign)
    return Signed(file, pubdir / "meshtown-usign.pub", usign)


def text_of(*cmd):
    return subprocess.run([str(c) for c in cmd], check=True, capture_output=True, text=True).stdout


if __name__ == "__main__":
    unittest.main()
