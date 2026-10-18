# Paint.NET 5.1.12 UI parity target

Updated 2026-10-12. Replicating Paint.NET's UI is an explicit requirement of the Rust replacement. Matching the workspace layout alone does not complete it: commands, dialogs, tool options, input, visual states, and persistence must also match the frozen reference.

## Reference evidence

The supplied `paintdotnet.dll` is identified by the SHA-256 in [reference-manifest.json](reference-manifest.json). ILSpy 11.1.0.9782 inspected these original types locally; extracted code and original artwork are not distributed with the Rust application.

| Original type/member | Evidence used |
| --- | --- |
| `Controls.AppWorkspace.ResetFloatingForm` | Tools parks upper left, History upper right, Layers lower right, Colors lower left. |
| `Dialogs.MainForm` | Four distinct floating utility forms; hide rather than destroy them; workspace/document lifecycle. |
| `Controls.PdnToolBar` | Main menus, common actions, tool chooser/configuration, document thumbnail strip, utility buttons; thumbnail drag reordering enabled. |
| `Controls.ToolsControl` | Compact tool button grid; selected state; name/shortcut tooltips. |
| `Dialogs.LayersForm` | Layer list above a bottom toolbar: new, delete, duplicate, merge down, up, down, properties. Layer double-click invokes Properties. |
| `Dialogs.ColorsForm` | Primary/secondary colors, wheel, palette, compact/expanded controls. Reference compact client width is 209 DIPs; expanded design size is 386 × 266. |

