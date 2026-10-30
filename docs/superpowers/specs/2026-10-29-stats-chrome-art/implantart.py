"""The chrome's card art: 30x6 cells, two pixels to a cell stacked (30x12 pixels).

Every piece of chrome is a product shot of its hardware on the deck's dark panel, in
the register the approved Piston arm set (2026-10-29-stats-chrome-mock/gen.py): side-on
or three-quarter, gunmetal and steel greys, a bright edge on top surfaces and a dark
underside so each reads as a solid, and one light per piece in its own colour: cyan
unless the piece says otherwise (the Target lens is red, the Adrenal pump amber, the
Limbic damper a calm blue). Organic and chemical parts keep their own accents: toxin
green, night green, muscle red, copper, ceramic, skin, the Smart ink's magenta.

A drawing is 12 rows of 30 pixels, each a "#rrggbb" colour or None (the panel shows
through). `ART` maps each chrome id to its drawing's function, in the ledger's order;
`check()` holds every drawing to the format; `board.py` shows them all.

Most drawings are string art: one character a pixel, `R((x, "chars"), ...)` placing
runs on a row as the Piston arm's drawing does, the digits 1-9 the steel ramp dark to
bright and 0 a hole (see BASE). Plain bars and repeats use the Pix helpers. Ranges are
half-open: rect(x0, y0, x1, y1) fills x0..x1-1, y0..y1-1.
"""

import math

W, H = 30, 12
# The deck's panel. Never a pixel's colour: it would read as a hole.
PANEL = "#04060a"

# ---------------------------------------------------------------- palette

# Gunmetal to polished steel, dark to bright, as the Piston arm has them.
G1, G2, G3, G4, G5 = "#2a3440", "#4a5666", "#5a6676", "#6a7686", "#8a96a6"
G6, G7, G8, G9 = "#a8b4c0", "#c8d4e0", "#d8e2ec", "#e8f2fa"
DEEP = "#141b24"      # a socket, a slot, a hole in the hardware; darker than G1
SHINE = "#ffffff"     # a glint, a pixel at most

# The light.
CYAN, GLOW, CYAN_DK = "#00e5ff", "#7ff3ff", "#0a8aa0"
HAZE = "#0f4a58"      # cyan seen through glass, or its halo
RED, RED_GLOW, RED_DK, RED_DEEP = "#ff3040", "#ff8090", "#8a1424", "#3c0a12"
AMBER, AMBER_GLOW, AMBER_DK = "#ffb020", "#ffe08a", "#9a5c0c"
CALM, CALM_GLOW = "#4cc4ff", "#b4e8ff"
MAGENTA, MAG_GLOW, MAG_HALO = "#ff00c8", "#ff8ae6", "#b2668e"

# The organic and the chemical.
TOX_HI, TOX, TOX_LO, TOX_DK = "#d4f26a", "#9cc83a", "#5e8424", "#2e4416"
NV_HI, NV, NV_LO, NV_DK = "#b4ffe8", "#3ee8b0", "#139070", "#0a3e34"
MUS_HI, MUS, MUS_LO, MUS_DK, MUS_GLINT = "#e4505e", "#b4223a", "#7a1226", "#480a16", "#ffb0b8"
CU_HI, CU, CU_LO, CU_DK = "#f8c088", "#d88848", "#a05a28", "#5e3214"
CER_HI, CER, CER_LO, CER_DK = "#f8f4ec", "#e2dccc", "#bfb6a2", "#8a8272"
SKIN_HI, SKIN, SKIN_LO, SKIN_DK = "#c8a696", "#a8887a", "#86685e", "#5e4842"
SKIN_UNDER, SKIN_UNDER_LO = "#e8cabc", "#cfae9f"   # the underside of a peeled flap
SCL_HI, SCL, SCL_LO, SCL_DK = "#f6f2e4", "#dcd4ba", "#aca286", "#6e6856"
IRIS_DEAD, IRIS_DEAD_LO = "#9a7a3a", "#5e4a22"
CARB_HI, CARB, CARB_LO = "#5c6c7e", "#364252", "#262e3a"
HAZ, HAZ_BLK = "#ffc820", "#1c2229"
GLASS_HI, GLASS, GLASS_LO, GLASS_SPEC = "#a8d4e2", "#24404e", "#182c38", "#dcf6ff"
WIRE = "#34404e"      # the monowire, near invisible

# The common cylinder shadings, top row first.
STEEL4 = [G9, G7, G5, G3]
STEEL5 = [G8, G9, G7, G5, G3]
STEEL6 = [G8, G9, G7, G6, G4, G2]
STEEL7 = [G8, G9, G7, G6, G5, G4, G2]
STEEL8 = [G7, G9, G8, G7, G6, G5, G4, G2]


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(round(v)))) for v in t)


def mix(a, b, t):
    """a toward b by t."""
    ra, rb = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(ra[k] + (rb[k] - ra[k]) * t for k in range(3)))


# ---------------------------------------------------------------- the canvas

def sample(ramp, i, n):
    """The ramp's colour for row i of n, the ramp stretched over n."""
    if n <= 1:
        return ramp[0]
    return ramp[min(len(ramp) - 1, int(i * len(ramp) / n))]


def R(*segs, w=W):
    """A row of string art: each (x, chars) a run starting at x; '.' is empty."""
    r = ["."] * w
    for c, s in segs:
        assert c + len(s) <= w, (c, s)
        for i, ch in enumerate(s):
            r[c + i] = ch
    return "".join(r)


