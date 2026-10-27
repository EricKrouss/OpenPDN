#!/usr/bin/env python3
"""Merge two baseline macOS executables, validate and ad-hoc sign the result.

Run on macOS after building each Rust target with MACOSX_DEPLOYMENT_TARGET=11.0.
No build, release upload, Developer ID signing or notarization is performed.
"""
import argparse
import os
from pathlib import Path
import platform
import subprocess
import tempfile

from macos_binary import UNIVERSAL_ARCHITECTURES, validate_macos_binary


def merge(x86_64, arm64, output):
    if platform.system() != "Darwin":
        raise ValueError("Merging/signing requires macOS with Xcode Command Line Tools")
    if output.resolve() in (x86_64.resolve(), arm64.resolve()):
        raise ValueError("Output must not overwrite an input slice")
    for architecture, source in (("x86_64", x86_64), ("arm64", arm64)):
        validate_macos_binary(source.read_bytes(), {architecture})
    output.parent.mkdir(parents=True, exist_ok=True)
    # Do not replace a previously valid output when lipo, codesign or validation fails.
    with tempfile.TemporaryDirectory(prefix=".universal2-", dir=output.parent) as temporary:
        merged = Path(temporary) / "openpdn"
        subprocess.run(["xcrun", "lipo", "-create", str(x86_64), str(arm64), "-output", str(merged)], check=True)
        validate_macos_binary(merged.read_bytes(), UNIVERSAL_ARCHITECTURES)
        architectures = subprocess.check_output(["xcrun", "lipo", "-archs", str(merged)], text=True).split()
        if set(architectures) != UNIVERSAL_ARCHITECTURES:
            raise ValueError(f"lipo architecture verification failed: {architectures}")
        merged.chmod(0o755)
        # lipo/strip can invalidate signatures. Sign only after the final merge;
        # ad-hoc signing supports local execution but is not distribution trust.
        subprocess.run(["codesign", "--force", "--sign", "-", "--identifier", "dev.openpdn.editor", "--timestamp=none", str(merged)], check=True)
        subprocess.run(["codesign", "--verify", "--strict", str(merged)], check=True)
        validate_macos_binary(merged.read_bytes(), UNIVERSAL_ARCHITECTURES)
        os.replace(merged, output)
    print(output)


def main():
    root = Path(__file__).resolve().parent.parent
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--x86_64", type=Path, default=root / "target/x86_64-apple-darwin/release/openpdn")
    parser.add_argument("--arm64", type=Path, default=root / "target/aarch64-apple-darwin/release/openpdn")
    parser.add_argument("--output", type=Path, default=root / "target/universal2-apple-darwin/release/openpdn")
    args = parser.parse_args()
    merge(args.x86_64, args.arm64, args.output)


if __name__ == "__main__":
    main()
