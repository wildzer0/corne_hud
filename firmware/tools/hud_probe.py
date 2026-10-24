#!/usr/bin/env python3
"""Development probe for the Keyboard HUD Raw HID protocol (see docs/PROTOCOL.md).

Requires the `hidapi` package:  pip install hidapi
Close Vial before running it (both use the same HID interface).

    python hud_probe.py info
    python hud_probe.py state [--watch]
    python hud_probe.py keymap [--layer N] [--raw]
"""
import argparse
import struct
import sys
import time

import hid

USAGE_PAGE, USAGE = 0xFF60, 0x61
REPORT = 32
HUD_CMD = 0xF0
GET_INFO, GET_STATE, GET_KEYMAP = 0x01, 0x02, 0x03
STATUS_PENDING = 0xFF

MODS = ["LCtl", "LSft", "LAlt", "LGui", "RCtl", "RSft", "RAlt", "RGui"]
FLAGS = ["CapsLock", "CapsWord", "Colemak", "User", "NumLock", "SlaveOK"]
CAPS = ["Colemak", "CapsWord", "OSDetection", "HUD_TOG", "LayerLock", "Split"]
OS_DETECTED = ["unsure", "linux", "windows", "macos", "ios"]
OS_MODE = ["auto", "win", "mac", "lnx"]


def open_device():
    for d in hid.enumerate():
        if d["usage_page"] == USAGE_PAGE and d["usage"] == USAGE:
            dev = hid.device()
            dev.open_path(d["path"])
            print(f"# {d['manufacturer_string']} {d['product_string']} "
                  f"({d['vendor_id']:04x}:{d['product_id']:04x})", file=sys.stderr)
            return dev
    sys.exit("Raw HID interface (usage page 0xFF60) not found")


class Hud:
    def __init__(self, dev):
        self.dev = dev
        self.seq = 0

    def raw(self, payload):
        buf = bytes(payload).ljust(REPORT, b"\0")
        # Report ID 0x00 in front: required on Windows, harmless elsewhere with hidapi
        self.dev.write(b"\0" + buf)
        resp = bytes(self.dev.read(REPORT, 100))
        if len(resp) != REPORT:
            raise IOError(f"timeout or short read ({len(resp)} bytes)")
        return resp

    def cmd(self, sub, args=b""):
        self.seq = (self.seq + 1) & 0xFF
        resp = self.raw(bytes([HUD_CMD, sub, STATUS_PENDING, self.seq]) + bytes(args))
        if resp[0] == 0xFF:
            raise IOError("firmware without HUD support (id_unhandled)")
        if resp[0] != HUD_CMD or resp[1] != sub:
            raise IOError(f"unexpected response {resp[:4].hex()}")
        if resp[3] != self.seq:
            raise IOError(f"stale response: seq {resp[3]} != {self.seq}")
        if resp[2] == STATUS_PENDING:
            raise IOError("request echoed unchanged (Vial unlock in progress?)")
        if resp[2] != 0:
            raise IOError(f"error status {resp[2]}")
        return resp

    def info(self):
        r = self.cmd(GET_INFO)
        return {
            "protocol": r[4], "rows": r[5], "cols": r[6], "layers": r[7],
            "caps": struct.unpack_from("<H", r, 8)[0],
            "hud_tog": struct.unpack_from("<H", r, 10)[0],
            "keymap_max": r[12], "keymap_version": r[13],
        }

    def state(self):
        r = self.cmd(GET_STATE)
        return {
            "layer_state": struct.unpack_from("<I", r, 4)[0],
            "default_layer": r[8], "locked": r[9],
            "matrix": list(r[10:18]),
            "mods": r[18], "weak": r[19], "osm": r[20], "osm_locked": r[21],
            "osl": None if r[22] == 0xFF else r[22],
            "flags": r[23], "os_detected": r[24], "os_mode": r[25], "os_active": r[26],
            "hud_tog": r[27], "keymap_version": r[28],
        }

    def keymap(self, layer, rows, cols, chunk):
        out = []
        total = rows * cols
        for off in range(0, total, chunk):
            n = min(chunk, total - off)
            r = self.cmd(GET_KEYMAP, struct.pack("<BHB", layer, off, n))
            out += struct.unpack_from(f"<{n}H", r, 8)
        return out

    def via_raw_keymap(self, layer, rows, cols):
        """VIA 0x12 (dynamic_keymap_get_buffer): raw keycodes, big-endian, no Colemak."""
        size = rows * cols * 2
        base = layer * size
        data = b""
        for off in range(0, size, 28):
            n = min(28, size - off)
            r = self.raw(struct.pack(">BHB", 0x12, base + off, n))
            data += r[4:4 + n]
        return list(struct.unpack(f">{rows * cols}H", data))


