# Native backend validation

The requested completion target still includes full tool, effect and UI parity.
Passing backend checks establishes the exercised paths, not the entire product.

## OpenPDN ZIP validation, 2026-10-18

The renamed OpenPDN build passes 88 standard tests and all 14 explicit Metal
hardware tests in both debug and release on Apple M2 Pro, plus native floating
utilities and application startup. The binary SHA-256 is
`004d7397e3b4df85c89a5ad478e45879336296d2e5d98a647e3a9229a82aa419`.
Evidence is in `metal-validation-20261018T171157Z.tar.gz`. Final packaging also
extracts the macOS ZIP with ditto, verifies the executable mode, name, identifier
and hash, and launches the extracted OpenPDN.app with native Metal. The packaged
validation record includes these ZIP checks. All three platforms now use ZIP
bundles; Windows checks remain Wine/D3D12 rather than native Windows certification.


## Earlier bundle validation, 2026-10-18

The current Apple Silicon bundle was built on the user's Apple M2 Pro and
validated with native Metal. `metal-validation-20261018T165203Z.tar.gz` records
20 successful checks, including 88 standard tests, all 14 explicit hardware
tests in both debug and optimized release, original-PDN rendering, native window
startup, all four floating utility viewports and LaunchServices `.app` startup.
The executable SHA-256 is
`c50bf14151e7f567aa17b4ee9c3afa812c1bab387a91ba450f357540d7329f48`.
This supersedes the earlier connection-unavailable note in the status file.

README and validation documentation are refreshed after the checks. The Rust
source, tests, examples and lockfile match the validated source bundle exactly;
repackaging verifies the unchanged executable against its successful native
validation record. Package metadata contains that record's hash and retains
`feature_complete: false`. The installed application is not replaced by this
bundle-generation run. The older run below is retained as historical evidence.

## macOS / Metal

`python3 tools/package-validation-source.py` creates an allowlisted source ZIP
with SHA-256 hashes of every source/fixture file. It excludes the supplied
installer, extracted binaries, decompilation, developer .NET probes, credentials
and the .NET runtime. The bundle can build independently on an actual Mac.

Run `python3 tools/validate-macos.py` on macOS with Rust 1.95 or later, rustfmt,
Clippy and Xcode Command Line Tools. It verifies bundle hashes, records OS/SDK/
compiler/display information, checks formatting and strict Clippy, runs standard
tests, builds a release executable and forces `WGPU_BACKEND=metal`. A CPU adapter
or a different backend fails validation. The explicit GPU tests, original-PDN
rendering and a ten-frame native window smoke run follow. It also creates and
launches the native `.app` package. Smoke-run preferences use an isolated path.
`--skip-ui` records the omission and makes no window/presentation claim.

Logs, the rendered PNG and `result.json` are archived in `artifacts/metal-validation-*`.
These checks do not certify native IME, accessibility, tablet input, profiles,
HDR, monitor changes or matching reference screenshots. A successful `.app`
launch is not notarization or public-distribution certification.

The first Apple M2 Pro / Metal run exposed two real cross-platform defects:
the operating system's `atan2` rounding selected a different line constraint at
a 7.5-degree boundary, and Metal's undefined `atan2(0,0)` produced the wrong
conical-gradient center. Line snapping now uses pinned portable `libm` math;
the gradient shader defines the zero-vector angle explicitly. All 256 original
line fixtures pass on the Mac with the portable calculation. The fresh native run passes 75 standard tests and all fourteen explicit Metal GPU
tests, release startup, original-PDN rendering, a ten-frame window smoke check
and LaunchServices `.app` startup. Hardware identity is Apple M2 Pro / Metal /
IntegratedGpu. OS 27.2 (26B5091g), SDK 27.0 and Rust 1.96.0 are recorded in the
evidence archive. All fourteen optimized release GPU tests also pass. Tools,
History, Layers and Colors are created as four
native viewports; the floating-window smoke check rejects an embedded fallback.
The exact executable and source-manifest SHA-256 values are recorded in the
evidence and the native package's `build.json`.

The source-overlay cache regression check simulates an interrupted validation:
new source passes a debug check, the same deterministic ZIP is extracted again,
and the release profile must compile the new source. Source hashes and original
verified timestamps are tracked together so an interrupted run cannot silently
reuse an older release executable.
An additional simulated-command check ensures that a failed final packaging step
reports failure and returns a nonzero exit status, even after the native-checks
snapshot was saved.

Native X11 interaction checks with a task-private KWin manager on debug and
portable release builds drag all four
tool windows beyond the main editor, exercise their controls and modal isolation,
close/reopen them, restore their saved desktop positions after restart, and dock
them again. They verify drag endpoints and that Escape still cancels the modal
after clicking a disabled native utility. Mac and Wine checks currently establish native window creation and
startup; native dragging, monitor transitions and exact window chrome on those
platforms remain unverified.

## Windows D3D12 through Wine

The user requested Wine for D3D12 validation. Fourteen explicit hardware tests
pass with the Windows GNU target on RTX 5080 / Dx12 / DiscreteGpu. This exercises
wgpu's D3D12 path through VKD3D-Proton's Vulkan translation; it does not establish
native Windows driver behavior. Earlier Wine runs selected wgpu Vulkan directly
and therefore did not exercise D3D12.

The task-private prefix uses these independently downloaded developer tools:

| Component | Release | Archive SHA-256 |
| --- | --- | --- |
| [Microsoft DXC](https://github.com/microsoft/DirectXShaderCompiler/releases/tag/v1.8.2505.1) | 1.8.2505.1 | `9ad895a6b039e3a8f8c22a1009f866800b840a74b50db9218d13319e215ea8a4` |
| [VKD3D-Proton](https://github.com/HansKristian-Work/vkd3d-proton/releases/tag/v3.0.1) | 3.0.1 | `3cf2315522af5e43605ef6d3c41dad91387040bf97199934f3f7ab76caaa2f0c` |
| [DXVK DXGI](https://github.com/doitsujin/dxvk/releases/tag/v3.1.1) | 3.1.1 | `40565b4a724aadc4433fa4e010b4b23916d9b1f1baeee64e17186db94f54e608` |

Wine's built-in FXC rejected `RWByteAddressBuffer.Load4`; forcing DXC resolved
that compiler limitation. DXGI's GPU identity override is disabled for reporting.
These DLLs live only in the ignored task-private validation prefix and are not
copied into the editor or source bundles. The validation environment sets
`WGPU_BACKEND=dx12`, `WGPU_DX12_COMPILER=dxc`,
`WINEDLLOVERRIDES=d3d12,d3d12core,dxgi=n`, and
`DXVK_CONFIG=dxgi.hideNvidiaGpu = False`. GPU tracing can be enabled with
`OPENPDN_GPU_TRACE=1` to locate startup pipeline failures.

An initial packaged-release startup crashed inside VKD3D-Proton's D3D12 core.
The debug hardware suite and fourteen release-profile hardware tests pass with
`WGPU_DEBUG=1` (shader optimization disabled). Packaged release startup also passes
with that flag. The default optimized shader path through VKD3D-Proton 3.0.1 still
crashes inside its translation library and remains unresolved. No global Wine
prefix, Steam/Proton configuration or unrelated running game is changed.
