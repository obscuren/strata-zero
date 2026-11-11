"""Meridian Clearing, the one site every look draws: an office of 12 floors whose
mission copy holds L, 7F, 12F and B1. Every floor shares the core: the hub closet
(x 10-17), the lift lobby (x 19-31) with two lifts, and the stair core (x 44-48).

Plan characters:
  #  wall              .  floor            +  plain door        o  main door
  =  window            K  card door        P  keypad door       T  turnstile
  G  gate switch       c  camera           L  lift              S  stair
  t  terminal          v  server           X  data vault        V  safe
  r  turret            h  floor hub        d  desk              R  rack
  g  guard (meat)      @  Vex's body
"""

W, H = 50, 13
STREET_WALL = "####=====###=====####oooooooo####=====###=====####"
UPPER_WALL = "####=====###=====####========####=====###=====####"


def base():
    g = [["." for _ in range(W)] for _ in range(H)]
    for x in range(W):
        g[0][x] = g[12][x] = g[5][x] = "#"
    for y in range(H):
        g[y][0] = g[y][W - 1] = "#"
    for x in (9, 18, 32, 43):
        for y in range(1, 5):
            g[y][x] = "#"
    g[1][23] = g[1][27] = "L"
    g[2][13] = "h"
    g[2][46] = "S"
    g[3][18] = "K"
    return g


def put(g, pts, ch):
    for x, y in pts:
        g[y][x] = ch


def row(g, y, s):
    for x, ch in enumerate(s):
        g[y][x] = ch


def lobby():
    g = base()
    row(g, 12, STREET_WALL)
    put(g, [(4, 5)], "+")
    put(g, [(x, 5) for x in range(22, 27)], "T")
    put(g, [(27, 5)], "G")
    put(g, [(46, 5)], "K")
    put(g, [(32, 3)], "K")
    put(g, [(1, 1), (31, 1), (1, 6), (48, 9)], "c")
    put(g, [(x, 8) for x in range(4, 10)] + [(4, 9), (9, 9)], "d")
    put(g, [(7, 9), (25, 3)], "g")
    put(g, [(43, 10)], "t")
    put(g, [(44, 10)], "@")
    labels = [(5, 3, "MAIL"), (35, 2, "OFFICE"), (16, 10, "LOBBY")]
    return g, labels


def security():
    g = base()
    row(g, 12, UPPER_WALL)
    put(g, [(4, 5), (46, 5)], "K")
    put(g, [(25, 5), (37, 5)], "+")
    put(g, [(3, 3)], "t")
    put(g, [(6, 3)], "v")
    put(g, [(6, 4), (20, 10)], "g")
    put(g, [(34, 1), (1, 6), (48, 10)], "c")
    for y in (7, 9):
        put(g, [(x, y) for x in range(3, 44) if (x - 3) % 4 < 2], "d")
    labels = [(3, 1, "SEC"), (36, 2, "BREAK"), (17, 11, "OPEN PLAN")]
    return g, labels


def executive():
    g = base()
    row(g, 12, UPPER_WALL)
    put(g, [(4, 5), (25, 5), (37, 5)], "+")
    put(g, [(46, 5)], "K")
    put(g, [(x, 8) for x in range(1, 49)], "#")
    put(g, [(22, y) for y in range(9, 12)], "#")
    put(g, [(14, 8)], "P")
    put(g, [(34, 8)], "+")
    put(g, [(1, 1), (1, 6), (1, 11), (48, 11)], "c")
    put(g, [(11, 6)], "r")
    put(g, [(19, 10)], "V")
    labels = [(2, 2, "BOARD"), (35, 2, "ASSIST"), (4, 10, "EXEC"),
              (30, 10, "LOUNGE")]
    return g, labels


def servers():
    g = base()
    row(g, 12, "#" * W)
    put(g, [(4, 5)], "+")
    put(g, [(28, 5), (46, 5), (32, 3)], "K")
    put(g, [(25, y) for y in range(6, 12)], "#")
    put(g, [(25, 8)], "K")
    for y in (7, 9):
        put(g, [(x, y) for x in range(3, 21)], "R")
    put(g, [(7, 10)], "X")
    put(g, [(1, 6), (26, 6)], "c")
    put(g, [(39, 8)], "t")
    labels = [(3, 2, "PLANT"), (35, 2, "STORE"), (35, 10, "NOC"),
              (9, 11, "SERVERS")]
    return g, labels


