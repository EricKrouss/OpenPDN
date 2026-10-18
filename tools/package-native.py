#!/usr/bin/env python3
"""Create a deterministic development archive, without reference payloads.

This packages an already-built binary. Cross-compilation is recorded separately
from native platform validation; an archive is never a certification result.
"""
import argparse
import gzip
import hashlib
import io
import json
import platform
import plistlib
import re
import shutil
import subprocess
import tarfile
import zipfile
from pathlib import Path

root=Path(__file__).resolve().parent.parent
host_arch={"AMD64":"x86_64","aarch64":"arm64"}.get(platform.machine(),platform.machine())
target_hosts={"linux-x86_64":("Linux","x86_64"),"windows-x86_64":("Windows","x86_64"),"macos-arm64":("Darwin","arm64"),"macos-x86_64":("Darwin","x86_64")}
host_target=next((name for name,host in target_hosts.items() if host==(platform.system(),host_arch)),None)
parser=argparse.ArgumentParser(description=__doc__)
parser.add_argument("--target",choices=list(target_hosts),default=host_target)
parser.add_argument("--binary",type=Path)
parser.add_argument("--format",choices=("zip","tar.gz"),default="zip",help="Archive format (ZIP preserves executable permissions on Linux and macOS)")
parser.add_argument("--validation-result",type=Path,help="Successful native validation evidence for this exact binary")
args=parser.parse_args()
assert args.target,"Specify the built target explicitly"
windows=args.target.startswith("windows")
macos=args.target.startswith("macos")
cross_compiled=target_hosts[args.target]!=(platform.system(),host_arch)
target_triples={"linux-x86_64":"x86_64-unknown-linux-gnu","windows-x86_64":"x86_64-pc-windows-gnu","macos-arm64":"aarch64-apple-darwin","macos-x86_64":"x86_64-apple-darwin"}
binary=args.binary or root / "target" / (target_triples[args.target] if cross_compiled else "") / "release" / ("openpdn.exe" if windows else "openpdn")
assert binary.is_file(),f"Build the selected target first: {binary}"
header=binary.read_bytes()[:4096]
if windows:
    assert header[:2]==b"MZ", "The selected Windows binary is not a PE executable"
    offset=int.from_bytes(header[60:64],"little")
    assert header[offset:offset+6]==b"PE\0\0\x64\x86", "The selected Windows binary is not x86_64 PE"
elif args.target.startswith("linux"):
    assert header[:6]==b"\x7fELF\x02\x01" and int.from_bytes(header[18:20],"little")==62, "The selected Linux binary is not x86_64 ELF"
else:
    cpu=0x100000C if args.target=="macos-arm64" else 0x1000007
    assert header[:4]==b"\xcf\xfa\xed\xfe" and int.from_bytes(header[4:8],"little")==cpu, "The selected macOS binary has the wrong Mach-O architecture"
name="openpdn-0.1.0-dev-"+args.target
folder=root / "artifacts" / name
folder.mkdir(parents=True,exist_ok=True)
# Archive only the explicit allowlist below, never source or reference payloads.
binary_path="OpenPDN.app/Contents/MacOS/openpdn" if macos else ("openpdn.exe" if windows else "openpdn")
entries={binary_path:binary,"README.md":root/"README.md","Cargo.lock":root/"Cargo.lock"}
if (root/"LICENSE").is_file():entries["LICENSE"]=root/"LICENSE"
entries.update({"docs/"+p.name:p for p in (root/"docs").iterdir() if p.is_file()})
if (root/"artifacts/icons.svg").is_file():entries["docs/icons.svg"]=root/"artifacts/icons.svg"
metadata={"build":"0.1.0-dev","feature_complete":False,"target":args.target,
          "binary_path":binary_path,
          "binary_sha256":hashlib.sha256(binary.read_bytes()).hexdigest(),
          "cross_compiled":cross_compiled,
          "adjustment_entries":13,"effect_entries":40,"handcrafted_icons":121,
          "native_platform_validation":args.target=="linux-x86_64" and platform.system()=="Linux",
          "validation":"See docs/status.md for the exact tested platform and feature boundaries",
          "limits":"See docs/status.md","dotnet_runtime_required":False}
