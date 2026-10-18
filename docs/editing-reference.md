# Document editing and workspace evidence

Updated 2026-10-12. This work closes several disabled editor commands. It does not complete Paint.NET UI, file-format, renderer or native-platform parity.

## Recovered behavior

The supplied 5.1.12 `paintdotnet.dll` was inspected with ILSpy. Original code remains in ignored `reference/`; only independent Rust implementations and numeric fixtures are distributed.

- `CropToSelectionFunction.OnExecute` creates a document of the selection's clipped bounds, copies each layer's properties and document metadata, applies the actual selection alpha mask to every cropped layer, and clears the selection. The Rust command follows that shape rather than retaining everything inside a rectangular bounding box.
- `CanvasSizeAction.ResizeLayer` fills each resized layer with the chosen RGBA color, positions the old surface using one of nine anchors, copies overlapping pixels and retains the layer properties. Its midpoint calculation uses integer division towards zero, including odd negative size deltas. Transparent, primary, secondary, white and black fill choices are available.
- `MergeLayerDownFunction.OnExecute` applies the upper layer's blend mode and opacity to the lower layer's raw pixels, removes the upper layer and retains the lower layer's properties. It ignores both layer visibility and the selection. This can change the overall composition when the lower layer has nondefault properties, just as the recovered function does.
- `ImageMenu` supplies Crop Ctrl+Shift+X, Resize Ctrl+R, Canvas Size Ctrl+Shift+R, clockwise Ctrl+H, counterclockwise Ctrl+G and Flatten Ctrl+Shift+F. `EditMenu` supplies Cut/Copy/Paste, Copy Merged Ctrl+Shift+C, Paste into New Layer Ctrl+Shift+V and Paste into New Image Ctrl+Alt+V. These commands are wired to menus and corresponding existing handcrafted toolbar icons. Merge Down also has its Ctrl+M shortcut.

## GPU resizing and selection display

Resize uses separable GPU convolution, premultiplied alpha and optional linear-light processing. It offers Nearest Neighbor, Bilinear, Bicubic, Lanczos 3 and area/Fant sampling, percentage/absolute dimensions and aspect-ratio locking. The smaller intermediate orientation is selected; dimensions, storage and dispatch are checked before allocation. Layer properties and metadata survive, selection clears, and acceptance creates one undo transaction. Invalid dimensions and cancellation leave the document unchanged.

The five portable samplers are **not certified against the original WIC/color-profile output**. The reference has eight choices; Cubic Smooth, Linear Low Quality and Adaptive High Quality remain missing. Physical dimensions and resolution controls are implemented; exact last-used defaults, progress/cancellation during large resizes and original dialog styling remain pending.

Selection outlines now follow mask edges and holes. A packed bit mask is uploaded; the GPU marks black/white boundary pixels in the display texture. Document/composite output buffers remain unchanged, so exports and image processing do not include selection decoration. Exact marching animation, screen-space thickness, zoom/DPI and fractional reference selection coverage remain pending.

## Clipboard and persistence

Image clipboard operations use arboard's native Windows, macOS, X11 and Wayland data-control implementations. Copy uses the selected active-layer pixels; Copy Merged composites visible layers; Cut copies successfully before erasing. Paste can target the active layer, a new layer or a new document. Oversized images offer Expand canvas, Keep canvas size or Cancel. Keep retains the full source bitmap so off-canvas content can be moved back into view. Image bytes are read from the system clipboard; remembered application-local origin metadata is used only if the retrieved pixels still match. Pastes enter Move Selected Pixels as a floating transaction. Source pixels are retained through movement, scaling and rotation. Accepting pixels records a Paste transaction; an added layer and canvas expansion have preceding, separate undo entries, following the original actions. Cancelling the size prompt changes no document or history. Cancelling the live transform restores the prepared canvas/layer state; further undo steps remove those preparations.

X11 image clipboard flows were exercised locally. Other native clipboard backends, external-application format negotiation, reference vector selection clipboard formats and color-profile interoperability remain uncertified.

eframe persistence stores the window geometry and egui floating-window positions. Application preferences preserve utility visibility, reset generation, grid/rulers, expanded colors, primary/secondary colors, tool, brush size and tolerance. Tab's temporary hiding state is not saved as permanent visibility. A clean native X11 restart test confirms a moved History window, selected Gradient tool, hidden Colors window and grid state. Tests use temporary settings directories and do not overwrite desktop preferences. This is basic persistence; original docking, snapping, inactive transparency and per-document viewport restoration remain pending.

The native accessibility bridge is enabled. No screen-reader or keyboard-only certification is claimed.

### Clipboard continuation, 2026-10-18