def bits(value, names):
    return " ".join(n for i, n in enumerate(names) if value & (1 << i)) or "-"


def layer_names(mask):
    return " ".join(str(i) for i in range(32) if mask & (1 << i)) or "-"


def print_state(s, rows, cols):
    print(f"layers   : {layer_names(s['layer_state'])}  (default {layer_names(s['default_layer'])}, "
          f"llck {layer_names(s['locked'])}, osl {s['osl']})")
    print(f"mods     : {bits(s['mods'], MODS)} | weak {bits(s['weak'], MODS)} | "
          f"osm {bits(s['osm'], MODS)} | osm locked {bits(s['osm_locked'], MODS)}")
    print(f"flags    : {bits(s['flags'], FLAGS)}")
    det = OS_DETECTED[s['os_detected']] if s['os_detected'] < len(OS_DETECTED) else s['os_detected']
    print(f"os       : detected {det}, mode {OS_MODE[s['os_mode'] & 3]}, active {OS_MODE[s['os_active'] & 3]}")
    print(f"counters : hud_tog {s['hud_tog']}, keymap_version {s['keymap_version']}")
    pressed = [f"{r},{c}" for r in range(rows) for c in range(cols) if s["matrix"][r] & (1 << c)]
    print(f"pressed  : {' '.join(pressed) or '-'}")


def print_grid(codes, rows, cols, other=None):
    for r in range(rows):
        cells = []
        for c in range(cols):
            kc = codes[r * cols + c]
            mark = "*" if other is not None and other[r * cols + c] != kc else " "
            cells.append(f"{kc:04x}{mark}")
        print(f"  r{r}: " + " ".join(cells))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("command", choices=["info", "state", "keymap"])
    ap.add_argument("--watch", action="store_true", help="state: refresh at ~10 Hz")
    ap.add_argument("--layer", type=int, help="keymap: only this layer")
    ap.add_argument("--raw", action="store_true", help="keymap: compare with VIA raw keymap (* = differs)")
    a = ap.parse_args()

    hud = Hud(open_device())
    info = hud.info()
    rows, cols = info["rows"], info["cols"]

    if a.command == "info":
        for k, v in info.items():
            print(f"{k:15}: {v:#06x}" if k in ("caps", "hud_tog") else f"{k:15}: {v}")
        print(f"{'capabilities':15}: {bits(info['caps'], CAPS)}")
    elif a.command == "state":
        while True:
            t = time.perf_counter()
            s = hud.state()
            dt = (time.perf_counter() - t) * 1000
            if a.watch:
                print("\033[2J\033[H", end="")
            print_state(s, rows, cols)
            print(f"latency  : {dt:.1f} ms")
            if not a.watch:
                break
            time.sleep(0.1)
    else:
        layers = [a.layer] if a.layer is not None else range(info["layers"])
        for layer in layers:
            eff = hud.keymap(layer, rows, cols, info["keymap_max"])
            raw = hud.via_raw_keymap(layer, rows, cols) if a.raw else None
            print(f"layer {layer}:")
            print_grid(eff, rows, cols, raw)


if __name__ == "__main__":
    main()
