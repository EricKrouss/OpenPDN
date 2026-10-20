#!/usr/bin/env python3
"""Build and record native Metal evidence on an actual Mac; never emulate it."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys
import tarfile

root = Path(__file__).resolve().parent.parent
parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument("--skip-ui", action="store_true", help="Record a headless run without claiming window/presentation validation")
args = parser.parse_args()
if platform.system() != "Darwin":
    sys.exit("Native Metal validation must run on macOS. No checks were recorded as passing.")
for tool in ("cargo", "rustc", "xcrun"):
    if not shutil.which(tool):
        sys.exit(f"Missing {tool}: install Rust 1.95+ and Xcode Command Line Tools before running this script.")
stamp = datetime.datetime.now(datetime.timezone.utc).strftime("%Y%m%dT%H%M%SZ")
output = root / "artifacts" / ("metal-validation-" + stamp)
output.mkdir(parents=True)
env = os.environ.copy()
env["WGPU_BACKEND"] = "metal"
env.setdefault("CARGO_BUILD_JOBS", "2")
env.setdefault("MACOSX_DEPLOYMENT_TARGET", "11.0")
# Isolate smoke-run preferences from the user's normal editor workspace.
env["XDG_CONFIG_HOME"] = str(output / "config")
env["XDG_DATA_HOME"] = str(output / "data")
env["OPENPDN_STATE_PATH"] = str(output / "workspace.ron")
report = {"platform": platform.platform(), "architecture": platform.machine(),
          "backend_requested": "Metal", "feature_complete": False,
          "ui_requested": not args.skip_ui, "checks": [], "success": False}

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def refresh_source_timestamps(root, files):
    # Deterministic ZIP overlays reset even unchanged files to 1980. Preserve the
    # last verified timestamp for unchanged inputs, including after a run fails
    # before building every profile. Hash-only tracking can reuse an old release
    # binary after debug checks passed against newer source.
    state = root / "target/openpdn-source-state.json"
    previous = json.loads(state.read_text()) if state.exists() else {}
    current = {}
    for name, checksum in files.items():
        path = root / name
        if name.startswith(("src/", "tests/", "examples/")) or name in ("Cargo.toml", "Cargo.lock"):
            prior = previous.get(name)
            if isinstance(prior, dict) and prior.get("sha256") == checksum and isinstance(prior.get("mtime_ns"), int):
                stamp = prior["mtime_ns"]
                os.utime(path, ns=(stamp, stamp))
            else:
                os.utime(path, None)
        current[name] = {"sha256": checksum, "mtime_ns": path.stat().st_mtime_ns}
    state.parent.mkdir(exist_ok=True)
    state.write_text(json.dumps(current, sort_keys=True) + "\n")

def run(name, command):
    print(f"{name}: {' '.join(map(str, command))}", flush=True)
    log = output / (name + ".log")
    with log.open("w") as stream:
        result = subprocess.run(list(map(str, command)), cwd=root, env=env, stdout=stream, stderr=subprocess.STDOUT)
    report["checks"].append({"name": name, "exit_code": result.returncode, "log": log.name})
    if result.returncode:
        print(log.read_text(errors="replace")[-12000:], flush=True)
        raise RuntimeError(f"{name} failed; see {log}")
    return log.read_text(errors="replace")

try:
    manifest = root / "source-manifest.json"
    if manifest.exists():
        files = json.loads(manifest.read_text())["files"]
        for name, want in files.items():
            if digest(root / name) != want:
                raise RuntimeError(f"Source bundle checksum mismatch: {name}")
        report["source_manifest_sha256"] = digest(manifest)
        # ZIP entries deliberately have stable timestamps. When a newer bundle
        # overlays an existing checkout, Cargo's mtime checks can otherwise reuse
        # an older shader/binary despite changed contents. Track verified hashes
        # and advance the timestamps of changed compiler inputs before building.
        refresh_source_timestamps(root, files)
    report["lockfile_sha256"] = digest(root / "Cargo.lock")
    run("system", ["sw_vers"])
    run("displays", ["system_profiler", "SPDisplaysDataType"])
    run("rustc", ["rustc", "--version", "--verbose"])
    run("sdk", ["xcrun", "--show-sdk-version"])
    run("source-cache-check", [sys.executable, root / "tools/test-validation-timestamps.py"])
    run("failure-report-check", [sys.executable, root / "tools/test-validation-report.py"])
    run("format", ["cargo", "fmt", "--all", "--check"])
    run("clippy", ["cargo", "clippy", "--locked", "--all-targets", "--", "-D", "warnings"])
    run("standard-tests", ["cargo", "test", "--locked"])
    run("release-build", ["cargo", "build", "--locked", "--release"])
    binary = root / "target" / "release" / "openpdn"
    report["binary_sha256"] = digest(binary)
    adapter = run("gpu-info", [binary, "--gpu-info"]).strip()
    report["adapter"] = adapter
    fields = adapter.split(" | ")
    if len(fields) != 3 or fields[1] != "Metal" or fields[2] not in ("IntegratedGpu", "DiscreteGpu"):
        raise RuntimeError(f"Expected a native Metal hardware adapter, got {adapter!r}")
    run("hardware-tests", ["cargo", "test", "--locked", "--", "--ignored", "--nocapture"])
    hardware_targets = ["gpu", "editing_commands", "filters", "gaussian", "tool_blend", "neighborhood", "remaining_effects", "transform"]
    release_tests = ["cargo", "test", "--locked", "--release"]
    for target in hardware_targets:
        release_tests += ["--test", target]
    run("release-hardware-tests", release_tests + ["--", "--ignored", "--nocapture"])
    run("pdn-render", [binary, "--render", root / "tests/fixtures/pdn/small.pdn", output / "pdn-render.png"])
    if not args.skip_ui:
        run("window-smoke", [binary, "--smoke-frames", "10"])
        env["OPENPDN_FLOAT_UTILITIES"] = "1"
        floating = run("floating-window-smoke", [binary, "--smoke-frames", "20"])
        env.pop("OPENPDN_FLOAT_UTILITIES")
        for name in ("Tools", "History", "Layers", "Colors"):
            if f"native utility: {name} (Immediate)" not in floating:
                raise RuntimeError(f"Native {name} viewport was not created")
    run("icon-atlas", ["cargo", "run", "--locked", "--release", "--example", "icon_atlas", "--", "artifacts/icons.svg"])
    run("package", [sys.executable, root / "tools/package-native.py"])
    if not args.skip_ui:
        target = "macos-arm64" if platform.machine() == "arm64" else "macos-x86_64"
        package_name = "openpdn-0.1.0-dev-" + target
        extracted = output / "zip-extracted"
        run("zip-extraction", ["ditto", "-xk", root / "artifacts" / (package_name + ".zip"), extracted])
        app = extracted / package_name / "OpenPDN.app"
        app_log = output / "app-bundle.log"
        run("app-bundle-smoke", ["open", "-W", "-n", "--env", "WGPU_BACKEND=metal",
             "--env", "OPENPDN_STATE_PATH=" + str(output / "app-workspace.ron"),
             "--stdout", app_log, "--stderr", app_log, app, "--args", "--smoke-frames", "10"])
        # LaunchServices returning zero alone cannot prove that the app started.
        if "Metal" not in app_log.read_text(errors="replace"):
            raise RuntimeError("App bundle launch did not report its Metal adapter; inspect app-bundle.log")
    report["success"] = True
    # Keep a stable evidence snapshot inside the final package. The result.json
    # written below can additionally record the successful repackaging step.
    evidence = output / "native-checks.json"
    evidence.write_text(json.dumps(report, indent=2) + "\n")
    run("validated-package", [sys.executable, root / "tools/package-native.py", "--validation-result", evidence])
except (OSError, RuntimeError) as error:
    report["success"] = False
    report["error"] = str(error)
    print(error, file=sys.stderr)
finally:
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    archive = output.with_suffix(".tar.gz")
    with tarfile.open(archive, "w:gz") as bundle:
        for path in sorted(output.iterdir()):
            if path.is_file():
                bundle.add(path, arcname=output.name + "/" + path.name)
    print(f"Evidence: {archive}", flush=True)
    print("These checks do not certify complete Paint.NET behavior or UI parity.", flush=True)
sys.exit(0 if report["success"] else 1)
