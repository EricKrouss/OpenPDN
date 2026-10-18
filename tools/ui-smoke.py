#!/usr/bin/env python3
"""Exercise native X11 editing, Paint.NET workspace controls, and modal isolation.

Run under the documented isolated Xvfb display, never on the user's desktop.
"""
import os
import shutil
import subprocess
import time
import tempfile
from pathlib import Path
from types import SimpleNamespace
from Xlib import X, XK, display, protocol
from Xlib.ext import xtest
from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
OUT = Path(os.environ.get("OPENPDN_UI_OUTPUT", str(ROOT / "artifacts/ui-smoke")))
OUT.mkdir(parents=True, exist_ok=True)
assert os.environ.get("DISPLAY", "").startswith(":") and os.environ.get("OPENPDN_ISOLATED_UI") == "1", "Use the documented Xvfb command"
connection = display.Display()
log = (OUT / "app.log").open("w")
settings=tempfile.TemporaryDirectory(prefix="openpdn-ui-")
app_env=dict(os.environ, WAYLAND_DISPLAY="", XDG_DATA_HOME=settings.name+"/data", XDG_CONFIG_HOME=settings.name+"/config")
app_env["OPENPDN_PALETTES_PATH"]=settings.name+"/palettes"
if os.environ.get("OPENPDN_UI_SCOPE") == "palette-library":
    palette_dir=Path(app_env["OPENPDN_PALETTES_PATH"]);palette_dir.mkdir()
    (palette_dir/"Reference Short.txt").write_text("FF123456\n80405060\n")
process = subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY", str(ROOT / "target/debug/openpdn"))], stdout=log, stderr=log, env=app_env)
test_clipboard = None

def wait_for(predicate, seconds=10):
    deadline = time.monotonic() + seconds
    while time.monotonic() < deadline:
        if process.poll() is not None:
            raise AssertionError(f"Editor exited unexpectedly: {process.returncode}")
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError("Timed out waiting for editor UI")

def client_windows():
    root = connection.screen().root
    clients = root.get_full_property(connection.intern_atom("_NET_CLIENT_LIST"), X.AnyPropertyType)
    return [connection.create_resource_object("window", int(ident)) for ident in clients.value] if clients else root.query_tree().children

def desktop_geometry(window):
    geometry = window.get_geometry()
    point = connection.screen().root.translate_coords(window, 0, 0)
    return SimpleNamespace(x=point.x, y=point.y, width=geometry.width, height=geometry.height)

def focus_window(window):
    root = connection.screen().root
    if root.get_full_property(connection.intern_atom("_NET_SUPPORTING_WM_CHECK"), X.AnyPropertyType):
        event = protocol.event.ClientMessage(window=window.id, client_type=connection.intern_atom("_NET_ACTIVE_WINDOW"), data=(32,[2,X.CurrentTime,0,0,0]))
        root.send_event(event, event_mask=X.SubstructureRedirectMask | X.SubstructureNotifyMask)
        connection.sync()
        wait_for(lambda: (root.get_full_property(connection.intern_atom("_NET_ACTIVE_WINDOW"), X.AnyPropertyType).value[0] == window.id))
    else:
        window.set_input_focus(X.RevertToParent, X.CurrentTime)
        connection.sync()
    time.sleep(0.15)

def find_window():
    for item in client_windows():
        title = item.get_wm_name() or ""
        if "OpenPDN" in title and item.get_attributes().map_state == X.IsViewable and item.get_geometry().width >= 800:
            return item

def move(x, y):
    xtest.fake_input(connection, X.MotionNotify, x=int(x), y=int(y))
    connection.sync()

def button(pressed):
    xtest.fake_input(connection, X.ButtonPress if pressed else X.ButtonRelease, detail=1)
    connection.sync()

def click(x, y):
    move(x, y)
    time.sleep(0.12)
    button(True)
    time.sleep(0.12)
    button(False)
    time.sleep(0.18)

def key(name, modifiers=()):
    codes = [connection.keysym_to_keycode(XK.string_to_keysym(value)) for value in (*modifiers, name)]
    for code in codes:
        xtest.fake_input(connection, X.KeyPress, detail=code)
    for code in reversed(codes):
        xtest.fake_input(connection, X.KeyRelease, detail=code)
    connection.sync()
    time.sleep(0.25)

def shortcut(name, shift=False):
    key(name, ("Control_L", "Shift_L") if shift else ("Control_L",))

def modifier(name, pressed):
    xtest.fake_input(connection, X.KeyPress if pressed else X.KeyRelease,
                     detail=connection.keysym_to_keycode(XK.string_to_keysym(name)))
    connection.sync()

def capture(name):
    geometry = window.get_geometry()
    pixels = window.get_image(0, 0, geometry.width, geometry.height, X.ZPixmap, 0xffffffff)
    image = Image.frombytes("RGB", (geometry.width, geometry.height), pixels.data, "raw", "BGRX")
    image.save(OUT / name)
    return image

def accept_effect():
    key("Return", ("Control_L",))
    # Acceptance may wait for the latest background GPU result. The modal dim
    # layer disappears only after that result has been committed.
    wait_for(lambda: capture("effect-accept-wait.png").getpixel((100,430)) == (207,207,207),30)

def longest_run(values):
    best = (0, 0)
    start = None
    for index, value in enumerate([*values, False]):
        if value and start is None:
            start = index
        if not value and start is not None:
            if index - start > best[1] - best[0]:
                best = (start, index)
            start = None
    return best

def canvas_bounds(image, expected=(800, 600)):
    y = image.height // 2
    x0, x1 = longest_run([image.getpixel((x, y)) == (255, 255, 255) for x in range(image.width)])
    x = (x0 + x1) // 2
    y0, y1 = longest_run([image.getpixel((x, y)) == (255, 255, 255) for y in range(image.height)])
    assert (x1 - x0, y1 - y0) == expected, (x0, y0, x1, y1)
    return x0, y0, x1, y1

def canvas(name):
    return Image.open(OUT / name).crop(bounds).tobytes()