class Pix:
    """30 by 12 pixels, each None (the panel) or a colour."""

    def __init__(self):
        self.p = [[None] * W for _ in range(H)]

    def px(self, x, y, c):
        if 0 <= x < W and 0 <= y < H:
            self.p[y][x] = c

    def get(self, x, y):
        return self.p[y][x] if 0 <= x < W and 0 <= y < H else None

    def pts(self, pts, c):
        for x, y in pts:
            self.px(x, y, c)

    def rect(self, x0, y0, x1, y1, c):
        for y in range(y0, y1):
            for x in range(x0, x1):
                self.px(x, y, c)

    def hline(self, x0, x1, y, c):
        self.rect(x0, y, x1, y + 1, c)

    def vline(self, x, y0, y1, c):
        self.rect(x, y0, x + 1, y1, c)

    def hcyl(self, x0, y0, x1, y1, ramp):
        """A horizontal cylinder or bar: each row its ramp colour, top row first."""
        for y in range(y0, y1):
            self.hline(x0, x1, y, sample(ramp, y - y0, y1 - y0))

    def vcyl(self, x0, y0, x1, y1, ramp):
        """A vertical cylinder or bar: each column its ramp colour, left column first."""
        for x in range(x0, x1):
            self.vline(x, y0, y1, sample(ramp, x - x0, x1 - x0))

    def corners(self, x0, y0, x1, y1):
        """Rounds a plate: its four corner pixels cleared."""
        for x, y in [(x0, y0), (x1 - 1, y0), (x0, y1 - 1), (x1 - 1, y1 - 1)]:
            self.px(x, y, None)

    def sprite(self, rows, pal, x=0, y=0):
        """String art at (x, y): one character a pixel, '.' and ' ' left alone."""
        for j, row in enumerate(rows):
            for i, ch in enumerate(row):
                if ch not in ". ":
                    self.px(x + i, y + j, pal[ch])

    def mirror(self):
        """The left half copied onto the right, mirrored about the middle."""
        for y in range(H):
            for x in range(W // 2):
                self.p[y][W - 1 - x] = self.p[y][x]

    def rows(self):
        return [list(r) for r in self.p]


# The digits are the steel ramp, 1 dark to 9 bright, and 0 a hole; c q C h the cyan
# light, its glow, its dim and its haze; ! a glint.
BASE = {"0": DEEP, "1": G1, "2": G2, "3": G3, "4": G4, "5": G5, "6": G6, "7": G7, "8": G8, "9": G9,
        "c": CYAN, "q": GLOW, "C": CYAN_DK, "h": HAZE, "!": SHINE}


def pal(**extra):
    d = dict(BASE)
    d.update(extra)
    return d


def art(rows, palette=BASE):
    """String art straight to a canvas."""
    p = Pix()
    p.sprite(rows, palette)
    return p


# ================================================================= NEURAL

def neural_port():
    """A jack socket behind the ear: the flange that seals it to the skull, bolted at
    its corners; the port with its cyan ring and pin; the cable stub out the back,
    wires bare at the cut."""
    return art([
        R(),
        R((4, "7777777777")),
        R((3, "6844"), (7, "8998"), (11, "4482")),
        R((3, "644"), (6, "8qqcc4"), (12, "442"), (15, "6")),
        R((3, "64"), (5, "8qC00Cc3"), (13, "42"), (15, "877"), (25, "88"), (28, "q")),
        R((3, "64"), (5, "7c0750C3"), (13, "42"), (15, "655"), (18, "5555555"), (25, "99"), (27, "c")),
        R((3, "64"), (5, "6c0530C2"), (13, "42"), (15, "544"), (18, "3333333"), (25, "77"), (27, "89")),
        R((3, "64"), (5, "5cC00CC2"), (13, "42"), (15, "322"), (18, "2222222"), (25, "55"), (27, "C")),
        R((3, "644"), (6, "4cCCC2"), (12, "442"), (15, "2"), (25, "33"), (28, "c")),
        R((3, "6844"), (7, "3322"), (11, "4482")),
        R((4, "2222222222")),
        R(),
    ]).rows()


def limbic_damper():
    """A module with a dampening coil wound on its back between two flanges; in a
    window on its face a calm blue wave settles flat."""
    p = Pix()
    # The module: its lit top edge and face, the front, the underside, the pins.
    p.hcyl(2, 5, 28, 10, [G9, G7, G5, G4, G2])
    p.corners(2, 5, 28, 10)
    p.vline(27, 6, 9, G3)
    for x in range(4, 27, 2):
        p.px(x, 10, G6)
    # The coil: copper windings round the core, a dark line between each turn.
    for x in range(7, 23):
        turn = x % 2
        p.hcyl(x, 1, x + 1, 5, [CU_HI, CU_HI, CU, CU_LO] if turn else [CU, CU_LO, CU_DK, CU_DK])
    for x in (6, 23):
        p.hcyl(x, 1, x + 1, 6, [G9, G8, G7, G5, G3])
    # The window: a wave, then nothing.
    p.rect(4, 7, 12, 9, DEEP)
    p.pts([(4, 7), (6, 7)], CALM_GLOW)
    p.pts([(5, 8), (7, 8), (8, 8), (9, 8), (10, 8), (11, 8)], CALM)
    for x in range(15, 26, 2):
        p.vline(x, 7, 9, G3)
    return p.rows()


def feedback_fuse():
    """A fuse cartridge: blade contacts, knurled steel caps, and in the glass between
    them the filament, lit."""
    p = Pix()
    # One end, mirrored: the blade contact and the cap with its knurled ring.
    p.hcyl(1, 5, 4, 8, [G8, G6, G3])
    p.hcyl(4, 2, 9, 10, STEEL8)
    p.hcyl(6, 2, 7, 10, [G5, G6, G6, G5, G4, G3, G2, G1])
    p.vline(8, 3, 9, G4)
    p.mirror()
    # The glass, a streak of light along it, the filament and its halo.
    p.hcyl(9, 3, 21, 9, [GLASS_HI, GLASS, GLASS, GLASS, GLASS, GLASS_LO])
    p.hline(10, 16, 4, GLASS_SPEC)
    p.hline(12, 18, 5, HAZE)
    p.hline(12, 18, 7, HAZE)
    p.hline(9, 21, 6, CYAN)
    p.hline(13, 17, 6, GLOW)
    return p.rows()


# ================================================================= EYES

# A lens face-on: a steel bezel, a dark rim, the glass; n N g G the glass dark to
# bright, read through the palette it is drawn with.
LENS = [
    "...8998...",
    ".79999986.",
    ".98nnnn65.",
    "98nGNNNn54",
    "87nNggNn43",
    "76nNggNn32",
    "65nNNNNn32",
    ".54nnnn32.",
    ".44333322.",
    "...2222...",
]


def nightglass_optics():
    """A pair of low-light lenses in steel bezels, the glass night-green; the right one
    is still showing its maker's logo from the boot."""
    p = Pix()
    night = pal(n=NV_DK, N=NV_LO, g=NV, G=NV_HI)
    p.hcyl(1, 4, 3, 6, [G7, G3])
    p.hcyl(27, 4, 29, 6, [G7, G3])
    p.hcyl(13, 4, 17, 6, [G8, G4])
    p.sprite(LENS, night, 3, 1)
    logo = LENS[:3] + ["98nNGGNn54", "87nGNNGn43", "76nGNNGn32", "65nNGGNn32"] + LENS[7:]
    p.sprite(logo, night, 17, 1)
    return p.rows()


def target_lens():
    """An optic: a short ribbed barrel and its lead, the lens face-on, a red reticle box
    in the glass with its ticks."""
    p = Pix()
    p.hcyl(2, 4, 5, 8, [G5, G4, G3, G2])
    p.hcyl(5, 2, 15, 10, STEEL8)
    for x in (7, 10, 13):
        p.hcyl(x, 2, x + 1, 10, [G5, G6, G5, G4, G3, G2, G1, G1])
    p.sprite([
        "...8998...",
        ".89dkkd96.",
        ".9pddddd5.",
        "8ddrrrrdd4",
        "7kdrddrdk3",
        "6kdrddrdk3",
        "5ddrrrrdd2",
        ".4dddddd2.",
        ".43dkkd22.",
        "...2222...",
    ], pal(d=RED_DEEP, k=RED_DK, r=RED, p=RED_GLOW), 14, 1)
    return p.rows()


def secondhand_eyes():
    """Two eyes that never came as a pair: a big scratched one, its cyan iris still
    lit, and a small dead one with a crack across it."""
    p = Pix()
    eye = pal(W=SCL_HI, w=SCL, v=SCL_LO, V=SCL_DK, a=IRIS_DEAD, A=IRIS_DEAD_LO, k=G1)
    # The optic stubs.
    p.hcyl(1, 5, 4, 7, [G6, G3])
    p.hcyl(25, 6, 29, 8, [G5, G2])
    # The big one, looking right, two scratches across it.
    p.sprite([
        "...WWww...",
        ".WWWWwwwv.",
        ".WWWwwwwv.",
        "WWWwwwwwvv",
        "WWwwwwwwvv",
        "Wwwwwwwvvv",
        "wwwwwwvvvV",
        ".wwwwvvvV.",
        ".vvvvvvVV.",
        "...VVVV...",
    ], eye, 3, 1)
    p.sprite([".cc.", "c!0c", "c00C", ".CC."], eye, 8, 4)
    p.pts([(4, 6), (5, 5), (6, 4), (7, 3)], SCL_LO)
    p.pts([(5, 8), (6, 8)], SCL_LO)
    # The small one, looking up and away, dead, cracked through.
    p.sprite([
        "..WWww..",
        ".WWwwwv.",
        "WWwwwwvv",
        "Wwwwwvvv",
        "wwwwvvvV",
        "wwwvvvVV",
        ".vvvvVV.",
        "..VVVV..",
    ], eye, 18, 3)
    p.sprite(["aaa", "a0a", "aaA"], eye, 19, 4)
    p.pts([(22, 3), (22, 4), (23, 5), (22, 6), (22, 7), (23, 8), (23, 9), (22, 10)], G1)
    p.pts([(23, 4), (24, 6), (23, 7)], SCL_HI)
    return p.rows()


# ================================================================= EARS

def damper_cochlea():
    """A steel cochlea coiled tight, its lead running out to the damper cap: a dome
    with slits on a flange, a cyan tell on top."""
    p = Pix()
    outer = [(9, 8), (8, 8), (7, 8), (6, 9), (5, 9), (4, 9), (3, 9), (2, 8), (1, 8), (1, 7), (0, 6),
             (0, 5), (0, 4), (0, 3), (1, 2), (1, 1), (2, 1), (3, 0), (4, 0), (5, 0), (6, 0), (7, 1),
             (8, 1), (8, 2), (9, 3), (9, 4), (9, 5), (9, 6), (8, 6)]
    inner = [(7, 6), (7, 5), (7, 4), (7, 3), (6, 2), (5, 2), (4, 2), (3, 2), (2, 3), (2, 4), (2, 5),
             (2, 6), (3, 7), (4, 7), (5, 7), (5, 6)]
    for pts, (hi, mid, lo) in ((outer, (G9, G6, G4)), (inner, (G8, G5, G3))):
        for x, y in pts:
            dx, dy = x - 4.5, y - 4.5
            s = -(dx + dy) / (math.hypot(dx, dy) * math.sqrt(2))
            p.px(2 + x, 1 + y, hi if s > 0.35 else lo if s < -0.35 else mid)
    p.rect(6, 5, 8, 7, G7)
    p.px(6, 5, G9)
    # The lead, and the cap.
    p.hline(12, 17, 9, G6)
    p.sprite([
        R((5, "c"), w=11),
        R((4, "798"), w=11),
        R((3, "78996"), w=11),
        R((2, "6787875"), w=11),
        R((2, "5363634"), w=11),
        "89999999997",
        "45555555553",
        ".222222222.",
    ], BASE, 17, 3)
    return p.rows()


# ================================================================= JAW

def bulldog_jaw():
    """A heavy steel lower jaw from the side: the big hinge with its cyan pin, the
    ramus, the body bolted along its length, square teeth, a chin like a ram."""
    p = Pix()
    # The ramus and the body: the gum line, a lit face, dark beneath, the chin jutting.
    p.rect(2, 3, 8, 7, G5)
    p.hline(6, 27, 6, G4)
    p.hcyl(2, 7, 28, 10, [G8, G6, G5])
    p.hline(3, 28, 10, G2)
    p.vline(2, 3, 10, G7)
    p.vline(27, 6, 10, G4)
    p.pts([(28, 7), (28, 8), (28, 9)], G3)
    p.vline(8, 7, 10, G4)
    for x in (12, 17, 22):
        p.px(x, 8, G9)
        p.px(x, 9, G3)
    # Teeth: three molars, a canine, the incisors raked forward.
    for x0 in (8, 12, 16):
        p.hline(x0, x0 + 3, 4, G9)
        p.hline(x0, x0 + 3, 5, G7)
    p.sprite([R((20, "9"), (24, "999")), R((20, "98"), (23, "9998")), R((20, "76"), (23, "7776"))], BASE, 0, 3)
    # The hinge.
    p.sprite([".7997.", "798865", "78qc54", "76cC43", "655432", ".4332."], BASE, 1, 1)
    return p.rows()


def rebreather():
    """A filter canister where the back teeth were: vents along one end, the toxin
    filter glimpsed through a window, the gum plate it is rooted in."""
    p = Pix()
    p.hcyl(5, 2, 25, 9, STEEL7)
    for x0 in (3, 24):
        p.hcyl(x0, 1, x0 + 3, 10, [G7, G8, G9, G7, G6, G5, G4, G3, G2])
        p.corners(x0, 1, x0 + 3, 10)
    p.vline(5, 1, 10, G3)
    p.vline(24, 1, 10, G4)
    # Vents.
    for x in (8, 10, 12, 14):
        p.vline(x, 3, 7, DEEP)
    # The filter's window: a dark frame, the green pleats behind it.
    p.rect(16, 3, 23, 8, G1)
    for x in range(17, 22):
        hi = x % 2
        for j, y in enumerate(range(4, 7)):
            p.px(x, y, [TOX_HI, TOX, TOX_LO][j] if hi else [TOX, TOX_LO, TOX_DK][j])
    # The gum plate and two roots.
    p.hline(7, 23, 9, G3)
    p.pts([(9, 10), (10, 10), (19, 10), (20, 10)], G2)
    p.px(4, 4, CYAN)
    return p.rows()


def spit_dart():
    """A slim launcher for under the tongue: the gas bulb with its cyan tell, the
    barrel in its mount, a dart out of the muzzle with a green tip."""
    p = Pix()
    p.hcyl(2, 3, 8, 9, [G8, G9, G7, G6, G4, G2])
    p.corners(2, 3, 8, 9)
    p.px(4, 5, CYAN)
    p.hcyl(8, 5, 19, 8, [G9, G6, G3])
    p.hcyl(12, 4, 14, 9, [G7, G8, G6, G4, G2])
    p.hline(11, 15, 9, G3)
    p.hcyl(19, 4, 21, 9, [G7, G9, G7, G5, G3])
    # The dart.
    p.hline(22, 27, 6, G8)
    p.pts([(22, 4), (23, 5)], G6)
    p.pts([(22, 8), (23, 7)], G4)
    p.px(27, 6, TOX)
    p.px(28, 6, TOX_HI)
    return p.rows()


# ================================================================= SPINE

def redline_spine():
    """The spine on its side, head to the left: fast-wire vertebrae, their processes
    raked back, a cyan bus down the canal through all of them."""
    p = Pix()
    neck = ["...", "..7", ".97", "466", "ccc", "899", "677", "445", "..."]
    back = ["..7", ".97", ".86", "466", "ccc", "899", "677", "556", "333"]
    for i in range(7):
        x = 1 + 4 * i
        p.sprite(neck if i < 2 else back, BASE, x, 1)
        if i < 6:
            p.pts([(x + 3, 6), (x + 3, 8)], G1)
            p.px(x + 3, 7, CYAN_DK)
    p.hline(1, 29, 5, CYAN)
    for x in (7, 15, 23):
        p.px(x, 5, GLOW)
    return p.rows()


def hardback_brace():
    """A titanium brace laid on its side, neck to the left: the collar with its tell,
    heavy bolted plates each over the next, the tail plate tapering off."""
    p = Pix()
    # The plates, neck to tail, each lipped over the next: (x, top row, bottom row).
    plates = [(2, 3, 8), (6, 2, 9), (11, 1, 10), (16, 1, 10), (21, 2, 9)]
    for x, y0, y1 in plates:
        mid = (y0 + y1) // 2
        p.rect(x, y0, x + 5, y1 + 1, G6)
        p.rect(x, mid + 1, x + 5, y1 + 1, G5)
        p.hline(x, x + 4, y0, G9)
        p.px(x + 4, y0, G6)
        p.vline(x, y0 + 1, y1, G8)
        p.vline(x + 4, y0 + 1, y1 + 1, G3)
        p.hline(x, x + 5, y1, G2)
        # The spine channel through every plate, a bolt either side of it.
        p.hline(x + 1, x + 4, mid, G3)
        p.hline(x + 1, x + 4, mid + 1, G7)
        if y1 - y0 >= 7:
            p.px(x + 2, y0 + 2, G9)
            p.px(x + 2, y1 - 2, G9)
    # The tail, tapering off.
    for k, (y0, y1) in enumerate([(3, 8), (4, 7), (5, 6)]):
        p.hcyl(26 + k, y0, 27 + k, y1 + 1, [G8, G6, G5, G3])
    p.px(3, 5, CYAN)
    p.px(3, 6, CYAN_DK)
    return p.rows()


# ================================================================= BACK

def drone_cradle():
    """The cradle alone, bolted to the back: two clamp arms up from the base plate,
    the dock between them with its cyan contact lit. What docks in it is drawn on its
    own card."""
    return art([
        R(),
        R(),
        R((6, "987"), (21, "789")),
        R((6, "75"), (22, "57")),
        R((6, "64"), (22, "46")),
        R((6, "64"), (22, "46")),
        R((6, "64"), (12, "4qccq4"), (22, "46")),
        R((6, "64"), (12, "355553"), (22, "46")),
        R((6, "647"), (13, "4444"), (21, "746")),
        R((3, "888888888888888888888888")),
        R((3, "2444444444444444444444442")),
        R(),
    ]).rows()


# ================================================================= DRONES

# A drone's light is its own: the Scout's green (its Wave), the Mule's amber, the
# Lamp's warm white.
SCOUT_HI, SCOUT, SCOUT_DK = "#c8ffc8", "#78ff78", "#2a8a3a"
MULE_HI, MULE, MULE_DK = "#ffe8a8", "#ffc850", "#8a6420"
LAMP_HI, LAMP, LAMP_DK, BEAM, BEAM_LO = "#ffffff", "#fff0c8", "#a8987a", "#8c7c52", "#4c4430"
NET, NET_LO, CRATE, CRATE_HI = "#a89070", "#6e5c44", "#5a4630", "#8a6e4c"


def scout_drone():
    """A sparrow-sized drone side-on: two rotors blurred over it, its body slung
    between them, the green eye at the nose that maps the floor, two thin skids."""
    return art([
        R(),
        R((3, "3567887653"), (17, "3567887653")),
        R((7, "66"), (21, "66")),
        R((5, "77777"), (10, "9999999999"), (20, "77777")),
        R((9, "8888888888"), (19, "qg")),
        R((9, "7777777777"), (19, "gk")),
        R((9, "555555555555")),
        R((10, "3333333333")),
        R((11, "4"), (18, "4")),
        R((9, "5666"), (17, "6665")),
        R(),
        R(),
    ], pal(q=SCOUT_HI, g=SCOUT, k=SCOUT_DK)).rows()


def mule_drone():
    """A heavier drone, four rotors on long arms, a crate slung under it in a cargo
    net; its amber tell on the nose."""
    return art([
        R(),
        R((1, "35678876531"), (18, "35678876531")),
        R((6, "6"), (23, "6")),
        R((4, "7777"), (8, "99999999999999"), (22, "7777")),
        R((7, "8888888888888888"), (23, "qa")),
        R((7, "7777777777777777"), (23, "ak")),
        R((8, "55555555555555")),
        R((9, "n"), (11, "n"), (13, "n"), (15, "n"), (17, "n"), (19, "n"), (21, "n")),
        R((9, "nBBBBBBBBBBBBn")),
        R((9, "NbbbbbbbbbbbbN")),
        R((9, "nnnnnnnnnnnnnn")),
        R(),
    ], pal(q=MULE_HI, a=MULE, k=MULE_DK, n=NET, N=NET_LO, b=CRATE, B=CRATE_HI)).rows()


def lamp_drone():
    """A small drone with a lamp for a head: two rotors over a short body, the lamp's
    housing at the front and its warm beam thrown forward and down."""
    return art([
        R(),
        R((2, "35677653"), (13, "35677653")),
        R((5, "6"), (16, "6")),
        R((4, "777"), (7, "999999999"), (16, "777")),
        R((6, "88888888888"), (17, "679")),
        R((6, "77777777777"), (17, "5Lw"), (20, "yy")),
        R((7, "555555555"), (17, "4LL"), (20, "YYyy")),
        R((8, "3333333"), (17, "34"), (21, "YYyy")),
        R((9, "4"), (14, "4"), (22, "Yyy")),
        R((7, "566"), (13, "665"), (23, "yy")),
        R(),
        R(),
    ], pal(w=LAMP_HI, L=LAMP, Y=BEAM, y=BEAM_LO)).rows()


# ================================================================= ORGANS

def adrenal_pump():
    """A pump from under the ribs, a syringe's anatomy: the plunger driving in from the
    left, the body with its amber vial lit, the nozzle and the line out."""
    p = Pix()
    p.vcyl(2, 2, 4, 10, [G8, G5])
    p.px(2, 2, G9)
    p.hcyl(4, 5, 9, 7, [G9, G5])
    p.hcyl(8, 2, 21, 10, STEEL8)
    p.corners(8, 2, 21, 10)
    # The vial.
    p.rect(12, 3, 17, 9, G1)
    p.rect(13, 4, 16, 8, AMBER)
    p.pts([(13, 4), (13, 5)], AMBER_GLOW)
    p.hline(13, 16, 7, AMBER_DK)
    # The nozzle and the line.
    p.hcyl(21, 4, 23, 8, [G8, G9, G6, G4])
    p.hcyl(23, 5, 28, 7, [G6, G3])
    p.hline(24, 27, 5, AMBER)
    p.hcyl(27, 4, 29, 8, [G8, G7, G5, G3])
    return p.rows()


def second_heart():
    """A spare heart: a deep red muscle held in a steel cage, its vessels capped off in
    steel, a cyan pacer on the cage's foot."""
    p = Pix()
    # The vessels, capped.
    p.hcyl(3, 2, 10, 4, [G8, G4])
    p.hcyl(20, 2, 27, 4, [G8, G4])
    p.hcyl(2, 1, 3, 5, [G9, G7, G5, G3])
    p.hcyl(27, 1, 28, 5, [G9, G7, G5, G3])
    # The heart.
    p.sprite([
        ".RHR....Rrr.",
        "RHRRr..RRrrd",
        "RRRrrrrrrrrd",
        "RRrrrrrrrrrd",
        "Rrrrrrrrrrdd",
        ".rrrrrrrrdd.",
        "..rrrrrrdd..",
        "...rrrrdd...",
        "....rddD....",
        ".....DD.....",
    ], pal(H=MUS_GLINT, R=MUS_HI, r=MUS, d=MUS_LO, D=MUS_DK), 9, 1)
    # The cage: two bars over it, a ring at its foot.
    for x in (12, 17):
        p.vline(x, 1, 10, G7)
        p.px(x, 1, G9)
    p.hcyl(9, 10, 21, 11, [G6])
    p.pts([(9, 10), (20, 10)], G4)
    p.px(14, 10, CYAN)
    p.px(15, 10, GLOW)
    return p.rows()


def filter_liver():
    """A liver in steel, the big lobe left and tapering right, the seam between the
    lobes; a window onto green filter beds."""
    p = Pix()
    # The housing is steel warmed toward liver, so the shape says organ.
    liv = [mix(g, MUS_LO, 0.36) for g in (G9, G8, G7, G6, G5, G4, G3, G2)]
    rows = {1: (5, 23), 2: (3, 26), 3: (2, 28), 4: (2, 26), 5: (2, 23), 6: (2, 20), 7: (3, 17),
            8: (3, 14), 9: (4, 11), 10: (6, 9)}
    shade = {1: 0, 2: 1, 3: 2, 4: 3, 5: 3, 6: 4, 7: 4, 8: 5, 9: 6, 10: 7}
    for y, (x0, x1) in rows.items():
        p.hline(x0, x1 + 1, y, liv[shade[y]])
        p.px(x0, y, liv[1] if y < 6 else liv[3])
    for x in range(2, 29):
        ys = [y for y, (x0, x1) in rows.items() if x0 <= x <= x1]
        if ys:
            p.px(x, max(ys), liv[7])
    # The notch between the lobes.
    p.pts([(16, 1), (17, 1)], None)
    p.pts([(16, 2), (17, 2)], liv[5])
    # The window.
    p.rect(5, 3, 13, 8, G1)
    for x in range(6, 12):
        hi = x % 2
        for j, y in enumerate(range(4, 7)):
            p.px(x, y, [TOX_HI, TOX, TOX_LO][j] if hi else [TOX, TOX_LO, TOX_DK][j])
    p.hline(5, 13, 8, G7)
    return p.rows()


def capacitor_ribs():
    """A rib cage from the front, its sternum a steel keel, every rib a capacitor
    holding a cyan charge between its steel ends."""
    p = Pix()
    for y0 in (1, 4, 7):
        top = [(13, y0 + 1), (12, y0)] + [(x, y0) for x in range(7, 12)] + [(6, y0 + 1), (5, y0 + 2), (4, y0 + 3)]
        for x, y in top:
            p.px(x, y + 1, CYAN_DK if 8 <= x <= 11 else G4)
        for x, y in top:
            p.px(x, y, CYAN if 8 <= x <= 11 else G8)
        p.px(9, y0, GLOW)
        p.px(7, y0, G9)
    p.vline(14, 1, 11, G8)
    p.mirror()
    p.vline(15, 1, 11, G5)
    p.pts([(14, 1), (15, 1)], G9)
    return p.rows()


# ================================================================= SKIN

def skin_patch(p, x0=2, y0=1, x1=28, y1=11):
    """A patch of skin, lit along its top and left edges, dark along the bottom."""
    p.rect(x0, y0, x1, y1, SKIN)
    p.hline(x0, x1, y0, SKIN_HI)
    p.vline(x0, y0, y1, SKIN_HI)
    p.hline(x0, x1, y1 - 1, SKIN_DK)
    p.vline(x1 - 1, y0 + 1, y1, SKIN_LO)
    p.corners(x0, y0, x1, y1)


def subdermal_mesh():
    """A patch of skin with the mesh showing faintly through it, one corner peeled back
    over itself on the bare weave."""
    p = Pix()
    skin_patch(p)
    faint = mix(SKIN, SKIN_LO, 0.3)
    for y in range(2, 10):
        for x in range(3, 27):
            if (x + y) % 3 == 0 and (x - y) % 3 == 0:
                p.px(x, y, faint)
    k = 18   # the fold, along x - y = k
    for y in range(1, 11):
        for x in range(2, 28):
            if x - y > k:
                p.px(x, y, [[G8, G6], [G5, G1]][y % 2][x % 2])
    for x, y in [(22, 2), (26, 6), (24, 4)]:
        p.px(x, y, CYAN)
    # The flap, folded back over the skin along the crease: lit at the crease, a shadow
    # along its far edges.
    for y in range(1, 11):
        for x in range(2, 28):
            cx, cy = y + k, x - k
            if x - y <= k and 1 <= cy <= 10 and cx <= 27 and cx - cy >= k:
                d = k - (x - y)
                p.px(x, y, SKIN_UNDER if d <= 2 else SKIN_UNDER_LO if d <= 6 else SKIN_LO)
    for x, y in [(x, y) for y in range(2, 12) for x in range(14, 28)]:
        cx, cy = y + k, x - k
        flap = x - y <= k and 1 <= cy <= 10 and cx <= 27 and cx - cy >= k
        if not flap and p.get(x, y) in (SKIN, faint) and (
                p.get(x, y - 1) in (SKIN_UNDER, SKIN_UNDER_LO, SKIN_LO) or
                p.get(x + 1, y) in (SKIN_UNDER, SKIN_UNDER_LO, SKIN_LO)):
            p.px(x, y, SKIN_DK)
    return p.rows()


def ceramic_plating():
    """A chest plate in off-white ceramic lames, each lapped over the one below and
    bolted at its ends, narrowing to the waist; a steel collar at the neck with its tell,
    a seam down the middle."""
    half = [
        R(w=15),
        R((3, "kkkkkkkkk"), w=15),
        R((2, "k4KKKKKKKKj"), (13, "56"), w=15),
        R((1, "jJJJJJJJJJJJ"), (13, "34"), w=15),
        R((4, "kkkkkkkkkkk"), w=15),
        R((3, "k4KKKKKKKKKK"), w=15),
        R((2, "jJJJJJJJJJJJJ"), w=15),
        R((6, "kkkkkkkkk"), w=15),
        R((5, "k4KKKKKKKK"), w=15),
        R((4, "jJJJJJJJJJJ"), w=15),
        R((7, "jjjjjjjj"), w=15),
        R(w=15),
    ]
    p = Pix()
    p.sprite(half, pal(k=CER_HI, K=CER, j=CER_LO, J=CER_DK))
    p.mirror()
    for y in (5, 8):
        p.pts([(14, y), (15, y)], CER_LO)
    p.pts([(14, 2), (15, 2)], CYAN)
    p.pts([(14, 3), (15, 3)], CYAN_DK)
    return p.rows()


def grounding_weave():
    """A copper weave patch shedding a little lightning down to its ground point."""
    p = Pix()
    for j in range(5):
        for i in range(7):
            x, y = 2 + 2 * i, 1 + 2 * j
            if (i + j) % 2 == 0:
                p.hline(x, x + 2, y, CU_HI)
                p.hline(x, x + 2, y + 1, CU_LO)
            else:
                p.vline(x, y, y + 2, CU)
                p.vline(x + 1, y, y + 2, CU_DK)
    # The ground: a lug, the stake, the earth's three bars.
    p.hcyl(22, 2, 26, 4, [G9, G5])
    p.vcyl(23, 4, 25, 6, [G7, G4])
    p.hcyl(20, 6, 28, 7, [G8])
    p.hcyl(21, 8, 27, 9, [G6])
    p.hcyl(22, 10, 26, 11, [G4])
    # The leak.
    p.pts([(16, 6), (17, 5), (18, 4), (19, 5), (20, 4), (21, 3)], CYAN)
    p.pts([(17, 4), (19, 4), (20, 3)], HAZE)
    p.px(18, 4, GLOW)
    p.px(22, 2, SHINE)
    return p.rows()


def smart_ink():
    """A patch of skin with live ink in it: a die's five in magenta, a cyan die's three
    beside it, the lines glowing into the skin round them."""
    p = Pix()
    skin_patch(p)
    lines = {}
    for x in range(4, 13):
        lines[(x, 2)] = lines[(x, 9)] = MAGENTA
    for y in range(2, 10):
        lines[(4, y)] = lines[(12, y)] = MAGENTA
    for x, y in [(6, 4), (10, 4), (8, 6), (6, 7), (10, 7)]:
        lines[(x, y)] = MAG_GLOW
    for x in range(17, 24):
        lines[(x, 3)] = lines[(x, 9)] = CYAN
    for y in range(3, 10):
        lines[(17, y)] = lines[(23, y)] = CYAN
    for x, y in [(19, 5), (20, 6), (21, 7)]:
        lines[(x, y)] = GLOW
    halo_m, halo_c = mix(SKIN, MAGENTA, 0.2), mix(SKIN, CYAN, 0.16)
    for (x, y), c in lines.items():
        for dx, dy in ((1, 0), (-1, 0), (0, 1), (0, -1)):
            q = (x + dx, y + dy)
            if q not in lines and 3 <= q[0] <= 26 and 2 <= q[1] <= 9 and c in (MAGENTA, CYAN):
                p.px(q[0], q[1], halo_m if c == MAGENTA else halo_c)
    for (x, y), c in lines.items():
        p.px(x, y, c)
    return p.rows()


# ================================================================= ARMS

# The Piston arm's palette, which every arm shares.
ARM = {"M": "#4a5666", "m": "#2a3440", "U": "#e8f2fa", "S": "#a8b4c0", "d": "#5a6676", "J": "#6a7686",
       "c": CYAN, "P": "#8a96a6", "p": "#d8e2ec", "F": "#a8b4c0", "f": "#5a6676", "H": "#c8d4e0",
       "q": "#7ff3ff"}


def piston_arm():
    """The approved drawing, verbatim: shoulder plate, upper arm, the lit elbow, the
    piston over the forearm, the arc hand."""
    rows = [
        R(w=30),
        R((0, "MMMM"), (13, "JJJ"), w=30),
        R((0, "MmmM"), (12, "JJJJJ"), (17, "PPPP"), w=30),
        R((0, "MmmM"), (4, "UUUUUUUU"), (12, "JJcJJ"), (17, "PPPPpppppp"), (28, "q"), w=30),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ"), (17, "UUUUUUUU"), (25, "HHH"), (28, "H"), w=30),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ"), (17, "FFFFFFFF"), (25, "HHHH"), (29, "H"), w=30),
        R((0, "MmmM"), (4, "dddddddd"), (12, "JJcJJ"), (17, "FFFFFFFF"), (25, "HHHH"), w=30),
        R((0, "MmmM"), (12, "JJJJJ"), (17, "ffffffff"), (25, "HHH"), (28, "H"), w=30),
        R((0, "MMMM"), (13, "JJJ"), (26, "H"), (29, "H"), w=30),
        R((1, "MM"), (26, "H"), (29, "q"), w=30),
        R((27, "q"), w=30),
        R(w=30),
    ]
    return [[ARM.get(ch) if ch != "." else None for ch in r] for r in rows]


