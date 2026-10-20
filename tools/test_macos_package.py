"""Exercise archive contents with synthetic binaries, without claiming execution."""
import hashlib
import json
from pathlib import Path
import plistlib
import shutil
import struct
import subprocess
import sys
import tarfile
import tempfile
import unittest
import zipfile

from test_macos_binary import fat, thin


class PackageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix="openpdn-package-test-")
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        (self.root / "tools").mkdir()
        (self.root / "docs").mkdir()
        for name in ("package-native.py", "macos_binary.py"):
            shutil.copyfile(Path(__file__).with_name(name), self.root / "tools" / name)
        for name in ("README.md", "Cargo.lock", "LICENSE", "docs/status.md"):
            (self.root / name).write_text("Test payload\n")
        self.binary = self.root / "openpdn"
        self.binary.write_bytes(fat())

    def package(self, target="macos-universal2", format="zip", extra=(), success=True, default_binary=False):
        result = subprocess.run([sys.executable, str(self.root / "tools/package-native.py"), "--target", target, "--format", format,
                                 *([] if default_binary else ["--binary", str(self.binary)]), *extra], capture_output=True, text=True)
        if not success:
            self.assertNotEqual(result.returncode, 0, result.stdout)
            self.assertFalse(list((self.root / "artifacts").glob("*.zip")))
            return result.stderr
        self.assertEqual(result.returncode, 0, result.stderr)
        return self.root / "artifacts" / f"openpdn-0.1.0-dev-{target}.{format}"

    def test_zip_naming_plist_metadata_permissions_checksum_and_determinism(self):
        archive = self.package()
        before = archive.read_bytes()
        name = archive.stem
        with zipfile.ZipFile(archive) as source:
            binary_path = "OpenPDN.app/Contents/MacOS/openpdn"
            self.assertEqual(source.read(f"{name}/{binary_path}"), self.binary.read_bytes())
            self.assertEqual(source.getinfo(f"{name}/{binary_path}").external_attr >> 16, 0o100755)
            self.assertTrue(all(info.date_time == (1980, 1, 1, 0, 0, 0) for info in source.infolist()))
            metadata = json.loads(source.read(f"{name}/build.json"))
            self.assertEqual(metadata["target"], "macos-universal2")
            self.assertEqual(metadata["architectures"], ["arm64", "x86_64"])
            self.assertEqual(metadata["minimum_macos"], "11.0")
            self.assertEqual(metadata["bundled_native_libraries"], [])
            self.assertFalse(metadata["native_platform_validation"])
            self.assertFalse(metadata["feature_complete"])
            self.assertEqual(metadata["binary_sha256"], hashlib.sha256(self.binary.read_bytes()).hexdigest())
            self.assertEqual(set(metadata["macos_slices"]), {"x86_64", "arm64"})
            plist = plistlib.loads(source.read(f"{name}/OpenPDN.app/Contents/Info.plist"))
            self.assertEqual(plist["CFBundleExecutable"], "openpdn")
            self.assertEqual(plist["CFBundleIdentifier"], "dev.openpdn.editor")
            self.assertEqual(plist["LSMinimumSystemVersion"], "11.0")
            self.assertEqual(plist["LSMinimumSystemVersionByArchitecture"], {"arm64": "11.0", "x86_64": "11.0"})
            self.assertIn(f"{name}/LICENSE", source.namelist())
        self.assertEqual(Path(str(archive) + ".sha256").read_text(), hashlib.sha256(before).hexdigest() + "  " + archive.name + "\n")
        self.package()
        self.assertEqual(archive.read_bytes(), before)

    def test_tar_naming_permissions_and_determinism(self):
        archive = self.package(format="tar.gz")
        before = archive.read_bytes()
        with tarfile.open(archive) as source:
            member = source.getmember("openpdn-0.1.0-dev-macos-universal2/OpenPDN.app/Contents/MacOS/openpdn")
            self.assertEqual(member.mode, 0o755)
            self.assertEqual(source.extractfile(member).read(), self.binary.read_bytes())
        self.package(format="tar.gz")
        self.assertEqual(archive.read_bytes(), before)

    def test_default_universal_input_path(self):
        target = self.root / "target/universal2-apple-darwin/release/openpdn"
        target.parent.mkdir(parents=True)
        target.write_bytes(self.binary.read_bytes())
        self.package(default_binary=True)

    def test_thin_macos_targets_remain_available(self):
        for arch in ("x86_64", "arm64"):
            self.binary.write_bytes(thin(arch))
            self.package(target="macos-" + arch)

    def test_reject_thin_input_for_universal_package(self):
        self.binary.write_bytes(thin())
        self.assertIn("architectures", self.package(success=False))

    def test_reject_a_hidden_non_system_dependency(self):
        self.binary.write_bytes(fat([thin(), thin("arm64", dependencies=("@rpath/libfoo.dylib",))]))
        self.assertIn("non-system dependency", self.package(success=False))

    def test_reject_unvalidated_bundled_native_library(self):
        (self.root / "docs/accidental.dylib").write_bytes(thin(kind=6))
        self.assertIn("Unexpected bundled native payload", self.package(success=False))

    def test_reject_single_native_validation_record_for_universal_binary(self):
        self.assertIn("separate runtime evidence", self.package(extra=("--validation-result", str(self.root / "unused.json")), success=False))

    def test_source_validation_zip_includes_packaging_dependencies(self):
        for folder in ("src", "tests", "examples"):
            (self.root / folder).mkdir()
        (self.root / "Cargo.toml").write_text("Test manifest\n")
        for name in ("package-validation-source.py", "validate-macos.py", "test-validation-timestamps.py", "test-validation-report.py", "merge-macos-universal.py", "test_macos_binary.py", "test_macos_package.py"):
            shutil.copyfile(Path(__file__).with_name(name), self.root / "tools" / name)
        result = subprocess.run([sys.executable, str(self.root / "tools/package-validation-source.py")], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        with zipfile.ZipFile(self.root / "artifacts/openpdn-metal-validation-source.zip") as source:
            prefix = "openpdn-metal-validation/"
            manifest = json.loads(source.read(prefix + "source-manifest.json"))
            for name in ("tools/macos_binary.py", "tools/package-native.py", "tools/merge-macos-universal.py", "tools/test_macos_binary.py", "tools/test_macos_package.py"):
                self.assertEqual(source.read(prefix + name), (self.root / name).read_bytes())
            for name, digest in manifest["files"].items():
                self.assertEqual(hashlib.sha256(source.read(prefix + name)).hexdigest(), digest)

    def test_other_platform_archive_names_and_executable_paths(self):
        elf = bytearray(64)
        elf[:6] = b"\x7fELF\x02\x01"
        struct.pack_into("<H", elf, 18, 62)
        pe = bytearray(128)
        pe[:2] = b"MZ"
        struct.pack_into("<I", pe, 60, 64)
        pe[64:70] = b"PE\0\0\x64\x86"
        for target, data, binary_path in (("linux-x86_64", elf, "openpdn"), ("windows-x86_64", pe, "openpdn.exe")):
            self.binary.write_bytes(data)
            # Avoid asking objdump to inspect the intentionally synthetic ELF.
            if target.startswith("linux") and sys.platform == "linux" and shutil.which("objdump"):
                continue
            archive = self.package(target=target)
            with zipfile.ZipFile(archive) as source:
                self.assertEqual(source.read(f"{archive.stem}/{binary_path}"), data)


if __name__ == "__main__":
    unittest.main()
