"""A shell opened on the Net, mockups round 1: one style for every terminal you reach
from the Net, in place of the street machine's CRT. Drawn over the game's own frame
(seed 8, a street terminal by Kiroflux Mart held, its shell opened with `r`).

    python3 -I shell.py <shots dir> <out.html>

The shots dir holds net_shell.svg (today) and net_shell_bg.svg (the Net behind it),
from the `net_shell` shot test.

now · the street kiosk's CRT, bezel and all, floating over cyberspace.
A · jack window: a thin frame in the Net's signal colour, the host on its edge, the
    hold's time; a handshake instead of a boot.
B · on the grid: no window; the Net dims and the shell is text laid on it, a gutter
    rule and the host above; the most "inside the machine".
C · data pane: the shell takes the right of the map, full height, framed like the
    construct card; the map stays live on the left with you and the node."""

import html
import os
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, os.path.join(HERE, "..", "2026-10-19-gear-inventory-mock"))
sys.path.insert(0, os.path.join(HERE, "..", "2026-11-02-minimap-mock"))
from cells import Grid  # noqa: E402
import term  # noqa: E402

S, OUT = sys.argv[1], sys.argv[2]

SIGNAL = "#00ffc8"
CYAN = "#00e5ff"
MAGENTA = "#ff00c8"
WHITE = "#e6f0f5"
TEXT = "#cdd4de"
DIM = "#5f8a86"
VOID = "#010204"
GLASS = "#020a0c"
HELD = "#50e678"

HOST = "stack-101"
LINES = [
    ("jack › stack-101.net · STACK 101 SUBNET", SIGNAL),
    ("handshake ·········· ok", DIM),
    ("seat: user · for the hold's hour", DIM),
    ("HELIX/OS 4.1", DIM),
    ("Authorised users only. Sessions are logged.", TEXT),
    ("", TEXT),
    ("user@stack-101:/$ ls /net", WHITE),
    ("kiroflux-mart   velform-group-plaza   stack-101-terminal", TEXT),
    ("lumnet-industries   novflux-goods   velform-fab", TEXT),
]
PROMPT = "user@stack-101:/$ "


def rgb(c):
    c = c.lstrip("#")
    return tuple(int(c[i:i + 2], 16) for i in (0, 2, 4))


def hexc(t):
    return "#%02x%02x%02x" % tuple(max(0, min(255, int(v))) for v in t)


def scale(c, f):
    return hexc(tuple(v * f for v in rgb(c)))


def mix(a, b, f):
    return hexc(tuple(x + (y - x) * f for x, y in zip(rgb(a), rgb(b))))


def frame(name):
    return Grid.from_svg(os.path.join(S, name), 120, 40)


def dim(g, f, x0=0, y0=0, w=120, h=40):
    for y in range(y0, y0 + h):
        for x in range(x0, x0 + w):
            c = g.c[y][x]
            c[1] = scale(c[1], f)
            c[2] = scale(c[2], f)


def box(g, x, y, w, h, fg, bg, light=True):
    tl, tr, bl, br, hz, vt = ("┌", "┐", "└", "┘", "─", "│") if light else ("╔", "╗", "╚", "╝", "═", "║")
    g.fill(x, y, w, h, bg)
    g.put(x, y, tl + hz * (w - 2) + tr, fg, bg)
    for j in range(1, h - 1):
        g.put(x, y + j, vt, fg, bg)
        g.put(x + w - 1, y + j, vt, fg, bg)
    g.put(x, y + h - 1, bl + hz * (w - 2) + br, fg, bg)


# The terminal node in the frame, and the CRT's footprint today.
NODE = None
X, Y, W, H = 24, 8, 72, 24


def find_node(g):
    """The held terminal: the cell on the landed run's green glow, in the map."""
    for y in range(5, 37):
        for x in range(0, 80):
            if g.c[y][x][2].lower() == "#0f3a1f" and g.c[y][x][0] not in " ─━":
                return (x, y)
    return (20, 21)


def option_a():
    g = frame("net_shell_bg.svg")
    dim(g, 0.55)
    box(g, X, Y, W, H, SIGNAL, GLASS)
    g.put(X + 2, Y, " ◇ JACK · STACK 101 SUBNET ", SIGNAL, GLASS, bold=True)
    g.put(X + W - 22, Y, " user · HELD 59m ", HELD, GLASS)
    # faint lattice on the glass
    for yy in range(Y + 1, Y + H - 1, 2):
        for xx in range(X + 3, X + W - 2, 6):
            g.put(xx, yy, "·", scale(SIGNAL, 0.18), GLASS)
    for i, (line, fg) in enumerate(LINES):
        g.put(X + 3, Y + 2 + i, line, fg, GLASS, bold=line.startswith("jack"))
    g.put(X + 3, Y + 2 + len(LINES) + 1, PROMPT + "▮", WHITE, GLASS, bold=True)
    g.put(X + 2, Y + H - 1, " esc · back to the grid ", DIM, GLASS)
    return g


