"""Check that a build artifact is a self-contained ARM64 Linux ELF executable."""

import struct
import sys
from pathlib import Path


def verify(path):
    data = Path(path).read_bytes()
    if data[:7] != b"\x7fELF\x02\x01\x01" or len(data) < 64:
        raise ValueError("expected a 64-bit little-endian ELF executable")
    kind, machine = struct.unpack_from("<HH", data, 16)
    if kind not in (2, 3) or machine != 183:
        raise ValueError("expected an ARM64 executable")
    offset = struct.unpack_from("<Q", data, 32)[0]
    size, count = struct.unpack_from("<HH", data, 54)
    if size != 56 or count == 0 or offset + size * count > len(data):
        raise ValueError("invalid program headers")
    for index in range(count):
        header = offset + index * size
        segment = struct.unpack_from("<I", data, header)[0]
        if segment == 3:  # PT_INTERP names an external dynamic loader.
            raise ValueError("executable requires a dynamic loader")
        if segment == 2:  # PT_DYNAMIC may name external shared libraries.
            start = struct.unpack_from("<Q", data, header + 8)[0]
            length = struct.unpack_from("<Q", data, header + 32)[0]
            if length % 16 or start + length > len(data):
                raise ValueError("invalid dynamic section")
            for position in range(start, start + length, 16):
                tag = struct.unpack_from("<q", data, position)[0]
                if tag == 0:
                    break
                if tag == 1:  # DT_NEEDED
                    raise ValueError("executable requires a shared library")
    print(f"{path}: self-contained ARM64 ELF executable")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        sys.exit("usage: verify_binary.py EXECUTABLE")
    try:
        verify(sys.argv[1])
    except (OSError, ValueError, struct.error) as error:
        sys.exit(str(error))
