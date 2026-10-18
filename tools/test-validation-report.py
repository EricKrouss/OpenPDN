#!/usr/bin/env python3
"""Verify failure reporting with simulated commands; no hardware claim is made."""
import contextlib
import io
import json
from pathlib import Path
import runpy
import subprocess
import sys
import tempfile
from unittest import mock

source = Path(__file__).with_name("validate-macos.py")
for failed_check in ("standard-tests", "validated-package"):
    with tempfile.TemporaryDirectory(prefix="openpdn-validation-report-") as temporary:
        root = Path(temporary)
        script = root / "tools/validate-macos.py"
        script.parent.mkdir()
        script.write_bytes(source.read_bytes())
        (root / "Cargo.lock").write_text("simulated lockfile\n")
        binary = root / "target/release/openpdn"
        binary.parent.mkdir(parents=True)
        binary.write_bytes(b"simulated executable, never launched")

        def simulated_run(command, **kwargs):
            log = Path(kwargs["stdout"].name).stem
            if log == "gpu-info":
                kwargs["stdout"].write("Simulated adapter | Metal | IntegratedGpu\n")
            return subprocess.CompletedProcess(command, 1 if log == failed_check else 0)

        with mock.patch("platform.system", return_value="Darwin"), \
             mock.patch("platform.machine", return_value="arm64"), \
             mock.patch("platform.platform", return_value="Simulated-macOS"), \
             mock.patch("shutil.which", return_value="simulated-tool"), \
             mock.patch("subprocess.run", side_effect=simulated_run), \
             mock.patch.object(sys, "argv", [str(script), "--skip-ui"]), \
             contextlib.redirect_stdout(io.StringIO()), \
             contextlib.redirect_stderr(io.StringIO()):
            try:
                runpy.run_path(str(script), run_name="__main__")
            except SystemExit as error:
                assert error.code == 1, (failed_check, error.code)
            else:
                raise AssertionError("Failed validation did not exit")
        report = json.loads(next((root / "artifacts").glob("*/result.json")).read_text())
        assert report["success"] is False, failed_check
        assert failed_check in report["error"]
        assert report["checks"][-1] == {
            "name": failed_check, "exit_code": 1, "log": failed_check + ".log"
        }
print("Validation reports failure before building and after the native-checks snapshot")
