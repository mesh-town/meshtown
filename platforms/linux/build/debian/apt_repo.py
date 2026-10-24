#!/usr/bin/env python3
"""Keeps a signed apt repository (pool, dists/stable, InRelease, Release.gpg) built from meshtown .deb files."""
import argparse
import email.utils
import gzip
import hashlib
import io
import re
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

import deb

POOL = Path("pool/main/m/meshtown")
NAME = re.compile(r"^meshtown_(\d+)\.(\d+)\.(\d+)_(amd64|arm64)\.deb$")
HEAD = ("Origin: Mesh Town\nLabel: Mesh Town\nSuite: stable\nCodename: stable\n")
TAIL = ("Architectures: amd64 arm64\nComponents: main\nDescription: Mesh Town packages\nAcquire-By-Hash: yes\n")


class RepoError(Exception):
    pass


def sha(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def parse_name(name: str):
    m = NAME.match(name)
    if not m or m.group(4) not in deb.ARCHES:
        return None
    return tuple(int(x) for x in m.groups()[:3]), m.group(4)


def _check_one(path: Path) -> None:
    parsed = parse_name(path.name)
    if parsed is None:
        raise RepoError(f"unexpected package name: {path.name}")
    fields = dict(re.findall(r"^([\w-]+):\s*(.*)$", deb.read_control(path), re.M))
    version = ".".join(map(str, parsed[0]))
    if fields.get("Package") != "meshtown" or fields.get("Version") != version or fields.get("Architecture") != parsed[1]:
        raise RepoError(f"control of {path.name} does not match its name")


def check_incoming(incoming: Path) -> list:
    debs = sorted(Path(incoming).glob("*.deb"))
    if not debs:
        raise RepoError(f"no .deb files in {incoming}")
    for path in debs:
        _check_one(path)
    return debs


def add_to_pool(repo: Path, debs: list) -> None:
    pool = Path(repo) / POOL
    pool.mkdir(parents=True, exist_ok=True)
    for src in debs:
        dst = pool / src.name
        if not dst.exists():
            shutil.copyfile(src, dst)
        elif dst.read_bytes() != src.read_bytes():
            print(f"apt_repo: notice: pool keeps the published {src.name} (incoming differs)", file=sys.stderr)


def pool_by_arch(repo: Path) -> dict:
    out = {arch: [] for arch in sorted(deb.ARCHES)}
    pool = Path(repo) / POOL
    for path in (pool.iterdir() if pool.is_dir() else []):
        parsed = parse_name(path.name)
        if parsed is None:
            raise RepoError(f"unexpected file in pool: {path.name}")
        out[parsed[1]].append((parsed[0], path))
    return {arch: [p for _, p in sorted(items)] for arch, items in out.items()}


def prune_pool(repo: Path, keep: int) -> dict:
    if keep < 1:
        raise RepoError("--keep must be at least 1")
    pool = pool_by_arch(repo)
    for arch, paths in pool.items():
        for old in paths[:-keep]:
            old.unlink()
        pool[arch] = paths[-keep:]
    return pool


def write_direct_links(repo: Path, pool: dict) -> None:
    for arch, paths in pool.items():
        link = Path(repo) / f"meshtown_{arch}.deb"
        if paths:
            shutil.copyfile(paths[-1], link)
        else:
            link.unlink(missing_ok=True)


def packages_index(debs: list) -> bytes:
    paras = []
    for path in debs:
        control = deb.read_control(path).rstrip("\n")
        if "\n\n" in control:
            raise RepoError(f"blank line inside control of {path.name}")
        raw = path.read_bytes()
        extra = f"Filename: {POOL.as_posix()}/{path.name}\nSize: {len(raw)}\nSHA256: {sha(raw)}\n"
        at = control.index("Description:")
        paras.append(control[:at] + extra + control[at:])
    return ("\n\n".join(paras) + "\n").encode() if paras else b""


def gzip_bytes(data: bytes) -> bytes:
    buf = io.BytesIO()
    with gzip.GzipFile(fileobj=buf, mode="wb", mtime=0, compresslevel=9) as g:
        g.write(data)
    return buf.getvalue()


def release_text(files: dict, ts: float) -> str:
    lines = "".join(f" {sha(files[p])} {len(files[p])} {p}\n" for p in files)
    date = email.utils.formatdate(ts, usegmt=True)
    return f"{HEAD}Date: {date}\n{TAIL}SHA256:\n{lines}"


def _release_fields(stable: Path):
    try:
        return (Path(stable) / "Release").read_text().split("SHA256:\n")
    except (OSError, UnicodeDecodeError):
        return None


def previous_hashes(stable: Path) -> dict:
    parts = _release_fields(stable)
    out = {}
    for line in (parts[1].splitlines() if parts and len(parts) > 1 else []):
        fields = line.split()
        if len(fields) != 3:
            continue
        digest, _, rel = fields
        folder = Path(rel).parent
        data = _read_old(Path(stable), folder, rel, digest)
        if data is not None:
            out[(folder.as_posix(), digest)] = data
    return out


def _read_old(stable: Path, folder: Path, rel: str, digest: str):
    for cand in (stable / folder / "by-hash" / "SHA256" / digest, stable / rel):
        if cand.is_file() and sha(cand.read_bytes()) == digest:
            return cand.read_bytes()
    return None


def _old_date(stable: Path):
    parts = _release_fields(stable)
    m = re.search(r"^Date: (.*)$", parts[0], re.M) if parts else None
    try:
        return email.utils.parsedate_to_datetime(m.group(1)).timestamp() if m else None
    except (TypeError, ValueError):
        return None


def write_dists(repo: Path, pool: dict, now: float) -> Path:
    stable = Path(repo) / "dists" / "stable"
    prev, old = previous_hashes(stable), _old_date(stable)
    ts = int(now) if old is None or old < int(now) else int(old) + 1
    files = {}
    for arch in sorted(pool):
        data = packages_index(pool[arch])
        files[f"main/binary-{arch}/Packages"] = data
        files[f"main/binary-{arch}/Packages.gz"] = gzip_bytes(data)
    shutil.rmtree(Path(repo) / "dists", ignore_errors=True)
    for rel, data in files.items():
        _put(stable / rel, data)
        _put(stable / Path(rel).parent / "by-hash" / "SHA256" / sha(data), data)
    for (folder, digest), data in prev.items():
        _put(stable / folder / "by-hash" / "SHA256" / digest, data)
    _put(stable / "Release", release_text(files, ts).encode())
    return stable / "Release"


def _put(path: Path, data: bytes) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)


