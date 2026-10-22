#!/usr/bin/env python3
"""Analysis of the merge battery log (`make test-changed`): flakiness or breakage.

    flaky_log.py [--names] <log>

Flaky tests are failed Rust tests (nextest, `FAIL [` lines in the list after `Summary`) if
only they failed: not `test-js`/`test-py`/`web-check`/…, not compilation and not `creating test list failed`,
and no more than MAX_NAMES distinct names. Prints one line and exits:
  `flaky: <name> <name>` — code 0 (the last segment of the test path, in alphabetical order);
  `not flaky: <reason>` — code 1.
With `--names` and code 0 it prints the full names instead of the line, one per line (for `make test-one`).
"""
import re
import sys

MAX_NAMES = 3
FAIL_RE = re.compile(r"^\s*(?:TRY \d+ )?FAIL \[[^\]]*\]\s+(?:\([^)]*\)\s+)?\S+\s+(\S+)\s*$")
SUMMARY_RE = re.compile(r"^\s*Summary \[.*?, (\d+) failed")
MAKE_RE = re.compile(r"\*\*\* \[(?:\S*:)?([^\]\s]+)\] Error")
OWN_TARGETS = {"test", "test-changed"}


def _other_failure(lines: list[str]) -> str | None:
    for line in lines:
        if "creating test list failed" in line:
            return "test list"
        if "could not compile" in line:
            return "compilation"
        m = MAKE_RE.search(line)
        if m and m.group(1) not in OWN_TARGETS:
            return m.group(1)
    return None


def _failed_per_summary(lines: list[str]) -> tuple[int, set[str]]:
    """The sum of `N failed` from Summary and the names from the list after the first Summary."""
    declared, names, in_tail = 0, set(), False
    for line in lines:
        s = SUMMARY_RE.match(line)
        if s:
            declared += int(s.group(1))
            in_tail = True
            continue
        m = FAIL_RE.match(line)
        if in_tail and m:
            names.add(m.group(1))
    return declared, names


def classify(text: str) -> tuple[bool, list[str] | str]:
    """(True, full names) or (False, reason)."""
    lines = text.splitlines()
    other = _other_failure(lines)
    if other:
        return False, other
    declared, names = _failed_per_summary(lines)
    if not names:
        return False, "no failed Rust tests in the log"
    if declared != len(names):
        return False, f"{declared} failed, {len(names)} names (crash, timeout or leak)"
    if len(names) > MAX_NAMES:
        return False, f"{len(names)} different tests failed (more than {MAX_NAMES})"
    return True, sorted(names)


def main(argv: list[str]) -> int:
    args = [a for a in argv[1:] if a != "--names"]
    if len(args) != 1:
        print("flaky_log.py [--names] <log>", file=sys.stderr)
        return 2
    with open(args[0], encoding="utf-8", errors="replace") as f:
        ok, info = classify(f.read())
    if not ok:
        print(f"not flaky: {info}")
        return 1
    if "--names" in argv:
        print("\n".join(info))
    else:
        print("flaky: " + " ".join(n.rsplit("::", 1)[-1] for n in info))
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
