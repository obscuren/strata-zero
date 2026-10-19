"""Art reference for docs/superpowers/specs/2026-10-19-smg-sockets-design.md.

The mockups Jeffrey approved, as the code that drew them: the Ratchet SMG's
blueprint, the Hushcan and the Redeye sight in its sockets, and both pieces'
large blueprints. Pixels are half cells (a cell is two pixels stacked); `Pix.cells`
folds them into cells exactly as the build must: equal halves are a blank cell on
that colour, one empty half is a half block on the panel, else `▀` top on bottom.
Text and glyphs go over the folded cells, keeping the cell's background when they
give none. Ranges are half-open: rect(x0, y0, x1, y1) fills x0..x1-1, y0..y1-1.

Run it to print each drawing's cells: python3 2026-10-19-smg-sockets-art.py
"""

class Pix:
    def __init__(self, w, h_rows):
        self.w, self.h = w, h_rows * 2
        self.p = [[None] * w for _ in range(self.h)]
        self.text = []  # (cx, cy, s, fg, bg, bold)
        self.glyphs = []  # (cx, cy, ch, fg, bg)

    def px(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.p[y][x] = c

    def rect(self, x0, y0, x1, y1, c):
        """Fills [x0, x1) x [y0, y1)."""
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.px(x, y, c)

    def hline(self, x0, x1, y, c):
        self.rect(x0, y, x1, y + 1, c)

    def vline(self, x, y0, y1, c):
        self.rect(x, y0, x + 1, y1, c)

    def put(self, cx, cy, s, fg, bg=None, bold=False):
        self.text.append((cx, cy, s, fg, bg, bold))

    def glyph(self, cx, cy, ch, fg, bg=None):
        self.glyphs.append((cx, cy, ch, fg, bg))

    def cells(self):
        """{(cx, cy): (ch, fg, bg)}; bg None means the panel shows through."""
        out = {}
        for cy in range(self.h // 2):
            for cx in range(self.w):
                t, b = self.p[2 * cy][cx], self.p[2 * cy + 1][cx]
                if t is None and b is None:
                    continue
                if t == b:
                    out[(cx, cy)] = (" ", t, t)
                elif t is None:
                    out[(cx, cy)] = ("▄", b, None)
                elif b is None:
                    out[(cx, cy)] = ("▀", t, None)
                else:
                    out[(cx, cy)] = ("▀", t, b)
        for cx, cy, ch, fg, bg in self.glyphs:
            under = out.get((cx, cy))
            out[(cx, cy)] = (ch, fg, bg if bg else (under[2] if under else None))
        for cx, cy, s, fg, bg, bold in self.text:
            for i, ch in enumerate(s):
                under = out.get((cx + i, cy))
                b = bg if bg else (under[2] if under else None)
                out[(cx + i, cy)] = (ch, fg, b, bold)
        return out

    def onto(self, canvas, ox, oy, panel, keep=None):
        """Draws onto a Canvas at (ox, oy) cells."""
        for (cx, cy), cell in self.cells().items():
            ch, fg, bg = cell[0], cell[1], cell[2]
            bold = cell[3] if len(cell) > 3 else False
            x, y = ox + cx, oy + cy
            if 0 <= x < canvas.w and 0 <= y < canvas.h:
                canvas.cells[y][x] = [ch, fg, bg or panel, bold]


# ---- the Ratchet SMG and the pieces in its sockets

W, ROWS = 88, 15

# Gunmetal, stamped and worn.
GM = "#2e3138"
GM_HI = "#4a4f58"
GM_LO = "#1b1d22"
GM_EDGE = "#6b717b"
GM_LOWER = "#25282e"
GM_SEAM = "#3d424a"
WEAR = "#8d939c"
POLY = "#17181c"
POLY_HI = "#26282d"
BARREL = "#5b616a"
BARREL_HI = "#9aa1aa"
BARREL_LO = "#3f444c"
BLACK = "#0a0b0d"
STOCK = "#555a63"
STOCK_HI = "#7d838c"
PAD = "#121316"
TAPE = "#b0203c"
LABEL_TAPE = "#d9c9a0"
INK = "#1a1612"
TAPE_HI = "#d8475f"
# Connectors: gold, as the deck's.
GOLD = "#c9a227"
GOLD_HI = "#e8c54a"
GOLD_DK = "#8a6d14"
# Rail: bare aluminium.
ALU = "#b4bec8"
ALU_MID = "#7d8893"
ALU_DK = "#2b3138"

# Socket places, in cells: (name, x, y, w, h, kind)
MUZZLE = ("MUZZLE", 66, 4, 22, 5, "muzzle")
RAIL = ("RAIL", 34, 0, 24, 4, "rail")
SOCKETS = [MUZZLE, RAIL]


def gun(fitted=None):
    """The SMG; `fitted` maps a socket name to a piece drawn in it."""
    fitted = fitted or {}
    p = Pix(W, ROWS)
    # --- wire stock, extended, to the left
    p.hline(5, 24, 9, STOCK_HI)    # upper rod
    p.hline(5, 24, 16, STOCK)      # lower rod
    p.vline(14, 9, 17, STOCK)      # brace
    p.rect(21, 8, 24, 18, GM_LO)   # hinge block
    p.px(22, 10, WEAR)
    p.px(22, 15, WEAR)
    p.rect(2, 6, 6, 21, PAD)       # butt pad
    p.vline(5, 6, 21, STOCK)
    for y in range(7, 21, 2):
        p.px(3, y, POLY_HI)
    # --- receiver, a stamped box
    p.rect(24, 8, 60, 18, GM)
    p.rect(24, 16, 60, 18, GM_LOWER)
    p.hline(24, 60, 8, GM_HI)
    p.hline(25, 59, 9, GM_EDGE)
    p.hline(24, 60, 16, GM_SEAM)
    p.hline(24, 60, 17, GM_LO)
    p.vline(59, 9, 17, GM_HI)
    p.vline(24, 9, 17, GM_LO)
    # ejection port
    p.rect(45, 10, 55, 12, BLACK)
    p.hline(45, 55, 12, GM_HI)
    # rivets
    for x, y in [(26, 15), (57, 15), (26, 11), (42, 15)]:
        p.px(x, y, WEAR)
    # charging knob, top rear
    p.rect(27, 6, 31, 8, GM_HI)
    p.hline(27, 31, 6, WEAR)
    # --- barrel and its threads (the muzzle's connector)
    p.rect(60, 11, 66, 15, BARREL)
    p.hline(60, 66, 11, BARREL_HI)
    p.hline(60, 66, 13, BARREL_LO)
    p.hline(60, 66, 14, GM_LO)
    for x in range(66, 70):
        p.vline(x, 11, 15, GOLD_HI if x % 2 == 0 else GOLD_DK)
    # --- rail on top (the rail's connector)
    for x in range(34, 58):
        p.px(x, 6, ALU if x % 2 == 0 else None)
        p.px(x, 7, ALU_MID)
    # --- grip with the magazine inside, trigger and guard in front of it
    p.rect(38, 18, 48, 27, POLY)
    p.vline(38, 18, 27, POLY_HI)
    for y in range(19, 27, 2):
        p.hline(39, 47, y, POLY_HI)
    p.rect(38, 21, 48, 24, TAPE)  # grip wrap
    p.hline(38, 48, 21, TAPE_HI)
    p.rect(39, 27, 47, 28, GM_LO)  # magazine
    p.rect(35, 28, 51, 30, GM)     # its base plate
    p.hline(35, 51, 28, GM_HI)
    p.put(36, 14, " GOOD STUFF ", INK, LABEL_TAPE)
    p.vline(49, 18, 24, GM_HI)     # guard
    p.hline(49, 58, 23, GM_HI)
    p.vline(58, 18, 24, GM_HI)
    p.rect(52, 18, 54, 21, WEAR)   # trigger
    p.px(53, 21, WEAR)
    # --- stamping on the receiver
    p.put(28, 6, "RATCHET", WEAR, GM)
    p.put(28, 7, "9×19", GM_EDGE, GM)
    p.put(34, 7, "SN", GM_EDGE, GM)
    p.put(37, 7, "░░░░", GM_EDGE, GM)
    # the filed-down selector: a bright lever over two marks
    p.put(46, 7, "S", GM_EDGE, GM)
    p.put(48, 7, "▬", WEAR, GM)
    p.put(50, 7, "∞", GM_EDGE, GM)
    for name, art in fitted.items():
        art(p)
    return p


# --- muzzle hardware


CAN = "#1c1f24"
CAN_HI = "#3a3f47"
CAN_LO = "#0e1013"
CAN_INK = "#6b7480"
CAN_SHINE = "#5d6570"


def hushcan(p):
    """A fat suppressor on the threads: collar, ribbed body, end cap."""
    x0, x1, y0, y1 = 70, 88, 8, 18
    p.rect(x0, y0, x1, y1, CAN)
    p.hline(x0, x1, y0, CAN_HI)
    p.hline(x0, x1, y0 + 1, CAN_SHINE)
    p.rect(x0, y1 - 3, x1, y1 - 1, CAN_LO)
    p.hline(x0, x1, y1 - 1, BLACK)
    # knurled collar against the threads
    for x in range(x0, x0 + 3):
        for y in range(y0, y1):
            if (x + y) % 2 == 0:
                p.px(x, y, CAN_HI)
    p.vline(x0 + 3, y0, y1, CAN_LO)
    # rings
    for x in (79, 82, 85):
        p.vline(x, y0 + 2, y1 - 3, CAN_HI)
    # rounded end
    p.px(x1 - 1, y0, None)
    p.px(x1 - 1, y1 - 1, None)
    p.vline(x1 - 1, y0 + 1, y1 - 1, CAN_LO)
    p.put(74, 6, "HUSH", CAN_INK, CAN)


# --- rail hardware

SIGHT = "#15171b"
SIGHT_HI = "#3a3f47"
LENS = "#ff2a3c"
LENS_DK = "#5a0c14"


def redeye(p):
    """A reflex sight clamped to the rail: hood to the front, a red lens edge."""
    # clamp foot over the rail teeth
    p.rect(40, 4, 54, 6, ALU_DK)
    p.hline(40, 54, 4, ALU_MID)
    p.px(41, 5, ALU)
    # body
    p.rect(42, 1, 52, 4, SIGHT)
    p.hline(42, 52, 1, SIGHT_HI)
    # hood, taller, to the front
    p.rect(52, 0, 55, 4, SIGHT)
    p.hline(52, 55, 0, SIGHT_HI)
    p.vline(55, 0, 4, LENS)
    # turret knob
    p.rect(45, 0, 48, 1, SIGHT_HI)
    p.glyph(49, 1, "●", LENS, SIGHT)


# ---- the pieces drawn large

STICKER = "#e8e8ec"
BORE = "#050607"
CAVITY = "#121418"
BAFFLE = "#8d939c"
BAFFLE_HI = "#c3ccd5"
HATCH = "#3a3f47"
SOOT = "#2a2420"

HUSH_W, HUSH_ROWS = 72, 14


def hushcan_large(mods=("COOL +1",)):
    """Side view, muzzle end left (it screws on there), the lower half cut away to
    show the baffle stack."""
    p = Pix(HUSH_W, HUSH_ROWS)
    x0, x1 = 8, 70
    top, mid, bot = 2, 14, 26  # outer wall top, bore line, outer wall bottom
    # --- the outside, upper half: a shaded tube
    p.rect(x0, top, x1, mid, CAN)
    p.hline(x0, x1, top, CAN_HI)
    p.hline(x0, x1, top + 1, CAN_SHINE)
    p.hline(x0, x1, top + 2, CAN_HI)
    # rings turned into the tube
    for x in range(30, 66, 6):
        p.vline(x, top + 3, mid, CAN_HI)
        p.vline(x + 1, top + 3, mid, CAN_LO)
    # --- the cut, lower half: wall section hatched, bore channel, baffles
    p.rect(x0, mid, x1, bot, CAVITY)
    p.hline(x0 + 6, x1, mid + 1, BORE)
    for x in range(x0, x1):
        for y in (bot - 2, bot - 1):
            p.px(x, y, HATCH if (x + y) % 3 == 0 else CAN_LO)
    p.hline(x0, x1, mid, BAFFLE)  # the cut plane's edge
    # cone baffles: slanting from the wall up to the bore, opening to the muzzle end
    for bx in range(26, 64, 6):
        for i, y in enumerate(range(bot - 3, mid, -1)):
            x = bx - i // 2
            p.px(x, y, BAFFLE)
            p.px(x + 1, y, BAFFLE_HI if i % 2 == 0 else BAFFLE)
        # soot behind each baffle
        p.px(bx + 2, bot - 4, SOOT)
        p.px(bx + 3, bot - 4, SOOT)
    # the blast chamber behind the collar: empty, sooted
    for x in range(14, 22, 2):
        p.px(x, mid + 4, SOOT)
    # --- the collar: knurled, with the gold female thread inside
    p.rect(0, 1, x0 + 6, 27, CAN)
    for x in range(0, x0 + 6):
        for y in range(1, 14):
            if (x + y) % 2 == 0:
                p.px(x, y, CAN_HI)
    p.rect(0, 14, x0 + 6, 27, CAVITY)
    for x in range(0, x0 + 6):
        for y in range(24, 27):
            p.px(x, y, HATCH if (x + y) % 3 == 0 else CAN_LO)
    for x in range(0, 6):
        p.vline(x, 15, 22, GOLD_HI if x % 2 == 0 else GOLD_DK)
    p.hline(0, x0 + 6, 1, CAN_SHINE)
    p.hline(0, x0 + 6, 14, BAFFLE)
    # --- the end cap and its exit hole
    p.rect(x1, top + 1, x1 + 2, bot - 1, CAN_HI)
    p.px(x1 + 1, top + 1, None)
    p.px(x1 + 1, bot - 2, None)
    p.rect(x1, mid, x1 + 2, mid + 2, BORE)
    # --- writing
    p.put(17, 2, "HUSH", CAN_INK, CAN, bold=True)
    p.put(22, 2, "CAN", CAN_INK, CAN)
    p.put(17, 4, "9×19 · 1/2-28", CAN_INK, CAN)
    sticker = " " + mods[0] + " "
    p.put(52, 2, sticker, INK, STICKER, bold=True)
    p.put(26, 13, "▲ BAFFLE STACK", CAN_INK, None)
    p.put(0, 13, "▲ THREAD", CAN_INK, None)
    return p


# --- the Redeye sight

SIGHT_MID = "#22252b"
LENS_AMBER = "#ff8a3c"
RUBBER = "#0c0d0f"
RED_W, RED_ROWS = 52, 18


def redeye_large(mods=("REFLEX +1",)):
    p = Pix(RED_W, RED_ROWS)
    # a stub of rail under it, for scale
    for x in range(2, 50):
        p.px(x, 32, ALU if x % 2 == 0 else None)
        p.px(x, 33, ALU_MID)
    p.rect(2, 34, 50, 36, ALU_DK)
    # the clamp
    p.rect(6, 26, 46, 32, ALU_DK)
    p.hline(6, 46, 26, ALU_MID)
    for x in range(8, 46, 4):
        p.vline(x, 30, 32, BLACK)
    # cross-bolt knob, left
    p.rect(1, 25, 6, 31, ALU_MID)
    p.hline(1, 6, 25, ALU)
    p.vline(3, 26, 30, ALU_DK)
    # body
    p.rect(8, 10, 38, 26, SIGHT)
    p.hline(8, 38, 10, SIGHT_HI)
    p.hline(8, 38, 11, SIGHT_MID)
    p.hline(8, 38, 25, RUBBER)
    # hood, taller, to the front (right)
    p.rect(36, 3, 48, 26, SIGHT)
    p.hline(36, 48, 3, SIGHT_HI)
    p.vline(36, 4, 10, SIGHT_HI)
    # front glass, side-on: coated, red with an amber cast
    p.vline(48, 3, 26, LENS)
    p.vline(49, 4, 25, LENS_DK)
    p.px(48, 6, LENS_AMBER)
    p.px(48, 7, LENS_AMBER)
    # windage turret on top
    p.rect(18, 5, 27, 10, SIGHT_MID)
    p.hline(18, 27, 5, SIGHT_HI)
    for x in range(19, 27, 2):
        p.vline(x, 6, 10, SIGHT_HI)
    # battery cap: a disc with a coin slot
    for y in range(13, 23):
        for x in range(11, 21):
            dx, dy = x - 15.5, (y - 17.5)
            if dx * dx + dy * dy <= 22:
                p.px(x, y, SIGHT_MID)
    p.hline(13, 19, 17, BLACK)
    p.hline(13, 19, 18, SIGHT_HI)
    # writing
    p.put(23, 7, "REDEYE", "#cfd6dd", SIGHT, bold=True)
    p.put(23, 8, "2 MOA", "#6b7480", SIGHT)
    p.put(23, 10, "+", "#cfd6dd", SIGHT_MID)
    p.put(25, 10, "−", "#cfd6dd", SIGHT_MID)
    p.glyph(31, 10, "●", LENS, SIGHT)
    sticker = " " + mods[0] + " "
    p.put(37, 5, sticker, INK, STICKER, bold=True)
    return p


if __name__ == "__main__":
    def show(name, p):
        cells = p.cells()
        print(name, p.w, "x", p.h // 2)
        for cy in range(p.h // 2):
            print("".join(cells[(cx, cy)][0] if (cx, cy) in cells else "·" for cx in range(p.w)))
    show("ratchet bare", gun())
    show("ratchet fitted", gun({"MUZZLE": hushcan, "RAIL": redeye}))
    show("hushcan large", hushcan_large())
    show("redeye large", redeye_large())
