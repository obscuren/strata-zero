"""TALENTS, one tree at a time: three concepts on the deck's real frame (B was chosen).

    python3 -I gen.py SCRATCH m1.json && python3 -I page.py m1.json talent-trees.html

The frame, header and tab bar come from real shots of the deck on TALENTS at 100x30 and
120x40, SCRATCH/shots/tmp_talents_WxH.svg. They were taken with a throwaway ignored test
in screens/shots.rs (reverted after): the deck's `debug_runner`, `Deck::new`, a `0` key
to TALENTS, 200 ticks on, `shot_stack("tmp_talents_{w}x{h}", ...)`, run with SHOT_DIR
set. The body is drawn here: `trees.py` is the content, `icons.py` the first twelve
icons (the full set lives in ../2026-10-31-talent-board-art/talentart.py).
"""

import html
import os
import sys
import textwrap

sys.dont_write_bytecode = True
HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
from cells import Grid  # noqa: E402
from cellhtml import CSS, grid_html  # noqa: E402
from icons import ICONS, MAT  # noqa: E402
from trees import TREES, GATE  # noqa: E402

S = sys.argv[1]

CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
DIM = "#a0aab9"
FAINT = "#78828f"
FAINTER = "#5c6673"
TEXT = "#cdd4de"
WHITE = "#f2f8ff"
PANEL = "#04060a"
RULE = "#283c4b"
BAR = "#003c46"
GOOD = "#50e678"
AMBER = "#ffc850"
RED = "#ff4d6a"
OFF = "#2a3038"      # a locked icon's border
ARROW_OFF = "#3a4250"


def hexrgb(c):
    return tuple(int(c[i:i + 2], 16) for i in (1, 3, 5))


def rgbhex(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v))) for v in t)


def mix(a, b, t):
    ra, rb = hexrgb(a), hexrgb(b)
    return rgbhex(tuple(ra[k] + (rb[k] - ra[k]) * t for k in range(3)))


def grey(c, dark=0.55):
    r, g, b = hexrgb(c)
    l = 0.3 * r + 0.59 * g + 0.11 * b
    return rgbhex(tuple((0.2 * v + 0.8 * l) * dark for v in (r, g, b)))


# ---------------------------------------------------------------- the character

RANKS = {"quick_draw": 3, "double_tap": 2, "light_step": 2, "steady_hand": 1, "ice_pick": 2}
POINTS_LEFT = 2


class Tree:
    def __init__(self, idx):
        self.idx = idx
        self.name, self.colour, self.line, rows = TREES[idx]
        self.talents = [dict(tier=r[0], col=r[1], id=r[2], name=r[3], ranks=r[4], req=r[5], new=r[6],
                             words=r[7], desc=r[8]) for r in rows]
        self.by_id = {t["id"]: t for t in self.talents}
        self.tiers = max(t["tier"] for t in self.talents) + 1

    def points(self):
        return sum(RANKS.get(t["id"], 0) for t in self.talents)

    def rank(self, tid):
        return RANKS.get(tid, 0)

    def at(self, tier, col):
        for t in self.talents:
            if t["tier"] == tier and t["col"] == col:
                return t
        return None

    def needs(self, t):
        """(ok, words) for each thing the talent needs: the tier's points, the talent above."""
        out = []
        gate = GATE * t["tier"]
        have = self.points()
        if gate:
            if have >= gate:
                out.append((True, "%d points in %s" % (gate, self.name)))
            else:
                out.append((False, "%d points in %s · %d more" % (gate, self.name, gate - have)))
        if t["req"]:
            r = self.by_id[t["req"]]
            out.append((self.rank(r["id"]) >= r["ranks"], "%s %d/%d" % (r["name"], r["ranks"], r["ranks"])))
        return out

    def state(self, t):
        rank = self.rank(t["id"])
        if rank >= t["ranks"]:
            return "max"
        if all(ok for ok, _ in self.needs(t)) and POINTS_LEFT > 0:
            return "avail"
        return "part" if rank else "locked"

    def lit(self, t):
        return self.state(t) != "locked"

    def chain(self, t):
        """The line the talent sits in: up through what it needs, down through what needs it."""
        up = []
        cur = t
        while cur["req"]:
            cur = self.by_id[cur["req"]]
            up.insert(0, cur)
        down = []
        cur = t
        while True:
            kids = [k for k in self.talents if k["req"] == cur["id"]]
            if len(kids) != 1:
                break
            cur = kids[0]
            down.append(cur)
        return up + [t] + down


