"""Armor art: the drawings, and the mock pages that drew them into the game's real
blueprint window and KIT viewer. Round 1 is three punk takes on the Salvage jacket
(A, B, C: kept for later pieces), round 2 two brown ones (D picked), round 3 all
five pieces. export.py writes the picked drawings to flavor/item_art.ron.

The shots come from the `armor_on_its_blueprints` shot test:

    SHOT_DIR=<dir> cargo test -p strata-zero --target-dir target/smoke -- --ignored \\
        armor_on_its_blueprints
    python3 -I gen.py <dir> <out.html> [round]

The pages drawn before the pictures were in the game used shots of the pieces
without them; drawn now, the window shows the game's own picture under the mock's.
"""

import os
import random
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402
from px import Canvas, mix  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")
ROUND = sys.argv[3] if len(sys.argv) > 3 else "1"
PANEL = "#04060a"

W, H = 64, 44

# Shared metal.
STUD_HI, STUD, STUD_LO = "#eef2f6", "#9aa1aa", "#3d424a"
CHROME_HI, CHROME, CHROME_LO = "#ffffff", "#c9ced6", "#6b717b"
MAGENTA, MAGENTA_LO = "#ff2bd6", "#9c0f82"
TAN, TAN_LO = "#c9a447", "#7d6420"


def front_outline(c):
    """The jacket from the front, left half: collar notch, shoulder, a hanging sleeve
    clear of the body at the cuff, the body down to the hem."""
    return c.mirrored([(31, 4), (27, 1), (22, 1), (13, 4), (9, 8), (7, 16), (4, 38),
                       (4, 41), (11, 41), (13, 26), (15, 17), (16, 43), (31, 43)])


def back_outline(c):
    return c.mirrored([(31, 0), (24, 0), (13, 4), (9, 8), (7, 16), (4, 38), (4, 41),
                       (11, 41), (13, 26), (15, 17), (16, 43), (31, 43)])


def stud(c, x, y):
    c.stamp(x, y, ["ab", "bc"], {"a": STUD_HI, "b": STUD, "c": STUD_LO})


def spike(c, x, y):
    """A cone spike standing up from (x, y), three wide at the base."""
    c.stamp(x - 1, y - 3, [".a.", ".a.", "abc", "bbc"],
            {"a": STUD_HI, "b": STUD, "c": STUD_LO})


def safety_pin(c, x, y):
    c.stamp(x, y, ["aaaab", "c...b", "aaaab"], {"a": CHROME, "b": CHROME_HI, "c": CHROME_LO})


def weave(c, x0, y0, x1, y1, keep=None):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if keep and not keep(x, y):
                continue
            c.px(x, y, TAN if (x + y) % 2 == 0 else TAN_LO)


def sleeve_creases(c, col):
    for x0, y0, x1, y1 in [(6, 22, 11, 20), (5, 27, 10, 26), (6, 33, 11, 31)]:
        c.line(x0, y0, x1, y1, col)
        c.line(W - 1 - x0, y0, W - 1 - x1, y1, col)


# ---------------------------------------------------------------- A: studs & patches

def concept_a():
    c = Canvas(W, H)
    base, hi, lo, edge = "#2b2930", "#5a5563", "#141317", "#0b0a0d"
    lining, tee = "#0c0b0f", "#8c1830"
    c.poly(front_outline(c), base)
    # The collar's inside, then the tee in the open front.
    c.poly([(27, 1), (36, 1), (32, 4), (31, 4)], lining)
    c.poly([(28, 4), (35, 4), (33, 21), (30, 21)], tee)
    c.stamp(29, 8, [".a.a.", "aaaaa", ".aaa.", "..a.."], {"a": "#e8e2d0"})  # a little print
    c.shade(base, hi, lo)
    # Lapels, wide, biker cut, each edged in studs.
    lap = "#36333c"
    for flip in (False, True):
        pts = [(27, 2), (21, 4), (23, 13), (29, 20)]
        if flip:
            pts = [(W - 1 - x, y) for x, y in pts]
        c.poly(pts, lap)
        c.line(*pts[0], *pts[3], hi)
    for y in range(4, 19, 3):
        x = 27 - (y - 2) // 4
        stud(c, x - 4, y)
        stud(c, W - 1 - x + 3, y)
    # The closed zip below the opening.
    for y in range(21, 43):
        c.px(31, y, STUD if y % 2 else STUD_LO)
        c.px(32, y, STUD_LO if y % 2 else STUD)
    # Spikes on the shoulders.
    for x, y in [(15, 4), (19, 3), (23, 2)]:
        spike(c, x, y)
        spike(c, W - 1 - x, y)
    # A patch on each breast: a bolt on magenta, hazard on yellow.
    c.rect(17, 24, 25, 31, MAGENTA_LO)
    c.rect(18, 25, 24, 30, MAGENTA)
    c.stamp(19, 25, ["...aa", "..aa.", ".aaaa", "...a.", "..a..", ".a..."], {"a": "#120414"})
    for y in range(24, 30):
        for x in range(38, 47):
            c.px(x, y, "#ffd21a" if (x + y) % 6 < 3 else "#16140a")
    # Safety pins up the left sleeve, a rip on the right with the aramid showing.
    for y in (18, 24, 30):
        safety_pin(c, 7 if y < 30 else 6, y)
    rip = lambda x, y: (x - 53) ** 2 / 9 + (y - 28) ** 2 / 16 < 1 + 0.4 * ((x * 7 + y * 3) % 3 == 0)
    weave(c, 49, 23, 58, 33, rip)
    # Studded cuffs and a chain off the hem.
    for x in (5, 8):
        stud(c, x, 38)
        stud(c, W - 1 - x - 1, 38)
    for i, x in enumerate(range(18, 30, 2)):
        c.px(x, 40 + (1 if 2 < i < 5 else 0), CHROME)
        c.px(x + 1, 40 + (1 if 2 < i < 5 else 0), CHROME_LO)
    sleeve_creases(c, lo)
    return c


