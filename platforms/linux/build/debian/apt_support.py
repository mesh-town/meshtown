"""Helpers shared by the tests of apt_repo.py: temporary trees, packages, expected index lines, throwaway keys."""

import contextlib
import email.utils
import hashlib
import io
import re
import shutil
import subprocess
import tempfile
from pathlib import Path

import deb
import apt_repo
from test_support import fake_binary, make_key


NOW = 1_760_000_000.0
POOL = "pool/main/m/meshtown"
STUB_KEYRING = b"\x99\x01\x0d stub keyring, only bytes the package must carry\n"
NAME = re.compile(r"^meshtown_(\d+)\.(\d+)\.(\d+)_(amd64|arm64)\.deb$")
RELEASE_HEADER = [
    "Origin: Mesh Town",
    "Label: Mesh Town",
    "Suite: stable",
    "Codename: stable",
    "Date: {date}",
    "Architectures: amd64 arm64",
    "Components: main",
    "Description: Mesh Town packages",
    "Acquire-By-Hash: yes",
]
INDEX_PATHS = [f"main/binary-{arch}/{name}" for arch in ("amd64", "arm64") for name in ("Packages", "Packages.gz")]
_key_dir = None
_keys = {}


def remove_keys():
    global _key_dir
    if _key_dir is not None:
        shutil.rmtree(_key_dir, ignore_errors=True)
    _key_dir = None
    _keys.clear()


class Env:
    """A temporary tree: tmp/repo (empty), tmp/incoming (empty), a stub public key."""

    def __init__(self, test):
        self.tmp = Path(tempfile.mkdtemp(prefix="aptrepo-"))
        self.tmp.chmod(0o755)  # apt drops to the _apt user when run as root and must be able to read the tree
        test.addCleanup(shutil.rmtree, self.tmp, ignore_errors=True)
        self.repo = self.tmp / "repo"
        self.repo.mkdir()
        self.incoming = self.tmp / "incoming"
        self.incoming.mkdir()
        self.pubkey = self.tmp / "pub.gpg"
        self.pubkey.write_bytes(STUB_KEYRING)
        self.pool = self.repo / POOL


def setup(test) -> Env:
    return Env(test)


def build_debs(test, out: Path, versions: dict, payload: bytes | None = None) -> list:
    """Builds meshtown_<version>_<arch>.deb for every version of every arch into out; returns the paths."""
    scratch = Path(tempfile.mkdtemp(prefix="aptrepo-src-"))
    test.addCleanup(shutil.rmtree, scratch, ignore_errors=True)
    binary = fake_binary(scratch)
    if payload is not None:
        binary.write_bytes(payload)
    keyring = scratch / "meshtown.gpg"
    keyring.write_bytes(STUB_KEYRING)
    return [deb.build_deb(binary, version, arch, out, keyring=keyring)
            for arch, items in versions.items() for version in items]


def pool_map(repo: Path) -> dict:
    """The pool listing per arch in ascending version order, made without apt_repo."""
    listing = {arch: [] for arch in deb.ARCHES}
    for path in (repo / POOL).iterdir():
        match = NAME.match(path.name)
        listing[match.group(4)].append((tuple(int(match.group(i)) for i in (1, 2, 3)), path))
    return {arch: [p for _, p in sorted(items)] for arch, items in listing.items()}


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def index_path(repo: Path, arch: str, name: str = "Packages") -> Path:
    return repo / "dists" / "stable" / "main" / f"binary-{arch}" / name


def hash_dir(repo: Path, arch: str) -> Path:
    return index_path(repo, arch, "by-hash") / "SHA256"


def paragraph(path: Path) -> str:
    """The expected Packages paragraph of a package."""
    lines = deb.read_control(path).rstrip("\n").split("\n")
    at = next(n for n, line in enumerate(lines) if line.startswith("Description:"))
    data = path.read_bytes()
    extra = [f"Filename: {POOL}/{path.name}", f"Size: {len(data)}", f"SHA256: {sha(data)}"]
    return "\n".join(lines[:at] + extra + lines[at:])


def header_lines(ts: float) -> list:
    date = email.utils.formatdate(ts, usegmt=True)
    return [line.format(date=date) for line in RELEASE_HEADER]


def date_of(release: Path) -> str:
    return next(line[len("Date: "):] for line in release.read_text().splitlines() if line.startswith("Date: "))


class Published:
    """A repository built step by step with write_dists: add() puts a version of both arches in the pool."""

    def __init__(self, test):
        self.test = test
        self.env = Env(test)
        self.repo = self.env.repo
        self.clock = NOW

    def add(self, version):
        build_debs(self.test, self.env.pool, {"amd64": [version], "arm64": [version]})

    def publish(self) -> Path:
        self.clock += 60
        return apt_repo.write_dists(self.repo, pool_map(self.repo), self.clock)

    def current(self, arch) -> set:
        return {sha(index_path(self.repo, arch, name).read_bytes()) for name in ("Packages", "Packages.gz")}

    def stored(self, arch) -> set:
        return {p.name for p in hash_dir(self.repo, arch).iterdir()}


def keys(name: str):
    """A throwaway key pair made once per test run: (secret.asc, public keyring)."""
    global _key_dir
    if _key_dir is None:
        _key_dir = Path(tempfile.mkdtemp(prefix="aptrepo-keys-"))
        _key_dir.chmod(0o755)  # readable by the _apt user, see Env
    if name not in _keys:
        _keys[name] = make_key(_key_dir / name)
    return _keys[name]


def published(test) -> Env:
    """An Env whose repository holds a signed publication of 0.3.1 for both arches (needs gpg)."""
    env = setup(test)
    build_debs(test, env.incoming, {"amd64": ["0.3.1"], "arm64": ["0.3.1"]})
    secret, public = keys("a")
    env.pubkey = public
    apt_repo.update(env.repo, env.incoming, 4, secret, pubkey=public, now=NOW)
    return env


def gpgv(test, pubkey: Path, *files: Path):
    home = Path(tempfile.mkdtemp(prefix="mt-gpgv-"))
    test.addCleanup(shutil.rmtree, home, ignore_errors=True)
    return subprocess.run(["gpgv", "--homedir", str(home), "--keyring", str(Path(pubkey).resolve()),
                           *[str(f) for f in files]], capture_output=True, text=True, timeout=60)


def run_main(*argv):
    """apt_repo.main(argv) with stderr captured: (exit code, stderr text)."""
    err = io.StringIO()
    with contextlib.redirect_stderr(err), contextlib.redirect_stdout(io.StringIO()):
        code = apt_repo.main([str(a) for a in argv])
    return code, err.getvalue()