# ---------------------------------------------------------------- quadrant layer

QCH = {
    frozenset(): " ", frozenset([0]): "▘", frozenset([1]): "▝", frozenset([2]): "▖", frozenset([3]): "▗",
    frozenset([0, 1]): "▀", frozenset([2, 3]): "▄", frozenset([0, 2]): "▌", frozenset([1, 3]): "▐",
    frozenset([0, 3]): "▚", frozenset([1, 2]): "▞", frozenset([0, 1, 2]): "▛", frozenset([0, 1, 3]): "▜",
    frozenset([0, 2, 3]): "▙", frozenset([1, 2, 3]): "▟", frozenset([0, 1, 2, 3]): "█",
}
UL, UR, LL, LR = 0, 1, 2, 3
TOP_HALF, BOT_HALF, LEFT_HALF, RIGHT_HALF = (UL, UR), (LL, LR), (UL, LL), (UR, LR)


class Quads:
    def __init__(self):
        self.q = {}

    def set(self, x, y, quads, colour):
        d = self.q.setdefault((x, y), {})
        for k in quads:
            d[k] = colour

    def flush(self, g):
        for (x, y), d in self.q.items():
            if not (0 <= x < g.w and 0 <= y < g.h):
                continue
            ground = g.c[y][x][2]
            cols = list(dict.fromkeys(d.values()))
            a = cols[0]
            qa = frozenset(k for k, v in d.items() if v == a)
            if len(cols) == 1:
                g.c[y][x] = [QCH[qa], a, ground, False]
            else:
                b = cols[1]
                qb = frozenset(k for k, v in d.items() if v == b)
                if qa | qb == frozenset([0, 1, 2, 3]):
                    g.c[y][x] = [QCH[qa], a, b, False]
                else:
                    g.c[y][x] = [QCH[qa | qb], a, ground, False]


# ---------------------------------------------------------------- pieces

def tile_of(colour):
    return mix(PANEL, colour, 0.16)


def draw_icon(g, x, y, tree, t, lit=None):
    lit = tree.lit(t) if lit is None else lit
    rows = ICONS.get(t["id"])
    if rows is None:
        rows = ["......", ".cccc.", ".c..c.", ".c..c.", ".cccc.", "......"]
    pal = dict(MAT)
    pal["c"] = tree.colour
    pal["C"] = mix(tree.colour, "#ffffff", 0.5)
    tile = tile_of(tree.colour)
    for cy in range(3):
        for cx in range(6):
            tc = tile if rows[2 * cy][cx] == "." else pal[rows[2 * cy][cx]]
            bc = tile if rows[2 * cy + 1][cx] == "." else pal[rows[2 * cy + 1][cx]]
            if not lit:
                tc, bc = grey(tc), grey(bc)
            if tc == bc:
                g.c[y + cy][x + cx] = [" ", tc, tc, False]
            else:
                g.c[y + cy][x + cx] = ["▀", tc, bc, False]


def frame(q, x, y, colour, top=True, bottom=True):
    """A half-block frame hugging a 6x3 icon at x, y."""
    if top:
        q.set(x - 1, y - 1, [LR], colour)
        for i in range(6):
            q.set(x + i, y - 1, BOT_HALF, colour)
        q.set(x + 6, y - 1, [LL], colour)
    if bottom:
        q.set(x - 1, y + 3, [UR], colour)
        for i in range(6):
            q.set(x + i, y + 3, TOP_HALF, colour)
        q.set(x + 6, y + 3, [UL], colour)
    for j in range(3):
        q.set(x - 1, y + j, RIGHT_HALF, colour)
        q.set(x + 6, y + j, LEFT_HALF, colour)


