"""Synthetic Mach-O regression tests; these do not establish runtime support."""
import importlib.util
from pathlib import Path
import platform
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

from macos_binary import inspect_macho, validate_macos_binary, UNIVERSAL_ARCHITECTURES


def string_command(cmd, path):
    fixed = 24 if cmd in (0xC, 0x80000018, 0x8000001F, 0x80000023, 0x20) else 12
    payload = path.encode() + b"\0"
    size = (fixed + len(payload) + 7) // 8 * 8
    return struct.pack("<3I", cmd, size, fixed) + bytes(fixed - 12) + payload + bytes(size - fixed - len(payload))


def thin(arch="x86_64", minimum=0xB0000, legacy=False, dependencies=("/usr/lib/libSystem.B.dylib",), rpaths=(), platform_id=1, kind=2):
    cpu, subtype = (0x01000007, 3) if arch == "x86_64" else (0x0100000C, 0)
    commands = [struct.pack("<4I", 0x24, 16, minimum, 0xF0000) if legacy else
                struct.pack("<6I", 0x32, 24, platform_id, minimum, 0xF0000, 0)]
    commands.extend(string_command(0xC, path) for path in dependencies)
    commands.extend(string_command(0x8000001C, path) for path in rpaths)
    payload = b"".join(commands)
    return struct.pack("<8I", 0xFEEDFACF, cpu, subtype, kind, len(commands), len(payload), 0, 0) + payload


def fat(slices=None, wide=False, endian=">"):
    if slices is None:
        slices = [thin("x86_64"), thin("arm64")]
    offset = 8 + len(slices) * (32 if wide else 20)
    table, payload = [], []
    for data in slices:
        cpu, subtype = struct.unpack_from("<2I", data, 4)
        table.append(struct.pack(endian + ("IIQQII" if wide else "IIIII"), cpu, subtype, offset, len(data), 0, *([0] if wide else [])))
        payload.append(data)
        offset += len(data)
    return struct.pack(endian + "2I", 0xCAFEBABF if wide else 0xCAFEBABE, len(slices)) + b"".join(table + payload)


spec = importlib.util.spec_from_file_location("merge_macos", Path(__file__).with_name("merge-macos-universal.py"))
merge_macos = importlib.util.module_from_spec(spec)
spec.loader.exec_module(merge_macos)


class MachOTests(unittest.TestCase):
    def test_thin_targets_and_legacy_deployment_command(self):
        for arch in UNIVERSAL_ARCHITECTURES:
            for legacy in (False, True):
                info = validate_macos_binary(thin(arch, legacy=legacy), {arch})[arch]
                self.assertEqual(info["minimum_macos"], "11.0")
                self.assertEqual(info["dependencies"], ["/usr/lib/libSystem.B.dylib"])

    def test_fat32_fat64_and_both_byte_orders(self):
        for wide in (False, True):
            for endian in (">", "<"):
                self.assertEqual(set(validate_macos_binary(fat(wide=wide, endian=endian), UNIVERSAL_ARCHITECTURES)), UNIVERSAL_ARCHITECTURES)

    def test_system_frameworks_and_weak_dependencies(self):
        for cmd in (0xC, 0x80000018, 0x8000001F, 0x80000023, 0x20, 0xE):
            data = bytearray(thin(dependencies=()))
            command = string_command(cmd, "/System/Library/Frameworks/Metal.framework/Versions/A/Metal")
            struct.pack_into("<2I", data, 16, 2, len(data) - 32 + len(command))
            info = validate_macos_binary(bytes(data) + command, {"x86_64"})
            self.assertEqual(len(info["x86_64"]["dependencies"]), 1)

    def test_wrong_or_missing_architecture(self):
        for data, expected in ((thin(), UNIVERSAL_ARCHITECTURES), (fat(), {"arm64"}), (thin(), {"arm64"})):
            with self.assertRaisesRegex(ValueError, "architectures"):
                validate_macos_binary(data, expected)

    def test_non_executable_and_cpu_specializations(self):
        with self.assertRaisesRegex(ValueError, "executable"):
            validate_macos_binary(thin(kind=6), {"x86_64"})
        for arch, subtype in (("x86_64", 8), ("arm64", 2)):
            data = bytearray(thin(arch))
            struct.pack_into("<I", data, 8, subtype)
            with self.assertRaisesRegex(ValueError, "CPU/subtype"):
                inspect_macho(data)

    def test_deployment_target_in_every_slice(self):
        for slices in ([thin(minimum=0xC0000), thin("arm64")], [thin(), thin("arm64", minimum=0xB0001)]):
            with self.assertRaisesRegex(ValueError, "above 11.0"):
                validate_macos_binary(fat(slices), UNIVERSAL_ARCHITECTURES)
        with self.assertRaisesRegex(ValueError, "at least 11.0"):
            validate_macos_binary(thin("arm64", minimum=0xA0F00), {"arm64"})
        self.assertEqual(validate_macos_binary(thin(minimum=0xA0F00), {"x86_64"})["x86_64"]["minimum_macos"], "10.15")

    def test_non_macos_and_missing_deployment_target(self):
        for data in (thin(platform_id=2), thin(minimum=0), struct.pack("<8I", 0xFEEDFACF, 0x01000007, 3, 2, 0, 0, 0, 0)):
            with self.assertRaises(ValueError):
                inspect_macho(data)

    def test_non_system_dependency_or_rpath_in_either_slice(self):
        for path in ("@rpath/libfoo.dylib", "@loader_path/libfoo.dylib", "@executable_path/../Frameworks/Foo", "/opt/homebrew/lib/libfoo.dylib", "/usr/local/lib/libfoo.dylib", "/tmp/libfoo.dylib", "/usr/lib/../../tmp/foo"):
            for architecture in ("x86_64", "arm64"):
                for field in ("dependencies", "rpaths"):
                    slices = [thin(arch, **({field: (path,)} if arch == architecture else {})) for arch in ("x86_64", "arm64")]
                    with self.assertRaisesRegex(ValueError, "non-system dependency/rpath"):
                        validate_macos_binary(fat(slices), UNIVERSAL_ARCHITECTURES)

    def test_fat_header_mismatch_duplicate_overlap_and_bounds(self):
        cases = [fat([thin(), thin()])]
        for offset, value in ((8, 0x0100000C), (16, 8), (20, 0xFFFFFF), (24, 31), (36, 48)):
            data = bytearray(fat())
            struct.pack_into(">I", data, offset, value)
            cases.append(data)
        for data in cases:
            with self.assertRaises(ValueError):
                inspect_macho(data)

    def test_truncation_and_invalid_load_commands(self):
        for length in (0, 4, 7, 31, 34, len(thin()) - 1):
            with self.assertRaises(ValueError):
                inspect_macho(thin()[:length])
        for length in (4, 7, 20, len(fat()) - 1):
            with self.assertRaises(ValueError):
                inspect_macho(fat()[:length])
        for offset, value in ((16, 999), (20, 0xFFFFFF), (36, 7), (36, 0xFFFFFF), (64, 4)):
            data = bytearray(thin())
            struct.pack_into("<I", data, offset, value)
            with self.assertRaises(ValueError):
                inspect_macho(data)


