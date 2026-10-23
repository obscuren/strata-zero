"""The skills' card art: 34x10 cells, two pixels to a cell stacked (34x20 pixels).

Physical skills are hardware in the Ratchet's materials; abstract ones are neon signs
lit in the skill's stat colour, with one accent tube. `render` draws a skill learned or
not, by the same rules as the game (see 2026-10-23-skill-cards-design.md): unlearned
hardware goes through `grey`, an unlearned sign is cold glass with no halo. `export.py`
writes these drawings to flavor/skill_art.ron; `board.py` shows them all.

Ranges are half-open: rect(x0, y0, x1, y1) fills x0..x1-1, y0..y1-1.
"""

import math
from cells import Grid

W, ROWS = 34, 10
H = ROWS * 2
# The deck's panel. Drawn as a hole in hardware, it exports as an empty pixel.
PANEL = "#04060a"

# The Ratchet's materials.
GM, GM_HI, GM_LO, GM_EDGE = "#2e3138", "#4a4f58", "#1b1d22", "#6b717b"
WEAR, POLY, POLY_HI, BLACK = "#8d939c", "#17181c", "#26282d", "#0a0b0d"
BARREL, BARREL_HI, BARREL_LO = "#5b616a", "#9aa1aa", "#3f444c"
GOLD_HI, GOLD_DK = "#e8c54a", "#8a6d14"
ALU, ALU_MID = "#b4bec8", "#7d8893"
TAPE, TAPE_HI = "#b0203c", "#d8475f"
LABEL, LABEL_LO, INK = "#d9c9a0", "#bfae86", "#1a1612"

# The stats' colours, as the wizard has them.
STAT = {"BODY": "#78ff78", "REFLEX": "#00e5ff", "TECH": "#e6b43c", "COOL": "#ff00c8", "NET": "#6e96ff"}


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v))) for v in t)


def mix(a, b, t):
    """a toward b by t."""
    ra, rb = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(ra[k] + (rb[k] - ra[k]) * t for k in range(3)))


def grey(c, dark=0.6):
    """The colour unlit: three quarters toward its luma, then darkened."""
    r, g, b = hexrgb(c)
    l = 0.3 * r + 0.59 * g + 0.11 * b
    return rgbhex(tuple((0.25 * v + 0.75 * l) * dark for v in (r, g, b)))


