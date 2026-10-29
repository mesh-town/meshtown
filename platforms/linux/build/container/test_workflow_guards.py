"""Guards on the workflows that build and publish the container image (the files are read as text).

Publishing needs a release tag and an explicit push; the reusable workflow asks for no permissions of its own, so
a dry run called with fewer rights still starts.
Run from the tree root: python3 -m unittest discover -s platforms/linux/build/container -t platforms/linux/build/container
"""

import re
import unittest
from collections import namedtuple
from pathlib import Path

WORKFLOWS = Path(__file__).resolve().parents[4] / ".github" / "workflows"
Step = namedtuple("Step", "name condition script text")


class Publishing(unittest.TestCase):
    def test_publish_steps_are_guarded(self):
        steps = image_steps()
        for marker in ("skopeo login", "docker://", "gh release upload"):
            with self.subTest(marker=marker):
                # given
                publishing = [s for s in steps if marker in s.script]
                # then
                self.assertTrue(publishing, f"no step runs '{marker}'")
                for step in publishing:
                    self.assertIn("inputs.push", step.condition, f"{step.name}: runs without the push input")
                    self.assertIn("refs/tags/v", step.condition, f"{step.name}: runs outside a release tag")

    def test_reusable_workflow_asks_no_permissions(self):
        # given
        lines = code_lines(read("container-image.yml"))
        # then
        self.assertFalse([line for line in lines if re.match(r"\s*permissions\s*:", line)],
                         "a called workflow cannot ask for more than its caller grants")

    def test_dispatch_cannot_publish(self):
        # given
        text = read("container-image.yml")
        # then
        self.assertIn("push", trigger_inputs(text, "workflow_call"))
        self.assertNotIn("push", trigger_inputs(text, "workflow_dispatch"))
        self.assertIn("tag", trigger_inputs(text, "workflow_dispatch"))


class Downloads(unittest.TestCase):
    def test_download_steps_follow_from_artifacts(self):
        # given
        steps = image_steps()
        artifact = [s for s in steps if "download-artifact" in s.text]
        release = [s for s in steps if "gh release download" in s.script]
        # then
        self.assertEqual((len(artifact), len(release)), (1, 1), "one step per source")
        self.assertIn("inputs.from_artifacts", artifact[0].condition)
        self.assertNotIn("!", artifact[0].condition.replace("!=", ""), "artifacts are used when from_artifacts is true")
        self.assertRegex(release[0].condition, r"!\s*inputs\.from_artifacts")
        self.assertIn("sha256sum -c", release[0].script, "a downloaded archive must be checked against SHA256SUMS")


class Callers(unittest.TestCase):
    def test_callers_grant_only_what_they_need(self):
        # given
        release = job_block(read("release.yml"), "container-image")
        build = job_block(read("build.yml"), "container")
        # then
        self.assertRegex(release, r"packages:\s*write")
        self.assertRegex(release, r"contents:\s*read")
        self.assertRegex(release, r"push:\s*true")
        self.assertIn("./.github/workflows/container-image.yml", release)
        self.assertRegex(build, r"push:\s*false")
        self.assertRegex(build, r"contents:\s*read")
        self.assertNotRegex(build, r"packages\s*:", "a dry run must not ask to write packages")
        self.assertRegex(build, r"from_artifacts:\s*true")
        self.assertIn("./.github/workflows/container-image.yml", build)


# ---------- reading workflow text ----------

def read(name):
    return (WORKFLOWS / name).read_text()


def code_lines(text):
    return [line for line in text.splitlines() if not line.lstrip().startswith("#")]


def image_steps():
    """The steps of container-image.yml, split at the list items under `steps:`."""
    lines = code_lines(read("container-image.yml"))
    start = next(i for i, line in enumerate(lines) if re.match(r"\s*steps\s*:\s*$", line))
    bullet = re.match(r"\s*", lines[start + 1]).end()
    blocks, current = [], []
    for line in lines[start + 1:]:
        if line.strip() and re.match(r"\s*", line).end() < bullet:
            break
        if line[bullet:bullet + 2] == "- " and line[:bullet].strip() == "" and current:
            blocks.append(current)
            current = []
        current.append(line)
    return [step(block) for block in [*blocks, current] if block]


def step(block):
    text = "\n".join(block)
    name = re.search(r"-\s+name:\s*(.+)", text)
    condition = re.search(r"^\s+if:\s*(.+)$", text, re.M)
    script = re.search(r"^\s+run:\s*[|>][-+]?\s*\n((?:.*\n?)*)", text, re.M)
    inline = re.search(r"^\s+run:\s*([^|>\s].*)$", text, re.M)
    return Step(name.group(1) if name else block[0].strip(), condition.group(1) if condition else "",
                (script.group(1) if script else "") + (inline.group(1) if inline else ""), text)


def trigger_inputs(text, trigger):
    """Names of the inputs a trigger (workflow_call or workflow_dispatch) declares."""
    lines = code_lines(text)
    start = next(i for i, line in enumerate(lines) if re.match(rf"  {trigger}\s*:", line))
    names, in_inputs = [], False
    for line in lines[start + 1:]:
        if line.strip() and not line.startswith("    "):
            break
        in_inputs = in_inputs or re.match(r"    inputs\s*:", line) is not None
        found = re.match(r"      ([A-Za-z_][\w-]*)\s*:\s*$", line)
        if in_inputs and found:
            names.append(found.group(1))
    return names


def job_block(text, job):
    """The text of one job of the `jobs:` map."""
    lines = code_lines(text)
    start = next(i for i, line in enumerate(lines) if re.match(rf"  {re.escape(job)}\s*:\s*$", line))
    end = next((i for i in range(start + 1, len(lines)) if re.match(r"  \S", lines[i])), len(lines))
    return "\n".join(lines[start:end])


if __name__ == "__main__":
    unittest.main()
