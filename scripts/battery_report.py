"""Battery report formatting (`battery.py`): the items table, the phase 1 steps table,
log tails of red items. Accepts objects with the `ItemResult` fields from battery.py
(duck typing — battery is not imported here, so there is no cycle)."""

from __future__ import annotations

from datetime import datetime
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parents[1]


def tail(text: str, n: int) -> str:
    lines = text.splitlines()
    return "\n".join(lines[-n:]) if lines else "(empty)"


def format_table(results: list) -> str:
    rows = ["| item | status | time | summary | log |", "|---|---|---|---|---|"]
    for r in results:
        status = "✅" if r.ok else f"❌ (code {r.returncode})"
        rel_log = r.log_path.relative_to(REPO_ROOT)
        summary = r.summary.replace("|", "\\|")
        rows.append(
            f"| `{r.item.label}` | {status} | {r.duration_sec:.2f}s | {summary} | `{rel_log}` |"
        )
    return "\n".join(rows)


def format_red_sections(results: list) -> str:
    red = [r for r in results if not r.ok]
    if not red:
        return ""
    parts = ["## Red items — log tail (30 lines)"]
    for r in red:
        parts.append(f"### {r.item.label}\n```\n{tail(r.output, 30)}\n```")
    return "\n\n".join(parts)


def format_steps_table(steps: list) -> str:
    rows = ["## Build (phase 1)", "", "| step | status | time | log |", "|---|---|---|---|"]
    for r in steps:
        status = "✅" if r.ok else f"❌ (code {r.returncode})"
        rows.append(
            f"| `{r.item.name}` | {status} | {r.duration_sec:.2f}s | "
            f"`{r.log_path.relative_to(REPO_ROOT)}` |"
        )
    return "\n".join(rows)


def build_report(
    results: list,
    commit: str,
    branch: str,
    generated_at: datetime,
    mode: str = "",
    steps: list | None = None,
) -> str:
    header = (
        f"# Acceptance battery — {generated_at:%Y-%m-%d %H:%M}\n\n"
        f"Commit `{commit}` (`{branch}`)."
    )
    parts = [header + (f"\nMode: {mode}" if mode else ""), format_table(results)]
    if steps:
        parts.append(format_steps_table(steps))
    red = format_red_sections(results + (steps or []))
    if red:
        parts.append(red)
    return "\n\n".join(parts) + "\n"
