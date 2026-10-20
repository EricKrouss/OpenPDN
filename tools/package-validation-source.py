#!/usr/bin/env python3
"""Create a deterministic, allowlisted source bundle for native Mac validation."""
from pathlib import Path
import hashlib
import json
import zipfile

root = Path(__file__).resolve().parent.parent
files = [root / name for name in ("Cargo.toml", "Cargo.lock", "README.md", "tools/validate-macos.py", "tools/test-validation-timestamps.py", "tools/test-validation-report.py", "tools/package-native.py", "tools/macos_binary.py", "tools/merge-macos-universal.py", "tools/test_macos_binary.py", "tools/test_macos_package.py")]
if (root / "LICENSE").is_file():
    files.append(root / "LICENSE")
for folder in ("src", "tests", "examples", "docs"):
    files += [p for p in sorted((root / folder).rglob("*")) if p.is_file() and "__pycache__" not in p.parts]
contents = {str(p.relative_to(root)): p.read_bytes() for p in files}
assert not any(name.endswith((".dll", ".pdb", ".exe", ".so", ".cs")) for name in contents)
manifest = {"purpose": "Native Metal validation source", "feature_complete": False,
            "files": {name: hashlib.sha256(data).hexdigest() for name, data in sorted(contents.items())}}
contents["source-manifest.json"] = (json.dumps(manifest, indent=2) + "\n").encode()
output = root / "artifacts/openpdn-metal-validation-source.zip"
output.parent.mkdir(exist_ok=True)
with zipfile.ZipFile(output, "w", compression=zipfile.ZIP_DEFLATED, compresslevel=9) as archive:
    for name, data in sorted(contents.items()):
        entry = zipfile.ZipInfo("openpdn-metal-validation/" + name, (1980, 1, 1, 0, 0, 0))
        entry.compress_type = zipfile.ZIP_DEFLATED
        entry.external_attr = 0o100644 << 16
        archive.writestr(entry, data, compresslevel=9)
sha = hashlib.sha256(output.read_bytes()).hexdigest()
output.with_suffix(".zip.sha256").write_text(sha + "  " + output.name + "\n")
print(f"{output}\n{sha}\n{len(files)} source and fixture files; no .NET runtime or reference binaries")
