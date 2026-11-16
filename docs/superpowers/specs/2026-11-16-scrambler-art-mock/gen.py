"""The field scrambler's picture: three takes (round 1, B picked), then B in fewer
colours (round 2, B2 picked: `field_scrambler`), drawn into the game's real blueprint
window and the GEAR & INVENTORY card. Uses the armor art mock's pixel canvas; the armor
mock's export.py writes the picked drawing to flavor/item_art.ron.

    SHOT_DIR=<dir> cargo test -p strata-zero --target-dir target/smoke -- --ignored \\
        scrambler_on_its_blueprint
    python3 -I gen.py <dir> <out.html> [round]
"""

import os
import random
import sys

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
ARMOR = os.path.join(HERE, "..", "2026-11-10-armor-art-mock")
sys.path.insert(0, ARMOR)
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402
from px import Canvas  # noqa: E402

S = sys.argv[1] if len(sys.argv) > 1 else os.path.join(HERE, "shots")
OUT = sys.argv[2] if len(sys.argv) > 2 else os.path.join(HERE, "index.html")
PANEL = "#04060a"
ITEM = "field_scrambler"

W, H = 60, 40

CHROME_HI, CHROME, CHROME_LO = "#ffffff", "#c9ced6", "#6b717b"
CASE, CASE_HI, CASE_LO = "#2a2d33", "#4a4f58", "#15171b"
WEB, WEB_HI, WEB_LO = "#3b3f2e", "#5c624a", "#22251a"
COPPER, COPPER_HI, COPPER_LO = "#c8763a", "#f0b070", "#6e3a16"
CYAN, MAGENTA, YELLOW, RED = "#00e5ff", "#ff2bd6", "#ffd21a", "#c8202c"
NOISE = ["#0b0d12", "#0b0d12", "#2a2e36", "#5a606c", "#9aa1aa", "#c9ced6", CYAN, MAGENTA]


def screw(c, x, y):
    c.stamp(x, y, ["ab", "bc"], {"a": CHROME_HI, "b": CHROME, "c": CHROME_LO})


def static(c, x0, y0, x1, y1, seed):
    r = random.Random(seed)
    for y in range(y0, y1 + 1):
        for x in range(x0, x1 + 1):
            c.px(x, y, r.choice(NOISE))


def webbing(c, pts, stitch_every=3):
    """A strap: the polygon in webbing, stitched along its outline."""
    c.poly(pts, WEB)
    c.shade(WEB, WEB_HI, WEB_LO)
    n = 0
    for y in range(c.h):
        for x in range(c.w):
            if c.get(x, y) == WEB_HI:
                n += 1
                if n % stitch_every == 0:
                    c.px(x, y, "#8a8f74")


