#!/usr/bin/env python3
"""Check actual Cargo release freshness across deterministic ZIP overlays."""
import ast
import hashlib
import json
import os
from pathlib import Path
import subprocess
import tempfile

source = Path(__file__).with_name("validate-macos.py")
tree = ast.parse(source.read_text())
function = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "refresh_source_timestamps")
scope = {"json": json, "os": os}
exec(compile(ast.Module(body=[function], type_ignores=[]), str(source), "exec"), scope)
refresh = scope[function.name]
with tempfile.TemporaryDirectory(prefix="openpdn-cargo-freshness-") as temporary:
    root = Path(temporary)
    (root / "src").mkdir()
    (root / "Cargo.toml").write_text('[package]\nname="freshness-probe"\nversion="0.0.0"\nedition="2024"\n[workspace]\n')
    code = root / "src/main.rs"
    def overlay(value):
        code.write_text('fn main() { println!("' + value + '"); }\n')
        files = {str(p.relative_to(root)): hashlib.sha256(p.read_bytes()).hexdigest() for p in (root / "Cargo.toml", code)}
        for name in files:
            os.utime(root / name, (315532800, 315532800))
        refresh(root, files)
    def cargo(*arguments):
        return subprocess.check_output(["cargo", *arguments, "--quiet", "--manifest-path", str(root / "Cargo.toml")], text=True).strip()
    overlay("old")
    assert cargo("run", "--release") == "old"
    overlay("new")
    cargo("check")  # A validation run ends before rebuilding its release binary.
    stamp = code.stat().st_mtime_ns
    overlay("new")
    assert code.stat().st_mtime_ns == stamp
    assert cargo("run", "--release") == "new", "Cargo reused the obsolete release"
    binary = root / "target/release/freshness-probe"
    built = binary.stat().st_mtime_ns
    overlay("new")
    assert cargo("run", "--release") == "new"
    assert binary.stat().st_mtime_ns == built, "Identical source forced an unnecessary rebuild"
print("Cargo release freshness survives an interrupted validation and repeated stable-timestamp overlays")
