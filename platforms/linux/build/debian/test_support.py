"""Helpers shared by the Debian packaging tests (Python standard library only).

make_key      a throwaway signing key: (armored secret key, binary public keyring)
fake_binary   an executable file of a few bytes that stands in for the node binary
HAS_GPG       gpg, gpgv and gpgconf are all installed
HAS_DPKG_DEB  dpkg-deb is installed
HAS_APT       apt-get is installed

Command line (used by shell tests): python3 test_support.py key DIR
writes DIR/secret.asc and DIR/meshtown.gpg.
"""

import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

KEY_UID = "Test <test@example.invalid>"

HAS_GPG = all(shutil.which(tool) for tool in ("gpg", "gpgv", "gpgconf"))
HAS_DPKG_DEB = shutil.which("dpkg-deb") is not None
HAS_APT = shutil.which("apt-get") is not None


def _gpg(home: Path, *args: str) -> bytes:
    cmd = ["gpg", "--homedir", str(home), "--batch", "--no-tty", "--pinentry-mode", "loopback",
           "--passphrase", "", *args]
    done = subprocess.run(cmd, capture_output=True)
    if done.returncode != 0:
        raise RuntimeError(f"{' '.join(cmd)} failed: {done.stderr.decode(errors='replace')}")
    return done.stdout


def make_key(tmp: Path) -> tuple[Path, Path]:
    """Generates a passwordless ed25519 signing key; returns (secret.asc, meshtown.gpg).

    secret.asc is the armored secret key, meshtown.gpg is the public key as plain OpenPGP
    packets (the output of `gpg --export`, not a keybox). The key is made in a temporary
    home directory of its own (short path: gpg-agent sockets live there) removed afterwards.
    """
    tmp = Path(tmp)
    tmp.mkdir(parents=True, exist_ok=True)
    home = Path(tempfile.mkdtemp(prefix="gk-"))
    os.chmod(home, 0o700)
    try:
        _gpg(home, "--quick-gen-key", KEY_UID, "ed25519", "sign", "never")
        secret = tmp / "secret.asc"
        public = tmp / "meshtown.gpg"
        secret.write_bytes(_gpg(home, "--armor", "--export-secret-keys"))
        public.write_bytes(_gpg(home, "--export"))
        return secret, public
    finally:
        subprocess.run(["gpgconf", "--homedir", str(home), "--kill", "gpg-agent"], capture_output=True)
        shutil.rmtree(home, ignore_errors=True)


def fake_binary(tmp: Path) -> Path:
    """An executable file of a few bytes named meshtown."""
    tmp = Path(tmp)
    tmp.mkdir(parents=True, exist_ok=True)
    path = tmp / "meshtown"
    path.write_bytes(b"#!/bin/sh\necho fake meshtown\n")
    path.chmod(0o755)
    return path


def main(argv: list[str]) -> int:
    if len(argv) != 3 or argv[1] != "key":
        print("usage: test_support.py key DIR", file=sys.stderr)
        return 2
    make_key(Path(argv[2]))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
