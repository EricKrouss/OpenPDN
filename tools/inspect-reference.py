#!/usr/bin/env python3
"""Record local reference provenance without copying the proprietary payload."""
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()

inputs = [ROOT / "paint.net.5.1.12.install.x64.zip",
          ROOT / "reference/installer/paint.net.5.1.12.install.x64.exe"]
inputs += sorted((ROOT / "reference/extracted/x64").glob("PaintDotNet*.dll"))
inputs += sorted((ROOT / "reference/extracted/x64").glob("paintdotnet*.dll"))
inputs += [ROOT / "reference/extracted/x64/License.txt"]
inputs += [ROOT / "reference/native-d2d/d2d1.dll", ROOT / "reference/native-d2d/d2d1.pdb", ROOT / "reference/native-gdiplus/gdiplus.dll", ROOT / "reference/native-gdiplus/gdiplus.pdb"]
fixtures = sorted((ROOT / "tests/fixtures").glob("paintnet-5.1.12-*")) + sorted((ROOT / "tests/fixtures/pdn").glob("*"))
fixtures += sorted((ROOT / "tests/fixtures").glob("native-*.json"))
assert all(p.is_file() for p in inputs), "Extract the supplied installer first"
assert len(fixtures) == 33 and all(p.is_file() for p in fixtures), "Generate blend, flood, posterize, gradient, tonal, filter, kernel, native analysis, spline, constraint, brush, selection, resolution, palette and PDN reference fixtures first"
manifest = {
    "baseline": "Paint.NET 5.1.12 x64",
    "reference_release_date": "2026-03-08",
    "inspection_date": "2026-10-12",
    "observed_setup_file_version": "5.112.9563.32325",
    "decompiler": "ILSpyCmd 11.1.0.9782",
    "oracle": "Original CompositionOps, FloodFillAlgorithm, PosterizeEffect2, GradientShapes/Repeaters/ColorBgra.Blend, LevelsParameters, FunctionInterpolator, BezierSplineUtil, TextTool navigation/deletion, brush plans/aliased stamps/centripetal smoothing, rectangle bounds, resolution conversions, palette parsing/defaults and integer RGB/HSV conversions, ColorPickerTool averaging, ColorBgra.Overwrite, GaussianBlurConvolutionEffect, PdnAddNoiseEffect lookup generation, HlslRandom, OctreeQuantizer, ProximityPaletteMap and PDN serialization managed IL executed using .NET 10.0.12 on Linux with ReadyToRun disabled; PDN and spline heap imports use the developer allocation shim",
    "opaque_channel_cases": 917504,
    "mixed_alpha_opacity_cases": 2688,
    "flood_color_pairs": 4096,
    "posterize_table_entries": 16128,
    "gradient_geometry_samples": 3927,
    "gradient_color_blends": 4096,
    "levels_rgb_samples": 4608,
    "curve_spline_samples": 4096,
    "gaussian_point_kernels": 7,
    "noise_lookup_entries": 16384,
    "pcg_random_cases": 4096,
    "native_temperature_gain_pairs": 621,
    "octree_unique_population_palettes": 48,
    "proximity_palette_queries": 24576,
    "line_natural_spline_cases": 64,
    "line_constraint_cases": 256,
    "inline_text_editing_cases": 512,
    "color_picker_average_cases": 256,
    "selection_rectangle_cases": 768,
    "measurement_conversion_cases": 117,
    "resize_constrainer_traces": 128,
    "resize_constrainer_states": 1408,
    "resolution_metadata_write_cases": 11,
    "palette_parser_cases": 25,
    "palette_file_encoding_cases": 8,
    "palette_library_padding_cases": 6,
    "default_palette_colors": 96,
    "integer_rgb_hsv_cases": 2048,
    "color_wheel_gesture_cases": 512,
    "color_wheel_pixel_cases": 512,
    "color_wheel_oracle_substitution": "Original GrabColor and wheel pixel IL cloned to DynamicMethod. Center/radius/snap field reads are replaced by constant providers; the original pure HSV pixel method is also cloned to avoid Windows UI static initialization. Numerical IL is retained.",
    "selection_oracle_substitution": "Original rectangle IL cloned to DynamicMethod with only Tool.get_Document replaced by a provider of an original Document, avoiding unrelated Windows UI static initialization. Numerical IL and original Document conversion calls are retained.",
    "brush_plan_cases": 256,
    "brush_aliased_stamp_cases": 256,
    "brush_smoothing_cases": 128,
    "drawing_overwrite_pixel_cases": 4096,
    "native_gaussian_optimization_kernel_cases": 45,
    "native_hatch_tiles": 53,
    "native_hatch_color_variants": 4,
    "native_hatch_oracle": "GDI+ 10.0.19041.329 under Wine; original HatchBrush bitmap API sequence at 96 DPI, with missing GetDCDpiScaleValue substituted by integer multiplier one. Monochrome coverage is exact; partially transparent color interpolation is not certified.",
    "bokeh_complete_kernels": 30,
    "sketch_sample_maps": 16,
    "native_turbulence_grids": 6,
    "native_red_eye_pixel_cases": 50,
    "native_red_eye_analysis_tables": 50,
    "native_red_eye_oracle": "Microsoft GDI+ 10.0.19041.329 executed through its native API under Wine; 50 original pixel results and traced polar analysis tables, including MXCSR round-down and retained area/strength accumulators. No GDI+ code is linked into the Rust runtime.",
    "native_direct2d_oracle": "Microsoft symbol server d2d1.dll 10.0.19041.329; CalculateGains emulated with Unicorn 2.1.4, bounded instruction count and host sqrtf; shader recovery details in filter-reference.md",
    "pdn_fixtures": "Original SaveToStream (14 blends and multichunk surfaces), plus original legacy gzip/inline serialization",
    "inputs": [{"path": str(p.relative_to(ROOT)), "bytes": p.stat().st_size, "sha256": digest(p)} for p in inputs],
    "fixtures": [{"path": str(p.relative_to(ROOT)), "bytes": p.stat().st_size, "sha256": digest(p)} for p in fixtures],
}
output = ROOT / "docs/reference-manifest.json"
output.write_text(json.dumps(manifest, indent=2) + "\n")
print(f"Recorded {len(inputs)} source hashes and {len(fixtures)} fixture hashes in {output}")
