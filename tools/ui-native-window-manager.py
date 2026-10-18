#!/usr/bin/env python3
"""Run the native-window gesture scope with KWin on an isolated Xvfb desktop.

Invoke with dbus-run-session and xvfb-run, never against the user's desktop.
The private window manager disables decorations so test coordinates refer to
client pixels. Real OS-managed title-strip movement remains enabled.
"""
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import time
from Xlib import X, display

root = Path(__file__).resolve().parent.parent
assert os.environ.get("OPENPDN_ISOLATED_UI") == "1"
assert os.environ.get("DISPLAY", "").startswith(":")
connection = display.Display()
desktop = connection.screen().root
support = connection.intern_atom("_NET_SUPPORTING_WM_CHECK")
assert not desktop.get_full_property(support, X.AnyPropertyType), "A window manager is already running; use an isolated Xvfb server"
with tempfile.TemporaryDirectory(prefix="openpdn-test-wm-") as temporary:
    env = dict(os.environ, XDG_CONFIG_HOME=temporary, XDG_DATA_HOME=temporary,
               QT_QPA_PLATFORM="xcb", KWIN_COMPOSE="N", WAYLAND_DISPLAY="")
    Path(temporary, "kwinrc").write_text("[org.kde.kdecoration2]\nNoPlugin=true\n[Compositing]\nEnabled=false\n")
    with (root / "reference/current-native-window-manager.log").open("w") as log:
        manager = subprocess.Popen(["kwin_x11", "--no-kactivities"], env=env, stdout=log, stderr=log)
        try:
            for _ in range(200):
                if desktop.get_full_property(support, X.AnyPropertyType):
                    break
                assert manager.poll() is None, "Window manager exited"
                time.sleep(0.05)
            else:
                raise AssertionError("Window manager did not start")
            result = subprocess.run([sys.executable, root / "tools/ui-smoke.py"],
                                    env=dict(os.environ, OPENPDN_UI_SCOPE="native-utilities", WAYLAND_DISPLAY=""))
        finally:
            manager.terminate()
            manager.wait(timeout=10)
connection.close()
sys.exit(result.returncode)
