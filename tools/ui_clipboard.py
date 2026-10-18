"""Small external PNG clipboard owner for private Xvfb interaction tests.

Only transfers the deliberately small PNG payloads used by ui-smoke.py. Never
connect this helper to the user's desktop clipboard.
"""
import io
import os
import queue
import threading

from Xlib import X, Xatom, display, protocol


class ImageClipboard:
    def __init__(self):
        assert os.environ.get("OPENPDN_ISOLATED_UI") == "1"
        assert os.environ.get("DISPLAY", "").startswith(":")
        self.commands = queue.Queue()
        self.thread = threading.Thread(target=self._serve, daemon=True)
        self.thread.start()

    def set_image(self, image):
        output = io.BytesIO()
        image.save(output, format="PNG")
        data = output.getvalue()
        assert len(data) < 200000, "This test owner does not implement INCR transfers"
        self._publish(("image/png", data))

    def set_text(self):
        self._publish(("UTF8_STRING", b"Clipboard test text without an image"))

    def _publish(self, value):
        done = threading.Event()
        self.commands.put((value, done))
        assert done.wait(5), "Private clipboard owner did not respond"

    def close(self):
        self._publish(None)
        self.thread.join(timeout=5)
        assert not self.thread.is_alive(), "Private clipboard owner did not stop"

    def _serve(self):
        connection = display.Display()
        owner = connection.screen().root.create_window(
            0, 0, 1, 1, 0, connection.screen().root_depth, X.InputOutput, X.CopyFromParent
        )
        clipboard = connection.intern_atom("CLIPBOARD")
        targets = connection.intern_atom("TARGETS")
        kind, data = targets, b""
        try:
            while True:
                try:
                    value, done = self.commands.get(timeout=0.02)
                    if value is None:
                        done.set()
                        return
                    mime, data = value
                    kind = connection.intern_atom(mime)
                    owner.set_selection_owner(clipboard, X.CurrentTime)
                    connection.sync()
                    done.set()
                except queue.Empty:
                    pass
                while connection.pending_events():
                    event = connection.next_event()
                    if event.type != X.SelectionRequest:
                        continue
                    prop = event.property or event.target
                    if event.target == targets:
                        event.requestor.change_property(prop, Xatom.ATOM, 32, [targets, kind])
                    elif event.target == kind:
                        event.requestor.change_property(prop, kind, 8, data)
                    else:
                        prop = X.NONE
                    event.requestor.send_event(
                        protocol.event.SelectionNotify(
                            time=event.time, requestor=event.requestor,
                            selection=event.selection, target=event.target, property=prop
                        ), event_mask=0,
                    )
                    connection.flush()
        finally:
            owner.destroy()
            connection.close()
