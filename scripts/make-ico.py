#!/usr/bin/env python3
"""Pack PNG files into a Windows .ico (PNG-compressed entries, Vista and later).

Usage: make-ico.py OUT.ico icon-16.png icon-32.png ...

Standard library only. Every input must be a square PNG of at most 256 px;
the size is read from the PNG header, so the file names do not matter.
"""
import struct
import sys

PNG_MAGIC = b"\x89PNG\r\n\x1a\n"


def png_size(data, name):
    if data[:8] != PNG_MAGIC or data[12:16] != b"IHDR":
        raise SystemExit(f"{name}: not a PNG file")
    w, h = struct.unpack(">II", data[16:24])
    if w != h or not 1 <= w <= 256:
        raise SystemExit(f"{name}: need a square PNG of 1..256 px, got {w}x{h}")
    return w


def pack(paths):
    images = []
    for p in paths:
        with open(p, "rb") as f:
            data = f.read()
        images.append((png_size(data, p), data))
    images.sort(key=lambda t: t[0])
    out = bytearray(struct.pack("<HHH", 0, 1, len(images)))  # reserved, type 1 = icon, count
    offset = 6 + 16 * len(images)
    for size, data in images:
        # width/height byte 0 means 256; planes 1, 32 bits per pixel.
        out += struct.pack("<BBBBHHII", size % 256, size % 256, 0, 0, 1, 32, len(data), offset)
        offset += len(data)
    for _, data in images:
        out += data
    return bytes(out)


def main(argv):
    if len(argv) < 3:
        raise SystemExit(__doc__)
    with open(argv[1], "wb") as f:
        f.write(pack(argv[2:]))
    print(f"created: {argv[1]}")


if __name__ == "__main__":
    main(sys.argv)
