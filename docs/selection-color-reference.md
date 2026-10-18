# Selection, resolution and color controls

Baseline: supplied Paint.NET 5.1.12 x64. This is implementation evidence, not
certification of complete tool, UI or native platform parity.

RectangleSelectTool's Normal, FixedRatio and FixedSize numerical IL produced
256 cases per mode. The developer probe clones that IL into DynamicMethod and
substitutes only Tool.get_Document with a provider of an original Document. This
avoids unrelated Windows UI static initialization; arithmetic, branches and
original document conversion calls remain unchanged. Rust bounds match all 768
results within 1e-10, including reverse drags, clipping and fractional sizes.
The final rectangle mask intersects the image and applies floor/ceil Int32Bound.

Normal rectangles use inclusive pixel endpoints. Shift constrains the smaller
clipped dimension. Fixed Ratio fits within those bounds while preserving the
drag's corner. Fixed Size treats the current cursor as its top-left corner and
clamps its position inside the document. Physical sizes use separate X/Y EXIF
resolution. EllipseSelectTool's Shift rule uses the drag as a circle's diameter.
SelectionTool supplies Ctrl-left Union, Alt-left Exclude, Ctrl-right Xor and
Alt-right Intersect. Pressing another mouse button during a selection drag moves
the trace origin until release. Replace clicks deselect; fixed-size selection can
emit without movement and clicking the same fixed rectangle deselects it.

The original Document produced 117 measurement conversion cases and the EXIF
SHORT/RATIONAL metadata for 144-by-120 DPI. Rust reads these opaque metadata
entries without executing serialized objects or changing unrelated metadata.
Physical selection and Text use that resolution; PDN round trips preserve it.
TextTool and AppSettingsToolsSectionExtensions determine point metrics from the
horizontal document resolution, or 96 DPI in Fixed96Dpi mode. The default is 12
points and the UI accepts 1–2,000. A render/layout test checks 144-DPI scaling and
pixel-identical fixed metrics. Resolved glyph sizes above 4,096 pixels are
currently rejected to bound portable glyph allocations.

PalettesService supplied all 96 default RGBA colors and 25 parser cases. Palette
lines use hexadecimal AARRGGBB, optional semicolon comments, and a maximum of 96
valid entries. Invalid lines are skipped. Six-digit input has alpha zero, and
variable-width unsigned hexadecimal and 0x prefixes follow the original parser.
File loading detects UTF-8/UTF-16/UTF-32 BOMs, accepts CR/LF/CRLF line endings, and uses replacement decoding for malformed sequences; eight original StreamReader cases pass. Saving writes UTF-8 with a BOM and uppercase eight-digit entries
through a temporary file and atomic replacement. Files above 1 MiB are rejected.
An empty imported palette remains empty, as in the reference parser. Named
library entries use the original EnsureValidPaletteSize behavior instead: truncate
to 96 colors and pad short inputs with white. Six original padding cases pass.
The palette menu discovers user and application entries, marks the first matching
entry, saves a named palette, opens its folder and resets the current swatches.
The save dialog lists existing names for replacement. Atomic writes reuse the
existing file for names differing only in case; exact non-ASCII invariant-culture
collation is not certified. The cross-platform user library uses the editor's
per-user application data folder. OPENPDN_PALETTES_PATH overrides it for
isolated validation. Application palettes are read from Palettes beside the
executable. The native X11 scope checks padding, named save/replacement, invalid
names, modal isolation, external file refresh and persistence across restart.

Int32RgbColor.ToHsv and Int32HsvColor.ToRgb supply 1,024 cases in each direction.
Rust matches every integer result, including truncation and hue 360. Hex edits
change RGB while preserving alpha. HSV edits retain their integer state rather
than drifting through repeated RGB conversion. ColorsForm confirms one-shot
swatch replacement and right-click assignment to the opposite selected color.
ColorWheelControl adds 512 original gesture results and 512 original wheel-pixel
results. The probe clones numerical IL and replaces only center/radius/snap field
reads with providers, plus the original pure HSV pixel call. Rust matches every
case. The procedural wheel uses the original 1.4 saturation power and one-pixel
alpha fringe. Pointer input preserves brightness, snaps hue with Shift, locks
hue with Alt, locks saturation with Ctrl and edits the opposite color with the
right button. Modifier transitions refresh their basis from the current color.
All 121 icons are independent hand-drawn vector paths; no image generation is
used. Six new drawings cover the color and selection controls.

Native Linux/X11 interaction checks exercise wheel brightness/modifier/right-button gestures, Shift squares, fixed ratios and
cursor-anchored fixed sizes, hex color entry, swatch replacement, opposite-color
right clicks, undo and clean settings restart. More/Less keeps the palette below
the wheel while expanding the numeric panel beside it. These checks do not
certify native Windows/macOS behavior.

Remaining gates include original four-times selection supersampling, retained
Boolean selection geometry, exact ellipse flattening, gesture-by-gesture
differentials, color-managed wheel presentation, exact dialog
dimensions, native font shaping/IME and platform/DPI validation. Legacy
NameValueCollection PDN metadata remains rejected: the supplied .NET 9 collection
serializer itself throws PlatformNotSupportedException, so the original fallback
cannot provide an executable oracle on the current runtime. Existing original
gzip/inline PDN fixtures remain covered.

## Image dimensions and resolution edits

New, Resize and Canvas Size now share physical width/height, pixel width/height,
resolution, inch/centimeter conversion and aspect controls. The model matches
128 original ResizeConstrainer traces (1,408 states). Resolution edits preserve
pixel dimensions; print-size edits change pixel dimensions using the current
resolution. Fractional dimensions remain intact until acceptance rounds pixels.
Percentage resizing uses the original size and preserves aspect-lock state.
Accepting unchanged Resize or Canvas Size settings preserves metadata, distinct
horizontal/vertical resolution and undo history, following both original actions.
The native X11 scope checks that both unchanged dialogs leave the next undo intact.

Document resolution writes match eleven original XML/EXIF values, including the
integer-or-thousandths RATIONAL encoding. Only Image-IFD resolution tags change;
other sections, metadata and profiles are preserved. DPI-only acceptance avoids
resampling and undo restores the old metadata. Native X11 checks cover 192-DPI
acceptance, a two-inch resize to 384 pixels, aspect preservation and both undo
steps. Original dialog styling, complete settings persistence and raster-format
resolution export remain open. Pixel-unit documents currently open the physical
controls at the default 96 DPI.
