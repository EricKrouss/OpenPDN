# Paint.NET document interoperability

Updated 2026-10-17. The native Rust editor now opens and atomically saves genuine `.pdn` files. Its runtime neither loads .NET nor instantiates types named by a file. `.opdn` remains a separate development container, now version 2 with metadata preservation and version 1 reading support.

## Implemented

- PDN3 magic, bounded 24-bit little-endian UTF-8 XML header, `00 01` serialization marker, inert NRBF classes/references and layer arrays.
- Deferred MemoryBlock surfaces: gzip or raw chunks, big-endian indices and lengths, out-of-order chunks, multiple chunks per layer, parent block bounds and cycle checks, and BGRA-to-RGBA conversion with hidden RGB preserved.
- Older whole-graph gzip serialization with inline byte/chunk blocks. The original serializer generated a fixture for this code path; this does not certify every historical Paint.NET release.
- Raster layers, ordering, Unicode names, visibility, opacity, all 14 blend modes and background flags.
- Opaque document/layer metadata pairs, including EXIF resolution and ICC payload strings; preservation of header custom content and saved-with version. Preserving profile bytes does not implement profile-aware display or processing.
- Size/depth/record/allocation limits; invalid dimensions, truncated or oversized chunks, duplicate/out-of-range indices, unknown layer types, unknown effective numeric blend modes and unsupported metadata layouts return errors. Atomic saves preserve the previous file if serialization fails.

## Independent verification

`tools/pdn-probe` executes the supplied 5.1.12 assemblies as an optional developer oracle. `small.pdn` has 14 differently configured layers, Unicode names, mixed alpha and 144×120 DPU. `chunks.pdn` has 257×257 surfaces crossing the 262144-byte chunk boundary. The original `SaveToStream` wrote both files; the JSON expectations contain every original pixel and layer attribute.

Rust reads those files, writes new PDNs, and reads them back. The original `Document.FromStream` also reads the Rust output. Its resulting JSON matches the original pixels, names, visibility, opacity, blend modes and resolution exactly. Run this check with:

```sh
python3 tools/verify-pdn.py
```

This requires the ignored extracted reference distribution, a C compiler and the developer .NET SDK. The allocation shim supports the original Windows heap imports on Linux; it is never bundled in the editor. The original legacy serialization path also generated `small-legacy.pdn`, covering gzip and inline memory blocks. Ordinary Rust tests read all three original fixtures without .NET.

## Linux continuation validation, 2026-10-17

The seven PDN integration tests now also exercise the application's file-open
dispatch with uppercase `.PDN` names, original resolution values and save/reload
preservation of layers, metadata and saved-with version. Copies of the original
fixture exercise numeric blend enums without legacy blend operations, unsupported
file versions, non-bitmap layers and unknown blend operations. Unknown effective
numeric modes now return an explicit error instead of silently becoming Normal.
This is deliberately stricter than the original reader's unknown-enum fallback;
all fourteen supported modes still pass the original fixture expectations.

The isolated X11/KWin `native-utilities` scope now loads all three authentic
fixtures while Tools, History, Layers and Colors are outside the editor. It
changes layer visibility through detached Layers, undoes the edit through its
focused keyboard shortcut and saves task-private copies. Truncated and PDN4
inputs display errors and leave the active document and displayed pixels intact.
The original files remain untouched and their hashes still match the manifest.
The original 5.1.12 reader independently verifies every pixel, layer attribute
and resolution value of all three UI-saved copies, including the legacy input.

The run used NVIDIA GeForce RTX 5080 / Vulkan / DiscreteGpu. The standard suite
passed 78 tests with 14 hardware tests left opt-in, and strict Clippy and the
debug build passed. Evidence is in `artifacts/resume-20261017/`. These are Linux
X11 and 5.1.12 serializer checks; they do not certify all historical releases,
Wayland gestures or profile-aware display.

## Open gates

The compatibility corpus still needs actual historical-version files, parent-backed surfaces, padded strides, broader metadata/profile samples and corrupted-file fuzzing. Legacy `NameValueCollection` metadata and non-bitmap layers are explicitly unsupported. Parent-block decoding has structural bounds checks but no original parent-backed fixture yet. The 512 MiB file/pixel-data, 16,777,216-pixel and 256-layer development limits still apply, together with the selected GPU's limits.

Validate save dialogs, settings, native Windows/macOS round trips and a wider file corpus before treating PDN interoperability as fully certified. Original-format support does not establish full application parity.