def make(key, name, build, **kw):
    g, labels = build()
    return dict(key=key, name=name, plan=["".join(r) for r in g], labels=labels, **kw)


LEVELS = [
    make("12F", "executive", executive, ice=[("grey", (26, 7))]),
    make("7F", "security", security, ice=[("white", (30, 8))]),
    make("L", "lobby", lobby, ice=[("white", (31, 7))]),
    make("B1", "servers", servers, ice=[("black", (9, 10))], gap=True),
]
BY_KEY = {lv["key"]: lv for lv in LEVELS}
PASSED = {"12F": "8F–11F", "7F": "2F–6F"}  # floors under each that the lift passes

KIND = {
    "c": "camera", "K": "card door", "P": "keypad", "G": "gate switch", "L": "lift",
    "t": "terminal", "v": "server", "X": "data vault", "V": "safe", "r": "turret",
    "h": "hub",
}

# What Vex holds at this moment, by level and plan position.
HELD = {
    ("L", 27, 5): "cracked",   # the gate switch: the turnstiles let her through
    ("L", 48, 9): "looped",    # the lobby camera on the way to the lifts
    ("L", 27, 1): "held",      # the goods lift, told to stop at B1
    ("L", 18, 3): "cracked",   # the hub closet's door: the way up the riser
}
JACKED = ("L", 43, 10)        # the kiosk terminal she is jacked into
YOU_NET = ("L", 20, 4)        # where she stands in the construct, in the lift lobby
FIREWALL = ["locked", "locked", "locked"]


def nodes(level):
    """Every device on a level's plan: (x, y, kind)."""
    out = []
    for y, r in enumerate(level["plan"]):
        for x, ch in enumerate(r):
            if ch in KIND:
                out.append((x, y, KIND[ch]))
    return out


def state(level, x, y):
    return HELD.get((level["key"], x, y))


if __name__ == "__main__":
    for lv in LEVELS:
        print(lv["key"])
        print("\n".join(lv["plan"]))
        print(nodes(lv))


# ------------------------------------------------------------------ bigger floors (D)
# Every floor stretched the same way, so a column means the same x on every level and
# the lifts and the riser line up through the tower.

DUP_ROWS = [3, 10]
COPY_SAME = set("#=oTR.d")


def _vertical(lv, x, y):
    p = lv["plan"]
    return p[y][x] in "#KP+G" and (p[y - 1][x] in "#KP+G" or p[y + 1][x] in "#KP+G")


def _pick_cols(n=8):
    cand = [x for x in range(2, W - 2)
            if not any(_vertical(lv, x, y) for lv in LEVELS for y in range(1, H - 1))]
    out = []
    for i in range(n):
        target = (i + 0.5) * W / n
        best = min((c for c in cand if c not in out), key=lambda c: abs(c - target))
        out.append(best)
    return sorted(out)


DUP_COLS = _pick_cols()
W2, H2 = W + len(DUP_COLS), H + len(DUP_ROWS)


def nx(x):
    return x + sum(1 for d in DUP_COLS if d < x)


def ny(y):
    return y + sum(1 for d in DUP_ROWS if d < y)


def _copy(c):
    if c in COPY_SAME:
        return c
    if c in "KP+G":
        return "#"
    return "."


def _stretch(plan):
    rows = []
    for y, r in enumerate(plan):
        out = []
        for x, c in enumerate(r):
            out.append(c)
            if x in DUP_COLS:
                out.append(_copy(c) if c != "T" else "T")
        rows.append("".join(out))
        if y in DUP_ROWS:
            rows.append("".join(_copy(c) for c in rows[-1]))
    return rows


def big(lv):
    return dict(lv, plan=_stretch(lv["plan"]),
                labels=[(nx(x), ny(y), w) for x, y, w in lv["labels"]],
                ice=[(g, (nx(x), ny(y))) for g, (x, y) in lv["ice"]])


BIG = [big(lv) for lv in LEVELS]
BIG_BY_KEY = {lv["key"]: lv for lv in BIG}


def big_pos(key, x, y):
    return (key, nx(x), ny(y))
