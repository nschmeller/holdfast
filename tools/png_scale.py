#!/usr/bin/env python3
"""Halve a PNG's dimensions with a box filter, and crop to an aspect.

Written because this machine has no PIL and the itch.io cover has to be exactly
630x500. Rendering at 630x500 on a 2x display gives a 1260x1000 screenshot, so
halving it is exact and needs no resampling cleverness.

    png_scale.py in.png out.png            halve
    png_scale.py in.png out.png 630 500    halve, then centre-crop to 630x500
"""
import struct
import sys
import zlib


def read_png(path):
    data = open(path, "rb").read()
    if data[:8] != b"\x89PNG\r\n\x1a\x08"[:8] and data[:8] != b"\x89PNG\r\n\x1a\n":
        raise SystemExit(f"{path} is not a PNG")
    i, idat, w, h, ct = 8, b"", 0, 0, 0
    while i < len(data):
        n = struct.unpack(">I", data[i : i + 4])[0]
        kind = data[i + 4 : i + 8]
        body = data[i + 8 : i + 8 + n]
        if kind == b"IHDR":
            w, h, depth, ct = struct.unpack(">IIBB", body[:10])
            if depth != 8 or ct not in (2, 6):
                raise SystemExit("only 8-bit RGB/RGBA is handled")
        elif kind == b"IDAT":
            idat += body
        i += 12 + n
    raw = zlib.decompress(idat)
    ch = 3 if ct == 2 else 4
    stride = w * ch + 1
    rows, prev = [], bytearray(w * ch)
    for y in range(h):
        off = y * stride
        f = raw[off]
        line = bytearray(raw[off + 1 : off + 1 + w * ch])
        if f == 1:
            for x in range(ch, len(line)):
                line[x] = (line[x] + line[x - ch]) & 255
        elif f == 2:
            for x in range(len(line)):
                line[x] = (line[x] + prev[x]) & 255
        elif f == 3:
            for x in range(len(line)):
                a = line[x - ch] if x >= ch else 0
                line[x] = (line[x] + ((a + prev[x]) >> 1)) & 255
        elif f == 4:
            for x in range(len(line)):
                a = line[x - ch] if x >= ch else 0
                b, c = prev[x], (prev[x - ch] if x >= ch else 0)
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[x] = (line[x] + pr) & 255
        rows.append(line)
        prev = line
    return w, h, ch, rows


def write_png(path, w, h, rows):
    body = b"".join(b"\x00" + bytes(r) for r in rows)
    def chunk(kind, payload):
        return (
            struct.pack(">I", len(payload))
            + kind
            + payload
            + struct.pack(">I", zlib.crc32(kind + payload) & 0xFFFFFFFF)
        )
    out = b"\x89PNG\r\n\x1a\n"
    out += chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 2, 0, 0, 0))
    out += chunk(b"IDAT", zlib.compress(body, 9))
    out += chunk(b"IEND", b"")
    open(path, "wb").write(out)


def main():
    if len(sys.argv) < 3:
        print(__doc__)
        return 2
    src, dst = sys.argv[1], sys.argv[2]
    w, h, ch, rows = read_png(src)
    hw, hh = w // 2, h // 2
    small = []
    for y in range(hh):
        a, b = rows[y * 2], rows[y * 2 + 1]
        line = bytearray(hw * 3)
        for x in range(hw):
            for c in range(3):
                s = (
                    a[(x * 2) * ch + c]
                    + a[(x * 2 + 1) * ch + c]
                    + b[(x * 2) * ch + c]
                    + b[(x * 2 + 1) * ch + c]
                )
                line[x * 3 + c] = s // 4
        small.append(line)
    if len(sys.argv) >= 5:
        tw, th = int(sys.argv[3]), int(sys.argv[4])
        x0, y0 = max(0, (hw - tw) // 2), max(0, (hh - th) // 2)
        small = [r[x0 * 3 : (x0 + tw) * 3] for r in small[y0 : y0 + th]]
        hw, hh = tw, min(th, len(small))
    write_png(dst, hw, hh, small)
    print(f"{dst} {hw}x{hh}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