def stroke(y):
    x0, y0, _, _ = bounds
    move(x0 + 80, y0 + y)
    button(True)
    for x in range(80, 400, 8):
        move(x0 + x, y0 + y + (x - 80) // 3)
        time.sleep(0.015)
    button(False)
    time.sleep(0.8)

def drag(start, end, button_id=1):
    move(*start)
    time.sleep(0.12)
    xtest.fake_input(connection, X.ButtonPress, detail=button_id);connection.sync()
    time.sleep(0.12)
    for index in range(1, 21):
        move(start[0] + (end[0] - start[0]) * index / 20, start[1] + (end[1] - start[1]) * index / 20)
        time.sleep(0.02)
    xtest.fake_input(connection, X.ButtonRelease, detail=button_id);connection.sync()
    time.sleep(0.3)

def drop_file(path):
    """Exercise winit's actual Xdnd file transfer on this isolated X server."""
    source = connection.screen().root.create_window(0,0,1,1,0,connection.screen().root_depth, X.InputOutput, X.CopyFromParent)
    selection = connection.intern_atom("XdndSelection")
    uri_type = connection.intern_atom("text/uri-list")
    source.set_selection_owner(selection, X.CurrentTime)
    def send(name, data):
        window.send_event(protocol.event.ClientMessage(window=window.id, client_type=connection.intern_atom(name), data=(32,data)), event_mask=0)
        connection.sync()
    send("XdndEnter", [source.id,5<<24,uri_type,0,0])
    send("XdndPosition", [source.id,0,(640<<16)|425,X.CurrentTime,connection.intern_atom("XdndActionCopy")])
    deadline=time.monotonic()+10
    transferred=False
    while time.monotonic()<deadline:
        while connection.pending_events():
            event=connection.next_event()
            if event.type==X.SelectionRequest:
                data=(path.resolve().as_uri()+"\r\n").encode()
                event.requestor.change_property(event.property,uri_type,8,data)
                event.requestor.send_event(protocol.event.SelectionNotify(time=event.time,requestor=event.requestor,selection=event.selection,target=event.target,property=event.property),event_mask=0)
                connection.sync();transferred=True
        if transferred:
            # winit requests URI data during hover, before accepting the drop.
            time.sleep(0.5);capture("26-drop-hover.png")
            send("XdndDrop", [source.id,0,X.CurrentTime,0,0])
            time.sleep(0.7);source.destroy();connection.sync();return
        time.sleep(0.05)
    source.destroy();raise AssertionError("Native Xdnd transfer was not requested")

try:
    window = wait_for(find_window)
    if os.environ.get("OPENPDN_UI_SCOPE") == "native-utilities":
        assert connection.screen().root.get_full_property(connection.intern_atom("_NET_SUPPORTING_WM_CHECK"), X.AnyPropertyType), "Run this scope through tools/ui-native-window-manager.py"
        window.configure(x=0, y=0)
        connection.sync()
        wait_for(lambda: (desktop_geometry(window).x, desktop_geometry(window).y) == (0, 0))
    else:
        assert (window.get_geometry().x, window.get_geometry().y) == (0, 0)
    window.set_input_focus(X.RevertToParent, X.CurrentTime)
    connection.sync()
    # Native mapping can precede the first GPU presentation, especially while
    # other hardware checks are compiling pipelines. Wait for actual pixels.
    wait_for(lambda: capture("startup-wait.png").getpixel((640,425)) == (255,255,255),30)
    if os.environ.get("OPENPDN_UI_SCOPE") == "clipboard":
        from ui_clipboard import ImageClipboard
        test_clipboard=ImageClipboard()
        key("Tab")  # Keep utility windows away from canvas comparisons.
        def blank_size(size, name):
            try:
                return canvas_bounds(capture(name), size)
            except AssertionError:
                return False
        for entry,size in (("shortcut",(913,517)),("toolbar",(235,379)),("menu",(321,123))):
            test_clipboard.set_image(Image.new("RGBA",size,(220,35,50,255)))
            if entry=="shortcut":shortcut("n")
            elif entry=="toolbar":click(19,36)
            else:click(15,10);click(60,35)
            wait_for(lambda:capture(f"clipboard-new-{entry}-fields.png").getpixel((100,430))!=(207,207,207))
            key("Return")
            wait_for(lambda:blank_size(size,f"clipboard-new-{entry}-accepted.png"))
            click(514,24)
            wait_for(lambda:blank_size((800,600),f"clipboard-new-{entry}-closed.png"))
        # A recognized photo above the editor's pixel cap still initializes
        # the real fields; accepting stays blocked by size validation.
        test_clipboard.set_image(Image.new("RGBA",(6000,4000),(220,35,50,255)))
        shortcut("n")
        wait_for(lambda:capture("clipboard-new-large-fields.png").getpixel((100,430))!=(207,207,207),30)
        key("Return");time.sleep(0.35)
        assert capture("clipboard-new-large-validation.png").getpixel((100,430))!=(207,207,207),"Oversized clipboard photo silently fell back to a smaller New document"
        key("Escape")
        wait_for(lambda:blank_size((800,600),"clipboard-new-large-cancelled.png"))
        test_clipboard.set_text()
        shortcut("n");capture("clipboard-new-no-image-fields.png");key("Return")
        wait_for(lambda:blank_size((800,600),"clipboard-new-no-image-accepted.png"))
        click(514,24)
        wait_for(lambda:blank_size((800,600),"clipboard-original.png"))
        print("Clipboard pixel dimensions initialize New through shortcut, toolbar and menu; text-only clipboard restores 800x600 defaults",flush=True)
        bounds=canvas_bounds(capture("clipboard-original.png"));x0,y0,_,_=bounds
        white=capture("clipboard-white.png").crop(bounds).tobytes()
        key("b");click(x0+60,y0+80)
        base=capture("clipboard-base.png").crop(bounds).tobytes()
        assert base!=white,"Test brush mark was not drawn"
        photo=Image.new("RGBA",(913,517),(210,40,70,255))
        photo.paste((20,80,220,255),(800,0,913,517))
        test_clipboard.set_image(photo)
        def unchanged(name):
            return capture(name).crop(bounds).tobytes()==base
        for new_layer in (False,True):
            shortcut("v",new_layer)
            wait_for(lambda:capture(f"clipboard-cancel-{new_layer}-prompt.png").getpixel((100,430))!=(207,207,207))
            key("Escape") if not new_layer else click(493,495)
            wait_for(lambda:unchanged(f"clipboard-cancel-{new_layer}.png"))
            shortcut("z")
            assert capture(f"clipboard-cancel-{new_layer}-undo.png").crop(bounds).tobytes()==white,"Cancelled size prompt added history or a layer"
            shortcut("y")
            wait_for(lambda:unchanged(f"clipboard-cancel-{new_layer}-redo.png"))
        # Default Enter expands only the axis that needs it. Live Ctrl+Z
        # removes pasted pixels; the next undo removes the Canvas Size step.
        shortcut("v")
        test_clipboard.set_image(Image.new("RGBA",(320,240),(30,160,80,255)))
        key("Return")  # Acceptance must use the bitmap captured before this clipboard change.
        wait_for(lambda:capture("clipboard-expand-preview.png").getpixel((640,430))==(210,40,70))
        shortcut("z")
        expanded=capture("clipboard-expand-pixels-undone.png")
        assert expanded.getpixel((190,300))!=(207,207,207),"Undoing pasted pixels also removed canvas expansion"
        shortcut("z")
        wait_for(lambda:unchanged("clipboard-expand-canvas-undone.png"))
        test_clipboard.set_image(photo)
        # New Layer and Canvas Size remain distinct undo/redo entries before
        # Paste and the original Move tool's Enter/Deselect entry.
        shortcut("v",True);click(520,411)
        wait_for(lambda:capture("clipboard-layer-expand-preview.png").getpixel((640,430))==(210,40,70))
        key("Return")
        accepted=capture("clipboard-layer-expand-accepted.png")
        expanded_region=(190,180,1085,740)
        for step in range(4):
            shortcut("z");capture(f"clipboard-layer-expand-undo-{step}.png")
        wait_for(lambda:unchanged("clipboard-layer-expand-undone.png"))
        for step in range(4):shortcut("y")
        assert capture("clipboard-layer-expand-redone.png").crop(expanded_region).tobytes()==accepted.crop(expanded_region).tobytes(),"Paste expansion/layer redo changed pixels"
        for step in range(4):shortcut("z")
        wait_for(lambda:unchanged("clipboard-layer-expand-restored.png"))
        # Keep leaves all source pixels available to move back onto the canvas.
        shortcut("v");key("k",("Alt_L",))
        wait_for(lambda:capture("clipboard-keep-preview.png").getpixel((x0+650,y0+200))==(210,40,70))
        drag((x0+400,y0+250),(x0+200,y0+250))
        assert capture("clipboard-keep-moved.png").getpixel((x0+650,y0+200))==(20,80,220),"Keep discarded the off-canvas part of the clipboard photo"
        shortcut("z")
        wait_for(lambda:unchanged("clipboard-keep-cancelled.png"))
        shortcut("v",True);click(520,454);key("Return")
        capture("clipboard-layer-keep-accepted.png")
        for step in range(3):shortcut("z")
        wait_for(lambda:unchanged("clipboard-layer-keep-undone.png"))
        # A regular paste into the active layer must not allocate a new layer:
        # after accepting/deselecting, two undos return to the previous edit.
        small=Image.new("RGBA",(320,240),(30,160,80,255))
        test_clipboard.set_image(small)
        shortcut("v");key("Return")
        capture("clipboard-small-accepted.png")
        shortcut("z");shortcut("z")
        wait_for(lambda:unchanged("clipboard-small-undone.png"))
        test_clipboard.set_text();shortcut("v")
        wait_for(lambda:capture("clipboard-no-image-error.png").getpixel((100,430))!=(207,207,207))
        key("Escape")
        wait_for(lambda:unchanged("clipboard-no-image-recovered.png"))
        print("External PNG paste expand/keep/cancel, retained off-canvas pixels, new-layer and canvas undo/redo, active-layer paste and missing-image recovery passed",flush=True)
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "quirks":
        initial=capture("quirks-initial.png");bounds=canvas_bounds(initial)
        move(700,450)
        def chooser(name):return capture(name).crop((40,75,181,98)).tobytes()
        brush=chooser("quirks-brush.png")
        key("s");rectangle=chooser("quirks-rectangle.png")
        key("s");assert chooser("quirks-lasso.png")!=rectangle
        key("s",("Shift_L",));assert chooser("quirks-reverse.png")==rectangle,"Shift did not reverse the group"
        key("s");time.sleep(2.1);key("s")
        assert chooser("quirks-timeout.png")==rectangle,"Tool key did not reset after two seconds"
        key("b");key("s",("Shift_L",));wand=chooser("quirks-wand.png")
        assert wand!=rectangle
        key("b");key("o");line=chooser("quirks-line.png")
        key("o");shape=chooser("quirks-shape.png")
        assert line!=shape
        key("o",("Shift_L",));assert chooser("quirks-line-reverse.png")==line
        key("b");move(500,400);button(True);time.sleep(.15);key("s");button(False);time.sleep(.3)
        assert chooser("quirks-held-tool.png")==brush,"Tool changed during a stroke"
        shortcut("z")
        header=(12,532,170,553)
        primary=capture("quirks-primary.png").crop(header).tobytes()
        key("c");assert capture("quirks-secondary.png").crop(header).tobytes()!=primary
        key("c");assert capture("quirks-primary-restored.png").crop(header).tobytes()==primary
        key("x");assert capture("quirks-swapped.png").getpixel((25,570))==(255,255,255)
        key("x",("Shift_L",));assert capture("quirks-unswapped.png").getpixel((25,570))==(0,0,0)
        key("bracketright");shortcut("bracketright")
        width8=capture("quirks-width-eight.png").crop((190,75,350,98)).tobytes()
        key("o");key("bracketright");key("b")
        assert capture("quirks-shared-width.png").crop((190,75,350,98)).tobytes()!=width8
        key("bracketleft")
        assert capture("quirks-shared-eight.png").crop((190,75,350,98)).tobytes()==width8,"Line width was not shared with Paintbrush"
        shortcut("bracketleft");key("bracketleft")
        assert capture("quirks-width-two.png").crop((190,75,350,98)).tobytes()==initial.crop((190,75,350,98)).tobytes(),"Bracket steps did not return to width two"
        key("o");drag((500,400),(650,400));flat=capture("quirks-flat-line.png")
        key("comma");arrow=capture("quirks-arrow-line.png")
        assert flat.crop(bounds).tobytes()!=arrow.crop(bounds).tobytes(),"Start-cap shortcut did not update the live line"
        key("slash");key("period");dashed=capture("quirks-dashed-line.png")
        assert dashed.crop(bounds).tobytes()!=arrow.crop(bounds).tobytes()
        key("Return",("Control_L",));shortcut("z")
        key("b");key("o",("Shift_L",))
        drag((500,380),(650,460));rect=capture("quirks-live-rectangle.png")
        key("a");rounded=capture("quirks-live-rounded.png")
        assert rect.crop(bounds).tobytes()!=rounded.crop(bounds).tobytes(),"A did not change the live shape"
        key("a",("Shift_L",));assert capture("quirks-live-reversed.png").crop(bounds).tobytes()==rect.crop(bounds).tobytes()
        key("Return",("Control_L",));shortcut("z")
        assert capture("quirks-shape-undo.png").crop(bounds).tobytes()==bytes([255])*(800*600*3),"Shape options created extra undo steps"
        key("Tab");click(190,125);key("Escape");key("s")
        assert chooser("quirks-enter-selection-tool.png")==rectangle,"Selection key was intercepted by toolbar focus"
        drag((500,350),(600,450));key("Return");key("f");click(800,500)
        filled=capture("quirks-enter-deselect.png")
        assert filled.getpixel((800,500))==(0,0,0) and filled.getpixel((550,400))==(0,0,0),"Enter did not deselect before filling"
        shortcut("z");shortcut("d")
        key("p");click(640,425);key("z");click(700,450);key("b")
        region=(300,180,1200,780)
        def marker(name):
            shot=capture(name);pixels=shot.crop(region)
            points=[(x+region[0],y+region[1]) for y in range(pixels.height) for x in range(pixels.width) if pixels.getpixel((x,y))==(0,0,0)]
            assert len(points)==4,(name,len(points))
            return min(x for x,y in points),min(y for x,y in points)
        first=marker("quirks-before-pan.png");selected=chooser("quirks-selected-before-pan.png")
        modifier("space",True);time.sleep(.2);drag((900,650),(800,550));modifier("space",False);time.sleep(.4)
        second=marker("quirks-space-pan.png")
        assert second==(first[0]-100,first[1]-100),(first,second)
        time.sleep(.5);assert marker("quirks-space-stopped.png")==second,"Panning continued after release"
        drag((850,600),(800,550),button_id=2);third=marker("quirks-middle-pan.png")
        assert third==(second[0]-50,second[1]-50),(second,third)
        assert chooser("quirks-selected-after-pan.png")==selected,"Temporary pan changed the selected tool"
        before=capture("quirks-before-post-pan-paint.png").crop(region).tobytes()
        key("p");click(900,600);shortcut("z")
        assert capture("quirks-post-pan-undo.png").crop(region).tobytes()==before,"Panning painted pixels or created an undo step"
        key("t");click(800,550);time.sleep(1);capture("quirks-text-start.png")
        key("c");key("space");key("x");time.sleep(.5);capture("quirks-text-live.png");key("Return",("Control_L",))
        assert capture("quirks-text-keys.png").crop(region).tobytes()!=before,"Tool shortcuts intercepted inline text"
        shortcut("z")
        print("Native timed/reverse tool keys, held-stroke isolation, C/X colors, shared bracket widths, live caps/dashes/shapes, Enter deselect, Space/middle pan, text isolation and undo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "native-utilities":
        click(85,10);time.sleep(0.4);move(145,166);time.sleep(0.5);capture("native-window-menu.png")
        click(350,263);time.sleep(0.7)
        capture("native-main-after-float.png")
        def native_tool(name):
            for item in client_windows():
                if (item.get_wm_name() or "").startswith(name) and "OpenPDN" in (item.get_wm_name() or "") and item.get_attributes().map_state==X.IsViewable:
                    return item
        tools={name:wait_for(lambda name=name:native_tool(name)) for name in ("Tools","History","Layers","Colors")}
        history=tools["History"];g=desktop_geometry(history)
        drag((g.x+90,g.y+20),(g.x+490,g.y+20));time.sleep(0.5)
        assert desktop_geometry(history).x>=1280,"Native utility title drag was clamped to the main window"
        for name,(x,y) in {"Tools":(1300,100),"Layers":(1650,480),"Colors":(1400,480)}.items():
            tool=tools[name];g=desktop_geometry(tool)
            drag((g.x+45,g.y+20),(x+45,y+20));time.sleep(0.5)
            actual=desktop_geometry(tool)
            pixels=connection.screen().root.get_image(0,0,1920,1080,X.ZPixmap,0xffffffff)
            Image.frombytes("RGB",(1920,1080),pixels.data,"raw","BGRX").save(OUT/f"native-drag-{name}.png")
            assert actual.x>=1280,f"{name} could not move outside the main window: {(actual.x,actual.y)}"
            assert abs(actual.x-x)<=4 and abs(actual.y-y)<=4,f"{name} title drag overshot the cursor: {(actual.x,actual.y)} instead of {(x,y)}"
        pixels=connection.screen().root.get_image(0,0,1920,1080,X.ZPixmap,0xffffffff)
        Image.frombytes("RGB",(1920,1080),pixels.data,"raw","BGRX").save(OUT/"native-outside-desktop.png")
        # Load files produced by the original Paint.NET serializer while all
        # four utilities are outside the editor. Save only task-private copies.
        for fixture in ("small", "chunks", "small-legacy"):
            source=ROOT/f"tests/fixtures/pdn/{fixture}.pdn"
            original=source.read_bytes()
            path=OUT/f"ui-{fixture}-rust.pdn"
            shutil.copyfile(source,path)
            focus_window(window)
            drop_file(path);key("Tab");key("Return")
            wait_for(lambda:path.name in (window.get_wm_name() or ""))
            region=(500,320,780,610)
            before=capture(f"native-pdn-{fixture}.png").crop(region).tobytes()
            layers=tools["Layers"];g=desktop_geometry(layers)
            pixels=layers.get_image(0,0,g.width,g.height,X.ZPixmap,0xffffffff)
            Image.frombytes("RGB",(g.width,g.height),pixels.data,"raw","BGRX").save(OUT/f"native-pdn-{fixture}-layers.png")
            # The first row is the uppermost original layer, not a flattened
            # imported bitmap. Its visibility is editable in the native window.
            focus_window(layers)
            click(g.x+g.width-24,g.y+57)
            wait_for(lambda:capture(f"native-pdn-{fixture}-hidden.png").crop(region).tobytes()!=before)
            focus_window(layers);shortcut("z")
            wait_for(lambda:capture(f"native-pdn-{fixture}-undo.png").crop(region).tobytes()==before)
            shortcut("s")
            assert "*" not in (window.get_wm_name() or ""), "Saving a PDN left unsaved edits"
            focus_window(window)
            if fixture=="small":
                for label,data in (("truncated",original[:100]),("version",b"PDN4"+original[4:])):
                    invalid=Path(settings.name)/f"{label}.pdn"
                    invalid.write_bytes(data)
                    drop_file(invalid);key("Tab");key("Return")
                    wait_for(lambda:capture(f"native-pdn-{label}-error.png").getpixel((100,430))!=(207,207,207))
                    key("Escape")
                    wait_for(lambda:capture(f"native-pdn-{label}-recovered.png").crop(region).tobytes()==before)
                    assert path.name in (window.get_wm_name() or ""), "Failed load replaced the active document"
            assert source.read_bytes()==original, "Original PDN fixture was changed"
            # The imported document is the second thumbnail; close its own x.
            click(514,24)
            wait_for(lambda:"Untitled" in (window.get_wm_name() or ""))
        print("Original PDN3 and legacy files loaded with external utility windows; native layer visibility/undo/save and failed-load recovery passed",flush=True)
        # Actual native control clicks still edit the parent document.
        g=desktop_geometry(tools["Tools"]);click(g.x+20,g.y+198)
        click(500,400);time.sleep(0.4)
        assert capture("native-tool-paint.png").getpixel((500,400))!=(255,255,255),"Detached Tools did not select a working tool"
        focus_window(window)
        shortcut("r");time.sleep(0.4)
        g=desktop_geometry(history);click(g.x+70,g.y+47);time.sleep(0.4)
        key("Escape")
        isolated=capture("native-modal-isolation.png")
        assert isolated.getpixel((500,400))!=(255,255,255),"Detached History changed the image beneath a modal"
        assert isolated.getpixel((100,430))==(207,207,207),"A disabled native utility intercepted the dialog's Escape key"
        click(g.x+70,g.y+47);time.sleep(0.4)
        assert capture("native-history-undo.png").getpixel((500,400))==(255,255,255),"Detached History did not navigate document history"
        focus_window(history);shortcut("r")
        assert capture("native-modal-from-utility.png").getpixel((100,430))!=(207,207,207),"Detached keyboard shortcut did not open the root dialog"
        key("Escape")
        assert capture("native-modal-from-utility-cancelled.png").getpixel((100,430))==(207,207,207),"Dialog opened from a native utility did not cancel"
        # Close only this native window, then restore it through the shortcut.
        history.send_event(protocol.event.ClientMessage(window=history.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        wait_for(lambda:not native_tool("History"))
        focus_window(window);key("F6")
        history=wait_for(lambda:native_tool("History"))
        assert desktop_geometry(history).x>=1280,"Reopening a native window lost its position"
        focus_window(history);key("F8")
        wait_for(lambda:not native_tool("Colors"));key("F8")
        wait_for(lambda:native_tool("Colors"))
        positions={name:(desktop_geometry(native_tool(name)).x,desktop_geometry(native_tool(name)).y) for name in tools}
        window.send_event(protocol.event.ClientMessage(window=window.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        process.wait(timeout=15)
        process=subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY",str(ROOT/"target/debug/openpdn"))],stdout=log,stderr=log,env=app_env)
        window=wait_for(find_window);window.configure(x=0,y=0);connection.sync();time.sleep(0.7)
        for name,position in positions.items():
            tool=wait_for(lambda name=name:native_tool(name));g=desktop_geometry(tool)
            assert (g.x,g.y)==position,f"Native {name} position did not survive restart"
            pixels=connection.screen().root.get_image(0,0,1920,1080,X.ZPixmap,0xffffffff)
            Image.frombytes("RGB",(1920,1080),pixels.data,"raw","BGRX").save(OUT/f"native-before-dock-{name}.png")
            focus_window(tool);time.sleep(0.15)
            tool.configure(stack_mode=X.Above);connection.sync();time.sleep(0.15)
            click(g.x+20,g.y+20)
            try:wait_for(lambda name=name:not native_tool(name))
            except AssertionError as e:raise AssertionError(f"Redocking {name} failed") from e
        capture("native-redocked.png")
        move(1105,120);xtest.fake_input(connection,X.ButtonPress,detail=3);xtest.fake_input(connection,X.ButtonRelease,detail=3);connection.sync();time.sleep(0.4)
        capture("native-context-float.png");click(1000,136)
        wait_for(lambda:native_tool("History"))
        print("All four native tool windows dragged beyond the editor; controls, modal isolation, close/reopen, focused shortcuts, restart positions and redocking passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "palette-library":
        initial=capture("library-initial.png")
        click(75,809);time.sleep(0.4);capture("library-menu.png")
        click(250,621);time.sleep(0.4)
        short=capture("library-short-loaded.png")
        assert short.getpixel((19,726))==(18,52,86),"Named palette row did not load"
        assert short.getpixel((43,726))==(255,255,255),"Short library palette was not padded with white"
        click(75,809);time.sleep(0.4);capture("library-match.png")
        click(250,656);time.sleep(0.4);capture("library-save-dialog.png")
        for c in "bad":key(c)
        key("slash")
        for c in "name":key(c)
        key("Return");click(300,300);capture("library-invalid-name.png")
        assert len(list(palette_dir.iterdir()))==1,"Invalid palette name was saved"
        click(550,359);shortcut("a")
        for c in "savedswatch":key(c)
        key("Return");time.sleep(0.4)
        saved_path=palette_dir/"savedswatch.txt"
        assert saved_path.exists(),"Save name dialog did not write the palette"
        colors=lambda:[s for s in saved_path.read_text(encoding="utf-8-sig").splitlines() if s and not s.startswith(";")]
        assert len(colors())==96 and colors()[0]=="FF123456","Named save did not preserve all padded colors"
        assert capture("library-saved.png").getpixel((300,300))==(255,255,255),"Palette modal leaked drawing input"
        click(75,809);time.sleep(0.4);click(250,718);time.sleep(0.4)
        assert capture("library-default.png").getpixel((19,726))==(0,0,0),"Reset palette failed"
        click(75,809);time.sleep(0.4);click(250,621);time.sleep(0.4)
        assert capture("library-reloaded.png").getpixel((19,726))==(18,52,86),"Saved palette was not selectable"
        click(75,809);time.sleep(0.4);click(250,718);time.sleep(0.4)
        click(75,809);time.sleep(0.4);click(250,656);time.sleep(0.4)
        capture("library-existing-dialog.png");click(540,429);key("Return");time.sleep(0.4)
        assert colors()[0]=="FF000000" and len(list(palette_dir.iterdir()))==2,"Selecting an existing name did not replace the palette"
        assert not (window.get_wm_name() or "").startswith("*"),"Palette save dirtied the document"
        window.send_event(protocol.event.ClientMessage(window=window.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        process.wait(timeout=15)
        process=subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY",str(ROOT/"target/debug/openpdn"))],stdout=log,stderr=log,env=app_env)
        window=wait_for(find_window);window.set_input_focus(X.RevertToParent,X.CurrentTime);connection.sync();time.sleep(0.7)
        click(75,809);time.sleep(0.4);capture("library-restart-menu.png");click(250,594);time.sleep(0.4)
        assert capture("library-restart-loaded.png").getpixel((19,726))==(18,52,86),"Named library did not survive restart"
        (palette_dir/"Reference Short.txt").write_text("FFABCDEF\n")
        click(75,809);time.sleep(0.4);click(250,594);time.sleep(0.4)
        assert capture("library-external-refresh.png").getpixel((19,726))==(171,205,239),"Opening the menu did not refresh changed palette files"
        print("Native palette discovery, white padding, named save, invalid names, modal isolation, replacement, reset, external refresh and restart passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "persistence":
        initial=capture("persistence-initial.png")
        key("g");key("F8");click(318,36)
        drag((1105,117),(900,185))
        saved=capture("persistence-saved.png")
        assert saved.crop((880,170,1180,360)).tobytes()!=initial.crop((880,170,1180,360)).tobytes(),"History move did not change its position"
        window.send_event(protocol.event.ClientMessage(window=window.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        process.wait(timeout=15)
        process=subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY",str(ROOT / "target/debug/openpdn"))],stdout=log,stderr=log,env=app_env)
        window=wait_for(find_window);window.set_input_focus(X.RevertToParent,X.CurrentTime);connection.sync();time.sleep(0.7)
        restored=capture("persistence-restored.png")
        for region in [(4,72,180,99),(4,560,232,815),(880,170,1180,360)]:
            assert restored.crop(region).tobytes()==saved.crop(region).tobytes(),f"Workspace state failed to persist: {region}"
        print("Native restart preserved tool, colors window visibility, grid and floating History position")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "image-size":
        key("Tab");time.sleep(0.4)
        click(280,210)
        base=capture("size-original.png");bounds=canvas_bounds(base)
        # An unchanged Resize or Canvas Size must not consume the next undo.
        for canvas in (False, True):
            shortcut("r",canvas);time.sleep(0.4)
            click(487,606) if canvas else click(488,586)
            shortcut("z");time.sleep(0.3)
            assert capture(f"size-noop-{canvas}-undo.png").crop(bounds).tobytes()!=base.crop(bounds).tobytes(),"Unchanged size dialog added an undo entry"
            shortcut("y");time.sleep(0.3)
            assert capture(f"size-noop-{canvas}-redo.png").crop(bounds).tobytes()==base.crop(bounds).tobytes(),"No-op dialog changed the drawing or redo history"
        shortcut("r");time.sleep(0.4);initial=capture("size-initial-dialog.png")
        click(558,481);shortcut("a");key("1");key("9");key("2");click(750,290)
        changed=capture("size-dpi-edited.png")
        assert changed.crop((520,349,565,365)).tobytes()==initial.crop((520,349,565,365)).tobytes(),"Resolution edit changed pixel width"
        assert changed.crop((540,425,586,441)).tobytes()!=initial.crop((540,425,586,441)).tobytes(),"Resolution edit did not update print size"
        click(488,586);time.sleep(0.4)
        assert canvas_bounds(capture("size-dpi-accepted.png"))==bounds,"DPI-only edit resampled canvas"
        shortcut("r");time.sleep(0.4);capture("size-dpi-reopened.png")
        click(560,433);shortcut("a");key("2");click(750,290);click(488,586);time.sleep(0.5)
        resized=canvas_bounds(capture("size-physical-accepted.png"), (384,288))
        assert (resized[2]-resized[0],resized[3]-resized[1])==(384,288),f"Physical dimensions did not use 192 DPI: {resized}"
        shortcut("z");time.sleep(0.3);assert canvas_bounds(capture("size-physical-undo.png"))==bounds
        shortcut("z");shortcut("r");time.sleep(0.4)
        restored=capture("size-dpi-undo.png")
        assert restored.crop((540,425,586,441)).tobytes()==initial.crop((540,425,586,441)).tobytes(),"Undo did not restore document resolution"
        key("Escape");capture("size-finished.png")
        print("Native DPI-only acceptance, physical resize at 192 DPI, pixel dimensions and metadata undo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "editing":
        initial=capture("editing-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(x0+80,y0+80);key("b");stroke(140)
        base=capture("editing-base.png").crop(bounds).tobytes()
        shortcut("r");capture("editing-resize-dialog.png");key("Escape")
        assert capture("editing-resize-cancel.png").crop(bounds).tobytes()==base
        shortcut("r",True);capture("editing-canvas-dialog.png");key("Escape")
        assert capture("editing-canvas-cancel.png").crop(bounds).tobytes()==base
        shortcut("r");click(580,311);shortcut("a");key("5");key("0");click(750,290);click(488,586);time.sleep(0.5)
        resized=capture("editing-resize-accepted.png");assert resized.getpixel((440,330))==(255,255,255) and resized.getpixel((300,330))!=(255,255,255),"Resize did not change canvas dimensions"
        shortcut("z");assert capture("editing-resize-undo.png").crop(bounds).tobytes()==base
        shortcut("r",True);click(540,315);shortcut("a");key("1");key("0");key("0");key("0");click(750,290);click(487,606);time.sleep(0.4)
        sized=capture("editing-canvas-accepted.png");assert sized.getpixel((150,450))!=(207,207,207),"Canvas Size did not grow canvas"
        shortcut("z");assert capture("editing-canvas-undo.png").crop(bounds).tobytes()==base
        shortcut("a");selected=capture("editing-selected.png").crop(bounds).tobytes();shortcut("c");shortcut("x")
        erased=capture("editing-cut.png").crop(bounds).tobytes();assert erased!=base,"Cut shortcut inert"
        shortcut("z");assert capture("editing-cut-undo.png").crop(bounds).tobytes()==selected
        shortcut("v",True);capture("editing-paste-layer.png")
        shortcut("m");capture("editing-merge.png");shortcut("z");shortcut("z")
        key("d",("Control_L",));assert capture("editing-paste-undo.png").crop(bounds).tobytes()==base
        key("s");drag((x0+60,y0+110),(x0+420,y0+180));shortcut("x",True)
        capture("editing-crop.png");shortcut("z");shortcut("d")
        assert capture("editing-crop-undo.png").crop(bounds).tobytes()==base,"Crop/undo mismatch"
        key("v",("Control_L","Alt_L"));time.sleep(0.5);capture("editing-paste-new-image.png")
        assert "Untitled" in (window.get_wm_name() or "")
        print("Native resize/canvas/crop/cut/copy/paste/new-image/merge/undo flows passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "transforms":
        initial=capture("transform-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");key("b");drag((x0+100,y0+150),(x0+300,y0+150))
        base_image=capture("transform-base.png")
        base=base_image.crop(bounds).tobytes()
        stroke_pixel=base_image.getpixel((x0+200,y0+150))
        assert stroke_pixel!=(255,255,255),"Transform test brush mark was not drawn"
        key("s");drag((x0+60,y0+120),(x0+340,y0+190))
        selected=capture("transform-selected.png").crop(bounds).tobytes();shortcut("c")
        shortcut("v",True);capture("transform-paste-floating.png")
        drag((x0+200,y0+150),(x0+300,y0+250))
        moved=capture("transform-paste-moved.png")
        assert moved.getpixel((x0+300,y0+250))==stroke_pixel,"Floating paste did not move"
        assert moved.getpixel((x0+200,y0+150))==stroke_pixel,"Paste erased the original layer"
        drag((x0+440,y0+290),(x0+580,y0+360))
        scaled=capture("transform-paste-scaled.png")
        assert scaled.getpixel((x0+300,y0+280))==stroke_pixel,"Corner handle did not scale paste"
        drag((x0+470,y0+290),(x0+370,y0+390),3)
        rotated=capture("transform-paste-rotated.png")
        assert rotated.crop(bounds).tobytes()!=scaled.crop(bounds).tobytes(),"Right drag did not rotate"
        click(815,86)
        assert capture("transform-paste-cancelled.png").crop(bounds).tobytes()==selected,"Cancel did not restore the pre-paste pixels and selection"
        shortcut("z")  # Original New Layer is a separate preparation entry.
        shortcut("v",True);shortcut("z")
        assert capture("transform-paste-cancelled-undo.png").crop(bounds).tobytes()==selected,"Ctrl+Z did not cancel floating paste"
        shortcut("z")
        shortcut("v",True);drag((x0+200,y0+150),(x0+300,y0+250));key("Return")
        accepted=capture("transform-paste-accepted.png").crop(bounds).tobytes()
        assert accepted!=base,"Accept lost pasted pixels"
        shortcut("z");shortcut("z");shortcut("d")
        assert capture("transform-paste-undo.png").crop(bounds).tobytes()==base,"Accepted paste was not reversible"
        # A selection-only transform preserves every image pixel after deselection.
        key("s");drag((x0+60,y0+120),(x0+340,y0+190));key("m");key("m")
        drag((x0+200,y0+150),(x0+300,y0+250));key("Return")
        assert capture("transform-mask-only.png").crop(bounds).tobytes()==base,"Move Selection modified image pixels"
        print("Native floating paste move/scale/rotation/cancel/accept/undo and selection-only flows passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "line":
        initial=capture("line-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(x0+80,y0+80);key("o");base=capture("line-base.png").crop(bounds).tobytes()
        drag((x0+120,y0+160),(x0+600,y0+160));capture("line-straight-live.png")
        drag((x0+280,y0+160),(x0+280,y0+300));capture("line-spline-live.png")
        key("Return");accepted=capture("line-spline-accepted.png")
        assert accepted.getpixel((x0+280,y0+300))==(0,0,0),"Spline missed its moved knot"
        shortcut("z");assert capture("line-undo.png").crop(bounds).tobytes()==base,"Line was not one transaction"
        shortcut("y");assert capture("line-redo.png").crop(bounds).tobytes()==accepted.crop(bounds).tobytes()
        drag((x0+100,y0+400),(x0+550,y0+400));drag((x0+250,y0+400),(x0+250,y0+500));key("Escape")
        wait_for(lambda:capture("line-cancelled.png").crop(bounds).tobytes()==accepted.crop(bounds).tobytes())
        shortcut("z")
        modifier("Shift_L",True);modifier("Alt_L",True)
        drag((x0+400,y0+300),(x0+550,y0+309))
        modifier("Alt_L",False);modifier("Shift_L",False)
        capture("line-centered-constrained.png")
        drag((x0+400,y0+300),(x0+430,y0+350));capture("line-whole-move.png")
        drag((x0+430,y0+322),(x0+458,y0+350));key("Return")
        rotated=capture("line-whole-rotate.png")
        assert any(rotated.getpixel((x0+x,y0+210))==(0,0,0) for x in range(428,434)),"Whole-curve rotation missed expected vertical line"
        assert rotated.getpixel((x0+300,y0+350))==(255,255,255),"Whole-curve move/rotation left old pixels"
        shortcut("z");assert capture("line-modifiers-undo.png").crop(bounds).tobytes()==base,"Move/rotation split the curve transaction"
        print("Native Line/Curve control-point drag, centered/constrained drawing, whole move/rotation, finish/cancel and one-transaction undo/redo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") in ("filters", "composites", "neighborhoods", "remaining"):
        initial=capture("filters-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(x0+80,y0+80);key("g");drag((x0+160,y0+250),(x0+640,y0+250));key("b");stroke(80);stroke(250)
        base=capture("filters-base.png").crop(bounds).tobytes()
        for name, row in ([] if os.environ.get("OPENPDN_UI_SCOPE") != "filters" else [("highlights",146),("temperature",300)]):
            click(215,10);click(230,row);time.sleep(0.5);capture("filters-"+name+"-dialog.png")
            click(614,375 if name=="highlights" else 399);shortcut("a");key("5");key("0");key("Return");time.sleep(0.4)
            capture("filters-"+name+"-preview.png");accept_effect();time.sleep(0.4)
            accepted=capture("filters-"+name+"-accepted.png").crop(bounds).tobytes();assert accepted!=base,name+" adjustment inert"
            shortcut("z");assert capture("filters-"+name+"-undo.png").crop(bounds).tobytes()==base
            click(215,10);click(230,row);time.sleep(0.4);key("Escape");assert capture("filters-"+name+"-cancel.png").crop(bounds).tobytes()==base
        cases=[("fragment",90,112),("gaussian",90,134),("motion",90,178),("radial",90,200),("square",90,244),("zoom",90,288),("bulge",134,134),("polar",134,266),("tile",134,288),("twist",134,310),("glow",200,200),("pencil",68,112)]
        if os.environ.get("OPENPDN_UI_SCOPE") == "composites":
            cases=[("ink",68,68),("surface",90,266),("portrait",200,266),("shadow",178,178)]
        if os.environ.get("OPENPDN_UI_SCOPE") == "neighborhoods":
            cases=[("oil",68,90),("median",90,156),("reduce",156,178),("outline",244,288),("noise",156,156),("frosted",134,200)]
        if os.environ.get("OPENPDN_UI_SCOPE") == "remaining":
            cases=[("bokeh",90,90),("sketch",90,222),("quantize",112,112),("crystal",134,156),("dents",134,178),("pixelate",134,244),("redeye",200,222),("sharpen",200,244),("straighten",200,288),("clouds",222,222),("julia",222,244),("mandelbrot",222,266),("turbulence",222,288),("edges",244,244)]
        for name, category, row in cases:
            def open_filter():
                click(277,10);move(330,category);time.sleep(0.6);move(480,row);time.sleep(0.2);capture("filters-"+name+"-menu.png");click(480,row);time.sleep(0.5)
                if name=="straighten":
                    # Zero is the reference default; set an angle for the flow.
                    click(614,400);shortcut("a");key("1");key("2");key("Return");time.sleep(0.4)
                if name=="quantize":
                    # A black/white drawing is already losslessly representable.
                    click(614,400);shortcut("a");key("2");key("Return");time.sleep(0.4)
                if name=="zoom":
                    click(614,409);shortcut("a");key("1");key("Return");click(590,460);time.sleep(0.4)
                if name=="shadow":
                    capture("filters-shadow-before-edit.png")
                    click(543,449);time.sleep(0.4);capture("filters-shadow-color-picker.png")
                    click(620,797);click(700,590);click(700,335)
                    click(497,471);time.sleep(0.4)
                    assert capture("filters-shadow-colored-dialog.png").getpixel((543,449))!=(0,0,0),"shadow color control inert"
                if name=="oil":
                    click(540,436);click(540,478);capture("filters-oil-square.png")
            open_filter();capture("filters-"+name+"-dialog.png")
            stroke(400);key("Escape");assert capture("filters-"+name+"-cancel.png").crop(bounds).tobytes()==base,name+" modal/cancel"
            open_filter();accept_effect();time.sleep(0.4)
            accepted=capture("filters-"+name+"-accepted.png").crop(bounds).tobytes();assert accepted!=base or name in ("redeye","quantize"),name+" inert"
            if accepted==base:
                # An unchanged operation intentionally adds no history entry.
                shortcut("f");time.sleep(0.4)
                assert capture("filters-"+name+"-repeat.png").crop(bounds).tobytes()==base
                continue
            shortcut("z");wait_for(lambda:capture("filters-"+name+"-undo.png").crop(bounds).tobytes()==base,30)
            shortcut("f");wait_for(lambda:capture("filters-"+name+"-repeat.png").crop(bounds).tobytes()==accepted,30)
            shortcut("z");wait_for(lambda:capture("filters-"+name+"-base.png").crop(bounds).tobytes()==base,30)
            if name in ("noise","frosted"):
                open_filter();click(520,448 if name=="noise" else 460);accept_effect()
                randomized=capture("filters-"+name+"-randomized.png").crop(bounds).tobytes()
                assert randomized!=accepted,name+" Randomize inert"
                shortcut("z");shortcut("f")
                assert capture("filters-"+name+"-random-repeat.png").crop(bounds).tobytes()==randomized,name+" random seed not retained"
                shortcut("z");open_filter();click(520,448 if name=="noise" else 460)
                click(510,494 if name=="noise" else 506);accept_effect()
                assert capture("filters-"+name+"-reset.png").crop(bounds).tobytes()==accepted,name+" Reset changed instance seed"
                shortcut("z")
            if name=="frosted":
                open_filter();click(614,387);shortcut("a");key("5");key("Return")
                capture("filters-frosted-linked-min.png")
                click(614,363);shortcut("a");key("2");key("Return")
                capture("filters-frosted-linked-max.png");key("Escape")
                assert capture("filters-frosted-linked-cancel.png").crop(bounds).tobytes()==base
        print("Native filter scope",os.environ.get("OPENPDN_UI_SCOPE"),"preview/cancel/modal/commit/undo/repeat flows passed:",len(cases))
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "color-wheel":
        capture("wheel-initial.png");click(161,543);capture("wheel-expanded.png")
        click(280,655);shortcut("a")
        for c in "804020":key(c)
        key("Return");click(190,125)
        capture("wheel-brightness-base.png")
        click(201,632);image=capture("wheel-brightness-preserved.png")
        r,g,b=image.getpixel((25,570))
        assert r==127 and g<30 and b<30,"Wheel replaced current brightness"
        # Alt projects onto the original hue ray; a perpendicular cursor desaturates.
        modifier("Alt_L",True);click(132,697);modifier("Alt_L",False)
        gray=capture("wheel-hue-lock.png").getpixel((25,570))
        assert gray[0]==gray[1]==gray[2]==127,"Alt hue projection failed"
        # Ctrl keeps the zero saturation of that gray while changing hue.
        modifier("Control_L",True);click(70,625);modifier("Control_L",False)
        assert capture("wheel-radius-lock.png").getpixel((25,570))==gray,"Ctrl saturation lock failed"
        # Shift snaps the angle to 15 degrees; inspect the retained numeric hue.
        modifier("Shift_L",True);click(181,667);modifier("Shift_L",False)
        snap=capture("wheel-snap.png")
        r,g,b=snap.getpixel((25,570))
        assert r==127 and abs(g-(r+b)//2)<=1,"Shift hue snap did not select 30 degrees"
        before=snap.getpixel((25,570))
        move(201,632);xtest.fake_input(connection,X.ButtonPress,detail=3);connection.sync();time.sleep(0.2)
        capture("wheel-right-selector.png")
        xtest.fake_input(connection,X.ButtonRelease,detail=3);connection.sync();time.sleep(0.2)
        image=capture("wheel-opposite-color.png")
        assert image.getpixel((25,570))==before,"Right wheel gesture changed active color"
        r,g,b=image.getpixel((49,591));assert r==255 and g<30 and b<30,"Right wheel gesture did not edit opposite color"
        print("Native wheel brightness, Alt hue lock, Ctrl saturation lock, Shift snapping and opposite-color right gesture passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "selection-colors":
        initial=capture("colors-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(82,87);key("Escape");key("s");click(190,125)
        def number(x,value,y=87):
            click(x,y);shortcut("a")
            for c in str(value):key(c)
            key("Return");click(190,125)
        def fill(name,inside):
            key("f");click(*inside);shortcut("d");move(190,125)
            return capture(name)
        def reset():
            shortcut("z");shortcut("z");shortcut("z");key("s");move(190,125)
        modifier("Shift_L",True);drag((x0+100,y0+100),(x0+260,y0+140));modifier("Shift_L",False)
        square=fill("selection-square.png",(x0+110,y0+110))
        assert square.getpixel((x0+120,y0+120))==(0,0,0)
        assert square.getpixel((x0+145,y0+120))==(255,255,255),"Shift rectangle failed square constraint"
        reset();click(350,87);click(350,136)
        number(464,16);number(551,9)
        capture("selection-ratio-controls.png")
        drag((x0+100,y0+100),(x0+259,y0+259))
        ratio=fill("selection-ratio.png",(x0+120,y0+120))
        assert ratio.getpixel((x0+250,y0+180))==(0,0,0)
        assert ratio.getpixel((x0+150,y0+195))==(255,255,255),"Fixed ratio ignored"
        reset();click(350,87);click(350,161)
        number(464,40);number(551,30)
        drag((x0+100,y0+100),(x0+300,y0+250))
        fixed=fill("selection-fixed.png",(x0+310,y0+260))
        assert fixed.getpixel((x0+310,y0+260))==(0,0,0)
        assert fixed.getpixel((x0+290,y0+260))==(255,255,255),"Fixed size anchored at first press"
        assert fixed.getpixel((x0+345,y0+260))==(255,255,255),"Fixed width ignored"
        reset();move(190,125);capture("selection-saved-options.png")
        key("Tab");capture("colors-visible.png");click(161,543);capture("colors-expanded.png")
        # Editing RGB hex must preserve alpha and paint using the selected color.
        click(280,655);shortcut("a")
        for c in "12AB34":key(c)
        key("Return");click(25,809);click(19,726)
        capture("colors-custom-swatch.png")
        # Right-click chooses the opposite user color without changing which is active.
        move(43,726);xtest.fake_input(connection,X.ButtonPress,detail=3);xtest.fake_input(connection,X.ButtonRelease,detail=3);connection.sync();time.sleep(0.2)
        saved=capture("colors-final.png")
        assert saved.getpixel((25,570))==(18,171,52),"Hex or add-color action failed"
        assert saved.getpixel((49,591))==(255,0,0),"Right-click did not update the opposite color"
        window.send_event(protocol.event.ClientMessage(window=window.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        process.wait(timeout=15)
        process=subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY",str(ROOT/"target/debug/openpdn"))],stdout=log,stderr=log,env=app_env)
        window=wait_for(find_window);window.set_input_focus(X.RevertToParent,X.CurrentTime);connection.sync()
        wait_for(lambda:capture("colors-restart.png").getpixel((25,570))==(18,171,52))
        restarted=capture("colors-restart.png")
        assert restarted.getpixel((19,726))==(18,171,52),"Custom palette not persisted"
        assert restarted.crop((190,75,715,98)).tobytes()==saved.crop((190,75,715,98)).tobytes(),"Selection size/mode not persisted"
        print("Native Shift square, fixed ratio, cursor-anchored fixed size, RGB hex, swatch replacement, right-click secondary and restart persistence passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "picker":
        initial=capture("picker-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(82,87);key("Escape");key("g");drag((350,300),(650,300));key("b")
        move(100,120);base=capture("picker-base.png")
        def tool_label(image):
            # Focus changes text contrast and button borders; compare glyphs.
            pixels=image.crop((42,80,140,94)).convert("L").tobytes()
            threshold=(min(pixels)+max(pixels))/2
            return bytes(p<threshold for p in pixels)
        brush_tool=tool_label(base)
        key("p");pencil_tool=tool_label(capture("picker-pencil-tool.png"));key("b");key("k")
        # Drag continuously samples, and the default keeps Color Picker active.
        drag((x0+180,y0+200),(x0+260,y0+200));move(100,120)
        wait_for(lambda:capture("picker-picked.png").crop(bounds).tobytes()==base.crop(bounds).tobytes())
        key("p");click(x0+300,y0+400);move(100,120)
        painted=capture("picker-painted-color.png")
        expected=base.getpixel((x0+260,y0+200));actual=painted.getpixel((x0+300,y0+400))
        assert max(abs(a-b) for a,b in zip(expected,actual))<=1,(expected,actual)
        shortcut("z");assert capture("picker-paint-undo.png").crop(bounds).tobytes()==base.crop(bounds).tobytes()
        key("b");key("k");click(240,87);capture("picker-sampling-menu.png");click(240,136)
        click(540,87);capture("picker-behavior-menu.png");click(540,136)
        move(x0+240,y0+220);button(True);time.sleep(0.3)
        assert tool_label(capture("picker-before-release.png"))!=brush_tool,"Picker switched before release"
        button(False);move(100,120)
        wait_for(lambda:tool_label(capture("picker-previous-tool.png"))==brush_tool)
        key("k");click(540,87);click(540,158);click(x0+250,y0+220);move(100,120)
        wait_for(lambda:tool_label(capture("picker-switch-pencil.png"))==pencil_tool)
        assert capture("picker-final-pixels.png").crop(bounds).tobytes()==base.crop(bounds).tobytes()
        print("Color Picker drag sampling, chosen color, sample size, release-only previous/Pencil switch and unchanged pixels passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "brush-options":
        initial=capture("brush-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(82,87);key("Escape");key("b")
        click(190,125)
        def number(x,value):
            click(x,87);shortcut("a")
            for c in str(value):key(c)
            key("Return");click(190,125)
        number(288,40);number(810,100)
        move(190,125);base=capture("brush-controls.png").crop(bounds)
        def draw(name):
            drag((x0+200,y0+200),(x0+450,y0+200));move(190,125)
            wait_for(lambda:capture(name).crop(bounds).tobytes()!=base.tobytes())
            result=capture(name).crop(bounds)
            shortcut("z");move(190,125)
            wait_for(lambda:capture("brush-undo.png").crop(bounds).tobytes()==base.tobytes())
            return result
        hard=draw("brush-hard.png")
        number(810,0);soft=draw("brush-soft.png")
        assert hard.tobytes()!=soft.tobytes(),"Hardness control inert"
        gray=lambda im:sum(1 for r,g,b in im.get_flattened_data() if r==g==b and 0<r<255)
        assert gray(soft)>gray(hard),"Soft brush did not feather"
        click(973,87);alias=draw("brush-aliased.png")
        assert gray(alias)==0,"Aliased brush produced fractional coverage"
        click(660,87);capture("brush-shape-menu.png");click(660,136);click(190,125)
        square=draw("brush-square.png")
        assert alias.tobytes()!=square.tobytes(),"Square brush choice inert"
        number(922,500);sparse=draw("brush-spacing.png")
        assert sparse.getpixel((300,200))==(255,255,255),"Spacing did not leave gaps"
        assert sparse.getpixel((200,200))!=(255,255,255),"Spaced brush missing its initial stamp"
        assert square.tobytes()!=sparse.tobytes(),"Spacing control inert"
        click(1077,87);click(1177,87);click(190,125)
        saved=capture("brush-final-options.png")
        assert saved.crop(bounds).tobytes()==base.tobytes(),"Option edits changed the image"
        window.send_event(protocol.event.ClientMessage(window=window.id,client_type=connection.intern_atom("WM_PROTOCOLS"),data=(32,[connection.intern_atom("WM_DELETE_WINDOW"),X.CurrentTime,0,0,0])),event_mask=0);connection.sync()
        process.wait(timeout=15)
        process=subprocess.Popen([os.environ.get("OPENPDN_UI_BINARY",str(ROOT / "target/debug/openpdn"))],stdout=log,stderr=log,env=app_env)
        window=wait_for(find_window);window.set_input_focus(X.RevertToParent,X.CurrentTime);connection.sync()
        wait_for(lambda:capture("brush-restart.png").crop((190,75,1240,98)).tobytes()==saved.crop((190,75,1240,98)).tobytes())
        print("Native brush hardness, antialiasing, circle/square, spacing, option toggles, one-step undo and settings restart passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "drawing-options":
        initial=capture("options-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(82,87);key("Escape");key("b")
        base=capture("options-base.png").crop(bounds).tobytes()
        click(515,87);capture("options-pattern-menu.png");click(510,136)
        drag((x0+200,y0+200),(x0+450,y0+200))
        patterned=capture("options-pattern-brush.png").crop(bounds).tobytes()
        assert patterned!=base,"Pattern brush inert"
        shortcut("z");assert capture("options-pattern-undo.png").crop(bounds).tobytes()==base
        pattern_label=capture("options-pattern-label.png").crop((445,76,570,98)).tobytes()
        click(515,87);capture("options-reset-menu.png");click(510,113)
        wait_for(lambda:capture("options-solid-label.png").crop((445,76,570,98)).tobytes()!=pattern_label)
        drag((x0+200,y0+200),(x0+450,y0+200))
        solid=capture("options-solid-brush.png").crop(bounds).tobytes()
        assert solid!=patterned,"Pattern fill choice did not alter the brush"
        shortcut("z")
        click(380,87);capture("options-blend-menu.png");click(380,136)
        capture("options-blend-selected.png")
        print("Native brush pattern selection, solid reset, drawing and undo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "gaussian":
        initial=capture("gaussian-initial.png");bounds=canvas_bounds(initial)
        key("Tab");click(82,87);key("Escape");key("g");drag((350,300),(600,300));key("b")
        stroke(300);stroke(470);base=capture("gaussian-base.png").crop(bounds).tobytes()
        results=[]
        for quality in range(1,5):
            def open_gaussian():
                click(277,10);move(330,90);time.sleep(0.6);click(480,134);time.sleep(0.5)
                click(614,388);shortcut("a");key("3");key("0");key("Return")
                click(614,436);shortcut("a");key(str(quality));key("Return");time.sleep(0.6)
                capture("gaussian-quality-"+str(quality)+"-dialog.png")
            open_gaussian();stroke(400);key("Escape")
            assert capture("gaussian-quality-"+str(quality)+"-cancel.png").crop(bounds).tobytes()==base
            open_gaussian();accept_effect();time.sleep(0.4)
            accepted=capture("gaussian-quality-"+str(quality)+"-accepted.png").crop(bounds).tobytes()
            assert accepted!=base,"Gaussian quality inert: "+str(quality)
            results.append(accepted)
            shortcut("z");wait_for(lambda:capture("gaussian-undo.png").crop(bounds).tobytes()==base,30)
            shortcut("f");wait_for(lambda:capture("gaussian-repeat.png").crop(bounds).tobytes()==accepted,30)
            shortcut("z");wait_for(lambda:capture("gaussian-base-again.png").crop(bounds).tobytes()==base,30)
        assert len(set(results))==4,"Gaussian quality choices did not produce distinct output"
        print("All four Gaussian qualities passed preview/cancel/modal/accept/undo/repeat")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "text":
        bounds=canvas_bounds(capture("text-initial.png"));x0,y0,_,_=bounds
        key("Tab");click(82,87);key("Escape")
        base=capture("text-base.png").crop(bounds).tobytes()
        position=(x0+250,y0+170)
        def type_letters(value):
            for letter in value:
                if letter=="\n":key("Return")
                elif letter==" ":key("space")
                elif letter==".":key("period")
                else:key(letter)
        def start():
            key("t");click(*position)
        def finish(name):
            key("Escape");key("b")
            return capture(name).crop(bounds).tobytes()
        start();type_letters("abcd");key("Home");key("Right");key("Delete");key("x")
        key("End");key("BackSpace");key("Return");type_letters("line")
        key("Home");key("Up");key("Home");key("Delete")
        actual=finish("text-navigation.png")
        assert actual!=base,"Inline typing did not paint text"
        shortcut("z");assert capture("text-navigation-undo.png").crop(bounds).tobytes()==base
        start();type_letters("xc\nline");expected=finish("text-navigation-expected.png")
        assert actual==expected,"Canvas caret/navigation produced unexpected text"
        shortcut("z")
        start();type_letters("one... two");key("Left",("Control_L",));key("BackSpace",("Control_L",));type_letters(" ")
        actual=finish("text-word-delete.png");shortcut("z")
        start();type_letters("one two");expected=finish("text-word-expected.png")
        assert actual==expected,"Word deletion differs from TextTool"
        shortcut("z")
        start();type_letters("first")
        click(x0+600,y0+400);type_letters("second");finish("text-two-edits.png")
        shortcut("z");first=capture("text-second-undo.png").crop(bounds).tobytes()
        assert first!=base
        shortcut("z");assert capture("text-first-undo.png").crop(bounds).tobytes()==base
        start();type_letters("first");expected=finish("text-first-expected.png")
        assert first==expected,"A second text edit overwrote the first transaction"
        print("Inline Text typing, caret navigation, word deletion, Escape acceptance, separate edits and undo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "content":
        initial=capture("content-initial.png");bounds=canvas_bounds(initial)
        x0,y0,_,_=bounds
        key("Tab")  # Keep floating utility windows out of the content comparison.
        base=capture("content-base.png").crop(bounds).tobytes()
        click(82,87);key("Escape");key("t");click(x0+260,y0+160)
        capture("content-text-start.png")
        for letter in "Rust": key(letter)
        key("Return")
        for letter in "Paint": key(letter)
        capture("content-text-edit.png")
        click(391,87);drag((354,87),(370,87))
        # Original TextTool moves by right-drag anywhere in the canvas.
        move(x0+260,y0+160)
        xtest.fake_input(connection, X.ButtonPress, detail=3);connection.sync()
        move(x0+320,y0+180);time.sleep(0.12)
        xtest.fake_input(connection, X.ButtonRelease, detail=3);connection.sync();time.sleep(0.3)
        capture("content-text-styled-moved.png")
        key("Return",("Control_L",));key("b")
        text=capture("content-text-committed.png").crop(bounds).tobytes()
        text_image=Image.open(OUT / "content-text-committed.png")
        ink=sum(text_image.getpixel((x,y))!=(255,255,255) for x in range(x0+250,x0+400) for y in range(y0+170,y0+250))
        assert text!=base and ink>150,"Text did not rasterize actual letters"
        shortcut("z");wait_for(lambda:capture("content-text-undo.png").crop(bounds).tobytes()==base)
        shortcut("y");wait_for(lambda:capture("content-text-redo.png").crop(bounds).tobytes()==text)
        shortcut("z")
        key("t");click(x0+260,y0+160);key("x");key("Escape");key("b")
        assert capture("content-text-escape.png").crop(bounds).tobytes()!=base,"Escape must accept text, as in TextTool"
        shortcut("z")
        assert capture("content-text-escape-undo.png").crop(bounds).tobytes()==base,"Escape acceptance was not one undo step"
        key("o");key("o");drag((x0+250,y0+280),(x0+480,y0+440));capture("content-shape-live.png")
        drag((x0+365,y0+360),(x0+425,y0+330));capture("content-shape-moved.png")
        drag((x0+540,y0+410),(x0+600,y0+460));capture("content-shape-resized.png")
        key("Return",("Control_L",));key("b")
        shape=capture("content-shape-committed.png").crop(bounds).tobytes()
        assert shape!=base,"Shape did not rasterize"
        shortcut("z");wait_for(lambda:capture("content-shape-undo.png").crop(bounds).tobytes()==base)
        shortcut("y");wait_for(lambda:capture("content-shape-redo.png").crop(bounds).tobytes()==shape)
        shortcut("z");key("o");key("o");drag((x0+250,y0+280),(x0+480,y0+440));key("Escape");key("b")
        assert capture("content-shape-cancel.png").crop(bounds).tobytes()==base,"Shape cancellation changed pixels"
        key("o");key("o");modifier("Shift_L",True);modifier("Alt_L",True)
        drag((x0+400,y0+300),(x0+500,y0+360))
        modifier("Alt_L",False);modifier("Shift_L",False)
        key("Return",("Control_L",));key("b")
        centered=capture("content-shape-centered-constrained.png")
        assert centered.getpixel((x0+340,y0+300))==(0,0,0) and centered.getpixel((x0+400,y0+240))==(0,0,0),"Centered constrained rectangle has incorrect bounds"
        assert centered.getpixel((x0+320,y0+300))==(255,255,255),"Centered rectangle ignored its constrained extent"
        shortcut("z");assert capture("content-shape-modifiers-undo.png").crop(bounds).tobytes()==base
        click(215,10);capture("content-adjustments-menu.png");key("Escape")
        print("Text input/style/move/commit/cancel/undo/redo and Shapes drag/move/resize/center/constrain/commit/cancel/undo/redo passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "tonal":
        initial=capture("tonal-initial.png");bounds=canvas_bounds(initial);x0,y0,_,_=bounds
        key("Tab");click(x0+80,y0+80);key("g");drag((x0+160,y0+250),(x0+440,y0+250));key("b")
        base=capture("tonal-base.png").crop(bounds).tobytes()
        def modal(name):
            image=capture(name)
            assert image.getpixel((x0+780,y0+20))[0]<220,"Expected a modal dialog"
        def undo_to_base(name):
            shortcut("z");wait_for(lambda:capture(name).crop(bounds).tobytes()==base)
        click(215,10);click(230,234);time.sleep(0.5);modal("tonal-levels-dialog.png")
        click(570,467);capture("tonal-levels-preview.png");stroke(400)
        key("Escape");assert capture("tonal-levels-cancel.png").crop(bounds).tobytes()==base
        click(215,10);click(230,234);time.sleep(0.5);click(570,467);capture("tonal-levels-second-preview.png");accept_effect()
        leveled=capture("tonal-levels-accepted.png").crop(bounds).tobytes();assert leveled!=base
        undo_to_base("tonal-levels-undo.png");shortcut("y");wait_for(lambda:capture("tonal-levels-redo.png").crop(bounds).tobytes()==leveled);undo_to_base("tonal-levels-base.png")
        click(215,10);click(230,103);time.sleep(0.5);modal("tonal-curves-dialog.png")
        click(618,350);capture("tonal-curves-preview.png");key("Escape");assert capture("tonal-curves-cancel.png").crop(bounds).tobytes()==base
        click(215,10);click(230,103);time.sleep(0.5);click(618,350);drag((618,350),(630,370));accept_effect()
        curved=capture("tonal-curves-accepted.png").crop(bounds).tobytes();assert curved!=base
        undo_to_base("tonal-curves-undo.png");shortcut("y");wait_for(lambda:capture("tonal-curves-redo.png").crop(bounds).tobytes()==curved);undo_to_base("tonal-curves-base.png")
        click(277,10);move(330,244);time.sleep(0.5);click(480,266);time.sleep(0.5);modal("tonal-emboss-dialog.png")
        key("Escape");assert capture("tonal-emboss-cancel.png").crop(bounds).tobytes()==base
        click(277,10);move(330,244);time.sleep(0.5);click(480,266);time.sleep(0.5);accept_effect()
        embossed=capture("tonal-emboss-accepted.png").crop(bounds).tobytes();assert embossed!=base
        undo_to_base("tonal-emboss-undo.png");shortcut("f");wait_for(lambda:capture("tonal-emboss-repeat.png").crop(bounds).tobytes()==embossed);undo_to_base("tonal-emboss-base.png")
        click(277,10);move(330,134);time.sleep(0.5);capture("tonal-distort-menu.png");click(480,222);time.sleep(0.5);modal("tonal-morphology-dialog.png")
        key("Escape");assert capture("tonal-morphology-cancel.png").crop(bounds).tobytes()==base
        click(277,10);move(330,134);time.sleep(0.5);click(480,222);time.sleep(0.5);accept_effect()
        assert capture("tonal-morphology-accepted.png").crop(bounds).tobytes()!=base
        undo_to_base("tonal-morphology-undo.png")
        click(215,10);click(230,36);time.sleep(0.5)
        assert capture("tonal-auto-level.png").crop(bounds).tobytes()!=base
        undo_to_base("tonal-auto-undo.png")
        print("Levels/Curves preview/cancel/commit/undo/redo, modal isolation, Auto-Level, Emboss repeat and Morphology passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "tools":
        initial=capture("remaining-initial.png");bounds=canvas_bounds(initial)
        x0,y0,_,_=bounds
        stroke(80);capture("remaining-base.png");base=canvas("remaining-base.png")
        key("g");drag((x0+160,y0+250),(x0+440,y0+250));capture("remaining-gradient.png")
        assert canvas("remaining-gradient.png")!=base,"Gradient did not paint"
        shortcut("z");wait_for(lambda:capture("remaining-gradient-undo.png").crop(bounds).tobytes()==base)
        move(x0+160,y0+250);button(True);move(x0+400,y0+250);time.sleep(0.5);key("Escape");button(False);time.sleep(0.4)
        assert capture("remaining-gradient-cancel.png").crop(bounds).tobytes()==base,"Escape did not roll back the gradient"
        key("l")
        xtest.fake_input(connection,X.KeyPress,detail=connection.keysym_to_keycode(XK.string_to_keysym("Control_L")));connection.sync()
        click(x0+120,y0+93)
        xtest.fake_input(connection,X.KeyRelease,detail=connection.keysym_to_keycode(XK.string_to_keysym("Control_L")));connection.sync()
        click(x0+500,y0+200);key("b");capture("remaining-clone.png")
        assert canvas("remaining-clone.png")!=base,"Clone did not copy the selected source"
        shortcut("z");wait_for(lambda:capture("remaining-clone-undo.png").crop(bounds).tobytes()==base)
        key("r");drag((x0+500,y0+300),(x0+580,y0+300));key("b");capture("remaining-recolor.png")
        assert canvas("remaining-recolor.png")!=base,"Recolor did not replace the secondary color"
        shortcut("z");wait_for(lambda:capture("remaining-recolor-undo.png").crop(bounds).tobytes()==base)
        click(215,10);click(230,80);time.sleep(0.6);capture("remaining-brightness.png")
        key("Escape");time.sleep(0.6)
        assert capture("remaining-brightness-cancel.png").crop(bounds).tobytes()==base
        click(215,10);click(230,124);time.sleep(0.6);capture("remaining-exposure.png")
        key("Escape");time.sleep(0.6)
        click(215,10);click(230,167);time.sleep(0.6);capture("remaining-hue.png")
        key("Escape");time.sleep(0.6)
        assert capture("remaining-adjustment-cancel.png").crop(bounds).tobytes()==base
        click(277,10);time.sleep(0.5);capture("remaining-effects-menu.png")
        move(330,200);time.sleep(0.6);capture("remaining-photo-menu.png")
        click(480,310);time.sleep(0.6);capture("remaining-vignette-dialog.png")
        key("Escape");time.sleep(0.6)
        assert capture("remaining-vignette-cancel.png").crop(bounds).tobytes()==base,"Vignette cancel did not restore pixels"
        click(277,10);move(330,200);time.sleep(0.5);click(480,310);time.sleep(0.6)
        accept_effect();time.sleep(0.6);capture("remaining-vignette-accepted.png")
        accepted=canvas("remaining-vignette-accepted.png")
        assert accepted!=base,"Vignette did not commit"
        shortcut("z");wait_for(lambda:capture("remaining-vignette-undo.png").crop(bounds).tobytes()==base)
        shortcut("y");wait_for(lambda:capture("remaining-vignette-redo.png").crop(bounds).tobytes()==accepted)
        shortcut("f");time.sleep(0.6);capture("remaining-repeat.png")
        assert canvas("remaining-repeat.png")!=accepted,"Repeat Last Effect did not reuse the vignette"
        shortcut("z");wait_for(lambda:capture("remaining-repeat-undo.png").crop(bounds).tobytes()==accepted)
        shortcut("z");wait_for(lambda:capture("remaining-effects-base.png").crop(bounds).tobytes()==base)
        click(277,10);move(330,244);time.sleep(0.6);capture("remaining-stylize-menu.png");click(480,310);time.sleep(0.6);capture("remaining-relief-dialog.png")
        key("Escape");time.sleep(0.6)
        assert capture("remaining-relief-cancel.png").crop(bounds).tobytes()==base
        print("Gradient paint/undo/cancel, Clone Stamp source/undo, Recolor/undo, adjustment/effect dialogs, Vignette cancel/commit/undo/redo and Ctrl+F repeat passed")
        raise SystemExit(0)
    if os.environ.get("OPENPDN_UI_SCOPE") == "drop":
        drop_file(ROOT / "tests/fixtures/pdn/small.pdn")
        capture("29-drop-choice.png")
        key("Tab");key("Return")
        wait_for(lambda: "small.pdn" in (window.get_wm_name() or ""))
        capture("30-dropped-pdn-open.png")
        print("Native Xdnd transfer, open choice and original PDN opening passed")
        raise SystemExit(0)
    # Use an explicitly opaque, connected fixture. A tolerance-zero wand on a
    # soft two-pixel stroke correctly selects only one exact alpha value.
    for x, value in [(288, "20"), (810, "100")]:
        click(x,87);shortcut("a")
        for c in value:key(c)
        key("Return");click(190,125)
    click(973,87);click(190,125)  # Disable brush antialiasing for this fixture.
    initial = capture("01-initial.png")
    bounds = canvas_bounds(initial)
    stroke(80)
    wait_for(lambda: "*" in (window.get_wm_name() or ""))
    capture("02-stroke.png")
    assert set(capture("fixture-opaque-stroke.png").crop(bounds).get_flattened_data()) <= {(0,0,0),(255,255,255)}, "Wand fixture must have opaque, aliased edges"
    shortcut("z")
    wait_for(lambda: "*" not in (window.get_wm_name() or ""))
    wait_for(lambda: capture("03-undo.png").crop(bounds).tobytes() == canvas("01-initial.png"))
    shortcut("y")
    wait_for(lambda: "*" in (window.get_wm_name() or ""))
    wait_for(lambda: capture("04-redo.png").crop(bounds).tobytes() == canvas("02-stroke.png"))
    assert canvas("01-initial.png") != canvas("02-stroke.png"), "Drawing did not change canvas pixels"
    assert canvas("03-undo.png") == canvas("01-initial.png"), "Undo did not restore the canvas"
    assert canvas("04-redo.png") == canvas("02-stroke.png"), "Redo did not restore the stroke"
    shortcut("n", shift=True)
    assert "*" in (window.get_wm_name() or ""), "Add Layer shortcut was intercepted by New Image"
    stroke(290)
    capture("06-layer-stroke.png")
    assert canvas("06-layer-stroke.png") != canvas("04-redo.png"), "Active layer drawing did not change pixels"
    click(1252, 596)  # Visibility checkbox for the top row at the default 1280x850 layout.
    wait_for(lambda: capture("07-hidden-layer.png").crop(bounds).tobytes() == canvas("04-redo.png"))
    assert canvas("07-hidden-layer.png") == canvas("04-redo.png"), "Hidden layer still contributes pixels"
    shortcut("z")
    wait_for(lambda: capture("08-undo-visibility.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    assert canvas("08-undo-visibility.png") == canvas("06-layer-stroke.png"), "Visibility undo did not restore pixels"
    # Clicking a History entry rewinds, restores, and toggles the adjacent state.
    click(1100, 146)
    wait_for(lambda: capture("history-start.png").crop(bounds).tobytes() == canvas("01-initial.png"))
    click(1100, 218)
    wait_for(lambda: capture("history-restore.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    click(1100, 218)
    wait_for(lambda: capture("history-adjacent.png").crop(bounds).tobytes() == canvas("04-redo.png"))
    click(1100, 218)
    wait_for(lambda: capture("history-restore-again.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    # Every utility window toggles independently. Tab hides/restores the complete set.
    for function, area in [("F5", (7, 104, 69, 390)), ("F6", (1061, 104, 1272, 366)), ("F7", (1061, 542, 1272, 820)), ("F8", (7, 545, 218, 806))]:
        before = capture("workspace-before-toggle.png")
        key(function)
        hidden = capture(f"hidden-{function}.png")
        assert hidden.crop(area).tobytes() != before.crop(area).tobytes(), f"{function} did not hide its window"
        key(function)
    key("Tab")
    hidden = capture("09-hidden-windows.png")
    assert hidden.crop((7, 104, 69, 390)).tobytes() != initial.crop((7, 104, 69, 390)).tobytes()
    key("Tab")
    wait_for(lambda: capture("09-restored-windows.png").getpixel((10, 140)) != (207, 207, 207))
    # Place History over the image and click its blank content: painting must be blocked.
    drag((1140, 117), (670, 267))
    moved_history = capture("10-moved-history.png")
    # Sample below the history labels: a disabled redo entry can cross row 400
    # and split the otherwise uniform panel background.
    hx0, hx1 = longest_run([247 <= moved_history.getpixel((x, 430))[0] <= 249 and len(set(moved_history.getpixel((x, 430)))) == 1 for x in range(1280)])
    assert 150 < hx1 - hx0 < 260, "Moved History content could not be located"
    click((hx0 + hx1) // 2, 430)
    key("F6")
    wait_for(lambda: capture("11-panel-hit-isolation.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    assert canvas("11-panel-hit-isolation.png") == canvas("06-layer-stroke.png"), "Floating window allowed a canvas stroke underneath"
    key("F6")
    key("F6", ("Control_L", "Shift_L"))
    wait_for(lambda: capture("12-reset-history.png").getpixel((1070, 140)) != (207, 207, 207))
    key("F4")
    capture("13-layer-properties.png")
    stroke(80)  # Outside the modal dialog but inside the image.
    key("Escape")
    wait_for(lambda: capture("14-modal-hit-isolation.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    assert canvas("14-modal-hit-isolation.png") == canvas("06-layer-stroke.png"), "Modal dialog allowed canvas editing underneath"
    shortcut("n")
    capture("05-new-dialog.png")
    key("Escape")
    # Magic Wand selects only the connected painted region on the active layer.
    click(24, 223)
    x0, y0, _, _ = bounds
    click(x0 + 90, y0 + 293)
    key("Delete")
    shortcut("d")
    wait_for(lambda: capture("15-wand-erased.png").crop(bounds).tobytes() == canvas("04-redo.png"))
    shortcut("z")  # Erase also restores the selection
    shortcut("d")
    wait_for(lambda: capture("16-wand-undo.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    # A freehand lasso encloses the second stroke, without changing the first layer.
    click(24, 173)
    move(x0 + 60, y0 + 270); button(True); time.sleep(0.35)
    for x, y in [(410,270),(410,430),(60,430),(60,270)]:
        move(x0+x,y0+y); time.sleep(0.35)
    button(False);time.sleep(0.3)
    capture("17-lasso.png")
    key("Delete");shortcut("d")
    wait_for(lambda: capture("18-lasso-erased.png").crop(bounds).tobytes() == canvas("04-redo.png"))
    shortcut("z");shortcut("d")
    wait_for(lambda: capture("19-lasso-undo.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    # Move selected pixels handles overlapping source/destination and one-step undo.
    click(24,148)
    drag((x0+60,y0+270),(x0+410,y0+430))
    click(49,148)
    drag((x0+100,y0+300),(x0+130,y0+335))
    shortcut("d")
    assert capture("20-moved-pixels.png").crop(bounds).tobytes() != canvas("06-layer-stroke.png")
    shortcut("z");shortcut("z");shortcut("d")
    wait_for(lambda: capture("21-move-undo.png").crop(bounds).tobytes() == canvas("06-layer-stroke.png"))
    # Review the complete adjustment menu and custom glyphs.
    click(215,10)
    capture("22-adjustments-menu.png")
    key("Escape")
    # Color a stroke, then prove a GPU preview changes pixels outside the modal.
    click(42,774)
    click(24,277)
    stroke(60)
    colored = capture("23-colored-stroke.png")
    sample = (x0 + 150, y0 + 84)
    assert colored.getpixel(sample)[0] > colored.getpixel(sample)[1], "Palette did not select red"
    click(215,10); click(230,278)
    wait_for(lambda: capture("24-sepia-preview.png").getpixel(sample) != colored.getpixel(sample))
    time.sleep(0.6)  # Let the modal dimming animation settle before comparing pixels.
    capture("24-sepia-preview.png")
    stroke(80)  # A preview modal must block edits outside its bounds.
    preview_before = Image.open(OUT / "24-sepia-preview.png")
    preview_after = capture("24b-preview-isolation.png")
    assert preview_before.crop((x0,y0,x0+800,y0+200)).tobytes() == preview_after.crop((x0,y0,x0+800,y0+200)).tobytes(), "Preview modal allowed a canvas stroke"
    key("Escape")
    wait_for(lambda: capture("25-sepia-cancel.png").crop(bounds).tobytes() == canvas("23-colored-stroke.png"))
    # Several previews still produce a single accepted history transaction.
    click(215,10);click(230,278)
    click(495,412);click(590,412);click(540,412)
    click(547,458)
    wait_for(lambda: capture("26-sepia-accepted.png").getpixel(sample) != colored.getpixel(sample))
    time.sleep(0.6);capture("26-sepia-accepted.png")
    shortcut("z")
    wait_for(lambda: capture("27-sepia-undo.png").crop(bounds).tobytes() == canvas("23-colored-stroke.png"))
    shortcut("y")
    wait_for(lambda: capture("28-sepia-redo.png").crop(bounds).tobytes() == canvas("26-sepia-accepted.png"))
    drop_file(ROOT / "tests/fixtures/pdn/small.pdn")
    capture("29-drop-choice.png")
    key("Tab");key("Return")
    wait_for(lambda: "small.pdn" in (window.get_wm_name() or ""))
    capture("30-dropped-pdn-open.png")
    print(f"Native editing, workspace/modal isolation, Magic Wand, lasso, move/undo, GPU preview/commit/undo and native PDN file drop passed; captures: {OUT}")
finally:
    process.terminate()
    try:
        process.wait(timeout=5)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=15)
    connection.close()
    if test_clipboard is not None:
        test_clipboard.close()
    log.close()
    settings.cleanup()
