# Sampled brush recovery

Updated 2026-10-12. Reference: the supplied Paint.NET 5.1.12 x64 assemblies.
The Rust application does not link or execute their code.

`SampledCircleBrush`, `SampledSquareBrushRenderer`, `BasicSampledBrushRenderer`,
`SampledBrushRenderer`, `BrushStrokeSampleWetList`,
`SmoothedBrushInputPointList`, `BrushToolChangesBase` and `AppSettings.BrushSection`
establish the implemented settings and path formulas:

- Circle and square tips; size 1–2,000 pixels, hardness 0–100%, spacing 1–500%.
  At 96 DPI the reference starts with a 2-pixel circle, 75% hardness, 15% spacing,
  antialiasing, smoothing and pressure sensitivity enabled.
- The renderer's flow opacity is `flow³`, but both built-in tips and the brush tool
  fix flow at one. There is no additional flow control to replicate.
- Pressure scales diameter by `clamp(pressure × 127/128 + 1/128, 0, 1)`.
  Subpixel diameters retain at least one pixel of render size with squared area
  compensation in opacity when antialiasing is enabled.
- Antialiased spacing is `max(1, renderedDiameter × spacing)` along Euclidean
  distance. Aliased diameter and spacing round to half pixels with ties to even;
  aliased stamp positions round away from zero, and the path uses maximum axis
  distance. Spacing carries across input events; the endpoint is not forcibly
  stamped when it falls between samples.
- Smoothing uses centripetal Catmull-Rom, alpha 0.5, tension zero. Its distance
  calculation includes pressure as a third coordinate. End controls are linear
  extrapolations. Segment subdivisions use the ceiling of positional distance,
  and stored interpolation parameters use float32.
- A softened tip interpolates its hard diameter from `diameter/2 × 1.18875` to
  the full diameter. The black shape on white is converted to linear light,
  blurred with sigma `diameter × (1-hardness)/6`, converted to sRGB and inverted.
  Aliasing bypasses hardness. Minimum sprite render diameter is 32 pixels.

The developer probe exports 256 original diameter/flow/pressure plans, 256
original aliased bounds/spacing plans and 128 original smoothing evaluations.
The first two groups match exactly; smoothing agrees within 1e-10. These are
managed-method comparisons, not original native brush pixel fixtures.

The application prepares a portable tip once per stroke, accumulates stamp
coverage, then applies the selected content blend and fill once through the
selection mask. Thus overlapping stamps build mask coverage without repeatedly
applying the source color's alpha. Brush, Eraser, Clone Stamp and Recolor use fractional pointer coordinates and
the new path. Clone opacity multiplies stamp coverage before selection coverage.
Recolor clips its soft stroke through the tolerance stencil; exact native color
stencil feathering remains pending. Pencil keeps its one-pixel mask. All brush
options persist in workspace settings.

The tip rasterizer is CPU code using tiny-skia, a separable Gaussian and bilinear
sampling. Its convolution working scale follows the recovered native Quality
pre-scale bounds. Exact Direct2D geometry coverage, paired taps/post-scaling,
mip selection, MultisampleLinear reconstruction, large-sigma multiple passes and
native floating-point/8-bit conversion remain uncertified. This implementation
must not be described as native pixel parity or GPU brush rasterization; canvas
composition and presentation remain hardware accelerated.

Pressure is consumed when egui supplies a normalized Touch force. Missing force
uses full pressure. Physical tablet/pen delivery on each platform, original
input debouncing, screen-space preview, large-document brush performance and
native platform behavior still require work. The public path renderer clips
off-canvas constant-pressure segments before emitting stamps and bounds
smoothing subdivisions/variable-pressure work for extreme input coordinates.

The isolated X11 `brush-options` scope confirms functional width, hardness,
antialiasing, circle/square selection and spacing, option-only image isolation,
one-step undo and settings restoration after a clean restart. CPU tests also cover pressure, event-boundary spacing,
off-canvas input, fractional selections, Eraser, overlap and the 2,000-pixel tip.
Seven new icons are hand-drawn vector paths, bringing the atlas to 113 icons.