def arm_root(p):
    """The Piston arm's shoulder plate, upper arm and lit elbow, which the Grip arm
    shares."""
    p.sprite([
        R(),
        R((0, "MMMM"), (13, "JJJ")),
        R((0, "MmmM"), (12, "JJJJJ")),
        R((0, "MmmM"), (4, "UUUUUUUU"), (12, "JJcJJ")),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ")),
        R((0, "MmmM"), (4, "SSSSSSSS"), (12, "JcccJ")),
        R((0, "MmmM"), (4, "dddddddd"), (12, "JJcJJ")),
        R((0, "MmmM"), (12, "JJJJJ")),
        R((0, "MMMM"), (13, "JJJ")),
        R((1, "MM")),
    ], ARM)


def grip_arm():
    """The Piston arm's shoulder and elbow on a hydraulic forearm, the ram's sleeve and
    rod, a hose under it, and a clamp for a hand."""
    p = Pix()
    arm_root(p)
    p.hcyl(17, 2, 22, 9, STEEL7)
    p.vline(17, 3, 8, G6)
    p.hcyl(22, 4, 24, 7, [G9, G7, G4])
    p.pts([(16, 9), (17, 10), (22, 10), (23, 9)], G2)
    p.hline(18, 22, 10, G3)
    # The clamp: its back plate, the two jaws, their tips turned in.
    p.vcyl(23, 1, 25, 11, [G8, G4])
    p.corners(23, 1, 25, 11)
    p.hcyl(25, 1, 29, 3, [G9, G5])
    p.hcyl(25, 8, 29, 10, [G7, G3])
    p.pts([(28, 3), (27, 3)], G6)
    p.pts([(28, 7), (27, 7)], G5)
    p.px(28, 3, G7)
    return p.rows()


