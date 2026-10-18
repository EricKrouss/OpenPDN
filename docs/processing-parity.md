# Tools and GPU processing evidence

Updated 2026-10-12. The frozen reference is the supplied Paint.NET 5.1.12 x64 distribution; assembly and fixture hashes are in [reference-manifest.json](reference-manifest.json). This document distinguishes recovered algorithms from original-renderer certification.

## Implemented additions

| Feature | Recovered reference | Present behavior and boundary |
| --- | --- | --- |
| Gradient | `GradientShapes`, `GradientRepeaters`, `GradientToolContentRenderer`, `GradientColorRenderer`, `ColorBgra32.Blend` | Seven geometries, three repeats, reversal, color/transparency and normal/overwrite modes; GPU rendering while dragging, Escape rollback, one undo step. Uses the reference non-antialiased, non-dithered color path. Adaptive antialiasing/dithering, retained editable handles, tool blend modes beyond Normal/Overwrite and reference input certification remain pending. |
| Clone Stamp | `CloneStampTool`, `CloneStampToolContentRenderer` | Ctrl-click source layer/point, persistent integer source offset, source marker, circular strokes, primary/secondary alpha as opacity, selection clipping, immutable source snapshot per stroke and undo. Shared circle/square, hardness, spacing, antialiasing, smoothing and pressure options are implemented. Stable source identity after layer reordering/deletion and original brush pixels remain pending. Switching documents clears the source. |
| Recolor | `RecolorTool`, `RecolorToolContentRenderer`, `FloodFillAlgorithm` | Secondary-color or stroke-start sampling, clamped RGBA channel differences, reference tolerance distance, normal composition, reverse colors with the secondary button, selection clipping and undo. Soft brush masks and shared brush options are implemented, including fractional selection coverage. Exact native tolerance-stencil feathering and full original brush behavior remain pending. |
| Text | `TextTool` font properties, per-line positions/alignment and live edit lifecycle | COSMIC Text system-font discovery, Unicode shaping/fallback and rasterization; fixed-96-DPI point sizes, bold/italic/underline/strikeout, alignment, primary/secondary color/alpha, antialias/overwrite, selection clipping, movable position, preview/finish/cancel/undo. Uses inline canvas input, a shaped caret, hit testing, line/word navigation, IME preedit/commit/cancel, right-drag movement and Escape acceptance. DirectWrite pixel equality, image-DPI font metrics, rendering-mode choices, native IME and exact caret metrics and native color-font certification remain pending. Text rasterization is CPU-side; canvas composition/presentation uses wgpu. [Font stack](https://docs.rs/cosmic-text/0.19.0/cosmic_text/). |
| Shapes | All 29 original Basic/Polygons/Stars/Arrows/Callouts/Symbols guide definitions, star inner ratios, rectangle outline/fill insets and rounded radii | 29 distinct presets, outline/fill/both, primary/secondary role reversal, fractional outline, radius, dashes, antialias/overwrite, rotation and retained move/resize handles. CPU vector coverage with GPU canvas composition. All 15 drawing blend choices and 53 hatch fills are available. Exact Direct2D coverage, remaining dash controls, four-corner/pivot/rotation interactions, aspect constraints and custom XAML remain pending. [Rasterizer](https://docs.rs/tiny-skia/0.12.0/tiny_skia/). |
| Auto-Level / Levels | `AutoLevelGpuEffect`, `PdnAutoLevel`, `LevelsParameters`, `HistogramData` | Selected RGB histogram, 0.5%/99.5% endpoints and mean-derived gamma, per-channel input/output points and gamma 0.1..10; Auto button, histogram, GPU transfer preview/reset/cancel/accept. Original managed lookup tables are fixture-verified. Profile conversion and original histogram/Levels dialog layout remain pending. |
| Curves | `CurvesGpuEffect`, `FunctionInterpolator.SplineInterpolator`, `LuminosityTableTransferEffect` shader | Editable natural cubic spline, 1,024-entry tables, RGB/luminosity modes, channel editing, point add/drag/remove, gamut-preserving luminosity change, GPU preview and one accepted undo step. Original managed spline samples are fixture-verified; original GPU/profile output and complete dialog UI remain pending. |
| Brightness / Contrast | `BrightnessContrastGpuEffect`, `PdnBrightnessContrastEffect` | Both integral controls default 0, range -100..100. Recovered luma-preserving shift, separate full-contrast threshold, GPU preview/reset/cancel/accept. |
| Exposure | `ExposureGpuEffect` | Integral -200..200, default 0, divided by 100 into stops. Uses sRGB decoding, linear exposure scaling and sRGB encoding. Display/document profile processing remains pending. |
| Hue / Saturation | `HueSaturationGpuEffect`, `PdnHueSaturationLightnessEffect` | Hue -180..180, saturation 0..200 (default 100), lightness -100..100. Recovered saturation amplification above 100 and premultiplied lightness interpolation. HSL conversion and output remain subject to original Direct2D comparison. |
| Vignette | `VignetteGpuEffect`, `PdnVignetteEffect` | Center X/Y -1..1, radius 0.1..4 (default 0.5), strength 0..1 (default 1). Reference cosine-to-the-fourth falloff in linear space; selection bounds determine center/radius. GPU preview, cancel, commit and repeat. |
| Relief | `ReliefGpuEffect`, `PdnReliefEffect` | Angle -180..180 (default 45); recovered directional 3x3 RGBA kernel, mirrored image borders and GPU preview. Original Direct2D convolution, partial-alpha intermediates, border phase and profile equality remain uncertified. |
| Emboss | `EmbossGpuEffect`, `PdnEmbossEffect` | Angle 0..360, default 0; recovered directional grayscale 3x3 kernel, mirrored border, 128/255 offset, GPU preview/undo/repeat. Original Direct2D intermediates, alpha and profile comparison remain pending. |
| Morphology | `MorphologyGpuEffect`, Direct2D Morphology property mapping | Width/height 1..100, default 5, linked controls, dilate/erode, separable premultiplied-channel extrema and selection-preserving final write. Original kernel extent/centering, border, partial-alpha intermediate rounding and output-region behavior remain uncertified; the [Direct2D backend documentation](https://learn.microsoft.com/en-us/windows/win32/direct2d/morphology) does not substitute for original pixel fixtures. |

All forty effect entries support Repeat Last Effect (Ctrl+F) with their last accepted parameters, applying to the current active layer and selection. Cancellation does not replace the remembered effect. Repeating or accepting an effect creates one history transaction. Adjustments do not replace the last effect.

The exposure backend follows the [Direct2D exposure effect](https://learn.microsoft.com/en-us/windows/win32/direct2d/exposure-effect) used by the original. Relief follows the recovered kernel with the hard mirrored border described by [Direct2D convolution](https://learn.microsoft.com/en-us/windows/win32/direct2d/convolve-matrix). Emboss uses the original backend's [grayscale matrix](https://learn.microsoft.com/en-us/windows/win32/direct2d/grayscale-effect), with RGB weights 0.299, 0.587 and 0.114. These documents describe backend semantics; they do not replace original output fixtures.

All thirteen adjustments and forty effect entries are now active. See [native adjustment and float filter recovery](filter-reference.md) for algorithms, independent fixtures, GPU buffer handling, controls and explicit limitations. The effect roster is implemented. All four Gaussian Blur quality modes are implemented; full original-renderer certification remains unfinished.

## Independent tonal fixtures

`tools/tonal-probe` runs original `LevelsParameters.CreateLookupTable` and `FunctionInterpolator` managed IL. The fixture contains 4,608 RGB Levels outputs across 18 configurations (including degenerate input points and reversed output points) and 4,096 natural cubic spline samples across four curves, including extrapolation and narrow control-point spacing. CPU comparisons pass within 0.000001 per component. Hardware checks cover identity/configured transfer, luminosity, selection/alpha, Emboss flat fields and Morphology dilation/erosion, maximum sizes and selection preservation. These hardware equation/invariant checks do not establish original Direct2D output equality.

```sh
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/tonal-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-tonal.json
```

## Independent gradient fixtures

`tools/gradient-probe` executes original managed IL with Windows ReadyToRun disabled. It records 3,927 scalar geometry/repeater outputs (all seven geometries and three repeats on a 17x11 grid) and 4,096 `ColorBgra.Blend` outputs across 16 pairs of alpha endpoints and all 256 weights.

```sh
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/gradient-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-gradients.json
```

The explicit hardware test compares gradient geometry against those original outputs within one quantized weight and verifies all 4,096 color interpolations exactly. Computing linear coefficients in double precision on the CPU before uploading their float representations follows the original coefficient construction and avoids a GPU reciprocal-rounding difference at alpha thresholds. The shader also preserves the original solid/zero-length gradient bypass, including hidden RGB with zero alpha.

Additional tests cover clone opacity, overlapping source/destination without feedback, selection clipping, recolor channel shifts without repeated application, undo, large out-of-bounds pointer coordinates, neutral adjustments, brightness/contrast branches, hue rotations/desaturation, exposure equations, Vignette selection bounds, directional Relief kernels and mirrored edges, transparency gradients, reversal and invalid parameters.

GPU results for the new adjustments/effects are compared with recovered equations and focused invariants. They are not independently captured original Direct2D output. Original GPU/profile comparisons and native Windows/macOS validation remain required.

## UI verification

The additional content and tonal scopes exercise coalesced pointer press/motion, actual multiline text, font/style controls and live positioning, Shapes drawing/moving/resizing, finish/cancel/undo/redo, Levels/Curves point or gamma edits and transactional previews, Auto-Level, Emboss repeat and Morphology. The isolated native X11 exercise uses actual pointer and keyboard events to verify gradient drag/undo/Escape, clone source capture and undo, recolor and undo, adjustment/effect dialogs, Vignette rollback/commit/undo/redo and Ctrl+F repeat. Captures and logs stay in ignored `artifacts/ui-smoke/` and `reference/`. These checks do not establish exact reference screenshot parity, tablet behavior or native Windows/macOS input.

```sh
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=tools python3 tools/ui-smoke.py
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=content python3 tools/ui-smoke.py
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=tonal python3 tools/ui-smoke.py
```

## Drawing blend and hatch coverage

`ContentBlendMode` exposes 14 composition operators plus Overwrite. The editor
shares this setting across Brush, Pencil, Bucket, Clone Stamp, Text, Line/Curve,
Shapes and Gradient. Original `ColorBgra.Overwrite` matches 4,096 independent
managed cases, including zero alpha, full replacement and partial coverage.
Selection/glyph/stroke coverage remains separate from content alpha. GPU
gradients match all 15 choices at every selection byte for three source alphas.
Brush, Eraser, Clone Stamp and Recolor now accumulate sampled stamp coverage before applying source
color opacity. Circle/square tips, hardness, spacing, antialiasing, smoothing and
pressure settings are implemented. Flow is fixed at one in the original built-in
tips. Exact native brush coverage and physical tablet input certification remain
unfinished; see [brush recovery](brush-reference.md).

All 53 hatch choices are image-anchored 8x8 fills, with primary/secondary colors,
and work in Brush, Bucket, Line/Curve and Shapes. Their numerical coverage comes
from the native GDI+ API used by the original `HatchBrush`. The developer probe
uses the hash-checked 10.0.19041.329 DLL under Wine and substitutes a multiplier
of one for Wine's missing `GetDCDpiScaleValue`, specifically for the 96-DPI bitmap
fixture. The DLL and shim are not runtime dependencies. Monochrome tiles match
exactly. Alpha-weighted colored interpolation differs from native premultiplied
rounding by up to eight byte values in this corpus; it remains a parity gap.

Tests verify tile anchoring, negative coordinates, transparent colors, brush,
fill, line and shape output, clipping, stroke overlap and undo. New blend/fill
icons use hand-authored vector paths. Toolbar options scroll horizontally when
needed; this keeps controls reachable but is not the original overflow UI.

## Color Picker options

The six original sample sizes (1, 3, 5, 11, 31 and 51 pixels) use clipped document
bounds and premultiplied float32 accumulation followed by straight-alpha rounding.
The numeric routine matches all 256 original managed cases, including empty and
fully transparent samples. Single-pixel mode preserves hidden RGB as the original
`GetSample` fast path does. Sampling ignores selections and can use the active
layer or merged image; Ctrl temporarily enables merged sampling. Left/right drag
updates primary/secondary color continuously. On release the tool stays active,
returns to the previous tool, or switches to Pencil. All three behaviors and
sampling settings are persisted. Exact original cursor/sample-preview appearance
and native pen input remain uncertified.