class Pix:
    """W by 2*ROWS pixels, each empty (None) or a colour, and labels over the cells."""

    def __init__(self, w=W, rows=ROWS):
        self.w, self.h = w, rows * 2
        self.p = [[None] * w for _ in range(self.h)]
        self.text = []  # (cx, cy, text, ink, ground)

    def px(self, x, y, c):
        if 0 <= x < self.w and 0 <= y < self.h:
            self.p[y][x] = c

    def get(self, x, y):
        if 0 <= x < self.w and 0 <= y < self.h:
            return self.p[y][x]
        return None

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.px(x, y, c)

    def hline(self, x0, x1, y, c):
        self.rect(x0, y, x1, y + 1, c)

    def vline(self, x, y0, y1, c):
        self.rect(x, y0, x + 1, y1, c)

    def line(self, x0, y0, x1, y1, c):
        n = max(abs(x1 - x0), abs(y1 - y0), 1)
        for i in range(n + 1):
            self.px(round(x0 + (x1 - x0) * i / n), round(y0 + (y1 - y0) * i / n), c)

    def disc(self, cx, cy, r, c):
        for y in range(self.h):
            for x in range(self.w):
                if (x + 0.5 - cx) ** 2 + (y + 0.5 - cy) ** 2 <= r * r:
                    self.px(x, y, c)

    def put(self, cx, cy, s, ink, ground):
        self.text.append((cx, cy, s, ink, ground))

    def recolour(self, f):
        for y in range(self.h):
            for x in range(self.w):
                if self.p[y][x]:
                    self.p[y][x] = f(self.p[y][x])
        self.text = [(cx, cy, s, f(ink), f(ground)) for cx, cy, s, ink, ground in self.text]

    def grid(self, panel=PANEL):
        """Folded into cells as the game folds them, the labels over the cells."""
        g = Grid(self.w, self.h // 2, panel)
        for j in range(self.h // 2):
            for i in range(self.w):
                t, b = self.p[2 * j][i], self.p[2 * j + 1][i]
                if t is None and b is None:
                    g.c[j][i] = [" ", "#000000", panel, False]
                elif t == b:
                    g.c[j][i] = [" ", t, t, False]
                elif t is None:
                    g.c[j][i] = ["▄", b, panel, False]
                elif b is None:
                    g.c[j][i] = ["▀", t, panel, False]
                else:
                    g.c[j][i] = ["▀", t, b, False]
        for cx, cy, s, ink, ground in self.text:
            for k, ch in enumerate(s):
                if 0 <= cx + k < self.w and 0 <= cy < self.h // 2:
                    g.c[cy][cx + k] = [ch, ink, ground, False]
        return g


# Brass, grip tape, ICE and steel.
BRASS_HI, BRASS, BRASS_LO, BRASS_DK = "#e8c86e", "#b8923c", "#7a5c22", "#4e3a14"
GRIP, GRIP_HI = "#9b1a7a", "#d23aa8"
ICE_HI, ICE_LT, ICE, ICE_LO, ICE_DK = "#e6faff", "#b4e8f6", "#7ccbe6", "#3f8fb0", "#123a4c"
STEEL_HI, STEEL, STEEL_LO = "#c9d0d8", "#8e969f", "#5a616a"
RUBBER, RUBBER_HI = "#1f2126", "#33363d"
PCB, PCB_HI, PCB_LO, TRACE = "#1d5a3c", "#2b7a52", "#123a27", "#c9a640"
RUST_HI, RUST, RUST_LO = "#c27a44", "#8f4a22", "#5a2c14"
DUCT_HI, DUCT, DUCT_LO = "#b9bec5", "#8c929a", "#61666e"
WHITE_HI, WHITE, WHITE_LO = "#eef1f4", "#c8ced6", "#959ca6"
RED, RED_HI = "#c8243a", "#ef4a5e"
LED_G, LED_C = "#3dff8a", "#00e5ff"


# ---------------------------------------------------------------- hardware

def ice_breaker(p):
    # The block: a top face, a front with one lit facet, a shadowed side.
    for y in range(3, 6):
        p.hline(18 + (6 - y), 31 + (6 - y), y, ICE_HI)
    p.rect(18, 6, 31, 20, ICE)
    for y in range(6, 20):
        for x in range(18, 31):
            if (x - 18) + (y - 6) < 7:
                p.px(x, y, ICE_LT)
    for x in range(31, 34):
        p.vline(x, 6 - (x - 31), 20 - (x - 31), ICE_LO)
    p.hline(18, 31, 19, ICE_LO)
    # The crack, from where the pick bit, with its lit lip under it.
    crack = [(18, 11), (19, 11), (20, 12), (21, 12), (22, 11), (23, 11), (24, 12), (25, 13),
             (26, 13), (27, 14), (28, 14), (29, 15), (23, 10), (24, 9), (25, 8), (26, 8), (27, 7),
             (24, 13), (24, 14), (25, 15), (25, 16)]
    for x, y in crack:
        if p.get(x, y + 1) not in (None,) and (x, y + 1) not in crack:
            p.px(x, y + 1, ICE_HI)
    for x, y in crack:
        p.px(x, y, ICE_DK)
    for x, y in [(28, 9), (21, 16), (29, 4)]:
        p.px(x, y, "#ffffff")
    # The ice axe: a shaft three pixels wide, taped low, a spike at its foot.
    for i in range(15):
        x, y = 4 + i // 3, 19 - i
        p.px(x, y, POLY_HI)
        p.px(x + 1, y, POLY)
        p.px(x + 2, y, GM_LO)
    for i in range(2, 7):
        x, y = 4 + i // 3, 19 - i
        p.px(x, y, TAPE_HI)
        p.px(x + 1, y, TAPE)
        p.px(x + 2, y, TAPE)
    p.px(3, 19, STEEL_HI)
    # The head across the shaft's top: the adze to the left, the pick curving down
    # into the crack.
    p.rect(5, 3, 13, 6, STEEL)
    p.hline(5, 13, 3, STEEL_HI)
    p.hline(6, 13, 5, STEEL_LO)
    p.rect(3, 3, 5, 5, STEEL_HI)
    for x, y in [(13, 4), (14, 5), (15, 6), (16, 7), (17, 8), (17, 9), (18, 10)]:
        p.px(x, y, STEEL_HI)
        p.px(x, y + 1, STEEL_LO)
    p.px(18, 11, STEEL_HI)
    # Chips of ICE in the air.
    for x, y, c in [(15, 10, ICE_HI), (14, 12, ICE_LT), (16, 14, ICE_HI), (13, 9, ICE),
                    (15, 16, ICE), (12, 13, ICE_HI)]:
        p.px(x, y, c)


def vault_run(p):
    # The door's frame, the round door with its lit rim, its bolts.
    import math
    p.disc(16, 10, 10.0, GM_LO)
    p.disc(16, 10, 9.0, GM)
    p.disc(16, 10, 7.6, STEEL)
    for y in range(p.h):
        for x in range(p.w):
            d = math.hypot(x + 0.5 - 16, y + 0.5 - 10)
            if 7.6 < d <= 9.0 and (x + 0.5 - 16) + (y + 0.5 - 10) < -2:
                p.px(x, y, GM_EDGE)
            if 6.4 < d <= 7.6:
                p.px(x, y, STEEL_LO if (x + 0.5 - 16) + (y + 0.5 - 10) > 0 else STEEL_HI)
    for k in range(8):
        a = k * math.pi / 4 + math.pi / 8
        p.px(math.floor(16 + 8.3 * math.cos(a)), math.floor(10 + 8.3 * math.sin(a)), BARREL_HI)
    # The wheel: four spokes with knobs round a hub.
    p.rect(18, 9, 21, 11, STEEL_HI)
    p.rect(11, 9, 14, 11, STEEL_HI)
    p.rect(15, 12, 17, 15, STEEL_HI)
    p.rect(15, 5, 17, 8, STEEL_HI)
    p.rect(21, 8, 23, 12, ALU)
    p.rect(9, 8, 11, 12, ALU)
    p.rect(14, 15, 18, 17, ALU)
    p.rect(14, 3, 18, 5, ALU)
    p.disc(16, 10, 2.3, ALU)
    p.disc(16, 10, 1.0, BARREL_LO)
    # The hinges on the left.
    p.rect(3, 5, 6, 8, GM_LO)
    p.rect(3, 12, 6, 15, GM_LO)
    p.hline(3, 6, 5, GM_EDGE)
    p.hline(3, 6, 12, GM_EDGE)
    # What came back out: a gold bar and a shard.
    p.rect(26, 15, 33, 18, GOLD_DK)
    p.rect(27, 15, 33, 17, GOLD_HI)
    p.hline(27, 33, 15, "#fff1a8")
    p.rect(29, 12, 31, 15, "#ff00c8")
    p.px(29, 12, "#ff8ae6")


def handguns(p):
    # The slide: sights, serrations, the port.
    p.rect(6, 4, 29, 9, GM)
    p.hline(6, 29, 4, GM_HI)
    p.hline(6, 29, 8, GM_LO)
    p.rect(7, 3, 9, 4, GM_EDGE)
    p.px(27, 3, GM_EDGE)
    for x in (8, 10, 12):
        p.vline(x, 5, 8, GM_LO)
    p.rect(17, 5, 22, 7, BLACK)
    p.hline(17, 22, 7, GM_HI)
    p.px(28, 4, GM)
    # The barrel's crown.
    p.rect(29, 5, 31, 8, BARREL)
    p.hline(29, 31, 5, BARREL_HI)
    p.vline(31, 5, 8, BARREL_LO)
    # The frame with its rail, the guard and the trigger.
    p.rect(11, 9, 28, 11, POLY)
    p.hline(11, 28, 9, POLY_HI)
    for x in (21, 23, 25):
        p.px(x, 10, GM_LO)
    p.hline(15, 23, 14, POLY_HI)
    p.vline(23, 11, 14, POLY_HI)
    p.px(22, 13, POLY_HI)
    p.vline(17, 11, 13, WEAR)
    p.px(18, 13, WEAR)
    # The grip, raked, taped, on its base plate.
    for y in range(9, 19):
        x0 = 7 - (y - 9) // 3
        p.hline(x0, x0 + 8, y, POLY)
        p.px(x0, y, POLY_HI)
        if y % 2 == 0:
            p.px(x0 + 3, y, POLY_HI)
            p.px(x0 + 6, y, POLY_HI)
    for y in (13, 14):
        x0 = 7 - (y - 9) // 3
        p.hline(x0, x0 + 8, y, TAPE_HI if y == 13 else TAPE)
    p.hline(3, 13, 19, GM)
    p.hline(3, 13, 18, GM_HI)
    # A spent case leaving the port.
    p.px(20, 2, GOLD_HI)
    p.px(21, 2, GOLD_HI)
    p.px(21, 1, GOLD_DK)


def lockpick(p):
    # The shackle.
    for y in range(3, 10):
        p.px(11, y, ALU)
        p.px(12, y, ALU_MID)
        p.px(21, y, ALU)
        p.px(22, y, ALU_MID)
    p.hline(13, 21, 1, ALU)
    p.hline(13, 21, 2, ALU_MID)
    p.px(12, 2, ALU)
    p.px(21, 2, ALU)
    # The body: lit top and left, shadowed right and foot, rivets.
    p.rect(8, 9, 26, 20, BRASS)
    p.hline(8, 26, 9, BRASS_HI)
    p.vline(8, 9, 20, BRASS_HI)
    p.vline(25, 10, 20, BRASS_LO)
    p.hline(9, 26, 19, BRASS_LO)
    p.hline(9, 25, 11, BRASS_LO)
    p.hline(9, 25, 10, "#d4ae56")
    for x in (10, 23):
        p.px(x, 17, BRASS_DK)
    # The keyway.
    p.disc(17, 13.6, 1.3, BLACK)
    p.rect(16, 14, 18, 17, BLACK)
    # The tension wrench, in the foot of the keyway and out to the left.
    p.hline(3, 17, 16, STEEL_HI)
    p.vline(3, 16, 20, STEEL)
    # The pick from the right: its hook in the keyway, its grip taped in the stat's colour.
    p.hline(18, 28, 14, STEEL_HI)
    p.px(17, 13, STEEL_HI)
    p.rect(28, 13, 34, 16, GRIP)
    p.hline(28, 34, 13, GRIP_HI)


def hacking(p):
    # The port: a slab of deck with its socket and lights.
    p.rect(24, 2, 34, 19, GM)
    p.hline(24, 34, 2, GM_HI)
    p.vline(24, 3, 19, GM_EDGE)
    p.hline(25, 34, 18, GM_LO)
    p.rect(24, 8, 28, 13, BLACK)
    p.hline(25, 28, 13, GM_HI)
    for y, c in [(4, LED_C), (6, LED_G)]:
        p.px(30, y, c)
        p.px(31, y, GM_LO)
    for y in (15, 16):
        p.hline(27, 33, y, GM_LO if y == 15 else GM_HI)
    # The plug: ribbed body, an aluminium collar, gold pins into the socket.
    p.rect(12, 7, 21, 14, POLY)
    p.hline(12, 21, 7, POLY_HI)
    p.hline(12, 21, 13, BLACK)
    for x in (13, 15, 17):
        p.vline(x, 8, 13, POLY_HI)
    p.rect(21, 6, 23, 15, ALU)
    p.vline(22, 6, 15, ALU_MID)
    p.rect(23, 9, 27, 12, GOLD_HI)
    p.hline(23, 27, 11, GOLD_DK)
    # The cable, three wide, out to the left and down, a NET-blue stripe along it.
    for x in range(5, 12):
        p.vline(x, 9, 12, RUBBER)
        p.px(x, 9, RUBBER_HI)
        p.px(x, 10, "#3b5bb0")
    for k in range(9):
        x, y = 4 - k // 3, 11 + k
        p.hline(x, x + 3, y, RUBBER)
        p.px(x + 1, y, "#3b5bb0")
    # Data in the socket.
    p.px(27, 10, LED_C)


def electronics(p):
    # The board: traces, a chip, a capacitor.
    p.rect(1, 9, 24, 19, PCB)
    p.hline(1, 24, 9, PCB_HI)
    p.hline(1, 24, 18, PCB_LO)
    for x0, y0, x1, y1 in [(3, 11, 9, 11), (9, 11, 9, 16), (9, 16, 21, 16), (14, 11, 22, 11),
                           (3, 14, 6, 14)]:
        p.line(x0, y0, x1, y1, TRACE)
    p.rect(11, 12, 18, 15, BLACK)
    p.hline(11, 18, 12, "#2a2c31")
    for x in range(11, 18, 2):
        p.px(x, 11, ALU_MID)
        p.px(x, 15, ALU_MID)
    p.disc(4.5, 16.5, 1.6, "#3a63c4")
    p.px(4, 15, "#7fa0f0")
    # A joint going down under the tip, silver and bright.
    p.px(21, 11, WHITE_HI)
    p.px(22, 11, ALU)
    # The iron: grip, guard, shaft, tip on the joint, a thread of smoke.
    for k in range(9):
        x, y = 33 - k, 1 + k // 2
        p.px(x, y - 1, "#f2cf6a")
        p.px(x, y, "#e6b43c")
        p.px(x, y + 1, "#a87a1c")
        if k in (2, 5):
            p.px(x, y, "#5a4010")
            p.px(x, y + 1, "#5a4010")
    for k in range(4):
        p.px(24 - k, 5 + k // 2, STEEL_HI)
        p.px(24 - k, 6 + k // 2, STEEL)
    p.px(22, 10, STEEL_HI)
    p.px(21, 10, "#ffd27a")
    for x, y in [(20, 8), (21, 7), (20, 6), (21, 5)]:
        p.px(x, y, "#5a6070")


def scrapcraft(p):
    # Two rusted pipe ends, taped together.
    for x0, x1 in [(1, 15), (19, 33)]:
        p.rect(x0, 5, x1, 11, RUST)
        p.hline(x0, x1, 5, RUST_HI)
        p.hline(x0, x1, 10, RUST_LO)
    for x, y in [(4, 7), (9, 8), (25, 7), (30, 9), (12, 6)]:
        p.px(x, y, RUST_LO)
    p.rect(1, 6, 2, 10, BLACK)
    p.rect(13, 4, 21, 12, DUCT)
    p.hline(13, 21, 4, DUCT_HI)
    p.hline(13, 21, 11, DUCT_LO)
    for x in (15, 18):
        p.vline(x, 5, 11, DUCT_LO)
    p.px(21, 12, DUCT)
    p.px(22, 13, DUCT_LO)
    # The wrench under it, its jaw open.
    p.rect(4, 15, 26, 17, STEEL)
    p.hline(4, 26, 15, STEEL_HI)
    p.rect(26, 13, 31, 19, STEEL)
    p.hline(26, 31, 13, STEEL_HI)
    p.rect(28, 15, 32, 17, None or "#04060a")
    p.rect(2, 14, 6, 18, STEEL_LO)
    p.disc(3.5, 16, 1.0, "#04060a")


def longarm(p):
    # The stock, the receiver, the barrel.
    p.rect(0, 9, 8, 14, POLY)
    p.hline(0, 8, 9, POLY_HI)
    p.rect(0, 14, 3, 15, POLY)
    p.rect(8, 9, 21, 13, GM)
    p.hline(8, 21, 9, GM_HI)
    p.hline(8, 21, 12, GM_LO)
    p.rect(21, 10, 34, 12, BARREL)
    p.hline(21, 34, 10, BARREL_HI)
    p.rect(31, 9, 34, 13, GM_LO)
    # The scope on its rings.
    p.rect(9, 4, 22, 7, BLACK)
    p.hline(9, 22, 4, GM_HI)
    p.rect(7, 3, 10, 8, GM)
    p.rect(21, 3, 24, 8, GM)
    p.px(23, 5, LED_C)
    p.vline(12, 7, 9, GM_EDGE)
    p.vline(19, 7, 9, GM_EDGE)
    # The magazine, the grip, the trigger.
    p.rect(14, 13, 18, 18, GM_LO)
    p.vline(14, 13, 18, GM_HI)
    for y in range(13, 18):
        p.hline(9 - (y - 13) // 2, 12 - (y - 13) // 2, y, POLY)
    p.rect(9, 15, 12, 16, TAPE)
    p.hline(12, 14, 15, POLY_HI)
    p.px(13, 13, WEAR)


def blades(p):
    # The blade up and to the right: a lit edge, a wide flat, a dark spine, a clip point.
    for k in range(21):
        x, y = 12 + k, 14 - (k * 12) // 20
        width = 4 if k < 15 else max(1, 4 - (k - 14))
        for d in range(width):
            c = [WHITE_HI, STEEL_HI, STEEL, STEEL_LO][d] if d < 4 else STEEL_LO
            p.px(x, y - d, c)
    p.px(33, 1, WHITE_HI)
    for k in range(3, 12, 4):
        x, y = 12 + k, 14 - (k * 12) // 20
        p.px(x, y - 2, STEEL_LO)
    # The guard and the handle, wrapped.
    p.rect(10, 12, 12, 18, GM_EDGE)
    for k in range(9):
        x, y = 9 - k, 15 + (k * 4) // 9
        p.px(x, y, RUBBER_HI)
        p.px(x, y + 1, RUBBER)
        p.px(x, y - 1, RUBBER)
        if k % 2:
            p.px(x, y, TAPE)
    p.rect(0, 18, 2, 20, GM)


def close_work(p):
    # Brass knuckles: four rings over a grip bar, lit from the top left.
    for k in range(4):
        cx = 8 + k * 6
        p.disc(cx, 8, 3.6, BRASS)
        p.disc(cx - 0.6, 7.4, 3.0, BRASS_HI)
        p.disc(cx, 8, 2.9, BRASS)
        p.disc(cx, 8.3, 1.8, "#04060a")
    p.rect(4, 11, 30, 14, BRASS)
    p.hline(4, 30, 11, BRASS_HI)
    p.hline(5, 29, 13, BRASS_LO)
    # The palm bar, rounded at the ends.
    p.rect(8, 15, 26, 18, BRASS)
    p.hline(8, 26, 15, BRASS_HI)
    p.hline(9, 25, 17, BRASS_LO)
    p.rect(6, 14, 8, 17, BRASS_LO)
    p.rect(26, 14, 28, 17, BRASS_LO)
    # A scuff of blood on a knuckle.
    p.px(21, 5, RED)
    p.px(22, 5, RED_HI)


def fixers_ear(p):
    # A burner: a scratched slab, a lit screen with the job on it, keys.
    p.rect(10, 2, 22, 20, RUBBER)
    p.hline(10, 22, 2, RUBBER_HI)
    p.vline(10, 3, 20, RUBBER_HI)
    p.rect(12, 4, 20, 10, "#1b3a2a")
    p.put(12, 2, " JOB? ", "#3dff8a", "#1b3a2a")
    p.hline(12, 20, 9, "#2c5a42")
    for y in (12, 14, 16):
        for x in (12, 15, 18):
            p.rect(x, y, x + 2, y + 1, "#3b3f47")
    p.px(15, 18, "#3b3f47")
    p.px(16, 18, "#3b3f47")
    # Its antenna stub and the signal.
    p.rect(19, 0, 21, 2, RUBBER_HI)
    for r, c in [(4, "#ff00c8"), (7, "#a8008a"), (10, "#5a0048")]:
        for a in range(-60, 61, 6):
            t = math.radians(a)
            p.px(round(21 + r * math.cos(t)), round(3 + r * math.sin(t)), c)


def medtech(p):
    # A hard case: lid, body, latches, a handle, the cross.
    p.rect(5, 6, 29, 19, WHITE)
    p.hline(5, 29, 6, WHITE_HI)
    p.vline(5, 7, 19, WHITE_HI)
    p.hline(6, 29, 18, WHITE_LO)
    p.vline(28, 7, 18, WHITE_LO)
    p.hline(5, 29, 10, WHITE_LO)
    p.rect(12, 2, 22, 4, GM)
    p.rect(12, 4, 14, 6, GM)
    p.rect(20, 4, 22, 6, GM)
    p.hline(12, 22, 2, GM_HI)
    for x in (8, 25):
        p.rect(x, 9, x + 2, 12, GM_EDGE)
    p.rect(15, 11, 19, 18, RED)
    p.rect(13, 13, 21, 16, RED)
    p.hline(15, 19, 11, RED_HI)
    p.hline(13, 15, 13, RED_HI)
    p.hline(19, 21, 13, RED_HI)


def field_patch(p):
    # A roll of tape, its strip pulled out across.
    p.disc(10, 10, 8.4, DUCT_LO)
    p.disc(9.4, 9.4, 7.6, DUCT)
    p.disc(8.6, 8.4, 4.2, DUCT_HI)
    p.disc(9.4, 9.4, 3.8, DUCT)
    p.disc(10, 10, 3.6, "#7a6a50")
    p.disc(10, 10, 2.6, "#04060a")
    p.rect(17, 15, 33, 18, DUCT)
    p.hline(17, 33, 15, DUCT_HI)
    for x in range(18, 33, 3):
        p.px(x, 17, DUCT_LO)
    # A gel patch on top of the strip.
    p.rect(22, 4, 32, 13, LABEL)
    p.hline(22, 32, 4, "#efe3c2")
    p.hline(22, 32, 12, LABEL_LO)
    p.rect(24, 6, 30, 11, "#41c98a")
    p.hline(24, 30, 6, "#8af0bf")
    p.rect(26, 7, 28, 10, "#e8fff3")


HARDWARE = {
    "hacking": hacking, "ice_breaker": ice_breaker, "vault_run": vault_run,
    "electronics": electronics, "scrapcraft": scrapcraft,
    "handguns": handguns, "longarm": longarm,
    "blades": blades, "close_work": close_work,
    "lockpick": lockpick, "fixers_ear": fixers_ear,
    "medtech": medtech, "field_patch": field_patch,
}


# ---------------------------------------------------------------- neon

class Sign:
    """A neon sign: tube pixels in the main colour and in an accent."""

    def __init__(self):
        self.tubes = {}

    def px(self, x, y, accent=False):
        if 0 <= x < W and 0 <= y < H:
            self.tubes[(x, y)] = accent

    def line(self, x0, y0, x1, y1, accent=False):
        n = max(abs(x1 - x0), abs(y1 - y0), 1)
        for i in range(n + 1):
            self.px(round(x0 + (x1 - x0) * i / n), round(y0 + (y1 - y0) * i / n), accent)

    def poly(self, pts, closed=True, accent=False):
        for a, b in zip(pts, pts[1:] + (pts[:1] if closed else [])):
            self.line(*a, *b, accent)

    def arc(self, cx, cy, rx, ry, a0, a1, accent=False, step=3):
        for a in range(a0, a1 + 1, step):
            t = math.radians(a)
            self.px(round(cx + rx * math.cos(t)), round(cy + ry * math.sin(t)), accent)


def overclock(s):
    s.arc(17, 17, 13, 13, 180, 310, step=1)
    s.arc(17, 17, 13, 13, 312, 360, True, 1)
    for a in (210, 250, 290):
        t = math.radians(a)
        s.line(round(17 + 10 * math.cos(t)), round(17 + 10 * math.sin(t)),
               round(17 + 11 * math.cos(t)), round(17 + 11 * math.sin(t)))
    t = math.radians(-22)
    s.line(17, 17, round(17 + 10 * math.cos(t)), round(17 + 10 * math.sin(t)), True)
    s.line(4, 17, 30, 17)


def snapshot(s):
    s.arc(20, 10, 6.5, 6.5, 0, 360, step=2)
    for x0, y0, x1, y1 in [(20, 1, 20, 4), (20, 16, 20, 19), (11, 10, 14, 10), (26, 10, 29, 10)]:
        s.line(x0, y0, x1, y1)
    s.px(20, 10, True)
    for y, x0 in [(6, 3), (10, 1), (14, 3)]:
        s.line(x0, y, x0 + 5, y, True)


def riposte(s):
    s.line(5, 17, 27, 3)
    s.line(7, 3, 29, 17)
    s.line(7, 13, 10, 16)
    s.line(25, 16, 28, 13)
    for x, y in [(17, 6), (17, 5), (14, 9), (13, 9), (20, 9), (21, 9), (17, 13)]:
        s.px(x, y, True)


def stealth(s):
    # A closed eye: the lid's curve, its crease above, the lashes hanging down.
    s.arc(17, 3, 13, 8, 15, 165, step=1)
    s.arc(17, 1, 11, 5, 40, 140, True, 2)
    for a in (35, 60, 90, 120, 145):
        t = math.radians(a)
        x, y = 17 + 13 * math.cos(t), 3 + 8 * math.sin(t)
        s.line(round(x), round(y), round(x + 3.4 * math.cos(t)), round(y + 3.4 * math.sin(t)))


def vanish(s):
    s.arc(9, 5, 2.5, 2.5, 0, 360, step=15)
    s.arc(9, 15, 6, 6, 180, 360, step=4)
    s.line(3, 15, 3, 18)
    s.line(15, 15, 15, 18)
    for k, x in enumerate((19, 25, 30)):
        for y in range(3 + k, 18, 2 + k):
            s.px(x, y, True)
            if k == 0:
                s.px(x + 1, y, True)


def streetwise(s):
    s.poly([(4, 3), (24, 3), (30, 7), (24, 11), (4, 11)])
    s.line(8, 11, 8, 19)
    s.line(9, 11, 9, 19)
    s.line(5, 19, 12, 19)
    for x, y in [(8, 8), (9, 6), (10, 7), (11, 8), (12, 6), (13, 7), (14, 8), (15, 6), (16, 7),
                 (17, 8), (18, 7), (19, 6)]:
        s.px(x, y, True)


def haggle(s):
    s.poly([(3, 6), (20, 6), (26, 11), (20, 16), (3, 16)])
    s.arc(21.5, 11, 1.4, 1.4, 0, 360, step=30)
    s.arc(10, 11, 3, 3, 0, 360, step=12, accent=True)
    for dx, dy in [(-1, -1), (1, -1), (-1, 1), (1, 1)]:
        s.px(10 + 4 * dx, 11 + 4 * dy, True)
    s.line(30, 3, 30, 15, True)
    s.line(27, 12, 30, 15, True)
    s.line(33, 12, 30, 15, True)


def stim_cocktail(s):
    s.poly([(6, 3), (28, 3), (17, 12)])
    s.line(17, 12, 17, 17)
    s.line(11, 18, 23, 18)
    s.line(10, 6, 24, 6, True)
    s.line(22, 1, 25, 9)
    for x, y in [(20, 0), (21, 0), (22, 0), (23, 0), (19, 1), (24, 1)]:
        s.px(x, y, True)


def athletics(s):
    s.arc(19, 3, 2, 2, 0, 360, step=20)
    s.line(18, 6, 15, 12)
    s.line(17, 7, 22, 9)
    s.line(22, 9, 25, 7)
    s.line(17, 7, 12, 9)
    s.line(15, 12, 20, 15)
    s.line(20, 15, 19, 19)
    s.line(15, 12, 10, 15)
    s.line(10, 15, 6, 14)
    for y in (6, 10, 14):
        s.line(2, y, 6, y, True)


def freerun(s):
    # The wall, in bricks.
    s.poly([(20, 12), (33, 12), (33, 19), (20, 19)], accent=True)
    s.line(20, 15, 33, 15, True)
    for x in (24, 29):
        s.line(x, 12, x, 15, True)
    s.line(26, 15, 26, 19, True)
    # A runner flat over it: head, a long body, one hand on the wall, legs trailing.
    s.arc(24, 5, 1.8, 1.8, 0, 360, step=20)
    s.line(21, 6, 12, 8)
    s.line(20, 7, 22, 11)
    s.line(18, 7, 21, 3)
    s.line(12, 8, 7, 6)
    s.line(12, 8, 6, 11)
    # The arc it took.
    for x, y in [(2, 17), (3, 15), (4, 14), (6, 13)]:
        s.px(x, y, True)


def second_wind(s):
    s.arc(12, 7, 5, 4.5, 150, 360, step=4)
    s.arc(22, 7, 5, 4.5, 180, 390, step=4)
    s.line(8, 10, 17, 18)
    s.line(26, 10, 17, 18)
    s.poly([(1, 11), (9, 11), (11, 8), (14, 15), (17, 4), (19, 11), (33, 11)], closed=False, accent=True)


NEON = {
    "overclock": (overclock, "TECH", "#ff3355"), "snapshot": (snapshot, "REFLEX", "#ffffff"),
    "riposte": (riposte, "REFLEX", "#fff27a"), "stealth": (stealth, "COOL", "#ff7ae0"),
    "vanish": (vanish, "COOL", "#a8008a"), "streetwise": (streetwise, "COOL", "#00e5ff"),
    "haggle": (haggle, "COOL", "#e6b43c"), "stim_cocktail": (stim_cocktail, "TECH", "#ff00c8"),
    "athletics": (athletics, "BODY", "#b6ffb6"), "freerun": (freerun, "BODY", "#3a9a3a"),
    "second_wind": (second_wind, "BODY", "#ff3355"),
}

PLATE, PLATE_HI, SCREW, COLD = "#101217", "#1b1e25", "#4a4f58", "#363b44"


def neon(skill, lit):
    """The sign on its plate. Lit, every pixel next to a tube glows with that tube's
    colour, the main tubes' halos first and the accent's over them; cold, the tubes are
    grey glass and nothing glows."""
    draw, stat, accent = NEON[skill]
    s = Sign()
    draw(s)
    colour = STAT[stat]
    p = Pix(W, ROWS)
    p.rect(0, 0, W, H, PLATE)
    p.hline(0, W, 0, PLATE_HI)
    for x, y in [(0, 0), (W - 1, 0), (0, H - 1), (W - 1, H - 1)]:
        p.px(x, y, SCREW)
    if lit:
        for pass_accent in (False, True):
            halo = mix(PLATE, accent if pass_accent else colour, 0.28)
            for (x, y), acc in sorted(s.tubes.items(), key=lambda t: (t[0][1], t[0][0])):
                if acc != pass_accent:
                    continue
                for dx in (-1, 0, 1):
                    for dy in (-1, 0, 1):
                        if (x + dx, y + dy) not in s.tubes:
                            p.px(x + dx, y + dy, halo)
    for (x, y), acc in s.tubes.items():
        p.px(x, y, (accent if acc else colour) if lit else COLD)
    return p


def sign(skill):
    """The sign's tubes: (x, y) -> accent or not."""
    s = Sign()
    NEON[skill][0](s)
    return s.tubes


ORDER = [
    ("NETRUNNER", ["hacking", "ice_breaker", "vault_run"]),
    ("WRENCH", ["electronics", "scrapcraft", "overclock"]),
    ("IRON", ["handguns", "longarm", "snapshot"]),
    ("EDGE", ["blades", "close_work", "riposte"]),
    ("GHOST", ["stealth", "lockpick", "vanish"]),
    ("STREET", ["streetwise", "haggle", "fixers_ear"]),
    ("RIPPER", ["medtech", "field_patch", "stim_cocktail"]),
    ("MEAT", ["athletics", "freerun", "second_wind"]),
]


def drawn(skill):
    """A hardware drawing as drawn, learned."""
    p = Pix(W, ROWS)
    HARDWARE[skill](p)
    return p


def render(skill, learned=True):
    if skill in NEON:
        return neon(skill, learned)
    p = drawn(skill)
    if not learned:
        # A hole stays the panel, as the game draws it: it exports as an empty pixel.
        p.recolour(lambda c: c if c == PANEL else grey(c))
    return p