def surgeon_arm():
    """A slim white arm: a small shoulder, a fine elbow, a long forearm, and long
    fingers with a joint at every other pixel."""
    return art([
        R(),
        R((25, "UJUJU")),
        R((0, "MMM"), (24, "U")),
        R((0, "MmM"), (11, "JJJ"), (22, "JHH")),
        R((0, "MmM"), (3, "UUUUUUUU"), (11, "JcJ"), (14, "UUUUUUUU"), (22, "JHHUJUJq")),
        R((0, "MmM"), (3, "dddddddd"), (11, "JcJ"), (14, "SSSSSSSS"), (22, "JHH")),
        R((0, "MmM"), (11, "JJJ"), (22, "JHHUJUJU")),
        R((0, "MMM"), (23, "HH")),
        R((24, "UJUJ")),
        R(),
        R(),
        R(),
    ], ARM).rows()


def wrecker_arm():
    """A demolition arm a size too big: a great pauldron, a thick upper arm, a lit
    elbow, a hazard-striped forearm and a fist with its thumb over the knuckles."""
    p = Pix()
    p.hcyl(0, 1, 6, 10, [G8, G7, G6, G5, G5, G4, G4, G3, G2])
    p.corners(0, 1, 6, 10)
    p.px(2, 3, G9)
    p.px(2, 4, G3)
    p.hcyl(6, 3, 12, 8, [G9, G7, G6, G5, G3])
    p.sprite([".4554.", "457754", "47cqc4", "4cccc3", "45cc43", "455543", ".3443."], BASE, 11, 2)
    p.hcyl(17, 1, 23, 11, [G8, G9, G7, G6, G6, G5, G5, G4, G3, G2])
    for y in range(3, 8):
        for x in range(18, 23):
            p.px(x, y, HAZ if (x + y) % 4 < 2 else HAZ_BLK)
    p.sprite([
        "79998..",
        "666654.",
        "8888889",
        "5555556",
        "7777778",
        "4444445",
        "6666667",
        "3333334",
        "5555556",
        ".22222.",
    ], BASE, 23, 1)
    return p.rows()


