# Implementation status

Updated 2026-10-18. Goal: a feature-complete, hardware-accelerated Rust replacement for the supplied Paint.NET 5.1.12 distribution, native on Linux, Windows and macOS. **The replacement remains incomplete.** UI replication is an explicit completion requirement; see [UI parity](ui-parity.md).

## Present implementation

| Area | Present behavior | Parity boundary |
| --- | --- | --- |
| UI | Paint.NET workspace arrangement: seven menus, two toolbar rows, thumbnail document strip, four movable utility windows, window shortcuts, history navigation and Layer Properties modal | Partial. Exact styling, all dialogs/commands/options, snapping, complete workspace persistence and native platform parity remain open. See [UI parity](ui-parity.md). |
| Runtime | Native Rust application, egui/winit UI, wgpu compute and presentation on one device | No .NET runtime in the application. The optional reference oracle is a separate developer tool. |
| Layer composition | All 14 reference layer blend modes, 0–255 opacity, visibility and ordering | Integer CPU and GPU arithmetic matches the original managed CompositionOps fixtures. This does not establish color-managed canvas or HDR parity. |
| Editing | Basic brush, pencil, eraser, bucket, picker, rectangle/ellipse/lasso selection, Magic Wand, selection algebra, live pixel/selection translation, scaling and rotation, seven gradient geometries/three repeats, Clone Stamp, Recolor, live Text, all 29 Shapes presets, live four-point Line/Curve, pan/zoom | The flood color-distance formula matches 4,096 original pairs in straight/premultiplied modes. Other tool options, stroke coverage, selection rasterization and interaction are not yet reference certified. The GPU selection overlay follows mask edges and holes; exact animation and screen-space thickness remain pending. |
| Documents | Independent document tabs/history, RGBA layers, names/visibility/opacity/blend/order, add/duplicate/delete/merge-down, flatten, flips, 90°/180° rotations, canvas sizing, crop, GPU resizing and bitmap clipboard | Five resize samplers and native bitmap clipboard are implemented; remaining reference samplers and advanced workspace behavior remain pending. See [editing evidence](editing-reference.md). |
| History | Copy-on-write snapshots, transaction-per-stroke, undo/redo, branching, saved-state tracking | Disk-backed spill and recovery remain pending. History entry jumps and a single transaction for accepted layer properties are implemented. |
| Adjustments | GPU Invert Colors/Alpha, Black and White, Sepia, RGBA Posterize, Brightness/Contrast, Exposure, Hue/Saturation, Auto-Level, Levels, Curves, Highlights/Shadows and Temperature/Tint; configurable background previews/cancel/undo | Posterize matches 16,128 original lookup entries. Black and White/Sepia follow recovered shader equations; Temperature/Tint matches 621 native CPU gain pairs. Original GPU/color-profile equivalence remains pending. |
| Effects | All forty effect entries with GPU pixel processing, including the final fourteen, with controls, background previews, cancellation, selection clipping, undo and Ctrl+F repeat | Recovered equations, original kernels and focused native GDI+ checks; complete original Direct2D/profile certification remains pending. All four Gaussian Blur qualities execute GPU paths; native optimization/kernel fixtures pass. See [processing evidence](processing-parity.md). |
| Files | Real PDN3 read/write, legacy gzip/inline reads, metadata/header preservation, atomic saves; development `.opdn` v2/v1; selected raster codecs | Original-reader round trips pass for the supplied corpus. Broader historical PDNs, profile interpretation, save settings, codecs and multi-page policy remain pending. See [PDN compatibility](pdn-compatibility.md). |
| Platform | Linux development/release builds; Windows GNU cross-build with Wine checks; native Apple M2 Pro/Metal builds and GPU checks; native-build CI configuration for all three OS families | Windows/macOS CI has not run. Native Apple M2 Pro/Metal checks pass; native Windows, tablet, scanner, printer and display-profile validation remain open. |

## Validation obtained

Brush, Eraser, Clone Stamp and Recolor now expose circle/square tips, the original 1–2,000-pixel size
range, hardness, spacing, antialiasing, smoothing and pressure settings. Recovered
math matches 256 original brush plans, 256 aliased stamp plans and 128 smoothing
cases. The isolated native UI checks the functional controls and undo. Exact
Direct2D brush pixels and physical tablet behavior remain uncertified. See
[brush evidence and limits](brush-reference.md).