def option_b():
    g = frame("net_shell_bg.svg")
    dim(g, 0.28, 0, 4, 80, 32)
    x, y = X + 2, Y + 1
    g.put(x, y, "◇ " + HOST + ".net", SIGNAL, bold=True)
    g.put(x + 20, y, "· STACK 101 SUBNET · user · held 59m", DIM)
    for i in range(y + 2, y + 2 + len(LINES) + 3):
        g.put(x - 2, i, "▏", scale(SIGNAL, 0.7))
    for i, (line, fg) in enumerate(LINES):
        g.put(x, y + 2 + i, line, fg, bold=line.startswith("jack"))
    g.put(x, y + 3 + len(LINES), PROMPT + "▮", WHITE, bold=True)
    return g


def option_c():
    g = frame("net_shell_bg.svg")
    px, py, pw, ph = 40, 4, 40, 32
    box(g, px, py, pw, ph, CYAN, "#05080d")
    g.put(px + 2, py, " ◇ " + HOST.upper() + " ", CYAN, "#05080d", bold=True)
    g.put(px + 2, py + 1, "STACK 101 SUBNET · user · held 59m", DIM, "#05080d")
    g.put(px + 1, py + 2, "─" * (pw - 2), "#283c4b", "#05080d")
    wrapped = []
    for line, fg in LINES:
        while len(line) > pw - 4:
            cut = line.rfind(" ", 0, pw - 4)
            cut = cut if cut > 0 else pw - 4
            wrapped.append((line[:cut], fg))
            line = line[cut:].lstrip()
        wrapped.append((line, fg))
    for i, (line, fg) in enumerate(wrapped):
        g.put(px + 2, py + 4 + i, line, fg, "#05080d", bold=line.startswith("jack"))
    g.put(px + 2, py + 5 + len(wrapped), PROMPT + "▮", WHITE, "#05080d", bold=True)
    g.put(px + 1, py + ph - 2, "─" * (pw - 2), "#283c4b", "#05080d")
    g.put(px + 2, py + ph - 1, " esc · back to the grid ", DIM, "#05080d")
    return g


OPTIONS = [
    ("now", "Today: the district's street kiosk, a CRT with its maker's bezel, LED and boot "
            "(“MEMORY CHECK OK”), hanging over cyberspace.", frame("net_shell.svg")),
    ("A · jack window", "Same size and place as today, but the Net's own: a thin frame in "
                        "the signal colour, the host on its edge and the hold's time on the "
                        "right, a faint lattice on the glass. A handshake instead of a boot. "
                        "The Net dims behind.",
     option_a()),
    ("B · on the grid", "No window at all: the Net dims and the shell is text laid on it, a "
                        "signal rule down its left and the host above. The most “inside the "
                        "machine”; least like a screen.", option_b()),
    ("C · data pane", "The shell takes the right of the map, full height, framed like the "
                      "construct card; the map stays live on the left with you and the node. "
                      "Narrower lines wrap.", option_c()),
]

parts = []
for name, text, g in OPTIONS:
    parts.append(
        f"<section><h2>{html.escape(name)}</h2><p>{html.escape(text)}</p>"
        f"<div class='frame'>{term.render(g, 12.5)}</div></section>"
    )
page = f"""<!doctype html><html><head><meta charset="utf-8"><title>Net shell</title>
<style>{term.CSS}
body{{background:#0b0d12;color:#cdd4de;font-family:-apple-system,system-ui,sans-serif;margin:24px}}
h1{{font-size:20px}} h2{{font-size:16px;color:#00ffc8;margin:28px 0 6px}}
p{{max-width:900px;color:#9aa4b2;margin:0 0 10px}}
.frame{{display:inline-block;border:1px solid #283c4b}}
</style></head><body>
<h1>A shell on the Net · one style for every terminal you reach from the Net</h1>
<p>Seed 8: a street terminal by Kiroflux Mart held from the Net, its shell opened with
<code>r</code>. Pick one; the street's kiosks keep their CRTs.</p>
{''.join(parts)}
</body></html>"""
open(OUT, "w").write(page)
print("wrote", OUT)