# ================================================================= HAND

def wrist_plug(p, x0=1):
    """The plug a hand fits a chrome arm's socket by, with its cyan ring."""
    p.hcyl(x0, 4, x0 + 4, 9, [G7, G8, G6, G4, G2])
    p.vline(x0 + 3, 4, 9, CYAN_DK)
    p.px(x0 + 3, 5, CYAN)


def hand(p, reach):
    """A chrome hand from the side off its wrist plug: the palm, the thumb raised, four
    fingers out to `reach`, their last x, index to little."""
    wrist_plug(p, 1)
    p.hcyl(5, 3, 14, 11, [G8, G7, G7, G6, G6, G5, G4, G3])
    p.vline(5, 3, 11, G6)
    p.corners(5, 3, 14, 11)
    p.hline(10, 14, 2, G8)
    p.hline(12, 15, 1, G9)
    p.px(15, 1, G7)
    for i, (y, x1) in enumerate(zip((3, 5, 7, 9), reach)):
        p.hline(14, x1 + 1, y, G9 if i == 0 else G8)
        p.hline(14, 16, y + 1, G5 if i < 3 else G4)
        p.hline(16, x1, y + 1, G3)
        for jx in (16, 19):
            if jx < x1:
                p.px(jx, y, G6)


def arc_hand():
    """An open chrome hand off its wrist plug, arcs leaping from the fingertips."""
    p = Pix()
    hand(p, (19, 20, 19, 17))
    arcs = [(21, 3, CYAN), (22, 2, GLOW), (23, 2, GLOW), (24, 1, CYAN), (25, 1, HAZE),
            (22, 5, CYAN), (23, 6, GLOW), (24, 5, GLOW), (25, 5, GLOW), (26, 6, CYAN), (27, 5, CYAN),
            (28, 5, HAZE), (21, 7, CYAN), (22, 8, GLOW), (23, 8, GLOW), (24, 9, CYAN), (25, 10, HAZE),
            (21, 6, GLOW), (19, 10, HAZE), (18, 9, CYAN)]
    for x, y, c in arcs:
        p.px(x, y, c)
    return p.rows()