- `cargo fmt --all --check`, strict Clippy, standard Rust tests, and release compilation passed on Linux with Rust 1.96.0.
- Fifty-one standard Rust tests passed, including PDN bidirectional/legacy/malformed cases, flood distances, selection algebra and translation, history navigation across mixed edits and branch replacement: original composition fixtures, exhaustive channel tables, history/branch/save behavior, selection/fill/erase/rotation, project round trips and atomic overwrite, invalid inputs, single-stamp/full-erase behavior, and lossless raster round trips/JPEG transparency flattening. New checks cover original line constraints and centered, aspect-constrained, fractional shape bounds.
- An explicit native GPU test passed on **NVIDIA GeForce RTX 5080 / Vulkan / DiscreteGpu**. It rejects software adapters. GPU composition matched original reference outputs on 917,504 exhaustive opaque channel cases and 2,688 mixed-alpha/opacity cases; hidden-layer and masked adjustment checks also passed. A second explicit hardware test matched all 16,128 original Posterize table entries; Black and White/Sepia readbacks matched recovered scalar equations within one byte and preserved alpha/selection boundaries. These equation checks do not certify original Direct2D/profile output.
- A third hardware test passed 3,927 original gradient geometry samples (within one quantized weight), all 4,096 original gradient color blends exactly, new adjustment equations/neutral settings, Vignette selection bounds, directional Relief kernels, transparency/reversal and invalid parameters. Clone/recolor tests cover source snapshots, opacity, selection, overlapping stroke coverage and undo. See [processing evidence](processing-parity.md) for the remaining reference boundaries.
- A fourth explicit hardware test passed identity and configured RGB/luminosity transfers, selection/alpha preservation, Emboss flat-field output and separable Morphology dilation/erosion including maximum kernel sizes. CPU tests compare 4,608 original RGB Levels samples and 4,096 original spline samples; all 29 shape presets render distinct selected outputs, and Unicode text tests cover alpha, styles, alignment, selection and undo.
- A fifth explicit hardware test passed the sixteen new effects and two adjustments on RTX 5080/Vulkan: independent direct 2D Square Blur, 621 native temperature gain pairs, selection boundaries, source sampling, neutral byte preservation, single-pixel borders and extreme/invalid parameters. Seven Gaussian kernels match original managed IL. Native Direct2D pixels and profiles remain uncertified.
- A sixth explicit hardware test covers the six neighborhood/random effects, independent sorted percentile and linear histogram oracles, mirrored reduction/scatter, alpha and selection, invalid/extreme controls, and stable random seeds. The original Add Noise table matches all 16,384 entries and PCG matches 4,096 original cases. The earlier batch passed thirty-one standard tests and six hardware tests. The completed effect roster now passes thirty-five standard tests and nine explicit hardware tests on this host. Native Direct2D output remains uncertified.
- The reference outputs were generated by executing the original 5.1.12 managed IL with .NET's Windows ReadyToRun code disabled. See `reference-manifest.json` for source hashes, oracle identity and fixture hashes. CPU routine parity does not prove original UI/display/effect parity.
- The Linux UI launched under isolated Xvfb and used the RTX 5080 adapter. Captured canvas pixels checked drawing, undo/redo, layers/visibility, Magic Wand and lasso deletion, and pixel translation with undo. The workspace passed F5–F8 and Tab hide/restore, moving/resetting History, floating-panel hit isolation, and Layer Properties modal hit isolation. Sepia passed repeated live previews, cancel rollback, modal canvas isolation, one accepted history transaction and undo/redo. A native X11 Xdnd transfer opened an original `.pdn` through the drop-choice dialog. The additional tools exercise passed gradient drag/undo/Escape, clone source capture/undo, recolor/undo, new adjustment/effect dialogs, Vignette cancel/commit/undo/redo and Ctrl+F repeat. The content scope checks actual text glyphs, styling and positioning, Shapes dragging/moving/resizing, and finish/cancel/undo/redo. The tonal scope checks Levels/Curves preview transactions, Auto-Level, Emboss repeat and Morphology. These are interaction checks, not reference screenshot certification. Xvfb presentation is not a Wayland, multi-monitor, HDR or latency validation.

Local logs and screenshots are in ignored `reference/` and `artifacts/ui-smoke/`. GPU tests are ignored by default in CI because ordinary runners cannot prove hardware acceleration. The separate explicit GPU run above supplied that evidence on this host.