class MergeTests(unittest.TestCase):
    def test_rejects_input_overwrite_before_invoking_tools(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(merge_macos.platform, "system", return_value="Darwin"):
            path = Path(temporary) / "openpdn"
            with self.assertRaisesRegex(ValueError, "overwrite"):
                merge_macos.merge(path, path, path)

    def test_bad_input_never_reaches_lipo(self):
        with tempfile.TemporaryDirectory() as temporary, mock.patch.object(merge_macos.platform, "system", return_value="Darwin"), mock.patch.object(merge_macos.subprocess, "run") as run:
            path = Path(temporary) / "input"
            path.write_bytes(thin())
            with self.assertRaisesRegex(ValueError, "architectures"):
                merge_macos.merge(path, path, Path(temporary) / "out")
            run.assert_not_called()

    def test_failed_signing_preserves_previous_output(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            x86, arm, output = root / "x86", root / "arm", root / "out"
            x86.write_bytes(thin())
            arm.write_bytes(thin("arm64"))
            output.write_bytes(b"previous valid output")

            def run(command, **kwargs):
                if command[:2] == ["xcrun", "lipo"]:
                    Path(command[-1]).write_bytes(fat())
                    return subprocess.CompletedProcess(command, 0)
                raise subprocess.CalledProcessError(1, command)

            with mock.patch.object(merge_macos.platform, "system", return_value="Darwin"), mock.patch.object(merge_macos.subprocess, "run", side_effect=run), mock.patch.object(merge_macos.subprocess, "check_output", return_value="x86_64 arm64\n"):
                with self.assertRaises(subprocess.CalledProcessError):
                    merge_macos.merge(x86, arm, output)
            self.assertEqual(output.read_bytes(), b"previous valid output")
            self.assertEqual(sorted(path.name for path in root.iterdir()), ["arm", "out", "x86"])

    @unittest.skipUnless(platform.system() == "Darwin", "requires real macOS lipo, clang and codesign")
    def test_real_apple_tools_merge_package_and_native_cli_execution(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            source = root / "main.c"
            source.write_text("int main(void) { return 0; }\n")
            for arch in ("x86_64", "arm64"):
                subprocess.run(["xcrun", "clang", "-arch", arch, "-mmacosx-version-min=11.0", str(source), "-o", str(root / arch)], check=True)
            output = root / "universal"
            merge_macos.merge(root / "x86_64", root / "arm64", output)
            validate_macos_binary(output.read_bytes(), UNIVERSAL_ARCHITECTURES)
            (root / "tools").mkdir()
            (root / "docs").mkdir()
            for name in ("package-native.py", "macos_binary.py"):
                shutil.copyfile(Path(__file__).with_name(name), root / "tools" / name)
            for name in ("README.md", "Cargo.lock"):
                (root / name).write_text("Apple-tools integration fixture\n")
            subprocess.run([sys.executable, str(root / "tools/package-native.py"), "--target", "macos-universal2", "--binary", str(output)], check=True)
            artifacts = root / "artifacts"
            name = "openpdn-0.1.0-dev-macos-universal2"
            subprocess.run(["shasum", "-a", "256", "-c", name + ".zip.sha256"], cwd=artifacts, check=True)
            extracted = root / "extracted"
            subprocess.run(["ditto", "-x", "-k", str(artifacts / (name + ".zip")), str(extracted)], check=True)
            binary = extracted / name / "OpenPDN.app/Contents/MacOS/openpdn"
            self.assertTrue(binary.stat().st_mode & 0o111)
            # Keep the same Apple command order as the final package smoke jobs.
            subprocess.run(["xcrun", "lipo", str(binary), "-verify_arch", "x86_64", "arm64"], check=True)
            subprocess.run(["codesign", "--verify", "--strict", str(binary)], check=True)
            subprocess.run(["arch", "-" + platform.machine(), str(binary), "--help"], check=True)


if __name__ == "__main__":
    unittest.main()