def badge(g, x, y, tree, t, cursor=False):
    st = tree.state(t)
    fg = {"max": AMBER, "avail": GOOD, "part": GOOD, "locked": FAINTER}[st]
    s = "%d/%d" % (tree.rank(t["id"]), t["ranks"])
    g.put(x, y, s, fg, bold=cursor or st != "locked")


def arrows(g, q, tree, pos, heads, bar=True):
    """Prerequisite arrows: a bar from the talent above, an arrowhead on the target.
    `pos(tier, col)` is an icon's top-left cell. Heads are drawn after the flush."""
    for t in tree.talents:
        if not t["req"]:
            continue
        s = tree.by_id[t["req"]]
        colour = tree.colour if tree.rank(s["id"]) >= s["ranks"] else ARROW_OFF
        xs, ys = pos(s["tier"], s["col"])
        xt, yt = pos(t["tier"], t["col"])
        if s["col"] == t["col"]:
            start = ys + 3
        else:
            mid = ys + 1
            if t["col"] < s["col"]:
                q.set(xt + 2, mid, [LR], colour)
                q.set(xt + 3, mid, BOT_HALF, colour)
                for x in range(xt + 4, xs - 1):
                    q.set(x, mid, BOT_HALF, colour)
            else:
                for x in range(xs + 7, xt + 2):
                    q.set(x, mid, BOT_HALF, colour)
                q.set(xt + 2, mid, BOT_HALF, colour)
                q.set(xt + 3, mid, [LL], colour)
            start = mid + 1
        for y in range(start, yt - 1):
            q.set(xt + 2, y, RIGHT_HALF, colour)
            q.set(xt + 3, y, LEFT_HALF, colour)
        heads.append((xt + 2, yt - 1, colour))


def put_heads(g, heads):
    for x, y, colour in heads:
        g.c[y][x] = ["◥", colour, PANEL, False]
        g.c[y][x + 1] = ["◤", colour, PANEL, False]


def wrap(s, w):
    return textwrap.wrap(s, w) or [""]


# ---------------------------------------------------------------- the frame

def body(w, h):
    top = 5 if h < 40 else 10
    return top, h - 3   # body rows top..bot-1; the status strip at bot, hints at bot+1


def base(w, h, hint):
    g = Grid.from_svg(S + "/shots/tmp_talents_%dx%d.svg" % (w, h), w, h)
    top, bot = body(w, h)
    for y in range(top, h - 1):
        for x in range(1, w - 1):
            g.c[y][x] = [" ", TEXT, PANEL, False]
    x = 2
    for part, fg in hint:
        g.put(x, h - 2, part, fg)
        x += len(part)
    return g


HINT = [("←→↑↓", TEXT), (" move · ", DIM), ("[ ]", TEXT), (" trees · ", DIM), ("space", TEXT),
        (" learn · ", DIM), ("Q/E", TEXT), (" tabs · ", DIM), ("0-8", TEXT), (" jump · ", DIM),
        ("tab", TEXT), (" close", DIM)]


def tree_tabs(g, x, y, sel, right, compact=False):
    """`[ ◂  MEAT 0  ▐EDGE 8▌  WRENCH 0 ...  ▸ ]`"""
    g.put(x, y, "[ ◂", DIM)
    x += 5
    for i in range(len(TREES)):
        tr = Tree(i)
        label = "%s %d" % (tr.name, tr.points())
        if i == sel:
            pill = mix(PANEL, tr.colour, 0.30)
            g.put(x, y, "▐", pill)
            g.put(x + 1, y, label, WHITE, bg=pill, bold=True)
            g.put(x + 1 + len(label), y, "▌", pill)
        else:
            g.put(x + 1, y, tr.name, mix(tr.colour, PANEL, 0.25))
            g.put(x + 2 + len(tr.name), y, str(tr.points()), FAINT)
        x += len(label) + (3 if compact else 4)
    g.put(x - 1, y, "▸ ]", DIM)
    return x + 2