def toolfingers():
    """A chrome hand whose fingers end in tools: a hook, a blade, a screwdriver, a
    probe with a lit tip."""
    p = Pix()
    hand(p, (17, 18, 17, 16))
    # The hook, curling up and back.
    p.hline(18, 23, 3, G7)
    p.pts([(23, 2), (24, 1), (23, 0)], G9)
    p.px(24, 2, G6)
    # The blade: a lit edge along the top, the spine tapering under it.
    p.hline(19, 28, 5, G9)
    p.hline(19, 24, 6, G5)
    p.px(27, 5, SHINE)
    # The screwdriver: a thin shaft, a flat tip.
    p.hline(18, 23, 7, G6)
    p.vline(23, 7, 9, G9)
    p.vline(24, 7, 9, G7)
    # The probe, its tip lit.
    p.hline(17, 25, 10, G5)
    p.px(25, 10, CYAN)
    p.px(26, 10, GLOW)
    return p.rows()


# ================================================================= FOREARM

def monowire_spool():
    """A wrist spool face-on on its cuff, the wire wound on it catching cyan, and the
    wire out of it near invisible but where it catches the light."""
    p = Pix()
    p.hcyl(1, 4, 18, 10, [G6, G5, G4, G4, G3, G2])
    p.corners(1, 4, 18, 10)
    p.sprite([
        "...8998...",
        ".89qqqC96.",
        ".8q8887C5.",
        "8q877765C5",
        "7C760054h4",
        "6C650043h3",
        "5C654332h3",
        ".4h4332h3.",
        ".43hhhh32.",
        "...2222...",
    ], BASE, 4, 1)
    for x in range(10, 29):
        p.px(x, 1 if x < 17 else 2 if x < 23 else 3, WIRE)
    p.px(14, 1, SHINE)
    p.px(20, 2, GLOW)
    p.px(26, 3, G8)
    p.px(28, 3, G6)
    return p.rows()


