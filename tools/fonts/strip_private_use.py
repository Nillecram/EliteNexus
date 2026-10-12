"""Removes Private Use Area (U+E000..U+F8FF) mappings from a TrueType font's character map.

Inter maps hundreds of private-use code points to stylistic alternates. The Phosphor icon font that
Elite Nexus uses lives in the same range, and egui takes a character from the first font in the
fallback list that has it, so Inter's alternates would replace the icons. With these mappings gone
Inter answers for real characters only and the icons come from Phosphor. Glyph outlines, metrics and
all other tables are untouched. Standard library only.

    python tools/fonts/strip_private_use.py data/fonts/inter/Inter-Regular.ttf [...]
"""
import struct
import sys

PUA = range(0xE000, 0xF900)


def parse_tables(data):
    num = struct.unpack(">H", data[4:6])[0]
    tables = []
    for i in range(num):
        tag, _chk, off, ln = struct.unpack(">4sIII", data[12 + 16 * i: 28 + 16 * i])
        tables.append([tag, data[off:off + ln]])
    return tables


def read_cmap(cmap):
    """All mappings from the format 4 and format 12 subtables: {code: glyph}."""
    _ver, n = struct.unpack(">HH", cmap[:4])
    mapping = {}
    for i in range(n):
        _pid, _eid, so = struct.unpack(">HHI", cmap[4 + 8 * i: 12 + 8 * i])
        fmt = struct.unpack(">H", cmap[so:so + 2])[0]
        if fmt == 4:
            segx2 = struct.unpack(">H", cmap[so + 6:so + 8])[0]
            segs = segx2 // 2
            ends = struct.unpack(">%dH" % segs, cmap[so + 14: so + 14 + segx2])
            starts = struct.unpack(">%dH" % segs, cmap[so + 16 + segx2: so + 16 + 2 * segx2])
            deltas = struct.unpack(">%dh" % segs, cmap[so + 16 + 2 * segx2: so + 16 + 3 * segx2])
            ro_pos = so + 16 + 3 * segx2
            offsets = struct.unpack(">%dH" % segs, cmap[ro_pos: ro_pos + segx2])
            for k in range(segs):
                for c in range(starts[k], ends[k] + 1):
                    if c == 0xFFFF:
                        continue
                    if offsets[k] == 0:
                        g = (c + deltas[k]) & 0xFFFF
                    else:
                        addr = ro_pos + 2 * k + offsets[k] + 2 * (c - starts[k])
                        g = struct.unpack(">H", cmap[addr:addr + 2])[0]
                        if g:
                            g = (g + deltas[k]) & 0xFFFF
                    if g:
                        mapping.setdefault(c, g)
        elif fmt == 12:
            ngroups = struct.unpack(">I", cmap[so + 12:so + 16])[0]
            for gi in range(ngroups):
                s, e, g0 = struct.unpack(">III", cmap[so + 16 + 12 * gi: so + 28 + 12 * gi])
                for c in range(s, e + 1):
                    mapping[c] = g0 + (c - s)
    return mapping


def runs(pairs):
    """Consecutive codes with consecutive glyphs: [(start, end, first_glyph)]."""
    out = []
    for c, g in pairs:
        if out and c == out[-1][1] + 1 and g == out[-1][2] + (c - out[-1][0]):
            out[-1][1] = c
        else:
            out.append([c, c, g])
    return out


def format4(mapping):
    segs = [(s, e, (g - s) & 0xFFFF) for s, e, g in runs(sorted((c, g) for c, g in mapping.items() if c < 0xFFFF))]
    segs.append((0xFFFF, 0xFFFF, 1))
    n = len(segs)
    search = 2 * (1 << (n.bit_length() - 1))
    selector = n.bit_length() - 1
    body = struct.pack(">%dH" % n, *[e for _, e, _ in segs]) + b"\0\0"
    body += struct.pack(">%dH" % n, *[s for s, _, _ in segs])
    body += struct.pack(">%dH" % n, *[d for _, _, d in segs])
    body += struct.pack(">%dH" % n, *([0] * n))
    length = 14 + len(body)
    return struct.pack(">HHHHHHH", 4, length, 0, 2 * n, search, selector, 2 * n - search) + body


def format12(mapping):
    groups = runs(sorted(mapping.items()))
    body = b"".join(struct.pack(">III", s, e, g) for s, e, g in groups)
    return struct.pack(">HHIII", 12, 0, 16 + len(body), 0, len(groups)) + body


def checksum(b):
    b = b + b"\0" * (-len(b) % 4)
    return sum(struct.unpack(">%dI" % (len(b) // 4), b)) & 0xFFFFFFFF


def build(tables):
    num = len(tables)
    search = 16 * (1 << (num.bit_length() - 1))
    head = struct.pack(">IHHHH", 0x00010000, num, search, num.bit_length() - 1, num * 16 - search)
    offset = 12 + 16 * num
    records, blobs = b"", b""
    for tag, blob in tables:
        if tag == b"head":
            blob = blob[:8] + b"\0\0\0\0" + blob[12:]  # checkSumAdjustment is recomputed below
        records += struct.pack(">4sIII", tag, checksum(blob), offset, len(blob))
        padded = blob + b"\0" * (-len(blob) % 4)
        blobs += padded
        offset += len(padded)
    font = bytearray(head + records + blobs)
    adjust = (0xB1B0AFBA - checksum(bytes(font))) & 0xFFFFFFFF
    head_off = next(struct.unpack(">I", font[12 + 16 * i + 8: 12 + 16 * i + 12])[0] for i, (t, _) in enumerate(tables) if t == b"head")
    font[head_off + 8: head_off + 12] = struct.pack(">I", adjust)
    return bytes(font)


def strip(path):
    data = open(path, "rb").read()
    tables = parse_tables(data)
    cmap = next(b for t, b in tables if t == b"cmap")
    mapping = read_cmap(cmap)
    kept = {c: g for c, g in mapping.items() if c not in PUA}
    f4, f12 = format4(kept), format12(kept)
    # Encoding records: Unicode BMP and full, Windows BMP and full (sorted by platform, encoding).
    o4 = 4 + 8 * 4
    o12 = o4 + len(f4)
    new = struct.pack(">HH", 0, 4) + b"".join(struct.pack(">HHI", p, e, o) for p, e, o in [(0, 3, o4), (0, 4, o12), (3, 1, o4), (3, 10, o12)]) + f4 + f12
    for t in tables:
        if t[0] == b"cmap":
            t[1] = new
    out = build(tables)
    assert read_cmap(new) == kept, "round trip"
    open(path, "wb").write(out)
    print(f"{path}: {len(mapping) - len(kept)} private-use mappings removed, {len(kept)} kept")


if __name__ == "__main__":
    for p in sys.argv[1:]:
        strip(p)