# ---------------------------------------------------------------- A · ARMORY

def concept_a(w, h, sel, cursor):
    tree = Tree(sel)
    g = base(w, h, HINT)
    top, bot = body(w, h)
    q = Quads()
    heads = []
    big = h >= 40
    pitch = 13 if big else 12
    x0 = 8
    vgap = 2 if big else 1
    y0 = top + (3 if big else 2)

    def pos(tier, col):
        return x0 + pitch * col, y0 + (3 + vgap) * tier

    tree_tabs(g, 2, top, sel, w)
    pts = "%d points left" % POINTS_LEFT
    g.put(w - 3 - len(pts), top, pts, CYAN, bold=True)
    if big:
        g.put(2, top + 1, tree.name, tree.colour, bold=True)
        g.put(3 + len(tree.name), top + 1, "· " + tree.line, FAINT)

    # Points needed, down the left, lit once the tree holds them.
    have = tree.points()
    for tier in range(tree.tiers):
        _, y = pos(tier, 0)
        need = GATE * tier
        g.put(2, y + 1, "%2d" % need, tree.colour if have >= need else FAINTER, bold=have >= need)

    for t in tree.talents:
        x, y = pos(t["tier"], t["col"])
        draw_icon(g, x, y, tree, t)
        st = tree.state(t)
        on = (t["tier"], t["col"]) == cursor
        colour = WHITE if on else {"max": AMBER, "avail": GOOD, "part": GOOD, "locked": OFF}[st]
        # At 100x30 the tiers sit a row apart: frames there are side bars only, so the
        # icons stay apart; the cursor alone gets the whole frame.
        frame(q, x, y, colour, top=big or on, bottom=big or on)
    arrows(g, q, tree, pos, heads)
    q.flush(g)
    put_heads(g, heads)
    for t in tree.talents:
        x, y = pos(t["tier"], t["col"])
        badge(g, x + 7, y + 2, tree, t, (t["tier"], t["col"]) == cursor)

    # The panel on the right: the talent under the cursor.
    div = x0 + pitch * 3 + 12
    for y in range(top + 1, bot):
        g.put(div, y, "│", RULE)
    px = div + 3
    pw = w - 2 - px
    t = tree.at(*cursor)
    panel_a(g, px, top + 2, pw, bot - 1, tree, t, big)
    return g


def panel_a(g, x, y, w, ybot, tree, t, big):
    q = Quads()
    draw_icon(g, x + 1, y, tree, t)
    st = tree.state(t)
    frame(q, x + 1, y, {"max": AMBER, "avail": GOOD, "part": GOOD, "locked": OFF}[st])
    q.flush(g)
    nx = x + 10
    g.put(nx, y, t["name"].upper(), tree.colour, bold=True)
    rank = tree.rank(t["id"])
    rc = {"max": AMBER, "avail": GOOD, "part": GOOD, "locked": FAINTER}[st]
    g.put(nx, y + 1, "rank %d/%d" % (rank, t["ranks"]), rc, bold=True)
    g.put(nx + 8, y + 1, " · tier %d" % (t["tier"] + 1), FAINT)
    g.put(nx, y + 2, tree.name + (" · new" if t["new"] else ""), FAINT)
    yy = y + 4 + (1 if big else 0)
    for line in wrap(t["desc"], w):
        g.put(x, yy, line, TEXT)
        yy += 1
    yy += 1
    if rank < t["ranks"]:
        g.put(x, yy, "NEXT RANK", DIM, bold=True)
        yy += 1
        for line in wrap(t["words"][rank], w - 2):
            g.put(x + 2, yy, line, TEXT)
            yy += 1
    if rank:
        g.put(x, yy, "NOW", DIM, bold=True)
        yy += 1
        held = " · ".join(t["words"][:rank])
        for line in wrap(held, w - 2):
            g.put(x + 2, yy, line, FAINT)
            yy += 1
    needs = tree.needs(t)
    if needs:
        yy += 1
        g.put(x, yy, "NEEDS", DIM, bold=True)
        yy += 1
        for ok, words in needs:
            g.put(x + 2, yy, ("✓ " if ok else "✗ ") + words, GOOD if ok else RED)
            yy += 1
    yy += 1
    g.put(x, yy, "LINE", DIM, bold=True)
    yy += 1
    chain = tree.chain(t)
    lx = x + 2
    for i, c in enumerate(chain):
        word = c["name"]
        if i:
            if lx + 3 + len(word) > x + w:
                yy += 1
                lx = x + 2
            g.put(lx, yy, " ▸ " if lx > x + 2 else "▸ ", FAINTER)
            lx += 3 if lx > x + 2 else 2
        fg = WHITE if c is t else (tree.colour if tree.rank(c["id"]) >= c["ranks"] else FAINT)
        g.put(lx, yy, word, fg, bold=c is t)
        lx += len(word)
    # The key, on the panel's last row.
    if st == "avail":
        g.put(x, ybot, "space", WHITE, bg=BAR, bold=True)
        g.put(x + 6, ybot, "learn rank %d" % (rank + 1), TEXT)
    elif st == "max":
        g.put(x, ybot, "at its best", AMBER)
    else:
        g.put(x, ybot, "out of reach", FAINTER)