if args.target=="linux-x86_64" and platform.system()=="Linux" and shutil.which("objdump"):
    symbols=subprocess.check_output(["objdump","-T",str(binary)],text=True)
    versions={tuple(map(int,v.split("."))) for v in re.findall(r"GLIBC_(\d+\.\d+)",symbols)}
    metadata["max_glibc_symbol_version"]=".".join(map(str,max(versions))) if versions else None
contents={path:source.read_bytes() for path,source in entries.items()}
if args.validation_result:
    evidence=args.validation_result.read_bytes()
    result=json.loads(evidence)
    assert result.get("success") is True and result.get("binary_sha256")==metadata["binary_sha256"], "Validation must pass for this exact binary"
    assert all(check.get("exit_code")==0 for check in result["checks"]), "Native checks contain a failure"
    expected_backend={"macos":"Metal","linux":"Vulkan","windows":"Dx12"}[args.target.split("-")[0]]
    assert result.get("backend_requested")==expected_backend, "Validation backend does not match target"
    assert result.get("ui_requested") is True, "Window validation was skipped"
    metadata["native_platform_validation"]=True
    metadata["native_validation_evidence_sha256"]=hashlib.sha256(evidence).hexdigest()
    contents["docs/native-validation-result.json"]=evidence
if macos:
    contents["OpenPDN.app/Contents/Info.plist"]=plistlib.dumps({
        "CFBundleName":"OpenPDN","CFBundleDisplayName":"OpenPDN",
        "CFBundleIdentifier":"dev.openpdn.editor","CFBundlePackageType":"APPL",
        "CFBundleExecutable":"openpdn","CFBundleShortVersionString":"0.1.0",
        "CFBundleVersion":"1","NSHighResolutionCapable":True,
        "LSApplicationCategoryType":"public.app-category.graphics-design",
        "NSHumanReadableCopyright":"OpenPDN development build. See bundled documentation and font notices.",
    },sort_keys=True)
    contents["OpenPDN.app/Contents/PkgInfo"]=b"APPL????"
contents["build.json"]=(json.dumps(metadata,indent=2)+"\n").encode()
for path,data in contents.items():
    output=folder/path;output.parent.mkdir(parents=True,exist_ok=True);output.write_bytes(data)
    output.chmod(0o755 if path==binary_path and not windows else 0o644)
archive=root/"artifacts"/(name+(".zip" if args.format=="zip" else ".tar.gz"))
if args.format=="zip":
    with zipfile.ZipFile(archive,"w",compression=zipfile.ZIP_DEFLATED,compresslevel=9) as output:
        for path,data in sorted(contents.items()):
            info=zipfile.ZipInfo(name+"/"+path,date_time=(1980,1,1,0,0,0));info.compress_type=zipfile.ZIP_DEFLATED;info.create_system=3
            info.external_attr=(0o100755 if path==binary_path and not windows else 0o100644)<<16
            output.writestr(info,data,compresslevel=9)
else:
    with archive.open("wb") as raw,gzip.GzipFile(filename="",mode="wb",fileobj=raw,mtime=0) as compressed,tarfile.open(fileobj=compressed,mode="w") as output:
        for path,data in sorted(contents.items()):
            info=tarfile.TarInfo(name+"/"+path);info.size=len(data);info.mode=0o755 if path==binary_path else 0o644;info.mtime=0;info.uid=0;info.gid=0
            output.addfile(info,io.BytesIO(data))
checksum=hashlib.sha256(archive.read_bytes()).hexdigest()
(archive.parent/(archive.name+".sha256")).write_text(checksum+"  "+archive.name+"\n")
print(archive);print(checksum)
