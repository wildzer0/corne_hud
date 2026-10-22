#!/usr/bin/env python3
"""Draws the Keyboard HUD icon and writes every format the build needs.

    python3 packaging/icon/make_icon.py      (from app/; needs numpy)

Outputs, next to this script:
- icon.png       512x512, for README and documentation
- icon.ico       Windows .exe icon (build.rs), PNG entries 16-256
- icon.icns      macOS .app icon (CI)
- icon-64.rgba   window icon embedded in the binary (64x64 RGBA, raw)

The design: a split keyboard with column stagger (Corne), one key per side
in the colours of the SYM and NAV layers.
"""

import struct
import zlib
from pathlib import Path

import numpy as np

HERE = Path(__file__).resolve().parent
SIZE = 512
SS = 4  # supersampling

BG_TOP = (0x2A, 0x2E, 0x3A)
BG_BOTTOM = (0x16, 0x17, 0x1D)
KEY = (0x4A, 0x50, 0x63)
CYAN = (0x00, 0xB9, 0xD7)
GREEN = (0x3C, 0xC8, 0x5A)


def rounded_rect(x, y, x0, y0, x1, y1, r):
    """Coverage mask (0/1) of a rounded rectangle."""
    cx = np.clip(x, x0 + r, x1 - r)
    cy = np.clip(y, y0 + r, y1 - r)
    return ((x - cx) ** 2 + (y - cy) ** 2 <= r * r) & (x >= x0) & (x <= x1) & (y >= y0) & (y <= y1)


def keys():
    """(x0, y0, x1, y1, colour) of every key, in 512 px units."""
    k, gap = 54, 10
    half = 3 * k + 2 * gap
    mid_gap = 44
    left = (SIZE - 2 * half - mid_gap) / 2
    stagger = [18, 0, 8]  # outer, middle, inner column
    top = 132
    out = []
    for side in (0, 1):
        x_half = left if side == 0 else left + half + mid_gap
        for c in range(3):
            col = c if side == 0 else 2 - c  # mirrored stagger
            x0 = x_half + c * (k + gap)
            for r in range(3):
                y0 = top + stagger[col] + r * (k + gap)
                colour = KEY
                if r == 1 and col == 2:  # home row, inner column
                    colour = CYAN if side == 0 else GREEN
                out.append((x0, y0, x0 + k, y0 + k, colour))
        # two thumb keys, towards the middle
        y0 = top + 3 * (k + gap) + 14
        for t in range(2):
            i = t + 1 if side == 0 else t
            x0 = x_half + i * (k + gap)
            out.append((x0, y0, x0 + k, y0 + k, KEY))
    return out


def draw():
    n = SIZE * SS
    coords = (np.arange(n) + 0.5) / SS
    x, y = np.meshgrid(coords, coords)
    img = np.zeros((n, n, 4), dtype=np.float32)

    bg = rounded_rect(x, y, 8, 8, SIZE - 8, SIZE - 8, 112)
    t = ((y - 8) / (SIZE - 16)).clip(0, 1)[..., None]
    grad = np.array(BG_TOP) * (1 - t) + np.array(BG_BOTTOM) * t
    img[..., :3] = np.where(bg[..., None], grad, 0)
    img[..., 3] = bg * 255.0

    for x0, y0, x1, y1, colour in keys():
        m = rounded_rect(x, y, x0, y0, x1, y1, 11)
        img[m, :3] = colour

    # box filter down to SIZE
    img = img.reshape(SIZE, SS, SIZE, SS, 4).mean(axis=(1, 3))
    return img


def resize(img, size):
    """Area resampling from SIZE to `size` (premultiplied, so edges stay clean)."""
    if size == SIZE:
        return img
    pre = img.copy()
    pre[..., :3] *= pre[..., 3:4] / 255.0
    src = np.arange(SIZE + 1) * size / SIZE
    # weights of each source pixel in each destination pixel, per axis
    w = np.zeros((size, SIZE))
    for i in range(SIZE):
        a, b = src[i], src[i + 1]
        for j in range(int(a), min(int(np.ceil(b)), size)):
            w[j, i] = max(0.0, min(b, j + 1) - max(a, j))
    w /= w.sum(axis=1, keepdims=True)
    # rows, then columns: (size, SIZE, 4) -> (size, size, 4)
    out = np.tensordot(w, pre, axes=(1, 0))
    out = np.tensordot(out, w, axes=(1, 1)).transpose(0, 2, 1)
    alpha = out[..., 3:4]
    out[..., :3] = np.where(alpha > 0, out[..., :3] * 255.0 / np.maximum(alpha, 1e-6), 0)
    return out


def to_bytes(img):
    return np.clip(np.round(img), 0, 255).astype(np.uint8)


def png(img):
    data = to_bytes(img)
    h, w = data.shape[:2]
    raw = b"".join(b"\0" + data[r].tobytes() for r in range(h))

    def chunk(tag, body):
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)

    return (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(raw, 9))
        + chunk(b"IEND", b"")
    )


def ico(images):
    header = struct.pack("<HHH", 0, 1, len(images))
    offset = len(header) + 16 * len(images)
    entries, blobs = b"", b""
    for size, data in images:
        dim = 0 if size >= 256 else size
        entries += struct.pack("<BBBBHHII", dim, dim, 0, 0, 1, 32, len(data), offset + len(blobs))
        blobs += data
    return header + entries + blobs


def icns(images):
    body = b"".join(tag + struct.pack(">I", len(data) + 8) + data for tag, data in images)
    return b"icns" + struct.pack(">I", len(body) + 8) + body


def main():
    img = draw()
    sizes = {s: resize(img, s) for s in (16, 24, 32, 48, 64, 128, 256, 512)}
    pngs = {s: png(i) for s, i in sizes.items()}
    (HERE / "icon.png").write_bytes(pngs[512])
    (HERE / "icon.ico").write_bytes(ico([(s, pngs[s]) for s in (16, 24, 32, 48, 64, 128, 256)]))
    (HERE / "icon.icns").write_bytes(
        icns([(b"icp4", pngs[16]), (b"icp5", pngs[32]), (b"icp6", pngs[64]), (b"ic07", pngs[128]),
              (b"ic08", pngs[256]), (b"ic09", pngs[512])])
    )
    (HERE / "icon-64.rgba").write_bytes(to_bytes(sizes[64]).tobytes())
    print("icone scritte in", HERE)


if __name__ == "__main__":
    main()
