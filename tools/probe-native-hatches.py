#!/usr/bin/env python3
"""Execute the original hatch-tile API; export numerical tile coverage and RGBA fixtures."""
import hashlib
import os
from pathlib import Path
import subprocess
ROOT=Path(__file__).resolve().parents[1]
library=ROOT/'reference/native-gdiplus/gdiplus.dll'
assert hashlib.sha256(library.read_bytes()).hexdigest()=='7765be8d676c17aeca32086ba8e33b750fbca0fcc1e85067b5202217c08f0462'
binary=library.parent/'hatch-probe.exe'
output=ROOT/'tests/fixtures/native-hatches.json'
def wine_path(p):return 'Z:'+str(p).replace('/','\\')
subprocess.run(['x86_64-w64-mingw32-g++','-std=c++17','-O2','-static',str(ROOT/'tools/native-hatch-probe.cpp'),'-o',str(binary)],check=True)
subprocess.run(['wine',str(binary),wine_path(library),wine_path(output)],check=True,env={**os.environ,'WINEDLLOVERRIDES':'gdiplus=n','WINEDEBUG':'-all','WINEPREFIX':str(ROOT/'reference/wine-disassembly')})
