#!/usr/bin/env python3
"""Compatibility entry point for the native Linux development packager."""
import platform
import runpy
from pathlib import Path
assert platform.system()=="Linux","This entry point handles the native Linux build"
runpy.run_path(str(Path(__file__).with_name("package-native.py")),run_name="__main__")