The frozen 5.1.12 `NewImageAction` asks the native clipboard reader for image
pixel dimensions on every invocation, falling back to `GetNewDocumentSize`.
That method uses an 800×600 DIP default (600×800 for a portrait screen), scaled
to the display DPI. Clipboard dimensions replace that default without supplying
the physical print resolution. `NewFileDialog` uses the default physical DPU and
the remembered aspect constraint. The Rust New command now reads the clipboard
afresh through Ctrl+N, File → New and the toolbar. It uses the copied bitmap's
pixel width/height and 96 DPI; absent, unreadable or unsupported image data falls
back to the existing 800×600 default. It retains the aspect option while rebasing
its ratio to the new image. Recognized images above the pixel cap retain their
true dimensions so the dialog can report its size limit. Portrait-screen/DPI-scaled fallback sizes, persisted
last physical units and native vector-selection clipboard sizing remain open.

`PasteImageAction.ShowExpandCanvasTaskDialog` and the original resource strings
confirm three independent choices with Expand as the default and Cancel as the
escape action. The separate `PasteInToNewLayerAction` creates its blank layer
first and steps back if paste is cancelled. Expansion uses the maximum of each
old/image dimension, a top-left anchor and transparent fill. Rust now preserves
the same successful history order: Add New Layer, Canvas Size, Paste, followed
by the Move tool's Deselect when Enter/Escape accepts. Only applicable steps are
recorded. The complete GPU preview is validated before committing preparation
history, so failed paste setup leaves no empty layer or canvas edit. The size
dialog retains one captured bitmap without cloning its pixels every frame.
It supports the three buttons, default/focused Enter, Escape and Alt+E/K/C.

The isolated `clipboard` UI scope serves real PNG bytes from a separate X11
clipboard owner, on a private Xvfb display. It checks fresh New fields through
all three entry points, text-only fallback, both paste destinations and all size
choices, mouse and keyboard actions, separate layer/canvas/pixel undo and redo,
movement of previously clipped source pixels, captured-image stability when the
clipboard changes mid-dialog, and missing-image error recovery. The existing
floating-transform scope also passes. Both use RTX 5080/Vulkan; no desktop
clipboard is read or replaced. CPU tests verify dimensions, metadata/layer
properties, insertion order, undo/redo, cancellation/redo-branch preservation,
invalid images, layer limits and expansion memory preflight. Captures and logs
are in `artifacts/clipboard-20261018/`. Exact original task-dialog artwork and
cross-application/platform format negotiation remain uncertified.

## Verification

- Three CPU tests exercise all nine canvas anchors at enlarged/shrunk/unchanged dimensions, nonrectangular/partial-alpha crops, merge properties/visibility/selection, selected/merged bitmap copies, clipped pastes, invalid inputs and history.
- Two explicit RTX 5080/Vulkan tests cover five samplers in both gamma modes at four output geometries, nearest-neighbor coordinates, constant-field preservation, transparent-edge colors, linear-light averaging, metadata, invalid sizes and undo. A display-texture readback checks every selection outline pixel, including the hole, and verifies unchanged document pixels.
- The isolated `editing` UI scope checks resize/canvas dialogs, accept/cancel/undo, bitmap cut/copy/paste/new-layer/new-image, merge and crop. The `persistence` scope closes and restarts the native app using the same isolated preference directory.
- A Windows GNU release executable cross-compiles on Linux, starts under Wine and detects RTX 5080/Vulkan. The previous milestone passed 61 standard Windows-target tests and all 14 explicit hardware tests pass under Wine, including fontless text, original geometry/processing fixtures, PDN and floating transforms. This is not native Windows/D3D12 validation. The Apple Silicon Rust target passes compilation checks. Native Apple M2 Pro / Metal now passes 74 standard tests, fourteen explicit hardware tests, release startup, PDN rendering and an actual app-bundle launch; see [native validation](native-validation.md).
- Final Linux/Wine packages launch, render the original PDN fixture identically and identify the discrete adapter. Linux packaging caps imported glibc symbols at 2.35. A supplemental older-runtime CLI probe passes, but GPU loading in the mixed Ubuntu/host-driver environment segfaults; older-distribution GPU validation remains open. See the complete [platform boundary](status.md).

Run the native UI scopes with:

```sh
OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=editing xvfb-run -a -s '-screen 0 1280x850x24' python3 tools/ui-smoke.py
OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=persistence xvfb-run -a -s '-screen 0 1280x850x24' python3 tools/ui-smoke.py
```

## Floating transforms and Line/Curve