The new isolated filter UI scope passed both adjustment edit/accept/undo/cancel
flows and twelve effect preview/cancel/modal-isolation/accept/undo/repeat flows.
It also checks a nondefault Zoom Blur fifth parameter and draggable center.
A second scope checks Ink Sketch, Surface Blur, Soften Portrait and Drop Shadow;
Drop Shadow uses a nondefault color and Shadow Only, including repeat storage.
The checks caught and fixed text-only menu hit areas; effect/adjustment menu
rows now accept clicks across their icons, labels and remaining row space.

The additional neighborhood UI scope checks six dialogs through preview/cancel/modal isolation/accept/undo/repeat, plus circle/square choice, randomization, reset seed retention and linked scatter controls.

The new filter checks additionally compare direct 2D Square Blur, seven original Gaussian kernels and 621 native temperature gain pairs, plus independent Surface Blur neighborhoods and translated Drop Shadow integration. Hardware tests cover the sixteen new effects, selection and alpha boundaries, neutral cases, extreme controls and invalid inputs. See [native recovery and filter limits](filter-reference.md).

The final fourteen effect entries passed native Linux/X11 preview/cancel/modal isolation/accept/undo/repeat tests. Quantize was exercised with a reduced palette and Straighten with a nonzero angle; an unchanged Red Eye operation correctly adds no history entry. The four composite dialogs also passed again after shared dialog/pipeline changes.

The final effect hardware checks cover all fourteen operations, every Pixelate sampling combination, all 46 render mix choices, both palette algorithms and dither limits, Bokeh against an independent direct 2D complex convolution, neutral/single-pixel behavior, stable seeds and selection boundaries. Red Eye additionally matches 50 native GDI+ output cases within one byte and 50 original analysis tables within 0.0002. Original Bokeh/Sketch kernels and native Turbulence tables are independently checked. This is Linux/Vulkan evidence; it does not certify full original renderer/UI or native Windows/macOS parity.

## Reverse engineering evidence

The original installer contains 536 embedded files; the full payload is now extracted locally, including PaintDotNet.Core, Data, Fundamentals, Effects.Gpu and Resources. ILSpy 11.1.0.9782 inspected the exact supplied assemblies. The original payload and local decompilation stay in ignored `reference/`; they are not part of a replacement binary or development package.

Original routines inspected include `UserBlendOps`, `CompositionOps`, `ByteUtil.FastScale`, `ColorBgra.GetIntensityByte`, unary inversion/desaturation, `FloodFillAlgorithm` color distance, `Document.FromStreamImpl`/`SaveToStream`, GPU invert setup, `GrayscaleEffect2`, `SepiaEffect2`, `PosterizeEffect2.GenerateLevels`, gradient shapes/repeaters/color blending, clone/recolor renderers, brightness/contrast/HSL shaders, Vignette/Relief/Emboss kernels, TextTool, all 29 shape definitions, LevelsParameters, HistogramData, FunctionInterpolator and luminosity-transfer shaders. The binary shows PDN3 magic, a 24-bit little-endian XML header length, a `00 01` marker, .NET BinaryFormatter serialization, and deferred compressed memory blocks. The Rust codec now interprets the graph and deferred pixels, with original-reader acceptance and pixel/layer/resolution comparison. See [PDN compatibility](pdn-compatibility.md) for remaining corpus limits.

The [official feature inventory](reference-features.md) has acceptance gates for 19 tools, 29 shapes, 14 layer blends, 13 adjustments, 40 effects, the documented file families, color profiles/palettes, history, native integrations and extensions. Documentation can drift; executable-derived controls and fixtures must settle exact 5.1.12 behavior.

## Remaining goal work

