# Paint.NET 5.1.12 behavior inventory

Research date: 2026-10-08. Target: the supplied Paint.NET 5.1.12 distribution. Its official release date is 2026-03-08; it adds Kazakh and updates bundled DDS FileType Plus to 1.12.14 and JPEG XL FileType to 1.2.1. [Official release](https://blog.paint.net/2026/03/08/paint-net-5-1-12-is-now-available/)

This is a requirements inventory, **not a claim that OpenPDN implements these features**. Sources are first-party user documentation and developer announcements. The `/doc/latest/` pages are moving documentation, often with older revision dates; they must be reconciled with the exact 5.1.12 executable before behavioral parity is certified. No proprietary implementation or assets are reproduced here.

Acceptance criteria below are project test proposals. Documentation establishes feature intent; actual reference-generated fixtures establish pixel, interaction, parameter, and persistence behavior. A menu entry or an approximately similar algorithm does not satisfy a parity requirement.

## Documents, editing sessions, and workspace

| Required capability | Documented behavior | Proposed acceptance |
| --- | --- | --- |
| Multiple documents | Thumbnail tabs, tab switching and reordering; histories belong to individual documents. [Overview](https://paint.net/doc/latest/index.html), [Edit](https://paint.net/doc/latest/EditMenu.html) | Edit two different images, reorder/switch tabs, and verify each document's pixels, dirty state, active layer, viewport, and undo state independently. |
| File lifecycle | New white document; default 800×600 adjusted for system DPI or clipboard dimensions. Open, URL open, ten recent files, Save, Save As, Save All, Close, Exit, and unsaved-change prompts. New-image dialog includes dimensions, print size, resolution, aspect lock, and units. [File](https://paint.net/doc/latest/FileMenu.html) | Exercise named/unnamed/modified documents, cancel every dialog, verify paths and saved content, and confirm Save All does not lose unsaved edits. |
| Utility panels | Tools, History, Layers, and Colors; show/hide via F5–F8. Tools panel can be repositioned/reset. [Tools](https://paint.net/doc/latest/ToolsWindow.html), [Shortcuts](https://paint.net/doc/latest/KeyboardMouseCommands.html) | Panels remain usable at multiple DPI scales and after resizing, monitor changes, hiding, and resetting. |
| View controls | Actual size, zoom in/out, fit window, fit selection, pixel grid at ≥200%, rulers, and pixel/inch/centimeter units. Selection bounds and pointer position appear in rulers. [View menu](https://paint.net/doc/latest/ViewMenu.html) | View actions leave document pixels/history unchanged. Check pointer-to-pixel mapping and ruler values at fractional DPI and extreme zoom. |
| History | Editing and selection actions are recorded; jump to an entry, Undo/Redo, and compare adjacent states. Editing after an undo discards the redo branch. Closing the image/application discards session history. [History](https://paint.net/doc/latest/HistoryWindow.html) | Rewind/redo a mixed sequence of pixels, masks, layers, properties, and dimensions; compare every state. Test branch replacement, independent documents, and disk spill without corruption. |
| History capacity | Session history is constrained by available memory/disk rather than a fixed action count; temporary disk storage is used. [Overview](https://paint.net/doc/latest/index.html), [Requirements](https://paint.net/doc/latest/SystemRequirements.html) | Run a long editing session larger than the configured RAM budget; verify undo completeness, bounded memory, and graceful storage exhaustion. |

## All 19 tools

The tool roster comes from the [official Tools window](https://paint.net/doc/latest/ToolsWindow.html). Shared selection constraints and tool blending are specified separately below. Each tool must be reachable through the UI and its documented keyboard shortcut, and must be undoable when it changes document state.

| Tool | Required behavior/options and source | Proposed acceptance fixture |
| --- | --- | --- |
| Rectangle Select | Any size, fixed ratio, fixed size; pixels/inches/centimeters; Shift square constraint; fixed-size/ratio off-canvas clamping. [Rectangle](https://paint.net/doc/latest/RectangleSelectionTool.html) | Drag in all directions, use fractional ratios and physical units, cross every canvas edge, and compare mask/bounds. |
| Lasso Select | Pointer path creates a freeform region, closed back to its starting point. [Selections](https://paint.net/doc/latest/SelectionTools.html) | Concave, self-crossing, tiny and off-canvas paths; compare final topology and coverage. |
| Ellipse Select | Elliptical selection, with Shift constraining a circle. [Ellipse](https://paint.net/doc/latest/EllipseSelectionTool.html) | Small odd/even diameters, reversed drag, circle constraint, partial clipping. |
| Magic Wand | Contiguous/global; layer/composite sampling; tolerance; premultiplied/straight alpha comparison. Before finishing, move the seed or change tolerance to recompute. [Wand](https://paint.net/doc/latest/MagicWand.html) | Disconnected equal colors, translucent edges, fully transparent pixels with differing RGB, and a composite that differs from its active layer. |
| Move Selected Pixels | Translate/scale/rotate active pixels, or the entire active layer without a selection. Leaves transparency; supports copying, movable rotation center, flips through handles, proportional/centered resize, 1/10-pixel nudges, and 15° snapping. Five samplers: nearest, bilinear, multisample bilinear, anisotropic, bicubic; gamma-aware or ignored-gamma sampling. [Move](https://paint.net/doc/latest/MoveTools.html) | Asymmetric colored marker with alpha; shrink/enlarge/rotate/reflect, cross canvas boundaries and return, compare each sampler/gamma mode, then undo. |
| Move Selection | Same geometric manipulation applied to the selection outline while image pixels remain untouched. [Move](https://paint.net/doc/latest/MoveTools.html) | Hash image before/after; compare moved mask and selection history, including off-canvas pivots. |
| Zoom | Left/right click zoom in/out; rectangle zoom; Ctrl-wheel; predefined steps. [View tools](https://paint.net/doc/latest/ViewTools.html) | Zoom about a marked location, fit a dragged area, preserve editing coordinates. |
| Pan | Drag, temporary Space activation, wheel/Shift-wheel, keyboard movement, overscroll and autoscroll. [View tools](https://paint.net/doc/latest/ViewTools.html) | Pan at image edges under another tool, confirm no accidental stroke or image mutation. |
| Paint Bucket | Contiguous/global fill, tolerance, layer/composite sampling, alpha comparison, patterned fills, primary/secondary mouse colors, blending. Live seed, color, and tolerance changes recompute before Finish. [Bucket](https://paint.net/doc/latest/PaintBucket.html) | Holes, disjoint colors, selection boundaries, alpha/RGB mismatch, pattern origin, live changes, finish and undo. |
| Gradient | Seven geometries; RGBA color or alpha-only transparency; no/wrapped/reflected repeat; editable handles and direction reversal; tool blend mode. Transparency mode uses primary alpha and inverse secondary alpha. [Gradient](https://paint.net/doc/latest/GradientTool.html) | Compare every geometry × repeat × color/transparency mode, partial-alpha endpoints, reverse direction, clipped selections, and live handle movement. |
| Paintbrush | Primary/secondary stroke colors; fractional size, pressure-controlled width, hardness, spacing, input smoothing, solid/patterned fills. Hardness is ignored without antialiasing. [Brush](https://paint.net/doc/latest/Paintbrush.html) | Recorded pointer/pressure sequences, fast sparse input, varying hardness/spacing, overlapping semitransparent strokes, fractional widths. |
| Eraser | Circular brush; size/pressure/hardness/spacing/smoothing/antialiasing. Mouse-associated color alpha controls erasure strength. Full erase writes transparent black; partial erase retains RGB. [Eraser](https://paint.net/doc/latest/EraserTool.html) | Opaque/translucent initial pixels, repeated soft erasure, primary versus secondary alpha, hidden RGB, clipped edge. |
| Pencil | Single-pixel tip/one-pixel path, primary/secondary color and alpha, tool blend modes. [Pencil](https://paint.net/doc/latest/Pencil.html) | Horizontal, steep, diagonal, reversed and interrupted paths; exact pixel coverage and overwrite alpha. |
| Color Picker | Active layer or composite; square samples 1, 3, 5, 11, 31, or 51 pixels; averaged sample; primary/secondary destination. Post-pick: stay, previous tool, or Pencil. [Picker](https://paint.net/doc/latest/ColorPicker.html) | Color/alpha checkerboard, partially off-canvas samples, hidden layers, all post-pick choices. |
| Clone Stamp | Ctrl-click source; same/different layer within a document; source/destination offset persists. Size, pressure, hardness, spacing, smoothing, antialiasing, blending, selection quality, and mouse-associated color opacity. [Clone](https://paint.net/doc/latest/CloneStamp.html) | Copy from another layer, overlapping source/destination, alpha scaling, preserve offset across tool changes, reset source. |
| Recolor | Circular brush; fractional size, pressure, hardness, spacing, tolerance, alpha comparison, smoothing, antialiasing, selection quality. Sampling once or matching secondary color; right click reverses color roles. [Recolor](https://paint.net/doc/latest/RecolorTool.html) | Hue/shading variations, tolerance extremes, transparent hidden RGB, both sample modes and buttons. |
| Text | Live font/size/color/style/alignment/position editing, fractional point size, image-DPI or fixed-96-DPI metric, colored fonts. Bold/italic/underline/strikeout; smooth, modern sharp, classic sharp rendering; antialiasing and blending. Finish rasterizes text. [Text](https://paint.net/doc/latest/TextTool.html) | Same licensed test font and text across DPI/metrics; multiline, emoji/color fonts, non-Latin shaping, committed versus live editing. Exact font raster parity needs reference evidence. |
| Line/Curve | Straight segments, cubic spline, or Bézier; four handles; live move/rotation; primary/secondary color; fractional width, start/end caps, dashes/dots, patterned fill, antialiasing, and blending. Centered drawing and angular constraints. [Line/Curve](https://paint.net/doc/latest/LineCurveTool.html) | Same four points in all curve modes; each cap/dash at odd/fractional width; changed properties before finish; transparent overwrite. |
| Shapes | 29 presets; outline/fill/both, mouse color-role reversal, fractional outline, rounded-corner size, patterned fill, dashes, antialiasing, blending, selection quality. Live transform and movable pivot; custom XAML shapes. [Shapes](https://paint.net/doc/latest/ShapeTools.html) | Render each preset in all drawing modes; edit/flip/rotate before finish; verify a supplied custom shape independently. |

Gradient geometries: linear, reflected linear, diamond, radial, conical, clockwise spiral, anticlockwise spiral. [Gradient](https://paint.net/doc/latest/GradientTool.html)

Shape roster: rectangle, rounded rectangle, ellipse, diamond, trapezoid, parallelogram, triangle, right-angle triangle; pentagon, hexagon, heptagon, octagon; three/four/five/six-point stars; arrow, notched arrow, pentagon arrow, chevron arrow; rectangular, rounded rectangular, ellipse, and cloud callouts; lightning bolt, check mark, multiply, gear, heart. [Shapes](https://paint.net/doc/latest/ShapeTools.html)

## Selections, clipboard, transforms, and layers

| Area | Required behavior and source | Proposed acceptance |
| --- | --- | --- |
| Mask algebra | Replace, union, subtraction, intersection, exclusive-or; pixelated/antialiased clipping; dashed boundary and tool-specific highlighting. Drawing, effects, and adjustments obey the selection. [Selections](https://paint.net/doc/latest/SelectionTools.html) | Test overlapping masks, holes, disconnected islands, fractional edges; verify zero changes outside selected coverage for every destructive command. |
| Clipboard | Cut, Copy, Copy Merged, Paste, Paste into New Layer/Image; pasted pixels overwrite, become selected, and activate Move Selected Pixels. Oversized paste offers canvas expansion. Selection geometry uses JSON polygon lists and supports combine modes on paste. [Edit](https://paint.net/doc/latest/EditMenu.html) | Exchange alpha-bearing pixels and selection JSON with reference Paint.NET; compare origin, bounds, active tool/layer, and cancellation. |
| Selection commands | Erase writes transparent black and deselects; Cut writes transparent white and deselects. Fill uses primary color and retains selection; invert, select all, deselect. [Edit](https://paint.net/doc/latest/EditMenu.html) | Include transparent pixels with meaningful hidden RGB and check byte-level results, mask lifetime, clipboard payload, and undo. |
| Image-wide operations | Crop to selection, Resize, Canvas Size, horizontal/vertical flips, 90° clockwise/counterclockwise, 180°, Flatten. Nonrectangular crop uses selection bounds and clears outside-mask pixels. Resize has percent/absolute sizes, DPI/print units, aspect lock, gamma correction. [Image](https://paint.net/doc/latest/ImageMenu.html) | Use distinct asymmetric layers; verify every layer, dimensions, selection reset, pixel results and undo. |
| Resize samplers | Bicubic/Catmull-Rom, smooth bicubic, bilinear, low-quality bilinear, adaptive sharp, Lanczos, Fant, nearest neighbor. Canvas Size has anchor and transparent/primary/secondary/white/black fill options. [Image](https://paint.net/doc/latest/ImageMenu.html) | High-contrast impulses, checkerboards, ramps, translucent borders; downscale and upscale across all modes and anchors. |
| Layer operations | Add transparent-black layer above active; delete except final layer; duplicate attributes/pixels; merge down; visibility; import files as layers with canvas enlargement; layer flips/180°; top/above/below/bottom focus and ordering. Properties: name, visibility, opacity, blend. [Layers](https://paint.net/doc/latest/LayersMenu.html) | Compare pixels, stack ordering, focus, naming and all properties after each action and undo; verify imported multi-layer reference files. |
| Layer semantics | Only one active layer; visibility excludes composition but retains the layer in PDN. Per-layer opacity is 0–255. Stack ordering affects the composite. [Working with layers](https://paint.net/doc/latest/WorkingWithLayers.html) | Hidden-layer persistence, opacity extremes, reorder transparent layers, merge versus flatten under different blends. |
| Rotate/Zoom | Layer rotation about X/Y/Z, pan, zoom, quality; no/repeat/mirror tiling; nearest or bilinear sampling. [Rotate/Zoom](https://paint.net/doc/latest/RotateZoom.html) | Perspective grids, all tiling modes, nonzero tilt and both samplers; compare edge handling and undo. |

Fourteen layer blends: Normal, Multiply, Additive, Color Burn, Color Dodge, Reflect, Glow, Overlay, Difference, Negation, Lighten, Darken, Screen, Xor. Tool blends apply as if drawing above the active layer and merging down; tool-only Overwrite replaces destination pixels including alpha. [Blend modes](https://paint.net/doc/latest/BlendModes.html)

Blend acceptance: reference tables across color/alpha extremes, opacity 0/1/127/128/254/255, swapped layer order, hidden layers, and transparent RGB. Test every layer blend and applicable tool blend on both CPU and GPU; freeze rounding and working-space behavior from observations rather than generic formulas.

## All 13 documented adjustments

These affect the active layer/selection. Configurable dialogs have combined live controls; outside-selection pixels are unchanged. [Adjustments](https://paint.net/doc/latest/AdjustmentsMenu.html)

| Adjustment | Required control/behavior |
| --- | --- |
| Auto-Level | Automatic leveling; no dialog; equivalent to Levels' Auto. |
| Black and White | Grayscale; no dialog. |
| Brightness / Contrast | Brightness and contrast. |
| Curves | Luminosity or RGB-channel control curves. |
| Exposure | Exposure change. |
| Highlights / Shadows | Highlight and shadow changes. |
| Hue / Saturation | Hue, saturation, lightness. |
| Invert Alpha | Alpha inversion; applying twice restores original. |
| Invert Colors | Color inversion; applying twice restores original. |
| Levels | Color range and gamma. |
| Posterize | Independently linked/unlinked RGB quantization, 2–64 levels. The supplied 5.1.12 `PosterizeGpuEffect` additionally exposes alpha enable/levels, enabled by default; executable evidence takes priority. |
| Sepia | Sepia intensity. |
| Temperature and Tint | Color temperature and tint. |

All entries above: [official Adjustments menu](https://paint.net/doc/latest/AdjustmentsMenu.html). The summary Menu Bar page's older count of nine is not sufficient evidence for this baseline.

Curves needs add/drag/remove control points and independent/pair/all-channel editing. [Curves](https://paint.net/doc/latest/Curves.html) Levels needs input black/white, output black/gray/white, gamma/channel selection, editable swatches and numeric controls, live input/output histograms. [Levels](https://paint.net/doc/latest/Levels.html)

Adjustment acceptance: capture each reference dialog's exact labels, defaults, ranges, precision, reset and cancellation. Compare minimum/default/maximum and representative mixed settings using RGB/gray/alpha ramps and profiles; confirm dialog cancellation restores pixels and accepted operations undo exactly.

## All 40 documented built-in effects

Counting the individual detailed pages yields 40 effects. The older Menu Bar summary advertises 33 and the Photo introduction says five while its detail lists six; executable inspection must settle these inconsistencies. Each family below has its own first-party source. Parameters are abbreviated here; exact defaults/ranges remain fixture work.

| Family | Effect roster and documented controls | Source |
| --- | --- | --- |
| Artistic (3) | Ink Sketch: outline, coloring. Oil Painting: brush size, coarseness. Pencil Sketch: tip size, range. | [Artistic](https://paint.net/doc/latest/EffectsArtisticMenu.html) |
| Blurs (10) | Bokeh: radius, gamma boost, quality. Fragment: count, distance, rotation. Gaussian: radius, gamma boost, quality. Median: radius, percentile, quality. Motion: angle, distance, centered, edge behavior. Radial: angle/center/quality. Sketch: radius, percentile, smoothness. Square: radius, gamma boost. Surface: radius, threshold. Zoom: distance, focus, center, quality. | [Blurs](https://paint.net/doc/latest/EffectsBlursMenu.html) |
| Color (1) | Quantize: median-cut/octree, ≤256 colors, nine dithering levels. | [Color](https://paint.net/doc/latest/EffectsColorMenu.html) |
| Distort (9) | Bulge: strength, center, edges, quality. Crystalize: cells, quality, randomize. Dents: scale, refraction, detail, turbulence, angle, quality, randomize. Frosted Glass: scatter bounds, diffusion, smoothness, randomize. Morphology: erode/dilate, linked width/height. Pixelate: cell size, down/up samplers. Polar Inversion: scale, offset, edges, quality. Tile Reflection: angle, tile size, curvature, edges, quality. Twist: amount/direction, size, center, quality. | [Distort](https://paint.net/doc/latest/EffectsDistortMenu.html) |
| Noise (2) | Add Noise: intensity, saturation, coverage, randomize; fully transparent areas remain unchanged. Reduce Noise: radius, strength. Median moved to Blurs. | [Noise](https://paint.net/doc/latest/EffectsNoiseMenu.html) |
| Object (1) | Drop Shadow: radius, distance, angle, opacity, color, shadow-only; requires an alpha boundary. | [Object](https://paint.net/doc/latest/EffectsObjectMenu.html) |
| Photo (6) | Glow: radius, brightness, contrast. Red Eye Removal: strength. Sharpen: amount, threshold. Soften Portrait: softness, lighting, warmth. Straighten: ±45°, 15° snapping, bicubic/nearest/bilinear. Vignette: center, radius, strength; grayscale halo. | [Photo](https://paint.net/doc/latest/EffectsPhotoMenu.html) |
| Render (4) | Clouds: scale, roughness, blend, randomize, two editable colors. Julia: factor, zoom, angle, quality, blend. Mandelbrot: same plus invert colors. Turbulence: octaves, period, size, turbulence/fractal-sum choice, randomize, blend/overwrite. | [Render](https://paint.net/doc/latest/EffectsRenderMenu.html) |
| Stylize (4) | Edge Detect: strength, blur, Prewitt/Sobel, overlay. Emboss: angle, grayscale output. Outline: thickness, intensity, quality. Relief: angle, combines with source. | [Stylize](https://paint.net/doc/latest/EffectsStylizeMenu.html) |

Effects apply only inside an active selection. Repeat Last Effect reuses parameters without showing the dialog (Ctrl+F); plugin menu entries carry identifying metadata. [Effects menu](https://paint.net/doc/latest/EffectsMenu.html)

Effect acceptance: verify all 40 individually, including transparent boundaries, edge modes, offset centers, non-square/panoramic images, gamma/profile interactions, selections, cancellation, repeat and undo. Seeded random effects need reference seed/output capture; matching a distribution is not exact parity. No-op aliases for separate named effects do not meet acceptance.

## Color and palettes

Image Color Profile supports sRGB, Adobe RGB, Display P3, ProPhoto RGB, imported ICC/ICM and system display profiles, plus profile export. Assign changes interpretation without changing samples; Convert transforms all layers into the target color space. [Image menu](https://paint.net/doc/latest/ImageMenu.html)

Primary/secondary slots, alpha, RGB/HSV controls, editable hex, wheel modifiers, swap/reset, and compact/expanded palettes are required. [Colors](https://paint.net/doc/latest/ColorsWindow.html) Palettes contain 96 colors, expose the first 32 when compact, and support edit/save/load/reset; text files use AARRGGBB per line and semicolon comments, padding missing entries with white and ignoring excess entries. [Palettes](https://paint.net/doc/latest/WorkingWithPalettes.html)

Color acceptance: compare sample values versus displayed color separately. Exercise assign versus convert, profile save/reopen, copied content between different profiles, profile export, all-layer conversion, gamma-corrected transforms/effects and palette alpha round trips. Evaluate SDR and HDR/wide-gamut output on real configured displays; screenshots alone do not establish display accuracy.

## File format and platform integration gates

The official load/save roster has 13 families. [File menu](https://paint.net/doc/latest/FileMenu.html)

| Format | Extensions | Required gate |
| --- | --- | --- |
| Native Paint.NET | `.pdn` | Read and write actual 5.1.12 files, including layers/properties/profile. |
| PNG | `.png` | Alpha, profiles, metadata, bit-depth and indexed variants. |
| JPEG | `.jpg`, `.jpeg`, `.jpe`, `.jfif`, `.exif` | Quality settings and supported metadata/profile behavior. |
| JPEG XL | `.jxl` | Reference bundled 1.2.1 codec behavior. |
| JPEG XR | `.jxr`, `.wdp`, `.wmp` | Import/export actual reference fixtures. |
| Bitmap | `.bmp`, `.dib`, `.rle` | Supported layouts, bit depths, compression. |
| GIF | `.gif` | Transparency and dithering settings. |
| TGA | `.tga` | 24/32-bit and RLE choices. |
| DDS | `.dds` | Reference bundled FileType Plus 1.12.14 behavior. |
| TIFF | `.tif`, `.tiff` | Supported layouts, profiles and metadata. |
| HEIC | `.heic`, `.heif`, `.hif` | Native reference requires Microsoft HEVC extension. |
| WebP | `.webp` | Lossy/lossless/alpha behavior; documented size limit 16383². |
| AVIF | `.avif` | Load/save limit requires validation: same page says 65536² for load, 65535² for save. |

Native PDN is lossless and preserves layers; multi-layer save defaults to PDN and single-layer save defaults to PNG. AVIF, DDS, WebP, and JPEG XL use bundled native FileType plugins. [Overview](https://paint.net/doc/latest/index.html) Existing PDN depends on older .NET serialization, and the official roadmap places a new documented format in future v6 work. [Roadmap](https://paint.net/roadmap.html)

Save configuration offers preview/file-size feedback and automatic bit-depth choice. Scan acquisition uses WIA 2.0; Print uses Windows' photo-printing UI. [File menu](https://paint.net/doc/latest/FileMenu.html)

Format acceptance: test actual Paint.NET-produced inputs and reopened Rust outputs, separately for lossless pixel equality, lossy quality, layer/property equality, and metadata/profile preservation. A new Rust-native container is useful but does not satisfy PDN interoperability. Scan/print need working platform-native equivalents and device tests on Linux, Windows and macOS. Encoders must expose real settings; merely accepting an extension is insufficient.

## Acceleration and extension architecture

Paint.NET 5.1 migrated built-in adjustments/effects to GPU except Quantize and Red Eye Removal. It uses profile-aware linear working space for GPU effects. The release also describes compute-shader Median Blur, a GPU tile cache, shared presentation swapchains, DXGI flip presentation, and asynchronous presentation to reduce input latency. [5.1 announcement](https://blog.paint.net/2024/11/12/paint-net-5-1-is-now-available/)

Earlier 5.0 behavior documents GPU canvas/UI, 32-bit-float effect processing, accelerated bicubic/anisotropic pixel transforms, and separate UI/canvas versus image-rendering adapter settings. [5.0 announcement](https://blog.paint.net/2023/01/10/paint-net-5-0-is-now-available/) Graphics settings select acceleration and rendering device; color status reports the active display configuration. Canvas border, shadow and checkerboard brightness, saved tool defaults, pen settings, plugin errors, diagnostics and update settings are user-visible configuration. [Settings](https://paint.net/doc/latest/SettingsDialog.html)

The reference runs on 64-bit Windows with .NET; Direct3D 11-class GPU support is expected, with software rendering for unsupported graphics. SDR/software output is sRGB; HDR/wide-gamut output uses Windows Advanced Color. [Requirements](https://paint.net/doc/latest/SystemRequirements.html)

Acceleration acceptance: use a native Rust GPU backend on all three target OSes; identify the adapter/backend at runtime and prove actual GPU work with readback compared to independently observed reference results. Measure paint response, pan/zoom, composition, transform, effect preview and final render on meaningful documents. Include software fallback, hardware-disabled runs, device loss, memory pressure, large images exceeding texture dimensions, and cancellation. Creating a GPU surface without accelerating editing/composition/effects is not feature-complete acceleration.

Plugins include Effects/Adjustments and FileTypes, installed as DLLs, discovered at startup, shown in menus or format lists, and diagnosed through Plugin Errors. Installation mode changes discovery locations. [Plugins](https://paint.net/doc/latest/InstallPlugins.html) Official developer examples distinguish CPU `BitmapEffect` and Direct2D `GpuEffect`; GPU inputs include layers, document composite and selection mask. [Developer samples](https://github.com/paintdotnet/PdnV5EffectSamples) API documentation is incomplete and currently published under `preview`; it is not a frozen 5.1.12 specification. [API overview](https://www.paint.net/doc/api/preview/index.html)

Extension acceptance: first supply a Rust-native documented CPU/GPU effect and file-codec interface with selection, color, preview, cancellation and error handling. Existing Paint.NET DLL compatibility is a separate gate: inventory each required DLL's .NET, native-code, UI and Direct2D dependencies and demonstrate actual execution. Pure Rust reimplementation does not automatically make existing DLLs portable. Custom XAML shape and palette import are independently testable data compatibility requirements.

## Unresolved evidence and completion criteria

1. **Executable inventory:** reconcile the live documentation against the pinned installer: exact menus, parameters, defaults/ranges, built-in shape/pattern definitions, localized labels, dialogs and accelerators. Detailed counts above are documented scope, not executable observations.
2. **Algorithms:** public user docs omit enough detail that exact tolerance, stroke interpolation/smoothing, antialiasing, blend rounding, histogram/auto-level, random seeds, some effect kernels and border semantics remain unverified. Capture original behavior and keep unknowns explicit.
3. **Document serialization:** current PDN encoding/schema, metadata, compression and legacy versions need independent interoperability evidence. The roadmap does not provide a current public format specification.
4. **Fonts and clipboard:** cross-platform font shaping/rasterization, color fonts and system clipboard payloads require native OS tests. The Text page's point-size formula deserves executable checking rather than direct implementation.
5. **OS integrations:** scanner/printer, file association, dialogs, accessibility, pen pressure, display profiles/HDR, locale and installation/update flows need platform-native behavior. Windows-only original integrations cannot be presumed portable.
6. **Extensions:** unlimited third-party plugin feature scope cannot be certified without a fixed compatibility corpus. Record bundled codecs and every required external plugin/version separately; report incompatible plugins directly.
7. **Scope exclusions from the pinned baseline:** future text/mask/adjustment layers, high-bit-depth storage and layer groups appear in the roadmap, not as established 5.1.12 features. They may be added, but cannot substitute for existing raster-tool parity. [Roadmap](https://paint.net/roadmap.html)

The goal is complete only when every required capability has a shipped implementation and recorded acceptance evidence, all 19 tools/13 adjustments/40 effects work through the desktop UI, format and PDN interoperability are demonstrated, accelerated output and fallback are checked, and native Linux/Windows/macOS release packages pass the same representative user workflows. Compilation, headless unit tests, Wine execution, API-only operations, and a Linux-only GPU check each provide narrower evidence; none alone certifies the requested feature-complete cross-platform replacement.
