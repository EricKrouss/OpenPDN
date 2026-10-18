#!/usr/bin/env python3
"""Round trip original PDNs through Rust, then compare the original reader's output.

Requires the locally extracted Paint.NET 5.1.12 payload and developer-only .NET SDK.
The optional allocation shim supports the original Windows heap imports on Linux.
"""
import json
import os
import subprocess
from pathlib import Path
ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / 'artifacts/pdn-oracle'
OUT.mkdir(parents=True, exist_ok=True)

def run(*command, **kwargs):
    subprocess.run(command, cwd=ROOT, check=True, **kwargs)

run('cc', '-shared', '-fPIC', '-O2', 'tools/pdn-probe/native-heap.c', '-o', 'reference/native-heap.so')
for name in ('small', 'chunks'):
    run('cargo', 'run', '--locked', '--example', 'pdn_roundtrip', '--', f'tests/fixtures/pdn/{name}.pdn', f'artifacts/pdn-oracle/{name}-rust.pdn')
run('cargo', 'run', '--locked', '--example', 'pdn_roundtrip', '--', '--new', 'artifacts/pdn-oracle/new-rust.pdn')
run('cargo', 'run', '--locked', '--example', 'pdn_roundtrip', '--', '--new-resolution', 'artifacts/pdn-oracle/resolution-rust.pdn')
run('dotnet', 'run', '--project', 'tools/pdn-probe', '--', 'reference/extracted/x64', 'reference/native-heap.so', 'artifacts/pdn-oracle', 'verify', env={**os.environ, 'COMPlus_ReadyToRun':'0', 'DOTNET_ReadyToRun':'0'})
for name in ('small','chunks'):
    expected=json.loads((ROOT / f'tests/fixtures/pdn/{name}.json').read_text())
    actual=json.loads((OUT / f'{name}-rust.original-reader.json').read_text())
    assert actual == expected, f'Original reader found differences in {name}'
    print(f'{name}: original reader confirms every pixel, layer name/visibility/opacity/blend and resolution')

new=json.loads((OUT/'new-rust.original-reader.json').read_text())
assert (new['width'],new['height']) == (3,2)
assert new['layers'][0]['pixels'] == [[12,34,56,78]]*6
print('New Rust-created document: original reader confirms dimensions and alpha pixels')
resolution=json.loads((OUT/'resolution-rust.original-reader.json').read_text())
assert (resolution['dpu_x'],resolution['dpu_y']) == (37.795,42.25)
assert resolution['layers'][0]['pixels'] == [[12,34,56,78]]*6
print('New Rust resolution metadata: original reader confirms 37.795 x 42.25 pixels/cm and every alpha pixel')