1. Broaden PDN compatibility fixtures to historical versions, parent surfaces, padded strides, metadata/profiles and malformed/fuzzed files; validate native Windows/macOS interoperability. Current original 5.1.12 bidirectional round trips are implemented.
2. Complete Text/Shapes and Gradient/Clone Stamp/Recolor options, plus remaining selection rendering/geometry options, reference clipboard formats, complete transform controls, remaining samplers and tablet input. Certify existing tool behavior using reference-authored input sequences.
3. Certify Gaussian Blur native pixels/profile behavior and all thirteen implemented adjustments and forty effect entries, with original parameter ranges/defaults and working-space behavior. Use hardware compute for image-processing paths, including previews and selection clipping.
4. Replace whole-document GPU uploads with tiled resident storage and dirty regions; extend asynchronous effect previews to all long-running operations, add bounded disk-backed history, large-document handling and device-loss recovery. Current limits are 16,777,216 pixels, 256 layers, 512 MiB project data, and the selected GPU's storage/dispatch limits.
5. Deliver original-format encoders/configuration, full settings, certified accessibility and clipboard/file/print/scan integrations and an explicit extension compatibility strategy. Named palettes now support discovery, saving, replacement and folder access. Existing .NET CPU plugins and Direct2D GPU plugins cannot be claimed compatible with the present native editor.
6. Replicate the full Paint.NET UI and interaction model using the frozen executable: complete menus, tool options, dialogs, colors, complete workspace persistence/docking, shortcuts, accessibility and theme/DPI behavior. Validate matching screenshots and input sequences; see [UI parity](ui-parity.md).
7. Run native Windows/D3D12, macOS/Metal and Linux/Vulkan builds and UI tests. Validate discrete/integrated devices, software fallback, input devices, DPI, profiles, packaging and recovery. Produce reproducible packages, notices and checksums for all three OS families.

Completion requires every applicable feature gate to be implemented and demonstrated; basic tooling, matching blend arithmetic, configured CI and a Linux launch do not satisfy the whole goal.

The current UI includes 121 hand-crafted vector icons and the full 13-adjustment/40-effect menu roster. All thirteen adjustments and forty effect entries are enabled; other disabled commands remain unfinished features. All nineteen tools have basic implementations, including live Text and Shapes. Drag/drop offers open or matching-size layer import.

The latest source passes seventy-five standard tests and fourteen explicit RTX 5080/Vulkan tests, including GPU resizing and display-only selection outlines. Native X11 resize/canvas/crop/cut/copy/paste/merge flows and clean workspace restart persistence pass. Original Octree reduction matches 48 unique-population palettes; original proximity ordering matches 24,576 queries, including ties. Windows-target CPU and GPU checks also run under Wine; the precise counts are in [editing and platform evidence](editing-reference.md). These Linux/Wine checks do not certify native Windows. Separate native Mac evidence is in [native validation](native-validation.md).

Live floating paste and selection transforms now retain source pixels through off-canvas motion, scaling, reflection and rotation. Nearest and bilinear sampling with optional linear light run on the GPU. Line/Curve has four handles, Spline/Bezier/Straight modes, four caps and five dash styles; its natural spline geometry matches 64 original control polygons. Text and Shapes now honor fractional selection alpha. Effect previews use one background worker with one replaceable pending request; stale/cancelled results are discarded and acceptance waits for the latest result. GPU work already submitted is not preempted. See [editing evidence](editing-reference.md) for exact behavior and remaining limits.

The final source also includes a bundled font fallback for minimal installations. An empty-font-database test covers regular and bold/italic text; system fonts remain available. The original-reader PDN checks pass again after these changes, including exact pixels, all layer properties and resolution for all three Rust-written documents.

Final Linux and Windows GNU development archives contain the tested binaries, documentation, handcrafted SVG atlas, lockfile and font notices. Stable archive timestamps and SHA-256 sidecars make repackaging identical inputs deterministic. The packager checks ELF/PE/Mach-O architecture before creating an archive. Native CI now packages its own platform's binary and notices; those remote jobs have not run. A native Apple Silicon app/archive is now built and tested on the user's Mac; see [native validation](native-validation.md).