def launcher_sleeve():
    """A spring tube along the forearm, the spring cocked behind its slot, a grenade
    sitting in the muzzle with its cyan band lit."""
    p = Pix()
    p.hcyl(2, 3, 19, 10, STEEL7)
    p.corners(2, 3, 19, 10)
    # The spring, wound round the sleeve and standing proud of it.
    for x in range(5, 16):
        if x % 2:
            p.hcyl(x, 2, x + 1, 11, [G8, G9, G8, G7, G6, G5, G4, G3, G2])
        else:
            p.hcyl(x, 3, x + 1, 10, [G3, G4, G3, G3, G2, G2, G1])
    for x in (6, 13):
        p.hline(x, x + 2, 10, G3)
    p.hcyl(18, 2, 20, 11, [G7, G9, G8, G7, G6, G5, G4, G3, G2])
    p.sprite([
        "..65..",
        ".7886.",
        "789975",
        "778764",
        "cqcccC",
        "566543",
        ".4432.",
    ], BASE, 21, 3)
    return p.rows()


# ================================================================= LEGS

# The far leg of a pair, drawn a step dimmer than the near one.
DIM = str.maketrans({"9": "a", "8": "a", "7": "b", "6": "b", "5": "B", "4": "B", "3": "B",
                     "c": "C", "x": "k", "X": "K"})
