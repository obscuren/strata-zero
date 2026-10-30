"""Art reference for docs/superpowers/specs/2026-10-19-stickers-design.md.

The mockups Jeffrey approved, as the code that drew them: the 29 test stickers, how a
sticker looks at each turn, where stickers may lie on the pawn deck and the Ratchet,
and the sticker tray's layout. It is the truth for every pixel, glyph and colour.

Pixels are half cells (a cell is two pixels stacked); `cells()` folds them exactly as
`ui/pixels.rs` does: equal halves are a blank cell on that colour, one empty half is
a half block whose other side shows what is under it, else `▀` top on bottom. A
picture row is a string of one char per pixel: `.` is empty, any other char a key in
the picture's colours.

Run it to print every sticker at every turn: python3 2026-10-19-stickers-art.py
"""

# ---- turns

# A half turn: the glyph's turned form. Anything not here turns to itself (N O S X Z
# H I x o s z l 0 8 and the like are their own half turns).
TURN180 = {
    "A": "∀", "B": "ꓭ", "C": "Ɔ", "D": "ᗡ", "E": "Ǝ", "F": "Ⅎ", "G": "⅁", "J": "ſ",
    "K": "ꓘ", "L": "˥", "M": "W", "P": "Ԁ", "R": "ꓤ", "T": "⊥", "U": "∩", "V": "Λ",
    "W": "M", "Y": "⅄",
    "a": "ɐ", "b": "q", "c": "ɔ", "d": "p", "e": "ǝ", "f": "ɟ", "g": "ƃ", "h": "ɥ",
    "i": "ı", "j": "ɾ", "k": "ʞ", "m": "ɯ", "n": "u", "p": "d", "q": "b", "r": "ɹ",
    "t": "ʇ", "u": "n", "v": "ʌ", "w": "ʍ", "y": "ʎ", "6": "9", "9": "6",
    "‿": "⁀", "⁀": "‿", "(": ")", ")": "(", "<": ">", ">": "<", "^": "v",
    "↑": "↓", "↓": "↑", "←": "→", "→": "←", "▲": "▼", "▼": "▲", "◀": "▶", "▶": "◀",
    "▸": "◂", "◂": "▸", "▾": "▴", "▴": "▾",
    "▀": "▄", "▄": "▀", "▌": "▐", "▐": "▌", "◢": "◤", "◤": "◢", "◣": "◥", "◥": "◣",
}
# A quarter turn clockwise, for symbols with a true one. Letters and digits have none:
# they stand upright, stacked.
TURN90 = {
    "‿": "(", "⁀": ")", "(": "⁀", ")": "‿", "<": "^", ">": "v", "^": ">", "v": "<",
    "↑": "→", "→": "↓", "↓": "←", "←": "↑", "▲": "▶", "▶": "▼", "▼": "◀", "◀": "▲",
    "▸": "▾", "▾": "◂", "◂": "▴", "▴": "▸",
    "▀": "▐", "▐": "▄", "▄": "▌", "▌": "▀", "◢": "◣", "◣": "◤", "◤": "◥", "◥": "◢",
    "-": "|", "|": "-", "─": "│", "│": "─", "/": "\\", "\\": "/",
}
TURNS = [0, 90, 180, 270]


def quarter(ch, times):
    for _ in range(times % 4):
        ch = TURN90.get(ch, ch)
    return ch


# ---- the two kinds of sticker

class Lettered:
    """One row of text on a ground. Its text carries its own margin: a space each end."""

    def __init__(self, text, ink, ground, bold=True):
        self.text, self.ink, self.ground, self.bold = text, ink, ground, bold

    def cells(self, turn):
        """{(x, y): (glyph, fg, bg, bold)}. Level: the text. 180: reversed, each glyph
        turned. 90: a column three wide, one cell of text a row, top to bottom, the
        glyph upright (or quarter-turned when it has a true form) in the middle with
        ground either side. 270: the same, bottom to top, turned the other way."""
        t, out = self.text, {}
        if turn == 0:
            for i, ch in enumerate(t):
                out[(i, 0)] = (ch, self.ink, self.ground, self.bold)
        elif turn == 180:
            for i, ch in enumerate(reversed(t)):
                out[(i, 0)] = (TURN180.get(ch, ch), self.ink, self.ground, self.bold)
        else:
            seq, times = (t, 1) if turn == 90 else (t[::-1], 3)
            for i, ch in enumerate(seq):
                for dx in range(3):
                    glyph = quarter(ch, times) if dx == 1 else " "
                    out[(dx, i)] = (glyph, self.ink, self.ground, self.bold)
        return out


