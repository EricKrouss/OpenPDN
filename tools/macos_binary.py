"""Inspect macOS release inputs without executing them or requiring Apple tools.

OpenPDN links third-party native code statically. Only Apple system dylibs and
frameworks may be loaded; fail closed if a dependency starts requiring bundling.
"""
import struct
from pathlib import PurePosixPath

MINIMUM_MACOS = "11.0"
UNIVERSAL_ARCHITECTURES = frozenset(("x86_64", "arm64"))
CPUS = {0x01000007: ("x86_64", 3), 0x0100000C: ("arm64", 0)}
DYLIB_COMMANDS = {0xC, 0x80000018, 0x8000001F, 0x80000023, 0x20}
FAT_FORMATS = {
    b"\xca\xfe\xba\xbe": (">", False),
    b"\xbe\xba\xfe\xca": ("<", False),
    b"\xca\xfe\xba\xbf": (">", True),
    b"\xbf\xba\xfe\xca": ("<", True),
}


def version_tuple(value):
    parts = tuple(map(int, value.split(".")))
    if not 1 <= len(parts) <= 3 or any(part < 0 for part in parts):
        raise ValueError(f"Invalid macOS version: {value}")
    return parts + (0,) * (3 - len(parts))


def version_string(value):
    major, minor, patch = value >> 16, (value >> 8) & 255, value & 255
    return f"{major}.{minor}" + (f".{patch}" if patch else "")


def system_library(path):
    return (path.startswith(("/usr/lib/", "/System/Library/"))
            and ".." not in PurePosixPath(path).parts)


def command_string(command, offset):
    if not 8 <= offset < len(command) or b"\0" not in command[offset:]:
        raise ValueError("Malformed Mach-O load-command string")
    return command[offset:].split(b"\0", 1)[0].decode("utf-8")


def thin_slice(data):
    if len(data) < 32 or data[:4] != b"\xcf\xfa\xed\xfe":
        raise ValueError("Expected a 64-bit little-endian Mach-O slice")
    _, cpu, subtype, kind, count, size, _, _ = struct.unpack_from("<8I", data)
    if cpu not in CPUS or (subtype & 0xFFFFFF) != CPUS[cpu][1]:
        raise ValueError("Unsupported Mach-O CPU/subtype (require baseline x86_64 or arm64)")
    if 32 + size > len(data) or count > size // 8:
        raise ValueError("Truncated Mach-O load commands")
    architecture = CPUS[cpu][0]
    minimum = None
    dependencies, rpaths = [], []
    cursor = 32
    for _ in range(count):
        if cursor + 8 > 32 + size:
            raise ValueError("Truncated Mach-O load command")
        cmd, cmdsize = struct.unpack_from("<2I", data, cursor)
        if cmdsize < 8 or cmdsize % 8 or cursor + cmdsize > 32 + size:
            raise ValueError("Invalid Mach-O load-command size")
        command = data[cursor:cursor + cmdsize]
        if cmd in (0x32, 0x24):  # LC_BUILD_VERSION / LC_VERSION_MIN_MACOSX
            if minimum is not None:
                raise ValueError("Duplicate Mach-O deployment target")
            if cmd == 0x32:
                if cmdsize < 24 or struct.unpack_from("<I", command, 8)[0] != 1:
                    raise ValueError("Expected macOS LC_BUILD_VERSION")
                minimum = struct.unpack_from("<I", command, 12)[0]
            else:
                if cmdsize < 16:
                    raise ValueError("Truncated LC_VERSION_MIN_MACOSX")
                minimum = struct.unpack_from("<I", command, 8)[0]
        elif cmd in DYLIB_COMMANDS or cmd in (0x8000001C, 0xE):
            required_size = 24 if cmd in DYLIB_COMMANDS else 12
            if cmdsize < required_size:
                raise ValueError("Truncated Mach-O dependency command")
            offset = struct.unpack_from("<I", command, 8)[0]
            if offset < required_size:
                raise ValueError("Invalid Mach-O dependency string offset")
            path = command_string(command, offset)
            (rpaths if cmd == 0x8000001C else dependencies).append(path)
        cursor += cmdsize
    if cursor != 32 + size or minimum is None or minimum == 0:
        raise ValueError("Missing or invalid Mach-O deployment target/load commands")
    return {"architecture": architecture, "cpu": cpu, "subtype": subtype,
            "file_type": kind, "minimum_macos": version_string(minimum),
            "dependencies": dependencies, "rpaths": rpaths}


def inspect_macho(data):
    """Read thin, FAT32 or FAT64 binaries; cross-check fat headers and slices."""
    magic = bytes(data[:4])
    if magic not in FAT_FORMATS:
        info = thin_slice(data)
        return {info["architecture"]: info}
    endian, wide = FAT_FORMATS[magic]
    if len(data) < 8:
        raise ValueError("Truncated universal Mach-O header")
    count = struct.unpack_from(endian + "I", data, 4)[0]
    entry_size = 32 if wide else 20
    table_end = 8 + count * entry_size
    if not 1 <= count <= 2 or table_end > len(data):
        raise ValueError("Invalid universal Mach-O architecture table")
    slices, ranges = {}, []
    for index in range(count):
        values = struct.unpack_from(endian + ("IIQQII" if wide else "IIIII"), data, 8 + index * entry_size)
        cpu, subtype, offset, size, alignment = values[:5]
        if alignment > 30 or offset % (1 << alignment) or offset < table_end or size < 32 or offset + size > len(data):
            raise ValueError("Invalid universal Mach-O slice bounds/alignment")
        if any(offset < end and start < offset + size for start, end in ranges):
            raise ValueError("Overlapping universal Mach-O slices")
        ranges.append((offset, offset + size))
        info = thin_slice(data[offset:offset + size])
        if (cpu, subtype) != (info["cpu"], info["subtype"]):
            raise ValueError("Universal Mach-O header does not match its slice")
        if info["architecture"] in slices:
            raise ValueError("Duplicate universal Mach-O architecture")
        slices[info["architecture"]] = info
    return slices


def validate_macos_binary(data, architectures, minimum_macos=MINIMUM_MACOS):
    """Validate every executable slice and its complete direct dependency list.

    No third-party dylibs/frameworks are currently bundled. Reject @rpath,
    @loader_path and developer-machine paths rather than silently omitting a
    dependency or shipping a library that only supports one architecture.
    """
    slices = inspect_macho(data)
    if set(slices) != set(architectures):
        raise ValueError(f"Mach-O architectures {sorted(slices)} do not match {sorted(architectures)}")
    for architecture, info in slices.items():
        if info["file_type"] != 2:  # MH_EXECUTE
            raise ValueError(f"{architecture}: expected a Mach-O executable")
        if version_tuple(info["minimum_macos"]) > version_tuple(minimum_macos):
            raise ValueError(f"{architecture}: requires macOS {info['minimum_macos']}, above {minimum_macos}")
        if architecture == "arm64" and version_tuple(info["minimum_macos"]) < (11, 0, 0):
            raise ValueError("arm64 macOS deployment target must be at least 11.0")
        for path in info["dependencies"] + info["rpaths"]:
            if not system_library(path):
                raise ValueError(f"{architecture}: non-system dependency/rpath {path!r}; bundling native libraries is not supported")
    return slices
