# macOS universal2 builds

The native workflow creates `openpdn-0.1.0-dev-macos-universal2.zip` and its
`.zip.sha256` checksum. One `OpenPDN.app` contains baseline Intel `x86_64` and
Apple Silicon `arm64` slices. `arm64e` and `x86_64h` specializations are rejected.
Linux and Windows package names are unchanged. Thin macOS targets remain
available for local validation and troubleshooting.

The deployment target is **macOS 11.0 on both architectures**. CI sets
`MACOSX_DEPLOYMENT_TARGET=11.0` for Rust and native code, and packaging rejects
any slice whose Mach-O minimum OS exceeds that target. The application plist
records `LSMinimumSystemVersion` and the same minimum by architecture. This is
a build requirement, not evidence of a successful run on Big Sur. A working
Metal adapter with the features required by wgpu/OpenPDN is also necessary;
universal2 does not make every Intel Mac or GPU supported.

## Build and package on a Mac

Install Rust 1.95+ and Xcode Command Line Tools. Use the same checkout, lockfile,
toolchain and deployment target for both builds. Keep parallelism low. From the
repository root, using the default `target` directory:

```sh
rustup target add x86_64-apple-darwin aarch64-apple-darwin
export CARGO_BUILD_JOBS=2
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --locked --release --target x86_64-apple-darwin --bin openpdn
cargo build --locked --release --target aarch64-apple-darwin --bin openpdn
python3 tools/merge-macos-universal.py
python3 tools/package-native.py --target macos-universal2
```

The merge tool also accepts `--x86_64 PATH`, `--arm64 PATH` and `--output PATH`.
Pass `--binary PATH` to the packager when using a custom output directory. It
validates the two inputs separately, invokes Apple `lipo -create`, verifies
the merged architectures using both `lipo` and a Mach-O parser, then ad-hoc signs
and verifies the final executable. An unsuccessful merge/signing operation does
not replace an existing output. Input binaries are never modified.

Third-party native dependencies are currently linked statically into each
slice. **No third-party dylibs or frameworks are bundled.** The parser checks
all slices' load commands, including weak/re-exported dependencies and rpaths.
Only absolute Apple system paths under `/usr/lib/` or `/System/Library/` are
accepted. Homebrew paths, `@rpath`, `@loader_path` and `@executable_path` imports
fail packaging instead of producing a ZIP with missing or single-architecture
libraries. Unexpected native files in the documentation payload also fail.
Adding a dynamic dependency requires extending this policy to copy, merge,
relocate, sign and recursively validate every bundled library first.

ZIP entries preserve executable mode and use deterministic timestamps. `build.json`
records both architectures, per-slice minimum OS and dependencies, the final
executable hash and the absence of bundled libraries. The existing
`cross_compiled` field compares the package target to the packaging host: it is
always true for a universal2 target, even when each input was built natively.
It does not describe runtime testing. `native_platform_validation` remains false
for universal2; a single architecture's `--validation-result` is rejected.
The executable's ad-hoc signature supports local loading on Apple Silicon;
Developer ID application signing, Gatekeeper acceptance and notarization are
separate distribution tasks and are not performed by this workflow.

## CI checks and remaining runtime work

`.github/workflows/native.yml` uses explicit `macos-15-intel` and `macos-15`
runners, confirms their native CPU architectures, then runs formatting,
Clippy, the Python packaging tests and Rust CPU/reference tests on each.
Each runner builds its own release slice with two Cargo jobs. A dependent
macOS job merges those exact inputs, records Apple `vtool` deployment information
and `otool` dependencies, and uploads the universal ZIP and checksum under
`openpdn-macos-universal2`. Downloading raw slice artifacts loses executable
permissions; the merge restores them before signing.

Two further jobs download the **same final ZIP**, verify its checksum, extract
with `ditto`, check executable permissions, both architectures and the code
signature, then run `arch -x86_64 ... --help` on Intel and `arch -arm64 ... --help`
on Apple Silicon. Native CPU/reference tests and CLI startup exercise real
architecture-specific code when these jobs pass. They do not test Metal, GUI
startup, editing, clipboard, file dialogs or the macOS 11.0 baseline. Hosted
runner graphics are not used to certify hardware compatibility.

The explicit runner labels follow [GitHub's runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
Rust's deployment environment variable is documented in the
[Apple target requirements](https://doc.rust-lang.org/rustc/platform-support/apple-darwin.html).

For graphics and interactive validation, test this exact extracted universal
executable on both a physical Intel Mac and an Apple Silicon Mac. Record its
SHA-256, OS, native CPU and GPU identity; exercise `--gpu-info`, original-PDN
rendering, `--smoke-frames 10`, application launch, floating tools, clipboard,
open/save dialogs and representative edits/exports. Verify Metal is selected
and reject software adapters. Run the opt-in hardware suites from the README
on both native architectures as well. `tools/validate-macos.py` provides native
thin-build evidence; its previous Apple M2 Pro results do not certify the new
universal executable or Intel behavior. Test on macOS 11.0 separately before
claiming that minimum has been demonstrated at runtime.

On Linux/Windows, this command tests synthetic thin/FAT32/FAT64 headers,
architecture and minimum OS rejection, dependency policy, failure preservation,
archive paths/plists/metadata, permissions, checksums and determinism:

```sh
python3 -m unittest discover -s tools -p 'test_macos_*.py' -v
```

One additional test runs real Apple clang/lipo/codesign, packages and extracts
the C fixture with `ditto`, verifies its signature and executes its native slice
only on macOS. Linux fixture tests cannot establish that Apple tools
will accept an OpenPDN build or that either application slice will run.

The workflow has read-only repository permissions and uploads CI artifacts only.
It does not publish releases or comment on issues. After committing/pushing this
branch with authorization, its normal push/PR workflow will run. A manual rerun
can use `gh workflow run native.yml --ref codex/macos-universal2` after that ref
exists remotely. Release `0.1.0` remains ARM64-only until a separately authorized
release is published.