class Picture:
    """Half-block pixels; a turn turns the pixel grid, width and height swapped."""

    def __init__(self, rows, colours):
        self.rows, self.colours = rows, colours

    def grid(self, turn):
        g = [list(r) for r in self.rows]
        for _ in range((turn // 90) % 4):
            h = len(g)
            g = [[g[h - 1 - y][x] for y in range(h)] for x in range(len(g[0]))]
        return g

    def cells(self, turn):
        """{(x, y): (glyph, fg, bg or None, False)}; bg None: the cell under shows."""
        g = self.grid(turn)
        h, w = len(g), len(g[0])
        px = lambda x, y: None if y >= h or g[y][x] == "." else self.colours[g[y][x]]
        out = {}
        for cy in range((h + 1) // 2):
            for cx in range(w):
                t, b = px(cx, 2 * cy), px(cx, 2 * cy + 1)
                if t is None and b is None:
                    continue
                if t == b:
                    out[(cx, cy)] = (" ", t, t, False)
                elif t is None:
                    out[(cx, cy)] = ("▄", b, None, False)
                elif b is None:
                    out[(cx, cy)] = ("▀", t, None, False)
                else:
                    out[(cx, cy)] = ("▀", t, b, False)
        return out


def size(cells):
    return (max(x for x, _ in cells) + 1, max(y for _, y in cells) + 1)


def diecut(rows, edge="c"):
    """Grows a picture by a pixel all round and rings its shape in `edge`: the vinyl
    margin a die-cut sticker keeps. Content holds the rows this returns."""
    h, w = len(rows), len(rows[0])
    g = [["."] * (w + 2) for _ in range(h + 2)]
    for y in range(h):
        for x in range(w):
            g[y + 1][x + 1] = rows[y][x]
    out = [r[:] for r in g]
    for y in range(h + 2):
        for x in range(w + 2):
            if g[y][x] == "." and any(
                0 <= y + dy < h + 2 and 0 <= x + dx < w + 2 and g[y + dy][x + dx] != "."
                for dy in (-1, 0, 1) for dx in (-1, 0, 1)
            ):
                out[y][x] = edge
    return ["".join(r) for r in out]


def rgb(h):
    return (int(h[1:3], 16), int(h[3:5], 16), int(h[5:7], 16))


CREAM = "#f0e6d0"

# ---- the test pile: (id, name, description, art)

LETTERED = [
    ("sticker_no_future", "No Future",
     "Magenta vinyl, white caps. Half the pawn decks in the Slab wear one; nobody knows who printed the first.",
     Lettered(" NO FUTURE ", "#e8e8ec", "#ff00c8")),
    ("sticker_dead_face", "Dead face",
     "A hazard-yellow face with crosses for eyes, smiling about it anyway.",
     Lettered(" x‿x ", "#000000", "#ffcc00")),
    ("sticker_orodyne", "Orodyne",
     "Corporate red, a diamond and the name. Off a crate that fell off a truck that fell off a tower.",
     Lettered(" ◆ ORODYNE ", "#f4f4f4", "#c4141f")),
    ("sticker_arasu_tek", "Arasu·Tek",
     "Promo vinyl from a product launch nobody came back from.",
     Lettered(" ARASU·TEK ", "#c4141f", "#e8e8ec")),
    ("sticker_warranty_void", "Warranty void",
     "WARRANTY VOID IF REMOVED. Too late for that.",
     Lettered(" WARRANTY VOID ", "#111114", "#c8ccd4")),
    ("sticker_flatline", "Flatline",
     "Acid green, the colour of the last thing a monitor shows.",
     Lettered(" FLATLINE ", "#0a0b0d", "#7dff3a")),
    ("sticker_jack_in", "Jack in",
     "Cyan on black with an arrow, in case you forget which way the plug goes.",
     Lettered(" JACK IN ▸ ", "#00e5ff", "#00262c")),
    ("sticker_chrome_flesh", "Chrome > flesh",
     "A ripperdoc's giveaway. The clinic closed; the slogan didn't.",
     Lettered(" CHROME > FLESH ", "#e8e8ec", "#2e3138")),
    ("sticker_deadbeef", "0xDEADBEEF",
     "Amber hex, for anyone who reads memory dumps for fun.",
     Lettered(" 0xDEADBEEF ", "#ffb000", "#120c02")),
    ("sticker_soul_not_found", "Soul not found",
     "Error pink on black. Came free with a braindance.",
     Lettered(" SOUL NOT FOUND ", "#ff5fd2", "#14040f")),
    ("sticker_sudo", "sudo rm -rf /",
     "Terminal green. Never run it; always stick it.",
     Lettered(" sudo rm -rf / ", "#50e678", "#03110a", bold=False)),
    ("sticker_run_hot", "Run hot",
     "Safety orange. A pawn deck's motto, made official.",
     Lettered(" RUN HOT ", "#0a0b0d", "#ff6a00")),
    ("sticker_zero_day", "Zero day",
     "Electric blue, for the hole nobody has patched yet.",
     Lettered(" ZERO DAY ", "#e8e8ec", "#2a2ad0")),
    ("sticker_slab_kings", "Slab Kings",
     "Gang yellow on black. Wearing it outside their blocks is a statement.",
     Lettered(" SLAB KINGS ", "#ffcc00", "#0a0b0d")),
    ("sticker_ice_breaker", "Ice breaker",
     "Frost blue, the colour of a construct right before it cracks.",
     Lettered(" ICE BREAKER ", "#0a1a2a", "#9fe8ff")),
]

PICTURES = [
    ("sticker_chrome_skull", "Chrome skull",
     "Bone-white skull, red glass eyes. Classic for a reason.",
     Picture([
         "..wwwwww..",
         ".wwwwwwww.",
         "wwrrwwrrww",
         "wwrrwwrrww",
         "wwwwkkwwww",
         ".wwwwwwww.",
         "..wkwkwk..",
         "..wwwwww..",
     ], {"w": "#d8dce4", "r": "#ff2a3c", "k": "#111114"})),
    ("sticker_oni", "Oni",
     "A red demon mask, gold eyes, white fangs. Keeps the bad spirits in the ICE.",
     Picture(diecut([
         "h........h",
         "hh.rrrr.hh",
         ".rrrrrrrr.",
         "rryyrryyrr",
         "rrrrrrrrrr",
         "rkkkkkkkkr",
         "rkwkkkkwkr",
         ".rrrrrrrr.",
     ]), {"h": "#e8dcc0", "r": "#d01c2a", "y": "#ffcc00", "k": "#1a0508", "w": "#f4f4f4", "c": CREAM})),
    ("sticker_neko", "Neko",
     "Black cat, cyan eyes, magenta outline. Crosses your path on purpose.",
     Picture(diecut([
         "k........k",
         "kk......kk",
         "kkkkkkkkkk",
         "kcckkkkcck",
         "kkkkkkkkkk",
         "kkkkppkkkk",
         ".kkkkkkkk.",
         "..kkkkkk..",
     ], "m"), {"k": "#111114", "c": "#00e5ff", "p": "#ff5fd2", "m": "#ff00c8"})),
    ("sticker_high_voltage", "High voltage",
     "A warning triangle peeled off a substation door.",
     Picture([
         "....yy....",
         "...yyyy...",
         "...ykky...",
         "..yykkyy..",
         "..yykkyy..",
         ".yyyyyyyy.",
         ".yyykkyyy.",
         "yyyyyyyyyy",
     ], {"y": "#ffcc00", "k": "#0a0b0d"})),
    ("sticker_net_eye", "Net eye",
     "Magenta lids round a cyan iris. The net is watching; now so is your deck.",
     Picture([
         "...mmmmmm...",
         ".mmwwccwwmm.",
         "mwwccbbccwwm",
         ".mmwwccwwmm.",
         "...mmmmmm...",
     ], {"m": "#ff00c8", "w": "#e8e8ec", "c": "#00e5ff", "b": "#000000"})),
    ("sticker_outrun", "Outrun",
     "A striped sun over a magenta grid, sold at every arcade counter.",
     Picture([
         "pppppppppppp",
         "ppppyyyypppp",
         "pppyyyyyyppp",
         "ppyyyyyyyypp",
         "pppppppppppp",
         "ppoooooooopp",
         "pppppppppppp",
         "MMMMMMMMMMMM",
         "kkMkkkMkkkMk",
         "kMkkkkMkkkkM",
     ], {"p": "#2a0a4a", "y": "#ffcc00", "o": "#ff6a00", "M": "#ff00c8", "k": "#14051f"})),
    ("sticker_ramen", "Ramen",
     "Noodles, an egg and chopsticks. Free with the large.",
     Picture(diecut([
         ".......s.s",
         "......s.s.",
         "yyyyywwyyy",
         "rrrrrrrrrr",
         ".rrwwwwrr.",
         "..rrrrrr..",
         "...kkkk...",
     ]), {"s": "#c8a46a", "y": "#ffd66a", "w": "#f4f4f4", "r": "#d01c2a", "k": "#5a0c14", "c": CREAM})),
    ("sticker_stim", "Stim",
     "A red-and-white capsule. Doesn't work, tastes worse.",
     Picture([
         ".rrrwww.",
         "rRrrwwWw",
         "rrrrwwww",
         ".rrrwww.",
     ], {"r": "#e0203a", "R": "#ff7a8a", "w": "#e8e8ec", "W": "#ffffff"})),
    ("sticker_pulse", "Pulse",
     "A pink heart with a heartbeat through it. Still going.",
     Picture([
         ".ppp..ppp.",
         "pppppppppp",
         "ppppwppppp",
         "wwwwpwpwww",
         ".ppppwppp.",
         "..pppppp..",
         "...pppp...",
         "....pp....",
     ], {"p": "#ff2a6a", "w": "#ffffff"})),
    ("sticker_data_chip", "Data chip",
     "Gold pins, a cyan die. Holds nothing, looks expensive.",
     Picture([
         ".g.g.g.g.",
         "kkkkkkkkk",
         "kHHHHHHHk",
         "kHcccccHk",
         "kHcccccHk",
         "kHHHHHHHk",
         "kkkkkkkkk",
         ".g.g.g.g.",
     ], {"g": "#e8c54a", "k": "#1b1d22", "H": "#2e3138", "c": "#00e5ff"})),
    ("sticker_sakura", "Sakura",
     "One cherry blossom, pink as a neon sign in the rain.",
     Picture([
         "...pp...",
         ".p.pp.p.",
         "pppyyppp",
         ".ppyypp.",
         "..pppp..",
         ".pp..pp.",
     ], {"p": "#ffb7d5", "y": "#ffcc00"})),
    ("sticker_glitch_smile", "Glitch smile",
     "A yellow smiley out of register, magenta one side, cyan the other.",
     Picture([
         "...myyyyc...",
         "..myyyyyyc..",
         ".myykyykyyc.",
         ".myyyyyyyyc.",
         ".mykyyyykyc.",
         ".myykkkkyyc.",
         "..myyyyyyc..",
         "...myyyyc...",
     ], {"y": "#ffe600", "k": "#111114", "m": "#ff00c8", "c": "#00e5ff"})),
    ("sticker_scan_me", "Scan me",
     "A code that leads to an ad that leads to another code.",
     Picture(diecut([
         "kkkwkwkkk",
         "kwkwwwkwk",
         "kkkwkwkkk",
         "wwwkwkwww",
         "kwkkwkkwk",
         "wkwwkwwkw",
         "kkkwkkwkw",
         "kwkwwkwkk",
         "kkkwkwkwk",
     ], "w"), {"k": "#0a0b0d", "w": "#f4f4f4"})),
    ("sticker_corp_seal", "Corp seal",
     "A red disc on white, the seal off a shipping manifest.",
     Picture([
         "wwwwwwwwww",
         "wwwrrrrwww",
         "wwrrrrrrww",
         "wwrrrrrrww",
         "wwwrrrrwww",
         "wwwwwwwwww",
     ], {"w": "#f4f4f4", "r": "#c4141f"})),
]

STICKERS = LETTERED + PICTURES

# The stickers every new pawn deck copy is made with, where the paint is today:
# (sticker, x, y, turn) in drawing cells.
PAWN_FACTORY = [("sticker_no_future", 50, 10, 0), ("sticker_dead_face", 45, 13, 0)]

# ---- where stickers may lie: inclusive cell rects in drawing coordinates, the
# kept ones first, the dropped ones taken out of them.

# The pawn deck (84 x 22). The slab's body, less every part the inspect pointer stops
# on, the readouts, and what sits in the edge (sockets, jacks, cables lie outside it).
PAWN_KEEP = [(2, 5, 81, 16)]
PAWN_DROP = [
    (4, 5, 18, 5),     # the maker plate (Part::Plate)
    (48, 5, 67, 5),    # ● PWR  ● LINK  ● HOT
    (3, 6, 44, 13),    # the glass and its bezel
    (50, 7, 63, 8),    # TRACE: label, ten cells, the number
    (69, 6, 78, 12),   # the failsafe: its red lip, frame, cover and label
    # The ROM label and every ROM strip the deck shows on row 14: from x 4 to the
    # last strip's end (`Part::Rom` rects), 4..=29 with two ROMs.
]

# The Ratchet SMG (88 x 15).
RATCHET_KEEP = [
    (24, 4, 59, 8),    # the receiver's side, stamping included
    (38, 9, 47, 14),   # the grip, its wrap, the magazine and its base plate
    (2, 3, 5, 10),     # the stock's butt pad
]
RATCHET_DROP = [
    (45, 5, 54, 6),    # the ejection port
]


def mask(w, h, keep, drop):
    m = [[False] * w for _ in range(h)]
    for x0, y0, x1, y1 in keep:
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                m[y][x] = True
    for x0, y0, x1, y1 in drop:
        for y in range(y0, y1 + 1):
            for x in range(x0, x1 + 1):
                m[y][x] = False
    return m


def fits(art, turn, x, y, m):
    """Every cell the sticker writes lies on the area."""
    return all(
        0 <= y + cy < len(m) and 0 <= x + cx < len(m[0]) and m[y + cy][x + cx]
        for (cx, cy) in art.cells(turn)
    )


# ---- the tray (option A, "the tray rises"), relative to the body and the drawing

# The band spans the drawing's columns at the body's foot. Its height is the tallest
# sticker in the whole tray (level) plus four: a bracket row, the art rows, the count
# row, a bracket row, and the readout on the body's last row. Tiles start 13 columns
# into the drawing (the label STICKERS at 2), 3 apart, each as wide as its sticker
# (at least its count), art bottom-aligned on the count row, `×N` under it in the
# label's colour. `‹` at column 11 and `›` at the drawing's last column, on the band's
# first art row, when more lie off either side. The pointed tile's brackets sit one
# cell outside its art-and-count box.
TRAY_LABEL_X, TRAY_TILES_X, TRAY_GAP = 2, 13, 3
LABEL = "#6b6575"   # the label and the counts, as WAREZ's
MARK = "#8b8596"    # ‹ ›


def tray_rows(pack):
    return max(size(art.cells(0))[1] for art in pack) + 4


if __name__ == "__main__":
    for sid, name, desc, art in STICKERS:
        print(f"== {sid}  {name}")
        if isinstance(art, Picture):
            for r in art.rows:
                print("   ", r)
        for turn in TURNS:
            cells = art.cells(turn)
            w, h = size(cells)
            print(f"  {turn}°  {w}x{h}")
            for y in range(h):
                print("    " + "".join(cells.get((x, y), (" ",))[0] for x in range(w)))
    pawn = mask(84, 22, PAWN_KEEP, PAWN_DROP + [(4, 14, 29, 14)])
    for sid, x, y, turn in PAWN_FACTORY:
        art = next(a for s, _, _, a in STICKERS if s == sid)
        assert fits(art, turn, x, y, pawn), sid
    print("factory stickers fit the pawn deck")
