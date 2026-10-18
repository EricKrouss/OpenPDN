#!/usr/bin/env python3
"""Link the Linux release against a pinned older glibc ABI with Zig.

Uses Zig 0.14.1, downloaded with a pinned SHA-256 from ziglang.org unless --zig
is provided. This does not install system packages, change Cargo configuration,
or distribute a toolchain. Rust dependencies use the normal locked release build.
"""
import argparse
import hashlib
import platform
import re
import shlex
import shutil
import subprocess
import tarfile
import tempfile
import urllib.request
from pathlib import Path

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--zig", type=Path)
args = parser.parse_args()
assert platform.system() == "Linux" and platform.machine() == "x86_64", "Run on x86_64 Linux"
zig = args.zig.resolve() if args.zig else root / "reference/tools/zig-x86_64-linux-0.14.1/zig"
if not zig.is_file() and args.zig is None:
    tools = zig.parent.parent
    tools.mkdir(parents=True, exist_ok=True)
    archive = tools / "zig-x86_64-linux-0.14.1.tar.xz"
    if not archive.is_file():
        with tempfile.NamedTemporaryFile(dir=tools, suffix=".download", delete=False) as output:
            partial = Path(output.name)
            try:
                with urllib.request.urlopen("https://ziglang.org/download/0.14.1/zig-x86_64-linux-0.14.1.tar.xz", timeout=60) as source:
                    shutil.copyfileobj(source, output)
                output.close()
                partial.replace(archive)
            finally:
                partial.unlink(missing_ok=True)
    assert hashlib.sha256(archive.read_bytes()).hexdigest() == "24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c", "Zig download checksum mismatch"
    with tarfile.open(archive) as source:
        source.extractall(tools, filter="data")
assert subprocess.check_output([str(zig), "version"], text=True).strip() == "0.14.1", "Use the pinned Zig 0.14.1 compiler"
wrapper = root / "reference/tools/portable-linux-linker"
wrapper.parent.mkdir(parents=True, exist_ok=True)
wrapper.write_text("#!/bin/sh\nexec " + shlex.quote(str(zig)) + " cc -target x86_64-linux-gnu.2.35 \"$@\"\n")
wrapper.chmod(0o755)
subprocess.run(["cargo", "rustc", "--locked", "--release", "--bin", "openpdn", "--", "-C", "linker=" + str(wrapper)], cwd=root, check=True)
binary = root / "target/release/openpdn"
symbols = subprocess.check_output(["objdump", "-T", str(binary)], text=True)
versions = {tuple(map(int, v.split("."))) for v in re.findall(r"GLIBC_(\d+\.\d+)", symbols)}
assert versions and max(versions) <= (2, 35), "The binary still imports a newer glibc ABI"
print(f"{binary}: maximum imported glibc version {'.'.join(map(str, max(versions)))}")
