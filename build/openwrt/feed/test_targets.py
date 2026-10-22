"""Tests of targets.py: the label table and the ELF check (no external tools).

Run from the tree root: python3 -m unittest discover -s build/openwrt/feed -t build/openwrt/feed
"""

import unittest

import targets
from feed_fixtures import EXPECTED_LABELS, TRIPLE_MACHINES, fake_elf, tmpdir


class LabelTable(unittest.TestCase):
    def test_has_exactly_the_14_stage_1_labels_with_their_rust_targets(self):
        # given: the expected table, copied into the fixtures
        # when
        table = dict(targets.LABELS)
        # then
        self.assertEqual(table, EXPECTED_LABELS)

    def test_has_no_mips_and_no_later_labels(self):
        # given
        labels = set(targets.LABELS)
        later = {"arm_arm926ej-s", "arm_arm1176jzf-s_vfp", "arm_fa526", "arm_xscale"}
        # when
        offending = [name for name in labels if "mips" in name or name.startswith(("i386", "powerpc", "riscv"))]
        # then
        self.assertEqual(offending, [], "mips, i386, ppc and riscv labels are not in the initial set")
        self.assertEqual(labels & later, set(), "armv5/v6 labels are not in the initial set")

    def test_every_rust_target_has_a_machine(self):
        # given
        used = set(targets.LABELS.values())
        # when
        machines = dict(targets.TRIPLE_MACHINE)
        # then
        self.assertEqual(machines, TRIPLE_MACHINES)
        self.assertLessEqual(used, set(machines), "a label points at a target with no machine")


class ElfMachine(unittest.TestCase):
    def test_reports_the_machine_of_a_static_elf(self):
        for machine in ("aarch64", "arm", "x86_64"):
            with self.subTest(machine=machine):
                # given
                path = fake_elf(tmpdir(self) / "bin", machine)
                # when
                found = targets.elf_machine(path)
                # then
                self.assertEqual(found, machine)


class MipsMachines(unittest.TestCase):
    def test_tells_little_and_big_endian_mips_apart(self):
        for machine in ("mipsel", "mips"):
            with self.subTest(machine=machine):
                # given
                path = fake_elf(tmpdir(self) / "bin", machine)
                # when
                found = targets.elf_machine(path)
                # then
                self.assertEqual(found, machine)

    def test_dynamic_mips_is_refused(self):
        # given
        path = fake_elf(tmpdir(self) / "bin", "mipsel", static=False)
        # when / then
        with self.assertRaises(ValueError):
            targets.elf_machine(path)


class ElfRefusals(unittest.TestCase):
    def test_text_file_is_not_an_elf(self):
        # given
        path = tmpdir(self) / "notes.txt"
        path.write_text("just some text, long enough to hold an ELF header if it were one\n" * 3)
        # when / then
        with self.assertRaises(ValueError):
            targets.elf_machine(path)

    def test_empty_file_is_not_an_elf(self):
        # given
        path = tmpdir(self) / "empty"
        path.write_bytes(b"")
        # when / then
        with self.assertRaises(ValueError):
            targets.elf_machine(path)

    def test_dynamic_elf_with_an_interpreter_is_refused(self):
        for machine in ("aarch64", "arm", "x86_64"):
            with self.subTest(machine=machine):
                # given: PT_INTERP present, the binary needs a libc on the router
                path = fake_elf(tmpdir(self) / "bin", machine, static=False)
                # when / then
                with self.assertRaises(ValueError):
                    targets.elf_machine(path)


if __name__ == "__main__":
    unittest.main()
