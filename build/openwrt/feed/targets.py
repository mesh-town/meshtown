"""OpenWrt package architecture labels (the initial set) and ELF checks for the binaries that go into the feed."""
import struct
from pathlib import Path

AARCH64 = "aarch64-unknown-linux-musl"
X86_64 = "x86_64-unknown-linux-musl"
ARMV7 = "armv7-unknown-linux-musleabi"

LABELS = {
    "aarch64_cortex-a53": AARCH64,
    "aarch64_cortex-a72": AARCH64,
    "aarch64_cortex-a76": AARCH64,
    "aarch64_generic": AARCH64,
    "x86_64": X86_64,
    "arm_cortex-a5_vfpv4": ARMV7,
    "arm_cortex-a7": ARMV7,
    "arm_cortex-a7_neon-vfpv4": ARMV7,
    "arm_cortex-a7_vfpv4": ARMV7,
    "arm_cortex-a8_vfpv3": ARMV7,
    "arm_cortex-a9": ARMV7,
    "arm_cortex-a9_neon": ARMV7,
    "arm_cortex-a9_vfpv3-d16": ARMV7,
    "arm_cortex-a15_neon-vfpv4": ARMV7,
}

TRIPLE_MACHINE = {AARCH64: "aarch64", X86_64: "x86_64", ARMV7: "arm"}

_E_MACHINE = {183: "aarch64", 40: "arm", 62: "x86_64"}
_EM_MIPS = 8
_PT_INTERP = 3


def _has_interp(head: bytes, is64: bool, endian: str) -> bool:
    if is64:
        phoff, = struct.unpack_from(endian + "Q", head, 32)
        phentsize, phnum = struct.unpack_from(endian + "HH", head, 54)
    else:
        phoff, = struct.unpack_from(endian + "I", head, 28)
        phentsize, phnum = struct.unpack_from(endian + "HH", head, 42)
    for i in range(phnum):
        at = phoff + i * phentsize
        if at + 4 > len(head):
            return False
        if struct.unpack_from(endian + "I", head, at)[0] == _PT_INTERP:
            return True
    return False


def elf_machine(path) -> str:
    """Machine name of a static ELF file (aarch64, arm, x86_64, mipsel, mips); ValueError for non-ELF, unknown machine or a dynamic binary."""
    head = Path(path).read_bytes()[:65536]
    if len(head) < 64 or head[:4] != b"\x7fELF":
        raise ValueError(f"{path}: not an ELF file")
    is64 = head[4] == 2
    endian = "<" if head[5] == 1 else ">"
    machine, = struct.unpack_from(endian + "H", head, 18)
    name = "mipsel" if endian == "<" else "mips"
    found = name if machine == _EM_MIPS else _E_MACHINE.get(machine)
    if found is None:
        raise ValueError(f"{path}: unsupported ELF machine {machine}")
    if _has_interp(head, is64, endian):
        raise ValueError(f"{path}: dynamically linked (PT_INTERP), a static binary is required")
    return found