The [official main-window guide](https://paint.net/doc/latest/MainWindow.html) and its annotated screenshot establish the overall arrangement. The [toolbar](https://paint.net/doc/latest/Toolbar.html), [tools](https://paint.net/doc/latest/ToolsWindow.html), [history](https://paint.net/doc/latest/HistoryWindow.html), [layers](https://paint.net/doc/latest/LayersWindow.html), and [keyboard guide](https://paint.net/doc/latest/KeyboardMouseCommands.html) supply interaction requirements. These moving pages include 2023-era illustrations; exact 5.1.12 executable observations take priority when they differ.

## Implemented workspace work

- Seven top-level menus in reference order: File, Edit, View, Image, Layers, Adjustments, Effects. Some document/workspace commands remain absent or disabled. All effect entries are enabled.
- Two toolbar rows: common actions/view controls, then tool choice and applicable implemented options. Unavailable actions remain disabled.
- Thumbnail document strip with active highlight, unsaved asterisk, close control, switching, and drag reordering.
- Centered image on a grey editing surface, with existing GPU checkerboard composition, scroll/pan, pixel grid at 200% or greater, pixel rulers, pointer coordinates, dimensions, and status-bar zoom.
- Four separate movable utility windows at the reference corners, close/show buttons, F5–F8 toggles, Tab hide/restore, and Ctrl+Shift+F5–F8 reset. History and Layers are resizable.
- Compact tool grid preserving the reference 19 positions. Nineteen basic tool implementations can be selected, including Magic Wand, lasso, live pixel/selection transforms, Zoom, Gradient, Clone Stamp and Recolor. Text has live Unicode input/font/style/alignment/position editing, and Shapes has all 29 presets with live move/resize handles. Text uses inline canvas editing with caret navigation and IME composition support; exact DirectWrite metrics, native IME and control behavior remain pending. The UI uses 121 distinct hand-crafted vector icons; the SVG atlas comes from the same geometry as the native UI. Exact reference-icon equivalence remains uncertified.
- Layer thumbnails, selection highlight, visibility checkbox, bottom action toolbar, drag ordering, and Ctrl-click move-to-end behavior. Merge Down is enabled. F4/double-click opens a separate Properties modal with name, visibility, blending, opacity, OK, and Cancel. Accepted changes form one history transaction.
- File drag/drop hover artwork and an open/import-as-layers choice. Layer imports currently require matching dimensions and flatten imported documents. Profile conversion and original sizing prompts remain pending.
- Thirteen GPU adjustments and forty effects with preview/reset/cancel/accept controls, plus Ctrl+F effect repeat with up to eight parameters. Center thumbnails support dragging the effect origin; edge choices use named dropdowns and Motion Blur has a Centered checkbox. Gaussian Blur exposes all four quality modes. All 13 adjustments and 40 effects appear in their menus with custom icons; all adjustment and effect entries are enabled.
- Resize and Canvas Size modals, Crop, clockwise/counterclockwise/180° rotation, bitmap clipboard commands and Merge Down are enabled, with the recovered menu shortcuts. Five GPU resize samplers, aspect/percentage controls, nine canvas anchors and five fill choices are available; physical units are implemented; remaining samplers and exact dialog styling remain pending.
- GPU selection outlines follow actual mask edges and holes. Exact marching animation and zoom/DPI thickness remain pending.
- Basic workspace persistence restores floating-window/native-window geometry and selected preferences after a restart. The accessibility bridge is enabled; screen-reader behavior remains uncertified.
- Separate History list with rewind/redo by entry, adjacent-state toggling, and Undo/Redo buttons.
- Separate Colors window with primary/secondary swatches, swap/reset, hue/saturation wheel, compact palette, and expandable RGBA controls. The palette now uses all 96 reference default swatches.

## Completion gates

Tools, History, Layers and Colors can now float as native viewports outside the
main window. View → Floating Tool Windows supports individual/all-window mode
changes; an embedded title's context menu floats it, and each native window has
a dock button. Native controls share the document, GPU and undo history. Modal
dialogs disable native editing controls. F5–F8 also work with a native utility
focused. Closing a utility hides it; it does not close the editor. Floating state,
absolute positions and resizable window sizes persist. Reset shortcuts recover
off-screen windows. Absolute position restoration is unavailable on Wayland,
where positioning belongs to the compositor. Native title chrome, monitor/DPI
transitions, window ownership/focus and inactive opacity still require platform
and reference certification.

The isolated X11 `native-utilities` scope runs with a private KWin window manager
on a 1920×1080 desktop and a 1280×850 main window. Decorations are disabled in
the test manager to keep client-pixel coordinates stable. Native title-strip
dragging delegates pointer capture and movement to the operating system; it
does not reconstruct desktop coordinates from a moving window's local pointer.
The scope drags all four native utility windows beyond the main
window, clicks detached Tools/History controls, checks modal isolation, native
close/reopen and focused shortcuts, restarts with the same positions, redocks
each window and floats History through its title menu. The portable release
passes this scope too. Drag endpoints must match the intended desktop positions
within four pixels. Disabled native utilities request focus for the active modal
and forward keyboard input to it if the window manager rejects that request.
This is not a multi-monitor/DPI or native Mac gesture
certification.

| Gate | Still required |
| --- | --- |
| Visual identity | Matched 5.1.12 screenshots at fixed DPI, theme, window size and state. Certify spacing, fonts, icon silhouettes/colors, borders, focus/hover/disabled states and shadows. Current egui window chrome remains visibly different. |
| Complete command surface | Every menu item, submenu, enable rule, context menu, accelerator, tooltip, settings/help entry, and corresponding action. Disabled placeholders do not count as implementations. |
| Tools and options | All 19 tools and their complete controls, ranges, defaults, shortcut cycling, live edits and Finish behavior. |
| Dialogs | Reference layout, validation, defaults, preview/rollback and keyboard behavior for new/open/save/configuration, resize/canvas size, properties, adjustments, all effects, settings, print and acquisition. Properties currently commits on OK; original live preview behavior is not certified. |
| Workspace behavior | Snap/docking, inactive-window opacity and passthrough, complete position/size persistence, theme settings, full image-list overflow behavior, per-document viewport state, monitor/DPI changes, and off-screen recovery. |
| Colors | Original palettes and file controls, HSV/numeric/hex workflows, color-management interactions, full compact/expanded behavior and exact wheel mapping. |
| Accessibility and input | Focus order, keyboard-only operation, screen readers, text input/IME, tablet/pressure, mouse capture, temporary pan and platform-specific shortcuts. |
| Native validation | Linux X11/Wayland, Windows/D3D12 and macOS/Metal; fixed reference comparisons, multiple DPI scales, narrow/wide layouts and multiple monitors. |

## Verification

On 2026-10-11 the native Linux exercise passed on the RTX 5080 Vulkan adapter. Formatting, strict Clippy, document/file/selection tests, bounded PDN/NRBF tests, and original file interoperability checks also passed. The standard run leaves all twelve hardware tests explicitly ignored. Separate explicit runs passed on the RTX 5080, including original blend, posterize and gradient fixtures. Recovered-equation checks for the other adjustments and Vignette/Relief have narrower scope than original GPU/profile parity; see [processing evidence](processing-parity.md).

`tools/ui-smoke.py` exercises the native Linux window under isolated Xvfb. It compares canvas pixels for brush/undo/redo, layer creation/drawing/visibility, Magic Wand deletion, lasso deletion and pixel translation with undo. It verifies window toggles/reset, floating-window hit isolation and modal hit isolation. Sepia checks cover live previews, cancel rollback, blocked canvas input during a modal, acceptance as one history transaction, undo and redo. An additional scope checks Gradient drag/undo/Escape, Clone Stamp source/undo, Recolor/undo, new adjustment/effect dialogs, Vignette rollback/commit/undo/redo and Ctrl+F repeat. The content scope checks actual text letters, multiline editing, finish/cancel/undo/redo and Shapes drag/move/resize. The tonal scope checks Levels/Curves preview transactions, Auto-Level, Emboss repeat and Morphology. A native X11 Xdnd transfer checks hover feedback, the open/import choice and opening an original `.pdn` file. Captures are in ignored `artifacts/ui-smoke/`. Rust tests also verify history navigation across pixel, layer, selection, and dimension edits and a replacement redo branch.

These checks verify the exercised Rust interactions. They do not establish screenshot parity with a live 5.1.12 Windows session or native Windows/macOS behavior. The full UI and application goals remain incomplete.

The filter scope additionally checks both new adjustment edit/accept/undo/cancel
flows, and preview/cancel/modal isolation/accept/undo/repeat for all twelve new
effects. Zoom Blur exercises its fifth parameter and draggable center. Menu
entries have full-row hit regions, including the handmade icons and the space
after short labels.

The additional composite scope exercises Ink Sketch, Surface Blur, Soften
Portrait and Drop Shadow, including preview, modal canvas isolation, cancellation,
acceptance, undo and repeat. The shadow color picker and Shadow Only checkbox
are tested with nondefault color and six-parameter repeat storage. Exact original
color-wheel/angle-chooser styling remains pending.

The neighborhood scope checks Oil Painting, Median, Outline, Reduce Noise, Add
Noise and Frosted Glass. Oil Painting has circle/square kernel choices. Random
effects have Randomize buttons with stable preview/repeat seeds; Reset retains
the instance seed. Frosted Glass links minimum/maximum scatter radii. These
remain egui dialogs; original dialog layout and widget styling are uncertified.

The final fourteen effect dialogs passed the isolated remaining scope through preview, cancel, blocked canvas input, acceptance, undo and repeat. Their handcrafted menu icons are enabled. Quantize exposes palette algorithm, color count, dithering and alpha threshold; Pixelate exposes six downsampling and three upsampling choices; the render effects expose 46 named mix modes, and Clouds exposes two RGBA colors. Exact original dialog layout, tabs and angle/color widgets remain open. This is interaction validation, not screenshot certification.

The isolated editing scope passes resize/canvas accept/cancel/undo, crop, Cut/Copy/Paste/new-layer/new-image and Merge Down. The persistence scope confirms the chosen tool, grid, utility visibility and a moved History window after a clean restart. See [editing evidence and remaining boundaries](editing-reference.md).

Floating paste and Move tools now expose eight scaling handles, a rotation handle/right drag, numeric transform values, two sampling choices, gamma, Finish and Cancel. Enter/Escape finish and deselect; Ctrl+Z cancels a live transform. Line/Curve now exposes four editable points, Spline/Bezier/Straight, four endpoint caps, five dash styles, whole-curve move/rotation and finish/cancel. Line/Curve and Shapes support centered and constrained drawing; Shapes preserves fractional bounds. Native interaction scopes cover both additions. These controls and independent rasterization still need original screenshot/input certification.

Configurable effect previews run in the background, coalesce queued edits and discard cancelled/stale results. The Preview control displays rendering state; acceptance waits for the latest result and remains one transaction. Complete original progress/cancellation presentation and all other asynchronous editor operations remain pending.

Inline Text now accepts keyboard/IME events on the canvas, with native input focus,
a shaped caret, per-line hit testing, word/line navigation, right-drag movement and
Escape acceptance. Native X11 tests cover independent text edits, caret movement,
word deletion, acceptance and undo. Accessibility metadata identifies the text
widget; screen-reader and actual native IME workflows remain uncertified.

All four Gaussian Blur qualities pass the dedicated native interaction scope.

Brush, Eraser, Clone Stamp and Recolor expose size, circle/square, hardness, spacing, antialiasing,
smoothing and pressure with seven hand-drawn option icons. The brush-options
scope confirms functional controls, feathered/aliased output, gaps at high
spacing, option isolation one-step undo and settings restoration after a clean restart. Original toolbar/DPI screenshots,
screen-space brush previews and physical pen input remain uncertified.
Shared drawing blend and hatch dropdowns use handmade icons and pattern previews;
all 15 blend choices and 53 hatch choices are reachable. A native scope checks
patterned versus solid drawing and undo. Toolbar scrolling keeps options reachable
in narrow windows; exact Paint.NET overflow, control placement and screenshot
parity remain open.

Color Picker exposes all six original sample sizes, active/merged sampling and all
three release behaviors. The native interaction scope checks continuous sampling,
painting with the chosen color, unchanged document pixels, sample-size selection
and release-only return to the previous tool/Pencil. Color averaging matches 256
original managed cases. Exact cursor and sampling-preview styling remains open.


Selection rectangle sizing and Colors numeric/palette controls now have
reference-derived behavior and native interaction checks; see
[selection and color evidence](selection-color-reference.md). More/Less expands
Colors horizontally while keeping the palette beneath the wheel. Exact native
control dimensions, color-managed wheel presentation, palette-library collation, utility
window opacity/passthrough and full DPI/platform certification remain open.

The 2026-10-17 Linux continuation reverified native window movement on the
RTX 5080/Vulkan adapter under isolated X11/KWin. All four windows move completely
beyond the 1280×850 editor, retain positions after restart, close/reopen, accept
focused shortcuts and redock. The same scope now loads original Paint.NET PDN3
and legacy fixtures, edits their layers through detached Layers, undoes and saves
copies, and checks recovery after truncated/unsupported file errors. The original
5.1.12 reader verifies the saved copies against its fixture expectations. See
[PDN evidence and limits](pdn-compatibility.md). Native Wayland window placement,
multiple monitors, exact chrome and cross-platform gestures remain unverified.

The 2026-10-18 clipboard continuation makes every New entry point initialize
pixel dimensions from the current image clipboard, with the existing 800×600
fallback when no supported image is readable. Oversized Paste and the separate
Paste into New Layer command offer Expand canvas, Keep canvas size and Cancel.
Keep retains off-canvas source pixels for movement; cancellation before paste
changes no history. Successful layer/canvas preparations have the original
separate undo steps before pasted pixels. The native X11 `clipboard` scope uses
an external PNG owner on its private display and checks the fields, choices,
undo/redo and error recovery. See [clipboard behavior and limits](editing-reference.md).


The 2026-10-18 input continuation ports timed/Shift-reversed tool cycling,
held-stroke key behavior, C/X colors, shared bracket-adjusted widths, original
line-cap/dash cycling, live A/Shift+A shape changes, Enter deselection and
Space/middle-button temporary pan. Native X11 checks cover controls, previews,
text isolation and undo. See [reference evidence and limits](input-quirks.md).
