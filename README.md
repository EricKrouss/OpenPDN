# OpenPDN
OpenPDN is a native Rust image editor reverse engineered from **Paint.NET 5.1.12**. It rebuilds Paint.NET's tools, effects, file format, behavior and UI for Linux, Windows and macOS, replacing the .NET runtime with a native Rust implementation. Development is ongoing; this is **not yet a feature-complete replacement**.

The executable is `openpdn` (`openpdn.exe` on Windows), and the macOS application bundle is `OpenPDN.app`. Existing project files, preferences and named palettes remain compatible.

Development bundles are ZIP archives for Linux x86_64, Windows x86_64 and macOS Apple Silicon. Extract the archive and run `openpdn`, `openpdn.exe`, or `OpenPDN.app` for your platform. Linux and macOS ZIP entries preserve executable permissions.

Download the builds and SHA-256 checksums from [GitHub Releases](https://github.com/EricKrouss/OpenPDN/releases).

The runtime uses Rust, egui/winit, and wgpu. Layer composition, gradients and pixel processing for all thirteen adjustments and forty effects run as GPU compute shaders; the UI and canvas use the same device. Linux uses Vulkan, Windows uses Direct3D 12, and macOS uses Metal through wgpu. Hover over the tool name in the status bar to identify the actual selected adapter; startup also logs it. Software adapters are possible and do not count as hardware validation.

## Run

Rust 1.95 or later and working native graphics drivers are required. On Linux, install the X11/Wayland and xkbcommon development libraries required by eframe. The lockfile pins dependencies.

```sh
cargo run --locked --release
cargo run --locked --release -- /path/to/image.png
```

The workspace now follows Paint.NET’s arrangement: thumbnail document tabs, two toolbar rows, and separate movable Tools, History, Layers, and Colors windows. F5–F8 toggle the windows, Tab hides/restores them, Ctrl+Shift+F5–F8 resets them, and F4 opens Layer Properties. Exact visual and complete interaction parity remain unfinished; see [UI parity](docs/ui-parity.md).

Use **View → Floating Tool Windows** to float any utility window, or right-click
its title and choose **Float Outside Main Window**. Floating windows are native
OS windows and can move beyond the editor or onto another monitor. Drag their
title strip; use its dock icon to return the panel to the editor. Geometry and
floating state are saved. Wayland lets the compositor place windows and does not
expose absolute positions for restoration. Named palettes can be saved, selected
and replaced from the Colors panel’s Palettes menu.

Paint.NET's input quirks include timed and Shift-reversed tool cycling, C/X color
shortcuts, shared bracket-adjusted brush widths, live line/shape option shortcuts,
Enter deselection, and temporary Space/middle-button panning. See
[reference evidence and remaining input limits](docs/input-quirks.md).

Magic Wand uses a span-based flood traversal with the original color metric.
The [PhotoCraft comparison and selection checks](docs/magic-wand-reference.md)
explain the accuracy tradeoffs and current limits.

The current editor supports document tabs, primary/secondary RGBA colors, basic brush/pencil/eraser/bucket, Color Picker with six sample sizes and three release actions, rectangle/ellipse/lasso selections, Magic Wand, selection combination, live pixel/selection translation, scaling and rotation, seven gradient geometries with repeat/transparency controls, Clone Stamp, Recolor, live Text, 29 Shapes presets, live Line/Curve with four editable control points, pan/zoom, layer properties/order/visibility, 14 blends, image flips/rotation/flatten, GPU resizing, canvas sizing/crop, merge-down, native bitmap clipboard, workspace persistence, undo/redo, PDN/project saves and image export. All thirteen adjustments are available: Invert Colors, Invert Alpha, Black and White, Sepia, Posterize, Brightness / Contrast, Exposure, Hue / Saturation, Auto-Level, Levels, Curves, Highlights / Shadows and Temperature and Tint. Configurable adjustments and all forty effects have background GPU previews with cancellation and single-transaction undo. Only the latest queued settings are rendered; cancelled and superseded results cannot alter the document. Ctrl+F repeats the last accepted effect. Drawing exposes the original 15 shared blend choices, and Brush/Bucket/Line/Shapes expose all 53 hatch fills. Text supports inline multiline Unicode input, caret navigation, IME composition, system fonts, size/style/alignment, color, position, antialiasing and acceptance/undo; Escape accepts text as in the original. Shapes supports all 29 recovered guide geometries, outline/fill/both, fractional width, rounded radius, rotation, dashes and live move/resize. Line/Curve includes Spline, Bezier and Straight modes, four endpoint caps, five dash styles, centered/constrained drawing and whole-curve movement/rotation. Shapes also supports centered and aspect-constrained drawing with fractional bounds. Floating paste retains source pixels through movement, scaling and rotation, with Finish/Cancel. Added layers and canvas expansion keep their own undo steps. Oversized pastes offer Expand canvas, Keep canvas size and Cancel; Paste into New Layer remains a separate command. New Image starts with the current clipboard image dimensions, or 800×600 without a supported image. Rectangle Select now supports Normal, Fixed Ratio and Fixed Size with physical units. Colors has the native 96-color default palette, RGB/HSV/hex/alpha controls and palette text file interchange. Text supports document-DPI and Fixed (96 DPI) font metrics. New/Resize/Canvas Size expose print dimensions and DPI controls. Detailed Paint.NET tool options remain incomplete. File drops offer opening documents or importing matching-size images as layers. Open supports PNG, JPEG, BMP, TIFF, GIF, WebP, ICO and QOI through Rust decoders; not all original formats are supported.

The final fourteen effect entries are now enabled: Bokeh, Sketch Blur, Quantize, Crystalize, Dents, Pixelate, Red Eye Removal, Sharpen, Straighten, Clouds, Julia Fractal, Mandelbrot Fractal, Turbulence and Edge Detect. Controls include sampling/quality, palette/dither/alpha, random seeds, RGBA render colors and 46 render mix modes. Palette construction and Red Eye analysis run on the CPU; palette mapping, error diffusion, eye correction and other image processing run on the GPU. See [algorithm evidence and remaining precision limits](docs/filter-reference.md).

Save projects as `.pdn` to preserve layers in Paint.NET's actual format. Bidirectional reference checks verify original Paint.NET files and original-reader acceptance of Rust-written files, including pixels, 14 blends, Unicode names and resolution. Metadata/profile payloads are preserved; color-managed rendering remains pending. See [PDN interoperability and limits](docs/pdn-compatibility.md).

`.opdn` remains a separate development ZIP format, with metadata in version 2 and backward reading of version 1. Raster export flattens layers. Exact renderer certification and extension compatibility remain explicit requirements. All four Gaussian Blur qualities now execute GPU paths, including the recovered native downsampled kernels. Native Direct2D pixel/profile certification remains pending. Detailed options and exact behavior of implemented tools also remain unfinished; see [processing evidence](docs/processing-parity.md) and [new filter evidence](docs/filter-reference.md).

The tool grid, workspace controls, adjustments and effect menus use 121 hand-crafted vector icons, including Magic Wand, separate pixel/selection moves, clipboard commands and drag-and-drop. No image generation or copied Paint.NET artwork is used. Export the exact artwork with `cargo run --locked --example icon_atlas -- artifacts/icons.svg`.

## Verify

Brush, Eraser, Clone Stamp and Recolor now include circle/square tips, hardness, spacing, antialiasing,
smoothing and pressure options, with the original 1–2,000-pixel width range.
Their recovered path math has original-method fixtures; portable tip pixels and
physical tablet input still need native certification. See [brush evidence](docs/brush-reference.md).

```sh
cargo fmt --all --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo test --locked --test gpu -- --ignored --nocapture
cargo test --locked --test filters -- --ignored --nocapture
cargo test --locked --test gaussian -- --ignored --nocapture
cargo test --locked --test tool_blend -- --ignored --nocapture
cargo test --locked --test neighborhood -- --ignored --nocapture
cargo test --locked --test remaining_effects -- --ignored --nocapture
cargo test --locked --test editing_commands -- --ignored --nocapture
cargo test --locked --test transform -- --ignored --nocapture
cargo run --locked -- --gpu-info
cargo run --locked -- --render image.opdn flattened.png
cargo run --locked -- --smoke-frames 10
```

The GPU test is opt-in and rejects CPU/software adapters. Standard CI runs CPU/reference tests and builds on all three OS families; it does not prove native hardware behavior. The checked-in reference fixtures come from executing original managed blend, flood-distance, posterize-table, gradient, Levels/spline, Gaussian kernels, noise tables/random generation and PDN serialization routines, rather than treating the Rust implementation as its own oracle.

An isolated Linux UI exercise requires Xvfb, python-xlib and Pillow:

```sh
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 python3 tools/ui-smoke.py
```

For native window dragging, use a private KWin session on the isolated desktop
(requires `kwin_x11` and `dbus-run-session`):

```sh
dbus-run-session -- xvfb-run -a -s '-screen 0 1920x1080x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 python3 tools/ui-native-window-manager.py
```

## Reverse engineering

The reverse-engineering work examines Paint.NET's .NET assemblies, image formats, numerical algorithms and UI behavior. Recovered behavior is reimplemented in Rust and checked against results from original Paint.NET methods and real `.pdn` files. Paint.NET binaries and local decompilation remain development references and are never bundled with the editor. The reference probes under `tools/` use developer-only .NET, Unicorn or Wine tooling; the Rust application has no .NET runtime dependency.

See the [feature inventory](docs/reference-features.md), [status and remaining gates](docs/status.md), and [binary/fixture provenance](docs/reference-manifest.json) for the recovered behavior and its validation evidence.

The replacement remains incomplete until the inventory has implemented, reference-verified behavior and native validation on all requested platforms. See the status file for limitations of this development build.

Linux, Windows GNU and Apple Silicon release binaries build from the lockfile. The Windows target has Wine CPU and D3D12 checks through Wine/VKD3D-Proton; native Windows remains unverified. The current Apple M2 Pro/Metal bundle passes 88 standard tests, all 14 hardware tests in both debug and release, native floating-window startup and app-bundle launch. [Native backend validation](docs/native-validation.md) and [editing evidence](docs/editing-reference.md) record the exercised behavior and remaining limits. `tools/package-native.py` packages already-built binaries with stable archive timestamps and checksums; package reproducibility does not certify reproducible compiler output across hosts.

For the distributed x86_64 Linux build, run `python3 tools/build-portable-linux.py` before packaging. It uses a checksum-pinned Zig 0.14.1 linker to cap imported glibc symbols at 2.35, avoiding this development host's newer runtime dependencies. The toolchain stays in ignored `reference/`; it is not bundled. Graphics drivers and the native window/font libraries still come from the host. The package has not been certified on every distribution.

A self-contained source bundle and actual-Mac validation script are documented in [native backend validation](docs/native-validation.md).