def _gpg(home: Path, *args, tool="gpg", cwd=None) -> str:
    cmd = [tool, "--homedir", str(home)] + (["--batch"] if tool == "gpg" else []) + [str(a) for a in args]
    return subprocess.run(cmd, check=True, capture_output=True, text=True, cwd=cwd).stdout


def _fingerprints(listing: str) -> list:
    out, after_sec = [], False
    for line in listing.splitlines():
        f = line.split(":")
        if f[0] == "sec":
            after_sec = True
        elif f[0] == "fpr" and after_sec:
            out.append(f[9])
            after_sec = False
        elif f[0] in ("ssb", "pub"):
            after_sec = False
    return out


def sign_release(stable: Path, key: Path) -> None:
    home = Path(tempfile.mkdtemp(prefix="mt-gpg-"))
    try:
        _gpg(home, "--import", Path(key).resolve())
        fprs = _fingerprints(_gpg(home, "--with-colons", "--list-secret-keys"))
        if len(fprs) != 1:
            raise RepoError(f"expected one secret key in {key}, found {len(fprs)}")
        sign = ["--yes", "--local-user", fprs[0], "--digest-algo", "SHA512"]
        _gpg(home, *sign, "--clearsign", "-o", stable / "InRelease", stable / "Release")
        _gpg(home, *sign, "--armor", "--detach-sign", "-o", stable / "Release.gpg", stable / "Release")
    finally:
        subprocess.run(["gpgconf", "--homedir", str(home), "--kill", "gpg-agent"], capture_output=True)
        shutil.rmtree(home, ignore_errors=True)


def verify_release(stable: Path, pubkey: Path) -> None:
    home = Path(tempfile.mkdtemp(prefix="mt-gpg-"))
    try:
        for files in (["InRelease"], ["Release.gpg", "Release"]):
            r = subprocess.run(["gpgv", "--homedir", str(home), "--keyring", str(Path(pubkey).resolve()), *files],
                               cwd=stable, capture_output=True, text=True)
            if r.returncode != 0:
                raise RepoError(f"signature check failed for {files[0]}: {r.stderr.strip()}")
    finally:
        shutil.rmtree(home, ignore_errors=True)


def update(repo, incoming, keep, key, pubkey=deb.KEYRING, now=None) -> None:
    repo, incoming = Path(repo), Path(incoming)
    if keep < 1:
        raise RepoError("--keep must be at least 1")
    for label, f in (("key", key), ("public key", pubkey)):
        if not Path(f).is_file():
            raise RepoError(f"{label} file not found: {f}")
    if not incoming.is_dir():
        raise RepoError(f"incoming folder not found: {incoming}")
    debs = check_incoming(incoming)
    repo.mkdir(parents=True, exist_ok=True)
    add_to_pool(repo, debs)
    pool = prune_pool(repo, keep)
    write_direct_links(repo, pool)
    shutil.copyfile(pubkey, repo / "meshtown.gpg")
    release = write_dists(repo, pool, time.time() if now is None else now)
    sign_release(release.parent, Path(key))
    verify_release(release.parent, Path(pubkey))


class _Parser(argparse.ArgumentParser):
    def error(self, message):
        raise RepoError(message)


def _parser() -> argparse.ArgumentParser:
    p = _Parser(prog="apt_repo.py", description=__doc__)
    sub = p.add_subparsers(dest="cmd", required=True)
    u = sub.add_parser("update", help="add packages from INCOMING to REPO and re-sign it")
    u.add_argument("repo", type=Path)
    u.add_argument("incoming", type=Path)
    u.add_argument("--keep", type=int, required=True, help="versions kept per architecture")
    u.add_argument("--key", type=Path, required=True, help="armored secret key")
    u.add_argument("--pubkey", type=Path, default=deb.KEYRING, help="binary public keyring")
    return p


def main(argv=None) -> int:
    try:
        a = _parser().parse_args(argv)
        update(a.repo, a.incoming, a.keep, a.key, a.pubkey)
    except (RepoError, ValueError, OSError) as e:
        print(f"apt_repo: {e}", file=sys.stderr)
        return 1
    except subprocess.CalledProcessError as e:
        tail = "\n".join((e.stderr or "").splitlines()[-20:])
        print(f"apt_repo: {e.cmd[0]} failed ({e.returncode})\n{tail}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