# ---------------------------------------------------------------- B · BOARD

def concept_b(w, h, sel, cursor):
    tree = Tree(sel)
    hint = [("←→↑↓", TEXT), (" move · ", DIM), ("[ ]", TEXT), (" trees · ", DIM), ("space", TEXT),
            (" burn a point · ", DIM), ("Q/E", TEXT), (" tabs · ", DIM), ("tab", TEXT), (" close", DIM)]
    g = base(w, h, hint)
    top, bot = body(w, h)
    board = mix(PANEL, tree.colour, 0.035)
    dots = mix(PANEL, tree.colour, 0.14)
    bx0, bx1 = 15, 66
    for y in range(top, bot):
        for x in range(bx0, bx1):
            g.c[y][x] = [" ", TEXT, board, False]
            if (x - bx0) % 4 == 2 and (y - top) % 2 == 1:
                g.c[y][x] = ["·", dots, board, False]

    # The trees, down the left: a slot each with the points in it.
    yy = top
    g.put(2, yy, "TREES", DIM, bold=True)
    g.put(9, yy, "[ ]", FAINTER)
    yy += 2
    for i in range(len(TREES)):
        tr = Tree(i)
        p = tr.points()
        if i == sel:
            for x in range(1, 14):
                g.c[yy][x][2] = mix(PANEL, tr.colour, 0.22)
                g.c[yy + 1][x][2] = mix(PANEL, tr.colour, 0.22)
            g.put(1, yy, "▶", tr.colour, bold=True)
        g.put(2, yy, tr.name, tr.colour if i == sel else mix(tr.colour, PANEL, 0.3), bold=i == sel)
        g.put(12 - len(str(p)), yy, str(p), WHITE if i == sel else FAINT, bold=i == sel)
        fill = min(6, p // 2)
        g.put(2, yy + 1, "▰" * fill, tr.colour if i == sel else mix(tr.colour, PANEL, 0.4))
        g.put(2 + fill, yy + 1, "▱" * (6 - fill), FAINTER)
        g.put(9, yy + 1, "/12", FAINTER)
        yy += 3
    yy += 1
    g.put(2, yy, "%d to burn" % POINTS_LEFT, CYAN, bold=True)

    # The board.
    x0 = bx0 + 7
    pitch = 11
    y0 = top + 2

    def pos(tier, col):
        return x0 + pitch * col, y0 + 4 * tier

    g.put(bx0 + 1, top, tree.name, tree.colour, bold=True)
    g.put(bx0 + 2 + len(tree.name), top, "· " + tree.line, FAINT)
    # The power rail: lit down to the last open tier.
    have = tree.points()
    rail_x = bx0 + 1
    open_to = max(t for t in range(tree.tiers) if have >= GATE * t)
    _, last_y = pos(open_to, 0)
    for y in range(y0 - 1, pos(tree.tiers - 1, 0)[1] + 3):
        lit = y <= last_y + 1
        g.c[y][rail_x] = ["┃", tree.colour if lit else ARROW_OFF, board, False]
    for tier in range(tree.tiers):
        _, y = pos(tier, 0)
        need = GATE * tier
        lit = have >= need
        c = tree.colour if lit else ARROW_OFF
        g.c[y + 1][rail_x] = ["┣", c, board, False]
        g.put(rail_x + 1, y + 1, "━", c, bg=board)
        g.put(rail_x + 2, y + 1, "%2d" % need, c if lit else FAINTER, bg=board, bold=lit)

    # Traces, under the chips.
    for t in tree.talents:
        if not t["req"]:
            continue
        s = tree.by_id[t["req"]]
        powered = tree.rank(s["id"]) >= s["ranks"]
        c = tree.colour if powered else ARROW_OFF
        xs, ys = pos(s["tier"], s["col"])
        xt, yt = pos(t["tier"], t["col"])
        cx = xt + 2
        if s["col"] == t["col"]:
            y = ys + 3
        else:
            mid = ys + 1
            if t["col"] < s["col"]:
                g.put(cx, mid, "┌", c, bg=board)
                for x in range(cx + 1, xs - 1):
                    g.put(x, mid, "─", c, bg=board)
                g.put(xs - 2, mid, "●", c, bg=board)
            else:
                for x in range(xs + 8, cx):
                    g.put(x, mid, "─", c, bg=board)
                g.put(cx, mid, "┐", c, bg=board)
            y = mid + 1
        while y < yt - 1:
            g.put(cx, y, "│", c, bg=board)
            y += 1
        g.put(cx, yt - 1, "▼", c, bg=board)

    for t in tree.talents:
        x, y = pos(t["tier"], t["col"])
        draw_icon(g, x, y, tree, t)
        rank = tree.rank(t["id"])
        st = tree.state(t)
        pin = tree.colour if rank else (DIM if st == "avail" else ARROW_OFF)
        for j in range(3):
            g.put(x - 1, y + j, "┤", pin, bg=board)
            g.put(x + 6, y + j, "├", pin, bg=board)
        # Rank LEDs beside the chip, bottom up.
        for r in range(t["ranks"]):
            on = r < rank
            led = (AMBER if st == "max" else tree.colour) if on else (GOOD if st == "avail" and r == rank else FAINTER)
            g.put(x + 7, y + 2 - r, "●" if on else "○", led, bg=board, bold=on)
        if (t["tier"], t["col"]) == cursor:
            for cx, cy, ch in [(x - 2, y - 1, "┏"), (x + 8, y - 1, "┓"), (x - 2, y + 3, "┗"), (x + 8, y + 3, "┛")]:
                g.put(cx, cy, ch, WHITE, bg=board, bold=True)

    # The readout.
    px = bx1 + 2
    pw = w - 2 - px
    t = tree.at(*cursor)
    rank = tree.rank(t["id"])
    st = tree.state(t)
    y = top
    g.put(px, y, "> " + t["id"] + ".tal", tree.colour, bold=True)
    y += 1
    leds = "".join("●" if r < rank else "○" for r in range(t["ranks"]))
    g.put(px + 2, y, "%s %s · tier %d" % (leds, "%d/%d" % (rank, t["ranks"]), t["tier"] + 1), DIM)
    y += 1
    g.put(px, y, "─" * pw, RULE)
    y += 1
    g.put(px, y, t["name"].upper(), WHITE, bold=True)
    y += 1
    for line in wrap(t["desc"], pw):
        g.put(px, y, line, FAINT)
        y += 1
    y += 1
    if rank < t["ranks"]:
        g.put(px, y, "NEXT", DIM, bold=True)
        for line in wrap(t["words"][rank], pw - 6):
            g.put(px + 6, y, line, TEXT)
            y += 1
    if rank:
        g.put(px, y, "NOW", DIM, bold=True)
        for line in wrap(" · ".join(t["words"][:rank]), pw - 6):
            g.put(px + 6, y, line, FAINT)
            y += 1
    for i, (ok, words) in enumerate(tree.needs(t)):
        if i == 0:
            g.put(px, y, "NEED", DIM, bold=True)
        g.put(px + 6, y, ("✓ " if ok else "✗ ") + words, GOOD if ok else RED)
        y += 1
    y += 1
    g.put(px, y, "LINE", DIM, bold=True)
    for c in tree.chain(t):
        fg = WHITE if c is t else (tree.colour if tree.rank(c["id"]) >= c["ranks"] else FAINT)
        g.put(px + 6, y, ("▸ " if c is t else "  ") + c["name"], fg, bold=c is t)
        y += 1
    if st == "avail":
        g.put(px, bot - 1, "[space]", WHITE, bg=BAR, bold=True)
        g.put(px + 8, bot - 1, "burn a point", TEXT)
    return g


# ---------------------------------------------------------------- C · SHOWCASE

def emblem_edge(g, cx, cy, rx, ry, colour, keep):
    """A big crosshair behind the tree, in pixels: two to a cell, stacked."""
    W, H = g.w, g.h * 2
    px = {}
    for py in range(H):
        for x in range(W):
            dx = (x + 0.5 - cx) / rx
            dy = (py + 0.5 - cy * 2) / (ry * 2)
            d = (dx * dx + dy * dy) ** 0.5
            on = 0.86 < d < 1.0 or 0.40 < d < 0.47
            if abs(x + 0.5 - cx) < 0.6 and 0.25 < abs(dy) < 1.18:
                on = True
            if abs(py + 0.5 - cy * 2) < 0.6 and 0.12 < abs(dx) < 1.12:
                on = True
            if on:
                px[(x, py)] = colour
    for y in range(g.h):
        for x in range(g.w):
            if not keep(x, y):
                continue
            t, b = px.get((x, 2 * y)), px.get((x, 2 * y + 1))
            ground = g.c[y][x][2]
            if t and b:
                g.c[y][x] = [" ", t, t, False]
            elif t:
                g.c[y][x] = ["▀", t, ground, False]
            elif b:
                g.c[y][x] = ["▄", b, ground, False]


def concept_c(w, h, sel, cursor):
    tree = Tree(sel)
    hint = [("←→↑↓", TEXT), (" move · ", DIM), ("[ ]", TEXT), (" trees · ", DIM), ("space", TEXT),
            (" learn · ", DIM), ("Q/E", TEXT), (" tabs · ", DIM), ("tab", TEXT), (" close", DIM)]
    g = base(w, h, hint)
    top, bot = body(w, h)
    big = h >= 40
    pitch = 12
    span = pitch * 3 + 10
    x0 = 1 + (w - 2 - span) // 2
    vgap = 2 if big else 1
    y0 = top + (3 if big else 2)

    def pos(tier, col):
        return x0 + pitch * col, y0 + (3 + vgap) * tier

    # Title row.
    g.put(2, top, tree.name, tree.colour, bold=True)
    g.put(3 + len(tree.name), top, "· " + tree.line, FAINT)
    right = "%d in %s · " % (tree.points(), tree.name)
    pts = "%d points left" % POINTS_LEFT
    g.put(w - 3 - len(pts), top, pts, CYAN, bold=True)
    g.put(w - 3 - len(pts) - len(right), top, right, DIM)

    # The tree's emblem, faint, behind everything.
    ey0 = pos(0, 0)[1]
    ey1 = pos(tree.tiers - 1, 0)[1] + 3
    emblem_edge(g, x0 + span / 2 - 2, (ey0 + ey1) / 2, span * 0.62, (ey1 - ey0) / 2 + 0.6,
                mix(PANEL, tree.colour, 0.075), lambda x, y: top < y < bot - 1 and 1 <= x < w - 1)

    q = Quads()
    heads = []
    arrows(g, q, tree, pos, heads)
    on = tree.at(*cursor)
    cx, cy = pos(*cursor)
    frame(q, cx, cy, WHITE)
    q.flush(g)
    put_heads(g, heads)
    for t in tree.talents:
        x, y = pos(t["tier"], t["col"])
        draw_icon(g, x, y, tree, t)
        badge(g, x + 7, y + 2, tree, t, t is on)

    # Tree tabs along the bottom, as the old trees had them.
    tabs_w = sum(len("%s %d" % (Tree(i).name, Tree(i).points())) + 4 for i in range(len(TREES))) + 4
    tree_tabs(g, 1 + (w - 2 - tabs_w) // 2, bot - 1, sel, w)

    # The tooltip, beside the cursor, on whichever side has more room.
    t = on
    rank = tree.rank(t["id"])
    st = tree.state(t)
    tw = 24 if not big else 30
    left = cursor[1] <= 1
    tx = (cx - 3 - tw) if left else (cx + 11)
    tx = max(1, min(w - 1 - tw, tx))
    lines = []
    lines.append([(t["name"], WHITE, True)])
    rc = {"max": AMBER, "avail": GOOD, "part": GOOD, "locked": FAINTER}[st]
    lines.append([("rank %d/%d" % (rank, t["ranks"]), rc, True), ("  tier %d" % (t["tier"] + 1), FAINT, False)])
    for ln in wrap(t["desc"], tw - 4):
        lines.append([(ln, AMBER, False)])
    lines.append([])
    if rank < t["ranks"]:
        lines.append([("next rank:", DIM, False)])
        for ln in wrap(t["words"][rank], tw - 4):
            lines.append([(ln, TEXT, False)])
    for ok, words in tree.needs(t):
        if not ok:
            for ln in wrap("needs " + words, tw - 4):
                lines.append([(ln, RED, False)])
    if st == "avail":
        lines.append([])
        lines.append([("space", WHITE, True), (" learns it", DIM, False)])
    th = len(lines) + 2
    ty = max(top + 1, min(bot - 1 - th, cy + 1 - th // 2))
    border = tree.colour
    for j in range(th):
        for i in range(tw):
            g.c[ty + j][tx + i] = [" ", TEXT, "#0a0f16", False]
    g.put(tx, ty, "┌" + "─" * (tw - 2) + "┐", border)
    g.put(tx, ty + th - 1, "└" + "─" * (tw - 2) + "┘", border)
    for j in range(1, th - 1):
        g.put(tx, ty + j, "│", border)
        g.put(tx + tw - 1, ty + j, "│", border)
    for j, ln in enumerate(lines):
        x = tx + 2
        for s, fg, bold in ln:
            g.put(x, ty + 1 + j, s, fg, bold=bold)
            x += len(s)
    # A pointer from the tooltip to the icon.
    py = cy + 1
    if left:
        for x in range(tx + tw, cx - 1):
            g.put(x, py, "─", border)
        g.put(tx + tw - 1, py, "├", border)
    else:
        for x in range(cx + 10, tx):
            g.put(x, py, "─", border)
        g.put(tx, py, "┤", border)
    return g


# ---------------------------------------------------------------- the page

def shot(g, scale=1.0):
    return grid_html(g)


def build(out):
    edge = 1
    shots = {
        "a": concept_a(100, 30, edge, (2, 0)),
        "a_locked": concept_a(100, 30, edge, (2, 1)),
        "a_big": concept_a(120, 40, edge, (2, 0)),
        "b": concept_b(100, 30, edge, (2, 0)),
        "b_big": concept_b(120, 40, edge, (2, 1)),
        "c": concept_c(100, 30, edge, (2, 0)),
        "c_big": concept_c(120, 40, edge, (3, 0)),
    }
    return shots


if __name__ == "__main__":
    shots = build(sys.argv[2])
    import json
    json.dump({"css": CSS, "shots": {k: shot(v) for k, v in shots.items()},
               "text": {k: v.text() for k, v in shots.items()}}, open(sys.argv[2], "w"))