# ---------------------------------------------------------------- B: the back piece

def concept_b():
    c = Canvas(W, H)
    base, hi, lo = "#272c37", "#4a5263", "#151920"
    c.poly(back_outline(c), base)
    c.shade(base, hi, lo)
    # Bleach spatter, the same every time.
    r = random.Random(7)
    for _ in range(70):
        x, y = r.randrange(4, 60), r.randrange(6, 41)
        if c.get(x, y) == base:
            c.px(x, y, mix(base, "#9aa6bb", r.uniform(0.25, 0.6)))
    # The collar from behind, the yoke and its studs.
    c.rect(24, 0, 39, 2, "#1a1e27")
    c.hline(24, 39, 0, hi)
    for x in range(15, 49):
        c.px(x, 8 + (1 if 21 < x < 42 else 0), lo)
    for x in range(17, 47, 4):
        stud(c, x, 10)
    for x, y in [(14, 4), (18, 2), (22, 1)]:
        spike(c, x, y + 1)
        spike(c, W - 1 - x, y + 1)
    # The back patch: stitched round, a skull with live eyes, the slogan in paint.
    c.rect(19, 13, 44, 35, "#0d0d10")
    for x in range(19, 45, 2):
        c.px(x, 13, "#d8d8d8")
        c.px(x + 1, 35, "#d8d8d8")
    for y in range(13, 36, 2):
        c.px(19, y + 1, "#d8d8d8")
        c.px(44, y, "#d8d8d8")
    skull = ["....#####....",
             "..#########..",
             ".###########.",
             ".###########.",
             ".##eE###eE##.",
             ".##EE###EE##.",
             ".#####.#####.",
             "..####.####..",
             "...#######...",
             "...#.#.#.#...",
             "....#####...."]
    c.stamp(26, 15, skull, {"#": "#e8e2d0", "e": "#ffffff", "E": MAGENTA})
    # A jack in the skull's temple, its cord running off the patch.
    c.rect(37, 18, 39, 19, CHROME)
    c.line(40, 19, 43, 22, "#2a2f3a")
    c.line(40, 20, 43, 23, MAGENTA_LO)
    c.rect(20, 28, 43, 31, "#0d0d10")
    c.label(22, 14, "NO CARRIER", MAGENTA)
    c.label(26, 16, "+++ATH0", "#e8e2d0", bold=False)
    for x, y in [(19, 13), (43, 13), (19, 34), (43, 34)]:
        stud(c, x, y)
    # Tartan at the hem and cuffs.
    for y in range(40, 44):
        for x in range(0, W):
            if c.on(x, y):
                red = "#9c1b2a" if (x // 2 + y) % 3 else "#16090b"
                c.px(x, y, "#e0c14a" if x % 6 == 0 and y == 41 else red)
    sleeve_creases(c, lo)
    return c


# ---------------------------------------------------------------- C: hi-vis salvage

def concept_c():
    c = Canvas(W, H)
    base, hi, lo = "#c4541d", "#e8803f", "#7a320f"
    c.poly(front_outline(c), base)
    c.poly([(27, 1), (36, 1), (32, 4), (31, 4)], "#2a1206")
    c.shade(base, hi, lo)
    # Road grime.
    r = random.Random(3)
    for _ in range(120):
        x, y = r.randrange(4, 60), r.randrange(4, 43)
        if c.get(x, y) == base:
            c.px(x, y, mix(base, "#3a1c0c", r.uniform(0.2, 0.5)))
    # Collar and the zip, done up.
    c.rect(26, 1, 37, 3, "#3a1a0a")
    for y in range(4, 43):
        c.px(31, y, STUD if y % 2 else "#2a1206")
        c.px(32, y, "#2a1206" if y % 2 else STUD)
    # Reflective bands across the chest and both sleeves, scuffed.
    for y0, x0, x1 in [(21, 16, 47), (31, 4, 13), (31, 50, 59)]:
        for y in (y0, y0 + 1):
            for x in range(x0, x1 + 1):
                if c.on(x, y) and r.random() > 0.08:
                    c.px(x, y, CHROME if y == y0 else CHROME_LO)
        for x in range(x0 + 2, x1, 9):
            c.px(x, y0, CHROME_HI)
    # The aramid panel sewn on outside, thread big and black.
    weave(c, 18, 8, 29, 17)
    for x in range(18, 30, 2):
        c.px(x, 8, "#1a0a04")
        c.px(x + 1, 17, "#1a0a04")
    for y in range(8, 18, 2):
        c.px(18, y + 1, "#1a0a04")
        c.px(29, y, "#1a0a04")
    c.rect(19, 14, 28, 15, TAN)
    c.label(20, 7, "AR-7", "#1a0a04")
    # Duct tape crosses over the slashes.
    tape, tape_lo = "#a3a8ae", "#6b7077"
    for cx, cy in [(54, 20), (41, 33)]:
        for d in range(-4, 5):
            c.px(cx + d, cy + d, tape)
            c.px(cx + d + 1, cy + d, tape_lo)
            c.px(cx + d, cy - d, tape)
            c.px(cx + d + 1, cy - d, tape_lo)
    # A sprayed tag on the right breast, dripping.
    tag = ["mmmmmmm",
           ".....m.",
           "....m..",
           "...m...",
           "..m....",
           ".m.....",
           "mmmmmmm"]
    c.stamp(37, 9, tag, {"m": MAGENTA})
    for x, n in [(37, 4), (40, 2), (43, 5)]:
        c.vline(x, 16, 16 + n, MAGENTA_LO)
    c.stamp(45, 9, ["..m..", ".m.m.", "m...m", ".m.m.", "..m.."], {"m": MAGENTA})
    sleeve_creases(c, lo)
    return c


# ---------------------------------------------------------------- round 2: brown leather

BROWN, BROWN_HI, BROWN_LO = "#5c3b24", "#8c603c", "#2e1d12"
BRASS, BRASS_LO = "#d9b45a", "#7d6420"
THREAD = "#e0d0a8"


def scuff(c, base, seed, n, toward, lo=0.2, hi=0.45, box=(4, 2, 60, 43)):
    r = random.Random(seed)
    for _ in range(n):
        x, y = r.randrange(box[0], box[2]), r.randrange(box[1], box[3])
        if c.get(x, y) == base:
            c.px(x, y, mix(base, toward, r.uniform(lo, hi)))


def snap(c, x, y):
    c.stamp(x, y, ["ab", "bc"], {"a": "#fff0b8", "b": BRASS, "c": BRASS_LO})


def concept_d():
    """A's cut in worn brown: no spikes, brass snaps, the patches faded to fit."""
    c = Canvas(W, H)
    lining, tee = "#1a100a", "#c9c0a8"
    c.poly(front_outline(c), BROWN)
    c.poly([(27, 1), (36, 1), (32, 4), (31, 4)], lining)
    c.poly([(28, 4), (35, 4), (33, 21), (30, 21)], tee)
    c.stamp(29, 8, [".a.a.", "aaaaa", ".aaa.", "..a.."], {"a": "#8c1830"})
    c.shade(BROWN, BROWN_HI, BROWN_LO)
    # Worn pale at the shoulders and elbows, cracked dark elsewhere.
    scuff(c, BROWN, 11, 90, "#b08458", box=(6, 2, 58, 30))
    scuff(c, BROWN, 12, 60, "#1e120a", 0.3, 0.6)
    lap = "#6b4529"
    for flip in (False, True):
        pts = [(27, 2), (21, 4), (23, 13), (29, 20)]
        if flip:
            pts = [(W - 1 - x, y) for x, y in pts]
        c.poly(pts, lap)
        c.line(*pts[0], *pts[3], BROWN_HI)
    for x, y in [(21, 4), (23, 12)]:
        snap(c, x, y)
        snap(c, W - 2 - x, y)
    for y in range(21, 43):
        c.px(31, y, BRASS if y % 2 else BRASS_LO)
        c.px(32, y, BRASS_LO if y % 2 else BRASS)
    # Epaulettes, buttoned down with brass.
    for x0, x1 in [(13, 22), (41, 50)]:
        c.line(x0, 5 if x0 < 30 else 2, x1, 2 if x0 < 30 else 5, BROWN_LO)
    snap(c, 20, 2)
    snap(c, 42, 2)
    # A faded bolt patch, a round pin, safety pins, the rip with the weave.
    c.rect(17, 24, 25, 31, "#5e1420")
    c.rect(18, 25, 24, 30, "#a8283a")
    c.stamp(19, 25, ["...aa", "..aa.", ".aaaa", "...a.", "..a..", ".a..."], {"a": "#e8e2d0"})
    c.stamp(40, 24, [".aaa.", "abbba", "abcba", "abbba", ".aaa."],
                {"a": "#1a1a1a", "b": "#ffd21a", "c": "#1a1a1a"})
    for y in (18, 24, 30):
        safety_pin(c, 7 if y < 30 else 6, y)
    rip = lambda x, y: (x - 53) ** 2 / 9 + (y - 28) ** 2 / 16 < 1 + 0.4 * ((x * 7 + y * 3) % 3 == 0)
    weave(c, 49, 23, 58, 33, rip)
    for x in range(48, 60, 2):
        c.px(x, 22 + (x % 4 == 0), THREAD)
    sleeve_creases(c, BROWN_LO)
    return c


def concept_e():
    """Stitched together from salvage: odd browns per panel, seams in pale thread, the
    aramid panel sewn on outside, a tape with ZERO on it."""
    c = Canvas(W, H)
    c.poly(front_outline(c), BROWN)
    c.poly([(27, 1), (36, 1), (32, 4), (31, 4)], "#1a100a")
    # Panels: the right front cognac, the left sleeve near black, the yoke tan.
    cognac, oxblood, dark, tan = "#8a4f26", "#5a2420", "#2c1f18", "#9a7048"
    c.recolour(lambda x, y, col: x >= 32 and 13 <= y and x <= 47, cognac)
    c.recolour(lambda x, y, col: x <= 15 and y >= 9 or (x <= 16 and y >= 15), dark)
    c.recolour(lambda x, y, col: y <= 6 + (1 if 20 < x < 43 else 0), tan)
    c.recolour(lambda x, y, col: x >= 48 and 28 <= y <= 36, oxblood)
    for col, hi_, lo_ in [(BROWN, BROWN_HI, BROWN_LO), (cognac, "#b8743c", "#4a2812"),
                          (dark, "#4a3a30", "#140d09"), (tan, "#c49a6a", "#5e4228"),
                          (oxblood, "#7e3a32", "#2e100e")]:
        c.shade(col, hi_, lo_)
    scuff(c, BROWN, 21, 70, "#a87c50")
    scuff(c, cognac, 22, 50, "#3a1e0c", 0.25, 0.5)
    scuff(c, dark, 23, 40, "#6a5a4c", 0.3, 0.5)
    # Seams in pale thread, dashed, where the panels meet.
    def seam(pts):
        for i in range(len(pts) - 1):
            x0, y0 = pts[i]
            x1, y1 = pts[i + 1]
            n = max(abs(x1 - x0), abs(y1 - y0))
            for k in range(0, n + 1, 2):
                c.px(round(x0 + (x1 - x0) * k / n), round(y0 + (y1 - y0) * k / n), THREAD)
    seam([(14, 7), (20, 6), (43, 6), (50, 7)])
    seam([(33, 13), (47, 13)])
    seam([(48, 28), (60, 28)])
    seam([(48, 36), (60, 36)])
    seam([(16, 9), (16, 42)])
    # Cross stitches over a mended slash on the left front.
    c.line(19, 33, 27, 29, BROWN_LO)
    for x in range(19, 28, 2):
        y = 33 - (x - 19) // 2
        c.px(x, y - 1, THREAD)
        c.px(x + 1, y + 1, THREAD)
    # The zip, done half up.
    for y in range(12, 43):
        c.px(31, y, BRASS if y % 2 else BRASS_LO)
        c.px(32, y, BRASS_LO if y % 2 else BRASS)
    c.poly([(28, 4), (35, 4), (32, 12), (31, 12)], "#c9c0a8")
    # The aramid panel on the chest, sewn on with fat stitches.
    weave(c, 18, 10, 27, 19)
    for x in range(18, 28, 2):
        c.px(x, 10, THREAD)
        c.px(x + 1, 19, THREAD)
    for y in range(10, 20, 2):
        c.px(18, y + 1, THREAD)
        c.px(27, y, THREAD)
    # A strip of tape on the cognac side with a word on it.
    c.rect(36, 16, 45, 17, "#d8cfb4")
    c.label(37, 8, "ZERO", "#1a1410")
    for x in (38, 42):
        snap(c, x, 22)
    sleeve_creases(c, BROWN_LO)
    return c


ROUND2 = [
    ("d", "D · Worn brown",
     "A's cut in old brown leather, worn pale at the shoulders and cracked dark "
     "elsewhere. No spikes: brass snaps on the lapels and epaulettes, a brass zip. A "
     "faded red bolt patch, a round pin, safety pins up the sleeve and the rip with "
     "the aramid weave showing, a dirty white tee under.", concept_d),
    ("e", "E · Stitched from salvage",
     "Pieced together from whatever leather there was: a cognac front panel, a "
     "near-black sleeve, a tan yoke, an oxblood patch on the elbow, every seam in pale "
     "thread. The aramid panel is sewn on the chest, a slash is mended with cross "
     "stitches, and a strip of tape on the chest says ZERO.", concept_e),
]

# ---------------------------------------------------------------- round 3: the rest

def tartan(c, x0, y0, x1, y1):
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            if c.on(x, y):
                red = "#9c1b2a" if (x // 2 + y) % 3 else "#16090b"
                c.px(x, y, "#e0c14a" if x % 6 == 0 and y % 4 == 1 else red)


def cargo_pants():
    """Olive cargos: a studded belt, eleven pockets with things in them, a bondage
    strap knee to knee, a tartan knee, a wallet chain, the cuffs rolled."""
    w, h = 40, 44
    c = Canvas(w, h)
    base, hi, lo = "#4b4a35", "#6e6c4e", "#2a2a1d"
    c.poly(c.mirrored([(19, 0), (6, 0), (4, 8), (3, 42), (4, 43), (18, 43), (18, 16),
                       (19, 15)]), base)
    c.shade(base, hi, lo)
    scuff(c, base, 31, 70, "#9a9a80", box=(3, 4, 37, 43))
    # Rolled cuffs, the fly, creases.
    for x0, x1 in [(3, 18), (21, 36)]:
        c.rect(x0, 40, x1, 41, hi)
        c.rect(x0, 42, x1, 43, lo)
    c.vline(19, 3, 13, lo)
    for x, y0, y1 in [(10, 30, 38), (29, 30, 38)]:
        c.vline(x, y0, y1, lo)
    # The belt, studded, its buckle off centre.
    c.rect(5, 1, 34, 2, "#121212")
    for x in range(6, 34, 3):
        c.px(x, 1, STUD_HI)
        c.px(x, 2, STUD_LO)
    c.rect(22, 0, 25, 3, CHROME)
    c.rect(23, 1, 24, 2, "#121212")
    # Pockets: two slash, a coin, two cargo with flap pockets on them, two at the
    # knee, two at the shin: eleven.
    c.line(6, 3, 9, 9, lo)
    c.line(33, 3, 30, 9, lo)
    c.rect(26, 4, 29, 6, lo)
    for x0 in (4, 26):
        c.rect(x0, 17, x0 + 9, 27, lo)
        c.rect(x0 + 1, 18, x0 + 8, 26, base)
        c.rect(x0, 16, x0 + 9, 19, hi)
        c.hline(x0, x0 + 9, 19, lo)
        c.px(x0 + 4, 18, STUD)
        c.rect(x0 + 2, 22, x0 + 6, 25, lo)
        c.hline(x0 + 2, x0 + 6, 22, hi)
    for x0 in (6, 27):
        c.rect(x0, 33, x0 + 6, 37, lo)
        c.hline(x0, x0 + 6, 33, hi)
    for x0 in (13, 21):
        c.rect(x0, 28, x0 + 4, 31, lo)
        c.hline(x0, x0 + 4, 28, hi)
    # What is in them: a screwdriver, a loop of cable, a folded note.
    c.vline(8, 13, 16, "#c8202c")
    c.vline(9, 13, 16, "#ff4050")
    c.px(8, 12, CHROME)
    c.stamp(29, 13, ["m..m", "m..m", ".mm."], {"m": MAGENTA})
    c.rect(34, 30, 36, 32, "#e8e2d0")
    # The strap from knee to knee, rings at its ends; the tartan knee; pins; chain.
    c.rect(12, 30, 27, 31, "#121212")
    for x in (11, 28):
        c.stamp(x - 1, 29, [".a.", "a.a", ".a."], {"a": CHROME})
    tartan(c, 22, 21, 25, 26)
    for y in (24, 28):
        safety_pin(c, 2, y)
    for i, (x, y) in enumerate([(7, 3), (6, 5), (5, 7), (5, 9), (5, 11), (6, 13)]):
        c.px(x, y, CHROME if i % 2 else CHROME_LO)
    return c


def hardhat():
    """The dock's hardhat in profile: scuffed yellow, a dead phone hung on copper wire
    for a visor, the union sticker still on it, DOCK 9 stencilled on the shell."""
    w, h = 60, 36
    c = Canvas(w, h)
    shell, hi, lo = "#e0b21c", "#ffd84a", "#8a6a0a"
    # The dome: half an ellipse; the brim, longer at the front (right).
    for y in range(6, 25):
        for x in range(w):
            if ((x - 27) / 21) ** 2 + ((y - 24) / 18) ** 2 <= 1:
                c.px(x, y, shell)
    c.rect(4, 23, 52, 25, shell)
    c.poly([(44, 23), (56, 24), (56, 26), (44, 26)], shell)
    c.shade(shell, hi, lo)
    c.hline(5, 50, 26, lo)
    # Ribs over the crown.
    for x0 in (20, 34):
        for y in range(8, 23):
            c.px(x0 + (y - 8) // 6 * (1 if x0 > 27 else -1), y, lo)
    scuff(c, shell, 41, 80, "#5a4508", 0.2, 0.5, box=(6, 6, 50, 26))
    # DOCK 9 on the shell.
    c.rect(11, 16, 26, 19, hi)
    c.label(12, 8, "DOCK 9", "#1a1404")
    # The visor: a dead phone hung under the front of the brim on two struts of
    # copper wire, wound round the brim where they hold; tape down its side, its
    # glass cracked, the union sticker in the corner.
    copper, copper_lo = "#e08a3c", "#7a3a10"
    for x in (46, 53):
        for y in range(23, 29):
            c.px(x, y, copper if y % 2 else copper_lo)
            c.px(x + 1, y, copper_lo if y % 2 else copper)
    c.rect(43, 28, 57, 35, "#151515")
    c.rect(44, 29, 55, 34, "#0b1a22")
    for i, y in enumerate((30, 32)):
        c.hline(45, 49 + 3 * i, y, "#00e5ff")
    c.hline(45, 46, 34, "#00e5ff")
    c.line(50, 29, 53, 34, "#cfefff")
    c.rect(56, 28, 57, 35, "#a3a8ae")
    c.stamp(52, 29, [".rr.", "rwwr", ".rr."], {"r": "#c8202c", "w": "#f0f0f0"})
    # A sticker on the crown.
    c.rect(28, 10, 33, 13, "#f0f0f0")
    c.rect(29, 11, 32, 12, "#151515")
    # Nothing stands above the dome: drop the empty rows over it (an even count, so
    # the label keeps its cell).
    c.p, c.h = c.p[6:], c.h - 6
    c.labels = [(x, y - 3, t, f, b) for x, y, t, f, b in c.labels]
    return c


def mirrorshades():
    """Wraparound chrome: a neon street in the lenses, one lens cracked, the bridge
    taped, a safety pin for a hinge."""
    w, h = 64, 22
    c = Canvas(w, h)
    frame, frame_hi = "#1a1c22", "#5a606c"
    sky = ["#0b1030", "#1b1650", "#3a1868", "#6a1a78", "#b01e86", "#ff2bd6", "#ff7ae0"]
    for flip in (False, True):
        pts = [(5, 5), (29, 4), (29, 9), (26, 16), (18, 18), (9, 17), (5, 12)]
        if flip:
            pts = [(w - 1 - x, y) for x, y in pts]
        c.poly(pts, frame)
        inner = [(7, 6), (28, 5), (28, 9), (25, 15), (18, 16), (10, 16), (7, 12)]
        if flip:
            inner = [(w - 1 - x, y) for x, y in inner]
        lens = Canvas(w, h)
        lens.poly(inner, "#000000")
        # The street reflected: sky down to a pink horizon, towers of a few widths
        # against it, a lit window here and there.
        r = random.Random(5 if flip else 6)
        top = {}
        x = 0
        while x < w:
            span, height = r.choice((2, 3, 4)), r.randrange(10, 14)
            for k in range(span):
                top[x + k] = height
            x += span
        for y in range(h):
            for x in range(w):
                if lens.on(x, y):
                    col = sky[min(len(sky) - 1, max(0, (y - 5) * len(sky) // 9))]
                    if y >= top[x]:
                        lit = r.random() < 0.12 and y > top[x]
                        col = r.choice(("#00e5ff", "#ffd21a")) if lit else "#07060c"
                    c.px(x, y, col)
        # One glare across each lens.
        x0 = 49 if flip else 13
        c.line(x0, 6, x0 + 3, 10, "#ffffff")
        c.line(x0 + 1, 6, x0 + 4, 10, "#ffd6f6")
    # Brow bar and the temples reaching back.
    c.hline(5, 58, 4, frame_hi)
    c.hline(5, 58, 3, frame)
    c.rect(0, 4, 5, 6, frame)
    c.rect(58, 4, 63, 6, frame)
    c.hline(0, 4, 4, frame_hi)
    # The bridge, taped.
    c.rect(29, 4, 34, 9, "#e8e4d8")
    for y in (5, 7):
        c.hline(29, 34, y, "#b8b4a8")
    # The cracked lens.
    c.line(40, 6, 43, 10, "#ffffff")
    c.line(43, 10, 41, 13, "#ffffff")
    c.line(43, 10, 48, 12, "#ffffff")
    c.line(48, 12, 52, 11, "#ffffff")
    # A safety pin through the left hinge.
    c.stamp(0, 7, ["aaaab", "c...b", "aaaab"], {"a": CHROME, "b": CHROME_HI, "c": CHROME_LO})
    return c


def ballistic_weave():
    """A vest of layered weave, quilted, buckled shut, patched and pinned, with two
    flattened slugs still caught in it: the burst it stopped."""
    w, h = 52, 44
    c = Canvas(w, h)
    base, hi, lo = "#34373e", "#5a5f6a", "#17191d"
    c.poly(c.mirrored([(25, 14), (17, 0), (11, 0), (9, 6), (6, 12), (3, 17), (4, 43),
                       (25, 43)]), base)
    c.poly([(17, 0), (34, 0), (26, 14), (25, 14)], "#0c0c0f")
    c.shade(base, hi, lo)
    # The weave, and quilting every five rows.
    for y in range(h):
        for x in range(w):
            if c.get(x, y) == base:
                if y % 5 == 0:
                    c.px(x, y, lo)
                elif (x + 2 * y) % 4 == 0:
                    c.px(x, y, "#2a2d33")
    # Studs round the armholes.
    for x, y in [(10, 4), (8, 8), (6, 12), (4, 17)]:
        stud(c, x, y)
        stud(c, w - 2 - x, y)
    # Three straps buckled across the front.
    for y in (19, 27, 35):
        c.rect(18, y, 33, y + 1, "#0d0d0f")
        c.rect(24, y - 1, 27, y + 2, CHROME)
        c.rect(25, y, 26, y + 1, "#0d0d0f")
    # Patches and button badges.
    c.rect(7, 22, 14, 28, "#5e1420")
    c.rect(8, 23, 13, 27, "#a8283a")
    c.line(9, 24, 12, 26, "#e8e2d0")
    c.line(12, 24, 9, 26, "#e8e2d0")
    for (x, y), ring in zip([(37, 18), (42, 20), (38, 23)], [MAGENTA, "#ffd21a", "#00e5ff"]):
        c.stamp(x, y, [".rr.", "rkkr", "rkkr", ".rr."], {"r": ring, "k": "#111111"})
    tartan(c, 7, 34, 14, 40)
    # Two slugs caught in it, the weave torn pale round them.
    for x, y in [(16, 13), (37, 31)]:
        for dx in range(-3, 4):
            for dy in range(-3, 4):
                if 5 <= dx * dx + dy * dy <= 10 and c.on(x + dx, y + dy):
                    c.px(x + dx, y + dy, "#8a8f99" if (dx + dy) % 2 else lo)
        c.stamp(x - 1, y - 1, ["abb", "bbc", "bcc"],
                {"a": "#f0b070", "b": "#c87a3a", "c": "#7a4018"})
    return c


ROUND3 = [
    ("salvage_jacket", "Salvage jacket · D, picked", "Worn brown leather, no spikes.",
     concept_d),
    ("ballistic_weave", "Ballistic weave",
     "A vest of layered weave, quilted and buckled shut, studs round the armholes, a "
     "red X patch, three button badges, a tartan patch, and two flattened slugs still "
     "caught in it, the weave torn pale round each: \"stops the first burst and "
     "remembers the second\". A, B or C from round 1 could take its place.",
     ballistic_weave),
    ("cargo_pants", "Cargo pants",
     "Olive cargos with a studded belt, eleven pockets (a screwdriver, a loop of cable "
     "and a note sticking out of them), a bondage strap from knee to knee, a tartan "
     "knee, safety pins, a wallet chain, the cuffs rolled.", cargo_pants),
    ("hardhat", "Hardhat with visor",
     "The dock's hardhat in profile, scuffed yellow, DOCK 9 stencilled on the shell. "
     "The visor is the recipe: a dead phone "
     "hinged on with copper wire, its screen lit, tape down one side and the union "
     "sticker still on it.", hardhat),
    ("mirrorshades", "Mirrorshades",
     "Cheap wraparound chrome with a neon street in the lenses, one lens cracked, "
     "the bridge wrapped in tape and a safety pin for a hinge.", mirrorshades),
]

CONCEPTS = [
    ("a", "A · Studs &amp; patches",
     "Black leather courier, biker lapels edged in studs, spikes on the shoulders, a "
     "red tee under. A bolt patch and a hazard patch on the chest, safety pins up one "
     "sleeve, and a rip in the other with the aramid weave showing through. A chain off "
     "the hem.", concept_a),
    ("b", "B · The back piece",
     "Turned round to show its back: black denim spattered with bleach, a studded yoke, "
     "shoulder spikes, a stitched patch with a skull, a jack in its temple and its eyes "
     "lit, NO CARRIER sprayed over +++ATH0. Tartan at the hem.", concept_b),
    ("c", "C · Hi-vis salvage",
     "A courier's orange shell gone feral: grime, scuffed reflective bands, the aramid "
     "panel sewn on outside with fat black stitches and stencilled AR-7, duct-tape "
     "crosses over the slashes, a sprayed Z tag still dripping.", concept_c),
]


# ---------------------------------------------------------------- the screens

def find(g, ch, x=None):
    for y in range(g.h):
        for i in range(g.w):
            if g.c[y][i][0] == ch and (x is None or i == x):
                return i, y
    return None


def blueprint(art, w, h, item="salvage_jacket"):
    g = Grid.from_svg(os.path.join(S, "blueprint_armor_%s_%dx%d.svg" % (item, w, h)), w, h)
    # The body: between the bezel's sides, under its top, over the status line.
    left, top = find(g, "▗")
    right = find(g, "▖")[0]
    bottom = find(g, "▝")[1]
    x0, x1, y0, y1 = left + 1, right - 1, top + 1, bottom - 2
    g.fill(x0, y0, x1 - x0 + 1, y1 - y0 + 1, PANEL)
    cells = art.cells(PANEL)
    ox = x0 + (x1 - x0 + 1 - cells.w) // 2
    oy = y0 + (y1 - y0 + 1 - cells.h) // 2
    g.paste(cells, ox, oy)
    return g


def kit(art, w, h, item="salvage_jacket"):
    g = Grid.from_svg(os.path.join(S, "kit_armor_%s_%dx%d.svg" % (item, w, h)), w, h)
    # The viewer: the row with its caption holds the lower ticks, and the upper ticks
    # stand over them; the dashed box between is cleared.
    rows = ["".join(cell[0] for cell in r) for r in g.c]
    vb = next(y for y, r in enumerate(rows) if "space inspect ┘" in r)
    vr = rows[vb].index("┘", rows[vb].index("space inspect"))
    vx = rows[vb].rindex("└", 0, vr)
    vy = next(y for y in range(vb - 1, -1, -1) if rows[y][vx] == "┌" and rows[y][vr] == "┐")
    g.fill(vx + 1, vy + 1, vr - vx - 1, vb - vy - 1, PANEL)
    cells = art.half().cells(PANEL)
    ox = vx + (vr - vx + 1 - cells.w) // 2
    oy = vy + (vb - vy + 1 - cells.h) // 2
    g.paste(cells, ox, oy)
    return g


ROUNDS = {
    "1": ("Armor art · round 1: the Salvage jacket, three ways",
          "Drawn into the real blueprint window (120×40) and the KIT viewer, which shows "
          "it at half size. All three are kept for other pieces.", CONCEPTS),
    "2": ("Armor art · round 2: the Salvage jacket in brown leather",
          "No spikes. A, B and C stay kept for other pieces (round 1).", ROUND2),
    "3": ("Armor art · round 3: all five pieces",
          "D for the jacket; the other four drawn to sit beside it.", ROUND3),
}


def page(rnd):
    title, sub, concepts = ROUNDS[rnd]
    out = ["<h1>%s</h1>" % title, '<p class="sub">%s</p>' % sub]
    for key, title, blurb, draw in concepts:
        art = draw()
        out.append('<section class="opt" data-choice="%s" onclick="toggleSelect(this)">' % key)
        out.append("<h2>%s</h2><p>%s</p>" % (title, blurb))
        out.append('<div class="pair">')
        item = key if len(key) > 1 else "salvage_jacket"
        out.append("<figure>%s<figcaption>The blueprint window, 120×40</figcaption></figure>"
                   % term.render(blueprint(art, 120, 40, item), 9.0))
        out.append("<figure>%s<figcaption>GEAR &amp; INVENTORY, 100×30: the viewer at half "
                   "size</figcaption></figure>" % term.render(kit(art, 100, 30, item), 9.0))
        out.append("</div></section>")
    return "".join(out)


CSS = """
:root{--bg:#0a0d13;--fg:#cdd4de;--muted:#8a95a5;--rule:#1d2a38;--head:#ffffff;
 --cyan:#00e5ff;--magenta:#ff00c8;color-scheme:dark}
""" + term.CSS + """
body{background:var(--bg);color:var(--fg);font:15px/1.55 -apple-system,system-ui,sans-serif;
 margin:0;padding:24px 16px 80px}
main{max-width:1600px;margin:0 auto}
h1{font-size:24px;margin:0 0 4px;color:var(--head);text-wrap:balance}
h2{font-size:18px;margin:0 0 6px;color:var(--cyan)}
p{max-width:900px}
.sub{color:var(--muted);margin:0 0 18px}
.opt{border:1px solid var(--rule);border-radius:8px;padding:16px;margin:22px 0;cursor:pointer}
.opt.selected{border-color:var(--magenta);box-shadow:0 0 0 1px var(--magenta)}
figure{margin:8px 0;overflow-x:auto;max-width:100%;min-width:0}
figcaption{color:var(--muted);font-size:13px;margin-top:6px}
.pair{display:flex;gap:18px;flex-wrap:wrap;align-items:flex-start}
"""


def main():
    html = ("<!DOCTYPE html><html><head><meta charset='utf-8'>"
            "<meta name='viewport' content='width=device-width,initial-scale=1'>"
            "<title>Armor art</title><style>%s</style></head><body><main>%s</main>"
            "</body></html>" % (CSS, page(ROUND)))
    with open(OUT, "w") as f:
        f.write(html)


if __name__ == "__main__":
    main()