The native host-linked Linux binary initially imported glibc 2.44 math symbols. The distributed binary is instead linked with Zig 0.14.1 against the glibc 2.35 ABI; `objdump` confirms no newer imported glibc versions. The package records that symbol floor. The pinned compiler archive SHA-256 is `24aeeec8af16c381934a6cd7d95c807a8cb2cf7df9fa40d359aa884195c4716c`, from the [official download index](https://ziglang.org/download/index.json). Native drivers/libraries still require distribution testing; the lower ABI floor is not an older-distribution certification.

The packaged Linux binary passes the full workspace interaction scope, ten-frame startup, hardware adapter detection and original-PDN rendering on this host. The Linux and Wine package renderings match pixel for pixel. A supplemental probe with privately extracted Ubuntu 2.35-0ubuntu3.15 libc starts the CLI successfully. Its GPU probe initially rejected the newer host libraries' `GLIBC_ABI_DT_RELR` requirements; with matching older Vulkan/X libraries added, dynamic loading segfaults. That mixed-library experiment is not a native Ubuntu installation, and its failure remains unresolved. No older-distribution GPU compatibility is claimed. The ordinary host and Wine GPU checks pass.

Native UI reruns exercise all forty effect entries across the filter, composite, neighborhood, final-effect, tonal and additional-tools scopes, plus Levels/Curves, Text/Shapes, Line/Curve, floating transforms, editing commands and workspace persistence. Relief is checked through preview/cancel; the other effect checks include acceptance and undo. Tests wait for background acceptance before comparing committed pixels. These are interaction and transaction checks, not original screenshot certification.

Line/Curve now supports Shift 15-degree constraints and Alt centered drawing, plus whole-curve move/rotation handles. Shapes supports Alt centered drawing, Shift aspect constraints using recovered preset ratios, and fractional bounds through live edits. The line constraint routine matches 256 original cases, including angle boundaries; reference JSON uses round-trip float parsing. Native X11 interaction checks cover the new controls and single-transaction undo. Exact original control placement, transform pivots, stroke coverage and per-gesture history remain uncertified.

The current continuation adds all four Gaussian Blur quality modes, inline Text
editing, 15 shared drawing blends and 53 hatch fills. Text navigation/deletion
matches 512 original cases; Overwrite matches 4,096 original pixel cases. Native
Gaussian optimization/kernel recovery covers 45 cases. Inline Text's UTF-8 cursor
preserves supplementary characters; this is safer than reproducing UTF-16 splits,
but those edge cases are not certified identical. Font metrics now follow document resolution or the Fixed (96 DPI) option. Native X11 checks pass Text navigation/IME model integration and all four
Gaussian qualities through cancellation, acceptance, undo and repeat. Real IME
candidate windows still need native platform validation. There are 121 handmade
icons. Hatch monochrome coverage is exact; partially transparent hatch color
interpolation, exact native brush coverage, complete effect sampling/profile behavior,
exact original dialogs and platform certification remain open.

Color Picker now implements all six original sampling sizes and all three release
behaviors, active/merged sampling and Ctrl override. Its average matches 256
original managed cases. Native interaction checks cover continuous sampling,
chosen-color painting, unchanged image pixels and switching only on release.

The refreshed Linux and Windows GNU packages include these changes and 121 handmade
icons. Both packaged binaries identify the RTX 5080 and render the original PDN
fixture identically. The portable Linux package also passes the inline Text,
Color Picker and all-four-Gaussian-quality native interaction scopes. The
refreshed package also passes brush controls, undo and settings restart. All 75
standard and 14 explicit hardware tests pass on Linux and for the Windows target
under Wine. Native Apple M2 Pro/Metal tests and a macOS app/archive now pass. Native Windows
validation remains open. Repackaging identical inputs is byte-for-byte
deterministic, and no reference DLL or .NET runtime is bundled.


The current selection/color continuation adds Normal, Fixed Ratio and Fixed Size
rectangle modes, width/height swapping and pixel/inch/centimeter units. Rectangle
bounds match 768 original results; 117 original measurement conversions pass.
Fixed sizes follow the cursor and stay inside the image, Shift rectangles remain
square, and Shift ellipses use the drag as their diameter. Ctrl/Alt left/right
selection combinations and origin movement are implemented. Partial masks now
invert by coverage; Delete uses Overwrite coverage and deselects in one history
entry. Analytic ellipse masks and scanline lasso masks still lack the original
four-times supersampling and retained Boolean geometry.

Colors now uses all 96 reference default swatches, checkerboard alpha previews,
swatch replacement, opposite-color right clicks and UTF-8 palette file load/save.
The original parser matches 25 cases and integer RGB/HSV controls match 2,048
cases. RGB hex edits preserve alpha. The expanded numeric panel sits beside the
wheel and palette; native X11 checks cover its hit regions and restart persistence.
Named palette-library management now supports saving, replacement, default reset and external refresh. Text now uses document DPI,
Points/Fixed (96 DPI) metrics and the native 12-point default and 1–2,000 size
range. Layout and raster scale agree; portable glyph rasterization currently
rejects resolved sizes above 4,096 pixels. Native DirectWrite/profile/IME parity
remains open. See [selection and color evidence](selection-color-reference.md).

The color wheel now follows 512 original gesture and 512 original pixel results,
including the nonlinear saturation radius, preserved brightness, 15-degree Shift
snapping, Alt hue lock, Ctrl saturation lock and opposite-color right gestures.
Its pixels are generated procedurally and displayed with GPU texture rendering.
Color-managed presentation and exact DPI/animation behavior remain uncertified.

New, Resize and Canvas Size now expose physical dimensions and resolution. The
shared model matches 128 original resize traces and eleven native metadata writes;
DPI-only edits retain pixels and participate in undo. Native Linux interaction
checks cover acceptance and physical resizing. The Windows D3D12 backend now
passes all fourteen explicit GPU tests under Wine on RTX 5080 through VKD3D-Proton.
Native Mac validation passes on the user's Apple M2 Pro; platform defects fixed
and exact evidence boundaries are in [native validation](native-validation.md).

Tools, History, Layers and Colors now float as native windows beyond the editor.
Individual/all-window controls, title-menu floating, redocking, native close/reopen,
focused shortcuts and geometry persistence are implemented. Native X11 checks
pass on debug and portable release builds, including dragging all four windows
outside the 1280×850 main window on a 1920×1080 desktop. Detached editing controls
remain disabled beneath modal dialogs. Exact chrome, ownership/focus, opacity,
snapping, monitor/DPI transitions and native platform gestures remain uncertified.

## Linux continuation, 2026-10-17

The explicit continuation priorities were native tool windows outside the main
frame and genuine Paint.NET file loading. Existing native viewports passed the
isolated X11/KWin drag, focus, modal isolation, restart, close/reopen and redocking
checks on RTX 5080/Vulkan. The scope now additionally loads the three authentic
5.1.12 serializer fixtures while all four utilities are outside the editor,
edits layer visibility, undoes and saves copies. Truncated and unsupported-version
loads leave the current document intact. The original reader verifies every
pixel, layer attribute and resolution of the UI-saved copies. The fixture hashes
remain unchanged.

The loader now rejects unknown effective numeric blend modes instead of silently
using Normal. Seven PDN integration tests cover original layered/legacy input,
application file dispatch, resolution and metadata round trips, corruption and
unsupported versions/types/modes. The full standard suite passes 78 tests, with
14 hardware tests left opt-in; formatting, strict Clippy and a debug build pass.
See [PDN coverage and remaining limits](pdn-compatibility.md) and local evidence
in `artifacts/resume-20261017/`. Native Wayland gestures, historical PDN corpus
expansion and profile-aware rendering remain open.

## Clipboard continuation, 2026-10-18

New Image now reads clipboard pixel dimensions on every Ctrl+N, File → New and
toolbar invocation; text-only/empty/unreadable image clipboards use 800×600.
Oversized Paste offers Expand canvas, Keep canvas size and Cancel, including for
the separate Paste into New Layer command. The latter preserves the original
layer/canvas/paste undo order. Preparation is validated and rendered before
history changes, and the size prompt holds one bitmap without per-frame copies.
Source pixels outside an unchanged canvas remain available for movement.

The isolated X11 clipboard scope checks all New entry points, fallback, button
and keyboard choices, prompt cancellation, a changed clipboard during the prompt,
layer/canvas/pixel undo and redo, movement of clipped pixels, active-layer paste
and missing-image recovery. The existing transform interaction scope also passes.
The frozen 5.1.12 actions and resource strings establish the behavior; exact
dialog artwork, portrait/DPI-scaled fallback sizes and vector/profile clipboard
formats remain open. See [clipboard evidence](editing-reference.md).


## Input quirks continuation, 2026-10-18

The frozen Tool, ToolConfigStrip, ColorsForm, ShapesToolBase and built-in tool/
shape catalogs establish the new keyboard behaviors: two-second cycling with
Shift reversal, held-stroke suppression with retained timer updates, C/X colors,
fraction-preserving bracket steps and shared width, native line-cap/dash order,
live shape cycling and ordinary Enter deselection. Temporary Space/left-drag and
middle-button panning retain the tool and affect neither pixels nor history.
Native press/move/release events preserve pan endpoints even within one frame.
See [input evidence and remaining quirks](input-quirks.md).

The isolated X11 quirks, clipboard, transform, line and external-window scopes
pass, including authentic PDN loading/saving with detached tool windows. Rust
formatting, strict Clippy, 85 standard tests and an Apple Silicon compilation
check pass. Native Mac validation and installation of this continuation are
unavailable because the previously authorized Mac SSH connection times out;
the earlier Metal archive does not validate these new changes. Full input/UI
and platform parity remain open.

## Magic Wand review, 2026-10-18

Review of PhotoCraft's pinned selection implementation confirmed a useful
scanline traversal, while its different tolerance metric and neighborhood edge
smoothing do not establish greater accuracy against the Paint.NET reference.
The Rust flood now fills spans without a separate visited image, and active-layer
sampling shares pixels instead of cloning them. Both Wand and Bucket retain the
original color/alpha rules. New exhaustive connectivity and mixed-alpha tests
pass alongside all 4,096 original paired color-distance cases. See the
[comparison, implementation and remaining selection gates](magic-wand-reference.md).

The updated source passes 88 standard Rust tests (14 hardware tests remain
opt-in), strict all-targets Clippy, formatting and the Apple Silicon compilation
check. All six selection tests also pass for the Windows target under Wine.
The release Linux binary retains a glibc 2.35 symbol floor. These compilation
and Wine checks do not certify native macOS or native Windows behavior.

The portable Linux release passes the native X11 editing scope, including
Magic Wand deletion/undo, lasso, layer visibility, History navigation, floating
panel and modal hit isolation, pixel movement, GPU preview/cancel/commit/undo
and an actual Xdnd drop of the original PDN fixture. The renderer identifies
NVIDIA GeForce RTX 5080 / Vulkan / DiscreteGpu. This is interaction evidence,
not original screenshot or native Wayland/platform certification.

The final portable release also passes the timed/reverse shortcut, shared-width,
live line/shape, Space/middle-pan, text-isolation and undo interaction scope.
The Windows release completes a 20-frame Wine/D3D12 startup on the RTX 5080
with all four utility windows created as native Immediate viewports. It uses
the documented private-prefix `WGPU_DEBUG=1` DXC workaround; this is not native
Windows driver certification. Fresh Linux and Windows development packages and
the native-Mac validation source bundle include this continuation and notices.
No new Mac binary was built or installed while its SSH connection was unavailable.

## Build bundles, 2026-10-18

The Mac connection became available during packaging, superseding the earlier
unavailable-platform note above. The current Rust source now passes native
Apple M2 Pro/Metal validation: 88 standard tests, 14 debug hardware tests, 14
optimized hardware tests, release startup, original-PDN rendering, creation of
all four native floating utilities and LaunchServices app-bundle startup. All
20 validation checks pass. The new macOS ARM64 app archive replaces the older
development bundle; this packaging run does not replace the user's installed app.

Linux x86_64, Windows x86_64 and macOS ARM64 development bundles include the
updated README explaining the Paint.NET reverse-engineering work. The README
no longer refers to the development reference installer. All bundles preserve
their incomplete-feature status, platform evidence, notices and SHA-256 sidecars.
The native Mac evidence is `metal-validation-20261018T165203Z.tar.gz`; see
[native validation](native-validation.md) for its exact binary identity.

## OpenPDN branding and storage cleanup, 2026-10-18

The Cargo package, Rust crate, executable, application/window titles, native
utilities and distribution bundles now use OpenPDN/openpdn. The macOS app is
OpenPDN.app with bundle identifier dev.openpdn.editor. Existing .opdn files keep
their original format identifier for compatibility; legacy environment overrides
and existing preference/palette locations remain readable. Updated tooling and
CI use the new names. Superseded build outputs and archives are removed after
the replacement bundles pass verification. Test source, reference fixtures,
research material and validation evidence are retained.


The renamed build passes strict Clippy and all 88 standard tests. Portable Linux
and Windows release builds pass, as do native X11 keyboard/pan regressions and
external utility-window/PDN workflows. The Windows executable starts on RTX 5080
under Wine/D3D12 with all four native utilities. Native Apple M2 Pro validation
passes all 88 standard tests and 14 Metal tests in both profiles; the macOS ZIP
is extracted and launched separately before delivery. All final bundles are ZIPs
with executable permissions and SHA-256 sidecars. Build-cache removal preserves
the new executables and complete source, fixtures, research and validation records.
