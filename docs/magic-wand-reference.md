# Magic Wand review

Updated 2026-10-18. Reviewed PhotoCraft at commit
`5c89f16b60fb70195e874ff6d0ef1d3edba56e8a`, alongside the frozen Paint.NET
5.1.12 `FloodFillAlgorithm` and our existing original-execution distance fixtures.

[PhotoCraft's selection implementation](https://github.com/storytold/photocraft/blob/5c89f16b60fb70195e874ff6d0ef1d3edba56e8a/crates/algo/src/selection.rs#L158-L379)
uses seed-relative, four-connected matching, or a global match. Its 8-bit path
fills horizontal spans and enqueues neighboring runs. Fully transparent pixels
match regardless of hidden RGB. Optional antialiasing blends the binary mask
with a 3-by-3 neighborhood; its tolerance checks every RGBA channel separately.
The command samples either the active layer or the merged image and combines
the result with the existing selection. See the
[command implementation](https://github.com/storytold/photocraft/blob/5c89f16b60fb70195e874ff6d0ef1d3edba56e8a/crates/engine/src/selection_cmds.rs#L108-L143).

## Resulting changes

OpenPDN now traverses contiguous regions as horizontal spans, using the output
mask to mark completed spans. This removes the separate full-image visited
buffer and avoids a per-pixel frontier for large uniform regions. Active-layer
sampling shares its existing immutable pixel buffer instead of copying every
pixel; merged sampling still requires a composite. Both Magic Wand and Paint
Bucket use this path. Incomplete image rows and out-of-bounds seeds produce an
empty mask.

The color metric is unchanged: Paint.NET's normalized Euclidean RGBA distance,
with its premultiplied or straight-alpha option and original byte rounding.
Premultiplied mode already ignores hidden RGB when both pixels are completely
transparent. Straight mode deliberately preserves RGB differences. These rules
remain checked against 4,096 paired outputs from the original executable.

This review does **not** establish that PhotoCraft's metric is more accurate.
Per-channel and Euclidean thresholds select different sets of colors. Its edge
smoothing changes mask coverage without recovering the true object boundary.
Neither behavior was substituted for the frozen reference rules. No claim of
improved semantic/object selection or exact original edge coverage is made.

## Verification

The selection tests exercise all 512 binary 3-by-3 images from all nine seeds
against an independent pixel-at-a-time reachability oracle. This covers diagonal
contacts, thin barriers, holes, split spans and reconnection. An additional 64
mixed-color/alpha images cover both alpha modes, six tolerances and contiguous/
global matching. Explicit checks cover seed-relative ramps, exact tolerance
boundaries, transparent hidden RGB, single rows/columns, a 1,024-by-513 uniform
region, invalid seeds and incomplete image input. Existing tests retain layer
sampling, selection algebra, fill clipping and undo coverage.

The traversal is implemented locally without adding a dependency. PhotoCraft's
license is reproduced in [the accompanying notice](photocraft-notice.txt).
Original selection supersampling, retained Boolean geometry, live flood editing,
and platform/display certification remain separate parity gates.

The native UI deletion fixture explicitly uses an opaque, aliased brush stroke:
at zero tolerance, selecting one alpha value of a soft stroke must not be
expected to select all other edge alpha values. Transparent and partially
transparent matching is verified independently by the pixel tests.

The portable Linux release passes the native X11 editing scope, including
Magic Wand deletion/undo, lasso, layer visibility, History navigation, floating
panel and modal hit isolation, pixel movement, GPU preview/cancel/commit/undo
and an actual Xdnd drop of the original PDN fixture. The renderer identifies
NVIDIA GeForce RTX 5080 / Vulkan / DiscreteGpu. This is interaction evidence,
not original screenshot or native Wayland/platform certification.
