"""Unpack the release archives of the targets the feed has labels for and write the `--bin` arguments for build_feed.py."""
import argparse
import sys
import tarfile
from pathlib import Path

import targets


def feed_triples() -> list:
    """Rust targets referenced by the label table, in order of first appearance."""
    return list(dict.fromkeys(targets.LABELS.values()))


def unpack(fetched: Path, out: Path, tag: str, strict: bool):
    """Return (bin arguments, error flag); archives that are missing are an error only when strict."""
    version = tag[1:] if tag.startswith("v") else tag
    args = []
    for triple in feed_triples():
        base = f"meshtown-cli-{version}-{triple}"
        archive = fetched / f"{base}.tar.gz"
        if not archive.is_file():
            if strict:
                print(f"::error::release {tag} has no {base}.tar.gz")
                return [], True
            print(f"::warning::release {tag} has no {base}.tar.gz; its labels are skipped")
            continue
        dest = out / triple
        dest.mkdir(parents=True, exist_ok=True)
        with tarfile.open(archive) as tar:
            tar.extractall(dest)
        args += ["--bin", f"{triple}={dest / base / 'meshtown-cli'}"]
    return args, False


def main(argv=None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetched", type=Path, required=True)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--tag", required=True)
    parser.add_argument("--args-file", type=Path, required=True)
    parser.add_argument("--strict", choices=["true", "false"], required=True)
    opts = parser.parse_args(argv)
    args, failed = unpack(opts.fetched, opts.out, opts.tag, opts.strict == "true")
    if failed:
        return 1
    if not args:
        print("::error::no target archives found")
        return 1
    opts.args_file.write_text("".join(f"{a}\n" for a in args))
    return 0


if __name__ == "__main__":
    sys.exit(main())
