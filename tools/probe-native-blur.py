#!/usr/bin/env python3
"""Bounded, offline native Direct2D Gaussian optimization/tap oracle.

Requires reference/probe-python's Unicorn. Native DLL code stays in reference;
only numeric parameters, transforms and kernel taps become test fixtures.
Host ceil/pow/exp implement the external CRT calls, rounded to float32.
"""
import hashlib
import json
import math
from pathlib import Path
import struct
from unicorn import Uc, UC_ARCH_X86, UC_MODE_64, UC_HOOK_CODE
from unicorn.x86_const import (UC_X86_REG_RAX, UC_X86_REG_RCX, UC_X86_REG_R8,
                              UC_X86_REG_RSP, UC_X86_REG_RIP,
                              UC_X86_REG_XMM0, UC_X86_REG_XMM1, UC_X86_REG_XMM3)

ROOT = Path(__file__).resolve().parents[1]
data = (ROOT / "reference/native-d2d/d2d1.dll").read_bytes()
assert hashlib.sha256(data).hexdigest() == "b1f99c4fd6cc6312f1991fea07363c71df09c47005e9eac488c7cafd399ba498"
pe = struct.unpack_from("<I", data, 0x3c)[0]
sections = struct.unpack_from("<H", data, pe+6)[0]
optional = struct.unpack_from("<H", data, pe+20)[0]
base = 0x180000000
uc = Uc(UC_ARCH_X86, UC_MODE_64)
uc.mem_map(base, 0x600000)
for i in range(sections):
    offset = pe+24+optional+40*i
    _, rva, size, raw = struct.unpack_from("<IIII", data, offset+8)
    uc.mem_write(base+rva, data[raw:raw+size])
uc.mem_map(0x100000, 0x20000)
stop, dispatch = 0x100000, 0x100100
instance, vtable, array, taps = 0x101000, 0x102000, 0x103000, 0x104000
stack, pre, post, pre_matrix, post_matrix = 0x110008, 0x108000, 0x108010, 0x108020, 0x108040
uc.mem_write(base+0x313898, struct.pack("<Q", dispatch))
uc.mem_write(instance, struct.pack("<Q", vtable))
uc.mem_write(instance+8, b"\1")
uc.mem_write(vtable+0x30, struct.pack("<Q", base+0xd6da0))

def float_reg(reg):
    return struct.unpack("<f", (uc.reg_read(reg)&0xffffffff).to_bytes(4,"little"))[0]

def set_float(reg, value):
    uc.reg_write(reg, int.from_bytes(struct.pack("<f", value), "little"))

def return_to_caller():
    rsp = uc.reg_read(UC_X86_REG_RSP)
    uc.reg_write(UC_X86_REG_RIP, struct.unpack("<Q", uc.mem_read(rsp,8))[0])
    uc.reg_write(UC_X86_REG_RSP, rsp+8)

def hook(machine, address, size, user):
    if address == dispatch:
        uc.reg_write(UC_X86_REG_RIP, uc.reg_read(UC_X86_REG_RAX))
        return
    if address == base+0xdfc46:
        set_float(UC_X86_REG_XMM0, math.ceil(float_reg(UC_X86_REG_XMM0)))
    elif address == base+0xdfcca:
        set_float(UC_X86_REG_XMM0, math.pow(float_reg(UC_X86_REG_XMM0),float_reg(UC_X86_REG_XMM1)))
    elif address == base+0xdfc6a:
        set_float(UC_X86_REG_XMM0, math.exp(float_reg(UC_X86_REG_XMM0)))
    elif address == base+0xbef38:
        # This routine allocates a CArray<KERNEL_TAP>, whose layout is ptr/count/capacity.
        from unicorn.x86_const import UC_X86_REG_RDX
        count = uc.reg_read(UC_X86_REG_RDX)
        assert count <= 256
        target = uc.reg_read(UC_X86_REG_RCX)
        uc.mem_write(target, struct.pack("<QII", taps, count, count))
        uc.reg_write(UC_X86_REG_RAX, 0)
    elif address != base+0xd9bc0:
        raise AssertionError(hex(address))
    return_to_caller()

for address in [dispatch,base+0xdfc46,base+0xdfcca,base+0xdfc6a,base+0xbef38,base+0xd9bc0]:
    uc.hook_add(UC_HOOK_CODE, hook, begin=address, end=address)

def invoke(rva, sigma):
    uc.mem_write(stack, struct.pack("<Q",stop))
    for reg, value in [(UC_X86_REG_RCX,instance),(UC_X86_REG_RSP,stack)]:
        uc.reg_write(reg,value)
    set_float(UC_X86_REG_XMM1,sigma)
    uc.emu_start(base+rva,stop,count=20000)
    assert uc.reg_read(UC_X86_REG_RIP)==stop

cases=[]
for quality in range(1,4):
    uc.mem_write(instance+0x90,struct.pack("<I",quality-1))
    for sigma in [0.01,1/3,2/3,1,1.4999,1.5,1.5001,2,2.0001,3,5,10,25,50,100]:
        uc.reg_write(UC_X86_REG_R8,0)
        set_float(UC_X86_REG_XMM3,0)
        uc.mem_write(stack+40,struct.pack("<QQQQ",pre,post,pre_matrix,post_matrix))
        invoke(0xbefa0,sigma)
        before = struct.unpack("<f",uc.mem_read(pre,4))[0]
        after = struct.unpack("<f",uc.mem_read(post,4))[0]
        matrices = [list(struct.unpack("<6f",uc.mem_read(p,24))) for p in [pre_matrix,post_matrix]]
        uc.reg_write(UC_X86_REG_R8,array)
        effective = struct.unpack("<f",struct.pack("<f",sigma))[0]*before
        invoke(0xbe340,effective)
        address,count,_=struct.unpack("<QII",uc.mem_read(array,16))
        kernel=[list(struct.unpack("<4f",uc.mem_read(address+i*16,16))) for i in range(count)]
        assert abs(sum(tap[2] for tap in kernel)*2-1)<1e-6
        cases.append(dict(quality=quality,sigma=sigma,pre=before,post=after,matrices=matrices,taps=kernel))
output=ROOT/"tests/fixtures/native-gaussian.json"
output.write_text(json.dumps(cases,indent=2)+"\n")
print(f"Wrote {len(cases)} native Gaussian plans and paired kernels to {output}")
