#!/usr/bin/env python3
"""Run the original GDI+ RedEyeCorrection; never bundle it with the Rust app.

Requires Wine, MinGW and the separately obtained, hash-checked Microsoft DLL.
An optional breakpoint records native analysis values before pixel correction.
Only numerical reference data is exported. No native instructions are copied.
"""
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
LIBRARY = ROOT / "reference/native-gdiplus/gdiplus.dll"
assert hashlib.sha256(LIBRARY.read_bytes()).hexdigest() == "7765be8d676c17aeca32086ba8e33b750fbca0fcc1e85067b5202217c08f0462"
BINARY = LIBRARY.parent / "probe.exe"
RAW = LIBRARY.parent / "analysis.bin"
OUTPUT = ROOT / "tests/fixtures/native-redeye.json"
def wine_path(path):
    return "Z:" + str(path).replace("/", "\\")
subprocess.run(["x86_64-w64-mingw32-g++", "-std=c++17", "-O2", "-static", str(ROOT/"tools/native-redeye-probe.cpp"), "-o", str(BINARY)], check=True)
subprocess.run(["wine", str(BINARY), wine_path(LIBRARY), wine_path(OUTPUT), wine_path(RAW)], check=True, env={**os.environ, "WINEDLLOVERRIDES":"gdiplus=n", "WINEDEBUG":"-all", "WINEPREFIX":str(ROOT/"reference/wine-disassembly")})
data=RAW.read_bytes()
assert len(data)==50*0x45c
samples=[]
for i in range(50):
    record=data[i*0x45c:(i+1)*0x45c]
    f=lambda offset:struct.unpack_from("<f",record,offset)[0]
    u=lambda offset:struct.unpack_from("<I",record,offset)[0]
    assert u(0x458)&0x6000==0x2000, "Native analysis did not use round-down arithmetic"
    span=u(0x88)-u(0x84)
    factor=(-.75 if span>=81 else -.66 if span>=41 else -.5 if span>=26 else -.25) if u(0x40) else 1
    samples.append([[0,0,41,33],[f(0x50),f(0x54),f(0x6c),u(0x44)],
                    [f(0x64),f(0x68),u(0x40),factor],[u(0x60),u(0x88),20.5,16.5]]+
                   [[f(0x98+4*j),f(0x188+4*j),f(0x278+4*j),0] for j in range(60)])
(ROOT/"tests/fixtures/native-redeye-analysis.json").write_text(json.dumps(samples,separators=(",",":"))+"\n")
print(f"Recorded {len(samples)} native analysis tables")