def coil(c, cx, y, w):
    """A band of copper wound round a rod, `w` wide, two rows."""
    for x in range(cx - w // 2, cx + w // 2 + 1):
        c.px(x, y, COPPER_HI if x < cx else COPPER)
        c.px(x, y + 1, COPPER if x < cx else COPPER_LO)


def pips(c, x, y, lit, of=4, w=4):
    for i in range(of):
        col = CYAN if i < lit else "#0e2a30"
        c.rect(x + i * (w + 1), y, x + i * (w + 1) + w - 1, y + 1, col)


# ---------------------------------------------------------------- A: the servo case

def servo_case():
    """A servo's case on a webbing strap, the gutted phone's cracked glass for a face
    full of static, the coil aerial standing off its shoulder, the charge in pips."""
    c = Canvas(W, H)
    # The strap behind it, a buckle on the left.
    webbing(c, [(0, 22), (59, 22), (59, 28), (0, 28)])
    c.rect(4, 20, 9, 30, CHROME_LO)
    c.rect(5, 21, 8, 29, CHROME)
    c.rect(6, 22, 7, 28, WEB_LO)
    # The case.
    c.rect(14, 12, 45, 36, CASE)
    c.shade(CASE, CASE_HI, CASE_LO)
    for x, y in [(15, 13), (43, 13), (15, 34), (43, 34)]:
        screw(c, x, y)
    # The phone's glass, cracked, full of static.
    c.rect(18, 15, 41, 26, "#0b0d12")
    static(c, 19, 16, 40, 25, 3)
    c.line(23, 16, 27, 20, "#ffffff")
    c.line(27, 20, 25, 25, "#ffffff")
    c.line(27, 20, 33, 22, "#ffffff")
    # The charge, three of four, and the switch.
    pips(c, 18, 29, 3)
    c.rect(39, 28, 41, 31, CASE_LO)
    c.rect(40, 28, 40, 29, RED)
    # Hazard tape round the foot.
    for x in range(14, 46):
        c.px(x, 35, YELLOW if (x // 2) % 2 else "#16140a")
    # The aerial off the top corner: a rod, three coils, a bead.
    c.vline(40, 2, 11, CHROME_LO)
    c.vline(41, 2, 11, CHROME)
    for y in (3, 6, 9):
        coil(c, 40, y, 6)
    c.rect(40, 0, 41, 1, MAGENTA)
    return c


# ---------------------------------------------------------------- B: the harness

CYAN_DIM = "#0e5a66"
GREYS = ["#2a2e36", "#5a606c", "#9aa1aa"]


def harness(noise=None, pupil=MAGENTA, bar=RED):
    """As it's worn: two straps crossed, the box where they cross, a loop of copper
    wound round it, and the field of noise it throws in the `noise` colours. The eye on
    the box has a `pupil`, and a `bar` through it unless that's None."""
    noise = noise or NOISE[2:]
    c = Canvas(W, H)
    cx, cy = 29.5, 19.5
    r = random.Random(11)
    # The field: static thinning out from the loop.
    for y in range(H):
        for x in range(W):
            d = ((x - cx) ** 2 + ((y - cy) * 1.5) ** 2) ** 0.5
            if 16 < d < 29 and r.random() < 0.55 * (1 - (d - 16) / 13):
                c.px(x, y, r.choice(noise))
    # The straps, crossed.
    webbing(c, [(0, 2), (5, 0), (59, 35), (54, 39)])
    webbing(c, [(54, 0), (59, 2), (5, 39), (0, 35)])
    # The loop: an oval of copper round the box.
    for y in range(H):
        for x in range(W):
            d = ((x - cx) ** 2 + ((y - cy) * 1.5) ** 2) ** 0.5
            if 13 <= d < 15.5:
                c.px(x, y, COPPER_HI if y < cy - 4 else (COPPER_LO if y > cy + 4 else COPPER))
    # The box where the straps cross.
    c.rect(20, 12, 39, 27, CASE)
    c.shade(CASE, CASE_HI, CASE_LO)
    for x, y in [(21, 13), (37, 13), (21, 25), (37, 25)]:
        screw(c, x, y)
    # An eye with a bar through it, and the charge.
    c.stamp(24, 15, ["...aaaaaa...",
                     ".aa......aa.",
                     "a...bbbb...a",
                     ".aa.bbbb.aa.",
                     "...aaaaaa..."], {"a": CHROME, "b": pupil})
    if bar:
        c.line(24, 21, 35, 14, bar)
    pips(c, 23, 23, 3, w=3)
    return c


# ---------------------------------------------------------------- C: the matchbox

def matchbox():
    """A matchbox, the sleeve's label an eye crossed out, its drawer slid open on coils
    and the phone's board; a webbing loop through the back."""
    c = Canvas(W, H)
    # The loop through the back, above and below the sleeve.
    webbing(c, [(16, 0), (24, 0), (24, 39), (16, 39)])
    # The drawer, slid out right: cardboard walls, the coils and the board inside.
    card, card_hi, card_lo = "#b08a5a", "#d8b480", "#6e5232"
    c.rect(32, 13, 56, 31, card)
    c.shade(card, card_hi, card_lo)
    c.rect(34, 15, 54, 29, "#3a2a18")
    for i, x in enumerate((36, 42)):
        for y in range(16, 28):
            c.px(x, y, COPPER_HI if y % 2 else COPPER)
            c.px(x + 1, y, COPPER if y % 2 else COPPER_LO)
            c.px(x + 2, y, COPPER_LO)
            c.px(x + 3, y, COPPER if y % 2 else COPPER_LO)
    c.rect(48, 16, 53, 28, "#1c4a2a")
    c.rect(49, 18, 52, 21, "#0b0d12")
    for y in (23, 25, 27):
        c.hline(49, 52, y, "#d0b050")
    c.rect(55, 21, 55, 23, CYAN)  # the drawer's lamp
    # The sleeve, its top edge lit, the striking strip down its side.
    sleeve = "#e8e2d0"
    c.poly([(4, 13), (36, 13), (38, 10), (6, 10)], "#fff8e8")
    c.rect(4, 13, 36, 33, sleeve)
    c.rect(4, 13, 5, 33, "#5a3a2a")
    for y in range(13, 34, 2):
        c.px(4, y, "#8a5a40")
    # The label: red frame, an eye with a bar through it, pips for the charge.
    c.rect(8, 15, 34, 31, RED)
    c.rect(9, 16, 33, 30, sleeve)
    c.stamp(12, 18, ["......aaaaaaaa......",
                     "...aaa........aaa...",
                     ".aa.....bbbb.....aa.",
                     "a......bbbbbb......a",
                     ".aa.....bbbb.....aa.",
                     "...aaa........aaa...",
                     "......aaaaaaaa......"], {"a": "#16140a", "b": MAGENTA})
    c.line(12, 26, 31, 17, RED)
    c.line(12, 27, 31, 18, RED)
    pips(c, 12, 28, 3)
    return c


def field_scrambler():
    """The picked drawing: B2, the harness with its noise in cyan only."""
    return harness([CYAN, CYAN_DIM, CYAN_DIM], CYAN, None)


ROUND2 = [
    ("B", "B · As it was",
     "Noise in greys, cyan and magenta; a magenta pupil and a red bar on the eye.",
     harness),
    ("B2", "B2 · Cyan only",
     "The noise in cyan and a dim cyan, the pupil cyan, no bar: copper, olive, the case "
     "and one accent.",
     field_scrambler),
    ("B3", "B3 · Grey noise",
     "The noise in three greys, so the only colour is on the box: a cyan pupil and the "
     "charge.",
     lambda: harness(GREYS, CYAN, None)),
]

CONCEPTS = [
    ("A", "A · The servo case",
     "A servo's case on a webbing strap. The phone's cracked glass is the face, full of "
     "static. The coil aerial stands off its shoulder, and the charge shows as pips.",
     servo_case),
    ("B", "B · The harness",
     "As it's worn: two straps crossed, the box where they cross, a loop of copper round "
     "it, and the field of noise it throws.", harness),
    ("C", "C · The matchbox",
     "Taken at its word: a matchbox with an eye crossed out on the label, the drawer slid "
     "open on the coils and the phone's board, a strap through the back.", matchbox),
]


# ---------------------------------------------------------------- the screens

def find(g, ch):
    for y in range(g.h):
        for i in range(g.w):
            if g.c[y][i][0] == ch:
                return i, y
    return None


def blueprint(art, w, h):
    g = Grid.from_svg(os.path.join(S, "blueprint_item_%s_%dx%d.svg" % (ITEM, w, h)), w, h)
    left, top = find(g, "▗")
    right = find(g, "▖")[0]
    bottom = find(g, "▝")[1]
    x0, x1, y0, y1 = left + 1, right - 1, top + 1, bottom - 2
    g.fill(x0, y0, x1 - x0 + 1, y1 - y0 + 1, PANEL)
    cells = art.cells(PANEL)
    g.paste(cells, x0 + (x1 - x0 + 1 - cells.w) // 2, y0 + (y1 - y0 + 1 - cells.h) // 2)
    return g


def kit(art, w, h):
    """The card's half-size drawing, in the dashed box over `space inspect`; None when
    the card has no room for it at this size."""
    g = Grid.from_svg(os.path.join(S, "kit_item_%s_%dx%d.svg" % (ITEM, w, h)), w, h)
    rows = ["".join(cell[0] for cell in r) for r in g.c]
    vb = next((y for y, r in enumerate(rows) if "space inspect ┘" in r), None)
    if vb is None:
        return None
    vr = rows[vb].index("┘", rows[vb].index("space inspect"))
    vx = rows[vb].rindex("└", 0, vr)
    vy = next(y for y in range(vb - 1, -1, -1) if rows[y][vx] == "┌" and rows[y][vr] == "┐")
    g.fill(vx + 1, vy + 1, vr - vx - 1, vb - vy - 1, PANEL)
    cells = art.half().cells(PANEL)
    g.paste(cells, vx + (vr - vx + 1 - cells.w) // 2, vy + (vb - vy + 1 - cells.h) // 2)
    return g


ROUNDS = {
    "1": ("The field scrambler, three ways", CONCEPTS),
    "2": ("The field scrambler · round 2: B with fewer colours", ROUND2),
}
ROUND = sys.argv[3] if len(sys.argv) > 3 else "1"


def page():
    heading, concepts = ROUNDS[ROUND]
    out = ["<h1>%s</h1>" % heading,
           '<p class="sub">Drawn into the real blueprint window and the GEAR &amp; INVENTORY '
           "card, which shows it at half size. Click one to mark it.</p>"]
    for key, title, blurb, draw in concepts:
        art = draw()
        out.append('<section class="opt" onclick="this.classList.toggle(\'selected\')">')
        out.append("<h2>%s</h2><p>%s</p><div class=\"pair\">" % (title, blurb))
        out.append("<figure>%s<figcaption>The blueprint window, 120×40</figcaption></figure>"
                   % term.render(blueprint(art, 120, 40), 9.0))
        for w, h in [(120, 40), (100, 30)]:
            g = kit(art, w, h)
            if g is not None:
                out.append("<figure>%s<figcaption>GEAR &amp; INVENTORY, %d×%d: the card at "
                           "half size</figcaption></figure>" % (term.render(g, 9.0), w, h))
                break
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
            "<title>Scrambler art</title><style>%s</style></head><body><main>%s</main>"
            "</body></html>" % (CSS, page()))
    with open(OUT, "w") as f:
        f.write(html)


if __name__ == "__main__":
    main()
