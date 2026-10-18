# Float filters and native adjustment recovery

All 13 adjustment entries and all 40 built-in effect entries are implemented and enabled. The final fourteen are Bokeh, Sketch Blur, Quantize, Crystalize, Dents, Pixelate, Red Eye Removal, Sharpen, Straighten, Clouds, Julia Fractal, Mandelbrot Fractal, Turbulence and Edge Detect. Each has controls, preview/reset/cancel, selection clipping, undo and repeat. Exact renderer/color profile, full UI and native Windows/macOS certification remain open.

## Evidence and provenance

The supplied Paint.NET 5.1.12 assemblies determine property ranges, defaults,
alpha/color-space configuration, sample maps, RGSS offsets and graph topology.
Local ILSpy output remains in ignored `reference/decompiled/`. No original DLL,
shader bytecode, decompiled source or artwork is included in the package.

Temperature/Tint and Highlights/Shadows delegate their arithmetic to Windows Direct2D.
Microsoft's public symbol server supplied an independent native reference:

| Artifact | Identity |
| --- | --- |
| [d2d1.dll](https://msdl.microsoft.com/download/symbols/d2d1.dll/070EC20A5c0000/d2d1.dll) | Version 10.0.19041.329, SHA-256 `b1f99c4fd6cc6312f1991fea07363c71df09c47005e9eac488c7cafd399ba498` |
| [Public symbols](https://msdl.microsoft.com/download/symbols/d2d1.pdb/CE5B15E4A995E35A1A76B85AECF1770A1/d2d1.pdb) | SHA-256 `76b5c56b5b12922a47ee7c7605becadf618994ed0998e7d16f24f6f7e7c2d0af` |

These identify a Windows 10 implementation, not every Direct2D revision on
which Paint.NET 5.1.12 can run. The original Paint.NET assembly selects these
effects; equality with its rendered output on a particular Windows machine
still requires a native capture.

Public symbols locate `CTemperatureAndTint::CalculateGains`, `TempTintToYUV`,
`Yuv1960toXYZ` and `UpdateColorMatrix`. The Rust implementation interpolates the
31-row Robertson CIE 1960 locus, applies the recovered tint projection, computes
XYZ and RGB white points, and applies the native red/blue gain ratios. The green
gain and alpha remain unchanged. `tools/probe-native-temperature.py` emulates
the original x86-64 arithmetic with Unicorn, bounded to 20,000 instructions per
configuration, supplying only `sqrtf`. It produces 621 independent gain pairs;
the Rust port matches each within 0.00001. Unicorn and the DLL are developer
oracle inputs, not runtime dependencies.

Public symbols also locate the compressed Highlights/Shadows shaders. Windows
Compression API algorithm 2 stores MSZIP data here; removing its buffer and
block headers exposes the deflate stream. Wine's `D3DDisassemble` decodes the
recovered DXBC without loading the native DLL. The reconstructed graph blurs
premultiplied input, unpremultiplies, computes a gradient mask from that blur,
then applies the native highlight/shadow rational curves, clarity, YUV conversion
and gamut normalization. The reference requests minimum precision; this port
uses float32. Native Gaussian optimization, min16 rounding, profile conversion
and partially transparent original output remain uncertified.

The final effect recovery adds original complex Bokeh coefficients and 30 complete generated kernels, 16 original Sketch Blur sample maps, native Direct2D Turbulence permutation/gradient grids for six seeds, and recovered Sharpen/Edge Detect DXBC equations. Crystalize, Dents, Clouds and fractal maps follow the supplied managed/HLSL graph, including RGSS and original numeric permutations.

Red Eye uses the original Paint.NET area grouping/strength wrapper and a separately obtained [Microsoft GDI+ RedEyeCorrection](https://learn.microsoft.com/en-us/windows/win32/api/gdipluseffects/nl-gdipluseffects-redeyecorrection) reference. The native DLL is version 10.0.19041.329, SHA-256 `7765be8d676c17aeca32086ba8e33b750fbca0fcc1e85067b5202217c08f0462`. Public symbols identify the center, polar statistics and correction functions. A developer probe executes the native API under Wine and traces numerical analysis values. Its 50 fixtures cover two backgrounds, five pupil sizes and all five strengths. Analysis reproduces native round-down arithmetic portably and retains the original state between areas/strength passes. GPU pixels match all 50 cases within one byte. This validates that GDI+ version and corpus, not every OS version or photograph.

## GPU processing

Float32 RGBA buffers stay on the GPU across processing passes, premultiplied
unless the recovered effect requires independent straight channels.
Only the final output is quantized to RGBA8, and the original pixels outside
the selection are restored exactly. Neighborhood effects sample the complete
source layer. Selection bounds determine the centers and sizes where the
Paint.NET wrapper does so; Radial/Zoom Blur use document coordinates.

| Effect | Recovered behavior and current limits |
| --- | --- |
| Bokeh | Original complex separable convolution, gamma boost and 1..10 component quality; 30 original kernels match within 0.000001, with independent direct 2D GPU output checks. |
| Sketch Blur | Original seeded System.Random disk sample maps and P-squared percentile marker estimator, 1..20 smoothing iterations. Original sample maps match; percentile quality/endpoints and alpha are exercised. Native pixel certification remains pending. |
| Quantize | Octree and portable Median Cut palettes, 2..256 colors, 0..8 error diffusion and alpha threshold. Mapping and Floyd-Steinberg diffusion run on the GPU in dependency-ordered diagonals; CPU builds the small palette. Original opaque histogram, complete-level Octree reduction, float conversions, proximity-list introsort/tie ordering, binary-alpha lossless palettes and transparent-pixel conversion are recovered. Forty-eight unique-population Octree palettes and 24,576 proximity queries match original IL. Exact WIC Median Cut and original process-randomized Octree equal-population hash ordering remain parity gaps. |
| Crystalize | Original numeric feature permutations, nearest-cell search, linear-light sampling and RGSS quality 1..5; user/instance seeds are retained across preview/reset/repeat. Native shader rounding remains uncertified. |
| Dents | Recovered improved Perlin displacement, selection-based scale, refraction/detail/turbulence/angle and mirrored RGSS quality. Native sampling/precision remains uncertified. |
| Pixelate | Selection anchor, cached downscaled image and separate upscale pass; all six downsampling and three upsampling choices execute. Cubic uses Catmull-Rom; footprint integration is capped at 64x64 samples. Anisotropic currently uses the same integrated linear footprint as Multisample Linear. Exact native mipmap/anisotropic and High Quality Cubic optimized kernels remain parity gaps. |
| Red Eye Removal | Small CPU center/polar analysis per selection component followed by GPU correction, repeated 1..5 times. Includes native area/strength state and final mask restoration. Fifty native analysis/output fixtures pass. Overlapping component bounding boxes, partial alpha and a broad photographic corpus need native comparison. |
| Sharpen | Recovered native 1.033 sampling offset, nine-tap coefficients, luma detail and threshold suppression; unassociated RGB sampling and original alpha. Native min16 and border handling remain uncertified. |
| Straighten | Auto scale to maintain image dimensions, -45..45 angle, Cubic/Linear/Nearest sampling in linear light. Exact native cubic kernel and boundary rounding remain uncertified. |
| Clouds | Original improved Perlin octaves/seed/scale/roughness, two RGBA colors and all 46 render mix choices. Uses current primary/secondary colors when opened; Reset and repeat retain those colors. Original color tabs/layout and native pixels remain uncertified. |
| Julia and Mandelbrot Fractal | Recovered escape iterations, factor/zoom/angle, RGSS quality and 46 render mix choices; Mandelbrot inversion. Native precision/color-profile certification remains pending. |
| Turbulence | Original native Park-Miller table generation and permutation, four gradient channels, octave/frequency/stitch behavior, noise/fractal choice, tile size/seed and 46 mix choices. Six complete native grids match within 0.0000001. GPU tile-repeat checks pass; native pixels remain uncertified. |
| Edge Detect | Recovered Sobel/Prewitt squared gradients, strength, Gaussian preblur and subtractive Overlay Edges with original alpha. Native min16, Gaussian optimization and partial-alpha certification remain pending. |
| Square Blur | Fractional-radius separable box weights, mirrored borders, linear-light processing and gamma boost; radius 0..300. |
| Gaussian Blur | Original High Quality convolution: radius/3 sigma, six-sigma support, 1/8-pixel Gaussian samples aggregated into point weights. Seven complete kernels match original managed IL fixtures, including sigma 100. Qualities 1..3 now use the recovered native optimization plans, symmetric paired taps, pre/post transforms and interpolation choices. Forty-five native plans/kernels match; an independent double-precision reconstruction agrees with hardware output within one byte. Exact Direct2D pixels/profile conversion remain pending. |
| Fragment | Rotated fragment offsets, bilinear premultiplied samples and normalization by valid geometric coverage, including partially covered edges. |
| Motion Blur | Centered or one-sided Gaussian line, 1/8-pixel samples aggregated into a sparse point kernel, named clamp/wrap/mirror/transparent borders. Native resource-texture out-of-range behavior and intermediate precision still need output fixtures. |
| Radial Blur | Angular sampling with `floor(2^(quality+5))` samples and coverage normalization. |
| Zoom Blur | Weighted scaling samples, focus exponent, clamp border, document center and fractional quality. |
| Bulge, Twist, Polar Inversion, Tile Reflection | Recovered sample maps, bilinear sampling, original rotated grid supersampling (quality squared), selection/document coordinate mapping and applicable edge choices. The polar singularity uses the center pixel; original undefined center/NaN behavior remains uncertified. |
| Glow | Recovered Gaussian/brightness/contrast/Screen graph, with premultiplied alpha composition. Native Gaussian Quality optimization and profile differences remain pending. |
| Pencil Sketch | Recovered blur/brightness/contrast/inversion/grayscale/Color Dodge graph. Native Gaussian optimization, exact dodge corner cases and original intermediate precision remain pending. |
| Ink Sketch | Recovered 5x5 convolution, preserved alpha, grayscale threshold and Darken blend over a fixed-radius Glow graph. Uniform-field Screen/Darken equations are checked; native optimized Gaussian/intermediate rounding remains pending. |
| Surface Blur | Recovered circular sample footprint, mirrored border, straight RGBA and per-channel `max(0, 1 - abs(sample-center) * 0.96 / threshold)` weights. Independent double-precision neighborhood checks cover three radii/thresholds and transparency. |
| Soften Portrait | Recovered softness-to-Gaussian scale, lighting/contrast, grayscale red/blue warmth and Overlay graph. Uniform fields isolate the blend/temperature equations; native Gaussian/profile output remains pending. |
| Drop Shadow | Linear sRGB processing, Gaussian alpha with transparent borders, fractional angle/distance translation, sRGB color control, opacity, Shadow Only and Source Over. Axis translation before point convolution preserves out-of-source blur support. Independent direct 2D Gaussian integration checks colored and black shadows, fractional movement, edge re-entry and composition. Native Quality optimization remains pending. |
| Median | Circular clamp samples and independent straight RGBA percentiles. Quality 1..9 controls finite interval search; Auto range uses circular extrema through quality 6 and square extrema above it. Bounds use half-float floor/ceiling. Percentile 0/100 returns exact extrema. Even quality uses the recovered four-way search; odd quality uses binary search. Independent sorted order-statistic checks cover both paths and transparent hidden RGB. |
| Outline | Two Median percentiles `(1-intensity)/2` and `(1+intensity)/2`, `1-(high-low)` RGB and high-percentile alpha. The original accepts thickness 1..70 but internally clamps the radius to 50; both limits are retained. |
| Oil Painting | Linear sRGB, clamp borders, circle/square kernels and 4..256 intensity bins. Lowest intensity wins ties. Alpha-zero samples still count in the histogram; the chosen bin averages premultiplied RGBA. Independent double-precision histogram checks cover both shapes, alpha and bin limits. |
| Reduce Noise | Mirrored circular neighborhood, strict per-channel sample-below-center population and the recovered negative strength factor `-0.2*strength*(1-0.75*intensity)`. Straight RGB and original alpha, including hidden RGB. Radius/strength zero preserves source bytes. |
| Add Noise | Original 16,384-entry inverse normal table, PCG coordinate hashing, coverage, fixed-point saturation/deviation, straight RGB and preserved alpha. All table entries and 4,096 hash/next-float cases match original managed IL. GPU pixels match an independent scalar implementation across saturation, coverage and full 32-bit instance seeds. |
| Frosted Glass | Linear sRGB premultiplied bilinear sampling with mirror borders, recovered exponent distribution and independent sample maps. Smoothness 1..8 selects 1/2/3/5/8/13/21/34 maps, without RGSS offsets. Minimum/maximum radii are linked. Original per-map seeds come from unexposed `Random.Shared.Next()` calls; this port regenerates valid 31-bit map seeds with PCG from a saved stream seed. This deliberate seed-source difference retains stable previews and repeat, but does not certify the original random sequence or native pixels. |

Neutral radius/strength/scale configurations preserve all source bytes,
including hidden RGB at alpha zero. General filter output uses premultiplied
sampling. Large float buffers are checked against the device's storage limit;
tiled storage and asynchronous processing remain pending. Pipeline specialization is cached lazily by operation so the driver compiles the relevant filter instead of the complete branch tree. Palette construction and Red Eye analysis are CPU work; correction/mapping and image filters use compute shaders. Red Eye strength passes currently read back between analyses.

## Reproduce validation

```sh
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/filter-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-gaussian.json
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/filter-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-noise.json noise
reference/probe-python/bin/python tools/probe-native-temperature.py
reference/probe-python/bin/python tools/probe-native-turbulence.py
python3 tools/probe-native-redeye.py
# Original managed final-effect kernels/tables:
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/filter-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-bokeh.json bokeh
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/filter-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-sketch.json sketch
COMPlus_ReadyToRun=0 DOTNET_ReadyToRun=0 dotnet run --project tools/filter-probe -- reference/extracted/x64 tests/fixtures/paintnet-5.1.12-effect-tables.json tables
cargo test --locked
cargo test --locked --test filters -- --ignored --nocapture
cargo test --locked --test neighborhood -- --ignored --nocapture
cargo test --locked --test remaining_effects -- --ignored --nocapture
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=filters python3 tools/ui-smoke.py
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=composites python3 tools/ui-smoke.py
xvfb-run -a -s '-screen 0 1280x900x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=neighborhoods python3 tools/ui-smoke.py
```

The explicit filter test requires a hardware adapter and compares Square Blur
against an independent direct 2D double-precision convolution. It checks native
gain fixtures, selection writes, source sampling outside selections, neutral
byte preservation, parameter rejection, single-pixel borders, extreme controls
and alpha behavior. These checks establish the exercised equations and
invariants on Linux/Vulkan, not complete original GPU or platform parity.

The UI adds center thumbnails with draggable markers, named edge choices,
Motion Blur's Centered checkbox, transactional previews and eight-parameter
repeat storage. Layout, original angle/vector widgets, original optimization
choices, localization, DPI and native accessibility still need parity work.

The isolated Linux/X11 scope passed edit/accept/undo/cancel for both adjustments
and preview/cancel/modal isolation/accept/undo/repeat for all twelve new effects.
Zoom Blur uses nondefault fifth-parameter and center values. Full-row menu hit
regions were verified after correcting short-label clicks outside the old text
button bounds.

The four composite/neighborhood dialogs passed the isolated native UI scope
with preview/cancel/modal isolation/accept/undo/repeat. Drop Shadow uses a
nondefault color and Shadow Only, verifying fifth/sixth parameter retention.
Drop Shadow exposes a color picker and Shadow Only checkbox. Original color
wheels, angle choosers and exact dialog arrangement remain pending. Convolution
alpha and clamp behavior follows the original graph and [Direct2D semantics](https://learn.microsoft.com/en-us/windows/win32/direct2d/convolve-matrix).
The original `IGpuEffectRenderInfo.ColorContext` defaults to WorkingSpaceLinear;
Drop Shadow retains this default, so its source/composition runs in linear sRGB
for the current sRGB document path. Other document color profiles remain open.

The neighborhood scope exercises all six additional dialogs, including a
nondefault square Oil Painting kernel, Randomize/Reset and repeat seed retention,
and mutually bounded Frosted Glass scatter radii. The independent hardware test
covers quality/endpoints, histogram shape/ties, mirror/clamp borders, transparent
RGB, selection reads/writes, neutral cases and extreme/invalid controls.

The remaining native UI scope checks all fourteen final dialogs, including a reduced Quantize palette and nonzero Straighten angle. Preview/cancel/modal isolation/accept/undo/repeat passed for every entry. No-op acceptance adds no history entry. The composite scope also passed again after shared dialog/pipeline changes.

```sh
xvfb-run -a -s '-screen 0 1280x850x24' env WAYLAND_DISPLAY= OPENPDN_ISOLATED_UI=1 OPENPDN_UI_SCOPE=remaining python3 tools/ui-smoke.py
```

## Additional Quantize recovery

The Octree now merges complete levels in child traversal order before reducing the last level by population. Forty-eight original palettes use distinct powers-of-two populations to isolate this behavior from process-randomized hash ties. Color averages follow the original f32 transfer, f64 accumulation, f32 average and byte rounding sequence. Proximity mapping uses the original introsort tie permutation for all 4,096 RGB cubes; 24,576 original queries match, including nearest-distance ties. The packed ordering table is uploaded with the palette; mapping and diffusion remain GPU compute. Binary-alpha lossless palette selection and transparent-pixel conversion before error diffusion also follow recovered managed code. Median Cut still uses a portable approximation to WIC. Equal-population Octree ordering is intentionally stable across repeat/platforms; the original uses randomized System.HashCode and may differ between processes.

## Native optimized Gaussian modes

`tools/probe-native-blur.py` executes the separately obtained Direct2D binary's
`CalculateOptimizationTransforms` and `GenerateKernelTaps` using bounded Unicorn
emulation. The native CRT ceil/pow/exp imports use host arithmetic rounded to
float32. Forty-five cases cover Speed, Balanced and Quality, threshold boundaries,
small sigma and sigma 100. Pre-scale is clamped to 0.1..1; post-scale and paired
Gaussian taps follow the recovered mode table. DXBC recovery establishes the
half-pixel sampling centers. No native code is linked into the editor.

A dedicated wgpu pipeline retains an extended source halo through the separable
passes. Speed reconstructs bilinearly; Balanced/Quality use portable Catmull-Rom
cubic reconstruction. This cubic kernel, native minimum precision and profile
conversion still need original Direct2D pixel certification. The independent
scalar oracle validates this reconstructed graph, not the Windows renderer.
The RTX 5080 checks cover transparent edges, gamma boost, selection restoration,
all four qualities, radius boundaries and distinct optimized outputs. The native
X11 `gaussian` scope checks every quality through preview, cancel, modal input
isolation, acceptance, undo and exact repeat.