FAR = pal(a=G5, b=G4, B=G2, k="#56626f", K="#3a4452")


def pair(p, leg, near, far_x, near_x):
    """A leg sprite twice: the far one dim and behind, the near one over it."""
    p.sprite([r.translate(DIM) for r in leg], FAR, far_x, 0)
    p.sprite(leg, near, near_x, 0)


def sprinter_shanks():
    """A pair of carbon legs standing side-on: the socket cup at the top, the knee
    hinge with its cyan light, a straight carbon shin with a spring in it, and the
    running blade curling back and forward to the toe."""
    leg = [
        "799997.",
        "689996.",
        ".68995.",
        ".6895..",
        ".5cc3..",
        "..xX...",
        "..96...",
        "..53...",
        "..96...",
        ".xX....",
        "xX.....",
        ".xxxxX.",
    ]
    p = Pix()
    pair(p, leg, pal(x="#74849a", X="#465264"), 9, 16)
    return p.rows()


def dog_legs():
    """A pair of digitigrade racing legs, one ahead of the other: a heavy thigh forward
    to the lit knee, a long shank raked back to a high hock, the foot down and forward
    to a hoof."""
    leg = [
        R((0, "79998"), w=9),
        R((1, "899965"), w=9),
        R((3, "8996c4"), w=9),
        R((3, "8996"), w=9),
        R((2, "8996"), w=9),
        R((1, "8996"), w=9),
        R((0, "8996"), w=9),
        R((0, "7C4"), w=9),
        R((1, "85"), w=9),
        R((2, "85"), w=9),
        R((2, "8999"), w=9),
        R((2, "45554"), w=9),
    ]
    p = Pix()
    pair(p, leg, BASE, 8, 14)
    return p.rows()


# ================================================================= the registry

ART = {
    "neural_port": neural_port,
    "limbic_damper": limbic_damper,
    "feedback_fuse": feedback_fuse,
    "nightglass_optics": nightglass_optics,
    "target_lens": target_lens,
    "secondhand_eyes": secondhand_eyes,
    "damper_cochlea": damper_cochlea,
    "bulldog_jaw": bulldog_jaw,
    "rebreather": rebreather,
    "spit_dart": spit_dart,
    "redline_spine": redline_spine,
    "hardback_brace": hardback_brace,
    "drone_cradle": drone_cradle,
    "scout_drone": scout_drone,
    "mule_drone": mule_drone,
    "lamp_drone": lamp_drone,
    "adrenal_pump": adrenal_pump,
    "second_heart": second_heart,
    "filter_liver": filter_liver,
    "capacitor_ribs": capacitor_ribs,
    "subdermal_mesh": subdermal_mesh,
    "ceramic_plating": ceramic_plating,
    "grounding_weave": grounding_weave,
    "smart_ink": smart_ink,
    "grip_arm": grip_arm,
    "piston_arm": piston_arm,
    "surgeon_arm": surgeon_arm,
    "wrecker_arm": wrecker_arm,
    "arc_hand": arc_hand,
    "toolfingers": toolfingers,
    "monowire_spool": monowire_spool,
    "launcher_sleeve": launcher_sleeve,
    "sprinter_shanks": sprinter_shanks,
    "dog_legs": dog_legs,
}

NAMES = {
    "neural_port": "Neural port", "limbic_damper": "Limbic damper", "feedback_fuse": "Feedback fuse",
    "nightglass_optics": "Nightglass optics", "target_lens": "Target lens",
    "secondhand_eyes": "Secondhand eyes", "damper_cochlea": "Damper cochlea",
    "bulldog_jaw": "Bulldog jaw", "rebreather": "Rebreather", "spit_dart": "Spit dart",
    "redline_spine": "Redline spine", "hardback_brace": "Hardback brace",
    "drone_cradle": "Drone cradle", "scout_drone": "Scout drone", "mule_drone": "Mule drone",
    "lamp_drone": "Lamp drone", "adrenal_pump": "Adrenal pump", "second_heart": "Second heart",
    "filter_liver": "Filter liver", "capacitor_ribs": "Capacitor ribs",
    "subdermal_mesh": "Subdermal mesh", "ceramic_plating": "Ceramic plating",
    "grounding_weave": "Grounding weave", "smart_ink": "Smart ink", "grip_arm": "Grip arm",
    "piston_arm": "Piston arm", "surgeon_arm": "Surgeon arm", "wrecker_arm": "Wrecker arm",
    "arc_hand": "Arc hand", "toolfingers": "Toolfingers", "monowire_spool": "Monowire spool",
    "launcher_sleeve": "Launcher sleeve", "sprinter_shanks": "Sprinter shanks", "dog_legs": "Dog legs",
}

# Where each goes, for the board's headings.
SITES = [
    ("NEURAL", ["neural_port", "limbic_damper", "feedback_fuse"]),
    ("EYES", ["nightglass_optics", "target_lens", "secondhand_eyes"]),
    ("EARS", ["damper_cochlea"]),
    ("JAW", ["bulldog_jaw", "rebreather", "spit_dart"]),
    ("SPINE", ["redline_spine", "hardback_brace"]),
    ("BACK", ["drone_cradle"]),
    ("BAY", ["scout_drone", "mule_drone", "lamp_drone"]),
    ("ORGANS", ["adrenal_pump", "second_heart", "filter_liver", "capacitor_ribs"]),
    ("SKIN", ["subdermal_mesh", "ceramic_plating", "grounding_weave", "smart_ink"]),
    ("ARM", ["grip_arm", "piston_arm", "surgeon_arm", "wrecker_arm"]),
    ("HAND", ["arc_hand", "toolfingers"]),
    ("FOREARM", ["monowire_spool", "launcher_sleeve"]),
    ("LEGS", ["sprinter_shanks", "dog_legs"]),
]


def check():
    """Every drawing 12 rows of 30, colours only as #rrggbb, never the panel, at most
    20 colours."""
    import re
    hexc = re.compile(r"^#[0-9a-f]{6}$")
    assert [i for _, ids in SITES for i in ids] == list(ART), "SITES and ART disagree"
    assert set(NAMES) == set(ART)
    for key, fn in ART.items():
        g = fn()
        assert len(g) == H, (key, len(g))
        cols = set()
        for y, row in enumerate(g):
            assert len(row) == W, (key, y, len(row))
            for x, c in enumerate(row):
                if c is None:
                    continue
                assert isinstance(c, str) and hexc.match(c), (key, x, y, c)
                assert c != PANEL, (key, x, y, "panel colour")
                cols.add(c)
        assert len(cols) <= 20, (key, len(cols))
    return True


if __name__ == "__main__":
    check()
    for key, fn in ART.items():
        n = len({c for r in fn() for c in r if c})
        print("%-18s %2d colours" % (key, n))