Original `PasteImageAction` routes the clipboard bitmap through `MoveTool.BeginPaste`. `MoveTool` retains bitmap/document sources and separate base/delta/edit matrices, supports a copy modifier and commits/deselects on both Enter and Escape. The Rust editor now retains a separate source, background and pre-edit document. Eight scale handles, a rotation handle/right drag, numeric center/scale/angle controls, Shift aspect/15-degree rotation constraints, arrow nudges and a copy modifier are available. Finish commits; Cancel or Ctrl+Z restores the pre-edit document. Enter/Escape commit and deselect, following the recovered Move tool. Two portable samplers (nearest/bilinear) and optional linear-light interpolation are available. Off-canvas pixels are retained until acceptance; repeated previews always sample the retained original. Exact interpolation choices, skew/pivot controls, original per-drag history semantics, subpixel geometry/color-profile coverage and reference input certification remain open.

Original `LineCurveShape` begins with four equally spaced points, pixel-center snapping and 0.4999-pixel endpoint extension. `LineCurveUtil` routes Straight through a polyline, Bezier through a cubic, and Spline through `BezierSplineUtil`'s natural cubic system. Its Flat/Rounded/Arrow/Filled Arrow caps, five dash patterns and 5× stroke-width arrow dimensions were recovered, along with the square dash-cap default. Rust implements these geometry/control choices, using tiny-skia rasterization and 0.1-pixel curve flattening. All four points remain editable after drawing; color/style changes preview from the pre-edit document. Shift constrains the initial line to 15-degree increments with the original axis/diagonal length rounding; Alt draws around the initial press. Whole-curve move/rotation handles preserve the four points as one transaction. Finish/Enter accepts, Escape/Cancel restores, and one history transaction captures the accepted curve. Exact Direct2D stroke coverage, endpoint/dash clipping, transform control placement/pivots and original per-gesture history behavior remain uncertified.

Sixty-four original four-knot spline control polygons match the Rust f64 solver within 1e-10, and 256 original constrained endpoints match within 1e-9. Reference JSON parsing preserves binary64 round trips, including half-angle boundaries. CPU checks cover three curve modes, twenty cap/dash combinations, alpha, fractional selection clipping, invalid controls and undo/redo. The native X11 line scope exercises a moved spline knot, centered/constrained drawing, whole-curve move/rotation, finish/cancel and undo/redo. Shapes uses the recovered base, triangle, regular-polygon and star aspect ratios; its bounds retain fractional coordinates and Shift constrains resizing. CPU checks cover all 29 preset constraints in four quadrants and fractional rasterization; the native scope checks centered/constrained rectangles. One explicit GPU test checks integer moves, off-canvas recovery, 90-degree rotation, reflected/enlarged sampling, 188-versus-128 linear/sRGB interpolation, transparent-edge colors, mask-only transforms and reversible paste. The native `transforms` scope checks floating paste move/scale/right-drag rotation, Cancel, Ctrl+Z, acceptance/undo and selection-only motion.

## Background effect previews

Configurable effects and tonal transfers run through a single background worker. It holds at most one running and one pending operation; a new request replaces the pending one. Generations prevent old results and failures from changing a cancelled dialog or a newer request. Preview-off restores the source document, and OK waits for the current result before recording a transaction. Cancel immediately restores editor state. GPU commands already submitted continue until completion; this is result cancellation, not GPU preemption. Resize, transforms, exports, instant adjustments and repeat operations still have synchronous stages. Tiled/resident storage, full progress reporting and device-loss recovery remain open.

Deterministic worker tests use channel barriers to exercise coalescing, in-flight cancellation, stale errors, single delivery, failure recovery and nonblocking shutdown. No timing sleeps are used in these tests.

## Font portability

Text retains system fonts and also loads the already-bundled egui fonts as a fallback. A fontless database uses Ubuntu/Hack for generic families; unavailable family names fall back to the generic sans family. This fixes a `cosmic-text` panic found while running the Windows target under Wine with no usable default font. A deterministic empty-font-database test exercises regular and bold/italic text. Font licenses are included in `font-licenses.txt`. This does not establish original font metrics, missing-script coverage, native IME or platform text-layout parity.

The current Text model matches 512 original navigation/deletion cases. Composition
unit checks cover replacing preedit, committing once, cancellation, surrounding
text deletion, supplementary Unicode characters and the 64 KiB edit limit. Native
X11 scopes check inline text entry, caret navigation, word deletion, move, Escape
acceptance, separate transactions and undo. Rendering and hit testing share the
COSMIC font layout. Text metrics now follow document resolution or Fixed (96 DPI), including shared caret/raster scale and persisted options. Exact DirectWrite shaping and native
IME candidate-window behavior remain separate gates.
