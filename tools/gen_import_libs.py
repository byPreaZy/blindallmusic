#!/usr/bin/env python3
"""Generate MinGW import libraries for Windows system DLLs.

This machine has no C toolchain / no Windows SDK: rustc links with
rust-lld in self-contained mode, whose bundled MinGW import libs cover
only kernel32/msvcrt/etc. Deps like getrandom (bcrypt), whoami
(advapi32) and tower/http stacks (ws2_32, userenv, …) need import libs
that don't exist. This script parses each DLL's PE export table into a
.def, then `rust-lld -flavor link` synthesizes `lib<name>.dll.a`.
"""

import struct
import subprocess
import sys
from pathlib import Path

SYS32 = Path(r"C:\Windows\System32")
OUT = Path(__file__).resolve().parent.parent / ".libs"
LLD = Path.home() / ".rustup" / "toolchains" / "stable-x86_64-pc-windows-gnullvm" / "lib" / "rustlib" / "x86_64-pc-windows-gnullvm" / "bin" / "rust-lld.exe"

DLLS = [
    "bcrypt.dll", "advapi32.dll", "ole32.dll", "oleaut32.dll",
    "userenv.dll", "ws2_32.dll", "ntdll.dll", "dbghelp.dll",
    "user32.dll", "secur32.dll", "crypt32.dll", "iphlpapi.dll",
]


def exports(dll: Path) -> list[str]:
    data = dll.read_bytes()

    def rva2off(rva: int) -> int:
        for s in sections:
            start, vsize, raw = s
            if start <= rva < start + max(vsize, 1):
                return raw + (rva - start)
        return rva

    pe = struct.unpack_from("<I", data, 0x3C)[0]
    num_sections = struct.unpack_from("<H", data, pe + 6)[0]
    opt_size = struct.unpack_from("<H", data, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from("<H", data, opt)[0]
    if magic != 0x20B:
        raise RuntimeError(f"{dll.name}: not PE32+")
    export_rva = struct.unpack_from("<I", data, opt + 0x70)[0]
    sections = []
    for i in range(num_sections):
        off = opt + opt_size + i * 40
        vsize = struct.unpack_from("<I", data, off + 8)[0]
        vaddr = struct.unpack_from("<I", data, off + 12)[0]
        raw = struct.unpack_from("<I", data, off + 20)[0]
        sections.append((vaddr, vsize, raw))

    eat = rva2off(export_rva)
    num_names = struct.unpack_from("<I", data, eat + 24)[0]
    names_rva = struct.unpack_from("<I", data, eat + 32)[0]
    names = []
    name_ptrs = rva2off(names_rva)
    for i in range(num_names):
        name_rva = struct.unpack_from("<I", data, name_ptrs + i * 4)[0]
        end = data.index(b"\x00", rva2off(name_rva))
        names.append(data[rva2off(name_rva):end].decode("ascii", "replace"))
    return names


def main() -> int:
    OUT.mkdir(exist_ok=True)
    for dll_name in DLLS:
        dll = SYS32 / dll_name
        if not dll.exists():
            print(f"skip {dll_name} (absent)")
            continue
        stem = dll_name.rsplit(".", 1)[0]
        names = exports(dll)
        def_path = OUT / f"{stem}.def"
        def_path.write_text(
            f"LIBRARY {dll_name}\nEXPORTS\n" + "".join(f"    {n}\n" for n in names)
        )
        lib = OUT / f"lib{stem}.dll.a"
        r = subprocess.run(
            [
                str(LLD), "-flavor", "link",
                "/dll", f"/def:{def_path}", "/noentry",
                f"/implib:{lib}", f"/out:{OUT / dll_name}",
                "/machine:x64",
            ],
            capture_output=True, text=True,
        )
        if r.returncode != 0:
            print(f"FAIL {dll_name}: {r.stderr.strip()[:400]}")
            continue
        print(f"ok {lib.name} ({len(names)} exports)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
