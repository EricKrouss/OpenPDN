#!/usr/bin/env python3
"""Bounded, offline CPU oracle. Requires reference/probe-python's unicorn package.

Reads the separately obtained Microsoft DLL, never loads or executes a Windows
process. Only emulates CTurbulence::InitializeGridLevel10 and its arithmetic helpers, with a bounded
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
samples=[]
for seed in [0,1,2,17,128,255]:
    uc.mem_write(0x101000,bytes(0x3100))
    uc.mem_write(0x101044,struct.pack("<I",seed))
    uc.mem_write(0x110008,struct.pack("<Q",stop))
    uc.reg_write(UC_X86_REG_RCX,0x101000)
    uc.reg_write(UC_X86_REG_RSP,0x110008)
    uc.emu_start(base+0x2159f8,stop,count=1000000)
    assert uc.reg_read(UC_X86_REG_RIP)==stop
    permutation=[struct.unpack("<I",uc.mem_read(0x101070+16*i,4))[0] for i in range(256)]
    gradients=[list(struct.unpack("<4f",uc.mem_read(0x102070+16*i,16))) for i in range(512)]
    samples.append(dict(seed=seed,permutation=permutation,gradients=gradients))
output=root/"tests/fixtures/native-turbulence.json"
output.write_text(json.dumps(samples)+"\n")
print(f"Wrote {len(samples)} native Turbulence grids to {output}")
