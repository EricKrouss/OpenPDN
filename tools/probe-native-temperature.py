#!/usr/bin/env python3
"""Bounded, offline CPU oracle. Requires reference/probe-python's unicorn package.

Reads the separately obtained Microsoft DLL, never loads or executes a Windows
process. Only emulates CalculateGains and its arithmetic helpers, with a bounded
instruction count; sqrtf is supplied by the host. No DLL bytes are exported.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import UC_X86_REG_RCX, UC_X86_REG_RDX, UC_X86_REG_R8, UC_X86_REG_RSP, UC_X86_REG_RIP, UC_X86_REG_XMM0

root = Path(__file__).resolve().parents[1]
data = (root / "reference/native-d2d/d2d1.dll").read_bytes()
assert hashlib.sha256(data).hexdigest() == "b1f99c4fd6cc6312f1991fea07363c71df09c47005e9eac488c7cafd399ba498"
pe = struct.unpack_from("<I", data, 0x3c)[0]
sections = struct.unpack_from("<H", data, pe + 6)[0]
optional = struct.unpack_from("<H", data, pe + 20)[0]
base = 0x180000000
uc = Uc(UC_ARCH_X86, UC_MODE_64)
uc.mem_map(base, 0x600000)
for i in range(sections):
    off = pe + 24 + optional + 40 * i
    _, rva, size, raw = struct.unpack_from("<IIII", data, off + 8)
    uc.mem_write(base + rva, data[raw:raw + size])
uc.mem_map(0x100000, 0x20000)
stop = 0x100000


def sqrt_hook(machine, address, size, user):
    value = struct.unpack("<f", (machine.reg_read(UC_X86_REG_XMM0) & 0xffffffff).to_bytes(4, "little"))[0]
    machine.reg_write(UC_X86_REG_XMM0, int.from_bytes(struct.pack("<f", math.sqrt(value)), "little"))
    rsp = machine.reg_read(UC_X86_REG_RSP)
    machine.reg_write(UC_X86_REG_RIP, struct.unpack("<Q", machine.mem_read(rsp, 8))[0])
    machine.reg_write(UC_X86_REG_RSP, rsp + 8)


uc.hook_add(UC_HOOK_CODE, sqrt_hook, begin=base + 0xdfcfa, end=base + 0xdfcfa)
samples = []
for t in sorted(set(range(-100, 101, 10)) | {-99, -75, -1, 1, 75, 99}):
    for tint in sorted(set(range(-100, 101, 10)) | {-1, 1}):
        uc.mem_write(0x101008, struct.pack("<ff", t / 100, tint / 100))
        uc.mem_write(0x110008, struct.pack("<Q", stop))
        for reg, value in [(UC_X86_REG_RCX, 0x101000), (UC_X86_REG_RDX, 0x102000), (UC_X86_REG_R8, 0x102004), (UC_X86_REG_RSP, 0x110008)]:
            uc.reg_write(reg, value)
        uc.emu_start(base + 0x229b58, stop, count=20000)
        assert uc.reg_read(UC_X86_REG_RIP) == stop
        samples.append([t, tint, *struct.unpack("<ff", uc.mem_read(0x102000, 8))])
output = root / "tests/fixtures/native-temperature.json"
output.write_text(json.dumps(samples, indent=2) + "\n")
print(f"Wrote {len(samples)} native CPU gain pairs to {output}")
